# Blueprint Subgraph Efficiency Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver authoritative subgraph export, duplicate, insert, copy, paste, and cut together with committed-delta selection, Blueprint-style canvas shortcuts, Shift box-selection union, and unsupported-menu cleanup.

**Architecture:** Rust owns portable subgraph export, untrusted snapshot validation, identity allocation, patch planning, revision/history, and the installed graph projection. React coordinates backend queries and mutations, writes only backend-produced snapshots to the system clipboard, derives post-mutation selection only from the committed delta, and keeps selection/viewport previews frontend-local. The implementation extends the existing graph document, mutation coordinator, editor session, viewport, and global keyboard boundaries rather than introducing a second graph protocol or canvas library.

**Tech Stack:** Rust 2024, Serde, Tauri 2, TypeScript 5.8, React 19, Zustand 5, Vitest 4, pnpm 11.

## Global Constraints

- The approved source design is `docs/superpowers/specs/2026-08-11-blueprint-style-graph-interaction-design.md`.
- Phase 3 starts only after Phase 1 atomic `DeleteNodes` and Phase 2 persisted reroute/compiler-transparency acceptance tests pass.
- One user-visible graph operation produces one `GraphDocumentPatch`, one graph revision, and one Rust history entry.
- The frontend sends intent and clipboard content; it never allocates document node, port-instance, or connection IDs and never authors graph patches.
- `ProjectState.project_data` remains authoritative, and graph writes continue through `ProjectState::apply_editor_graph_mutation`.
- Tauri commands remain thin. Frontend IPC remains under `src/services/`; views do not call `invoke`.
- Export is a read-only query: it changes no graph revision, history, projection, or event stream.
- Clipboard parsing is untrusted. Rust enforces schema, reference, resource, protocol, size, and depth limits before a patch is committed.
- Cut order is export, successful system-clipboard write, then one atomic `DeleteNodes`; failure before deletion leaves the graph unchanged.
- Duplicate and paste select only node IDs found in the committed mutation delta after projection installation.
- `Ctrl+A`, `F`, `Home`, and Shift box previews perform no IPC. `F` and `Home` create no graph history entry.
- Global keyboard and pointer listeners continue through `src/shared/utils/globalEvent.ts`.
- Preserve the active editor-group and `graphPath` boundaries; do not infer backend load state from frontend graph entities alone.
- Remove the old frontend-authored clipboard snapshot/store directly; this 0.x project does not keep a compatibility path.
- Run repository commands from the repository root through `pnpm` scripts. Do not invoke ad-hoc Cargo commands that create `src-tauri/target/`.
- Do not modify unrelated user work, including pre-existing changes outside this plan's file list.
- Do not commit during this plan. End every task at its review checkpoint. Create a Git commit only when the user explicitly requests one.

---

### Task 1: Verify Phase 1 and Phase 2 prerequisites

**Files:**
- Inspect: `src-tauri/src/node_system/document/mutation.rs:270-308`
- Inspect: `src/features/core/history/commands/deleteNodes.ts:1-20`
- Inspect: `src/features/application/editor/useEditorOperations.ts:150-178`
- Inspect: Phase 1 and Phase 2 focused test files produced by their implementation plans

**Interfaces:**
- Consumes: `EditorGraphMutationDto::DeleteNodes { node_ids: Vec<NodeId> }`, one frontend `DeleteNodes` intent, persisted reroute nodes, and compiler-transparent reroute analysis.
- Produces: A binary go/no-go result for Phase 3. This task changes no files.

- [ ] **Step 1: Confirm the atomic delete wire contract exists**

Verify the Rust and TypeScript mutation unions contain exactly the collection form:

```rust
DeleteNodes {
    node_ids: Vec<NodeId>,
}
```

```ts
{ type: 'deleteNodes'; payload: { nodeIds: string[] } }
```

Confirm `src/features/core/history/commands/deleteNodes.ts` submits one mutation and contains no loop over `nodeIds`.

- [ ] **Step 2: Run the Phase 1 atomic-delete regression filter**

Run:

```sh
pnpm rust:test -- delete_nodes
```

Expected: PASS; tests demonstrate duplicate/empty rejection, all-target validation, one revision, one history entry, and one-step undo/redo.

- [ ] **Step 3: Run the frontend command-wire regression**

Run:

```sh
pnpm test -- src/features/core/history/editorCommands.test.ts
```

Expected: PASS; `DeleteNodes` calls the mutation coordinator once for multiple IDs.

- [ ] **Step 4: Run the Phase 2 reroute regression filters**

Run:

```sh
pnpm rust:test -- reroute
```

Expected: PASS; persisted data and effect/control reroutes survive save/history and compile transparently.

- [ ] **Step 5: Stop if either prerequisite is absent**

If the inspected code still exposes singular `DeleteNode`, loops deletion requests, or lacks persisted reroutes, do not begin Task 2. Execute the approved Phase 1/2 plans first, then repeat Steps 1–4.

- [ ] **Step 6: Review checkpoint**

Record the passing command output in the implementation session notes. Do not create a commit.

---

### Task 2: Add portable Rust subgraph DTOs and authoritative export

**Files:**
- Create: `src-tauri/src/node_system/document/subgraph.rs`
- Create: `src-tauri/src/node_system/document/tests/subgraph.rs`
- Modify: `src-tauri/src/node_system/document/tests.rs:28`
- Modify: `src-tauri/src/node_system/document/mod.rs:3-45`
- Read for reuse: `src-tauri/src/node_system/catalog/localization.rs:126-269`
- Read for resource resolution: `src-tauri/src/project/project_reads.rs:26-53`

**Interfaces:**
- Consumes: `GraphDocument`, `NodeRegistry`, `CatalogMutationValidationSnapshot`, selected `Vec<NodeId>`, `DynamicPortBinding`, `InputState`, and `OrderKey`.
- Produces: `ClipboardSubgraphDto`, `export_subgraph`, schema/size constants, and clipboard-local identity types consumed by Tasks 3–7.

- [ ] **Step 1: Register the test module, define exact fixtures, and write executable failing export tests**

Add to `src-tauri/src/node_system/document/tests.rs`:

```rust
mod subgraph;
```

In `src-tauri/src/node_system/document/tests/subgraph.rs`, define these fixtures and helpers before the tests. `export_fixture` returns one valid static graph containing three nodes, three dynamic input instances, one selected-internal edge, and two crossing edges. `export_selected` returns the exact planner result under test.

```rust
use super::*;
use crate::node_system::document::{
    ClipboardNodeCreationDto, ClipboardPortRefDto, ClipboardSubgraphDto, MutationConflict,
    export_subgraph,
};
use crate::project::{CatalogMutationValidationSnapshot, ProjectInstanceId};

struct ExportFixture {
    graph_path: GraphResourcePath,
    document: GraphDocument,
    registry: NodeRegistry,
    catalog: CatalogMutationValidationSnapshot,
    first: NodeId,
    second: NodeId,
    external: NodeId,
    first_input_instance: PortInstanceId,
    second_input_instance: PortInstanceId,
    external_input_instance: PortInstanceId,
    internal_connection: ConnectionId,
    outgoing_connection: ConnectionId,
    incoming_connection: ConnectionId,
}

fn empty_catalog_snapshot() -> CatalogMutationValidationSnapshot {
    CatalogMutationValidationSnapshot {
        project_instance_id: ProjectInstanceId::new(),
        authority_generation: 0,
        resources: BTreeMap::new(),
    }
}

fn export_test_node(
    id: NodeId,
    position: NodePosition,
    user_label: Option<&str>,
    parameter_value: i64,
) -> DocumentNode {
    let mut parameters = ParameterValues::new();
    parameters.insert(
        ParameterKey::new("export_marker").unwrap(),
        json!(parameter_value),
    );
    DocumentNode {
        id,
        node_type: NodeTypeId::new("yssbi.test.editor_mutation").unwrap(),
        position,
        parameters,
        user_label: user_label.map(str::to_owned),
    }
}

fn user_input_address(node_id: NodeId, instance_id: PortInstanceId) -> PortAddress {
    PortAddress::instance(
        node_id,
        PortKey::new("inputs").unwrap(),
        instance_id,
    )
}

fn declared_output_address(node_id: NodeId) -> PortAddress {
    PortAddress::declared(node_id, PortKey::new("output").unwrap())
}

fn insert_user_input(
    document: &mut GraphDocument,
    node_id: NodeId,
    instance_id: PortInstanceId,
    order: &str,
) -> PortAddress {
    let address = user_input_address(node_id, instance_id);
    document.port_bindings.insert(
        address.clone(),
        DynamicPortBinding::UserCreated {
            order: OrderKey(order.into()),
        },
    );
    address
}

fn insert_connection(
    document: &mut GraphDocument,
    id: ConnectionId,
    output_node: NodeId,
    input: PortAddress,
    order: Option<&str>,
) {
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output: declared_output_address(output_node),
            input,
            order: order.map(|value| OrderKey(value.into())),
        },
    );
}

fn export_fixture() -> ExportFixture {
    let first = node_id(0x101);
    let second = node_id(0x102);
    let external = node_id(0x103);
    let first_input_instance = instance_id(0x201);
    let second_input_instance = instance_id(0x202);
    let external_input_instance = instance_id(0x203);
    let internal_connection = connection_id(0x301);
    let outgoing_connection = connection_id(0x302);
    let incoming_connection = connection_id(0x303);
    let mut document = GraphDocument::default();

    document.nodes.insert(
        first,
        export_test_node(
            first,
            NodePosition { x: 20.0, y: 30.0 },
            Some("Source"),
            11,
        ),
    );
    document.nodes.insert(
        second,
        export_test_node(
            second,
            NodePosition { x: 80.0, y: 90.0 },
            Some("Reroute"),
            22,
        ),
    );
    document.nodes.insert(
        external,
        export_test_node(
            external,
            NodePosition { x: 160.0, y: 180.0 },
            Some("External"),
            33,
        ),
    );

    let first_input = insert_user_input(
        &mut document,
        first,
        first_input_instance,
        "first-input",
    );
    let second_input = insert_user_input(
        &mut document,
        second,
        second_input_instance,
        "second-input",
    );
    let external_input = insert_user_input(
        &mut document,
        external,
        external_input_instance,
        "external-input",
    );
    document.input_states.insert(
        second_input.clone(),
        InputState {
            literal_override: Some(json!(42)),
        },
    );

    insert_connection(
        &mut document,
        internal_connection,
        first,
        second_input,
        Some("internal-order"),
    );
    insert_connection(
        &mut document,
        outgoing_connection,
        second,
        external_input,
        None,
    );
    insert_connection(
        &mut document,
        incoming_connection,
        external,
        first_input,
        None,
    );

    ExportFixture {
        graph_path: graph_path("events/export.yssbi-event"),
        document,
        registry: editor_mutation_registry(),
        catalog: empty_catalog_snapshot(),
        first,
        second,
        external,
        first_input_instance,
        second_input_instance,
        external_input_instance,
        internal_connection,
        outgoing_connection,
        incoming_connection,
    }
}

fn export_selected(
    fixture: &ExportFixture,
    node_ids: Vec<NodeId>,
) -> Result<ClipboardSubgraphDto, MutationConflict> {
    export_subgraph(
        &fixture.graph_path,
        &fixture.document,
        &fixture.registry,
        &fixture.catalog,
        node_ids,
    )
}

#[test]
fn subgraph_export_uses_relative_positions_and_omits_authority_ids() {
    let fixture = export_fixture();
    let snapshot = export_selected(&fixture, vec![fixture.second, fixture.first]).unwrap();

    assert_eq!(snapshot.nodes.len(), 2);
    assert_eq!(snapshot.nodes[0].local_id.0.as_ref(), "node/0");
    assert_eq!(snapshot.nodes[1].local_id.0.as_ref(), "node/1");
    assert_eq!(
        snapshot.nodes[0].relative_position,
        NodePosition { x: 0.0, y: 0.0 },
    );
    assert_eq!(
        snapshot.nodes[1].relative_position,
        NodePosition { x: 60.0, y: 60.0 },
    );

    let wire = serde_json::to_string(&snapshot).unwrap();
    for authority_id in [
        fixture.first.to_string(),
        fixture.second.to_string(),
        fixture.internal_connection.to_string(),
        fixture.first_input_instance.to_string(),
        fixture.second_input_instance.to_string(),
    ] {
        assert!(!wire.contains(&authority_id));
    }
}

#[test]
fn subgraph_export_preserves_parameters_labels_bindings_and_literals() {
    let fixture = export_fixture();
    let snapshot = export_selected(&fixture, vec![fixture.first, fixture.second]).unwrap();

    assert_eq!(snapshot.schema_version, 1);
    assert_eq!(snapshot.nodes.len(), 2);
    assert_eq!(snapshot.nodes[0].user_label.as_deref(), Some("Source"));
    assert_eq!(snapshot.nodes[1].user_label.as_deref(), Some("Reroute"));
    assert_eq!(
        snapshot.nodes[0]
            .parameters
            .get(&ParameterKey::new("export_marker").unwrap()),
        Some(&json!(11)),
    );
    assert!(matches!(
        snapshot.nodes[0].creation,
        ClipboardNodeCreationDto::Static { .. }
    ));
    assert_eq!(snapshot.port_bindings.len(), 2);
    assert!(snapshot.port_bindings.iter().all(|entry| matches!(
        entry.address.port,
        ClipboardPortRefDto::Instance { .. }
    )));
    assert_eq!(snapshot.input_states.len(), 1);
    assert_eq!(
        snapshot.input_states[0].state.literal_override,
        Some(json!(42)),
    );
}

#[test]
fn subgraph_export_keeps_only_internal_connections() {
    let fixture = export_fixture();
    let snapshot = export_selected(&fixture, vec![fixture.first, fixture.second]).unwrap();

    assert_eq!(snapshot.connections.len(), 1);
    assert_eq!(
        snapshot.connections[0].order,
        Some(OrderKey("internal-order".into())),
    );
    let wire = serde_json::to_string(&snapshot).unwrap();
    assert!(!wire.contains(&fixture.outgoing_connection.to_string()));
    assert!(!wire.contains(&fixture.incoming_connection.to_string()));
    assert!(!wire.contains(&fixture.external.to_string()));
    assert!(!wire.contains(&fixture.external_input_instance.to_string()));
}

#[test]
fn subgraph_export_rejects_empty_duplicate_and_missing_targets() {
    let fixture = export_fixture();
    let missing = node_id(0x999);

    for result in [
        export_selected(&fixture, Vec::new()),
        export_selected(&fixture, vec![fixture.first, fixture.first]),
        export_selected(&fixture, vec![fixture.first, missing]),
    ] {
        let error = result.unwrap_err();
        assert!(!error.code().is_empty());
        assert!(!error.to_string().is_empty());
    }
}
```

The Phase 2 reroute tests remain responsible for proving the built-in reroute protocol identity and compiler transparency. This export fixture proves that persisted node type, label, parameters, dynamic port state, literal state, and internal topology are copied without special-casing display projections.

- [ ] **Step 2: Run the export tests and verify they fail**

Run:

```sh
pnpm rust:test -- subgraph_export
```

Expected: FAIL because `ClipboardSubgraphDto` and `export_subgraph` do not exist.

- [ ] **Step 3: Define the portable DTO contract**

Create `src-tauri/src/node_system/document/subgraph.rs` with these public types and exact Serde policy:

```rust
pub const CLIPBOARD_SUBGRAPH_SCHEMA_VERSION: u32 = 1;
pub const MAX_CLIPBOARD_NODES: usize = 500;
pub const MAX_CLIPBOARD_CONNECTIONS: usize = 2_000;
pub const MAX_CLIPBOARD_PORT_BINDINGS: usize = 4_000;
pub const MAX_CLIPBOARD_INPUT_STATES: usize = 4_000;
pub const MAX_CLIPBOARD_PARAMETER_BYTES: usize = 1_048_576;
pub const MAX_CLIPBOARD_VALUE_DEPTH: usize = 64;
pub const MAX_CLIPBOARD_SERIALIZED_BYTES: usize = 4_194_304;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClipboardNodeId(pub Box<str>);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClipboardPortInstanceId(pub Box<str>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ClipboardNodeCreationDto {
    Static { node_type_id: NodeTypeId },
    ResourceBound {
        node_type_id: NodeTypeId,
        resource_path: CatalogResourcePath,
        create_args: ResourceBoundCreateArgsDto,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum ClipboardPortRefDto {
    Declared { key: PortKey },
    Instance {
        template: PortKey,
        local_instance_id: ClipboardPortInstanceId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardPortAddressDto {
    pub node_id: ClipboardNodeId,
    pub port: ClipboardPortRefDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardNodeDto {
    pub local_id: ClipboardNodeId,
    pub creation: ClipboardNodeCreationDto,
    pub parameters: ParameterValues,
    pub user_label: Option<String>,
    pub relative_position: NodePosition,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardPortBindingDto {
    pub address: ClipboardPortAddressDto,
    pub binding: DynamicPortBinding,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardInputStateDto {
    pub address: ClipboardPortAddressDto,
    pub state: InputState,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardConnectionDto {
    pub output: ClipboardPortAddressDto,
    pub input: ClipboardPortAddressDto,
    pub order: Option<OrderKey>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipboardSubgraphDto {
    pub schema_version: u32,
    pub nodes: Vec<ClipboardNodeDto>,
    pub port_bindings: Vec<ClipboardPortBindingDto>,
    pub input_states: Vec<ClipboardInputStateDto>,
    pub connections: Vec<ClipboardConnectionDto>,
}
```

- [ ] **Step 4: Implement deterministic export**

Add this exact public signature:

```rust
pub fn export_subgraph(
    graph_path: &GraphResourcePath,
    document: &GraphDocument,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
    node_ids: Vec<NodeId>,
) -> Result<ClipboardSubgraphDto, MutationConflict>;
```

Implementation rules:

1. Reject an empty list and duplicate direct targets.
2. Sort selected `NodeId`s before assigning `node/{index}`.
3. Validate every node exists, its protocol exists, and it is not managed.
4. Derive static creation identity from registry authority.
5. Derive resource-bound identity by matching the node type and authoritative parameter binding against `catalog.resources`; reject an absent or ambiguous resource.
6. Normalize positions by selected minimum `x/y`.
7. Sort port addresses, assign local instance IDs, and rewrite every binding/input/connection address.
8. Include only connections whose output and input node are selected.
9. Preserve parameters, user labels, dynamic bindings, literals, reroutes, and order keys.
10. Reject counts or serialized bytes above the declared constants.

- [ ] **Step 5: Export the new types from the document module**

In `src-tauri/src/node_system/document/mod.rs`, add explicit exports:

```rust
pub use subgraph::{
    CLIPBOARD_SUBGRAPH_SCHEMA_VERSION, ClipboardConnectionDto, ClipboardInputStateDto,
    ClipboardNodeCreationDto, ClipboardNodeDto, ClipboardNodeId, ClipboardPortAddressDto,
    ClipboardPortBindingDto, ClipboardPortInstanceId, ClipboardPortRefDto,
    ClipboardSubgraphDto, export_subgraph,
};
```

- [ ] **Step 6: Run focused export tests**

Run:

```sh
pnpm rust:test -- subgraph_export
```

Expected: PASS; exported JSON contains no authoritative entity UUIDs and excludes every external edge.

- [ ] **Step 7: Review checkpoint**

Run:

```sh
git --no-pager diff -- src-tauri/src/node_system/document/subgraph.rs src-tauri/src/node_system/document/tests/subgraph.rs src-tauri/src/node_system/document/tests.rs src-tauri/src/node_system/document/mod.rs
```

Expected: only Task 2 files appear. Do not commit.

---

### Task 3: Validate and instantiate untrusted subgraph snapshots

**Files:**
- Modify: `src-tauri/src/node_system/document/subgraph.rs`
- Modify: `src-tauri/src/node_system/document/tests/subgraph.rs`
- Modify: `src-tauri/src/node_system/document/error.rs`

**Interfaces:**
- Consumes: `ClipboardSubgraphDto`, target graph path/document/registry/catalog, and an anchor `NodePosition`.
- Produces: `instantiate_subgraph -> Result<GraphDocumentPatch, MutationConflict>`, stable `clipboard_subgraph_invalid` and `referenced_resource_unavailable` conflicts, and fresh document identities.

- [ ] **Step 1: Write failing validation and identity tests**

Add tests named:

```rust
subgraph_insert_allocates_fresh_document_ids
subgraph_insert_restores_dynamic_instances_literals_and_ordered_edges
subgraph_insert_rejects_wrong_schema_version
subgraph_insert_rejects_duplicate_local_ids
subgraph_insert_rejects_dangling_local_references
subgraph_insert_rejects_non_finite_positions
subgraph_insert_rejects_missing_protocol
subgraph_insert_rejects_missing_resource
subgraph_insert_rejects_each_limit_plus_one
subgraph_insert_has_zero_staged_effects_on_validation_failure
```

For fresh IDs, collect all `InsertNode`, `InsertPortBinding`, and `InsertConnection` operations and assert none equal source authority IDs or clipboard-local strings. For atomic failure, clone the target document, call the planner, and assert the original clone remains equal after every rejected case.

- [ ] **Step 2: Run the insert tests and verify they fail**

Run:

```sh
pnpm rust:test -- subgraph_insert
```

Expected: FAIL because `instantiate_subgraph` and stable clipboard conflicts do not exist.

- [ ] **Step 3: Add stable conflict categories**

Extend `MutationConflict` in `src-tauri/src/node_system/document/error.rs` with:

```rust
ClipboardSubgraphInvalid(Box<str>),
ReferencedResourceUnavailable(Box<str>),
```

Return exact codes from `MutationConflict::code()`:

```text
clipboard_subgraph_invalid
referenced_resource_unavailable
```

Detailed local IDs and resource addresses remain in logs/error details, not ordinary translated UI messages.

- [ ] **Step 4: Implement the instantiate signature and validation budget**

Add:

```rust
pub fn instantiate_subgraph(
    graph_path: &GraphResourcePath,
    document: &GraphDocument,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
    snapshot: ClipboardSubgraphDto,
    anchor: NodePosition,
) -> Result<GraphDocumentPatch, MutationConflict>;
```

Before allocating IDs, validate:

1. `schema_version == 1` and nodes are non-empty.
2. All collection limits, serialized size, parameter byte count, and JSON value depth.
3. Unique node local IDs and unique instance local IDs within their node/template scope.
4. Every binding/input/connection endpoint references an existing local node and valid declared/instance port.
5. Every target position is finite after adding anchor.
6. Static protocol availability, scope, managed role, and parameter validity.
7. Resource-bound path, allowed node type, target graph scope, and current resource availability.
8. Dynamic binding origins and literal targets resolve under the target authority.
9. Every internal connection passes normal direction, kind, type, ordering, orphan, and capacity validation.

- [ ] **Step 5: Generate operations in dependency order**

Allocate fresh `NodeId`, `PortInstanceId`, and `ConnectionId` in deterministic clipboard-local order, then generate exactly:

```text
InsertNode by clipboard node ID
InsertPortBinding by portable address
SetInputState by portable address
InsertConnection by portable endpoint tuple
```

Apply the resulting patch to a cloned target document with `apply_without_revision` before returning it. This guarantees all-or-nothing validation and gives `GraphDocumentPatch::inverse()` the valid reverse order.

- [ ] **Step 6: Run focused insert tests**

Run:

```sh
pnpm rust:test -- subgraph_insert
```

Expected: PASS; all malformed snapshots fail before authority state changes, and legal snapshots preserve bindings/literals/order while allocating fresh identities.

- [ ] **Step 7: Review checkpoint**

Run:

```sh
git --no-pager diff -- src-tauri/src/node_system/document/subgraph.rs src-tauri/src/node_system/document/tests/subgraph.rs src-tauri/src/node_system/document/error.rs
```

Expected: validation is centralized in `subgraph.rs`; no Tauri or UI concerns appear. Do not commit.

---

### Task 4: Add DuplicateSubgraph and InsertSubgraph authoritative mutations

**Files:**
- Modify: `src-tauri/src/node_system/document/subgraph.rs`
- Modify: `src-tauri/src/node_system/document/mutation.rs:262-552`
- Modify: `src-tauri/src/project/project_state.rs:4690-5025`
- Modify: `src-tauri/src/project/production_tests.rs`
- Modify: `src-tauri/src/event/event_project.rs` only if derives/imports require the new DTO; do not add a second result protocol

**Interfaces:**
- Consumes: Task 2 export, Task 3 instantiate, existing `MutationRequest<EditorGraphMutationDto>`, catalog validation snapshot, and projection replacement pipeline.
- Produces: `DuplicateSubgraph` and `InsertSubgraph` mutation variants; one committed delta/revision/history entry per operation.

- [ ] **Step 1: Write failing planner and ProjectState tests**

Add document tests:

```rust
subgraph_duplicate_offsets_every_node_and_excludes_external_edges
subgraph_duplicate_rejects_empty_and_duplicate_node_ids
```

Add production tests:

```rust
subgraph_mutation_advances_one_revision_and_one_history_entry
subgraph_mutation_undoes_and_redoes_in_one_step
subgraph_mutation_returns_complete_delta_and_projection
subgraph_mutation_stale_revision_has_zero_effects
subgraph_mutation_same_revision_allows_exactly_one_commit
```

The history test must compare history lengths before/after, assert `to_revision == from_revision.next()`, undo once, redo once, and compare complete graph contents.

- [ ] **Step 2: Run the mutation tests and verify they fail**

Run:

```sh
pnpm rust:test -- subgraph_mutation
```

Expected: FAIL because mutation variants are absent.

- [ ] **Step 3: Add the mutation variants**

Extend `EditorGraphMutationDto` exactly:

```rust
DuplicateSubgraph {
    node_ids: Vec<NodeId>,
    offset: NodePosition,
},
InsertSubgraph {
    snapshot: ClipboardSubgraphDto,
    anchor: NodePosition,
},
```

Add:

```rust
pub fn duplicate_subgraph(
    graph_path: &GraphResourcePath,
    document: &GraphDocument,
    registry: &NodeRegistry,
    catalog: &CatalogMutationValidationSnapshot,
    node_ids: Vec<NodeId>,
    offset: NodePosition,
) -> Result<GraphDocumentPatch, MutationConflict>;
```

Compute the selected bounds origin, export once, and instantiate at `origin + offset`. Do not expose a `CreateNodes` variant.

- [ ] **Step 4: Route both mutations through the existing commit gate**

In `ProjectState::apply_editor_graph_mutation_observed`, acquire a `CatalogMutationValidationSnapshot` for:

```rust
EditorGraphMutationDto::DuplicateSubgraph { .. }
EditorGraphMutationDto::InsertSubgraph { .. }
```

Pass it into `commit_editor_graph_mutation`; retain the current publication/project-instance/generation checks. Do not hold project locks during filesystem reads used to capture the catalog snapshot.

- [ ] **Step 5: Preserve the existing mutation result contract**

Continue returning the existing `GraphMutationResultDto` containing:

```text
projectInstanceId
delta
projectionReplacement
history
```

Do not add inserted IDs separately; Task 7 extracts them from `delta.payload.operations`.

- [ ] **Step 6: Run focused mutation/history tests**

Run:

```sh
pnpm rust:test -- subgraph_mutation
```

Expected: PASS; each operation increments one revision, adds one history entry, undoes/redoes once, and returns a projection matching committed authority.

- [ ] **Step 7: Run the Rust check**

Run:

```sh
pnpm rust:check
```

Expected: PASS with no warnings introduced by the new mutation arms or exports.

- [ ] **Step 8: Review checkpoint**

Inspect:

```sh
git --no-pager diff -- src-tauri/src/node_system/document/subgraph.rs src-tauri/src/node_system/document/mutation.rs src-tauri/src/project/project_state.rs src-tauri/src/project/production_tests.rs src-tauri/src/event/event_project.rs
```

Expected: all writes still flow through `ProjectState::apply_editor_graph_mutation`. Do not commit.

---

### Task 5: Expose export through IPC and define the TypeScript wire contract

**Files:**
- Modify: `src-tauri/src/commands/command_node_system.rs:1-57,446-548`
- Modify: `src-tauri/src/lib.rs:137-155`
- Modify: `src-tauri/src/project/project_state.rs`
- Modify: `src-tauri/src/project/filesystem/source_audit_tests.rs:10-39`
- Create: `src/shared/types/dto/clipboardSubgraph.ts`
- Create: `src/shared/types/dto/clipboardSubgraphWireParser.ts`
- Create: `src/shared/types/dto/clipboardSubgraphWireParser.test.ts`
- Modify: `src/shared/types/dto/editorMutation.ts:25-54`
- Create: `src/services/nodeSystem/graphSubgraphService.ts`
- Create: `src/services/nodeSystem/graphSubgraphService.test.ts`
- Modify: `src/services/nodeSystem/index.ts`

**Interfaces:**
- Consumes: `ProjectState::export_editor_subgraph`, `ClipboardSubgraphDto`, current project identity, graph path, and selected node IDs.
- Produces: `export_graph_subgraph` Tauri query, TypeScript DTO/parser, `GraphSubgraphService.exportSubgraph`, and duplicate/insert TypeScript mutation variants.

- [ ] **Step 1: Write failing command, parser, and service tests**

Rust command test assertions:

```text
export_graph_subgraph serializes camelCase DTO fields
export_graph_subgraph rejects stale project identity
export_graph_subgraph changes no revision/history/event count
clipboard conflicts map to stable public AppError codes
```

TypeScript parser tests must accept one complete version-1 fixture and reject foreign keys, wrong version, non-array collections, empty local IDs, and non-finite positions.

Service test must assert:

```ts
expect(invoke).toHaveBeenCalledWith('export_graph_subgraph', {
  projectInstanceId: 'project-a',
  graphPath: 'events/main.yssbi-event',
  nodeIds: ['node-a', 'node-b'],
});
```

- [ ] **Step 2: Run focused tests and verify they fail**

Run:

```sh
pnpm rust:test -- export_graph_subgraph
pnpm test -- src/shared/types/dto/clipboardSubgraphWireParser.test.ts src/services/nodeSystem/graphSubgraphService.test.ts
```

Expected: both commands FAIL because the command, DTO parser, and service do not exist.

- [ ] **Step 3: Add the ProjectState read API**

Add:

```rust
pub fn export_editor_subgraph(
    &self,
    project_instance_id: &ProjectInstanceId,
    graph_path: &GraphResourcePath,
    node_ids: Vec<NodeId>,
) -> Result<ClipboardSubgraphDto, MutationConflict>;
```

Capture catalog authority without holding `project_data`; then take a coherent graph snapshot under publication/read locks, validate lifecycle/generation, release locks, and call `export_subgraph`.

- [ ] **Step 4: Add and register the thin Tauri command**

Add:

```rust
#[tauri::command]
pub fn export_graph_subgraph(
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    graph_path: String,
    node_ids: Vec<NodeId>,
) -> Result<ClipboardSubgraphDto, AppError>;
```

Map `ClipboardSubgraphInvalid` and `ReferencedResourceUnavailable` with `AppError::new(error.code(), error.to_string())`. Register the command in `src-tauri/src/lib.rs` and add its name to `IDENTITY_REQUIRED_TAURI_COMMANDS`.

- [ ] **Step 5: Define the TypeScript DTO and strict parser**

Mirror every Rust field in camelCase in `clipboardSubgraph.ts`. Export:

```ts
export interface ClipboardSubgraphDto {
  schemaVersion: 1;
  nodes: ClipboardNodeDto[];
  portBindings: ClipboardPortBindingDto[];
  inputStates: ClipboardInputStateDto[];
  connections: ClipboardConnectionDto[];
}
```

Export from the parser:

```ts
export function parseClipboardSubgraphDto(value: unknown): ClipboardSubgraphDto;
```

Validate exact top-level keys and primitive shapes. Leave protocol/resource/reference semantics to Rust.

- [ ] **Step 6: Extend the editor mutation union**

Add:

```ts
| {
    type: 'duplicateSubgraph';
    payload: { nodeIds: string[]; offset: NodePositionDto };
  }
| {
    type: 'insertSubgraph';
    payload: { snapshot: ClipboardSubgraphDto; anchor: NodePositionDto };
  }
```

- [ ] **Step 7: Implement the service wrapper**

Add:

```ts
export class GraphSubgraphService {
  static async exportSubgraph(
    projectInstanceId: string,
    graphPath: string,
    nodeIds: string[],
  ): Promise<ClipboardSubgraphDto>;
}
```

Invoke only `export_graph_subgraph` and parse the unknown response before returning it.

- [ ] **Step 8: Run focused IPC/wire tests**

Run:

```sh
pnpm rust:test -- export_graph_subgraph
pnpm test -- src/shared/types/dto/clipboardSubgraphWireParser.test.ts src/services/nodeSystem/graphSubgraphService.test.ts
```

Expected: PASS; export remains read-only and malformed responses are rejected at the service boundary.

- [ ] **Step 9: Review checkpoint**

Run:

```sh
git --no-pager diff -- src-tauri/src/commands/command_node_system.rs src-tauri/src/lib.rs src-tauri/src/project/project_state.rs src-tauri/src/project/filesystem/source_audit_tests.rs src/shared/types/dto src/services/nodeSystem
```

Expected: views contain no `invoke`; command logic is parse/call/map only. Do not commit.

---

### Task 6: Replace the frontend-authored clipboard with the system clipboard

**Files:**
- Create: `src/services/clipboard/graphClipboardService.ts`
- Create: `src/services/clipboard/graphClipboardService.test.ts`
- Create: `src/services/clipboard/index.ts`
- Delete: `src/features/core/editor/clipboardSnapshot.ts`
- Delete: `src/features/core/editor/stores/useClipboardStore.ts`
- Modify: `src/features/core/editor/stores/index.ts`
- Modify: `src/features/core/dataStore/projectedEditorCapabilities.test.tsx`

**Interfaces:**
- Consumes: backend-produced `ClipboardSubgraphDto` and browser `navigator.clipboard.readText/writeText`.
- Produces: versioned `GraphClipboardEnvelope`, `writeGraphClipboard`, and `readGraphClipboard`; no Zustand clipboard state.

- [ ] **Step 1: Write failing system-clipboard service tests**

Use an injected/stubbed `navigator.clipboard` and assert:

```ts
const envelope = {
  format: 'application/vnd.yssbi.clipboard-subgraph+json',
  version: 1,
  snapshot,
};
```

Test exact cases:

```text
writes one JSON envelope
reads and parses a valid envelope
rejects non-JSON text
rejects a foreign format
rejects a wrong envelope version
propagates write permission failure
propagates read permission failure
```

- [ ] **Step 2: Run the clipboard service test and verify it fails**

Run:

```sh
pnpm test -- src/services/clipboard/graphClipboardService.test.ts
```

Expected: FAIL because the service does not exist.

- [ ] **Step 3: Implement the system clipboard envelope**

Export:

```ts
export interface GraphClipboardEnvelope {
  format: 'application/vnd.yssbi.clipboard-subgraph+json';
  version: 1;
  snapshot: ClipboardSubgraphDto;
}

export async function writeGraphClipboard(
  snapshot: ClipboardSubgraphDto,
): Promise<void>;

export async function readGraphClipboard(): Promise<ClipboardSubgraphDto>;
```

`writeGraphClipboard` must await `navigator.clipboard.writeText`. `readGraphClipboard` must parse JSON, validate exact envelope format/version, then call `parseClipboardSubgraphDto`.

- [ ] **Step 4: Remove the old clipboard implementation**

Delete `clipboardSnapshot.ts` and `useClipboardStore.ts`; remove the store export. Update `projectedEditorCapabilities.test.tsx` so it tests `canCopyNode` only and no longer expects a projection-authored snapshot.

- [ ] **Step 5: Run focused clipboard and capability tests**

Run:

```sh
pnpm test -- src/services/clipboard/graphClipboardService.test.ts src/features/core/dataStore/projectedEditorCapabilities.test.tsx
```

Expected: PASS; no test imports `buildClipboardSnapshot` or `useClipboardStore`.

- [ ] **Step 6: Confirm no legacy clipboard path remains**

Run:

```sh
pnpm typecheck
```

Expected: FAIL only at Task 7 call sites still importing the deleted APIs; record those exact diagnostics and continue directly to Task 7. No unrelated diagnostics are acceptable.

- [ ] **Step 7: Review checkpoint**

Inspect the diff and confirm there is no in-memory fallback used by cut. Do not commit.

---

### Task 7: Implement copy, cut, paste, duplicate, and committed-delta selection

**Files:**
- Create: `src/features/application/editorMutation/subgraphExportCoordinator.ts`
- Create: `src/features/application/editorMutation/subgraphExportCoordinator.test.ts`
- Create: `src/features/application/editorMutation/insertedNodeIdsFromDelta.ts`
- Create: `src/features/application/editorMutation/insertedNodeIdsFromDelta.test.ts`
- Create: `src/features/core/history/commands/duplicateSubgraph.ts`
- Create: `src/features/core/history/commands/insertSubgraph.ts`
- Modify: `src/features/core/history/commands/registryTypes.ts`
- Modify: `src/features/core/history/commands/index.ts`
- Modify: `src/features/core/history/types.ts`
- Modify: `src/features/core/history/commandExecutor.ts`
- Modify: `src/features/core/history/structuralChange.ts`
- Modify: `src/features/core/history/editorCommands.test.ts`
- Modify: `src/features/application/editor/useEditorOperations.ts`
- Modify: `src/features/application/editor/useEditorOperations.capabilities.test.tsx`
- Modify: `src/features/application/editor/editorMutationAvailability.ts`
- Modify: `src/app/i18n/locales/en-US.ts`
- Modify: `src/app/i18n/locales/zh-CN.ts`

**Interfaces:**
- Consumes: Tasks 5–6 services, current project identity, existing mutation coordinator, one atomic `DeleteNodes`, and committed `GraphDeltaDto`.
- Produces: lifecycle-safe export, result-preserving command execution, duplicate/paste inserted-node selection, and safe copy/cut flows.

- [ ] **Step 1: Write failing delta and lifecycle coordinator tests**

Implement tests for this exact pure function contract:

```ts
export function insertedNodeIdsFromDelta(delta: GraphDeltaDto): string[];
```

Assert it returns only `insert_node.node.id` values in operation order and ignores remove/update/binding/input/connection operations.

For `subgraphExportCoordinator`, simulate project replacement while export is pending and assert the old response is rejected as stale rather than written to clipboard.

- [ ] **Step 2: Write failing command/workflow tests**

Extend command tests to expect one mutation:

```ts
{
  type: 'duplicateSubgraph',
  payload: { nodeIds: ['node-a', 'node-b'], offset: { x: 40, y: 40 } },
}
```

```ts
{
  type: 'insertSubgraph',
  payload: { snapshot, anchor: { x: 120, y: 240 } },
}
```

Extend operations tests with exact order/failure cases:

```text
copy: export then clipboard write
cut success: export, clipboard write, one DeleteNodes, then clear selection
cut clipboard failure: no DeleteNodes and selection preserved
cut delete failure: clipboard retained and selection preserved
paste success: read clipboard, one InsertSubgraph, then select committed IDs
paste failure/stale: original selection preserved
duplicate success: one DuplicateSubgraph and committed IDs selected
all graph identity allocation remains outside the frontend
```

- [ ] **Step 3: Run focused tests and verify they fail**

Run:

```sh
pnpm test -- src/features/application/editorMutation/insertedNodeIdsFromDelta.test.ts src/features/application/editorMutation/subgraphExportCoordinator.test.ts src/features/core/history/editorCommands.test.ts src/features/application/editor/useEditorOperations.capabilities.test.tsx
```

Expected: FAIL because result-preserving commands and authoritative workflows are absent.

- [ ] **Step 4: Implement lifecycle-safe export**

Export:

```ts
export async function exportEditorSubgraph(input: {
  graphPath: string;
  nodeIds: string[];
}): Promise<ClipboardSubgraphDto>;
```

Capture project identity, call `GraphSubgraphService.exportSubgraph`, and assert the same project identity before returning.

- [ ] **Step 5: Preserve command results without breaking boolean callers**

Add:

```ts
export async function executeCommandWithResult<K extends AvailableCommandType>(
  graphPath: string,
  type: K,
  args: Parameters<CommandHandlerMap[K]['execute']>[1],
): Promise<Awaited<ReturnType<CommandHandlerMap[K]['execute']>> | null>;
```

Keep `executeCommand -> Promise<boolean>` as a wrapper over the result-preserving function. Both paths notify structural change exactly once for an applied result.

Register `DuplicateSubgraph` and `InsertSubgraph` command types, handlers, and structural classifications.

- [ ] **Step 6: Implement committed ID extraction**

Implement `insertedNodeIdsFromDelta` exactly as a filter over committed patch operations. Do not inspect the projection to guess which nodes are new, and do not call `crypto.randomUUID()`.

- [ ] **Step 7: Implement authoritative editor operations**

Use these behaviors:

```ts
const DUPLICATE_SUBGRAPH_OFFSET = { x: 40, y: 40 } as const;
```

Copy:

```text
validate all selected nodes are copyable
await exportEditorSubgraph
await writeGraphClipboard
```

Cut:

```text
await exportEditorSubgraph
await writeGraphClipboard
await one DeleteNodes
clear selection only when delete applied
```

Paste:

```text
await readGraphClipboard
await one InsertSubgraph with supplied anchor
when applied, extract inserted IDs from result.delta
set selection after the coordinator has installed projection replacement
```

Duplicate:

```text
await one DuplicateSubgraph with offset (40, 40)
when applied, extract and select committed IDs
```

On mutation failure, preserve selection. On cut deletion failure, show a translated message that copying succeeded but deletion failed.

- [ ] **Step 8: Enable supported mutations and add translated errors**

Set:

```ts
duplicateNodes: true,
pasteNodes: true,
```

Add English and Chinese keys for invalid clipboard, unavailable resource, clipboard read/write failure, export failure, paste failure, duplicate failure, and copy-succeeded-delete-failed.

- [ ] **Step 9: Run focused workflow tests**

Run:

```sh
pnpm test -- src/features/application/editorMutation/insertedNodeIdsFromDelta.test.ts src/features/application/editorMutation/subgraphExportCoordinator.test.ts src/features/core/history/editorCommands.test.ts src/features/application/editor/useEditorOperations.capabilities.test.tsx
```

Expected: PASS; copy/cut ordering and committed-delta selection match the design.

- [ ] **Step 10: Run typecheck**

Run:

```sh
pnpm typecheck
```

Expected: PASS; all deleted clipboard-store imports are gone and mutation result types agree.

- [ ] **Step 11: Review checkpoint**

Inspect the Task 7 diff. Confirm each duplicate/paste/cut graph write is one high-level mutation and selection changes happen only after applied outcomes. Do not commit.

---

### Task 8: Add Ctrl+A, F, Home, and shared viewport fitting

**Files:**
- Create: `src/features/core/viewport/fitViewport.ts`
- Create: `src/features/core/viewport/fitViewport.test.ts`
- Create: `src/features/core/canvas/canvasNodeBounds.ts`
- Create: `src/features/core/canvas/canvasNodeBounds.test.ts`
- Create: `src/features/application/editor/useGraphCanvasCommands.ts`
- Create: `src/features/application/editor/useGraphCanvasCommands.test.tsx`
- Modify: `src/features/core/viewport/editorViewport.ts`
- Modify: `src/features/core/viewport/canvasWheelZoom.ts:12-26`
- Modify: `src/features/core/viewport/index.ts`
- Modify: `src/features/core/canvas/index.ts`
- Modify: `src/features/application/editor/editorSessionTypes.ts`
- Modify: `src/features/application/editor/useEditorSessionCommands.ts`
- Modify: `src/features/application/editor/useEditorKeyboard.ts`
- Modify: `src/features/application/editor/useEditorKeyboard.test.tsx`
- Modify: `src/views/EditorView/EditorWindow.tsx:42-63`

**Interfaces:**
- Consumes: active editor group, graph path, selected node IDs, `[data-editor-group-id]`, `[data-node-id]`, and existing viewport session/persistence APIs.
- Produces: pure bounds fitting plus `selectAllNodes`, `focusSelectedNodes`, and `fitCompleteGraph` editor-session commands.

- [ ] **Step 1: Write failing pure viewport tests**

Define:

```ts
export interface WorldBounds {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export function fitWorldBounds(
  bounds: WorldBounds,
  viewportSize: { width: number; height: number },
  options?: { padding?: number; minScale?: number; maxScale?: number },
): EditorViewport;
```

Tests must assert 64px default padding, centered output, scale clamp `[0.1, 5]`, finite single-point bounds, and no mutation of input objects.

- [ ] **Step 2: Write failing canvas-command and keyboard tests**

Assert:

```text
Ctrl+A selects all selectable nodes in graph order
Meta+A behaves like Ctrl+A
F fits only current selected node bounds
F with empty selection does nothing
Home fits all node bounds
F/Home call no graph mutation and do not mark graph dirty
input/contenteditable/modal guards still suppress graph shortcuts
only the active editor group is affected
```

- [ ] **Step 3: Run focused tests and verify they fail**

Run:

```sh
pnpm test -- src/features/core/viewport/fitViewport.test.ts src/features/core/canvas/canvasNodeBounds.test.ts src/features/application/editor/useGraphCanvasCommands.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx
```

Expected: FAIL because fit and canvas commands do not exist.

- [ ] **Step 4: Centralize viewport scale limits and implement fitting**

Export from `editorViewport.ts`:

```ts
export const EDITOR_VIEWPORT_SCALE_LIMITS = { min: 0.1, max: 5 } as const;
```

Make wheel zoom reuse this constant. Implement `fitWorldBounds` with:

```text
scale = clamp(min(availableWidth / boundsWidth, availableHeight / boundsHeight))
x = viewportWidth / 2 - boundsCenterX * scale
y = viewportHeight / 2 - boundsCenterY * scale
```

Use a positive finite extent for zero-width/height bounds.

- [ ] **Step 5: Implement DOM-to-world bounds collection**

Export:

```ts
export function collectCanvasNodeWorldBounds(input: {
  canvasElement: HTMLElement;
  viewport: EditorViewport;
  nodeIds?: readonly string[];
}): WorldBounds | null;
```

Read live node `getBoundingClientRect()` values and reverse the viewport transform relative to the active canvas bounds. Return `null` when no requested node is present.

- [ ] **Step 6: Implement group-scoped canvas commands**

Export a hook returning:

```ts
{
  selectAllNodes(): void;
  focusSelectedNodes(): void;
  fitCompleteGraph(): void;
}
```

Select all only when the active resource is a loaded graph, and filter Rust-managed/non-selectable nodes. For F/Home, call `setViewportLive`, `commitViewport`, and `persistGraphViewport`; perform no IPC.

- [ ] **Step 7: Wire commands through EditorSession and keyboard**

Add the three command functions to the explicit session slice, merge them in `useEditorSessionCommands`, pass them from `EditorWindow`, and route:

```text
Ctrl/Meta+A -> selectAllNodes
plain F -> focusSelectedNodes
plain Home -> fitCompleteGraph
```

Call `preventDefault()` only when a graph command is actually routed.

- [ ] **Step 8: Run focused viewport/keyboard tests**

Run:

```sh
pnpm test -- src/features/core/viewport/fitViewport.test.ts src/features/core/canvas/canvasNodeBounds.test.ts src/features/application/editor/useGraphCanvasCommands.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx
```

Expected: PASS; F/Home remain frontend-only and active-group scoped.

- [ ] **Step 9: Review checkpoint**

Inspect the diff and confirm no global listener bypasses `globalEvent.ts`, and no viewport function imports graph mutation services. Do not commit.

---

### Task 9: Make Shift box selection union with the session-start selection

**Files:**
- Modify: `src/features/core/canvas/selectionSession.ts`
- Modify: `src/features/core/canvas/useCanvasInteraction.ts:74-96`
- Modify: `src/features/core/canvas/canvasPointerLoop.ts:52-68,160-183`
- Modify: `src/features/core/canvas/selectionHitTargets.test.ts`
- Modify: `src/features/core/canvas/canvasPointerLoop.test.ts`

**Interfaces:**
- Consumes: editor-group selection captured at pointerdown and current frame hit IDs.
- Produces: `baseNodeIds` selection sessions and deterministic `unionSelectionIds` used for both preview and pointerup.

- [ ] **Step 1: Write failing union/session tests**

Add exact cases:

```text
base [a,b] plus hits [b,c] returns [a,b,c]
repeated hit IDs are deduplicated
Shift drag uses selection captured at pointerdown even if store changes mid-session
Shift empty drag preserves base selection
plain drag replaces selection with hits
Shift blank click preserves base selection
plain blank click clears selection
preview and final selection use the same union order
```

- [ ] **Step 2: Run focused selection tests and verify they fail**

Run:

```sh
pnpm test -- src/features/core/canvas/selectionHitTargets.test.ts src/features/core/canvas/canvasPointerLoop.test.ts
```

Expected: FAIL because finalization currently replaces selection with hit IDs.

- [ ] **Step 3: Replace the boolean session flag with captured IDs**

Change session state to:

```ts
export type ActiveSelectionSession = {
  active: true;
  groupId: string;
  startX: number;
  startY: number;
  currentX: number;
  currentY: number;
  baseNodeIds: readonly string[];
};
```

Change start input to:

```ts
export function startSelectionSession(input: {
  groupId: string;
  startX: number;
  startY: number;
  baseNodeIds: readonly string[];
}): void;
```

- [ ] **Step 4: Add deterministic union and capture at pointerdown**

Export:

```ts
export function unionSelectionIds(
  baseNodeIds: readonly string[],
  hitNodeIds: readonly string[],
): string[];
```

Keep base order, then append unseen hit IDs in hit-test order. At pointerdown pass the current group selection when `shiftKey` is true and `[]` otherwise.

- [ ] **Step 5: Use union for preview and finalization**

In each animation frame, preview `unionSelectionIds(session.baseNodeIds, hits)`. On pointerup, send that same union to `setSelectedNodeIds`. A no-movement Shift click preserves base IDs; a no-modifier blank click clears selection.

- [ ] **Step 6: Run focused selection tests**

Run:

```sh
pnpm test -- src/features/core/canvas/selectionHitTargets.test.ts src/features/core/canvas/canvasPointerLoop.test.ts
```

Expected: PASS; preview/final results are identical and Shift behavior matches Shift-click semantics.

- [ ] **Step 7: Review checkpoint**

Inspect only the five Task 9 files and confirm pointer movement still performs no IPC and remains animation-frame throttled. Do not commit.

---

### Task 10: Hide permanently unsupported menu items

**Files:**
- Modify: `src/views/EditorView/ContextMenu/NodeContextMenu.tsx:1-78`
- Modify: `src/views/EditorView/ContextMenu/PinContextMenu.tsx:1-90`
- Create: `src/views/EditorView/ContextMenu/NodeContextMenu.test.tsx`
- Create: `src/views/EditorView/ContextMenu/PinContextMenu.test.tsx`

**Interfaces:**
- Consumes: existing context-menu component and runtime capabilities.
- Produces: menus that retain supported-but-currently-unavailable disabled actions while omitting permanently unsupported actions.

- [ ] **Step 1: Write failing rendering tests**

Assert Node menu omits translated labels/IDs for:

```text
disable
rename
collapse
```

Assert Pin menu omits:

```text
promoteToVar
```

Also assert copy/cut/duplicate/delete and break/reset/remove actions still render, with runtime capability-driven disabled state where applicable.

- [ ] **Step 2: Run focused menu tests and verify they fail**

Run:

```sh
pnpm test -- src/views/EditorView/ContextMenu/NodeContextMenu.test.tsx src/views/EditorView/ContextMenu/PinContextMenu.test.tsx
```

Expected: FAIL because permanently unsupported items are currently rendered disabled.

- [ ] **Step 3: Remove permanently unsupported items**

Delete the complete Node menu section containing disable/rename/collapse and remove unused `VscEdit`. Remove the Pin `promoteToVar` push and unused `VscSymbolVariable`.

Duplicate is now supported by Task 7, so render it as a normal action and remove the unavailable-message title/flag dependency. Keep `disabled` only for valid operations unavailable for the current node/pin.

- [ ] **Step 4: Run focused menu tests**

Run:

```sh
pnpm test -- src/views/EditorView/ContextMenu/NodeContextMenu.test.tsx src/views/EditorView/ContextMenu/PinContextMenu.test.tsx
```

Expected: PASS; unsupported items are absent from the DOM.

- [ ] **Step 5: Review checkpoint**

Inspect the menu diff and ensure context-menu compact spacing/radius behavior remains in shared primitives and is unchanged. Do not commit.

---

### Task 11: Run Phase 3 acceptance verification

**Files:**
- Verify all files listed in Tasks 2–10
- Do not modify unrelated files to silence pre-existing diagnostics

**Interfaces:**
- Consumes: every Phase 3 deliverable and Phase 1/2 prerequisites.
- Produces: fresh evidence that the complete cross-stack behavior satisfies the approved design.

- [ ] **Step 1: Run all focused Rust subgraph tests**

Run:

```sh
pnpm rust:test -- subgraph
```

Expected: PASS; export, insert, duplicate, history, stale, concurrency, command, and limits tests all pass.

- [ ] **Step 2: Run the focused frontend Phase 3 suite**

Run:

```sh
pnpm test -- src/shared/types/dto/clipboardSubgraphWireParser.test.ts src/services/nodeSystem/graphSubgraphService.test.ts src/services/clipboard/graphClipboardService.test.ts src/features/application/editorMutation/insertedNodeIdsFromDelta.test.ts src/features/application/editorMutation/subgraphExportCoordinator.test.ts src/features/core/history/editorCommands.test.ts src/features/application/editor/useEditorOperations.capabilities.test.tsx src/features/core/viewport/fitViewport.test.ts src/features/core/canvas/canvasNodeBounds.test.ts src/features/application/editor/useGraphCanvasCommands.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx src/features/core/canvas/selectionHitTargets.test.ts src/features/core/canvas/canvasPointerLoop.test.ts src/views/EditorView/ContextMenu/NodeContextMenu.test.tsx src/views/EditorView/ContextMenu/PinContextMenu.test.tsx
```

Expected: PASS; the focused suite reports zero failed tests.

- [ ] **Step 3: Run frontend and Rust static checks**

Run:

```sh
pnpm typecheck
pnpm rust:fmt:check
pnpm rust:check
```

Expected: all three commands PASS with no new diagnostics.

- [ ] **Step 4: Run the required cross-stack verification**

Run:

```sh
pnpm verify
```

Expected: PASS; frontend tests, Rust checks/tests, scientific Rust tests, and `git diff --check` all complete successfully.

- [ ] **Step 5: Inspect scope and whitespace**

Run:

```sh
git --no-optional-locks status --short
git --no-pager diff --check
git --no-pager diff --stat
```

Expected: `git diff --check` emits no output; status contains only Phase 3 files plus pre-existing user changes; no generated `src-tauri/target/` appears.

- [ ] **Step 6: Acceptance review checkpoint**

Review the final diff against these exact acceptance statements:

```text
Export preserves portable nodes, parameters, labels, dynamic bindings, literals, reroutes, and internal edges only.
Duplicate and insert allocate fresh authority IDs and commit atomically.
Cut never deletes before a successful system-clipboard write.
Duplicate/paste selection comes only from committed insert_node delta operations after projection installation.
Ctrl+A/F/Home and Shift box previews issue no IPC.
F/Home create no graph history entry.
Permanently unsupported menu items are hidden.
All failures preserve graph authority and current selection unless a mutation committed successfully.
```

Do not commit. If and only if the user explicitly requests a commit after reviewing this checkpoint, create one concise Git commit following the repository commit-message rules.
