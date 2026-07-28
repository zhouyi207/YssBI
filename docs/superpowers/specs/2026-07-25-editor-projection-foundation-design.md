# Editor Projection Foundation Design

## Scope

This is the first frontend migration slice for `docs/plan/node-architecture.md`. It replaces the graph-load and editor-hydration path with an authoritative Rust projection. It does not migrate graph mutations, history, catalog creation, or execution; those are separate dependent slices.

The slice is irreversible for graph loading: the frontend must not convert the new projection back into `GraphInstanceDTO`, call `resolve_graph_dynamic_pins`, or query `NodeDefinition` to reconstruct authoritative node interfaces.

## Goals

- Make `EditorGraphProjectionDto` sufficient to render the editor canvas.
- Load and hydrate graphs atomically by `graphPath` and projection basis.
- Preserve structured port addresses instead of introducing new persisted pin UUIDs.
- Render localized node/port metadata and diagnostics directly from Rust.
- Establish a projection store that later revisioned mutations and execution events can update.
- Remove the old graph-load response contract and dynamic-pin materialization step.

## Non-goals

- Migrating create/connect/delete/input mutation commands.
- Moving undo/redo to Rust history.
- Migrating the node catalog or drag creation payload.
- Migrating execution channels or result sources.
- Adding a compatibility adapter from projection DTOs to `GraphInstanceDTO`.
- Deleting every legacy graph DTO still needed by later, not-yet-migrated slices.

## Architecture

```text
ProjectState.project_data
  → GraphCompiler / AnalysisSnapshot
  → EditorGraphProjectionDto
  → graphProjectionService
  → validateEditorGraphProjection
  → projectionToGraphBucket
  → graphDataStore.replaceProjection
  → existing canvas selectors/components
```

Rust remains authoritative for document state, resolved interfaces, display metadata, diagnostics, and connection endpoints. React stores only an editor projection plus temporary interaction state.

The existing `graphDataStore` remains the single canvas entity store. It is refactored to accept projection replacements as its authoritative hydrate input instead of introducing a second long-lived graph store. Existing mutation methods remain until the next slice, but the load path cannot use old DTOs or registry enrichment.

## Rust projection contract

### Graph DTO

Extend `EditorGraphProjectionDto` with:

```rust
pub connections: Vec<EditorConnectionProjectionDto>
```

`connections` is sorted by stable `ConnectionId`.

### Node DTO

Extend `EditorNodeProjectionDto` with:

```rust
pub position: NodePositionDto
```

```rust
pub struct NodePositionDto {
    pub x: f64,
    pub y: f64,
}
```

Position comes directly from `GraphDocument.nodes[node_id].position`.

### Connection DTO

```rust
pub struct EditorConnectionProjectionDto {
    pub connection_id: Box<str>,
    pub output: PortAddressDto,
    pub input: PortAddressDto,
    pub order: Option<Box<str>>,
}
```

Connections preserve structured `PortAddressDto` endpoints. The frontend must not replace declared-port addresses with persisted pin UUIDs.

### Input binding DTO

Extend input `ResolvedPortDto` values with a purpose-specific editor binding summary:

```rust
pub input: Option<EditorInputBindingDto>
```

```rust
pub struct EditorInputBindingDto {
    pub literal_override: Option<serde_json::Value>,
    pub protocol_default: Option<serde_json::Value>,
    pub effective: EffectiveInputBindingKindDto,
}
```

```rust
pub enum EffectiveInputBindingKindDto {
    Connections,
    Literal,
    ProtocolDefault,
    Unbound,
}
```

Output ports use `None`. The DTO exposes editor values and precedence results, not protocol ASTs or compiler-local handles.

### Contract invariants

- `basis.graphPath`, top-level `graphPath`, and every node `graphPath` are equal.
- `basis.graphRevision == sourceRevision`.
- Every connection endpoint resolves to a projected port address.
- Node, port, connection, diagnostic, and related-location ordering is deterministic.
- The DTO contains no `GraphInstance`, `PinInstance`, snapshot-local handle, or localized identity.
- Locale changes may alter display/message text only; node IDs, node type IDs, addresses, connection IDs, position, and basis remain stable.

## Frontend modules

### Wire types

Create `src/services/nodeSystem/types.ts` containing only serializable camelCase wire DTOs matching Rust:

- `ProjectionBasisDto`
- `EditorGraphProjectionDto`
- `EditorNodeProjectionDto`
- `ResolvedPortDto`
- `PortAddressDto`
- `EditorConnectionProjectionDto`
- `EditorInputBindingDto`
- diagnostic and display DTOs

These types do not import React, Zustand, views, or legacy graph DTOs.

### IPC service

Create `src/services/nodeSystem/graphProjectionService.ts`:

```ts
loadGraph(graphPath: string, locale: string): Promise<EditorGraphProjectionDto>
hydrateGraph(graphPath: string, locale: string): Promise<EditorGraphProjectionDto>
```

- `loadGraph` invokes `load_project_graph`.
- `hydrateGraph` invokes `hydrate_editor_graph` for an already loaded resource.
- Both send an explicit locale.
- Neither performs domain conversion or state mutation.

### Pure projection domain

Create `src/features/domain/editorProjection/`:

- `types.ts`: opaque frontend projection types and `PortAddressKey`.
- `portAddressKey.ts`: one deterministic, collision-safe key function for structured port addresses.
- `validateProjection.ts`: runtime guards for graph path, revision, duplicate identities, and connection endpoints.
- `toGraphBucket.ts`: pure conversion to canvas entities.
- `index.ts`: public exports.

`PortAddressKey` is an in-memory lookup key only. It is derived from the full tagged address and is never persisted or sent to Rust as identity.

### Store integration

Refactor `graphDataStore` and `GraphEntityBucket` so a bucket also owns:

```ts
basis: ProjectionBasisDto
sourceRevision: number
diagnostics: DiagnosticDto[]
hasBlockingDiagnostics: boolean
```

Add one authoritative entry point:

```ts
replaceProjection(graphPath, projection, requestGeneration): ProjectionApplyResult
```

The replacement builds a complete bucket off-store, validates it, then swaps the bucket in one Zustand update. No partially replaced nodes or ports are observable.

Keep selectors narrow. Views continue selecting nodes, ports, and connections rather than subscribing to the full store.

## Canvas representation

The canvas may continue using string map keys internally, but those keys are derived `PortAddressKey` values. They are not backend pin IDs.

Projected node data contains:

- stable node ID and node type ID;
- Rust-provided title, description, user label, icon/style IDs;
- position;
- capabilities;
- parameter editors;
- node diagnostics.

Projected port data contains:

- full structured address;
- derived local address key;
- template key and display;
- direction, kind, instance kind, orphan/status;
- connection capability;
- resolved type/schema;
- input binding summary;
- port diagnostics selected from graph diagnostics.

Projected connections contain stable connection IDs and endpoint address keys derived from their structured endpoint DTOs.

The projection conversion must not call:

- `resolveNodeViewMeta`;
- `resolveEffectiveDefinition`;
- `buildInitialPins`;
- `crypto.randomUUID()`;
- any node registry lookup.

## Load and concurrency flow

1. `projectIOStore.loadGraph(graphPath)` allocates a monotonically increasing request generation for that graph.
2. It calls `graphProjectionService.loadGraph(graphPath, currentLocale)`.
3. The response is validated before state mutation.
4. The store ignores a response if a newer request generation exists.
5. A response with a lower `sourceRevision` than the current projection is rejected as stale.
6. A response with the same revision is allowed when it is the newest request; this supports locale-only rehydration.
7. The entire bucket and projection metadata are replaced atomically.
8. Resource/session state marks the graph loaded only after replacement succeeds.

Variables are not expected in the graph-load response. Project variables continue to come from the dedicated project resource query/store.

Locale changes re-run `hydrateGraph` for loaded graph tabs. They replace display and diagnostic text atomically without changing document identity.

## Error handling

- IPC failure preserves the last valid projection and marks the document stale/error through existing document/session state.
- Contract validation failure rejects the entire response and logs a structured projection-contract error.
- Stale responses are ignored without a user-facing error.
- A connection with a missing endpoint rejects the whole projection; it is not silently dropped.
- Blocking compiler diagnostics are still published and rendered; they do not make the projection unloadable.
- Unknown icon/style IDs fall back only at the visual component boundary and never alter node identity.

## Legacy removal in this slice

Remove from the graph-load path:

- `LoadedProjectGraphRow { graph, variables }`;
- `GraphInstanceDTO` as the `load_project_graph` response type;
- `ProjectService.loadProjectGraph`'s old contract;
- `GraphService.resolveGraphDynamicPins` invocation and fallback;
- `toFrontendGraph` conversion for graph loading;
- `resolveNodeViewMeta` enrichment during projection hydrate;
- variable merging from the graph-load response.

Delete `GraphService.resolveGraphDynamicPins` entirely if no remaining production caller exists after the cut. Do not delete unrelated legacy mutation DTOs until their owning slice migrates.

## Testing

### Rust

Add focused projection tests proving:

- node positions round-trip into projection;
- fixed and dynamic port addresses remain structured;
- connections are complete and deterministically ordered;
- input literal/default/connection precedence is projected correctly;
- orphan ports and structured diagnostic locations are preserved;
- locale changes do not alter identity, position, addresses, connections, or basis;
- malformed analysis/document source combinations fail projection construction.

### Frontend domain/store

Add tests proving:

- `portAddressKey` is deterministic and collision-safe across declared/instance addresses;
- projection conversion does not require a node registry;
- a complete projection atomically creates nodes, ports, connections, metadata, and diagnostics;
- lower revisions and older request generations are rejected;
- same-revision locale replacements update display without changing identity;
- missing connection endpoints reject the entire response;
- overlapping local node IDs remain isolated by `graphPath`.

### Application/service

Add tests proving:

- `loadGraph` sends `graphPath` and locale and consumes the projection directly;
- `resolve_graph_dynamic_pins` is never invoked;
- graph-load response variables are no longer expected;
- failed or stale loads preserve the last valid projection;
- a locale change rehydrates loaded tabs.

### Verification

Run from the repository root:

```sh
pnpm typecheck
pnpm test -- <focused frontend test files>
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::analysis::projection --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
pnpm verify
```

`pnpm verify` is required because this slice changes both Rust and React contracts.

## Completion criteria

The slice is complete when:

- opening an event or function graph renders solely from `EditorGraphProjectionDto`;
- node positions and all graph connections are present;
- dynamic and orphan interfaces arrive atomically from Rust;
- graph revision/basis and structured diagnostics are stored with the graph;
- locale refresh changes display only;
- the graph-load path has no `GraphInstanceDTO`, dynamic-pin command, frontend definition resolution, or pin generation;
- focused Rust/frontend tests, typecheck, formatting, `git diff --check`, and `pnpm verify` pass.

## Follow-up slices

1. Revisioned graph mutation, projection replacement events, operation-ID echo suppression, and Rust history.
2. Localized catalog, search index, stable creation descriptors, and Rust-authoritative node creation.
3. RunEvent correlation, execution UI, result source DTOs, release lifecycle, and diagnostics navigation.
