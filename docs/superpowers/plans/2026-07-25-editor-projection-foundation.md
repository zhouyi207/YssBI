# Editor Projection Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the React editor load and render graphs exclusively from a complete, localized, revisioned `EditorGraphProjectionDto` produced by Rust.

**Architecture:** Extend the Rust projection with node positions, graph connections, and editor input-binding summaries. Add purpose-specific frontend wire/domain modules, then atomically replace each `graphDataStore` bucket from validated projections without converting through `GraphInstanceDTO` or querying the frontend node registry.

**Tech Stack:** Rust, serde, Tauri 2, TypeScript 5.8, React 19, Zustand, Vitest, pnpm.

## Global Constraints

- `ProjectState.project_data` remains the authoritative graph state.
- React stores only the latest editor projection and temporary interaction state.
- Do not add a projection-to-`GraphInstanceDTO` adapter, fallback reader, dual hydrate path, or old-ID alias.
- Frontend services may import wire DTOs but must not import views or feature stores.
- Projection conversion must not call `resolveNodeViewMeta`, `resolveEffectiveDefinition`, `buildInitialPins`, `crypto.randomUUID`, or a node registry.
- All projection replacement is atomic by `graphPath` and carries `basis`, `sourceRevision`, diagnostics, and blocking state.
- Preserve later-slice legacy mutation/history/execution code unless this graph-load cut makes a symbol unreachable.
- Add each regression test before implementation and observe the expected RED result.
- Run Rust commands sequentially with `CARGO_BUILD_JOBS=1`.
- Do not commit; preserve unrelated working-tree changes.

---

### Task 1: Complete the Rust editor projection contract

**Files:**
- Modify: `src-tauri/src/node_system/analysis/projection.rs`
- Test: `src-tauri/src/node_system/analysis/projection.rs`

**Interfaces:**
- Consumes: `GraphDocument.nodes`, `GraphDocument.connections`, `GraphDocument.input_states`, protocol defaults, and `AnalysisSnapshot` resolved interfaces.
- Produces: `NodePositionDto`, `EditorConnectionProjectionDto`, `EditorInputBindingDto`, `EffectiveInputBindingKindDto`, and complete fields on `EditorGraphProjectionDto`, `EditorNodeProjectionDto`, and `ResolvedPortDto`.

- [ ] **Step 1: Add a failing complete-projection test**

Create a focused test named `editor_projection_includes_positions_connections_and_input_bindings`. Build a document containing two positioned nodes, a declared output-to-input connection, one literal input, one protocol-default input, and one unbound input. Assert:

```rust
assert_eq!(projection.nodes[0].position, NodePositionDto { x: 12.5, y: -4.0 });
assert_eq!(projection.connections[0].connection_id.as_ref(), "connection-1");
assert!(matches!(projection.connections[0].output, PortAddressDto::Declared { .. }));
assert_eq!(literal.input.as_ref().unwrap().effective, EffectiveInputBindingKindDto::Literal);
assert_eq!(connected.input.as_ref().unwrap().effective, EffectiveInputBindingKindDto::Connections);
assert_eq!(defaulted.input.as_ref().unwrap().effective, EffectiveInputBindingKindDto::ProtocolDefault);
assert_eq!(unbound.input.as_ref().unwrap().effective, EffectiveInputBindingKindDto::Unbound);
```

- [ ] **Step 2: Run the exact test and verify RED**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::analysis::projection::tests::editor_projection_includes_positions_connections_and_input_bindings --exact --test-threads=1
```

Expected: compile failure because the new DTO fields and types do not exist.

- [ ] **Step 3: Add the DTO types and fields**

Add camelCase serde DTOs:

```rust
pub struct NodePositionDto { pub x: f64, pub y: f64 }
pub struct EditorConnectionProjectionDto {
    pub connection_id: Box<str>,
    pub output: PortAddressDto,
    pub input: PortAddressDto,
    pub order: Option<Box<str>>,
}
pub struct EditorInputBindingDto {
    pub literal_override: Option<serde_json::Value>,
    pub protocol_default: Option<serde_json::Value>,
    pub effective: EffectiveInputBindingKindDto,
}
pub enum EffectiveInputBindingKindDto { Connections, Literal, ProtocolDefault, Unbound }
```

Add `connections`, `position`, and `input` fields exactly as specified in the approved design.

- [ ] **Step 4: Project the new fields from authoritative sources**

Use `DocumentNode.position` directly. Build connections from the document's `BTreeMap`, preserving stable ID order. For each input port, obtain the literal override from `document.input_states`, the protocol default from `PortSpec.input_binding.default_value`, and effective precedence from `GraphDocument::effective_input_binding(address, default)`. Output ports receive `input: None`.

- [ ] **Step 5: Extend existing fixtures and delta behavior**

Add `connections: Vec<EditorConnectionProjectionDto>` to `GraphProjectionDelta` and replace the complete connection list in `apply_delta` together with node replacements and diagnostics. The test must prove a connection change cannot retain the old endpoints.

- [ ] **Step 6: Verify Task 1 GREEN**

Run sequentially:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::analysis::projection --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
```

Expected: PASS.

---

### Task 2: Add frontend wire types and pure projection validation

**Files:**
- Create: `src/services/nodeSystem/types.ts`
- Create: `src/services/nodeSystem/graphProjectionService.ts`
- Create: `src/services/nodeSystem/index.ts`
- Create: `src/features/domain/editorProjection/types.ts`
- Create: `src/features/domain/editorProjection/portAddressKey.ts`
- Create: `src/features/domain/editorProjection/validateProjection.ts`
- Create: `src/features/domain/editorProjection/toProjectionEntities.ts`
- Create: `src/features/domain/editorProjection/index.ts`
- Create: `src/features/domain/editorProjection/editorProjection.test.ts`
- Modify: `src/services/index.ts`

**Interfaces:**
- Consumes: the camelCase Rust DTO from Task 1.
- Produces: `GraphProjectionService.loadGraph`, `GraphProjectionService.hydrateGraph`, `portAddressKey`, `validateEditorGraphProjection`, and `toProjectionEntities`.

- [ ] **Step 1: Write failing wire/domain tests**

Add tests that construct declared and instance addresses and assert:

```ts
expect(portAddressKey(declared)).toBe(portAddressKey({ ...declared }));
expect(portAddressKey(declared)).not.toBe(portAddressKey(instance));
expect(() => validateEditorGraphProjection(missingEndpointProjection)).toThrow(
  "projection connection 'connection-1' references a missing port",
);
```

Also assert a valid projection converts without a registry and preserves stable node type ID, localized display, position, addresses, connections, diagnostics, parameter editors, and input binding.

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```sh
pnpm test -- src/features/domain/editorProjection/editorProjection.test.ts
```

Expected: FAIL because the modules do not exist.

- [ ] **Step 3: Define exact wire DTOs**

Mirror Rust serde shapes in `src/services/nodeSystem/types.ts`. Use discriminated unions:

```ts
export type PortAddressDto =
  | { kind: 'declared'; nodeId: string; portKey: string }
  | { kind: 'instance'; nodeId: string; templateKey: string; instanceId: string };
```

Use `number` for revisions and `Record<string, string>` for the transparent Rust `ResourceVersionSet` JSON object. Use explicit unions for diagnostic locations and enum fields. Do not import legacy DTOs.

- [ ] **Step 4: Implement the thin IPC service**

Implement:

```ts
GraphProjectionService.loadGraph(graphPath, locale)
GraphProjectionService.hydrateGraph(graphPath, locale)
```

with `invoke<EditorGraphProjectionDto>('load_project_graph', { graphPath, locale })` and `invoke<EditorGraphProjectionDto>('hydrate_editor_graph', { graphPath, locale })`.

- [ ] **Step 5: Implement collision-safe address keys and guards**

Use a tagged, length-prefixed key rather than delimiter concatenation:

```ts
const part = (value: string) => `${value.length}:${value}`;
```

Validate graph-path equality, `basis.graphRevision === sourceRevision`, unique node/port/connection identities, node ownership of port addresses, and connection endpoint existence. Return the validated DTO; never repair malformed input.

- [ ] **Step 6: Implement pure entity conversion**

Return an `EditorProjectionEntities` object containing records for nodes, ports, connections and indexes. Keep full structured addresses on port entities and derive local address keys only for record indexing. Use Rust display/capabilities/diagnostics directly.

- [ ] **Step 7: Verify Task 2 GREEN**

Run:

```sh
pnpm test -- src/features/domain/editorProjection/editorProjection.test.ts
pnpm typecheck
```

Expected: PASS.

---

### Task 3: Make `graphDataStore` projection-backed and revision-aware

**Files:**
- Modify: `src/shared/types/store/graph.ts`
- Modify: `src/features/core/dataStore/graphEntityAccess.ts`
- Modify: `src/features/core/dataStore/graphDataStore.ts`
- Modify: `src/features/core/dataStore/graphDataStore.test.ts`
- Create: `src/features/core/dataStore/graphProjectionStore.test.ts`
- Modify: `src/features/core/dataStore/nodeView.ts`
- Modify: `src/views/EditorView/Nodes/DefaultNodeLayout.tsx`
- Modify: `src/views/EditorView/Nodes/MathNodeLayout.tsx`
- Modify: `src/views/EditorView/Layout/Detail/panels/NodeDetailPanel.tsx`

**Interfaces:**
- Consumes: `EditorGraphProjectionDto` and `EditorProjectionEntities` from Task 2.
- Produces: `replaceProjection(graphPath, projection, requestGeneration): ProjectionApplyResult`, projection metadata selectors, and canvas-compatible projected node/port/connection entities.

- [ ] **Step 1: Add failing atomic replacement tests**

Cover:

```ts
expect(result).toEqual({ applied: true, reason: 'newer' });
expect(bucket.sourceRevision).toBe(4);
expect(bucket.nodes[nodeId].title).toBe('Localized title');
expect(bucket.connections[connectionId].from).toBe(portAddressKey(output));
expect(bucket.diagnostics).toEqual(projection.diagnostics);
```

Add separate tests proving lower revisions and older request generations are ignored, same-revision newer-generation locale replacements update display, malformed projections leave the previous bucket byte-for-byte unchanged, and overlapping node IDs remain isolated by `graphPath`.

- [ ] **Step 2: Run the store tests and verify RED**

Run:

```sh
pnpm test -- src/features/core/dataStore/graphProjectionStore.test.ts
```

Expected: FAIL because projection metadata and `replaceProjection` do not exist.

- [ ] **Step 3: Extend projection-backed store entities**

Add structured projection fields to `NodeData`, `PinData`, and `ConnectionData` without storing a second identity. `NodeData.nodeType` must contain the stable Rust `nodeTypeId`; do not add a display-name node type alias. `PinData.id` becomes the local `PortAddressKey` and stores the full `address`.

Extend `GraphEntityBucket` with:

```ts
basis: ProjectionBasisDto;
sourceRevision: number;
requestGeneration: number;
diagnostics: DiagnosticDto[];
hasBlockingDiagnostics: boolean;
```

- [ ] **Step 4: Implement atomic `replaceProjection`**

Validate and convert before `set`. Apply only when:

- no current bucket exists; or
- `requestGeneration` is newer and `sourceRevision >= current.sourceRevision`.

Allow equal-revision replacements for localization. Reject older generations even when their revision is equal or higher, because the newer request owns the response order.

- [ ] **Step 5: Remove registry enrichment from projection replacement**

The projection path must not call `enrichNodeData`/`resolveNodeViewMeta`. Keep legacy `addGraphFromData` only for still-unmigrated tests/paths, clearly separate from `replaceProjection`; it must not be called by graph loading after Task 4.

- [ ] **Step 6: Adapt the known registry-dependent canvas consumers**

Change `nodeView.ts` to use projected title/description/style instead of `resolveNodeViewMeta`. Change `DefaultNodeLayout.tsx` and `MathNodeLayout.tsx` to derive repeatable-port controls from projected `instanceKind`/`canRemove` fields rather than `NodeDefinition.pinSlots`. Change `NodeDetailPanel.tsx` to render projected ports, parameter editors, capabilities, and diagnostics without `resolveEffectiveDefinition`. Icon/style fallback belongs in visual components. Do not infer ports from node type.

- [ ] **Step 7: Verify Task 3 GREEN**

Run:

```sh
pnpm test -- src/features/core/dataStore/graphProjectionStore.test.ts src/features/core/dataStore/graphDataStore.test.ts
pnpm typecheck
```

Expected: PASS.

---

### Task 4: Cut graph loading over to the projection service

**Files:**
- Modify: `src/features/core/dataStore/projectIOStore.ts`
- Modify: `src/features/core/dataStore/projectIOStore.test.ts`
- Modify: `src/services/project/projectService.ts`
- Modify: `src/services/graph/graphService.ts`
- Create: `src/features/application/editor/useProjectionLocaleSync.ts`
- Create: `src/features/application/editor/useProjectionLocaleSync.test.ts`
- Modify: `src/views/EditorView/EditorWindow.tsx`

**Interfaces:**
- Consumes: `GraphProjectionService` and `useGraphDataStore.replaceProjection`.
- Produces: graph load and locale rehydrate paths with latest-request-wins semantics.

- [ ] **Step 1: Replace old load tests with failing projection tests**

Mock `GraphProjectionService.loadGraph` and assert:

```ts
expect(GraphProjectionService.loadGraph).toHaveBeenCalledWith(graphPath, 'zh-CN');
expect(GraphService.resolveGraphDynamicPins).not.toHaveBeenCalled();
expect(useGraphDataStore.getState().getProjectionBasis(graphPath)).toEqual(projection.basis);
```

Add tests for stale cached graph reload, IPC failure preserving an existing bucket, an older in-flight response being ignored, and same-revision locale rehydrate updating display.

- [ ] **Step 2: Run the focused application tests and verify RED**

Run:

```sh
pnpm test -- src/features/core/dataStore/projectIOStore.test.ts
```

Expected: FAIL because `projectIOStore` still expects `{ graph, variables }` and calls dynamic-pin materialization.

- [ ] **Step 3: Implement request-generation ownership**

Maintain a per-graph generation counter next to `loadGraphInFlight`. Each new load/rehydrate request captures its generation and passes it to `replaceProjection`. Do not let an older promise clear a newer in-flight entry.

- [ ] **Step 4: Replace the graph-load workflow**

Use the current i18n locale, call `GraphProjectionService.loadGraph`, atomically apply it, and only then mark the graph resource/session loaded. Remove graph-response variable merging, `toFrontendGraph`, signature reconstruction from graph snapshots, dynamic-pin calls, and fallback hydration.

- [ ] **Step 5: Add locale rehydration for loaded tabs**

Implement `useProjectionLocaleSync` using `react-i18next`'s current language and loaded graph resources. On language change, call `GraphProjectionService.hydrateGraph` once per loaded graph path and apply through the same request-generation coordinator. Mount the hook in `EditorWindow.tsx`. Preserve temporary canvas interaction state outside the replaced projection bucket.

- [ ] **Step 6: Remove the obsolete service contract**

Delete `LoadedProjectGraphRow` and `ProjectService.loadProjectGraph`. Delete `GraphService.resolveGraphDynamicPins` if search confirms no remaining production callers. Remove now-unused imports and tests.

- [ ] **Step 7: Verify Task 4 GREEN**

Run:

```sh
pnpm test -- src/features/core/dataStore/projectIOStore.test.ts
pnpm typecheck
```

Expected: PASS.

---

### Task 5: Contract cleanup, regression matrix, and delivery verification

**Files:**
- Modify: `.superpowers/sdd/progress.md`
- Create or modify focused contract tests under `src/services/nodeSystem/` if service invocation is not already covered
- Modify: `src/features/core/dataStore/index.ts`
- Modify: `src/services/index.ts`
- Modify: old graph-load-only mocks/imports in `src/features/core/dataStore/projectIOStore.test.ts`

**Interfaces:**
- Consumes: completed Tasks 1–4.
- Produces: an accurate migration ledger and a verified first frontend slice.

- [ ] **Step 1: Add a no-legacy-load regression test**

Add a source-level or module-level test proving the production graph-load path does not reference:

```text
GraphInstanceDTO
resolve_graph_dynamic_pins
resolveEffectiveDefinition
toFrontendGraph
```

The test should target graph loading only; do not require deleting symbols still owned by later slices.

- [ ] **Step 2: Run focused frontend suites**

Run:

```sh
pnpm test -- src/features/domain/editorProjection/editorProjection.test.ts src/features/core/dataStore/graphProjectionStore.test.ts src/features/core/dataStore/projectIOStore.test.ts
pnpm typecheck
```

Expected: PASS.

- [ ] **Step 3: Run focused Rust suites sequentially**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::analysis::projection --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
```

Expected: PASS.

- [ ] **Step 4: Run cross-stack verification**

Run:

```sh
pnpm verify
git --no-pager diff --check
```

Expected: PASS. If `pnpm verify` exposes unrelated pre-existing failures, record exact failures and still run every focused command above.

- [ ] **Step 5: Update the execution ledger accurately**

Record this slice as complete only if graph loading is projection-only. Explicitly leave mutation/history, catalog creation, and execution integration as the next three frontend cuts. Do not claim all of `node-architecture.md` is complete.

- [ ] **Step 6: Review final scope**

Confirm the final diff contains only projection DTO/domain/store/service/load integration, focused tests, the approved spec/plan, and ledger updates. Preserve all unrelated pre-existing working-tree changes and do not commit.
