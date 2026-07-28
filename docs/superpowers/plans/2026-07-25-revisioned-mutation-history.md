# Revisioned Mutation and Rust History Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace legacy frontend graph mutation and undo/redo with revisioned Rust-authoritative mutations, committed projection replacements, and Rust-owned project history.

**Architecture:** React submits high-level serializable mutation intents against the current projection revision and never applies a committed graph patch locally. Rust validates the intent, allocates persistent identities, commits one history transaction, and returns a complete projection replacement; frontend graph state changes only through atomic projection replacement while drag previews live in a separate interaction store.

**Tech Stack:** Rust, serde, Tauri 2, TypeScript 5.8, React 19, Zustand, Vitest, pnpm.

## Global Constraints

- `ProjectState.project_data` remains authoritative.
- Do not expose arbitrary `GraphDocumentPatch` construction to React.
- Rust allocates every persistent `NodeId`, `ConnectionId`, and `PortInstanceId`.
- Frontend `OperationId` is correlation metadata only.
- No production mutation may address a fixed port by legacy pin UUID.
- No committed graph entity may be changed optimistically in `graphDataStore`.
- Temporary interaction state must live outside `graphDataStore` and never change source revision.
- Services must not import feature stores or views; domain code must not import services.
- Do not add legacy node ID aliases, pin-ID adapters, fallback mutation commands, or dual writes.
- CreateNode backend support is implemented, but the current legacy catalog UI remains disabled until the next stable creation-descriptor slice.
- Add every regression test before implementation and observe RED.
- During Tasks 1–6 run only focused Rust tests with `CARGO_BUILD_JOBS=1` and `--test-threads=1`.
- Do not run full `cargo test`, full `pnpm rust:test`, `pnpm verify`, or `pnpm verify:rust` during Tasks 1–6.
- Run the complete Rust suite exactly once in Task 7, serially.
- Do not commit; preserve unrelated working-tree changes.

---

### Task 1: Implement the high-level Rust mutation IR

**Files:**
- Modify: `src-tauri/src/node_system/document/model.rs`
- Modify: `src-tauri/src/node_system/document/mutation.rs`
- Modify: `src-tauri/src/node_system/document/patch.rs`
- Modify: `src-tauri/src/node_system/document/mod.rs`
- Modify: `src-tauri/src/node_system/document/tests.rs`
- Modify: `src-tauri/src/node_system/registry/mod.rs`
- Modify: `src-tauri/src/node_system/compiler/dynamic_interface.rs`

**Interfaces:**
- Consumes: immutable `NodeRegistry`, `GraphDocument`, protocol parameter/port constraints, and the request graph path.
- Produces: serializable `EditorGraphMutationDto`, `NodePositionMutationDto`, `DynamicPortBinding::UserCreated`, and `EditorGraphMutationDto::into_patch(...)` that allocates persistent identities in Rust.

- [ ] **Step 1: Add failing serde and identity-allocation tests**

Add focused tests named:

```text
editor_mutation_wire_is_stable_and_camel_case
create_connect_and_add_port_allocate_identity_in_rust
move_nodes_is_atomic_and_reversible
user_created_port_enforces_protocol_min_and_max
```

The wire test must cover every mutation variant. The identity test must assert the caller supplies no node/connection/port-instance ID and that the produced patch contains newly allocated Rust IDs.

- [ ] **Step 2: Run only the exact new tests and verify RED**

Run each filter sequentially:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::document::tests::editor_mutation_wire_is_stable_and_camel_case --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::document::tests::create_connect_and_add_port_allocate_identity_in_rust --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::document::tests::move_nodes_is_atomic_and_reversible --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::document::tests::user_created_port_enforces_protocol_min_and_max --exact --test-threads=1
```

Expected: compile failures because the high-level DTO and user-created binding do not exist.

- [ ] **Step 3: Add the serializable mutation DTO**

Implement the exact variants from the approved spec with:

```rust
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
```

Use existing strong identity, position, parameter, order, and value types. `CreateNode`, `Connect`, and `AddPortInstance` have no persisted identity field.

- [ ] **Step 4: Represent user-created port instances explicitly**

Add:

```rust
DynamicPortBinding::UserCreated { order: OrderKey }
```

Update serialization, structural validation, inverse patch behavior, resolved-interface materialization, and projection instance classification. Do not misuse `DynamicMemberLocator` for user-created ports.

- [ ] **Step 5: Convert intents to one validated patch**

Implement a conversion method that receives `graph_path`, `document`, and `registry`. It must:

- validate node type and parameters for CreateNode;
- allocate IDs in Rust;
- validate every move target before producing any `UpdateNode` operation;
- validate endpoint direction/kind/cardinality for Connect;
- validate literal policy for SetLiteral;
- validate `PortInstances::UserCreated`, min/max, template ownership, and cleanup for add/remove instance;
- reuse `delete_node_operations` for atomic deletion.

- [ ] **Step 6: Verify Task 1 with focused suites only**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::document::tests::editor_mutation --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::document::tests::user_created_port --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Do not run the full Rust suite.

---

### Task 2: Add committed mutation results and backend history status

**Files:**
- Modify: `src-tauri/src/node_system/document/history.rs`
- Modify: `src-tauri/src/node_system/document/mutation.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Modify: `src-tauri/src/commands/command_node_system.rs`
- Modify: `src-tauri/src/event/event_project.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/project/production_tests.rs`

**Interfaces:**
- Consumes: `MutationRequest<EditorGraphMutationDto>` from Task 1 and existing `ProjectHistory`.
- Produces: `GraphMutationResultDto`, `HistoryStatusDto`, `get_project_history_status`, and updated `ResourceMutationResultDto` with history status.

- [ ] **Step 1: Add failing command/result/history tests**

Add focused tests named:

```text
editor_mutation_returns_correlated_delta_projection_and_history_status
stale_editor_mutation_rejects_without_consuming_history
undo_redo_return_atomic_replacements_and_current_history_status
project_reload_clears_history_status
```

The first test must assert:

```text
delta.causedBy == request.operationId
delta.fromRevision == request.baseRevision
delta.toRevision == projection.sourceRevision
history.canUndo == true
history.canRedo == false
```

- [ ] **Step 2: Run only the exact new tests and verify RED**

Run sequentially:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::editor_mutation_returns_correlated_delta_projection_and_history_status --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::stale_editor_mutation_rejects_without_consuming_history --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::undo_redo_return_atomic_replacements_and_current_history_status --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::project_reload_clears_history_status --exact --test-threads=1
```

Expected: missing result/status types and command signatures.

- [ ] **Step 3: Add history availability to Rust**

Expose a read-only `ProjectHistory::status()` and `ProjectState::history_status()` returning:

```rust
HistoryStatusDto { can_undo, can_redo }
```

History status derives only from Rust stacks. Project reload/reset clears it.

- [ ] **Step 4: Implement high-level mutation application**

Add `ProjectState::apply_editor_graph_mutation(...)`:

1. take short snapshots of the document and immutable registry;
2. build the validated patch outside project write locks;
3. submit through the existing revisioned graph patch/history transaction path;
4. construct localized projection after commit and outside locks;
5. return correlated delta, replacement, and history status.

Do not emit events or compile projections while holding `project_data` locks.

- [ ] **Step 5: Change the Tauri graph mutation command contract**

`mutate_graph_document` now accepts `locale` and `MutationRequest<EditorGraphMutationDto>`, returns `GraphMutationResultDto`, and emits `GraphDelta` after commit. Remove public arbitrary-patch acceptance from this command.

Add thin `get_project_history_status`.

- [ ] **Step 6: Add history status to resource mutations**

Add `history` to `ResourceMutationResultDto`. Populate it for function signature, undo, redo, and variable effect results. Preserve one atomic `ResourceMutationCommitted` event.

- [ ] **Step 7: Verify Task 2 with focused tests only**

Run exact new tests, then:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- event::event_project::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- commands::command_node_system::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Do not run the full Rust suite.

---

### Task 3: Add frontend mutation DTOs, services, and atomic result application

**Files:**
- Create: `src/shared/types/dto/editorMutation.ts`
- Modify: `src/shared/types/dto/index.ts`
- Create: `src/services/nodeSystem/graphMutationService.ts`
- Create: `src/services/nodeSystem/historyService.ts`
- Modify: `src/services/nodeSystem/index.ts`
- Create: `src/features/application/editorMutation/pendingMutationRegistry.ts`
- Create: `src/features/application/editorMutation/applyMutationResult.ts`
- Create: `src/features/application/editorMutation/editorMutationCoordinator.ts`
- Create: `src/features/application/editorMutation/editorMutation.test.ts`
- Modify: `src/features/core/dataStore/graphDataStore.ts`
- Modify: `src/features/core/dataStore/graphEntityAccess.ts`
- Modify: `src/features/core/dataStore/graphProjectionStore.test.ts`

**Interfaces:**
- Consumes: Task 2 wire contracts and existing validated projection conversion.
- Produces: thin mutation/history services, pending operation correlation, `replaceProjectionsAtomically`, and `executeEditorMutation`.

- [ ] **Step 1: Add failing wire/service/coordinator tests**

Tests must prove:

- pending operation is registered before `invoke` executes;
- fixed ports are sent as structured addresses;
- response correlation/revisions are validated;
- a stale response cannot replace a newer projection;
- two valid replacements install in one store update;
- one malformed replacement causes zero replacements to install;
- conflict clears pending state and requests authoritative hydrate.

- [ ] **Step 2: Run only focused frontend tests and verify RED**

Run:

```sh
pnpm exec vitest run src/features/application/editorMutation/editorMutation.test.ts src/features/core/dataStore/graphProjectionStore.test.ts
```

Expected: missing DTO/service/coordinator/batch APIs.

- [ ] **Step 3: Define neutral shared wire DTOs**

Define exact camelCase discriminated unions matching Rust. Reuse `PortAddressDto`, `EditorGraphProjectionDto`, and projection replacement types from the shared projection DTO owner. Do not import service modules from domain/application types.

- [ ] **Step 4: Implement thin services**

Use `invoke` only. Send explicit `graphPath`, locale, and request. Services return typed DTOs and do not mutate stores or show toasts.

- [ ] **Step 5: Implement pending operation correlation**

Provide:

```ts
registerPendingMutation(record)
getPendingMutation(operationId)
completePendingMutation(operationId)
invalidatePendingMutationsForGraph(graphPath)
resetPendingMutations()
```

Use operation ID only; remove all domain-key/endpoint heuristics from the new path.

- [ ] **Step 6: Add atomic batch projection replacement**

Build and validate every candidate before one Zustand `set`. Validate duplicate graph paths, revision monotonicity, and projection path identity. Return a structured all-or-none result.

- [ ] **Step 7: Implement `executeEditorMutation`**

Read current projection basis, allocate an operation ID, register before service invocation, validate the committed result, atomically replace projection, update history status, and complete pending state in `finally`. On revision conflict, mark stale and hydrate without retrying.

- [ ] **Step 8: Verify Task 3 without full suites**

Run:

```sh
pnpm exec vitest run src/features/application/editorMutation/editorMutation.test.ts src/features/core/dataStore/graphProjectionStore.test.ts src/services/nodeSystem/graphProjectionService.test.ts
pnpm typecheck
git diff --check
```

No Rust full suite.

---

### Task 4: Replace legacy graph events with revisioned synchronization

**Files:**
- Create: `src/features/core/sync/handlers/ProjectMutationEventHandler.ts`
- Create: `src/features/core/sync/handlers/ProjectMutationEventHandler.test.ts`
- Modify: `src/features/core/sync/handlers/index.ts`
- Modify: `src/features/core/sync/types.ts`
- Modify: `src/features/core/sync/utils/eventParser.ts`
- Delete: `src/features/core/sync/handlers/NodeEventHandler.ts`
- Delete: `src/features/core/sync/handlers/ConnectionEventHandler.ts`
- Delete related legacy handler tests after replacement coverage exists
- Modify: `src/features/application/editorProjection/graphProjectionCoordinator.ts`

**Interfaces:**
- Consumes: `GraphDelta` and `ResourceMutationCommitted` wire DTOs from Task 3.
- Produces: operation-ID echo suppression, exact-next/gap/older revision policy, coalesced hydrate, and atomic multi-resource replacement handling.

- [ ] **Step 1: Add failing event ordering tests**

Cover:

```text
matching pending echo is suppressed
other exact-next delta hydrates once
gap delta hydrates once
older delta is ignored
resource mutation applies all valid replacements atomically
invalid resource replacement installs none and hydrates every affected graph
```

Dispatch realistic nested backend wire through `EventRegistry`, not handlers directly.

- [ ] **Step 2: Run the focused event tests and verify RED**

Run:

```sh
pnpm exec vitest run src/features/core/sync/handlers/ProjectMutationEventHandler.test.ts
```

- [ ] **Step 3: Implement revisioned project mutation handlers**

`GraphDelta` never applies patch operations to graph entities. It compares event revisions with the current projection and either suppresses, ignores, or requests one coalesced hydrate.

`ResourceMutationCommitted` validates all replacements before one atomic install and updates backend history status.

- [ ] **Step 4: Remove legacy graph DTO handlers and payload types**

Remove registrations/types for node-created/deleted/updated, pin-updated/inferred, and connection-created/deleted batch events. Keep unrelated project/resource/database/variable events.

- [ ] **Step 5: Remove heuristic echo suppression from graph synchronization**

Delete graph mutation usage of domain keys, node IDs, pin IDs, and endpoint strings. Leave unrelated non-graph suppressors only if they still have a producer.

- [ ] **Step 6: Verify Task 4 focused**

Run:

```sh
pnpm exec vitest run src/features/core/sync/handlers/ProjectMutationEventHandler.test.ts src/features/core/sync/registry/EventRegistry.test.ts
pnpm typecheck
git diff --check
```

No Rust full suite.

---

### Task 5: Move drag previews out of committed graph state and migrate editor commands

**Files:**
- Create: `src/features/core/graphInteraction/graphInteractionStore.ts`
- Create: `src/features/core/graphInteraction/graphInteractionStore.test.ts`
- Modify: `src/features/core/dataStore/graphDataStore.ts`
- Modify: `src/features/core/dataStore/graphEntityAccess.ts`
- Modify: `src/features/core/canvas/canvasPointerLoop.ts`
- Create: `src/features/core/canvas/canvasPointerLoop.test.ts`
- Modify: `src/features/core/history/commandExecutor.ts`
- Create: `src/features/core/history/editorCommands.test.ts`
- Replace implementations under `src/features/core/history/commands/` for move, literal, connect, disconnect, delete, and repeatable-port actions
- Delete legacy create/composite command implementation from production registry until the Catalog slice enables stable creation
- Delete: `src/services/graph/node/nodeService.ts`
- Delete: `src/services/graph/pin/pinService.ts`
- Delete: `src/services/graph/connection/connectionService.ts`
- Modify: `src/services/graph/graphService.ts`

**Interfaces:**
- Consumes: `executeEditorMutation` from Task 3.
- Produces: temporary position overrides and response-first editor mutation use cases.

- [ ] **Step 1: Add failing interaction and command tests**

Prove:

- pointer move changes only `positionOverrides`, not committed node position/revision;
- pointer-up sends one `moveNodes` mutation containing all final positions;
- success and failure clear overrides;
- connect/disconnect/literal/delete/repeatable-port actions do not mutate graph entities before response;
- no persistent identity is generated by `crypto.randomUUID` in production mutation modules;
- CreateNode UI action is explicitly unavailable rather than routed through a legacy alias.

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```sh
pnpm exec vitest run src/features/core/graphInteraction/graphInteractionStore.test.ts src/features/core/canvas/canvasPointerLoop.test.ts src/features/core/history/editorCommands.test.ts
```

Do not run all frontend tests.

- [ ] **Step 3: Implement the interaction store**

Use graph-scoped position overrides with focused actions from the spec. Canvas node-position selectors prefer an override without mutating `graphDataStore`.

- [ ] **Step 4: Migrate command execution to forward-only use cases**

`executeCommand` may keep command names for UI routing but returns mutation results, stores no inverse context, and does not push frontend history entries. Move/connect/disconnect/literal/delete/repeatable-port commands build high-level DTOs and call the coordinator.

- [ ] **Step 5: Remove committed optimistic APIs**

Delete graph store methods for authoritative add/delete/pin/connect drafts, rollback, reconcile, and batch DTO writes. Keep only projection replacement, selectors, and temporary-state-independent read APIs.

- [ ] **Step 6: Remove legacy graph mutation services**

Delete old node/pin/connection invoke wrappers and their exports. Source-audit production mutation modules for forbidden old command names and pin UUID payload fields.

- [ ] **Step 7: Verify Task 5 focused**

Run the exact changed Vitest files, then:

```sh
pnpm typecheck
git diff --check
```

No Rust full suite.

---

### Task 6: Replace frontend history stacks with Rust history availability

**Files:**
- Replace: `src/features/core/history/historyStore.ts`
- Delete obsolete inverse-context types from `src/features/core/history/types.ts`
- Delete obsolete command registry context mappings from `src/features/core/history/commands/registryTypes.ts`
- Modify: `src/features/application/editor/useEditorHistoryAvailability.ts`
- Create: `src/features/application/editor/useEditorHistoryAvailability.test.ts`
- Modify: `src/features/application/editor/useEditorOperations.ts`
- Modify: `src/features/application/editor/useEditorKeyboard.ts`
- Modify: `src/features/application/editor/editorSessionCommands.ts`
- Create: `src/features/application/editorMutation/historyCoordinator.ts`
- Create: `src/features/application/editorMutation/historyCoordinator.test.ts`
- Delete: `src/shared/types/dto/graphUndoPatch.ts`
- Remove GraphUndoPatch exports/usages

**Interfaces:**
- Consumes: `HistoryService`, `HistoryStatusDto`, atomic batch projection replacement, and current resource revision.
- Produces: backend-derived `canUndo/canRedo`, Rust undo/redo flow, and no frontend inverse replay.

- [ ] **Step 1: Add failing Rust-history frontend tests**

Prove:

- history store contains only availability and pending state;
- undo/redo call Rust service with anchor revision and operation ID;
- all replacements install atomically;
- conflict hydrates affected graphs and changes no committed entities locally;
- project switch clears availability and pending history request;
- no command context, timestamp merge, inverse patch, or graph snapshot remains.

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```sh
pnpm exec vitest run src/features/application/editorMutation/historyCoordinator.test.ts src/features/application/editor/useEditorHistoryAvailability.test.ts
```

- [ ] **Step 3: Replace the history store**

Store only:

```ts
canUndo
canRedo
pending
```

Update solely from backend status/query/results/events.

- [ ] **Step 4: Implement undo/redo coordinator**

Choose the current graph/function resource as concurrency anchor, register operation ID before invoke, validate all returned deltas/replacements, batch apply, update history status, and clear pending in `finally`.

- [ ] **Step 5: Remove inverse replay and GraphUndoPatch**

Delete stack push/merge/replay code, command handler `undo`/`redo`, inverse context DTOs, and old patch service calls. Update keyboard/menu availability selectors.

- [ ] **Step 6: Add source audit for mutation/history legacy paths**

Explicitly audit production mutation/history modules for:

```text
create_node
connect_pins
update_pin_user_value
apply_graph_patch
GraphUndoPatch
undoStack
redoStack
crypto.randomUUID persisted identity generation
```

- [ ] **Step 7: Verify Task 6 focused**

Run changed history/mutation/event tests plus:

```sh
pnpm typecheck
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::revisioned_signature_undo_and_redo_reject_conflicts_and_return_deltas --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

No full Rust suite.

---

### Task 7: Final review, ledgers, and the single full-suite checkpoint

**Files:**
- Modify: `.superpowers/sdd/progress.md`
- Modify: `.superpowers/sdd/task-production-backend-report.md`
- Create: focused source-audit test under `src/services/nodeSystem/editorMutationContract.test.ts`
- No production file is pre-authorized for Task 7; any final-review fix must enter the review fix loop with its exact file list and focused regression test

**Interfaces:**
- Consumes: completed Tasks 1–6.
- Produces: verified mutation/history cut, accurate ledgers, and one recorded full Rust checkpoint.

- [ ] **Step 1: Run focused preflight before the expensive checkpoint**

Run:

```sh
pnpm exec vitest run src/features/application/editorMutation src/features/core/graphInteraction src/features/core/sync/handlers/ProjectMutationEventHandler.test.ts src/services/nodeSystem/editorMutationContract.test.ts
pnpm typecheck
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Stop and fix slice-caused failures before running the full suite.

- [ ] **Step 2: Run the complete frontend verification**

Run:

```sh
pnpm verify:frontend
```

- [ ] **Step 3: Run the complete Rust suite exactly once**

Run serially with a generous timeout:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- --test-threads=1
```

Do not retry if it exhausts memory or stalls. Record the exact final output/termination.

- [ ] **Step 4: Run the scientific Rust suite once if the main Rust process completed**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test:sci -- --test-threads=1
```

- [ ] **Step 5: Run final static checks**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

- [ ] **Step 6: Update ledgers accurately**

Record this slice complete only if production mutation/history paths are Rust-authoritative. Record full-suite failures or resource termination exactly. Keep Catalog creation and execution integration open; do not claim the full node architecture complete.

- [ ] **Step 7: Review final scope**

Confirm no unrelated user changes were reverted, no legacy mutation compatibility path was added, no commit was created, and the final diff is limited to mutation/history/projection synchronization plus approved lifecycle support.
