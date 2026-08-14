# Blueprint Graph Phase 1 Atomic Interactions Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Phase 1 atomic graph operations and core connection wiring: authoritative replacement/movement/deletion/disconnection, Rust-authored port capabilities, and one explicit frontend interaction flow with Blueprint-style Ctrl-drag, Alt-disconnect, snapping, feedback, and Escape cancellation.

**Architecture:** `ProjectState.project_data` remains authoritative. React sends one domain intent through `GraphMutationService`; the existing thin `mutate_graph_document` command parses it, `EditorGraphMutationDto` plans one deterministic `GraphDocumentPatch`, `ProjectState` commits one revision/history transaction, and the project event/result installs the authoritative projection. Pointer movement and feedback remain local and animation-frame throttled; only a valid release/menu action invokes Rust.

**Tech Stack:** Rust, serde, Tauri 2, React 19, TypeScript 5.8, Zustand 5, Vitest 4, pnpm 11.

## Global Constraints

- Scope is strictly `docs/superpowers/specs/2026-08-11-blueprint-style-graph-interaction-design.md` Delivery Phase 1 (`§4`–`§10`, `§15`, `§16 Phase 1`, and only the Phase 1 cases from `§17`).
- Do not implement Phase 2 edge hit paths, edge selection, edge context menus, edge deletion UI, reroute nodes, or compiler reroute transparency.
- Do not implement Phase 3 subgraph export/duplicate/copy/paste/cut, `Ctrl+A`, `F`, `Home`, `Ctrl+D`, Shift-box union changes, or other keyboard/viewport work.
- Do not modify compiler or dataframe implementation/test files. In particular, leave `src-tauri/src/node_system/compiler/**`, `src-tauri/src/node_system/catalog/dataframe/**`, and current user work in `src/features/application/editor/handleGraphResourceDrop.test.ts`, `src/views/EditorView/Layout/sidebar/**`, and `docs/superpowers/plans/2026-08-11-sidebar-resource-node-drag.md` untouched. `src-tauri/src/project/production_tests.rs` has exactly two permitted contract migrations: replace its existing singular `EditorGraphMutationDto::Disconnect` fixture with a one-element `DisconnectConnections`, and update the pre-existing projection-failure test to the new precommit atomicity contract (zero observer/authority/history/publication effects); preserve every dataframe-related user change byte-for-byte.
- `ProjectState.project_data` is authoritative; the frontend must not author graph patches, predict IDs, optimistically mutate committed topology, or infer backend-loaded state from `graphEntities` alone.
- Keep `src-tauri/src/commands/command_node_system.rs` thin: parse/validate IPC, call `ProjectState`, map errors/results, and emit the existing project graph event only.
- Preserve the existing mutation route: frontend intent → `GraphMutationService` → `mutate_graph_document` → `ProjectState::apply_editor_graph_mutation` → one patch/history entry/revision → projection result/event installation.
- Collection mutations reject empty and duplicate direct targets, validate all direct targets before commit, deduplicate derived connections, sort operations deterministically, and produce exactly one patch/history transaction.
- Remove `DeleteNode` and `Disconnect` directly; this 0.x project must not retain compatibility variants or frontend fallback loops.
- A failed mutation, invalid drop, or Escape cancellation preserves topology, graph revision, history stacks, projection, selection, and emitted-event count. A stale revision is hydrated but never automatically replayed.
- A duplicate endpoint `Connect` is rejected before incumbent discovery/removal and before allocating a `ConnectionId`; it produces no patch, delta, revision/history/projection change, or event.
- `Connect` and `MoveConnections` must receive an `EditorMutationValidationSnapshot` built from the Rust projection at the request base revision. Domain planning passes the exact `(source_expr, target_expr, source_protocol.type_parameters, target_protocol.type_parameters)` to the existing `type_exprs_assignable` validator through `node_system::compatibility`; it must not trust frontend types or defer type validation to compiler execution.
- Data ports reject missing projection type data with `graph_connection_type_unavailable`; reject `Unknown` or undeclared generic expressions with `graph_connection_type_unresolved`; declared symbolic generics remain valid even when `TypeSummaryDto.resolved` is false. Control/effect ports bypass data assignability only after exact kind equality.
- Global `window`/`document` listeners must use `src/shared/utils/globalEvent.ts`.
- Pointer movement, hover compatibility, snapping, and replacement highlighting perform no IPC and remain animation-frame throttled.
- Use shared toast/i18n for ordinary messages; never show raw backend connection messages or UUID-bearing details in ordinary UI.
- Run all commands from the worktree root. Every Rust compile/test command in this plan includes `--jobs 1`; every Rust test command also includes `-- --test-threads=1`.
- Every Vitest command includes `--pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1`.
- Do not create commits. Ignore any generic skill instruction to commit; each task ends at a review/verification checkpoint only.

---

## Audited Baseline and Locked File Map

### Existing implementation to preserve

- `src-tauri/src/node_system/document/mutation.rs` owns `EditorGraphMutationDto::into_patch_with_compatibility`, connection validation/planning, node deletion planning, and `RevisionedGraphStore`.
- Real type inputs are `TypeExpr::{Concrete, Generic, Applied, Union, Unknown}` from `node_system/protocol/types.rs`; each owning `NodeProtocol.interface.type_parameters` is `Box<[TypeParameterId]>`.
- `TypeSummaryDto` in `node_system/analysis/projection.rs` exposes `resolved_type` per port and carries `pub(crate) internal_type_expr: Option<TypeExpr>` with `#[serde(skip)]`; therefore the mutation snapshot is Rust-only and cannot be authored by IPC clients.
- `src-tauri/src/project/project_state.rs:4695-5090` serializes authoritative commit under publication/project/history locks, applies one `ProjectHistoryTransaction::graph`, advances one graph revision, and snapshots the committed projection.
- `src-tauri/src/commands/command_node_system.rs:469-535` already provides the correct thin command and one `GraphDelta` emission path.
- `src/features/application/editorMutation/editorMutationCoordinator.ts` creates one revisioned request, installs the result through `applyMutationResult`, hydrates on stale revision, and does not replay.
- `src/features/core/canvas/canvasPointerLoop.ts` already throttles pointer movement with `requestAnimationFrame` and uses `addGlobalEventListener`.
- `src/features/core/dataStore/graphDataStore.ts` installs Rust projection port metadata directly into `PinData.connections`.

### Existing gaps this plan closes

- Rust DTOs are singular `DeleteNode { node_id }` and `Disconnect { connection_id }`; `Connect` rejects occupied `Single` ports before planning replacement; `MoveConnections`, `DisconnectPort`, and `DisconnectNode` do not exist.
- `delete_editor_node_operations` validates and deletes only one node; there is no shared deterministic disconnection planner.
- Frontend `DeleteNodes` loops `deleteNode` intents, `DisconnectPin` loops connection IDs from the projection, and `breakAllNodeLinks` loops pins—three pseudo-batches that violate one-action/one-transaction semantics.
- Projection `PortConnectionCapabilityDto` has `{ current, maximum, ordered, canConnect }`, and `source_from_projection` rejects occupied replaceable `Single` sources.
- Connection state is split among `useGestureStore`, module-global selection session state, `useEditorStore.pendingConnection`, and preview bindings; Ctrl-drag, snapping, replacement feedback, and graph-operation Escape precedence are absent.
- `MutationConflict::InvalidEditorMutation` currently maps to an internal/raw error. Phase 1 replaces expected graph-edit failures with a typed `EditorMutationError`; command mapping serializes stable safe codes, while detailed addresses/UUIDs remain logger-only.

### Files expected to change

- Rust domain/projection/tests: `src-tauri/src/node_system/document/mutation.rs`, `src-tauri/src/node_system/document/tests.rs`, `src-tauri/src/node_system/document/tests/editor_mutation_validation.rs`, `src-tauri/src/node_system/analysis/projection.rs`, `src-tauri/src/node_system/compatibility.rs`.
- Rust authority tests: create `src-tauri/src/project/blueprint_graph_phase1_tests.rs` and wire it from `src-tauri/src/project/mod.rs`; do not add Blueprint coverage to `src-tauri/src/project/production_tests.rs`, whose only permitted change is the existing singular-Disconnect fixture migration required for compilation.
- Rust command/event tests: create `src-tauri/src/commands/command_blueprint_graph_phase1_tests.rs`, wire it from `src-tauri/src/commands/mod.rs`, and expose only the existing emitter seam/error mapper as `pub(super)` from `command_node_system.rs`.
- Frontend wire/projection: `src/shared/types/dto/editorMutation.ts`, `src/shared/types/dto/editorProjection.ts`, `src/shared/types/dto/editorProjectionGuards.ts`, `src/shared/types/dto/editorMutationWireParser.ts`, their focused tests, golden fixtures, and `src/tests/helpers/editorProjectionFixtures.ts`.
- Frontend commands/application: `src/features/core/history/commands/{connectPins,deleteNodes,disconnectPin,index,registryTypes}.ts`, `src/features/core/history/editorCommands.test.ts`, `src/features/application/editor/useEditorOperations.ts`, and focused hook tests.
- Frontend interaction: `src/features/core/graphInteraction/graphInteractionStore.ts`, `src/features/core/canvas/{connectionInteraction,connectPreview,canvasPointerLoop,useCanvasInteraction}.ts`, `src/features/application/editor/useEditorKeyboard.ts`, `src/views/EditorView/Canvas/core/{ConnectionLine,EdgesOverlay}.tsx`, `src/views/EditorView/Pins/Pin.tsx`, and focused tests.
- Frontend copy: `src/app/i18n/locales/en-US.ts`, `src/app/i18n/locales/zh-CN.ts`.

---

### Task 1: Lock the backend conflict protocol and consolidate collection mutations

**Files:**
- Modify: `src-tauri/src/node_system/document/mutation.rs:75-125,270-530,725-745,1490-1534`
- Modify: `src-tauri/src/node_system/document/mod.rs:34-45`
- Modify: `src-tauri/src/commands/command_node_system.rs:34-60`
- Modify: `src-tauri/src/error.rs:1-55`
- Modify: `src-tauri/src/node_system/document/tests.rs:320-425`
- Modify: `src-tauri/src/node_system/document/tests/editor_mutation_validation.rs:620-700`

**Interfaces:**
- Produces this exact domain error protocol for expected Phase 1 rejections:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum EditorMutationErrorCode {
    GraphPortNotFound,
    GraphNodeNotFound,
    GraphConnectionNotFound,
    GraphPortOrphan,
    GraphConnectionDirectionMismatch,
    GraphConnectionKindMismatch,
    GraphConnectionTypeMismatch,
    GraphConnectionTypeUnavailable,
    GraphConnectionTypeUnresolved,
    GraphConnectionLimitReached,
    GraphConnectionOrderRequired,
    GraphConnectionOrderForbidden,
    GraphConnectionAlreadyExists,
    GraphConnectionMoveSourceEmpty,
    GraphConnectionMoveSamePort,
    GraphMutationEmptyTargets,
    GraphMutationDuplicateTarget,
    GraphManagedNodeDeleteForbidden,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorMutationError {
    pub code: EditorMutationErrorCode,
    pub detail: Box<str>,
}
```

`EditorMutationErrorCode::as_str()` returns these exact wire codes in enum order:

```text
graph_port_not_found
graph_node_not_found
graph_connection_not_found
graph_port_orphan
graph_connection_direction_mismatch
graph_connection_kind_mismatch
graph_connection_type_mismatch
graph_connection_type_unavailable
graph_connection_type_unresolved
graph_connection_limit_reached
graph_connection_order_required
graph_connection_order_forbidden
graph_connection_already_exists
graph_connection_move_source_empty
graph_connection_move_same_port
graph_mutation_empty_targets
graph_mutation_duplicate_target
graph_managed_node_delete_forbidden
```

`MutationConflict` gains `Editor(EditorMutationError)`; stale remains `StaleRevision` and maps to existing `graph_revision_conflict`. `src-tauri/src/error.rs` adds the exact serializable details DTO:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphMutationErrorDetailsDto {
    pub category: &'static str,
}

impl GraphMutationErrorDetailsDto {
    pub const VALUE: Self = Self { category: "graphMutation" };
}
```

`mutation_conflict_to_app_error` sets `AppError.details = Some(serde_json::to_value(GraphMutationErrorDetailsDto::VALUE).unwrap())`. The IPC rejection shape is exact and contains no address/UUID:

```json
{
  "code": "graph_connection_type_mismatch",
  "message": "Graph mutation rejected",
  "details": { "category": "graphMutation" }
}
```

For stale revision the shape is:

```json
{
  "code": "graph_revision_conflict",
  "message": "Graph revision changed",
  "details": { "category": "graphMutation" }
}
```

`mutation_conflict_to_app_error` logs `EditorMutationError.detail` with the project’s existing `tauri_plugin_log::log::warn!`, but serializes only the safe constant message/details above. Unexpected `Document`, `Projection`, and `History` errors remain `internal_error`.

- Produces these exact serde mutation variants and removes `DeleteNode`/`Disconnect`:

```rust
DeleteNodes { node_ids: Vec<NodeId> },
DisconnectConnections { connection_ids: Vec<ConnectionId> },
DisconnectPort { address: PortAddressDto },
DisconnectNode { node_id: NodeId },
```

- Produces one private helper shared by all disconnection variants:

```rust
fn disconnect_connection_operations(
    document: &GraphDocument,
    connection_ids: impl IntoIterator<Item = ConnectionId>,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict>
```

- [ ] **Step 1 (RED): Add stable error serialization and command-mapping tests**

In `mutation.rs`/`error.rs` tests and the existing inline tests in `command_node_system.rs`, add `phase1_error_protocol_` table cases for every `EditorMutationErrorCode` plus `StaleRevision`. Assert the exact top-level `code`, safe `message`, exact `details`, and absence of UUID/address substrings. Assert unexpected errors still serialize as `internal_error`.

- [ ] **Step 2: Run error protocol tests and confirm RED**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase1_error_protocol_ --jobs 1 -- --test-threads=1
```

Expected: FAIL because `EditorMutationErrorCode`, `EditorMutationError`, stable details, and mappings do not exist.

- [ ] **Step 3 (GREEN): Implement the exact domain/IPC error protocol**

Add the types and mappings exactly as declared above. Convert expected node/port/connection lookup, orphan, direction, kind, type, capacity, order, duplicate target, empty target, managed deletion, empty move source, and same-port move failures to `MutationConflict::Editor`. Keep detailed text only in `EditorMutationError.detail` for logs.

- [ ] **Step 4 (RED): Replace singular wire tests and add collection planner tests**

Add tests prefixed `phase1_collection_` that assert camelCase payloads, length-one arrays, and rejection of old singular wire variants. Add planner cases for deterministic group deletion, empty/duplicate/missing direct targets, mixed ordinary/managed deletion, and connection/port/node disconnection. Assert exact error codes from Step 3.

For successful deletion, assert operations are grouped and sorted exactly as: `RemoveConnection` by `ConnectionId`, `SetInputState` by `PortAddress`, `RemovePortBinding` by `PortAddress`, then `RemoveNode` by `NodeId`. Apply the patch and inverse to prove exact restoration.

- [ ] **Step 5: Run collection tests and confirm RED**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase1_collection_ --jobs 1 -- --test-threads=1
```

Expected: FAIL because the collection variants/helper and deterministic multi-node planner do not exist.

- [ ] **Step 6 (GREEN): Implement deterministic collection planners**

Validate direct collections before deriving any operation: reject empty arrays, reject duplicate direct IDs with `graph_mutation_duplicate_target`, resolve every direct target, and reject managed nodes. Build selected IDs with `BTreeSet`, scan each document collection once, deduplicate derived shared connections, and assemble one patch in the exact order asserted by Step 4. Never call mutation/history APIs from a planner.

- [ ] **Step 7: Run focused tests and Rust check (GREEN)**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase1_error_protocol_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase1_collection_ --jobs 1 -- --test-threads=1
pnpm exec cargo check --manifest-path src-tauri/Cargo.toml -p yssbi --jobs 1
```

Expected: PASS. All Rust call sites/tests use the new variants; TypeScript wire migration remains for Task 6.

- [ ] **Step 8: Review checkpoint (no commit)**

Confirm no expected Phase 1 rejection falls through to `InvalidEditorMutation`/`internal_error`, and `git grep` finds no Rust singular variants. Do not commit.

---

### Task 2: Make `Connect` replace occupied `Single` endpoints atomically

**Files:**
- Modify: `src-tauri/src/node_system/compatibility.rs:20-75,430-470`
- Modify: `src-tauri/src/node_system/document/mutation.rs:317-355,1030-1125`
- Modify: `src-tauri/src/project/project_state.rs:4695-4995`
- Modify: `src-tauri/src/node_system/document/tests/editor_mutation_validation.rs:730-910`

**Interfaces:**
- Consumes: unchanged `EditorGraphMutationDto::Connect { output, input, order }` plus a mandatory same-revision validation snapshot:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorMutationValidationSnapshot {
    pub graph_revision: GraphRevision,
    ports: BTreeMap<PortAddress, EditorMutationPortValidation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EditorMutationPortValidation {
    pub direction: PortDirection,
    pub kind: PortKind,
    pub orphan: bool,
    pub port_type: EditorMutationPortType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EditorMutationPortType {
    NotApplicable,
    Ready {
        expression: TypeExpr,
        type_parameters: Box<[TypeParameterId]>,
    },
    MissingResolvedType,
    MissingInternalTypeExpr,
    Unresolved {
        expression: TypeExpr,
        type_parameters: Box<[TypeParameterId]>,
    },
}

impl EditorMutationValidationSnapshot {
    pub(crate) fn from_projection(
        projection: &EditorGraphProjectionDto,
        registry: &NodeRegistry,
    ) -> Result<Self, String>;

    pub(crate) fn validate_connection_types(
        &self,
        output: &PortAddress,
        input: &PortAddress,
    ) -> Result<(), EditorMutationError>;
}
```

Snapshot construction is locked to these real sources:

1. `graph_revision` comes from `projection.basis.graph_revision` via `GraphRevision::new`.
2. Address/direction/kind/orphan come from each Rust `ResolvedPortDto` in `projection.nodes[*].ports`.
3. For each projection node, parse `EditorNodeProjectionDto.node_type_id` as `NodeTypeId`, resolve `registry.protocol(&node_type_id)`, and clone `protocol.interface.type_parameters` into every data port owned by that node. Type parameter IDs never come from the frontend wire.
4. For `PortKind::Control`/`Effect`, store `NotApplicable` regardless of `resolved_type`.
5. For `PortKind::Data`, `resolved_type == None` stores `MissingResolvedType`; `resolved_type.internal_type_expr == None` stores `MissingInternalTypeExpr`.
6. Otherwise recursively inspect the real `TypeExpr::{Concrete, Generic, Applied, Union, Unknown}`. `Unknown` or any `Generic(id)` absent from the owning `type_parameters` stores `Unresolved`; declared `Generic`, including nested `Applied`/`Union`, stores `Ready`. `TypeSummaryDto.resolved == false` alone is not an error because a declared symbolic generic is valid planner input.
7. A projection node type missing from `NodeRegistry`, an invalid projected `NodeTypeId`, or a duplicate projected `PortAddress` makes `from_projection` return `Err` and is mapped to existing unexpected `MutationConflict::Projection`; authority/history/event state remains unchanged.

`validate_connection_types` loads both snapshot ports, returns `graph_port_not_found` if either is absent, `graph_port_orphan` if either is orphan, `graph_connection_direction_mismatch` unless output/input directions are exact, and `graph_connection_kind_mismatch` unless kinds match. It then evaluates port types:

```rust
match (&output_port.port_type, &input_port.port_type) {
    (EditorMutationPortType::NotApplicable, EditorMutationPortType::NotApplicable)
        if output_port.kind == input_port.kind => Ok(()),
    (EditorMutationPortType::MissingResolvedType
        | EditorMutationPortType::MissingInternalTypeExpr, _)
    | (_, EditorMutationPortType::MissingResolvedType
        | EditorMutationPortType::MissingInternalTypeExpr) => {
        Err(editor_error(
            GraphConnectionTypeUnavailable,
            "connection endpoint projection has no authoritative type expression",
        ))
    }
    (EditorMutationPortType::Unresolved { .. }, _)
    | (_, EditorMutationPortType::Unresolved { .. }) => {
        Err(editor_error(
            GraphConnectionTypeUnresolved,
            "connection endpoint type expression is unresolved",
        ))
    }
    (
        EditorMutationPortType::Ready {
            expression: source,
            type_parameters: source_type_parameters,
        },
        EditorMutationPortType::Ready {
            expression: target,
            type_parameters: target_type_parameters,
        },
    ) if crate::node_system::compiler::type_exprs_assignable(
        source,
        target,
        source_type_parameters,
        target_type_parameters,
    ) => Ok(()),
    (EditorMutationPortType::Ready { .. }, EditorMutationPortType::Ready { .. }) => {
        Err(editor_error(
            GraphConnectionTypeMismatch,
            "connection endpoint types are not assignable",
        ))
    }
    _ => Err(editor_error(
        GraphConnectionKindMismatch,
        "connection endpoint kinds do not match",
    )),
}
```

The compiler function is called only from `node_system::compatibility`; no compiler file is modified. `ProjectState::apply_editor_graph_mutation` builds the snapshot from the Rust projection and the same captured `NodeRegistry`, verifies `snapshot.graph_revision == request.base_revision`, and passes it to `into_patch_with_compatibility`; `Connect`/`MoveConnections` reject a missing or mismatched snapshot as an internal invariant failure.

- Produces a private planner that returns incumbent removals followed by one insertion and a testable allocation seam:

```rust
fn connect_operations(
    document: &GraphDocument,
    registry: &NodeRegistry,
    validation: &EditorMutationValidationSnapshot,
    output: PortAddress,
    input: PortAddress,
    order: Option<OrderKey>,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict>;

fn connect_operations_with_id_allocator(
    document: &GraphDocument,
    registry: &NodeRegistry,
    validation: &EditorMutationValidationSnapshot,
    output: PortAddress,
    input: PortAddress,
    order: Option<OrderKey>,
    allocate: impl FnOnce() -> ConnectionId,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict>;
```

The production wrapper passes `ConnectionId::new`; tests pass a counting closure. The closure is invoked only after every validation and incumbent staging step succeeds.

- No new IPC payload and no frontend-provided incumbent IDs.

- [ ] **Step 1 (RED): Add snapshot assignability and replacement matrix tests**

In `editor_mutation_validation.rs`, add tests prefixed `phase1_type_snapshot_` using real `TypeExpr`, `TypeParameterId`, `TypeConstructorId`, `TypeSummaryDto`, projection nodes/ports, and `NodeRegistry` protocols:

```rust
// Concrete(core.float64) -> Concrete(core.float64): Ok.
// Concrete(core.float64) -> Concrete(core.string): graph_connection_type_mismatch.
// Generic(item), source params [item] -> Generic(item), target params [item]: Ok;
// the two local declarations occupy separate solver indices.
// Generic(source_item), source params [source_item] -> Generic(target_item),
// target params [target_item]: Ok; names need not match across protocols.
// Applied(core.data_series, [Generic(item)]) with source params [item]
// -> Applied(core.data_series, [Concrete(core.float64)]) with target params []: Ok.
// Different Applied constructors: graph_connection_type_mismatch.
// Generic(missing) absent from owning protocol params: graph_connection_type_unresolved.
// Applied/Union containing Unknown or undeclared Generic: graph_connection_type_unresolved.
// TypeExpr::Unknown: graph_connection_type_unresolved.
// resolved_type None: graph_connection_type_unavailable.
// TypeSummaryDto present with internal_type_expr None: graph_connection_type_unavailable.
// Declared Generic with TypeSummaryDto.resolved == false: Ready and assignable.
// Control -> Control and Effect -> Effect: Ok without resolved_type/internal expression.
// Control -> Effect or either non-data kind -> Data: graph_connection_kind_mismatch.
```

Assert the captured `Ready` values contain the exact source and target `Box<[TypeParameterId]>` from their separate owning protocols; do not synthesize a merged parameter list.

Add tests prefixed `phase1_connect_` for:

- occupied single input replacement;
- occupied single output replacement;
- independently occupied single input and output (two removals, one insertion);
- duplicate endpoint request where the exact output/input pair already exists: return `graph_connection_already_exists` before incumbent collection, with no patch, removal, or new ID;
- the spec’s “shared incumbent” case is the exact duplicate pair and therefore follows duplicate precedence: reject it before replacement collection rather than remove/reinsert it;
- full bounded `Multiple { max: Some(n) }` rejection without eviction;
- ordered target order-key preservation/validation;
- missing endpoint → `graph_port_not_found`;
- orphan → `graph_port_orphan`;
- direction → `graph_connection_direction_mismatch`;
- kind → `graph_connection_kind_mismatch`;
- authoritative type incompatibility → `graph_connection_type_mismatch`;
- missing `resolved_type` or missing `internal_type_expr` → `graph_connection_type_unavailable`;
- `Unknown` or a `Generic` not declared by its owning protocol (including nested `Applied`/`Union`) → `graph_connection_type_unresolved`;
- full bounded multiple → `graph_connection_limit_reached`;
- absent/present forbidden order key → `graph_connection_order_required` / `graph_connection_order_forbidden`;
- duplicate endpoint → `graph_connection_already_exists` with zero patch/removal/allocation.

Every failure leaves a cloned document byte-for-byte equal with unchanged revision.

Use stable connection IDs in fixtures and assert removed `DocumentConnection` values preserve their IDs and `OrderKey`s for inverse restoration.

- [ ] **Step 2: Run focused test and confirm RED**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase1_type_snapshot_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase1_connect_ --jobs 1 -- --test-threads=1
```

Expected: FAIL because the snapshot/type-state model does not exist and occupied `Single` currently returns “connection limit”.

- [ ] **Step 3 (GREEN): Build the complete snapshot and stage replacement planning**

Implement `EditorMutationValidationSnapshot::from_projection` and `validate_connection_types` exactly as specified above, including separate source/target type parameter arrays and stable unavailable/unresolved errors. Then refactor capacity validation so it distinguishes:

```rust
enum EndpointCapacity {
    Append,
    Replace(Vec<DocumentConnection>), // only occupied non-orphan Single
}
```

Use this mandatory order before allocating a new `ConnectionId`:

1. Resolve both endpoints from the document and validation snapshot; reject missing/orphan/direction/kind errors.
2. Scan `document.connections.values()` only for an exact existing `(output, input)` pair. If found, immediately return `graph_connection_already_exists`. Do not call incumbent collection, construct a patch, remove anything, or allocate an ID.
3. Validate authoritative type assignability through `EditorMutationValidationSnapshot::validate_connection_types`.
4. Validate order policy and reject full bounded `Multiple` endpoints.
5. Only now collect incumbent IDs from occupied `Single` endpoints into a `BTreeMap<ConnectionId, DocumentConnection>`.
6. Clone/stage the document, apply deterministic `RemoveConnection`s, revalidate capacity against staged topology, then append one `InsertConnection`.
7. Return removals ordered by `ConnectionId`, then insertion. Do not mutate `document` during planning.

The duplicate-endpoint unit test passes a counting allocator closure to `connect_operations_with_id_allocator` and asserts count `0`, `Err(graph_connection_already_exists)`, no patch, and unchanged document/revision. Task 4 extends this to history/projection/event invariants.

- [ ] **Step 4: Run focused and neighboring mutation tests (GREEN)**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase1_type_snapshot_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase1_connect_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi editor_mutation_ --jobs 1 -- --test-threads=1
pnpm exec cargo check --manifest-path src-tauri/Cargo.toml -p yssbi --jobs 1
```

Expected: PASS; the compiler’s existing invalid-document diagnostics remain untouched.

- [ ] **Step 5: Review checkpoint (no commit)**

Inspect the planner order and verify duplicate endpoint rejection precedes incumbent discovery, no replacement decision/incumbent ID entered an IPC DTO, and type validation is mandatory domain planning rather than compiler execution. Do not commit.

---

### Task 3: Add all-or-nothing `MoveConnections`

**Files:**
- Modify: `src-tauri/src/node_system/document/mutation.rs:270-530,1030-1140`
- Modify: `src-tauri/src/node_system/document/tests.rs:320-425`
- Modify: `src-tauri/src/node_system/document/tests/editor_mutation_validation.rs`

**Interfaces:**
- Produces this exact intent DTO:

```rust
MoveConnections {
    source: PortAddressDto,
    target: PortAddressDto,
},
```

- Rust resolves source connection IDs from the authoritative `GraphDocument`; frontend sends none.
- Requires the same `EditorMutationValidationSnapshot` as `Connect` and validates every derived output/input pair with `validate_connection_types` before any ID allocation.
- Produces the production planner plus an allocator seam used by domain/authority tests:

```rust
fn move_connection_operations(
    document: &GraphDocument,
    registry: &NodeRegistry,
    snapshot: &EditorMutationValidationSnapshot,
    source: PortAddress,
    target: PortAddress,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict>;

fn move_connection_operations_with_id_allocator(
    document: &GraphDocument,
    registry: &NodeRegistry,
    snapshot: &EditorMutationValidationSnapshot,
    source: PortAddress,
    target: PortAddress,
    allocate: &dyn Fn() -> ConnectionId,
) -> Result<Vec<GraphDocumentOperation>, MutationConflict>;
```

The production wrapper passes `ConnectionId::new`. The seam allocates exactly once per moved edge only after every proposal, replacement, duplicate, capacity, order, and type check succeeds. Failure and stale paths allocate zero IDs.

- Reuses Task 2 endpoint replacement/capacity validation and Task 1 deterministic disconnection ordering.

- [ ] **Step 1 (RED): Add wire and movement tests**

Add tests prefixed `phase1_move_connections_` covering:

```rust
// Source output -> target output: preserve each original input and order key.
// Source input -> target input: preserve each original output; ordered target rules apply.
// Move every source connection, not a frontend snapshot subset.
// Replace one occupied Single target exactly once.
// Reject source == target -> graph_connection_move_same_port.
// Reject empty source -> graph_connection_move_source_empty.
// Reject missing/orphan/direction/kind/type/capacity/order using Task 1's exact codes.
// Reject any invalid member of a multi-edge move all-or-nothing.
// On every failure: no patch, allocation, revision/history/projection/event/topology change.
```

For success, assert original connection IDs are removed and newly allocated IDs are inserted; inverse restores all original IDs and order keys exactly.

- [ ] **Step 2: Run focused test and confirm RED**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase1_move_connections_ --jobs 1 -- --test-threads=1
```

Expected: FAIL because `MoveConnections` does not exist.

- [ ] **Step 3 (GREEN): Implement a staged multi-edge move planner**

Implement `move_connection_operations(document, registry, snapshot, source, target)` with the exact signature above and this sequence:

1. Resolve source/target ports from the document and snapshot; require matching direction/kind and non-orphan status.
2. Reject `source == target` with `graph_connection_move_same_port` before deriving removals.
3. Collect all source-incident connections in `ConnectionId` order and reject an empty source with `graph_connection_move_source_empty`.
4. Derive each proposed endpoint pair by replacing only the source-side address.
5. Validate every derived pair through `EditorMutationValidationSnapshot::validate_connection_types`.
6. Stage removal of all moved connections plus replaceable target incumbents, deduplicated by `ConnectionId`.
7. Validate aggregate bounded capacity and order rules against one staged document.
8. Allocate/append all new connections only after every proposal validates.
9. Return one patch ordered as all removals by old ID followed by insertions in old-source-ID order.

- [ ] **Step 4: Run focused tests and Rust check (GREEN)**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase1_move_connections_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase1_connect_ --jobs 1 -- --test-threads=1
pnpm exec cargo check --manifest-path src-tauri/Cargo.toml -p yssbi --jobs 1
```

Expected: PASS.

- [ ] **Step 5: Review checkpoint (no commit)**

Confirm no API accepts source connection IDs and every validation happens before authoritative mutation. Do not commit.

---

### Task 4: Prove the six-mutation authority, history, concurrency, command, and event matrix

**Files:**
- Create: `src-tauri/src/node_system/testing/blueprint_phase1.rs`
- Modify: `src-tauri/src/node_system/testing/mod.rs:1-30` with `#[cfg(test)] pub(crate) mod blueprint_phase1;`
- Create: `src-tauri/src/project/blueprint_graph_phase1_tests.rs`
- Modify: `src-tauri/src/project/mod.rs:188-197` with `#[cfg(test)] mod blueprint_graph_phase1_tests;`
- Create: `src-tauri/src/commands/command_blueprint_graph_phase1_tests.rs`
- Modify: `src-tauri/src/commands/mod.rs:1-35` with `#[cfg(test)] mod command_blueprint_graph_phase1_tests;`
- Modify: `src-tauri/src/commands/command_node_system.rs:34-60,490-535` only to make `mutation_conflict_to_app_error` and `mutate_graph_document_with_emitter` `pub(super)`; do not move workflow into tests.
- Modify only if a matrix test exposes an authority bug: `src-tauri/src/project/project_state.rs:4695-5090`
- Restricted: `src-tauri/src/project/production_tests.rs` — only migrate the existing singular `Disconnect` fixture to one-element `DisconnectConnections`; no other edits.

**Interfaces:**
- `blueprint_phase1.rs` produces a reusable fixture and table enum:

```rust
pub(crate) enum Phase1ComplexMutation {
    ConnectReplacement,
    MoveConnections,
    DeleteNodes,
    DisconnectConnections,
    DisconnectPort,
    DisconnectNode,
}

pub(crate) const PHASE1_COMPLEX_MUTATIONS: [Phase1ComplexMutation; 6];

pub(crate) struct BlueprintPhase1Fixture {
    pub state: ProjectState,
    pub project_instance_id: ProjectInstanceId,
    pub graph_path: GraphResourcePath,
}
```

It provides `success_request(kind)`, `validation_failure_request(kind)`, `stale_request(kind)`, `competing_requests(kind)`, `authority_snapshot()`, and `projection()`; each request uses a distinct `OperationId` and the fixture contains stable old connection IDs/order keys.

- The authority matrix must execute every one of the six rows against every one of these columns:

| Column | Required assertions for each mutation |
| --- | --- |
| Success | exactly one revision increment; exactly one undo entry; complete delta; one-step undo/redo; `MoveConnections` removes old IDs and inserts fresh IDs in old-source-ID order; inverse restores exact old IDs/order keys; returned projection equals committed authority |
| Validation failure | exact row-specific code from the table below; document/revision/history/projection/publication unchanged; no delta/result |
| Duplicate/stale | duplicate applies to `ConnectReplacement` and proves no allocation/removal; stale applies to all six and preserves every snapshot |
| Same-revision race | two threads start from the same base revision; exactly one succeeds; exactly one returns `graph_revision_conflict`; authority/history/projection contain only winner |

The fixture locks one validation failure per row:

| Mutation | `validation_failure_request` | Exact code |
| --- | --- | --- |
| `ConnectReplacement` | incompatible authoritative projected types | `graph_connection_type_mismatch` |
| `MoveConnections` | target full bounded multiple | `graph_connection_limit_reached` |
| `DeleteNodes` | selection contains a managed node | `graph_managed_node_delete_forbidden` |
| `DisconnectConnections` | explicit missing connection ID | `graph_connection_not_found` |
| `DisconnectPort` | missing structured port address | `graph_port_not_found` |
| `DisconnectNode` | missing node ID | `graph_node_not_found` |

- The command/event matrix invokes the real `mutate_graph_document_with_emitter`, not `apply_editor_graph_mutation_observed`: non-empty success pushes exactly one `Event::Project(EventProject::GraphDelta { .. })`; validation failure, stale, and stable empty derived `DisconnectPort`/`DisconnectNode` no-ops push zero events.
- Empty derived `DisconnectPort`/`DisconnectNode` results are stable no-ops: empty delta with `from_revision == to_revision`, unchanged history/publication/projection/document, and no event. Empty direct collections remain rejected.

- [ ] **Step 1 (RED): Create the shared six-row fixture and authority matrix tests**

Add the three new files and exact `mod` wiring above. In `blueprint_graph_phase1_tests.rs`, table-drive all six enum rows through success, validation failure, stale, and same-revision race. For each row snapshot serialized graph content, graph revision, undo/redo lengths, projection, and publication state before execution. Assert the complete column contract above, including exact delta operations and one-step undo/redo.

For duplicate endpoint, use the `ConnectReplacement` fixture’s already-connected requested pair and assert `graph_connection_already_exists`, unchanged allocation counter, zero delta, and unchanged revision/history/projection/publication.

- [ ] **Step 2 (RED): Add real command/event emission matrix tests**

In `command_blueprint_graph_phase1_tests.rs`, invoke `mutate_graph_document_with_emitter` for every `PHASE1_COMPLEX_MUTATIONS` row in three table passes:

```rust
(success, expected_event_count = 1)
(validation_failure, expected_event_count = 0)
(stale, expected_event_count = 0)
```

For success, destructure the sole event and assert its `project_instance_id`, `graph_path`, `from_revision`, `to_revision`, `caused_by`, and full patch equal the returned `GraphMutationResultDto.delta`. For failure/stale, assert the returned stable code and an empty event vector. Do not substitute the observed callback seam.

- [ ] **Step 3: Run both new matrices and confirm RED**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi blueprint_graph_phase1_tests --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi command_blueprint_graph_phase1_tests --jobs 1 -- --test-threads=1
```

Expected: FAIL until the fixture modules, six-row contracts, stable errors, and complete mutation implementations exist.

- [ ] **Step 4 (GREEN): Make only matrix-driven authority integration fixes**

Preserve the existing authoritative sequence: validate base revision under authority locks → plan from current `project_data` with the same-revision validation snapshot → apply one `ProjectHistoryTransaction::graph` → install one document revision → advance publication → build one projection snapshot. Do not add I/O under locks, a second event protocol, or command-side planning.

- [ ] **Step 5: Run authority, command/event, and Rust checks (GREEN)**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi blueprint_graph_phase1_tests --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi command_blueprint_graph_phase1_tests --jobs 1 -- --test-threads=1
pnpm exec cargo check --manifest-path src-tauri/Cargo.toml -p yssbi --jobs 1
```

Expected: PASS for all six rows and all matrix columns.

- [ ] **Step 6: Review checkpoint (no commit)**

Confirm `src-tauri/src/project/production_tests.rs` differs only by the approved singular-Disconnect fixture migration, commands remain thin, success emits exactly once, failure/stale emit zero times, and no test relies only on `apply_editor_graph_mutation_observed`. Do not commit.

---

### Task 5: Project append/replace/move capabilities and unblock create-and-connect replacement

**Files:**
- Modify: `src-tauri/src/node_system/analysis/projection.rs:300-320,620-685,1080-1110,1500-1550`
- Modify: `src-tauri/src/node_system/compatibility.rs:35-75`
- Modify: `src-tauri/src/node_system/analysis/projection.rs` tests
- Modify: `src-tauri/src/node_system/catalog/tests.rs` only if golden projection assertions require it; do not touch dataframe modules.

**Interfaces:**
- Replaces `can_connect` with:

```rust
pub struct PortConnectionCapabilityDto {
    pub current: u32,
    pub maximum: Option<u32>,
    pub ordered: bool,
    pub can_append: bool,
    pub can_replace: bool,
    pub can_move: bool,
}
```

- `can_append`: non-orphan and below maximum/unbounded.
- `can_replace`: non-orphan `Single` with `current == 1`.
- `can_move`: non-orphan with `current > 0`.
- `source_from_projection` accepts a source when `can_append || can_replace`, while mutation planning revalidates authority.

- [ ] **Step 1 (RED): Add projection truth-table tests**

Add tests prefixed `phase1_connection_capability_` for empty/occupied `Single`, available/full bounded `Multiple`, unbounded/ordered multiple, orphan ports, and connected control/effect ports. Assert the exact serialized keys:

```json
{
  "current": 1,
  "maximum": 1,
  "ordered": false,
  "canAppend": false,
  "canReplace": true,
  "canMove": true
}
```

Also test `source_from_projection` accepts an occupied replaceable `Single` for contextual catalog/create-and-connect and rejects full bounded `Multiple`/orphan sources.

- [ ] **Step 2: Run test and confirm RED**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase1_connection_capability_ --jobs 1 -- --test-threads=1
```

Expected: FAIL because only `can_connect` exists.

- [ ] **Step 3 (GREEN): Compute capabilities from authoritative document state**

Update `project_connection_capability` using `ConnectionsPerPort` and actual current count. Keep capability values advisory at the frontend boundary; do not weaken `Connect`, `MoveConnections`, or create-and-connect validation.

Update `source_from_projection` from:

```rust
if port.orphan
    || (!port.connections.can_append && !port.connections.can_replace)
{
    return Err("source port cannot append or replace a connection".into());
}
```

to the explicit append-or-replace rule. For create-and-connect, keep type authority in `compatibility::ports_are_compatible(source, candidate)`, which already calls the authoritative assignability validator. After that check succeeds, `append_atomic_connection` calls a private `connect_operations_prevalidated_type` that shares Task 2 replacement/capacity/order planning but cannot be called outside `mutation.rs`; ordinary `Connect` and `MoveConnections` must use `EditorMutationValidationSnapshot` and cannot select this prevalidated path.

- [ ] **Step 4: Run projection/compatibility tests and Rust check (GREEN)**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase1_connection_capability_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi projection --jobs 1 -- --test-threads=1
pnpm exec cargo check --manifest-path src-tauri/Cargo.toml -p yssbi --jobs 1
```

Expected: PASS.

- [ ] **Step 5: Review checkpoint (no commit)**

Verify a full bounded `Multiple` is never marked replaceable and no frontend capability is trusted as authority. Do not commit.

---

### Task 6: Migrate frontend DTOs and remove pseudo-batch command loops

**Files:**
- Modify: `src/shared/types/dto/editorMutation.ts:20-60`
- Modify: `src/shared/types/dto/editorProjection.ts:120-140`
- Modify: `src/shared/types/dto/editorProjectionGuards.ts:145-160`
- Modify: `src/shared/types/dto/editorMutationWireParser.ts:190-210`
- Modify tests: `src/shared/types/dto/editorMutationWireParser.test.ts`, `src/services/nodeSystem/graphMutationService.test.ts`, `src/services/nodeSystem/nodeSystemGoldenContracts.test.ts`
- Create: `src/features/application/editorMutation/graphMutationError.ts`
- Create: `src/features/application/editorMutation/graphMutationError.test.ts`
- Modify: `src/features/application/editorMutation/editorMutationCoordinator.ts`
- Modify: `src/features/application/editorMutation/editorMutationCoordinator.test.ts`
- Modify: `src/app/i18n/locales/en-US.ts`
- Modify: `src/app/i18n/locales/zh-CN.ts`
- Modify fixtures: `src/tests/fixtures/node-system-contracts/editor-projection.json`, `src/tests/fixtures/node-system-contracts/function-editor-projection.json`, `src/tests/helpers/editorProjectionFixtures.ts`
- Modify: `src/features/core/history/types.ts`
- Modify: `src/features/core/history/commands/deleteNodes.ts`
- Modify: `src/features/core/history/commands/disconnectPin.ts` (rename its exported command/args to `DisconnectPort`; keep the file path to avoid an unrelated rename)
- Create: `src/features/core/history/commands/disconnectNode.ts`
- Create: `src/features/core/history/commands/disconnectConnections.ts`
- Create: `src/features/core/history/commands/moveConnections.ts`
- Modify: `src/features/core/history/commands/connectPins.ts`, `index.ts`, `registryTypes.ts`
- Modify tests: `src/features/core/history/editorCommands.test.ts`
- Modify: `src/features/application/editor/useEditorOperations.ts`
- Modify tests: `src/features/application/editor/useEditorOperations.capabilities.test.tsx`, `src/features/application/dataManagement/useNodeManagement.test.tsx`

**Interfaces:**
- TypeScript DTO variants exactly mirror Rust:

```ts
| { type: 'deleteNodes'; payload: { nodeIds: string[] } }
| { type: 'disconnectConnections'; payload: { connectionIds: string[] } }
| { type: 'disconnectPort'; payload: { address: PortAddressDto } }
| { type: 'disconnectNode'; payload: { nodeId: string } }
| { type: 'moveConnections'; payload: { source: PortAddressDto; target: PortAddressDto } }
```

- Projection capability becomes `{ current, maximum, ordered, canAppend, canReplace, canMove }`.
- Produces this exact frontend error union, with no fallback to backend `message` for recognized codes:

```ts
export type GraphMutationErrorCode =
  | 'graph_port_not_found'
  | 'graph_node_not_found'
  | 'graph_connection_not_found'
  | 'graph_port_orphan'
  | 'graph_connection_direction_mismatch'
  | 'graph_connection_kind_mismatch'
  | 'graph_connection_type_mismatch'
  | 'graph_connection_type_unavailable'
  | 'graph_connection_type_unresolved'
  | 'graph_connection_limit_reached'
  | 'graph_connection_order_required'
  | 'graph_connection_order_forbidden'
  | 'graph_connection_already_exists'
  | 'graph_connection_move_source_empty'
  | 'graph_connection_move_same_port'
  | 'graph_mutation_empty_targets'
  | 'graph_mutation_duplicate_target'
  | 'graph_managed_node_delete_forbidden'
  | 'graph_revision_conflict';

export function graphMutationErrorMessageKey(
  error: unknown,
): `canvas.connection.errors.${GraphMutationErrorCode}` | null;
```

`ExecuteEditorMutationOutcome` gains `{ status: 'rejected'; code: Exclude<GraphMutationErrorCode, 'graph_revision_conflict'> }`; stale revision keeps `{ status: 'conflict' }` after authoritative hydrate and is never replayed. Command handlers send one `executeGraphIntent` call and never discover/loop authoritative connection IDs for break-links.

- [ ] **Step 1 (RED): Update strict wire/parser and safe-error tests first**

Change exact-key tests and fixtures to the six capability keys. Add DTO compile assertions for all five new mutation variants and remove singular variants. Add malformed-object tests for missing/extra/non-boolean capability fields.

In `graphMutationError.test.ts`, table-drive all 17 exact codes above. For every code, assert a non-null i18n key exists in both locale objects, the localized value is non-empty, and neither the backend `message` nor a UUID fixture appears in returned user copy. In `editorMutationCoordinator.test.ts`, assert recognized validation codes return `status: 'rejected'`, `graph_revision_conflict` hydrates once and returns `status: 'conflict'`, and neither path replays the mutation.

- [ ] **Step 2 (RED): Rewrite command tests around one-intent semantics**

In `editorCommands.test.ts`, assert:

```ts
expect(executeEditorMutation).toHaveBeenCalledTimes(1);
expect(executeEditorMutation).toHaveBeenCalledWith(expect.objectContaining({
  mutation: { type: 'deleteNodes', payload: { nodeIds: ['node-a', 'node-b'] } },
}));
```

Add equivalent one-call tests for `DisconnectPort`, `DisconnectNode`, `DisconnectConnections`, and `MoveConnections`. Delete the old “stops disconnect sequencing” test because no sequence remains. Assert empty direct arrays are rejected before service invocation.

- [ ] **Step 3: Run focused Vitest and confirm RED**

```sh
pnpm exec vitest run src/shared/types/dto/editorMutationWireParser.test.ts src/services/nodeSystem/graphMutationService.test.ts src/services/nodeSystem/nodeSystemGoldenContracts.test.ts src/features/application/editorMutation/graphMutationError.test.ts src/features/application/editorMutation/editorMutationCoordinator.test.ts src/features/core/history/editorCommands.test.ts --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
```

Expected: FAIL against old DTOs and loop behavior.

- [ ] **Step 4 (GREEN): Migrate types, strict parsers, fixtures, and handlers**

Implement handlers with structured projection addresses only:

```ts
return executeGraphIntent(graphPath, {
  type: 'disconnectPort',
  payload: { address: pin.address },
});
```

Implement `graphMutationErrorMessageKey` as an exhaustive `Record<GraphMutationErrorCode, key>` and add the corresponding `canvas.connection.errors.<code>` key to both locale files. The coordinator recognizes codes by the top-level serialized `AppError.code`; it never displays `AppError.message`.

Add exact registry command names `MoveConnections`, `DisconnectPort`, `DisconnectNode`, and `DisconnectConnections` to `CommandType`/`CommandHandlerMap`. `DeleteNodes` sends the whole filtered array once. `breakAllNodeLinks` sends `DisconnectNode` once. Alt-click uses `DisconnectPort` once. Keep `DisconnectConnections` wired in the registry/service contract for Phase 2 callers, but do not add edge selection or edge UI now. `ConnectPins` must allow a compatible endpoint when each side has `canAppend || canReplace`.

- [ ] **Step 5: Run focused tests and typecheck (GREEN)**

```sh
pnpm exec vitest run src/shared/types/dto/editorMutationWireParser.test.ts src/services/nodeSystem/graphMutationService.test.ts src/services/nodeSystem/nodeSystemGoldenContracts.test.ts src/features/application/editorMutation/graphMutationError.test.ts src/features/application/editorMutation/editorMutationCoordinator.test.ts src/features/core/history/editorCommands.test.ts src/features/application/editor/useEditorOperations.capabilities.test.tsx src/features/application/dataManagement/useNodeManagement.test.tsx --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 6: Review checkpoint (no commit)**

Run `git grep` for `type: 'deleteNode'`, `type: 'disconnect'`, loops around `executeGraphIntent`, and `connections.canConnect`; all Phase 1 production call sites must be gone. Do not commit.

---

### Task 7: Introduce one explicit connection interaction model and pure feedback/snapping

**Files:**
- Create: `src/features/core/canvas/connectionInteraction.ts`
- Create: `src/features/core/canvas/connectionInteraction.test.ts`
- Modify: `src/features/core/graphInteraction/graphInteractionStore.ts`
- Modify: `src/features/core/graphInteraction/graphInteractionStore.test.ts`
- Modify: `src/features/core/canvas/selectionSession.ts`
- Modify: `src/shared/types/ui/editor.ts:110-155`
- Modify: `src/features/core/gesture/useGestureStore.ts` (retain only compatibility-free behavior still owned there; move pan/connect/drag session ownership to `CanvasInteraction`)
- Modify: `src/features/core/editor/stores/useEditorStore.ts`
- Modify: `src/features/core/editor/hooks/useEditorUIState.ts`
- Modify: `src/features/core/canvas/connectPreview.ts`
- Modify tests: `src/shared/utils/pinCompatibility.test.ts`
- Modify: `src/shared/utils/pinCompatibility.ts`

**Interfaces:**
- Produces graph-scoped mutually exclusive state (retain a `panning` variant only to preserve the existing canvas behavior not discussed by the spec):

```ts
type CanvasInteraction =
  | { type: 'idle' }
  | { type: 'panning'; session: PanSession }
  | { type: 'selecting'; session: SelectionSession }
  | { type: 'draggingNodes'; session: NodeDragSession }
  | { type: 'drawingConnection'; session: ConnectionDrawSession }
  | { type: 'movingConnections'; session: ConnectionMoveSession }
  | { type: 'pendingNodeCreation'; session: PendingNodeCreationSession };
```

- Produces pure feedback:

```ts
type ConnectionCompatibility =
  | { kind: 'append' }
  | { kind: 'replace'; displacedConnectionIds: string[] }
  | { kind: 'invalid'; reason: ConnectionInvalidReason };

type ConnectionInvalidReason =
  | 'same-port' | 'same-node' | 'same-direction' | 'kind-mismatch'
  | 'type-mismatch' | 'orphan' | 'capacity' | 'missing-address';
```

- `ConnectionMoveSession` stores only source `Pin`, pointer/hover/snap/feedback; never source connection IDs.

- [ ] **Step 1 (RED): Add reducer/store exclusivity and graph isolation tests**

Test that starting one interaction replaces the previous state, interactions are isolated/cleared by `graphPath`, position overrides remain local preview state, and Escape-style `cancelInteraction(graphPath)` returns to `idle` without touching `graphDataStore`.

- [ ] **Step 2 (RED): Add a pure compatibility and snapping matrix**

In `connectionInteraction.test.ts`, construct projected `PinData` fixtures and assert:

- append when both endpoints can append;
- replace when an occupied `Single` endpoint has `canReplace`;
- displaced IDs are advisory and derived from installed `pinConnections` only;
- full bounded multiple, orphan, same node/direction, kind/type mismatch are structured invalid reasons;
- nearest valid candidate within `CONNECTION_SNAP_RADIUS_PX` wins with stable distance/ID tie-breaking;
- invalid nearby candidates never snap;
- control/effect pins obey Rust capabilities, not a data-only shortcut.

- [ ] **Step 3: Run focused Vitest and confirm RED**

```sh
pnpm exec vitest run src/features/core/canvas/connectionInteraction.test.ts src/features/core/graphInteraction/graphInteractionStore.test.ts src/shared/utils/pinCompatibility.test.ts --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
```

Expected: FAIL because the interaction model and capability fields do not exist.

- [ ] **Step 4 (GREEN): Implement pure interaction transitions and feedback**

Keep `connectionInteraction.ts` framework-free. It may consume installed projection entities and pin geometry snapshots, but must not import services, views, Zustand, or Tauri. Store only UI/session state in `graphInteractionStore`; topology remains in `graphDataStore`.

Use one exported constant with a test-locked value:

```ts
export const CONNECTION_SNAP_RADIUS_PX = 18;
```

Update `connectPreview` to derive from `CanvasInteraction` rather than subscribing to a second connection gesture state. Remove obsolete `isReconnect`/connection helpers from `EditorGesture`; do not maintain dual compatibility paths.

- [ ] **Step 5: Run tests and typecheck (GREEN)**

```sh
pnpm exec vitest run src/features/core/canvas/connectionInteraction.test.ts src/features/core/graphInteraction/graphInteractionStore.test.ts src/shared/utils/pinCompatibility.test.ts --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 6: Review checkpoint (no commit)**

Confirm pointer-preview code has no service/invoke imports and no projected connection IDs are sent by move intent. Do not commit.

---

### Task 8: Wire Ctrl-drag, Alt-disconnect, valid/replace/invalid visuals, Escape, and safe errors

**Files:**
- Modify: `src/features/core/canvas/useCanvasInteraction.ts`
- Modify: `src/features/core/canvas/canvasPointerLoop.ts`
- Modify: `src/features/core/canvas/canvasPointerLoop.test.ts`
- Modify: `src/features/application/editor/useEditorKeyboard.ts`
- Create: `src/features/application/editor/useEditorKeyboard.test.tsx`
- Modify: `src/features/application/editor/useCanvasOverlayHandlers.ts`
- Modify: `src/views/EditorView/Canvas/overlays/PinResultSearchPalette.tsx`
- Modify: `src/views/EditorView/Canvas/core/ConnectionLine.tsx`
- Modify: `src/views/EditorView/Canvas/core/ConnectionLine.test.tsx`
- Modify: `src/views/EditorView/Canvas/core/EdgesOverlay.tsx`
- Modify: `src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx`
- Modify: `src/views/EditorView/Pins/Pin.tsx`
- Modify: `src/views/EditorView/Pins/Pin.preview.test.tsx`
- Consume: `src/features/application/editorMutation/graphMutationError.ts` and the coordinator outcome defined in Task 6; do not duplicate error parsing in canvas code.

**Interfaces:**
- Plain drag starts `drawingConnection`; `Ctrl`/`Meta` + drag on a connected `canMove` pin starts `movingConnections`.
- Pointer release sends exactly one `ConnectPins` or `MoveConnections` command only for `append`/`replace`; invalid release sends none.
- Alt + left click sends exactly one `DisconnectPort` command.
- Escape precedence within Phase 1: drawing/moving → pending node creation/palette → dragging/selecting preview → current node selection → page-level Zen Mode. Edge selection is absent until Phase 2.

- [ ] **Step 1 (RED): Expand pointer-loop tests before wiring**

Add tests to `canvasPointerLoop.test.ts` for:

```ts
// one rAF-throttled local preview update and zero IPC during pointermove
// valid append release -> one ConnectPins
// valid replacement release -> one ConnectPins and advisory displaced-edge highlight
// Ctrl-drag connected port -> one MoveConnections with source/target addresses only
// Alt-left-click -> one DisconnectPort
// invalid release -> zero commands and idle state
// snapped release uses snapped pin, not raw event target
// Escape during draw/move -> zero command, idle state, unchanged projection
// failed/stale command clears preview but never applies optimistic topology
```

- [ ] **Step 2 (RED): Add visual and keyboard precedence tests**

Test `ConnectionLine`/`Pin`/`EdgesOverlay` receive and expose append/replace/invalid state without adding edge hit paths. Replacement highlights only advisory displaced visible edges; invalid feedback uses a localized reason label/tooltip and no raw backend text. Test Escape calls graph interaction cancellation before Zen Mode and preserves selection when only a gesture is cancelled.

- [ ] **Step 3: Run focused Vitest and confirm RED**

```sh
pnpm exec vitest run src/features/core/canvas/canvasPointerLoop.test.ts src/views/EditorView/Canvas/core/ConnectionLine.test.tsx src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx src/views/EditorView/Pins/Pin.preview.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
```

Expected: FAIL because these gestures/visual states/Escape handling are absent.

- [ ] **Step 4 (GREEN): Wire pointer start/move/release to explicit sessions**

On pointer down, copy only the source pin/address and local pointer state into the interaction session. During rAF frames, query canvas-scoped `[data-pin-id]` geometry, compute pure feedback/snapping, and update session state. On release:

```ts
if (feedback.kind === 'append' || feedback.kind === 'replace') {
  await executeCommand(graphPath, interaction.type === 'movingConnections'
    ? 'MoveConnections'
    : 'ConnectPins', payload);
}
cancelInteraction(graphPath);
```

Never mutate `graphDataStore` topology. The authoritative result/event remains the only committed topology update.

- [ ] **Step 5 (GREEN): Render feedback without Phase 2 edge interaction**

- `Pin.tsx`: apply data attributes/classes for snapped valid, replacement, and invalid hovered targets.
- `ConnectionLine.tsx`: render preview color/style from structured feedback and terminate at snapped geometry.
- `EdgesOverlay.tsx`: accept `highlightedConnectionIds` for replacement advisory styling only; do not add pointer events, hit paths, selection, context menus, or reroute behavior.

- [ ] **Step 6 (GREEN): Implement Escape precedence via globalEvent**

In `useEditorKeyboard.ts`, before the existing Zen Mode branch, inspect the active graph interaction and cancel in Phase 1 order. Abort selection preview through its existing cleanup path and clear node selection only after no transient interaction/palette remains. Move `PinResultSearchPalette`'s direct `document.addEventListener('keydown', ...)` Escape handling to `addGlobalEventListener` or route it through this same cancellation owner so there is one ordered global path. Continue registering the keyboard listener with `addGlobalEventListener(window, 'keydown', handleKeyDown, { capture: true })`.

- [ ] **Step 7 (GREEN): Add localized safe feedback and logging**

Add matching `canvas.connection.feedback.*` keys in both locale files for append, replace, and each local `ConnectionInvalidReason`. Known invalid drops never invoke. For a Task 6 `{ status: 'rejected', code }` outcome, log the technical error/code with operation context and toast only `t(graphMutationErrorMessageKey({ code }))`; never concatenate or display backend `message`, `details`, addresses, or UUIDs. For `{ status: 'conflict' }`, show the `graph_revision_conflict` key after the coordinator hydrates once; never replay.

- [ ] **Step 8: Run focused and broader frontend tests (GREEN)**

```sh
pnpm exec vitest run src/features/core/canvas/connectionInteraction.test.ts src/features/core/canvas/canvasPointerLoop.test.ts src/features/core/graphInteraction/graphInteractionStore.test.ts src/features/core/history/editorCommands.test.ts src/views/EditorView/Canvas/core/ConnectionLine.test.tsx src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx src/views/EditorView/Pins/Pin.preview.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx src/features/application/editorMutation/graphMutationError.test.ts src/features/application/editorMutation/editorMutationCoordinator.test.ts --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 9: Review checkpoint (no commit)**

Use source inspection to verify pointer movement contains no `executeCommand`, `GraphMutationService`, or `invoke`; only release/menu actions submit one intent. Do not commit.

---

### Task 9: Phase 1 cross-layer acceptance and scope audit

**Files:**
- Test-only corrections in files already listed above.
- Do not touch compiler/dataframe, Phase 2, Phase 3, sidebar cleanup user files, or create a commit.

**Interfaces:**
- Acceptance is one user-visible operation → one intent → one patch → one revision → one history entry → one authoritative projection installation.

- [ ] **Step 1: Run all focused Rust Phase 1 tests serially**

```sh
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi phase1_ --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi blueprint_graph_phase1_tests --jobs 1 -- --test-threads=1
pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi command_blueprint_graph_phase1_tests --jobs 1 -- --test-threads=1
```

Expected: PASS for stable errors, collection planning, replacement, movement, capabilities, and the complete six-row authority/history/concurrency/command/event matrices.

- [ ] **Step 2: Run Rust compile check serially**

```sh
pnpm exec cargo check --manifest-path src-tauri/Cargo.toml -p yssbi --jobs 1
```

Expected: PASS.

- [ ] **Step 3: Run the complete relevant frontend matrix serially**

```sh
pnpm exec vitest run src/shared/types/dto/editorMutationWireParser.test.ts src/services/nodeSystem/graphMutationService.test.ts src/services/nodeSystem/nodeSystemGoldenContracts.test.ts src/shared/utils/pinCompatibility.test.ts src/features/core/history/editorCommands.test.ts src/features/core/graphInteraction/graphInteractionStore.test.ts src/features/core/canvas/connectionInteraction.test.ts src/features/core/canvas/canvasPointerLoop.test.ts src/features/application/editor/useEditorOperations.capabilities.test.tsx src/features/application/editorMutation/graphMutationError.test.ts src/features/application/editorMutation/editorMutationCoordinator.test.ts src/views/EditorView/Canvas/core/ConnectionLine.test.tsx src/views/EditorView/Canvas/core/EdgesOverlay.test.tsx src/views/EditorView/Pins/Pin.preview.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx --pool=threads --maxWorkers=1 --no-file-parallelism --maxConcurrency=1
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 4: Run diff/scope checks**

```sh
git diff --check
git status --short
git diff --name-only
```

Expected: no whitespace errors; no modified path under `src-tauri/src/node_system/compiler/` or `src-tauri/src/node_system/catalog/dataframe/`; `src-tauri/src/project/production_tests.rs` contains only the two approved contract migrations (singular Disconnect and projection-failure precommit atomicity); pre-existing main-workspace dataframe and worktree sidebar/drag changes remain preserved.

- [ ] **Step 5: Audit removed compatibility and forbidden loops**

Run:

```sh
git grep -n -E "DeleteNode|type: 'deleteNode'|Self::Disconnect \{|type: 'disconnect'|canConnect" -- src-tauri/src/node_system src/features src/shared src/services
git grep -n -E "for .*executeGraphIntent|for .*DisconnectPin|for .*deleteNode" -- src/features
```

Expected: no production matches for removed singular mutation variants, old `canConnect`, or mutation loops. Test names asserting rejection of old wire formats may remain.

- [ ] **Step 6: Manual acceptance checklist**

With an event/function graph already loaded through the normal lifecycle, verify:

1. Connecting to an occupied `Single` port replaces atomically and one Undo restores the exact old edge.
2. Connecting to a full bounded `Multiple` port shows invalid feedback and submits nothing.
3. Ctrl-drag moves every authoritative source-port connection; Escape or invalid release changes nothing.
4. Alt-click pin, Break All Links on node, multi-node delete, and single-node delete each create one history step.
5. Snapping and valid/replace/invalid feedback update during pointer movement without network/IPC calls.
6. A forced stale revision refreshes authority state and does not replay the gesture.
7. Ordinary UI never shows raw UUID-bearing connection errors.
8. Existing pan, node drag, selection box, pending node-creation palette, and Zen Mode Escape still behave in their precedence order.

- [ ] **Step 7: Final review checkpoint (explicitly no commit)**

Record command output and any pre-existing unrelated failures for handoff. Do not run `git commit`, do not create a branch, and do not modify this plan to add Phase 2/3 follow-ups.

---

## Locked Decisions and Residual Risks

1. **Pan preservation is locked.** Although absent from the spec’s sample union, existing pan remains a `panning` variant in the same mutually exclusive `CanvasInteraction`; it is not left in a parallel gesture state.
2. **Backend error protocol is locked.** Task 1 defines the exact domain enum, wire codes, safe IPC shape, command mapping, and logger-only details. Expected Phase 1 failures must not use raw `InvalidEditorMutation` UI messages.
3. **Type authority is locked.** Ordinary connect/move planning captures each port’s real `TypeExpr` plus its owning protocol’s separate `Box<[TypeParameterId]>`, then calls `type_exprs_assignable(source, target, source_parameters, target_parameters)` through `compatibility.rs`. Missing type data is unavailable; `Unknown`/undeclared generic is unresolved; declared same-name, different-name, and nested applied generics are solver inputs. Create-and-connect uses its already-authoritative `SourcePort`/`CandidatePort` check. No compiler file is modified.
4. **Duplicate endpoint precedence is locked.** Exact-pair detection occurs before incumbent discovery and ID allocation; it is a zero-effect rejection, including zero emitted events.
5. **Test isolation is locked.** Blueprint transaction/history tests live in `project/blueprint_graph_phase1_tests.rs`; command/event tests live in `commands/command_blueprint_graph_phase1_tests.rs`. `project/production_tests.rs` receives no new Blueprint test block; only its pre-existing singular-Disconnect fixture and pre-existing projection-failure assertion are migrated to the new contracts, while all dataframe user changes remain untouched.
6. **Selection semantics remain Phase 1-neutral.** Moving session ownership must preserve current Shift-box behavior byte-for-byte; Shift union changes remain Phase 3.
7. **Replacement highlighting remains pointer-transparent.** Styling displaced IDs is allowed; hit paths, edge selection/context menus/deletion, and reroutes remain Phase 2.
8. **Unrelated work remains protected.** Preserve main-workspace dataframe changes and worktree sidebar/drag changes exactly; final diff review must distinguish them from Phase 1 files.
