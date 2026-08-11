# Blueprint Edge Editing and Reroute Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Phase 2 Blueprint-style edge hit-testing, mutually exclusive edge selection, edge menu/delete/double-click reroute insertion, persisted built-in reroute nodes, compact reroute rendering, and compiler-transparent reroute semantics.

**Architecture:** Rust remains authoritative for graph topology, identity allocation, validation, history, and projection. The frontend sends one high-level intent per edge deletion or reroute insertion and never edits committed topology optimistically. Reroutes remain in `GraphDocument` and editor projections, while a compiler-only semantic normalization pass removes reroute nodes and reconnects value/control/effect dependencies before lowering so no reroute runtime operation is produced.

**Tech Stack:** Rust, Serde, YssBI graph document/registry/compiler, Tauri IPC, React 19, TypeScript, Zustand, SVG, shadcn context-menu primitives, Vitest, Cargo tests through repository-root `pnpm` scripts.

## Global Constraints

- Source design: `docs/superpowers/specs/2026-08-11-blueprint-style-graph-interaction-design.md`, especially sections 3, 6, 8, 11, 12, 15, 16 Phase 2, and 17.
- **Hard dependency:** Phase 1 must already provide atomic `DeleteNodes`, `DisconnectConnections`, `DisconnectPort`, and `DisconnectNode` mutations and must have removed frontend loops over authoritative graph mutations. Task 1 is a blocking acceptance gate; do not implement Phase 2 on the singular `DeleteNode`/`Disconnect` protocol.
- One user-visible graph action produces exactly one high-level mutation, one `GraphDocumentPatch`, one graph revision, and one Rust history entry.
- The frontend sends intent only; Rust allocates reroute node IDs and replacement connection IDs and determines the reroute protocol from the authoritative original edge.
- Do not add a generic composite mutation or compatibility shim for deprecated singular mutation variants.
- Do not optimistically add, remove, or split projected connections in React stores. Install only the authoritative projection returned by the existing mutation coordinator.
- Reroute nodes remain persisted and visible in editor projections but create no runtime identity operation.
- Preserve the original edge `OrderKey` only on the reroute-output-to-original-input connection; the source-to-reroute connection has no order key.
- Undo must restore the original `ConnectionId` and original `OrderKey` exactly.
- Node and connection selections are mutually exclusive. Box selection selects nodes only. A failed mutation preserves selection.
- Keep selection scoped to an editor group because one graph may be visible in multiple editor groups.
- Keep Rust and React boundaries intact: commands stay thin, frontend IPC remains under `src/services/`, and views do not call Tauri `invoke` directly.
- Use the shared context-menu primitives and shared toast system; do not add browser dialogs or a second UI library.
- Keep all global keyboard listeners routed through `src/shared/utils/globalEvent.ts`.
- Run commands from the repository root through `pnpm`; do not create `src-tauri/target/` through ad-hoc Cargo invocations.
- For Rust changes, run focused tests and `pnpm rust:check`. For frontend changes, run focused Vitest tests and `pnpm typecheck`. Because this phase spans frontend and Rust, run `pnpm verify` before delivery and `git diff --check` last.
- Do not commit during this plan unless the user explicitly requests a commit. Every task ends with a review checkpoint instead of a commit step.
- Preserve unrelated working-tree changes, including the existing user modification to `TODO.md`.

---

### Task 1: Enforce the Phase 1 Atomic-Mutation Gate

**Files:**
- Inspect: `src-tauri/src/node_system/document/mutation.rs:262-552`
- Inspect: `src-tauri/src/node_system/document/tests.rs:326-420`
- Inspect: `src/features/core/history/commands/deleteNodes.ts`
- Inspect: `src/features/core/history/commands/disconnectPin.ts`
- Inspect: `src/features/core/history/editorCommands.test.ts:63-221`
- Inspect: `src/shared/types/dto/editorMutation.ts:25-54`
- No Phase 2 file may be modified in this task.

**Interfaces:**
- Consumes: the completed Phase 1 mutation protocol.
- Requires these exact Rust variants:

```rust
DeleteNodes {
    node_ids: Vec<NodeId>,
},
DisconnectConnections {
    connection_ids: Vec<ConnectionId>,
},
DisconnectPort {
    address: PortAddressDto,
},
DisconnectNode {
    node_id: NodeId,
},
```

- Requires these exact TypeScript wire variants:

```ts
| { type: 'deleteNodes'; payload: { nodeIds: string[] } }
| { type: 'disconnectConnections'; payload: { connectionIds: string[] } }
| { type: 'disconnectPort'; payload: { address: PortAddressDto } }
| { type: 'disconnectNode'; payload: { nodeId: string } }
```

- Produces: a documented go/no-go result for Phase 2 execution.

- [ ] **Step 1: Confirm the Rust DTO exposes only the Phase 1 collection/domain variants**

Run:

```sh
git --no-pager grep -n "DeleteNode\|Disconnect" -- src-tauri/src/node_system/document/mutation.rs
```

Expected: matches include `DeleteNodes`, `DisconnectConnections`, `DisconnectPort`, and `DisconnectNode`; no `DeleteNode { node_id }` or singular `Disconnect { connection_id }` variant remains.

- [ ] **Step 2: Confirm frontend commands submit one mutation per user action**

Run:

```sh
git --no-pager grep -n "for (const nodeId\|for (const connectionId" -- src/features/core/history/commands
```

Expected: no match in delete/disconnect command handlers.

- [ ] **Step 3: Run the Phase 1 focused atomic command tests**

Run:

```sh
pnpm test -- src/features/core/history/editorCommands.test.ts
```

Expected: PASS; tests assert one `executeEditorMutation` call for multi-node delete and multi-edge/port disconnect.

Run:

```sh
pnpm rust:test -- disconnect_connections
```

Expected: PASS; empty/duplicate targets are rejected, derived connections are deterministic, and valid requests produce one patch.

Run:

```sh
pnpm rust:test -- delete_nodes
```

Expected: PASS; group deletion is atomic and managed-node failure has zero side effects.

- [ ] **Step 4: Stop if the gate is not satisfied**

If any expected interface is absent or either focused suite fails because the old singular protocol remains, stop this plan without modifying Phase 2 files. Execute the approved design's Phase 1 implementation first. Do not add local Phase 2 compatibility code around the old variants.

- [ ] **Step 5: Review checkpoint**

Review the DTOs and command tests and record that Phase 1 is accepted. Continue only when one-action/one-mutation behavior is proven. Do not commit; only commit if the user explicitly requests it.

---

### Task 2: Register Persisted Built-In Reroute Protocols

**Files:**
- Create: `src-tauri/src/node_system/catalog/core_nodes/reroute.rs`
- Modify: `src-tauri/src/node_system/catalog/core_nodes/mod.rs:1-63`
- Modify: `src-tauri/src/node_system/registry/model.rs:68-117`
- Modify: `src-tauri/src/node_system/compiler/control.rs:50-145`
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs:2678-2687`
- Test: `src-tauri/src/node_system/catalog/core_nodes/reroute.rs` (`#[cfg(test)]` module in the same file)
- Test: `src-tauri/src/node_system/registry/tests.rs:895-960`

**Interfaces:**
- Consumes: `ProviderFragment`, `assembled_interface`, `assembled_parameters`, `RegisteredNode::structural`, `NodeProtocol`, and existing semantic ID constructors.
- Produces these stable IDs and port keys:

```rust
pub(crate) const DATA_REROUTE_NODE_TYPE: &str = "yssbi.reroute.data";
pub(crate) const CONTROL_REROUTE_NODE_TYPE: &str = "yssbi.reroute.control";
pub(crate) const EFFECT_REROUTE_NODE_TYPE: &str = "yssbi.reroute.effect";
pub(crate) const REROUTE_INPUT_PORT: &str = "in";
pub(crate) const REROUTE_OUTPUT_PORT: &str = "out";
```

- Produces this registry role:

```rust
pub enum StructuralNodeRole {
    Sequence,
    Branch,
    Loop,
    Call,
    EventBegin,
    FunctionEntry,
    FunctionReturn,
    Reroute,
}
```

- Produces this registration function:

```rust
pub(crate) fn register(
    fragment: &mut ProviderFragment,
) -> Result<(), BuiltinAssemblyError>;
```

- [ ] **Step 1: Write failing protocol-contract tests**

Add tests in `src-tauri/src/node_system/catalog/core_nodes/reroute.rs` that build the built-in node system and assert:

```rust
#[test]
fn builtin_reroutes_have_stable_transparent_protocols() {
    let system = crate::node_system::catalog::build_builtin_node_system().unwrap();
    let registry = system.registry;

    let data = registry
        .protocol(&NodeTypeId::new(DATA_REROUTE_NODE_TYPE).unwrap())
        .unwrap();
    assert_eq!(data.catalog.style_id.as_str(), "builtin.reroute");
    assert!(data.catalog.hidden);
    assert!(data.parameters.parameters.is_empty());
    assert_eq!(data.interface.type_parameters.len(), 1);
    assert_eq!(data.interface.ports.len(), 2);
    assert_eq!(data.interface.ports[0].key.as_str(), REROUTE_INPUT_PORT);
    assert_eq!(data.interface.ports[0].direction, PortDirection::Input);
    assert_eq!(data.interface.ports[0].kind, PortKind::Data);
    assert_eq!(data.interface.ports[0].connections, ConnectionsPerPort::Single);
    assert_eq!(data.interface.ports[1].key.as_str(), REROUTE_OUTPUT_PORT);
    assert_eq!(data.interface.ports[1].direction, PortDirection::Output);
    assert_eq!(data.interface.ports[1].kind, PortKind::Data);
    assert_eq!(
        data.interface.ports[1].connections,
        ConnectionsPerPort::Multiple { max: None, ordered: false },
    );
    assert_eq!(data.interface.ports[0].value_type, data.interface.ports[1].value_type);

    for (node_type, kind) in [
        (CONTROL_REROUTE_NODE_TYPE, PortKind::Control),
        (EFFECT_REROUTE_NODE_TYPE, PortKind::Effect),
    ] {
        let protocol = registry
            .protocol(&NodeTypeId::new(node_type).unwrap())
            .unwrap();
        assert_eq!(protocol.catalog.style_id.as_str(), "builtin.reroute");
        assert!(protocol.catalog.hidden);
        assert!(protocol.parameters.parameters.is_empty());
        assert!(protocol.interface.type_parameters.is_empty());
        assert_eq!(protocol.interface.ports.len(), 2);
        assert_eq!(protocol.interface.ports[0].kind, kind);
        assert_eq!(protocol.interface.ports[1].kind, kind);
        assert_eq!(protocol.interface.ports[0].connections, ConnectionsPerPort::Single);
        assert_eq!(protocol.interface.ports[1].connections, ConnectionsPerPort::Single);
    }
}
```

Add a registry assertion that all three entries have `StructuralNodeRole::Reroute` and no leaf implementation.

- [ ] **Step 2: Run the new test and verify failure**

Run:

```sh
pnpm rust:test -- builtin_reroutes_have_stable_transparent_protocols
```

Expected: FAIL because the constants, protocols, and `Reroute` structural role do not exist.

- [ ] **Step 3: Implement the three protocols**

Create `reroute.rs` with one shared constructor. Use one generic type parameter for data and no generic parameters for control/effect. Use `LiteralPolicy::Forbidden` on the data input and no parameter schema. Register each protocol with `RegisteredNode::structural(Arc::new(protocol), StructuralNodeRole::Reroute)`.

The shared constructor must produce:

```rust
fn reroute_protocol(
    node_type: &'static str,
    kind: PortKind,
) -> Result<NodeProtocol, BuiltinAssemblyError>;
```

For data ports use:

```rust
let value = sid("value", TypeParameterId::new)?;
let input_type = TypeExpr::Generic(value.clone());
let output_type = TypeExpr::Generic(value.clone());
```

For control/effect ports use `TypeExpr::Unknown`, `ConnectionsPerPort::Single` on the input, and `ConnectionsPerPort::Multiple { max: None, ordered: false }` on the output so reroutes preserve Blueprint-style fan-out.

- [ ] **Step 4: Wire registration and exhaustive role matches**

In `core_nodes/mod.rs`, add:

```rust
mod reroute;
```

and call:

```rust
reroute::register(&mut fragment)?;
```

Add `Reroute` to `StructuralNodeRole`. In `validate_structural_contract`, accept `StructuralNodeRole::Reroute` without applying structured-control requirements; its exact port shape is enforced by its built-in protocol tests and compiler normalization validation. Extend `structural_role_name` with:

```rust
StructuralNodeRole::Reroute => "reroute",
```

Update all exhaustive `StructuralNodeRole` matches to handle `Reroute` explicitly rather than using wildcard branches.

- [ ] **Step 5: Run protocol and registry tests**

Run:

```sh
pnpm rust:test -- builtin_reroutes
```

Expected: PASS; all three protocols are frozen, hidden, style-tagged, and structural without implementations.

Run:

```sh
pnpm rust:test -- registry
```

Expected: PASS; registry form validation accepts reroute as a structural node and still rejects nodes with neither or both executable interpretations.

- [ ] **Step 6: Run Rust compilation check**

Run:

```sh
pnpm rust:check
```

Expected: PASS with no non-exhaustive structural-role match errors.

- [ ] **Step 7: Review checkpoint**

Review the protocol fingerprints, hidden catalog behavior, generic equality, port capacities, and absence of a lowerer. Do not commit; only commit if the user explicitly requests it.

---

### Task 3: Add the Atomic `InsertReroute` Mutation

**Files:**
- Modify: `src-tauri/src/node_system/document/mutation.rs:262-552`
- Modify: `src-tauri/src/node_system/document/mutation.rs:838-1124`
- Modify: `src-tauri/src/node_system/document/tests.rs:326-420`
- Test: `src-tauri/src/node_system/document/tests/editor_mutation_validation.rs`
- Test: `src-tauri/src/project/production_tests.rs:1990-2120`
- Modify: `src/shared/types/dto/editorMutation.ts:25-54`

**Interfaces:**
- Consumes: Task 2 constants and protocols, `GraphDocumentPatch`, `resolve_mutation_port`, `validate_position`, and Phase 1 history transaction path.
- Produces the Rust DTO:

```rust
InsertReroute {
    connection_id: ConnectionId,
    position: NodePosition,
},
```

- Produces the TypeScript DTO:

```ts
| {
    type: 'insertReroute';
    payload: {
      connectionId: string;
      position: NodePositionDto;
    };
  }
```

- Produces planner helpers:

```rust
fn reroute_node_type_for_kind(
    kind: PortKind,
) -> Result<NodeTypeId, MutationConflict>;

fn insert_reroute_operations(
    document: &GraphDocument,
    registry: &NodeRegistry,
    connection_id: ConnectionId,
    position: NodePosition,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict>;
```

- [ ] **Step 1: Add the failing Serde wire test**

Extend `editor_mutation_wire_is_stable_and_camel_case` with:

```rust
(
    EditorGraphMutationDto::InsertReroute {
        connection_id: connection,
        position: NodePosition { x: 12.5, y: -8.0 },
    },
    json!({
        "type": "insertReroute",
        "payload": {
            "connectionId": connection,
            "position": { "x": 12.5, "y": -8.0 }
        }
    }),
),
```

- [ ] **Step 2: Add failing planner tests**

In `editor_mutation_validation.rs`, add fixtures with concrete data, control, and effect edges, then add tests with these assertions:

```rust
#[test]
fn insert_reroute_splits_data_connection_atomically() {
    let (document, registry, original) = reroute_data_fixture(None);
    let patch = plan(
        EditorGraphMutationDto::InsertReroute {
            connection_id: original.id,
            position: NodePosition { x: 40.0, y: 20.0 },
        },
        &document,
        &registry,
    )
    .unwrap();

    assert_eq!(patch.operations.len(), 4);
    assert!(matches!(patch.operations[0], GraphDocumentOperation::RemoveConnection { .. }));
    assert!(matches!(patch.operations[1], GraphDocumentOperation::InsertNode { .. }));
    assert!(matches!(patch.operations[2], GraphDocumentOperation::InsertConnection { .. }));
    assert!(matches!(patch.operations[3], GraphDocumentOperation::InsertConnection { .. }));

    let mut applied = document.clone();
    applied.apply_patch(&patch).unwrap();
    assert!(!applied.connections.contains_key(&original.id));
    assert_eq!(applied.nodes.len(), document.nodes.len() + 1);
    assert_eq!(applied.connections.len(), document.connections.len() + 1);
}
```

Add separate tests named:

```text
insert_reroute_selects_control_and_effect_protocols
insert_reroute_preserves_order_on_downstream_connection
insert_reroute_inverse_restores_original_connection_identity
insert_reroute_rejects_missing_connection_without_side_effects
insert_reroute_rejects_non_finite_position_without_side_effects
```

The inverse test must apply `patch`, then `patch.inverse()`, and assert the complete document topology equals the original apart from the expected revision advancement performed by each `apply_patch` call; compare nodes/connections/bindings/input states directly and assert the original connection object is restored exactly.

- [ ] **Step 3: Run the new mutation tests and verify failure**

Run:

```sh
pnpm rust:test -- insert_reroute
```

Expected: FAIL because `InsertReroute` and its planner are undefined.

- [ ] **Step 4: Implement the DTO and protocol selection**

Add the variant and map kinds exactly:

```rust
fn reroute_node_type_for_kind(
    kind: PortKind,
) -> Result<NodeTypeId, MutationConflict> {
    let value = match kind {
        PortKind::Data => DATA_REROUTE_NODE_TYPE,
        PortKind::Control => CONTROL_REROUTE_NODE_TYPE,
        PortKind::Effect => EFFECT_REROUTE_NODE_TYPE,
    };
    NodeTypeId::new(value).map_err(|error| invalid_editor_mutation(error.to_string()))
}
```

- [ ] **Step 5: Implement the four-operation planner**

The planner must:

1. call `validate_position(position)`;
2. clone the authoritative original connection;
3. resolve and validate both original endpoint ports;
4. choose the reroute type from the original edge kind;
5. allocate one `NodeId` and two fresh `ConnectionId`s in Rust;
6. produce operations in this exact order:

```rust
RemoveConnection(original)
InsertNode(reroute)
InsertConnection(original.output -> reroute.in, order = None)
InsertConnection(reroute.out -> original.input, order = original.order)
```

7. apply the complete patch to a staged `GraphDocument` before returning operations.

Use declared addresses:

```rust
let reroute_input = PortAddress::declared(
    reroute_id,
    PortKey::new(REROUTE_INPUT_PORT).expect("built-in reroute input key is valid"),
);
let reroute_output = PortAddress::declared(
    reroute_id,
    PortKey::new(REROUTE_OUTPUT_PORT).expect("built-in reroute output key is valid"),
);
```

Do not call the public `CreateNode` descriptor path and do not expose request-local IDs to the frontend.

- [ ] **Step 6: Add the frontend DTO variant**

Extend `EditorGraphMutationDto` in `src/shared/types/dto/editorMutation.ts` with the exact `insertReroute` shape from the Interfaces block. No change is needed in `GraphMutationService`; it already accepts the union generically.

- [ ] **Step 7: Add a project-state history integration test**

In `src-tauri/src/project/production_tests.rs`, add a test named:

```text
insert_reroute_commits_one_revision_history_entry_and_exact_undo
```

The test must:

- install a graph containing one original connection;
- capture graph revision and history lengths;
- call `apply_editor_graph_mutation` once with `InsertReroute`;
- assert `to_revision == from_revision.next()`;
- assert one history entry was added;
- assert delta contains four operations;
- undo once and assert the original `DocumentConnection` including its ID and order is restored;
- redo once and assert the reroute topology returns;
- assert each returned projection matches authoritative graph revision.

- [ ] **Step 8: Run mutation and history tests**

Run:

```sh
pnpm rust:test -- insert_reroute
```

Expected: PASS for wire, planner, inverse, error, and project history tests.

Run:

```sh
pnpm rust:check
```

Expected: PASS.

- [ ] **Step 9: Review checkpoint**

Inspect the generated patch and inverse. Confirm the downstream edge alone carries `original.order`, undo restores the original connection object, and no frontend identity is accepted. Do not commit; only commit if the user explicitly requests it.

---

### Task 4: Normalize Reroutes Before Compiler Lowering

**Files:**
- Create: `src-tauri/src/node_system/compiler/reroute.rs`
- Create: `src-tauri/src/node_system/compiler/reroute_tests.rs`
- Modify: `src-tauri/src/node_system/compiler/mod.rs:3-67`
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs:572-787`
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs:2431-2493`
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs:2889-3024`
- Modify: `src-tauri/src/node_system/compiler/control.rs:282-380`

**Interfaces:**
- Consumes: Task 2 `StructuralNodeRole::Reroute`, `CompilerSemanticGraph`, `CompilerRegistry`, and the fully analyzed document.
- Produces:

```rust
pub(crate) fn normalize_reroutes<R: CompilerRegistry>(
    registry: &R,
    graph: CompilerSemanticGraph,
) -> Result<CompilerSemanticGraph, CompilerNodeDiagnostic>;

pub(crate) fn has_value_dependency_target(
    graph: &CompilerSemanticGraph,
    target: &PortAddress,
) -> bool;
```

- Normalization identity rule: every collapsed dependency uses the downstream connection identity. For effect dependencies, `effect_key` is the downstream connection ID string.
- Produces a semantic graph with no reroute nodes and no dependencies whose source or target is a reroute node.

- [ ] **Step 1: Write failing compiler transparency tests**

Create `reroute_tests.rs` using existing compiler test fixtures and add tests named:

```text
data_reroute_is_transparent_to_execution_plan
data_reroute_chain_collapses
data_reroute_propagates_generic_type
data_reroute_preserves_fanout
control_reroute_preserves_dependency_direction
effect_reroute_preserves_dependency_direction
reroute_does_not_hide_cycles
reroute_creates_no_runtime_operation
dangling_reroute_does_not_suppress_input_default
```

For the no-runtime-operation assertion, compile equivalent direct and rerouted graphs and assert:

```rust
let reroute_ids = document
    .nodes
    .values()
    .filter(|node| node.node_type.as_str().starts_with("yssbi.reroute."))
    .map(|node| node.id)
    .collect::<BTreeSet<_>>();

let plan = result.plan.as_ref().expect("rerouted graph compiles");
assert!(plan.operations.iter().all(|operation| {
    !reroute_ids.contains(&operation.node_id)
}));
```

Also assert the editor-facing `result.analysis.nodes` still contains each reroute ID while `result.semantic` does not.

- [ ] **Step 2: Run focused compiler tests and verify failure**

Run:

```sh
pnpm rust:test -- reroute_
```

Expected: FAIL because reroute normalization is not implemented and structural reroutes either reach lowering or leave disconnected semantic values.

- [ ] **Step 3: Implement deterministic reroute discovery**

In `compiler/reroute.rs`, resolve each semantic node through the registry and collect reroute metadata:

```rust
struct RerouteNode {
    node_id: NodeId,
    input: PortAddress,
    output: PortAddress,
    kind: PortKind,
}
```

Validate exactly one declared `in` port and one declared `out` port, matching directions and kinds. Return a stable compiler diagnostic located on the malformed node if a registered reroute protocol violates this invariant.

- [ ] **Step 4: Implement chain collapse**

Build incoming and outgoing dependency indexes keyed by reroute node ID. For every dependency whose target is a non-reroute node, trace its source backwards through zero or more reroutes until reaching a non-reroute source.

For value dependencies produce:

```rust
SemanticDependency::Value(ValueEdge {
    connection_id: downstream.connection_id,
    source: resolved_non_reroute_source,
    target: downstream.target,
})
```

For control dependencies produce:

```rust
SemanticDependency::Control(ControlEdge {
    connection_id: downstream.connection_id,
    source_node: resolved_source_node,
    source_port: resolved_source_port,
    target_node: downstream.target_node,
    target_port: downstream.target_port,
})
```

For effect dependencies produce:

```rust
SemanticDependency::Effect(EffectDependency {
    predecessor: resolved_predecessor,
    successor: downstream.successor,
    effect_key: downstream.effect_key,
})
```

Use a per-trace `BTreeSet<NodeId>` to detect reroute-only cycles and return a deterministic compiler diagnostic rather than recursing indefinitely.

Sort/deduplicate normalized dependencies deterministically before storing them in the boxed slice.

- [ ] **Step 5: Remove reroute nodes and reroute-only schema facts from the lowering semantic graph**

Filter `graph.nodes` to non-reroute nodes. Remove `resolved_schemas` entries whose address belongs to a reroute node. Do not mutate `AnalysisSnapshot`, `ValidatedInterfaceProjection`, or `GraphDocument`.

- [ ] **Step 6: Insert normalization after semantic validation and before lowering**

In `compile_snapshot`, preserve the full provisional graph for function ABI/resource analysis, then normalize the validated graph before `lower_graph`:

```rust
let semantic = state.semantic_graph();
let semantic = analysis.validated(semantic).map_err(/* existing mapping */)?;
let semantic = match normalize_reroutes(self.registry, semantic) {
    Ok(semantic) => semantic,
    Err(diagnostic) => {
        // Map to the existing internal analysis/lowering failure result shape.
        // Preserve the full analysis and editor projection in the returned CompileResult.
    }
};
```

Apply the same normalization path to nested function compilation before each `lower_graph` invocation.

- [ ] **Step 7: Stop using the raw document to decide whether a lowered data input is connected**

Replace the raw-document lookup at `pipeline.rs:3002-3005` with:

```rust
let has_connection = has_value_dependency_target(graph, &port.address);
```

This ensures a dangling reroute does not suppress a downstream literal or protocol default after normalization.

- [ ] **Step 8: Keep reroutes out of structured control regions**

The normalized semantic graph must reach `build_control_region` without reroute nodes. Add a defensive `StructuralNodeRole::Reroute` branch in control lowering that returns an internal compiler invariant if an unnormalized reroute reaches region construction; do not emit an empty runtime step.

- [ ] **Step 9: Run compiler tests**

Run:

```sh
pnpm rust:test -- reroute_
```

Expected: PASS for data/control/effect transparency, chain collapse, generic propagation, fan-out, cycle visibility, dangling defaults, and no runtime operation.

Run:

```sh
pnpm rust:test -- connection_limit
```

Expected: PASS; loading/compiling an invalid over-capacity document still emits the existing `compiler.connection.limit` diagnostic.

Run:

```sh
pnpm rust:check
```

Expected: PASS.

- [ ] **Step 10: Review checkpoint**

Compare direct and rerouted plan structures, excluding compilation provenance fields that legitimately include the graph revision. Confirm analysis/projection retains reroutes, semantic/lowering removes them, downstream dependency identity is preserved, and no identity kernel exists. Do not commit; only commit if the user explicitly requests it.

---

### Task 5: Add Frontend Edge Mutation Commands

**Files:**
- Create: `src/features/core/history/commands/disconnectConnections.ts`
- Create: `src/features/core/history/commands/insertReroute.ts`
- Modify: `src/features/core/history/commands/index.ts`
- Modify: `src/features/core/history/commands/registryTypes.ts`
- Modify: `src/features/core/history/types.ts`
- Modify: `src/features/core/history/index.ts`
- Modify: `src/features/core/history/structuralChange.ts`
- Test: `src/features/core/history/editorCommands.test.ts`
- Modify: `src/shared/types/dto/editorMutation.ts`

**Interfaces:**
- Consumes: Phase 1 `disconnectConnections` DTO and Task 3 `insertReroute` DTO.
- Produces:

```ts
export interface DisconnectConnectionsArgs {
  connectionIds: string[];
}

export interface InsertRerouteArgs {
  connectionId: string;
  position: { x: number; y: number };
}
```

- Produces command names `DisconnectConnections` and `InsertReroute`.

- [ ] **Step 1: Write failing command-wire tests**

Add tests to `editorCommands.test.ts` asserting:

```ts
await expect(executeCommand(graphPath, 'DisconnectConnections', {
  connectionIds: ['connection-1', 'connection-2'],
})).resolves.toBe(true);

expect(executeEditorMutation).toHaveBeenCalledTimes(1);
expect(executeEditorMutation).toHaveBeenCalledWith({
  graphPath,
  locale: 'en-US',
  mutation: {
    type: 'disconnectConnections',
    payload: { connectionIds: ['connection-1', 'connection-2'] },
  },
});
```

and:

```ts
await expect(executeCommand(graphPath, 'InsertReroute', {
  connectionId: 'connection-1',
  position: { x: 100, y: 60 },
})).resolves.toBe(true);

expect(executeEditorMutation).toHaveBeenCalledWith({
  graphPath,
  locale: 'en-US',
  mutation: {
    type: 'insertReroute',
    payload: {
      connectionId: 'connection-1',
      position: { x: 100, y: 60 },
    },
  },
});
```

Before resolving the mutation promise, assert `useGraphDataStore.getState().graphEntities[graphPath]` is reference-equal to its pre-command value.

- [ ] **Step 2: Run the command test and verify failure**

Run:

```sh
pnpm test -- src/features/core/history/editorCommands.test.ts
```

Expected: FAIL because the two commands are absent from the registry and command type union.

- [ ] **Step 3: Implement command handlers**

Implement `disconnectConnectionsCommand`:

```ts
export const disconnectConnectionsCommand:
  CommandHandler<DisconnectConnectionsArgs, boolean> = {
    async execute(graphPath, { connectionIds }) {
      if (connectionIds.length === 0) return false;
      const outcome = await executeGraphIntent(graphPath, {
        type: 'disconnectConnections',
        payload: { connectionIds },
      });
      return outcome.status === 'applied';
    },
  };
```

Implement `insertRerouteCommand`:

```ts
export const insertRerouteCommand:
  CommandHandler<InsertRerouteArgs, boolean> = {
    async execute(graphPath, { connectionId, position }) {
      const outcome = await executeGraphIntent(graphPath, {
        type: 'insertReroute',
        payload: { connectionId, position },
      });
      return outcome.status === 'applied';
    },
  };
```

- [ ] **Step 4: Register types, exports, and structural notifications**

Add both commands to `CommandType`, `CommandHandlerMap`, `commandRegistry`, public exports, and `STRUCTURAL_COMMANDS`. Do not add direct `GraphMutationService` calls outside `executeGraphIntent`.

- [ ] **Step 5: Run focused tests and typecheck**

Run:

```sh
pnpm test -- src/features/core/history/editorCommands.test.ts
```

Expected: PASS; each operation sends exactly one high-level mutation and makes no pre-response projection edit.

Run:

```sh
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 6: Review checkpoint**

Review that empty connection arrays are rejected locally, non-empty arrays are not looped, positions are passed unchanged, and both commands mark graph execution artifacts dirty only after an applied result. Do not commit; only commit if the user explicitly requests it.

---

### Task 6: Migrate to Mutually Exclusive Group-Scoped Graph Selection

**Files:**
- Create: `src/features/domain/graphSelection/graphSelection.ts`
- Create: `src/features/domain/graphSelection/graphSelection.test.ts`
- Create: `src/features/domain/graphSelection/index.ts`
- Modify: `src/features/core/layout/editorTabStore.ts:11-98,170-181,368-375,506-531,566-568`
- Modify: `src/features/core/layout/layoutTabQueries.ts:81-99`
- Modify: `src/features/core/layout/editorTabTestUtils.ts`
- Modify: `src/features/core/editor/hooks/useActiveEditorGroup.ts`
- Modify: `src/features/application/editor/editorSessionTypes.ts:91-113`
- Modify: `src/features/application/editor/useEditorGroup.ts:80-105`
- Modify: `src/features/core/canvas/useCanvasInteraction.ts:20-181`
- Modify: `src/features/application/editor/useEditorOperations.ts:24-214`
- Modify: `src/views/EditorView/Canvas/core/Canvas.tsx:25-214`
- Test: `src/features/application/editor/useEditorOperations.capabilities.test.tsx`
- Test: `src/features/application/editor/useEditorKeyboard.test.tsx`
- Test: `src/features/core/layout/layoutStore.test.ts`
- Test: affected close/load/reconcile tests containing `selectedNodeIds`

**Interfaces:**
- Consumes: Task 5 commands.
- Produces serializable group-scoped selection:

```ts
export interface GraphSelection {
  nodeIds: string[];
  connectionIds: string[];
}

export const EMPTY_GRAPH_SELECTION: GraphSelection = {
  nodeIds: [],
  connectionIds: [],
};
```

- Produces pure reducers:

```ts
export function selectNode(
  selection: GraphSelection,
  nodeId: string,
  toggle: boolean,
): GraphSelection;

export function selectConnection(
  selection: GraphSelection,
  connectionId: string,
  toggle: boolean,
): GraphSelection;

export function clearGraphSelection(): GraphSelection;
```

- Produces store updater:

```ts
export function updateEditorGroupSelection(
  updater: GraphSelection | ((previous: GraphSelection) => GraphSelection),
  targetGroupId?: string | null,
): void;
```

- [ ] **Step 1: Write pure failing selection tests**

Add tests asserting:

```ts
expect(selectNode(
  { nodeIds: [], connectionIds: ['edge-1'] },
  'node-1',
  false,
)).toEqual({ nodeIds: ['node-1'], connectionIds: [] });

expect(selectConnection(
  { nodeIds: ['node-1'], connectionIds: [] },
  'edge-1',
  false,
)).toEqual({ nodeIds: [], connectionIds: ['edge-1'] });

expect(selectConnection(
  { nodeIds: [], connectionIds: ['edge-1'] },
  'edge-2',
  true,
)).toEqual({ nodeIds: [], connectionIds: ['edge-1', 'edge-2'] });

expect(selectConnection(
  { nodeIds: [], connectionIds: ['edge-1', 'edge-2'] },
  'edge-1',
  true,
)).toEqual({ nodeIds: [], connectionIds: ['edge-2'] });
```

Use stable insertion order and do not mutate input objects.

- [ ] **Step 2: Run the pure tests and verify failure**

Run:

```sh
pnpm test -- src/features/domain/graphSelection/graphSelection.test.ts
```

Expected: FAIL because the selection module does not exist.

- [ ] **Step 3: Implement the pure selection module**

Implement normal/toggle behavior with arrays for memento serialization. Every node selection operation returns `connectionIds: []`; every connection selection operation returns `nodeIds: []`.

- [ ] **Step 4: Replace `selectedNodeIds` in editor-group placement**

Change `EditorGroupPlacement` to:

```ts
export interface EditorGroupPlacement {
  tabIds: string[];
  activeTabId: string | null;
  selection: GraphSelection;
  selectedTabIds: string[];
  locked?: boolean;
}
```

Every empty/new placement must use fresh arrays:

```ts
function createEmptySelection(): GraphSelection {
  return { nodeIds: [], connectionIds: [] };
}
```

Snapshot/apply memento must clone both arrays. Tab activation, graph close, graph unload, and projection reconciliation must clear both selection kinds where they previously cleared `selectedNodeIds`.

- [ ] **Step 5: Update editor-group hooks and canvas node selection**

Expose `selection` from `useActiveEditorGroup` and `useEditorGroup`. In `Canvas.tsx`, derive:

```ts
const selectedNodeIdsSet = useMemo(
  () => new Set(selection.nodeIds),
  [selection.nodeIds],
);

const selectedConnectionIdsSet = useMemo(
  () => new Set(selection.connectionIds),
  [selection.connectionIds],
);
```

Pass the connection set to `EdgesOverlay` in Task 7.

Update `useCanvasInteraction.onNodePointerDown` to use `selectNode(currentSelection, nodeId, e.ctrlKey || e.metaKey || e.shiftKey)` and preserve the resulting node IDs as the drag set.

Box selection writes `{ nodeIds: selectedIds, connectionIds: [] }` and remains node-only.

- [ ] **Step 6: Route Delete to the active selection kind and preserve selection on failure**

Update `useEditorOperations.deleteSelected`:

```ts
if (selection.connectionIds.length > 0) {
  const applied = await executeCommand(graphPath, 'DisconnectConnections', {
    connectionIds: selection.connectionIds,
  });
  if (applied) setSelection(clearGraphSelection());
  else uiStore.showToast('删除连接失败', 'error', 2000);
  return applied;
}

if (selection.nodeIds.length > 0) {
  const applied = await executeCommand(graphPath, 'DeleteNodes', {
    nodeIds: deletableNodeIds,
  });
  if (applied) setSelection(clearGraphSelection());
  else uiStore.showToast('删除失败', 'error', 2000);
  return applied;
}
```

Do not clear selection before awaiting the authoritative result.

- [ ] **Step 7: Update selection lifecycle tests**

Update fixtures and assertions from `selectedNodeIds` to `selection`. Add an operations test where `executeCommand` resolves `false` and assert the edge/node selection remains unchanged.

- [ ] **Step 8: Run selection, operations, keyboard, and layout tests**

Run:

```sh
pnpm test -- src/features/domain/graphSelection/graphSelection.test.ts src/features/application/editor/useEditorOperations.capabilities.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx src/features/core/layout/layoutStore.test.ts
```

Expected: PASS; node/edge selection is mutually exclusive, Delete dispatches exactly one command, and failures preserve selection.

Run:

```sh
pnpm typecheck
```

Expected: PASS with no remaining `selectedNodeIds` placement contract errors.

- [ ] **Step 9: Review checkpoint**

Review memento serialization, editor-group isolation, node drag behavior, box selection, and failure preservation. Confirm no `Set` is stored inside the Immer-backed placement. Do not commit; only commit if the user explicitly requests it.

---

### Task 7: Add Edge Hit Paths, Selection, Context Menu, Delete, and Double-Click Reroute

**Files:**
- Modify: `src/views/EditorView/Canvas/core/Edge.tsx:50-232`
- Create: `src/views/EditorView/Canvas/core/Edge.test.tsx`
- Modify: `src/views/EditorView/Canvas/core/EdgesOverlay.tsx:12-146`
- Modify: `src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx`
- Modify: `src/views/EditorView/Canvas/core/Canvas.tsx:91-197`
- Create: `src/views/EditorView/ContextMenu/EdgeContextMenu.tsx`
- Create: `src/views/EditorView/ContextMenu/EdgeContextMenu.test.tsx`
- Modify: `src/views/EditorView/ContextMenu/index.ts`
- Modify: `src/features/application/editor/CanvasContextMenuContext.tsx`
- Modify: `src/features/application/editor/useEditorOperations.ts`
- Modify: `src/features/application/editor/editorSessionTypes.ts`
- Modify: `src/app/i18n/locales/en-US.ts:37-75`
- Modify: `src/app/i18n/locales/zh-CN.ts:37-75`

**Interfaces:**
- Consumes: Task 5 commands and Task 6 `GraphSelection`.
- Produces `Edge` interaction props:

```ts
interface EdgeProps {
  edgeId: string;
  fromPinId?: string;
  toPinId?: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
  color?: string;
  thickness?: number;
  edgeKind?: EdgeKind;
  startIsInput?: boolean;
  isPullActive?: boolean;
  isFlowActive?: boolean;
  isError?: boolean;
  isRunning?: boolean;
  dimmed?: boolean;
  selected?: boolean;
  interactive?: boolean;
  onPointerDown?: (
    connectionId: string,
    event: React.PointerEvent<SVGPathElement>,
  ) => void;
  onContextMenu?: (
    connectionId: string,
    event: React.MouseEvent<SVGPathElement>,
  ) => void;
  onDoubleClick?: (
    connectionId: string,
    event: React.MouseEvent<SVGPathElement>,
  ) => void;
}
```

- Produces `EdgesOverlay` interaction props:

```ts
interface EdgesOverlayProps {
  graphPath: string;
  getPinWorldPos: (pinId: string) => { x: number; y: number } | null;
  getCanvasWorldPoint: (clientX: number, clientY: number) => { x: number; y: number };
  selectedConnectionIds: ReadonlySet<string>;
  interactive: boolean;
  dimmed?: boolean;
  onSelectConnection: (connectionId: string, toggle: boolean) => void;
  onDeleteConnections: (connectionIds: string[]) => Promise<boolean | undefined>;
  onInsertReroute: (
    connectionId: string,
    position: { x: number; y: number },
  ) => Promise<boolean | undefined>;
}
```

- Produces menu props:

```ts
export interface EdgeContextMenuProps {
  position: { x: number; y: number };
  onInsertReroute: () => void;
  onDelete: () => void;
  onClose: () => void;
}
```

- [ ] **Step 1: Write the failing `Edge` hit-path test**

Render `Edge` in happy-dom and assert:

```ts
const group = host.querySelector('[data-edge-id="edge-1"]')!;
const hitPath = group.querySelector('[data-edge-hit-path="edge-1"]')!;
const visiblePath = group.querySelector('[data-edge-visible-path="edge-1"]')!;

expect(hitPath.getAttribute('stroke')).toBe('transparent');
expect(hitPath.getAttribute('stroke-width')).toBe('12');
expect(hitPath.getAttribute('pointer-events')).toBe('stroke');
expect(visiblePath.getAttribute('stroke-width')).toBe('2');
```

Dispatch pointer/contextmenu/dblclick events and assert each callback receives `edge-1` and the event does not bubble to the canvas harness.

- [ ] **Step 2: Write failing overlay/menu behavior tests**

Add tests that assert:

- ordinary edge click selects one edge and clears nodes through the selection callback;
- Ctrl/Meta/Shift click passes `toggle=true`;
- right-click selects an unselected edge and opens the menu at client coordinates;
- deleting a selected edge deletes the full active edge selection;
- deleting an unselected context edge deletes only that edge;
- double click converts client coordinates to world coordinates and calls `onInsertReroute` once;
- `interactive=false` disables all hit-path callbacks;
- menu labels use `contextMenu.edge.insertReroute` and `contextMenu.edge.delete`.

- [ ] **Step 3: Run edge tests and verify failure**

Run:

```sh
pnpm test -- src/views/EditorView/Canvas/core/Edge.test.tsx src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx src/views/EditorView/ContextMenu/EdgeContextMenu.test.tsx
```

Expected: FAIL because hit paths, edge callbacks, and edge context menu do not exist.

- [ ] **Step 4: Add the transparent hit path without changing visible animation paths**

Inside the edge `<g>`, render the hit path before the visible path:

```tsx
<path
  d={pathData}
  fill="none"
  stroke="transparent"
  strokeWidth={12}
  strokeLinecap="round"
  pointerEvents={interactive ? 'stroke' : 'none'}
  data-edge-hit-path={edgeId}
  onPointerDown={(event) => {
    event.stopPropagation();
    onPointerDown?.(edgeId, event);
  }}
  onContextMenu={(event) => {
    event.preventDefault();
    event.stopPropagation();
    onContextMenu?.(edgeId, event);
  }}
  onDoubleClick={(event) => {
    event.preventDefault();
    event.stopPropagation();
    onDoubleClick?.(edgeId, event);
  }}
/>
```

Mark the existing base visible path with `data-edge-visible-path={edgeId}`. Keep every flow/glow/error path `pointer-events-none`.

For selected edges, use the existing accent variable and only alter the visible path:

```tsx
stroke={selected ? 'var(--accent-color)' : strokeColor}
strokeWidth={selected ? strokeW + 1 : strokeW}
```

- [ ] **Step 5: Implement `EdgeContextMenu` with shared primitives**

Use `ContextMenu` from `@/shared/ui/contextMenu` with two compact items:

```ts
[
  {
    items: [
      {
        id: 'insert-reroute',
        label: t('contextMenu.edge.insertReroute'),
        onClick: onInsertReroute,
      },
    ],
  },
  {
    items: [
      {
        id: 'delete-edge',
        label: t('contextMenu.edge.delete'),
        shortcut: 'Del',
        danger: true,
        onClick: onDelete,
      },
    ],
  },
]
```

Add English `Insert Reroute` / `Delete Connection` and Chinese `插入重定向节点` / `删除连接` keys.

- [ ] **Step 6: Implement overlay interaction routing**

Maintain one overlay-level context-menu state:

```ts
interface EdgeMenuState {
  connectionId: string;
  client: { x: number; y: number };
  world: { x: number; y: number };
}
```

On click call `onSelectConnection(edgeId, event.ctrlKey || event.metaKey || event.shiftKey)`.

On context menu, select the edge if it is not already selected, then store both client and world positions.

On menu delete, compute:

```ts
const ids = selectedConnectionIds.has(menu.connectionId)
  ? [...selectedConnectionIds]
  : [menu.connectionId];
```

and call `onDeleteConnections(ids)` once.

On menu insert and double click, call `onInsertReroute(connectionId, worldPosition)` once.

- [ ] **Step 7: Wire Canvas operations**

Add operations:

```ts
const deleteConnectionsById = useCallback(async (connectionIds: string[]) => {
  const graphPath = activeTabIdRef.current;
  if (!graphPath || connectionIds.length === 0) return false;
  const applied = await executeCommand(graphPath, 'DisconnectConnections', { connectionIds });
  if (!applied) uiStore.showToast('删除连接失败', 'error', 2000);
  return applied;
}, []);

const insertReroute = useCallback(async (
  connectionId: string,
  position: { x: number; y: number },
) => {
  const graphPath = activeTabIdRef.current;
  if (!graphPath) return false;
  const applied = await executeCommand(graphPath, 'InsertReroute', {
    connectionId,
    position,
  });
  if (!applied) uiStore.showToast('插入重定向节点失败', 'error', 2000);
  return applied;
}, []);
```

Pass them through editor session types into `Canvas` and `EdgesOverlay`. Use the existing canvas coordinate conversion; do not query Tauri during pointer movement.

- [ ] **Step 8: Run edge tests and typecheck**

Run:

```sh
pnpm test -- src/views/EditorView/Canvas/core/Edge.test.tsx src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx src/views/EditorView/ContextMenu/EdgeContextMenu.test.tsx
```

Expected: PASS for hoverable hit geometry, click/toggle, context menu, delete, double-click insertion, and disabled preview behavior.

Run:

```sh
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 9: Review checkpoint**

Review SVG stacking, event propagation, menu selection semantics, one-command deletion, coordinate conversion, and preview-group read-only behavior. Confirm no pointer-move IPC was introduced. Do not commit; only commit if the user explicitly requests it.

---

### Task 8: Render Reroutes as Compact Selectable Nodes

**Files:**
- Create: `src/views/EditorView/Nodes/RerouteNodeLayout.tsx`
- Create: `src/views/EditorView/Nodes/RerouteNodeLayout.test.tsx`
- Modify: `src/views/EditorView/Nodes/Node.tsx:39-74`
- Modify: `src/views/EditorView/Nodes/NodeContainer.tsx:66-95`
- Modify: `src/features/core/dataStore/nodeView.ts:20-55`
- Modify: `src/features/core/dataStore/nodeView.test.ts`
- Modify: `src/features/domain/node/utils/nodeClassNames.ts:20-68`

**Interfaces:**
- Consumes: Task 2 projected `display.styleId === 'builtin.reroute'` and the normal `UINode` input/output pins.
- Produces:

```ts
export type NodeLayoutKind = 'default' | 'math' | 'reroute';

export function getNodeLayoutKind(
  node: Pick<UINode, 'uiStyle'>,
): NodeLayoutKind;

export function getNodeMinSize(
  layout: NodeLayoutKind,
): React.CSSProperties;
```

- Produces component:

```ts
export interface RerouteNodeLayoutProps extends NodeProps {}

export const RerouteNodeLayout: React.MemoExoticComponent<
  React.FC<RerouteNodeLayoutProps>
>;
```

- [ ] **Step 1: Write failing style and compact-layout tests**

Extend `nodeView.test.ts`:

```ts
expect(getNodeLayoutKind({ uiStyle: 'builtin.reroute' })).toBe('reroute');
expect(getNodeLayoutKind({ uiStyle: 'math' })).toBe('math');
expect(getNodeLayoutKind({ uiStyle: 'builtin.default' })).toBe('default');
```

In `RerouteNodeLayout.test.tsx`, render a projected reroute and assert:

```ts
expect(host.querySelector('[data-reroute-node="node-1"]')).not.toBeNull();
expect(host.querySelector('[data-node-header]')).toBeNull();
expect(host.querySelector('[data-parameter-editor]')).toBeNull();
expect(host.querySelectorAll('[data-pin-id]')).toHaveLength(2);
```

Assert the container style resolves to 24 by 24 pixels and still invokes the normal node pointer-down callback.

- [ ] **Step 2: Run compact-layout tests and verify failure**

Run:

```sh
pnpm test -- src/features/core/dataStore/nodeView.test.ts src/views/EditorView/Nodes/RerouteNodeLayout.test.tsx
```

Expected: FAIL because the reroute layout kind and component do not exist.

- [ ] **Step 3: Implement layout-kind resolution and sizes**

Replace the boolean header helper with:

```ts
export function getNodeLayoutKind(
  node: Pick<UINode, 'uiStyle'>,
): NodeLayoutKind {
  if (node.uiStyle === 'builtin.reroute') return 'reroute';
  if (node.uiStyle === 'math') return 'math';
  return 'default';
}
```

Implement sizes:

```ts
export function getNodeMinSize(layout: NodeLayoutKind) {
  if (layout === 'reroute') {
    return { minWidth: 24, width: 24, minHeight: 24, height: 24 };
  }
  if (layout === 'math') {
    return { minWidth: 120, minHeight: 60 };
  }
  return { minWidth: 160, minHeight: undefined };
}
```

Update `NodeContainer` to call `getNodeMinSize(getNodeLayoutKind(node))`.

- [ ] **Step 4: Implement `RerouteNodeLayout`**

Render one compact circular body with input and output pin components using the same pin pointer handlers as ordinary nodes. Do not render title, header, parameter editors, add-port controls, literal controls, or execution text.

The root must include:

```tsx
<div
  data-reroute-node={node.id}
  className="relative h-6 w-6 rounded-full"
>
  {/* input pin anchored left; output pin anchored right */}
</div>
```

Reuse existing pin components rather than reimplementing pin compatibility or pointer behavior.

- [ ] **Step 5: Dispatch the reroute style in `Node.tsx`**

Use one layout decision:

```tsx
const layout = getNodeLayoutKind(node);

{layout === 'reroute' ? (
  <RerouteNodeLayout {...props} />
) : layout === 'math' ? (
  <MathNodeLayout {...props} />
) : (
  <DefaultNodeLayout {...props} />
)}
```

Keep `NodeContainer` around every layout so reroutes inherit authoritative position, selection ring, drag/move, node menu, and delete behavior.

- [ ] **Step 6: Run compact UI tests and typecheck**

Run:

```sh
pnpm test -- src/features/core/dataStore/nodeView.test.ts src/views/EditorView/Nodes/RerouteNodeLayout.test.tsx src/views/EditorView/Nodes/DefaultNodeLayout.test.tsx
```

Expected: PASS; reroutes are compact and existing default/math layouts retain their previous dimensions and controls.

Run:

```sh
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 7: Review checkpoint**

Review that reroute rendering is selected solely from Rust-authored `styleId`, no parameter editor is visible, normal node movement/deletion is reused, and no frontend-only bend-point state was introduced. Do not commit; only commit if the user explicitly requests it.

---

### Task 9: Run Cross-Layer Phase 2 Acceptance Verification

**Files:**
- Test: `src-tauri/src/node_system/document/tests/editor_mutation_validation.rs`
- Test: `src-tauri/src/project/production_tests.rs`
- Test: `src-tauri/src/node_system/compiler/reroute_tests.rs`
- Test: `src/features/core/history/editorCommands.test.ts`
- Test: `src/features/domain/graphSelection/graphSelection.test.ts`
- Test: `src/views/EditorView/Canvas/core/Edge.test.tsx`
- Test: `src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx`
- Test: `src/views/EditorView/ContextMenu/EdgeContextMenu.test.tsx`
- Test: `src/views/EditorView/Nodes/RerouteNodeLayout.test.tsx`
- No production behavior should be added in this task; fix only defects exposed by the acceptance suite and rerun the narrowest failing command first.

**Interfaces:**
- Consumes: all Task 2-8 outputs.
- Produces: fresh verification evidence for Phase 2 acceptance.
- Acceptance requires data/control/effect reroutes to preserve topology semantics, ordering, undo identity, and compilation behavior while edge interaction sends one mutation per committed action.

- [ ] **Step 1: Run the focused Rust mutation suite**

Run:

```sh
pnpm rust:test -- insert_reroute
```

Expected: PASS; data/control/effect insertion, order preservation, inverse identity, failure atomicity, history, and stale-revision tests pass.

- [ ] **Step 2: Run the focused compiler suite**

Run:

```sh
pnpm rust:test -- reroute_
```

Expected: PASS; generic propagation, dependency direction, fan-out, chain collapse, cycle visibility, dangling defaults, and absence of runtime reroute operations are proven.

Run:

```sh
pnpm rust:test -- connection_limit
```

Expected: PASS; the existing over-capacity diagnostic remains intact.

- [ ] **Step 3: Run the focused frontend suite**

Run:

```sh
pnpm test -- src/features/core/history/editorCommands.test.ts src/features/domain/graphSelection/graphSelection.test.ts src/features/application/editor/useEditorOperations.capabilities.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx src/views/EditorView/Canvas/core/Edge.test.tsx src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx src/views/EditorView/ContextMenu/EdgeContextMenu.test.tsx src/views/EditorView/Nodes/RerouteNodeLayout.test.tsx src/features/core/dataStore/nodeView.test.ts
```

Expected: PASS; hit paths, hover/select/toggle/menu/delete/double-click, one mutation per command, mutual exclusion, failure-preserved selection, and compact rendering all pass.

- [ ] **Step 4: Run language checks**

Run:

```sh
pnpm typecheck
```

Expected: PASS.

Run:

```sh
pnpm rust:fmt:check
```

Expected: PASS.

Run:

```sh
pnpm rust:check
```

Expected: PASS.

- [ ] **Step 5: Run the repository cross-layer verification**

Run:

```sh
pnpm verify
```

Expected: PASS; frontend tests, Rust tests, format checks, type checks, and repository verification complete successfully.

- [ ] **Step 6: Check whitespace and accidental scope**

Run:

```sh
git diff --check
```

Expected: no output and exit code 0.

Run:

```sh
git --no-pager diff --stat
```

Expected: only files listed in Tasks 2-8 and their directly affected selection lifecycle tests are changed; unrelated user files such as `TODO.md` are not included in implementation edits.

- [ ] **Step 7: Manual review checkpoint**

Using a local development run only after automated checks pass, verify:

1. a thin visible edge has an easy-to-hit invisible interaction width;
2. ordinary edge click clears node selection;
3. Ctrl/Shift edge click toggles edge selection only;
4. right-click exposes Insert Reroute and Delete Connection;
5. Delete/Backspace removes the active edge selection with one undo step;
6. double-click inserts one compact reroute at the pointer world position;
7. data, control, and effect edges retain their visual kind through reroute;
8. ordered target behavior is unchanged;
9. one undo restores the original unsplit edge and its identity;
10. graph execution produces no reroute operation or runtime trace step.

Do not commit after review. Present the verified diff and test evidence to the user; create a commit only if the user explicitly requests one.
