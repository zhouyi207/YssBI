# Revisioned Mutation and Rust History Design

## Scope

This is the second frontend migration slice for `docs/plan/node-architecture.md`. It replaces legacy graph mutation commands, pin-UUID addressing, optimistic authoritative graph edits, and frontend-owned undo/redo with revisioned Rust mutations and Rust-owned project history.

The Editor Projection Foundation is a prerequisite. Every committed mutation returns or triggers a complete `EditorGraphProjectionDto` replacement; React never reconstructs committed graph state from a patch.

## Goals

- Make all graph writes revisioned and Rust-authoritative.
- Use stable `NodeId`, `ConnectionId`, and structured `PortAddress` identities.
- Make Rust allocate all persistent identities.
- Remove frontend graph-state rollback and history replay.
- Apply committed projection replacements atomically.
- Support cross-resource undo/redo without exposing intermediate state.
- Keep drag and similar high-frequency visual feedback in temporary UI state.
- Detect stale responses, event gaps, and conflicts deterministically.

## Non-goals

- Migrating the localized node catalog and creation palette.
- Enabling the current legacy catalog to create nodes through an ID alias.
- Migrating RunEvent, execution UI, or result sources.
- Implementing collaborative multi-user editing.
- Applying `GraphDocumentPatch` directly in React.
- Keeping a compatibility path for legacy pin UUID mutation commands.

## Interaction policy

Structural graph state is authoritative-response-first.

The following operations do not change the committed frontend projection before Rust succeeds:

- create node;
- delete node;
- connect/disconnect;
- set/clear literal;
- add/remove user-created port instance;
- function signature mutation;
- undo/redo.

Node drag uses a temporary position override while the pointer is moving. Pointer-up submits one `MoveNodes` mutation. A failed mutation clears the override and reveals the unchanged committed position.

No temporary interaction state participates in persistence, compilation, history, revision comparison, or project events.

## Architecture

```text
View gesture
  → application mutation use case
  → GraphMutationService
  → MutationRequest<EditorGraphMutationDto>
  → ProjectState authoritative transaction
  → ProjectHistoryTransaction
  → GraphMutationResultDto
  → atomic projection replacement
  → graphDataStore
```

Project events are synchronization notifications, not a second write path.

```text
GraphDelta from another operation/window
  → revision comparison
  → coalesced authoritative hydrate

ResourceMutationCommitted
  → validate every replacement
  → atomically install all replacements
```

## Backend mutation protocol

### High-level mutation DTO

Add a serializable application DTO. The frontend cannot submit an arbitrary `GraphDocumentPatch`.

```rust
#[serde(tag = "type", content = "payload", rename_all = "camelCase")]
pub enum EditorGraphMutationDto {
    CreateNode {
        node_type_id: NodeTypeId,
        position: NodePosition,
        parameters: ParameterValues,
        user_label: Option<String>,
    },
    DeleteNode {
        node_id: NodeId,
    },
    MoveNodes {
        positions: Vec<NodePositionMutationDto>,
    },
    Connect {
        output: PortAddress,
        input: PortAddress,
        order: Option<OrderKey>,
    },
    Disconnect {
        connection_id: ConnectionId,
    },
    SetLiteral {
        address: PortAddress,
        literal: Option<TypedValue>,
    },
    AddPortInstance {
        node_id: NodeId,
        template: PortKey,
        order: Option<OrderKey>,
    },
    RemovePortInstance {
        address: PortAddress,
    },
}
```

```rust
pub struct NodePositionMutationDto {
    pub node_id: NodeId,
    pub position: NodePosition,
}
```

Rust validates the stable node type and protocol capabilities before converting this DTO into an internal `GraphMutation` and `GraphDocumentPatch`.

### Identity ownership

Rust allocates:

- `NodeId` for `CreateNode`;
- `ConnectionId` for `Connect`;
- `PortInstanceId` for `AddPortInstance` and projected-member materialization;
- revisions and history entry IDs.

The frontend may allocate `OperationId` because it is correlation metadata only. It never allocates a persisted graph identity.

### Mutation request

The wire request is:

```rust
MutationRequest<EditorGraphMutationDto> {
    resource,
    base_revision,
    operation_id,
    payload,
}
```

The command validates that `graph_path`, `request.resource`, and the current graph revision all agree before building a patch.

### Mutation result

Graph mutations return:

```rust
pub struct GraphMutationResultDto {
    pub delta: GraphDeltaEvent<GraphDocumentPatch>,
    pub projection_replacement: GraphProjectionReplacementDto,
    pub history: HistoryStatusDto,
}
```

```rust
pub struct HistoryStatusDto {
    pub can_undo: bool,
    pub can_redo: bool,
}
```

The projection is generated from committed authoritative state using the request locale. It is not generated before the transaction commits.

### Graph event

`EventProject::GraphDelta` contains the committed delta and its `causedBy` operation ID. It does not contain a legacy node/pin DTO and is not applied directly to React graph entities.

The initiating window uses the command response projection. Other windows use the event as a revisioned invalidation signal and hydrate authoritative projection state.

### Resource mutation result

`ResourceMutationResultDto` for function signature and history mutations also includes `HistoryStatusDto`.

Every projection replacement is built before one `ResourceMutationCommitted` event is emitted. A multi-resource history transaction remains atomic.

## Backend mutation behavior

### Create node

- Validate `NodeTypeId` exists in the immutable registry.
- Validate parameters against `ParameterSpec`.
- Allocate `NodeId` in Rust.
- Persist only `DocumentNode`; fixed ports are projected, not persisted.
- Return the node through the committed projection.

The current legacy catalog cannot call this API with display-derived IDs. The API is complete in this slice, but the UI creation action is enabled only by the later localized Catalog/Creation Descriptor slice.

### Delete node

Rust creates one patch containing node removal, associated input states, dynamic port bindings, and every connected edge. No intermediate disconnected-but-present state is committed or emitted.

### Move nodes

A drag commit sends all final positions in one mutation. Rust validates every node before applying any position. The transaction is one undo entry.

### Connect/disconnect

Endpoints are structured `PortAddress` values. Rust validates direction, ownership, port existence, orphan status, type/schema compatibility, and connection cardinality. Connection IDs are allocated by Rust.

### Literals

Rust validates that the address is an input and that the protocol allows literals. Connection/literal/default precedence remains in `GraphDocument::effective_input_binding`.

### User-created port instances

Rust validates `PortInstances::UserCreated`, min/max constraints, and connection cleanup. Derived ports cannot be manually added or removed.

## Frontend modules

### Wire DTOs

Extend the shared neutral DTO boundary under:

```text
src/shared/types/dto/editorMutation.ts
```

It owns:

- mutation request/result DTOs;
- mutation discriminated union;
- graph/resource delta DTOs;
- history status DTO;
- projection replacement DTO reuse.

Services and domain/application code import from shared types. Domain code does not import services.

### IPC service

Create:

```text
src/services/nodeSystem/graphMutationService.ts
src/services/nodeSystem/historyService.ts
```

Methods:

```ts
mutate(graphPath, locale, request): Promise<GraphMutationResultDto>
undo(locale, request): Promise<ResourceMutationResultDto>
redo(locale, request): Promise<ResourceMutationResultDto>
getHistoryStatus(): Promise<HistoryStatusDto>
```

Services are thin IPC wrappers and do not import stores, views, or application hooks.

### Application mutation coordinator

Create:

```text
src/features/application/editorMutation/
```

Responsibilities:

- obtain current projection revision;
- create and register `operationId` before invoke;
- call the service;
- validate result resource/path/revision/operation correlation;
- atomically apply projection replacement;
- update history status;
- clear pending operation state;
- on conflict, hydrate current projection and return a structured application error.

It does not contain graph business validation duplicated from Rust.

### Pending operation registry

Store only correlation state:

```ts
pending[operationId] = {
  graphPath,
  baseRevision,
  mutationType,
}
```

This registry does not contain rollback graph data.

A matching `GraphDelta.causedBy` is suppressed only while its operation remains pending. No domain-key, node-ID, pin-ID, or endpoint-string heuristic is used.

## Projection application

### Single graph result

Before replacing the graph bucket, verify:

- replacement graph path matches the requested graph;
- `delta.fromRevision == request.baseRevision`;
- `delta.toRevision == replacement.projection.sourceRevision`;
- `delta.causedBy == request.operationId`;
- replacement revision is not older than the current projection.

Then call the existing atomic `replaceProjection` path.

### Multi-resource result

Validate every replacement first. If any replacement is malformed or internally inconsistent, install none of them and hydrate every affected graph path.

Add a store-level batch replacement API that builds every candidate before one Zustand `set`.

### Events

#### Matching pending echo

Ignore the event body and wait for the command response. Remove pending state only after response success/failure handling completes.

#### Other exact-next delta

Do not apply the patch. Mark the graph stale and coalesce one `hydrate_editor_graph` request.

#### Revision gap

Mark stale and hydrate. Do not attempt to replay missing patches.

#### Older event

Ignore.

#### ResourceMutationCommitted

Validate and batch-apply all projection replacements. Update history status from the event.

## Temporary interaction store

Create:

```text
src/features/core/graphInteraction/graphInteractionStore.ts
```

State:

```ts
positionOverrides: Record<GraphPath, Record<NodeId, Position>>
```

Actions:

```ts
setPositionOverride(graphPath, nodeId, position)
clearPositionOverrides(graphPath, nodeIds?)
clearGraphInteraction(graphPath)
```

Canvas selectors prefer an override for rendering only. `graphDataStore` remains unchanged during pointer movement.

Pointer-up submits one `MoveNodes` mutation. Success and failure both clear the relevant overrides after committed-result or error handling.

## Rust-owned history

### Frontend history state

Replace local undo/redo stacks with a small projection of backend status:

```ts
interface HistoryAvailabilityState {
  canUndo: boolean;
  canRedo: boolean;
  pending: 'undo' | 'redo' | null;
}
```

No command context, inverse patch, timestamp merge, or frontend history entry remains.

### Undo/redo flow

1. Read an anchor resource and its current revision from the authoritative projection/session.
2. Register an `operationId`.
3. Call Rust undo/redo.
4. Validate all deltas and projection replacements.
5. Batch-apply replacements.
6. Update `HistoryStatusDto`.

Project reload and project switch clear local availability and query Rust. Rust history remains session-local and is cleared by project reload.

### High-frequency merge behavior

Frontend history merge windows are removed. A drag produces one backend mutation on pointer-up, so it naturally creates one history transaction. Text/literal editors commit according to their existing explicit apply/debounce boundary, with each committed mutation becoming one Rust history entry.

## Error handling

### Revision conflict

On `graph_revision_conflict` or `history_revision_conflict`:

- remove pending correlation state;
- clear relevant interaction overrides;
- mark affected graph stale;
- hydrate authoritative projection;
- show one shared error toast;
- do not automatically retry the stale mutation.

### Validation and resource errors

Preserve the current committed projection. Clear temporary state and render the structured backend error through the shared toast system.

### Event or result inconsistency

Do not partially apply replacements. Log the operation ID, graph paths, expected/current revisions, and reason. Mark all affected graphs stale and hydrate.

### Save failure

A mutation committed in memory remains represented by its authoritative revision and dirty state. Save failure must not roll back React or silently restore a previous graph projection.

## Legacy removal

Remove in this slice:

- Zustand `undoStack` and `redoStack` ownership;
- frontend command `undo`/`redo` replay;
- `GraphUndoPatch` application;
- old node/pin/connection mutation IPC wrappers;
- pin UUID mutation request payloads;
- graph entity optimistic draft/reconcile/rollback paths;
- domain-key and endpoint-string echo suppression;
- old node/pin/connection graph DTO event handlers;
- old mutation event payload types.

Application command names may remain as UI use-case identifiers if they route exclusively through the new mutation coordinator and store no inverse context.

Do not remove Catalog components in this slice. Their create action remains unavailable until they consume stable creation descriptors in the next slice.

## Concurrency invariants

- Pending correlation is registered before `invoke`.
- One operation ID identifies one request and its emitted echo.
- A command response cannot overwrite a newer projection revision.
- Events from another operation never mutate graph entities directly.
- Revision gaps always trigger authoritative hydrate.
- Multi-resource replacements are all-or-none in React.
- Temporary interaction state never changes source revision.
- Project switch invalidates pending mutations, history requests, and hydration responses.
- Graph rename/unload invalidates pending operations for the old graph path.

## Testing

### Development test policy

During implementation, run only focused Rust tests with one build job:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- <exact-module-or-test-filter> --test-threads=1
```

Do not run full `cargo test` or full `pnpm rust:test` per task. This avoids excessive memory pressure and editor stalls.

Run the complete Rust suite once, at the final delivery checkpoint only. Run it serially with `CARGO_BUILD_JOBS=1`. If it still exceeds available resources, stop and report the exact command and termination instead of repeatedly retrying.

Frontend focused tests may use `pnpm exec vitest run <files>` to avoid the repository script's literal `--` behavior that otherwise runs the full frontend suite.

### Rust focused coverage

- high-level DTO converts to one valid patch;
- Rust allocates node/connection/port-instance identity;
- stale revision rejects without mutation/history consumption;
- move batch is atomic and one history entry;
- delete removes all dependent structures atomically;
- invalid address/direction/cardinality/literal policy rejects;
- undo/redo returns all deltas, replacements, and history availability;
- event/response serde fields and operation ID correlation;
- project reload clears history;
- no lock is held during projection construction or event emission.

### Frontend focused coverage

- wire DTO and service invocation shape;
- pending operation registered before invoke;
- matching echo suppression by operation ID only;
- response validation and atomic replacement;
- stale response cannot overwrite newer projection;
- exact-next/gap events trigger one coalesced hydrate;
- multi-resource replacement failure applies none;
- position drag changes interaction store only;
- move success/failure clears overrides;
- undo/redo uses Rust service and stores no inverse context;
- project switch/rename/unload invalidates pending operations;
- source audit proves production mutation paths do not invoke legacy mutation commands.

## Verification

During each task:

```sh
pnpm exec vitest run <focused-files>
pnpm typecheck
CARGO_BUILD_JOBS=1 pnpm rust:test -- <focused-filter> --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Final checkpoint only:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- --test-threads=1
pnpm rust:test:sci -- --test-threads=1
pnpm verify:frontend
git diff --check
```

Do not run `pnpm verify` before the explicit final checkpoint because it includes the full Rust suite.

## Completion criteria

The slice is complete when:

- every production graph mutation uses `MutationRequest<EditorGraphMutationDto>`;
- committed graph state changes only through Rust and projection replacement;
- persistent graph identities are allocated only by Rust;
- frontend graph history stacks and replay logic are removed;
- undo/redo use Rust history and batch projection replacements;
- temporary drag state is separate from committed projection;
- graph events use revision/operation correlation rather than old DTO payloads;
- no production mutation invokes a legacy node/pin/connection command;
- focused tests and static checks pass;
- the one final serial full-Rust checkpoint is run once and its result is reported accurately.

## Follow-up slices

1. Localized catalog, search index, stable creation descriptors, and enabling CreateNode UI.
2. RunEvent correlation, execution UI, result source DTOs, lifecycle release, and diagnostics navigation.
