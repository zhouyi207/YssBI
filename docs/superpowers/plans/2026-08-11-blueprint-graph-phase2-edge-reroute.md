# Blueprint Graph Phase 2 Edge Reroute Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Phase 2 edge editing and persistent compiler-transparent reroute nodes: wide edge hit testing, mutually exclusive edge selection, edge context/delete interactions, one atomic double-click reroute mutation, compact reroute projection/rendering, and data/control/effect semantic transparency.

**Architecture:** Phase 1 remains the required foundation and owns atomic disconnection, the explicit canvas interaction state, and authoritative mutation publication. Phase 2 adds three hidden built-in reroute protocols marked with one registry-owned transparent behavior; `InsertReroute` expands one connection intent into one deterministic document patch, while a small compiler normalization module collapses reroute chains only in validated semantic input to lowering. React renders persisted reroute projections by Rust-authored `styleId`, keeps node/edge selections mutually exclusive, and sends exactly one `DisconnectConnections` or `InsertReroute` intent without optimistic topology edits.

**Tech Stack:** Rust, serde, Tauri 2, React 19, TypeScript 5.8, Zustand 5, SVG, shadcn/ui, Vitest 4, pnpm 11.

## Global Constraints

- Scope is strictly `docs/superpowers/specs/2026-08-11-blueprint-style-graph-interaction-design.md` Delivery Phase 2 (`§11`, `§12`, `§15`, `§16 Phase 2`, and only the Phase 2 cases from `§17`).
- Phase 1 plan `docs/superpowers/plans/2026-08-11-blueprint-graph-phase1-atomic-interactions.md` must be fully implemented and its focused acceptance matrix must pass before starting this plan.
- Do not implement Phase 3 subgraph export/duplicate/copy/paste/cut, `Ctrl+A`, `Ctrl+C`, `Ctrl+V`, `Ctrl+X`, `Ctrl+D`, `F`, `Home`, viewport fitting/focusing, committed-delta selection, or Shift-box union changes.
- `ProjectState.project_data` remains authoritative. React must not author patches, allocate/predict node or connection IDs, optimistically modify topology, or infer backend-loaded state from `graphEntities` alone.
- Preserve the Phase 1 mutation route: frontend intent → `GraphMutationService` → `mutate_graph_document` → `ProjectState::apply_editor_graph_mutation` → one patch/history entry/revision → one authoritative projection result/event installation.
- `InsertReroute` is one user action, one `EditorGraphMutationDto`, one `GraphDocumentPatch`, one revision, one history entry, and one emitted graph delta. Double click submits no preliminary disconnect or create request.
- Edge deletion/break submits exactly one Phase 1 `DisconnectConnections { connection_ids }` mutation. Do not reintroduce per-edge loops or the removed singular `Disconnect` variant.
- Node and connection selections are mutually exclusive. `Ctrl`/`Meta` or `Shift` toggles only within the active selection kind; box selection remains node-only and retains its Phase 1 behavior without fixing Phase 3 Shift-box union semantics.
- Mutation failure preserves topology, revision, history, projection, and selection. Clear deleted/replaced edge selection only after an applied authoritative result.
- Reroute nodes persist in `GraphDocument` and normal projection/save/history flows, but create no runtime operation, kernel, or identity step.
- The data reroute has one declared `Single` input and one declared unbounded unordered `Multiple` output sharing one generic type parameter. Control and effect reroutes each have one declared `Single` input and one declared unbounded unordered `Multiple` output of the matching kind.
- Preserve an original ordered target's `OrderKey` only on the reroute-output → original-input connection. The original-output → reroute-input connection always has `order: None`.
- Undo must restore the original `DocumentConnection`, including its original `ConnectionId` and `OrderKey`, in one step.
- Global `window`/`document` listeners continue to use `src/shared/utils/globalEvent.ts`. This phase adds no new global keyboard shortcuts beyond extending Phase 1 Delete/Escape handling for edge selection.
- Use shadcn/ui/shared context-menu primitives and shared i18n/toast behavior; do not add another UI library or browser/native dialogs.
- Run all commands from the worktree root. Every Rust compile/test command in this plan includes `--jobs 1`; every Rust test command also includes `-- --test-threads=1`.
- Every Vitest command includes `--pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1`.
- Do not create commits. Ignore generic skill instructions to commit; each task ends at a review/verification checkpoint only.
- Preserve current unrelated user changes in `src/features/application/editor/handleGraphResourceDrop.test.ts`, `src/views/EditorView/Layout/sidebar/**`, `src/views/EditorView/Layout/sidebarUi/SidebarDraggableItem.test.tsx`, and `docs/superpowers/plans/2026-08-11-sidebar-resource-node-drag.md`.
- Do not modify any file under `src-tauri/src/node_system/catalog/dataframe/`.
- The main working tree (`../..` when commands run from this worktree) currently has uncommitted user changes in `src-tauri/src/node_system/compiler/mod.rs`, `pipeline.rs`, `diagnostics.rs`, `dynamic_interface.rs`, `project.rs`, `schema_analysis.rs`, `tests_dynamic.rs`, `tests_dynamic_pipeline.rs`, `src-tauri/src/node_system/catalog/dataframe/mod.rs`, `catalog/dataframe/tests.rs`, plus untracked `src-tauri/src/node_system/compiler/dataframe.rs`. The Phase 2 worktree status cannot detect or protect those changes.
- Before any Phase 2 compiler file is edited, the controller must execute the explicit three-way compiler conflict gate in Task 3. The executor must stop at that gate; a clean Phase 2 worktree is not approval to proceed. Never overwrite, reset, stash, clean, copy over, or otherwise discard the main working tree user patch.

---

## Audited Baseline, Phase 1 Dependency, and Locked File Map

### Current baseline audited before Phase 1

- `src/views/EditorView/Canvas/core/Edge.tsx` computes one SVG path and renders only pointer-transparent visible/animation paths; its `<g>` already exposes `data-edge-id`.
- `src/views/EditorView/Canvas/core/EdgesOverlay.tsx` builds edge identity/kind/pin data from `graphDataStore`, renders the SVG with `pointer-events-none`, and has no selection, context menu, or double-click callback.
- `src/features/core/canvas/edgePath.ts` is the shared path geometry source. Hit paths must reuse the same `d` value through `Edge`; do not add a second Bézier implementation.
- `src/views/EditorView/Canvas/core/Canvas.tsx` composes `EdgesOverlay` before nodes inside `TransformContainer`, obtains world conversion from `useCanvasViewport`, and currently consumes node selection from `useEditorGroup`.
- `src/features/core/layout/editorTabStore.ts` stores volatile `selectedNodeIds` per editor group; there is no connection selection field.
- `src/features/application/editor/useEditorOperations.ts` owns Delete behavior and currently reads node selection only.
- `src/views/EditorView/ContextMenu/NodeContextMenu.tsx` demonstrates the required compact shared `ContextMenu` pattern.
- `src-tauri/src/node_system/document/mutation.rs` owns `EditorGraphMutationDto` and patch planning. Before Phase 1 it has singular delete/disconnect and append-only connect behavior; execute this plan only after Phase 1 replaces those interfaces.
- `src-tauri/src/node_system/catalog/core_nodes/` builds focused built-in families through `ProviderFragment`; `support.rs` owns reusable protocol/port helpers.
- `src-tauri/src/node_system/registry/model.rs` currently distinguishes leaf and structural behaviors. A reroute must receive an explicit third behavior rather than a fake no-op leaf kernel.
- `src-tauri/src/node_system/compiler/pipeline.rs` builds a complete analysis snapshot from the persisted document, validates types/cycles, derives a semantic graph, then lowers every non-structural semantic node. A protocol-only or fake leaf reroute would either fail lowering or create a forbidden runtime operation.
- `src-tauri/src/node_system/analysis/projection.rs` projects every persisted document node and connection using analysis interfaces plus Rust-authored `display.styleId`; this allows compact reroute rendering without adding a second projection protocol.

### Exact Phase 1 interfaces consumed by this plan

Phase 2 must consume the completed Phase 1 interfaces exactly; do not add fallback support for the pre-Phase-1 names:

```rust
EditorGraphMutationDto::DisconnectConnections {
    connection_ids: Vec<ConnectionId>,
}
```

```ts
{ type: 'disconnectConnections'; payload: { connectionIds: string[] } }
```

```ts
executeCommand(graphPath, 'DisconnectConnections', {
  connectionIds: string[],
}): Promise<boolean>
```

```ts
type CanvasInteraction =
  | { type: 'idle' }
  | { type: 'panning'; session: PanSession }
  | { type: 'selecting'; session: SelectionSession }
  | { type: 'draggingNodes'; session: NodeDragSession }
  | { type: 'drawingConnection'; session: ConnectionDrawSession }
  | { type: 'movingConnections'; session: ConnectionMoveSession }
  | { type: 'pendingNodeCreation'; session: PendingNodeCreationSession };

cancelInteraction(graphPath: string): void;
```

```ts
interface PortConnectionCapabilityDto {
  current: number;
  maximum: number | null;
  ordered: boolean;
  canAppend: boolean;
  canReplace: boolean;
  canMove: boolean;
}
```

Phase 1 also guarantees that `DeleteNodes`, `DisconnectPort`, and `DisconnectNode` each submit one authoritative intent, and that Phase 1 Escape precedence cancels transient graph interaction before clearing node selection or leaving Zen Mode.

### New stable Phase 2 interfaces

```rust
EditorGraphMutationDto::InsertReroute {
    connection_id: ConnectionId,
    position: NodePosition,
}
```

```ts
{ type: 'insertReroute'; payload: {
  connectionId: string;
  position: { x: number; y: number };
} }
```

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum TransparentNodeRole {
    Reroute,
}
```

```ts
export interface GraphSelection {
  nodeIds: Set<string>;
  connectionIds: Set<string>;
}
```

The persisted Zustand/Immer placement representation remains serializable arrays (`selectedNodeIds`, `selectedConnectionIds`); `getEditorGroupGraphSelection` is the sole adapter that constructs the public `Set`-based `GraphSelection` value.

### Files expected to change

- Rust catalog/registry: `src-tauri/src/node_system/catalog/core_nodes/reroute.rs` (new), `src-tauri/src/node_system/catalog/core_nodes/mod.rs`, `src-tauri/src/node_system/catalog/core_nodes/support.rs`, `src-tauri/src/node_system/catalog/mod.rs`, `src-tauri/src/node_system/catalog/tests.rs`, `src-tauri/src/node_system/registry/model.rs`, `src-tauri/src/node_system/registry/mod.rs`, `src-tauri/src/node_system/registry/validation.rs`, `src-tauri/src/node_system/registry/tests.rs`.
- Rust mutation/authority: `src-tauri/src/node_system/document/mutation.rs`, `src-tauri/src/node_system/document/tests.rs`, `src-tauri/src/node_system/document/tests/editor_mutation_validation.rs`, new focused `src-tauri/src/node_system/document/tests/insert_reroute.rs`, new focused `src-tauri/src/project/editor_reroute_mutation_tests.rs`, and one `mod editor_reroute_mutation_tests;` declaration in `src-tauri/src/project/mod.rs`. Do not add Phase 2 cases to `src-tauri/src/project/production_tests.rs`.
- Rust command event coverage: new focused `src-tauri/src/commands/command_node_system_reroute_tests.rs` loaded as a child test module from `src-tauri/src/commands/command_node_system.rs`; production command workflow remains unchanged.
- Rust projection/compiler: `src-tauri/src/node_system/analysis/projection.rs`, `src-tauri/src/node_system/compiler/reroute.rs` (new), `src-tauri/src/node_system/compiler/reroute_tests.rs` (new), `src-tauri/src/node_system/compiler/mod.rs`, and a minimal integration edit in `src-tauri/src/node_system/compiler/pipeline.rs` only after the controller conflict gate.
- Frontend DTO/command: `src/shared/types/dto/editorMutation.ts`, `src/shared/types/dto/editorMutationWireParser.ts`, their focused tests, `src/features/core/history/types.ts`, `src/features/core/history/commands/insertReroute.ts` (new), command indexes/registry, and `src/features/core/history/editorCommands.test.ts`.
- Frontend selection/interaction: `src/features/core/layout/editorTabStore.ts`, `src/features/core/layout/layoutTabQueries.ts`, focused layout tests, `src/features/application/editor/editorSessionTypes.ts`, the audited real hook `src/features/core/editor/hooks/useActiveEditorGroup.ts`, `src/features/application/editor/useEditorOperations.ts`, new focused `src/features/application/editor/edgeOperations.ts` and `edgeOperations.test.ts`, `src/features/core/canvas/useCanvasInteraction.ts`, and `src/features/application/editor/useEditorKeyboard.ts` tests.
- Frontend edge/rendering: `src/views/EditorView/Canvas/core/Edge.tsx`, `EdgesOverlay.tsx`, `Canvas.tsx`, their focused tests, `src/views/EditorView/ContextMenu/ConnectionContextMenu.tsx` (new), context-menu index/i18n, `src/views/EditorView/Nodes/RerouteNodeLayout.tsx` (new), `Node.tsx`, `NodeContainer.tsx`, node style utilities/tests, and focused reroute rendering tests.

---

### Task 1: Register persisted reroute protocols with explicit transparent behavior

**Files:**
- Create: `src-tauri/src/node_system/catalog/core_nodes/reroute.rs`
- Modify: `src-tauri/src/node_system/catalog/core_nodes/mod.rs`
- Modify: `src-tauri/src/node_system/catalog/core_nodes/support.rs`
- Modify: `src-tauri/src/node_system/catalog/mod.rs`
- Modify: `src-tauri/src/node_system/catalog/tests.rs`
- Modify: `src-tauri/src/node_system/registry/model.rs:12-126`
- Modify: `src-tauri/src/node_system/registry/mod.rs:362-417`
- Modify: `src-tauri/src/node_system/registry/validation.rs:314-371`
- Modify: `src-tauri/src/node_system/registry/tests.rs`

**Interfaces:**
- Produces stable built-in IDs and declared port keys:

```rust
pub(crate) const DATA_REROUTE_NODE_TYPE: &str = "yssbi.reroute.data";
pub(crate) const CONTROL_REROUTE_NODE_TYPE: &str = "yssbi.reroute.control";
pub(crate) const EFFECT_REROUTE_NODE_TYPE: &str = "yssbi.reroute.effect";
pub(crate) const REROUTE_INPUT_PORT: &str = "input";
pub(crate) const REROUTE_OUTPUT_PORT: &str = "output";
```

- Produces registry behavior:

```rust
pub enum TransparentNodeRole { Reroute }

RegisteredNode::transparent(
    protocol: Arc<NodeProtocol>,
    role: TransparentNodeRole,
) -> RegisteredNode

RegisteredNode::transparent_role(&self) -> Option<TransparentNodeRole>
```

- `RegisteredNode` behavior is exactly one of leaf, structural, or transparent; no protocol-only reroute and no no-op lowerer are allowed.
- All three protocols use `catalog.hidden = true` and `catalog.style_id = "builtin.reroute"` so they persist/project but do not appear in the ordinary palette.

- [ ] **Step 1 (RED): Add registry behavior exclusivity and fingerprint tests**

In `registry/tests.rs`, add tests prefixed `phase2_transparent_registry_` that build a minimal transparent protocol and assert:

```rust
assert_eq!(registered.transparent_role(), Some(TransparentNodeRole::Reroute));
assert!(registered.implementation().is_none());
assert!(registered.structural_role().is_none());
```

Also assert registry validation rejects any internal fixture combining transparent with leaf or structural behavior, and canonical registry fingerprint JSON contains:

```json
{ "kind": "Transparent", "role": "Reroute" }
```

- [ ] **Step 2 (RED): Add exact built-in reroute protocol contract tests**

In `catalog/tests.rs`, add `phase2_reroute_protocol_` tests for all three IDs. Assert they are hidden, use `builtin.reroute`, have no parameters/dynamic ports/managed role, and have exactly `input` then `output` declared ports. Lock capacities and kinds:

```rust
assert_eq!(input.connections, ConnectionsPerPort::Single);
assert_eq!(output.connections, ConnectionsPerPort::Multiple {
    max: None,
    ordered: false,
});
```

For data, assert one type parameter `T`, both ports use `TypeExpr::Generic(T)`, input literals are forbidden, and output schema is `Some(SchemaExpr::Input(input_key))`. For control/effect, assert matching `PortKind`, `TypeExpr::Unknown`, and no literal/schema contracts.

- [ ] **Step 3: Run focused Rust tests and confirm RED**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_transparent_registry_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_protocol_ --jobs 1 -- --test-threads=1
```

Expected: FAIL because transparent behavior and reroute protocols do not exist.

- [ ] **Step 4 (GREEN): Add the third closed registry behavior**

Add `transparent_role: Option<TransparentNodeRole>` beside the existing implementation/structural fields. Update constructors, debug output, validation, and canonical fingerprint matching so exactly these combinations are valid:

```rust
(Some(_), None, None)       // leaf
(None, Some(_), None)       // structural
(None, None, Some(_))       // compiler-transparent
```

All other combinations fail registry validation. Extend `CompilerRegistry::resolve` in Task 3; do not make transparent nodes look structural or executable here.

- [ ] **Step 5 (GREEN): Build the three focused hidden protocols**

In `core_nodes/reroute.rs`, register data, control, and effect variants through one shared helper parameterized by `PortKind`. Construct the output capacity explicitly as unbounded unordered `Multiple`; do not reuse helpers that force `Single` output. Use pure execution semantics for data/control and effectful ordered semantics for effect, but attach no kernel.

Export only the stable IDs/port keys and a narrow catalog helper through `catalog/mod.rs`:

```rust
pub(crate) fn reroute_node_type_for_kind(kind: PortKind) -> NodeTypeId;
```

The helper must match all three `PortKind` values and return the constants above; callers must not duplicate strings.

- [ ] **Step 6: Run focused catalog/registry tests and Rust check (GREEN)**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_transparent_registry_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_protocol_ --jobs 1 -- --test-threads=1
pnpm exec cargo check --manifest-path src-tauri/Cargo.toml -p yssbi --jobs 1
```

Expected: PASS. The existing compiler may still classify the newly registered transparent node as protocol-only until Task 3; do not add compiler behavior mapping or normalization here. Any compiler-specific failure is recorded and deferred until Task 3 has installed and run its RED tests behind the controller conflict gate.

- [ ] **Step 7: Review checkpoint (no commit)**

Confirm hidden reroutes are absent from localized palette search output, no runtime kernel handle contains `reroute`, no dataframe path changed, and do not commit.

---

### Task 2: Specify and implement atomic `InsertReroute` through dedicated RED suites

**Files:**
- Create: `src-tauri/src/node_system/document/tests/insert_reroute.rs`
- Modify: `src-tauri/src/node_system/document/tests.rs` only to declare `mod insert_reroute;`
- Create: `src-tauri/src/project/editor_reroute_mutation_tests.rs`
- Modify: `src-tauri/src/project/mod.rs` only to declare `#[cfg(test)] mod editor_reroute_mutation_tests;`
- Create: `src-tauri/src/commands/command_node_system_reroute_tests.rs`
- Modify: `src-tauri/src/commands/command_node_system.rs` only to load the child test module plus any proven exhaustive DTO/error mapping change; keep the command workflow unchanged
- Modify after all RED runs: `src-tauri/src/node_system/document/mutation.rs:262-552,838-1124`
- Do not modify: `src-tauri/src/project/production_tests.rs`

**Interfaces:**
- Consumes Task 1 `reroute_node_type_for_kind`, `REROUTE_INPUT_PORT`, and `REROUTE_OUTPUT_PORT`.
- Produces exact serde variant:

```rust
InsertReroute {
    connection_id: ConnectionId,
    position: NodePosition,
}
```

- Produces private planner:

```rust
fn insert_reroute_operations(
    document: &GraphDocument,
    registry: &NodeRegistry,
    connection_id: ConnectionId,
    position: NodePosition,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict>
```

- Successful operation ordering is exact: `RemoveConnection`, `InsertNode`, source-side `InsertConnection`, target-side `InsertConnection`.
- `editor_reroute_mutation_tests.rs` is the Phase 2 authority matrix owner. `command_node_system_reroute_tests.rs` is the real command emitter matrix owner. Neither suite is appended to the large `production_tests.rs`.

- [ ] **Step 1 (RED): Add all document wire/planner tests before the variant exists**

In the new `document/tests/insert_reroute.rs`, add tests prefixed `phase2_insert_reroute_document_` for exact camelCase wire shape, unknown/extra fields, non-finite position rejection, data/control/effect protocol selection, exact four-operation order, two fresh connection IDs, source `order: None`, downstream original `OrderKey`, missing/orphan/malformed endpoints, and patch/inverse byte-for-byte restoration of the original connection ID/order.

The locked wire value is:

```json
{
  "type": "insertReroute",
  "payload": {
    "connectionId": "00000000-0000-0000-0000-000000000101",
    "position": { "x": 120.5, "y": -30.0 }
  }
}
```

- [ ] **Step 2 (RED): Add the complete project authority matrix before planner implementation**

In new `project/editor_reroute_mutation_tests.rs`, build a focused fixture around the Phase 1 `apply_editor_graph_mutation_observed` test hooks. Add these exact prefixes and assertions:

- `phase2_reroute_authority_success_`: one revision increment, one undo entry, unchanged redo precondition, complete delta with one remove/one node insert/two connection inserts, returned projection at `to_revision`, persisted reroute plus two projected edges, and observer callback exactly once.
- `phase2_reroute_authority_failure_`: missing connection and invalid position leave document, revision ledger, undo/redo stacks, publication state, returned/installed projection snapshot, and observer count unchanged/zero.
- `phase2_reroute_authority_stale_`: stale base revision returns the Phase 1 stable conflict, changes no state, and calls observer zero times.
- `phase2_reroute_authority_history_`: one Undo restores exact original connection ID/`OrderKey` and removes reroute/two replacement edges; one Redo restores the exact committed reroute node ID and replacement connection IDs; each action advances exactly one revision/history publication.
- `phase2_reroute_authority_concurrency_`: two threads submit at the same base revision through the existing publication barrier/hook; exactly one commits, one is stale, one observer delta exists, and authority/history/projection describe only the winner.

Use Phase 1-equivalent assertions, including:

```rust
assert_eq!(result.delta.to_revision.get(), result.delta.from_revision.get() + 1);
assert_eq!(undo_len_after, undo_len_before + 1);
assert_eq!(observed_deltas, vec![result.delta.clone()]);
assert_eq!(result.projection_replacement.projection.source_revision,
           result.delta.to_revision.get());
```

- [ ] **Step 3 (RED): Add real command event emission tests before planner implementation**

Load `command_node_system_reroute_tests.rs` as a child module of `command_node_system.rs` so it can call private `mutate_graph_document_with_emitter` without widening production visibility. Add:

- `phase2_reroute_command_event_success_emits_once`: invoke the real command helper with serialized `InsertReroute`; assert one `Event::Project(EventProject::GraphDelta)` whose delta equals the returned result and no second event.
- `phase2_reroute_command_event_failure_emits_zero`: invalid connection ID returns an error and emitter vector remains empty.
- `phase2_reroute_command_event_stale_emits_zero`: stale base revision returns the Phase 1 conflict code and emitter vector remains empty.

Do not mock the emitter boundary and do not add a reroute-specific Tauri command.

- [ ] **Step 4: Run every new suite and confirm RED before production implementation**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_insert_reroute_document_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_authority_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_command_event_ --jobs 1 -- --test-threads=1
```

Expected: FAIL to compile or assert because `InsertReroute` and its planner do not exist. Do not continue unless the failure is specifically attributable to missing Phase 2 behavior rather than a broken fixture.

- [ ] **Step 5 (GREEN): Implement only the four-operation document planner**

Implement the planner in this order:

1. `validate_position(position)`.
2. Clone the original `DocumentConnection` by `connection_id` or return `DocumentError::ConnectionNotFound`.
3. Resolve both original endpoints through `resolve_mutation_port`; reject orphan, direction mismatch, or kind mismatch.
4. Resolve the Task 1 protocol from the original endpoint kind and verify its transparent role plus exact declared `input`/`output` contract.
5. Allocate one `NodeId` and two new `ConnectionId`s only after all validation.
6. Stage-apply this exact patch to a cloned document before returning it:

```rust
vec![
    GraphDocumentOperation::RemoveConnection { connection: original.clone() },
    GraphDocumentOperation::InsertNode { node: reroute_node },
    GraphDocumentOperation::InsertConnection {
        connection: DocumentConnection {
            id: source_connection_id,
            output: original.output.clone(),
            input: PortAddress::declared(reroute_id, input_key),
            order: None,
        },
    },
    GraphDocumentOperation::InsertConnection {
        connection: DocumentConnection {
            id: target_connection_id,
            output: PortAddress::declared(reroute_id, output_key),
            input: original.input.clone(),
            order: original.order.clone(),
        },
    },
]
```

Do not call `Connect` twice, commands, history, project state, or compiler APIs from the planner.

- [ ] **Step 6: Run document, authority, and command matrices (GREEN)**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_insert_reroute_document_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_authority_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_command_event_ --jobs 1 -- --test-threads=1
pnpm exec cargo check --manifest-path src-tauri/Cargo.toml -p yssbi --jobs 1
```

Expected: PASS. If authority or command tests fail after the planner passes, make only the proven generic boundary/exhaustive-match fix; preserve the Phase 1 commit path and the single existing command emit call.

- [ ] **Step 7: Review checkpoint (no commit)**

Verify project acceptance lives only in the dedicated file, the real command emits once on success and zero on failure/stale, no frontend-created IDs entered the DTO, inverse restores original identity/order, and do not commit.

---

### Task 3: Reproduce prerequisites in scratch, pass the controller conflict gate, then RED-test compiler transparency

**Conflict isolation:** Compiler scratch must reproduce the current uncommitted Phase 1 plus completed Phase 2 Tasks 1–2 state before compiler tests are added. It must not start from bare HEAD plus compiler tests. The prerequisite snapshot comes only from the active Phase 2 worktree; the main working tree compiler/dataframe patch is captured separately for three-way review and is never applied or copied into scratch. No step may modify the main working tree, the active Phase 2 worktree content, or either index.

**Files:**
- Create after controller approval: `src-tauri/src/node_system/compiler/reroute.rs`
- Create after controller approval: `src-tauri/src/node_system/compiler/reroute_tests.rs`
- Modify after controller approval: `src-tauri/src/node_system/compiler/mod.rs` with only `mod reroute;` and `#[cfg(test)] mod reroute_tests;`
- Modify after controller approval: `src-tauri/src/node_system/compiler/pipeline.rs` with only transparent behavior mapping/import and one normalization call
- Modify only for the transparent enum export: `src-tauri/src/node_system/registry/mod.rs`
- Never modify: `compiler/tests.rs`, `compiler/diagnostics.rs`, `compiler/dynamic_interface.rs`, `compiler/project.rs`, `compiler/schema_analysis.rs`, `compiler/tests_dynamic.rs`, `compiler/tests_dynamic_pipeline.rs`, `compiler/dataframe.rs`, `compiler/relational.rs`, or any `catalog/dataframe/**` file

**Interfaces:**

```rust
enum RegistryNodeBehavior<'a> {
    Leaf(&'a NodeImplementation),
    ProtocolOnly,
    Structural(StructuralNodeRole),
    Transparent(TransparentNodeRole),
}

pub(crate) fn collapse_reroute_semantics<R: CompilerRegistry>(
    registry: &R,
    graph: CompilerSemanticGraph,
) -> CompilerSemanticGraph
```

- Analysis remains based on the original persisted `GraphDocument`; only validated semantic input to lowering is collapsed.
- Evidence and scratch live in detached sibling paths `../phase2-reroute-compiler-evidence/` and `../phase2-reroute-compiler-scratch/`, not inside either existing worktree.

- [ ] **Step 1 (BLOCKING): Record the common base and capture the main working tree compiler patch separately**

Run read-only Git commands from the active Phase 2 worktree root:

```sh
mkdir -p ../phase2-reroute-compiler-evidence
git rev-parse HEAD > ../phase2-reroute-compiler-evidence/phase2-base.txt
git -C ../.. rev-parse HEAD > ../phase2-reroute-compiler-evidence/main-base.txt
git ls-files --stage > ../phase2-reroute-compiler-evidence/active-index-before.tsv
git -C ../.. ls-files --stage > ../phase2-reroute-compiler-evidence/main-index-before.tsv
git status --short > ../phase2-reroute-compiler-evidence/active-status-before.txt
git -C ../.. status --short > ../phase2-reroute-compiler-evidence/main-status-before.txt
git -C ../.. status --short -- src-tauri/src/node_system/compiler src-tauri/src/node_system/catalog/dataframe > ../phase2-reroute-compiler-evidence/main-compiler-status.txt
git -C ../.. diff --name-only HEAD -- src-tauri/src/node_system/compiler src-tauri/src/node_system/catalog/dataframe > ../phase2-reroute-compiler-evidence/main-compiler-tracked-paths.txt
git -C ../.. diff --binary HEAD -- src-tauri/src/node_system/compiler src-tauri/src/node_system/catalog/dataframe > ../phase2-reroute-compiler-evidence/main-compiler-tracked.patch
git -C ../.. hash-object --stdin-paths < ../phase2-reroute-compiler-evidence/main-compiler-tracked-paths.txt > ../phase2-reroute-compiler-evidence/main-compiler-hashes-only.txt
paste ../phase2-reroute-compiler-evidence/main-compiler-tracked-paths.txt ../phase2-reroute-compiler-evidence/main-compiler-hashes-only.txt > ../phase2-reroute-compiler-evidence/main-compiler-path-hashes.tsv
git -C ../.. diff --no-index /dev/null src-tauri/src/node_system/compiler/dataframe.rs > ../phase2-reroute-compiler-evidence/main-compiler-untracked-dataframe.patch || true
git -C ../.. hash-object src-tauri/src/node_system/compiler/dataframe.rs > ../phase2-reroute-compiler-evidence/main-compiler-untracked-dataframe.hash
git hash-object ../phase2-reroute-compiler-evidence/main-compiler-tracked.patch ../phase2-reroute-compiler-evidence/main-compiler-untracked-dataframe.patch > ../phase2-reroute-compiler-evidence/main-compiler-evidence-hashes.txt
```

Block unless `phase2-base.txt` and `main-base.txt` identify the intended common base or the controller explicitly records the different-base merge base. Inspect `main-compiler-status.txt`; every additional untracked compiler/dataframe file must receive its own `diff --no-index /dev/null <path>` patch and hash. These files are evidence for later three-way review only and must not appear in prerequisite manifests or scratch.

- [ ] **Step 2 (BLOCKING): Inventory and classify every active-worktree tracked/untracked change**

Capture the active Phase 2 worktree without changing its index:

```sh
git status --short > ../phase2-reroute-compiler-evidence/active-status.txt
git diff --name-status HEAD > ../phase2-reroute-compiler-evidence/active-all-tracked-status.tsv
git ls-files --others --exclude-standard > ../phase2-reroute-compiler-evidence/active-all-untracked-paths.txt
```

The controller creates `active-classification.tsv` with one row for every path from both inventories and exactly one class: `prerequisite-tracked`, `prerequisite-deleted`, `prerequisite-untracked`, `unrelated-user-work`, or `forbidden-compiler-dataframe`. Classification rules are locked:

- include every uncommitted Phase 1 implementation/test/fixture and every completed Phase 2 Task 1–2 implementation/test/module declaration;
- exclude this plan, sidebar/drag user work, and any other unrelated path;
- classify every `src-tauri/src/node_system/compiler/**` and `src-tauri/src/node_system/catalog/dataframe/**` path as `forbidden-compiler-dataframe` even if it unexpectedly appears in the active worktree;
- no path may be missing, duplicated, or assigned two classes.

From that review, write newline-delimited explicit manifests:

```text
../phase2-reroute-compiler-evidence/prerequisite-tracked-paths.txt
../phase2-reroute-compiler-evidence/prerequisite-deleted-paths.txt
../phase2-reroute-compiler-evidence/prerequisite-untracked-paths.txt
../phase2-reroute-compiler-evidence/excluded-unrelated-paths.txt
../phase2-reroute-compiler-evidence/forbidden-compiler-dataframe-paths.txt
```

Block if any inventory path is unclassified, if a prerequisite manifest contains a compiler/dataframe path, or if a Phase 1/Task 1–2 review checkpoint path is absent from the prerequisite manifests.

- [ ] **Step 3 (BLOCKING): Export a content-addressed prerequisite snapshot from the active worktree**

Export tracked modifications/deletions as one binary patch and untracked prerequisites as a tar archive. These commands read the active worktree only; they do not use `--cached`, `git add`, or any index-writing operation:

```sh
git diff --binary HEAD -- $(cat ../phase2-reroute-compiler-evidence/prerequisite-tracked-paths.txt) $(cat ../phase2-reroute-compiler-evidence/prerequisite-deleted-paths.txt) > ../phase2-reroute-compiler-evidence/prerequisite-tracked.patch
git hash-object --stdin-paths < ../phase2-reroute-compiler-evidence/prerequisite-tracked-paths.txt > ../phase2-reroute-compiler-evidence/prerequisite-tracked-hashes-only.txt
paste ../phase2-reroute-compiler-evidence/prerequisite-tracked-paths.txt ../phase2-reroute-compiler-evidence/prerequisite-tracked-hashes-only.txt > ../phase2-reroute-compiler-evidence/prerequisite-tracked-path-hashes.tsv
tar -cf ../phase2-reroute-compiler-evidence/prerequisite-untracked.tar -T ../phase2-reroute-compiler-evidence/prerequisite-untracked-paths.txt
git hash-object --stdin-paths < ../phase2-reroute-compiler-evidence/prerequisite-untracked-paths.txt > ../phase2-reroute-compiler-evidence/prerequisite-untracked-hashes-only.txt
paste ../phase2-reroute-compiler-evidence/prerequisite-untracked-paths.txt ../phase2-reroute-compiler-evidence/prerequisite-untracked-hashes-only.txt > ../phase2-reroute-compiler-evidence/prerequisite-untracked-path-hashes.tsv
git hash-object ../phase2-reroute-compiler-evidence/prerequisite-tracked.patch ../phase2-reroute-compiler-evidence/prerequisite-untracked.tar ../phase2-reroute-compiler-evidence/active-classification.tsv > ../phase2-reroute-compiler-evidence/prerequisite-snapshot-hashes.txt
```

Record deleted paths explicitly as `D<TAB>path` in `prerequisite-deleted-paths.tsv`. Block if either path/hash TSV has a different row count from its path manifest, if an expected prerequisite file is absent, or if snapshot artifacts are empty contrary to the completed prerequisite state.

- [ ] **Step 4 (BLOCKING): Create detached scratch at the exact base and replay only the prerequisite snapshot**

The controller creates the scratch from the recorded base; no temporary commit is allowed:

```sh
git worktree add --detach ../phase2-reroute-compiler-scratch $(cat ../phase2-reroute-compiler-evidence/phase2-base.txt)
git -C ../phase2-reroute-compiler-scratch apply --check --whitespace=error-all ../phase2-reroute-compiler-evidence/prerequisite-tracked.patch
git -C ../phase2-reroute-compiler-scratch apply --whitespace=nowarn ../phase2-reroute-compiler-evidence/prerequisite-tracked.patch
tar -xf ../phase2-reroute-compiler-evidence/prerequisite-untracked.tar -C ../phase2-reroute-compiler-scratch
```

`git apply` runs without `--index`/`--cached`; it modifies only scratch working-tree files. Do not use commit, stash, reset, restore, clean, checkout-overwrite, or copy any file from the main working tree.

- [ ] **Step 5 (BLOCKING): Verify scratch bytes, manifests, and compiler exclusion before tests**

From the active worktree root, recompute scratch hashes using the same manifests:

```sh
git -C ../phase2-reroute-compiler-scratch hash-object --stdin-paths < ../phase2-reroute-compiler-evidence/prerequisite-tracked-paths.txt > ../phase2-reroute-compiler-evidence/scratch-tracked-hashes-only.txt
paste ../phase2-reroute-compiler-evidence/prerequisite-tracked-paths.txt ../phase2-reroute-compiler-evidence/scratch-tracked-hashes-only.txt > ../phase2-reroute-compiler-evidence/scratch-tracked-path-hashes.tsv
git -C ../phase2-reroute-compiler-scratch hash-object --stdin-paths < ../phase2-reroute-compiler-evidence/prerequisite-untracked-paths.txt > ../phase2-reroute-compiler-evidence/scratch-untracked-hashes-only.txt
paste ../phase2-reroute-compiler-evidence/prerequisite-untracked-paths.txt ../phase2-reroute-compiler-evidence/scratch-untracked-hashes-only.txt > ../phase2-reroute-compiler-evidence/scratch-untracked-path-hashes.tsv
cmp ../phase2-reroute-compiler-evidence/prerequisite-tracked-path-hashes.tsv ../phase2-reroute-compiler-evidence/scratch-tracked-path-hashes.tsv
cmp ../phase2-reroute-compiler-evidence/prerequisite-untracked-path-hashes.tsv ../phase2-reroute-compiler-evidence/scratch-untracked-path-hashes.tsv
git -C ../phase2-reroute-compiler-scratch diff --exit-code HEAD -- src-tauri/src/node_system/compiler src-tauri/src/node_system/catalog/dataframe
git -C ../phase2-reroute-compiler-scratch ls-files --others --exclude-standard -- src-tauri/src/node_system/compiler src-tauri/src/node_system/catalog/dataframe > ../phase2-reroute-compiler-evidence/scratch-forbidden-untracked.txt
git ls-files --stage > ../phase2-reroute-compiler-evidence/active-index-after-replay.tsv
git -C ../.. ls-files --stage > ../phase2-reroute-compiler-evidence/main-index-after-replay.tsv
git status --short > ../phase2-reroute-compiler-evidence/active-status-after-replay.txt
git -C ../.. status --short > ../phase2-reroute-compiler-evidence/main-status-after-replay.txt
cmp ../phase2-reroute-compiler-evidence/active-index-before.tsv ../phase2-reroute-compiler-evidence/active-index-after-replay.tsv
cmp ../phase2-reroute-compiler-evidence/main-index-before.tsv ../phase2-reroute-compiler-evidence/main-index-after-replay.tsv
cmp ../phase2-reroute-compiler-evidence/active-status-before.txt ../phase2-reroute-compiler-evidence/active-status-after-replay.txt
cmp ../phase2-reroute-compiler-evidence/main-status-before.txt ../phase2-reroute-compiler-evidence/main-status-after-replay.txt
```

For every entry in `prerequisite-deleted-paths.txt`, assert the scratch path does not exist. Block on any `cmp` difference, present deleted path, non-empty `scratch-forbidden-untracked.txt`, compiler/dataframe diff from HEAD, unexpected path in scratch status, or active/main HEAD/index/status mutation caused by the snapshot process.

- [ ] **Step 6 (BLOCKING): Prove reproduced Phase 1 and Tasks 1–2 prerequisites pass in scratch**

Run from the scratch worktree root. If dependencies are absent, `pnpm install --frozen-lockfile` may populate only scratch ignored `node_modules`; it must not modify lockfiles.

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase1_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_transparent_registry_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_protocol_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_insert_reroute_document_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_authority_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_command_event_ --jobs 1 -- --test-threads=1
pnpm exec vitest run src/shared/types/dto/editorMutationWireParser.test.ts src/services/nodeSystem/graphMutationService.test.ts src/shared/utils/pinCompatibility.test.ts src/features/core/history/editorCommands.test.ts src/features/core/graphInteraction/graphInteractionStore.test.ts src/features/core/canvas/connectionInteraction.test.ts src/features/core/canvas/canvasPointerLoop.test.ts src/features/application/editor/useEditorOperations.capabilities.test.tsx src/features/application/editorMutation/editorMutationCoordinator.test.ts src/views/EditorView/Canvas/core/ConnectionLine.test.tsx src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx src/views/EditorView/Pins/Pin.preview.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
pnpm typecheck
```

Block on any failure. These results prove scratch contains the actual prerequisite state; passing bare-HEAD tests is insufficient.

- [ ] **Step 7 (RED): Only now add compiler tests in scratch and confirm failure before plumbing**

Add `reroute_tests.rs` and only the test-module declaration to scratch `compiler/mod.rs`. Cover normalization, compile transparency, chain/fan-out, control/effect direction, cycle visibility, no runtime identity, and invalid over-capacity `compiler.connection.limit` preservation:

```rust
assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
    diagnostic.code.as_str() == "compiler.connection.limit" && diagnostic.blocking
}));
assert!(result.plan.is_none());
```

Run from scratch root:

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_normalization_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_compile_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_connection_limit_ --jobs 1 -- --test-threads=1
```

Normalization/compile tests must fail for missing transparent plumbing/normalization. Record each result; do not accept fixture, prerequisite, or unrelated compiler failures as RED evidence.

- [ ] **Step 8 (SCRATCH GREEN): Prepare the minimal proposed compiler implementation and export its isolated diff**

In scratch only, add `reroute.rs`, `mod reroute;`, transparent behavior mapping/import, and one `collapse_reroute_semantics(...)` call after `analysis.validated(...)`. From the scratch root, run:

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_normalization_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_compile_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_connection_limit_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi dependency --jobs 1 -- --test-threads=1
pnpm exec cargo check --manifest-path src-tauri/Cargo.toml -p yssbi --jobs 1
```

Expected: PASS. Return to the active Phase 2 worktree root only to run these read-only export commands:

```sh
git -C ../phase2-reroute-compiler-scratch diff --binary HEAD -- src-tauri/src/node_system/compiler/mod.rs src-tauri/src/node_system/compiler/pipeline.rs src-tauri/src/node_system/compiler/reroute.rs src-tauri/src/node_system/compiler/reroute_tests.rs > ../phase2-reroute-compiler-evidence/phase2-proposed-compiler.patch
git hash-object ../phase2-reroute-compiler-evidence/phase2-proposed-compiler.patch > ../phase2-reroute-compiler-evidence/phase2-proposed-compiler.hash
```

The proposed patch must not contain prerequisite files, main-worktree compiler/dataframe changes, or any other compiler file.

- [ ] **Step 9 (BLOCKING CONTROLLER REVIEW): Perform the three-way hunk coordination**

For `compiler/mod.rs` and `compiler/pipeline.rs`, compare base `HEAD:<path>`, the separately captured main user side, and `phase2-proposed-compiler.patch`. Record `non-overlapping-approved`, `merged-hunk-approved`, or `blocked` per target in `../phase2-reroute-compiler-evidence/controller-decision.md`. Inspect exact symbols/imports/hunks, not filenames alone.

If main hashes drift, recapture the main evidence and repeat review. If active prerequisite path/hash manifests drift, discard scratch and repeat Steps 2–8. The executor cannot choose a side or proceed without controller approval.

- [ ] **Step 10 (REAL WORKTREE RED→GREEN): Apply only the approved compiler test hunk, prove RED, then apply approved production hunks**

In the active Phase 2 worktree, first apply/create only `reroute_tests.rs` plus its test-module declaration and rerun the three serial RED commands. Then apply only controller-approved `reroute.rs`, `mod.rs`, and `pipeline.rs` production hunks. Never import the main user compiler/dataframe patch into this worktree as part of Phase 2.

- [ ] **Step 11: Run focused compiler checks in the active Phase 2 worktree (GREEN)**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_normalization_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_compile_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_connection_limit_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi dependency --jobs 1 -- --test-threads=1
pnpm exec cargo check --manifest-path src-tauri/Cargo.toml -p yssbi --jobs 1
```

Expected: PASS; invalid over-capacity documents retain `compiler.connection.limit`, and valid plans contain no reroute runtime identity.

- [ ] **Step 12: Revalidate all evidence and remove scratch without touching existing worktrees (no commit)**

Recompute main compiler hashes, active prerequisite hashes, and both existing indexes. Any drift invalidates approval:

```sh
git -C ../.. hash-object --stdin-paths < ../phase2-reroute-compiler-evidence/main-compiler-tracked-paths.txt > ../phase2-reroute-compiler-evidence/main-compiler-hashes-final.txt
cmp ../phase2-reroute-compiler-evidence/main-compiler-hashes-only.txt ../phase2-reroute-compiler-evidence/main-compiler-hashes-final.txt
git -C ../.. hash-object src-tauri/src/node_system/compiler/dataframe.rs > ../phase2-reroute-compiler-evidence/main-compiler-untracked-dataframe-final.hash
cmp ../phase2-reroute-compiler-evidence/main-compiler-untracked-dataframe.hash ../phase2-reroute-compiler-evidence/main-compiler-untracked-dataframe-final.hash
git hash-object --stdin-paths < ../phase2-reroute-compiler-evidence/prerequisite-tracked-paths.txt > ../phase2-reroute-compiler-evidence/active-prerequisite-tracked-hashes-final.txt
cmp ../phase2-reroute-compiler-evidence/prerequisite-tracked-hashes-only.txt ../phase2-reroute-compiler-evidence/active-prerequisite-tracked-hashes-final.txt
git hash-object --stdin-paths < ../phase2-reroute-compiler-evidence/prerequisite-untracked-paths.txt > ../phase2-reroute-compiler-evidence/active-prerequisite-untracked-hashes-final.txt
cmp ../phase2-reroute-compiler-evidence/prerequisite-untracked-hashes-only.txt ../phase2-reroute-compiler-evidence/active-prerequisite-untracked-hashes-final.txt
git ls-files --stage > ../phase2-reroute-compiler-evidence/active-index-final.tsv
git -C ../.. ls-files --stage > ../phase2-reroute-compiler-evidence/main-index-final.tsv
cmp ../phase2-reroute-compiler-evidence/active-index-before.tsv ../phase2-reroute-compiler-evidence/active-index-final.tsv
cmp ../phase2-reroute-compiler-evidence/main-index-before.tsv ../phase2-reroute-compiler-evidence/main-index-final.tsv
```

Confirm Phase 2 compiler changes remain limited to the two new files and approved minimal wiring. Then remove only the detached scratch:

```sh
git worktree remove --force ../phase2-reroute-compiler-scratch
```

`--force` is permitted only for this controller-created disposable scratch after its proposed patch and hashes are durably recorded; it must never target the main or active Phase 2 worktree. Do not delete evidence, remove either existing worktree, alter either index, use temporary commits, or run stash/reset/restore/clean. Do not commit.

---

### Task 4: Project reroute nodes for compact rendering without a second wire model

**Files:**
- Modify: `src-tauri/src/node_system/analysis/projection.rs` tests
- Modify only if a proven projection omission exists: `src-tauri/src/node_system/analysis/projection.rs:639-855`
- Modify: `src-tauri/src/node_system/catalog/tests.rs`
- Modify frontend golden fixtures only if generated contracts include a reroute fixture: `src/tests/fixtures/node-system-contracts/editor-projection.json`
- Modify: `src/tests/helpers/editorProjectionFixtures.ts`

**Interfaces:**
- Consumes Task 1 hidden protocols and `builtin.reroute` style.
- Keeps existing `EditorNodeProjectionDto`; no `isReroute` boolean or frontend-only bend DTO is added.
- A projected reroute is identified for rendering by Rust-authored `display.styleId === 'builtin.reroute'`; node type IDs and resource paths remain opaque outside focused rendering classification.

- [ ] **Step 1 (RED): Add projection contract tests for every kind**

Add `phase2_reroute_projection_` Rust tests that build a persisted reroute node of each protocol and assert:

```rust
assert_eq!(node.display.style_id.as_deref(), Some("builtin.reroute"));
assert!(node.parameter_editors.is_empty());
assert!(!node.capabilities.managed);
assert!(node.capabilities.can_delete);
assert_eq!(node.ports.len(), 2);
```

Assert stable position/node identity, one input/one output, exact `PortKindDto`, shared resolved data type for the data variant, ordinary connection projection IDs/order, and no omission caused by compiler semantic normalization.

- [ ] **Step 2: Run projection tests and confirm RED or immediate GREEN**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_projection_ --jobs 1 -- --test-threads=1
```

Expected: ideally PASS after Tasks 1 and 3 because projection reads persisted document plus analysis; otherwise FAIL identifies a narrow projection gap.

- [ ] **Step 3 (GREEN): Fix only the proven projection gap**

If ports are missing, preserve reroute nodes in `AnalysisSnapshot.resolved_interfaces` and keep `build_editor_graph_projection` keyed to the original document analysis. Do not project from normalized compiler semantic nodes. Do not add runtime fields, frontend-authored flags, or a special reroute collection.

- [ ] **Step 4: Run projection/catalog tests and Rust check (GREEN)**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_projection_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_reroute_protocol_ --jobs 1 -- --test-threads=1
pnpm exec cargo check --manifest-path src-tauri/Cargo.toml -p yssbi --jobs 1
```

Expected: PASS.

- [ ] **Step 5: Review checkpoint (no commit)**

Confirm projection still contains persisted reroute nodes/connections while execution plan does not, and do not commit.

---

### Task 5: Add frontend `InsertReroute` wire and command on the Phase 1 mutation route

**Files:**
- Modify: `src/shared/types/dto/editorMutation.ts`
- Modify: `src/shared/types/dto/editorMutationWireParser.ts`
- Modify: `src/shared/types/dto/editorMutationWireParser.test.ts`
- Modify: `src/services/nodeSystem/graphMutationService.test.ts`
- Modify: `src/features/core/history/types.ts`
- Create: `src/features/core/history/commands/insertReroute.ts`
- Modify: `src/features/core/history/commands/index.ts`
- Modify: `src/features/core/history/commands/registryTypes.ts`
- Modify: `src/features/core/history/editorCommands.test.ts`
- Create: `src/features/application/editor/edgeOperations.ts`
- Create: `src/features/application/editor/edgeOperations.test.ts`
- Modify: `src/features/application/editor/index.ts`

**Interfaces:**
- Produces exact TypeScript DTO:

```ts
| {
    type: 'insertReroute';
    payload: {
      connectionId: string;
      position: { x: number; y: number };
    };
  }
```

- Produces command:

```ts
export interface InsertRerouteArgs {
  connectionId: string;
  position: { x: number; y: number };
}

executeCommand(graphPath, 'InsertReroute', args): Promise<boolean>
```

- Produces the only application helpers used by later edge UI tasks:

```ts
export async function disconnectConnectionsById(
  graphPath: string,
  connectionIds: readonly string[],
): Promise<boolean>;

export async function insertRerouteAtConnection(
  graphPath: string,
  connectionId: string,
  position: Readonly<{ x: number; y: number }>,
): Promise<boolean>;
```

- `edgeOperations.ts` depends only on `executeCommand` from `src/features/core/history`; it does not import views, Zustand stores, services, or Tauri.
- `disconnectConnectionsById` rejects an empty array locally, deduplicates IDs in first-seen order, and calls `executeCommand(graphPath, 'DisconnectConnections', { connectionIds })` exactly once.
- `insertRerouteAtConnection` validates a non-empty connection ID and finite coordinates, then calls `executeCommand(graphPath, 'InsertReroute', { connectionId, position: { ...position } })` exactly once.
- Task 6 `useEditorOperations.ts` calls `disconnectConnectionsById`; Task 7 context-menu actions call the operation exposed by `useEditorOperations`; Task 8 `useCanvasInteraction.ts` calls `insertRerouteAtConnection`. No later task may invent another helper with either name.

- [ ] **Step 1 (RED): Add strict parser, service wire, and application-helper tests**

Assert exact keys, finite numeric coordinates, and rejection of missing/extra keys. Lock the service payload:

```ts
expect(mutateGraphDocument).toHaveBeenCalledWith({
  graphPath,
  baseRevision,
  mutation: {
    type: 'insertReroute',
    payload: { connectionId: 'edge-1', position: { x: 120, y: 80 } },
  },
});
```

- [ ] **Step 2 (RED): Add one-intent command test**

In `editorCommands.test.ts`, assert `InsertReroute` calls `executeGraphIntent` exactly once and performs no `DisconnectConnections`, `CreateNode`, graph-store write, or ID allocation.

In `edgeOperations.test.ts`, assert the exact two helper signatures/behaviors: empty disconnect and invalid reroute coordinates make zero command calls; valid disconnect deduplicates and sends one `DisconnectConnections`; valid insertion sends one `InsertReroute`; false command results propagate unchanged.

- [ ] **Step 3: Run focused Vitest and confirm RED**

```sh
pnpm exec vitest run src/shared/types/dto/editorMutationWireParser.test.ts src/services/nodeSystem/graphMutationService.test.ts src/features/core/history/editorCommands.test.ts src/features/application/editor/edgeOperations.test.ts --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
```

Expected: FAIL because the DTO, command, and application helpers are absent.

- [ ] **Step 4 (GREEN): Add the strict DTO and thin command handler**

The command implementation is only:

```ts
const outcome = await executeGraphIntent(graphPath, {
  type: 'insertReroute',
  payload: args,
});
return outcome.status === 'applied';
```

Register exact command name `InsertReroute`. Implement and export both `edgeOperations.ts` helpers exactly as specified above, with no store access and one command call. Do not read the current edge from `graphDataStore`; Rust resolves the authoritative connection/kind/order.

- [ ] **Step 5: Run focused tests and typecheck (GREEN)**

```sh
pnpm exec vitest run src/shared/types/dto/editorMutationWireParser.test.ts src/services/nodeSystem/graphMutationService.test.ts src/features/core/history/editorCommands.test.ts src/features/application/editor/edgeOperations.test.ts --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 6: Review checkpoint (no commit)**

Confirm there is no frontend disconnect/create sequence and no caller supplies a reroute node ID, port ID, or replacement connection IDs. Do not commit.

---

### Task 6: Introduce mutually exclusive node/edge selection and atomic edge deletion

**Files:**
- Modify: `src/features/core/layout/editorTabStore.ts:11-117,370-375,500-570`
- Modify: `src/features/core/layout/layoutTabQueries.ts:81-99`
- Modify: `src/features/core/layout/layoutStore.test.ts`
- Modify: `src/features/core/layout/workbenchLayoutPersistence.test.ts`
- Modify: `src/features/application/editor/editorSessionTypes.ts`
- Modify: `src/features/core/editor/hooks/useActiveEditorGroup.ts`
- Modify: `src/features/core/editor/hooks/useActiveEditorGroup.test.tsx`
- Modify: `src/features/application/editor/useEditorOperations.ts`
- Modify: `src/features/application/editor/useEditorOperations.capabilities.test.tsx`
- Modify: `src/features/core/canvas/useCanvasInteraction.ts`
- Modify: Phase 1 interaction/selection tests in `src/features/core/canvas/connectionInteraction.test.ts` and `src/features/core/graphInteraction/graphInteractionStore.test.ts` only as needed for Escape precedence
- Modify: `src/features/application/editor/useEditorKeyboard.ts`
- Modify: `src/features/application/editor/useEditorKeyboard.test.tsx`

**Interfaces:**
- Adds serializable placement state:

```ts
selectedConnectionIds: string[];
```

- Produces public selection adapter/actions:

```ts
export interface GraphSelection {
  nodeIds: Set<string>;
  connectionIds: Set<string>;
}

getEditorGroupGraphSelection(groupId: string): GraphSelection;
updateEditorGroupSelectedNodeIds(...): void;       // clears connections
updateEditorGroupSelectedConnectionIds(...): void; // clears nodes
clearEditorGroupGraphSelection(groupId?: string | null): void;
```

- Consumes Task 5 `disconnectConnectionsById(graphPath, connectionIds): Promise<boolean>` from `src/features/application/editor/edgeOperations.ts`; `useEditorOperations.ts` is the sole selection-aware caller and owns post-success selection clearing.
- Node click uses `Ctrl`/`Meta` or `Shift` to toggle nodes; ordinary node click replaces node selection. Edge equivalents are wired in Task 7.
- `deleteSelected` chooses exactly one mutation kind: non-empty edge selection → one `disconnectConnectionsById` call; otherwise non-empty node selection → one `DeleteNodes`. Mixed selection cannot be represented.

- [ ] **Step 1 (RED): Add placement initialization/lifecycle/persistence tests**

Assert every empty/new/moved/merged/closed placement initializes or clears `selectedConnectionIds` beside `selectedNodeIds`. Memento round-trip preserves each array but never creates both non-empty. Legacy mementos missing `selectedConnectionIds` normalize to `[]` at the existing memento boundary; this is persisted layout normalization, not a graph mutation compatibility shim.

- [ ] **Step 2 (RED): Add pure mutual-exclusion action tests**

Test:

```ts
updateEditorGroupSelectedNodeIds(['node-a'], groupId);
expect(getEditorGroupGraphSelection(groupId)).toEqual({
  nodeIds: new Set(['node-a']),
  connectionIds: new Set(),
});

updateEditorGroupSelectedConnectionIds(['edge-a'], groupId);
expect(getEditorGroupGraphSelection(groupId)).toEqual({
  nodeIds: new Set(),
  connectionIds: new Set(['edge-a']),
});
```

Also assert duplicate IDs normalize once, Ctrl/Shift-style updater toggles are deterministic, box/node selection clears edge selection, and clear empties both.

- [ ] **Step 3 (RED): Add Delete and Escape precedence tests**

In `useEditorOperations.capabilities.test.tsx`, assert one selected edge sends exactly:

```ts
executeCommand(graphPath, 'DisconnectConnections', {
  connectionIds: ['edge-a', 'edge-b'],
});
```

Assert success clears edge selection after resolution, failure preserves it, and no `DeleteNodes` call occurs. Keep node deletion tests on `DeleteNodes`.

In `useEditorKeyboard.test.tsx`, extend Phase 1 precedence: active transient `CanvasInteraction` cancels first; otherwise Escape clears active edge/node selection before Zen Mode. Delete/Backspace delegates once to `deleteSelected`. Add no Phase 3 shortcut assertions.

- [ ] **Step 4: Run focused Vitest and confirm RED**

```sh
pnpm exec vitest run src/features/core/layout/layoutStore.test.ts src/features/core/layout/workbenchLayoutPersistence.test.ts src/features/application/editor/useEditorOperations.capabilities.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
```

Expected: FAIL because connection selection does not exist.

- [ ] **Step 5 (GREEN): Extend the existing per-group selection authority**

Keep one state authority in `editorTabStore`; do not duplicate selected edges in `useEditorStore` or `graphInteractionStore`. Every setter replaces both arrays atomically so mixed selection is impossible. Expose narrow selectors to avoid rerendering unrelated groups.

Preserve Phase 1 box-selection hit testing and Shift-box behavior byte-for-byte except that committing any node box selection clears edge selection.

- [ ] **Step 6 (GREEN): Route edge deletion through Phase 1 command**

Snapshot selected connection IDs from the active group, preserve deterministic projection order, call Task 5 `disconnectConnectionsById(tid, selectedConnectionIds)` exactly once, and clear only after `applied === true`. Do not call `executeCommand` directly here, inspect each connection, or submit a loop.

- [ ] **Step 7: Run selection/keyboard tests and typecheck (GREEN)**

```sh
pnpm exec vitest run src/features/core/layout/layoutStore.test.ts src/features/core/layout/workbenchLayoutPersistence.test.ts src/features/application/editor/useEditorOperations.capabilities.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx src/features/core/canvas/selectionHitTargets.test.ts --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 8: Review checkpoint (no commit)**

Confirm mixed selection is structurally impossible, failure preserves selection, no Phase 3 shortcut/box-union code was added, and do not commit.

---

### Task 7: Add wide edge hit paths, hover/select/context menu, and break interaction

**Files:**
- Modify: `src/views/EditorView/Canvas/core/Edge.tsx`
- Create: `src/views/EditorView/Canvas/core/Edge.interaction.test.tsx`
- Modify: `src/views/EditorView/Canvas/core/EdgesOverlay.tsx`
- Modify: `src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx`
- Modify: `src/views/EditorView/Canvas/core/Canvas.tsx`
- Create: `src/views/EditorView/ContextMenu/ConnectionContextMenu.tsx`
- Create: `src/views/EditorView/ContextMenu/ConnectionContextMenu.test.tsx`
- Modify: `src/views/EditorView/ContextMenu/index.ts`
- Modify: `src/features/application/editor/CanvasContextMenuContext.tsx`
- Modify: `src/features/core/canvas/useCanvasInteraction.ts`
- Modify: `src/features/application/editor/useEditorGroup.ts`
- Modify: `src/app/i18n/locales/en-US.ts`
- Modify: `src/app/i18n/locales/zh-CN.ts`

**Interfaces:**
- `Edge` receives:

```ts
selected?: boolean;
onPointerDown?: (event: React.PointerEvent<SVGPathElement>) => void;
onContextMenu?: (event: React.MouseEvent<SVGPathElement>) => void;
onDoubleClick?: (event: React.MouseEvent<SVGPathElement>) => void;
```

- `EdgesOverlay` receives current selected IDs and thin application callbacks; it owns only one local open context-menu descriptor.
- Transparent hit path uses the same `computeEdgePath` result as visible paths:

```tsx
<path
  data-edge-hit-target={edgeId}
  d={pathData}
  fill="none"
  stroke="transparent"
  strokeWidth={12}
  pointerEvents="stroke"
/>
```

- Context menu action delegates through `useEditorOperations` to the already-defined Task 5 helper `disconnectConnectionsById(graphPath, connectionIds)` once; `EdgesOverlay` and `ConnectionContextMenu` never import that helper directly.

- [ ] **Step 1 (RED): Add edge DOM hit-target and hover tests**

In `Edge.interaction.test.tsx`, assert one transparent hit path exists per edge, has `strokeWidth=12`, reuses the visible `d`, and alone accepts pointer events. Verify pointer enter/leave exposes hover styling, while visible/animation/glow paths remain `pointer-events-none`.

Assert selected styling is visually distinct but does not overwrite execution error/flow semantics; use data attributes/class assertions rather than exact theme colors.

- [ ] **Step 2 (RED): Add click selection behavior tests**

In `EdgesOverlay.test.tsx`, cover:

- ordinary click selects only that edge and clears nodes;
- `Ctrl`/`Meta` or `Shift` toggles within edge selection;
- clicking the canvas afterward clears edge selection through the existing canvas path;
- hit pointer down stops propagation and does not start box selection, pan, node drag, or connection gesture;
- box selection still targets nodes only;
- preview/non-interactive canvas renders visible edges but exposes no hit handlers.

- [ ] **Step 3 (RED): Add context menu/break tests**

Test right-click on an unselected edge selects it and opens one compact `ConnectionContextMenu`. Right-click on a member of a multi-edge selection retains that active edge set. The Break/Delete item calls one application callback with the selected IDs, closes only after invocation, and failure retains selection.

Use shared context-menu spacing/radius and add matching keys:

```ts
contextMenu.connection.breakLink
contextMenu.connection.breakSelectedLinks
contextMenu.connection.delete
```

- [ ] **Step 4: Run focused Vitest and confirm RED**

```sh
pnpm exec vitest run src/views/EditorView/Canvas/core/Edge.interaction.test.tsx src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx src/views/EditorView/ContextMenu/ConnectionContextMenu.test.tsx --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
```

Expected: FAIL because edges are pointer-transparent and no connection menu exists.

- [ ] **Step 5 (GREEN): Render one accessible hit path per edge**

Keep the outer SVG pointer-transparent for empty-space canvas behavior, and explicitly enable only each hit path with SVG `pointerEvents="stroke"`. Set `aria-label`/role only if the existing SVG accessibility tests support interactive graphics; otherwise expose stable `data-edge-hit-target` for testing and retain keyboard deletion through global selection.

`onPointerDown` must call `preventDefault()` and `stopPropagation()` before selection. Do not attach mutation calls to hover or pointer move.

- [ ] **Step 6 (GREEN): Wire selection and the shared compact context menu**

Keep mutation/application calls outside `Edge`. `EdgesOverlay` translates hit events into selection/context callbacks and renders at most one menu. `Canvas` supplies callbacks from `useEditorGroup`; the context action delegates once to Phase 1 `DisconnectConnections`.

- [ ] **Step 7: Run edge tests and typecheck (GREEN)**

```sh
pnpm exec vitest run src/views/EditorView/Canvas/core/Edge.interaction.test.tsx src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx src/views/EditorView/ContextMenu/ConnectionContextMenu.test.tsx src/features/application/editor/useEditorOperations.capabilities.test.tsx --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 8: Review checkpoint (no commit)**

Verify hit geometry has no second path algorithm, hover/click perform zero IPC, one menu action sends one collection mutation, non-interactive canvases stay inert, and do not commit.

---

### Task 8: Wire double-click atomic insertion and compact reroute rendering

**Files:**
- Modify: `src/views/EditorView/Canvas/core/EdgesOverlay.tsx`
- Modify: `src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx`
- Modify: `src/views/EditorView/Canvas/core/Canvas.tsx`
- Modify: `src/features/core/canvas/useCanvasInteraction.ts`
- Modify: `src/features/application/editor/useEditorGroup.ts`
- Modify: `src/features/application/editor/CanvasContextMenuContext.tsx`
- Create: `src/views/EditorView/Nodes/RerouteNodeLayout.tsx`
- Create: `src/views/EditorView/Nodes/RerouteNodeLayout.test.tsx`
- Modify: `src/views/EditorView/Nodes/Node.tsx`
- Modify: `src/views/EditorView/Nodes/NodeContainer.tsx`
- Modify: `src/features/core/dataStore/nodeView.ts`
- Modify: `src/features/core/dataStore/nodeView.test.ts`
- Modify: `src/features/domain/node/utils/nodeClassNames.ts`
- Modify or create focused test: `src/views/EditorView/Nodes/DefaultNodeLayout.test.tsx`

**Interfaces:**
- Edge double click calls:

```ts
insertRerouteAtConnection(
  graphPath: string,
  connectionId: string,
  position: Readonly<{ x: number; y: number }>,
): Promise<boolean>
```

- Position comes from existing canvas world conversion:

```ts
getCanvasLocalPoint(event.clientX, event.clientY)
```

- Compact rendering classification:

```ts
export const REROUTE_NODE_STYLE_ID = 'builtin.reroute';
export const REROUTE_NODE_WIDTH_PX = 32;
export const REROUTE_NODE_HEIGHT_PX = 20;
export const REROUTE_GRIP_SIZE_PX = 8;
export function uiNodeIsReroute(node: Pick<UINode, 'uiStyle'>): boolean;
```

- [ ] **Step 1 (RED): Add double-click command tests**

In `EdgesOverlay.test.tsx`, assert:

- double click stops propagation and sends exactly one `InsertReroute` command;
- payload includes only clicked `connectionId` and converted world position;
- no `DisconnectConnections`, `CreateNode`, `ConnectPins`, graph store topology setter, or predicted ID occurs;
- an applied result clears selection of the replaced original edge;
- failure/stale result preserves edge selection and topology;
- non-interactive canvas double click submits nothing.

- [ ] **Step 2 (RED): Add compact data/control/effect rendering tests**

In `RerouteNodeLayout.test.tsx`, build projected `UINode`s for all three kinds and assert:

- no title/header/category/parameter editor or inline literal input is rendered;
- exactly one input pin and one output pin are rendered on a compact horizontal body;
- the node root keeps `data-node-id`, persisted transform position, select ring, drag pointer handling, node context menu, and standard delete/move behavior;
- data/control/effect pin visuals come from projected pin kind/type, not node-type string switches;
- pin IDs/addresses are the Rust projection values.

In `nodeView.test.ts`, assert only `display.styleId === 'builtin.reroute'` selects compact layout.

- [ ] **Step 3: Run focused Vitest and confirm RED**

```sh
pnpm exec vitest run src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx src/views/EditorView/Nodes/RerouteNodeLayout.test.tsx src/features/core/dataStore/nodeView.test.ts --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
```

Expected: FAIL because double-click insertion and compact layout do not exist.

- [ ] **Step 4 (GREEN): Route double click to one authoritative command**

Pass `getCanvasLocalPoint` into `EdgesOverlay`. On hit-path double click, compute world position once and call the Task 5 helper `insertRerouteAtConnection(graphPath, connectionId, position)` through `useCanvasInteraction`; do not call `executeCommand` from the view. Clear only the replaced edge selection after `true`; the authoritative result/event creates and renders the reroute.

Do not select the newly inserted node by reading mutation delta; committed-delta post-operation selection belongs to Phase 3 duplicate/paste and is not needed for reroute acceptance.

- [ ] **Step 5 (GREEN): Add a focused compact layout without changing projection schema**

Branch `Node.tsx` by `uiNodeIsReroute(node)` before math/default layout. Update `NodeContainer`/size utilities to use exactly `width: 32px`, `height: 20px`, `minWidth: 32px`, and `minHeight: 20px` for `builtin.reroute`, with an exactly `8px × 8px` center grip. Lock these exported constants and computed styles in `RerouteNodeLayout.test.tsx`; do not use approximate or theme-dependent dimensions. Keep the standard container as the owner of persisted transform, selection, drag, execution diagnostics, and context menu.

`RerouteNodeLayout` renders only projected pins and a small center grip. It must not import services, mutation code, graph projection stores, or compiler identities.

- [ ] **Step 6: Run rendering/edge tests and typecheck (GREEN)**

```sh
pnpm exec vitest run src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx src/views/EditorView/Canvas/core/Edge.interaction.test.tsx src/views/EditorView/Nodes/RerouteNodeLayout.test.tsx src/features/core/dataStore/nodeView.test.ts src/views/EditorView/Nodes/DefaultNodeLayout.test.tsx --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 7: Manual interaction checkpoint (no commit)**

With an event/function graph loaded normally, verify data/control/effect edges can be double-clicked at zoomed/panned positions, one reroute appears at the pointer in the correct world coordinate, remains selectable/draggable/deletable, saves/reloads, and one Undo restores the original edge. Do not test or add duplicate/copy/paste/cut behavior in this phase and do not commit.

---

### Task 9: Phase 2 cross-layer acceptance, serial verification, and scope/conflict audit

**Files:**
- Test-only corrections in files already listed above.
- Do not modify Phase 3, sidebar cleanup, dataframe, unrelated compiler/schema analysis, or user-owned files.

**Interfaces:**
- Acceptance is edge gesture/menu intent → one authoritative mutation → one patch/revision/history entry → projection installation, while compiler analysis preserves editor reroutes and lowering receives collapsed semantics.

- [ ] **Step 1: Run all focused Rust Phase 2 tests serially**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase2_ --jobs 1 -- --test-threads=1
```

Expected: PASS for transparent registry/catalog, insertion/inverse, history/concurrency, projection, normalization, and compile transparency.

- [ ] **Step 2: Run focused neighboring Rust checks serially**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase1_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi projection --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi dependency --jobs 1 -- --test-threads=1
pnpm exec cargo check --manifest-path src-tauri/Cargo.toml -p yssbi --jobs 1
```

Expected: PASS; Phase 1 atomic semantics and ordinary compiler dependencies remain intact.

- [ ] **Step 3: Run the complete relevant frontend matrix serially**

```sh
pnpm exec vitest run src/shared/types/dto/editorMutationWireParser.test.ts src/services/nodeSystem/graphMutationService.test.ts src/features/core/history/editorCommands.test.ts src/features/application/editor/edgeOperations.test.ts src/features/core/editor/hooks/useActiveEditorGroup.test.tsx src/features/core/layout/layoutStore.test.ts src/features/core/layout/workbenchLayoutPersistence.test.ts src/features/application/editor/useEditorOperations.capabilities.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx src/features/core/canvas/selectionHitTargets.test.ts src/views/EditorView/Canvas/core/ConnectionLine.test.tsx src/views/EditorView/Canvas/core/Edge.interaction.test.tsx src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx src/views/EditorView/ContextMenu/ConnectionContextMenu.test.tsx src/views/EditorView/Nodes/RerouteNodeLayout.test.tsx src/features/core/dataStore/nodeView.test.ts --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 4: Run whitespace, status, and exact scope checks**

```sh
git diff --check
git status --short
git diff --name-only
git diff --name-only -- src-tauri/src/node_system/compiler src-tauri/src/node_system/catalog/dataframe
git -C ../.. status --short -- src-tauri/src/node_system/compiler src-tauri/src/node_system/catalog/dataframe
git hash-object ../phase2-reroute-compiler-evidence/main-compiler-tracked.patch ../phase2-reroute-compiler-evidence/main-compiler-untracked-dataframe.patch ../phase2-reroute-compiler-evidence/phase2-proposed-compiler.patch
```

Expected: no whitespace errors; no dataframe path in the Phase 2 diff; compiler changes limited to `compiler/mod.rs`, `compiler/pipeline.rs`, `compiler/reroute.rs`, and `compiler/reroute_tests.rs`; current evidence hashes match the controller-approved record; the main working tree user patch remains present and unmodified; unrelated sidebar/drag work remains preserved.

- [ ] **Step 5: Audit forbidden compatibility paths and Phase 3 scope**

```sh
git grep -n -E "type: 'disconnect'|EditorGraphMutationDto::Disconnect|reroute.*kernel|KernelHandle.*reroute" -- src src-tauri/src/node_system
git grep -n -E "DuplicateSubgraph|InsertSubgraph|ClipboardSubgraph|Ctrl\+A|Ctrl\+D|fit.*graph|focus.*selection" -- src src-tauri/src/node_system
git grep -n -E "for .*DisconnectConnections|for .*executeGraphIntent" -- src/features
```

Expected: no production singular disconnect/fake reroute kernel/mutation loop introduced. Existing pre-Phase-3 disabled labels or spec/test text may match the second audit; no new Phase 3 implementation may appear in this diff.

- [ ] **Step 6: Manual Phase 2 acceptance checklist**

1. Thin visible edges have a forgiving transparent hit area without blocking empty-canvas selection/pan.
2. Hover, ordinary selection, Ctrl/Meta/Shift edge toggling, node/edge mutual exclusion, and node-only box selection behave as specified.
3. Edge context Break/Delete and Delete/Backspace each submit one `DisconnectConnections`; one Undo restores all selected edges.
4. Mutation failure leaves edge selection and topology unchanged.
5. Double-clicking one data, control, or effect edge submits one `InsertReroute`; no intermediate topology flicker occurs.
6. Ordered input ordering remains on the downstream reroute edge and one Undo restores the original connection ID/order.
7. Reroute nodes are compact, persist through save/reload, and retain normal select/move/delete/history behavior without parameter UI.
8. Data generic type/schema facts, control direction, effect direction, multi-reroute chains, and cycles compile equivalently to direct edges.
9. An invalid loaded graph with a `Single` port over capacity still blocks compilation with `compiler.connection.limit`; reroute normalization does not hide it.
10. Runtime plans contain no reroute operation, kernel, or stable operation identity.
11. Existing Phase 1 connect replacement, Ctrl-drag, Alt-disconnect, snapping, feedback, and Escape behavior remain intact.
12. No duplicate/copy/paste/cut/Phase 3 keyboard or viewport behavior was added.

- [ ] **Step 7: Final review checkpoint (explicitly no commit)**

Record fresh command output, compiler conflict status, and any pre-existing unrelated failures for handoff. Do not run `git commit`, do not create a branch, and do not modify this plan to include Phase 3 follow-up implementation.

---

## Dependency and Risk Summary

1. **Hard Phase 1 dependency.** This plan assumes the old singular disconnect route is gone and `DisconnectConnections`, explicit `CanvasInteraction`, capability DTOs, safe errors, and Escape ordering are installed. If Phase 1 names differ, reconcile Phase 1 to its own locked interfaces rather than adding Phase 2 compatibility shims.
2. **Compiler conflict gate is mandatory and controller-owned.** The main working tree already has uncommitted changes in both Phase 2 wiring targets and eight additional compiler/dataframe files plus untracked `compiler/dataframe.rs`. Task 3 captures tracked/untracked patches and hashes, proves Phase 2 RED/creates the proposed diff in a disposable worktree, and requires a recorded three-way hunk decision. Clean status in this worktree is never sufficient. Hash drift reopens the gate; overlap remains blocked until the controller supplies a merged patch or rebased base. No executor action may overwrite or discard user work.
3. **Registry fingerprint changes are intentional.** Adding transparent behavior and three built-ins changes the canonical registry fingerprint. Tests should assert determinism and behavior identity, not pin unrelated old fingerprint bytes.
4. **Projection and compiler use different views by design.** Editor projection must use persisted document/analysis so reroutes remain visible; lowering uses normalized semantics so reroutes disappear from runtime operations. Accidentally normalizing the document snapshot would break projection/save/history and is forbidden.
5. **Selection storage is per editor group.** The existing placement store is the one authority. Adding serializable `selectedConnectionIds` avoids duplicate Zustand stores while the public adapter supplies the spec's `Set` interface. This phase must not redesign tab persistence or Phase 3 post-operation selection.
6. **SVG pointer-event layering is browser-sensitive.** Keep the overlay empty area pointer-transparent and enable only transparent hit strokes. Tests must prove hit paths stop propagation while canvas background behavior remains available.
7. **Ordered identity must be tested at mutation and history layers.** The downstream new edge carries the old `OrderKey`, but Undo restores the complete original `DocumentConnection` and ID; compiler normalization using downstream identity must not be confused with document identity restoration.
8. **No runtime no-op shortcut.** Registering reroutes as ordinary leaf nodes with a no-op kernel would violate acceptance even if values appear correct. Transparent registry behavior plus semantic collapse is required.
