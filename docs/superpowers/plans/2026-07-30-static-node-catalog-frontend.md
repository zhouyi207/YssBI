# Static Node Catalog Frontend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver a project-aware localized static node palette whose Rust-issued descriptor creates a node through the existing revisioned graph mutation path.

**Architecture:** Rust snapshots Catalog identity and localized items. A frontend service owns IPC, pure domain code owns search, a metadata-keyed Zustand store caches responses, and application adapters own loading and creation. Views compose these APIs and never call Tauri or reconstruct Node Protocol semantics.

**Tech Stack:** Rust/Tauri, React, TypeScript, Zustand, Vitest, shadcn/ui, OverlayScrollbar.

## Global Constraints

- Work on `shadcn`; no worktree or commit.
- Rust remains authoritative for project identity, Registry, defaults, scope validation, node/port IDs, and dynamic interfaces.
- No `get_editor_schema_command`, frontend Registry/schema/type store, `NodeDefinition`, direct view `invoke`, or contextual compatibility.
- Slice 4 enables static descriptors only; resource-bound descriptors and documentation remain disabled.
- Frontend tests use explicit file lists; no unqualified `pnpm test` or `pnpm verify`.
- Rust tests are focused and serial; no complete Rust suite.

---

## File Structure

- Modify Rust Catalog DTO/command/project snapshot files.
- Create `src/services/nodeSystem/catalogService.ts` and test.
- Create focused domain files under `src/features/domain/nodeCatalog/`.
- Create store/index/selectors under `src/features/core/nodeCatalog/`.
- Create application hook/adapter under `src/features/application/nodeCatalog/`.
- Modify `NodePalette.tsx`, `CanvasOverlays.tsx`, overlay handler, and capability flags.

### Task 1: Add coherent project-aware Catalog IPC

**Target response:**

```ts
interface LocalizedCatalogDto {
  projectInstanceId: string;
  registryFingerprint: string;
  resourcePublicationRevision: number;
  locale: string;
  categories: LocalizedCategoryDto[];
  items: LocalizedCatalogItemDto[];
}
```

- [ ] **Step 1: Add failing Rust tests** for stale project rejection, coherent metadata, camelCase serialization, no resource items, and static eligibility.
- [ ] **Step 2: Run focused tests.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- commands::command_node_system::tests::localized_catalog --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::catalog::tests::static_catalog_excludes_managed_and_resource_required_descriptors --exact --test-threads=1
```

- [ ] **Step 3: Extend `LocalizedCatalogDto`** with project instance, Registry fingerprint canonical string, and publication revision.
- [ ] **Step 4: Add a ProjectState coherent Catalog snapshot** that validates `ProjectInstanceId`, captures registry/catalog Arcs and metadata from one activation, releases locks, localizes, then revalidates lifecycle.
- [ ] **Step 5: Change `get_localized_node_catalog`** to accept `project_instance_id` and map stale identity to `catalog_project_stale`.
- [ ] **Step 6: Filter static descriptor eligibility in Rust**: exclude managed nodes and nodes with required parameters lacking defaults. React must not duplicate this rule.
- [ ] **Step 7: Re-run focused Rust tests.**

### Task 2: Add service DTOs and pure localized search

**Files:** create service/domain files and focused tests listed in the design.

- [ ] **Step 1: Write `catalogService.test.ts`** asserting exact invoke command and camelCase `{ projectInstanceId, locale }` arguments.
- [ ] **Step 2: Write pure search tests** asserting matches for title, aliases, technical terms, stable ID/backend `searchText`, and pinyin; assert description/documentation are excluded.
- [ ] **Step 3: Run the two tests and observe missing modules.**

```sh
pnpm test -- src/services/nodeSystem/catalogService.test.ts src/features/domain/nodeCatalog/search.test.ts
```

- [ ] **Step 4: Implement DTOs and `CatalogService.getLocalizedCatalog`**. Only this service imports `invoke`.
- [ ] **Step 5: Implement pure search/domain guards** with no React, Zustand, services, `NodeDefinition`, or compatibility imports.
- [ ] **Step 6: Re-run the exact tests.**

### Task 3: Add exact-metadata Catalog cache and lifecycle hook

- [ ] **Step 1: Add store/index tests** for isolation by project ID, locale, Registry fingerprint, and resource publication revision.
- [ ] **Step 2: Add hook tests** for initial load, locale change, cache reuse, stale project response suppression, and error state.
- [ ] **Step 3: Run exact files.**

```sh
pnpm test -- src/features/core/nodeCatalog/localizedSearchIndex.test.ts src/features/core/nodeCatalog/nodeCatalogStore.test.ts src/features/application/nodeCatalog/useLocalizedNodeCatalog.test.tsx
```

- [ ] **Step 4: Implement a workflow-free Zustand cache**; it stores response/status only and imports no service.
- [ ] **Step 5: Implement `useLocalizedNodeCatalog`** using narrow project selectors and `captureProjectIdentity`/`isCurrentProjectIdentity`. Store a response only if returned and current project identities match.
- [ ] **Step 6: Re-run exact tests.**

### Task 4: Route static creation through the revisioned mutation coordinator

**Mutation payload:**

```ts
{
  type: 'createNode',
  payload: {
    nodeTypeId: descriptor.nodeTypeId,
    position,
    parameters: {},
    userLabel: null,
  },
}
```

- [ ] **Step 1: Add adapter tests** asserting no ports, IDs, inferred types, dynamic interfaces, or arbitrary parameters are sent; resource-bound descriptors are rejected.
- [ ] **Step 2: Run the test and observe missing adapter.**

```sh
pnpm test -- src/features/application/nodeCatalog/createNodeFromDescriptor.test.ts
```

- [ ] **Step 3: Implement `createNodeFromDescriptor`** on top of `executeEditorMutation`; let it own graph revision and operation ID through the existing coordinator.
- [ ] **Step 4: Re-run the adapter test** and existing mutation contract test.

```sh
pnpm test -- src/features/application/nodeCatalog/createNodeFromDescriptor.test.ts src/features/application/editorMutation/editorMutation.test.ts
```

### Task 5: Replace the palette placeholder and enable only static capability

- [ ] **Step 1: Add `NodePalette.test.tsx`** for loading, error, localized category/item rendering, search, empty state, and selection.
- [ ] **Step 2: Update routing tests** so static palette selection calls descriptor creation while resource/sidebar legacy routes remain unavailable.
- [ ] **Step 3: Implement `NodePalette`** with application hooks, shadcn controls, and `OverlayScrollbar`; no service/Tauri imports.
- [ ] **Step 4: Update overlay composition/handler** to convert the selected descriptor and canvas position into the application action.
- [ ] **Step 5: Set capability flags exactly:** static create and Catalog descriptors true; resource-bound, contextual, documentation, duplicate, and paste false.
- [ ] **Step 6: Run exact UI files.**

```sh
pnpm test -- src/views/EditorView/Layout/NodePalette.test.tsx src/features/application/editor/editorUnavailableRouting.test.tsx
```

### Task 6: Contract and slice verification

- [ ] **Step 1: Add a source-contract test** proving the production palette chain does not import `NodeDefinition`, legacy builders, compatibility inference, or direct `invoke`.
- [ ] **Step 2: Run all Slice 4 frontend files explicitly.**

```sh
pnpm test -- src/services/nodeSystem/catalogService.test.ts src/features/domain/nodeCatalog/search.test.ts src/features/core/nodeCatalog/localizedSearchIndex.test.ts src/features/core/nodeCatalog/nodeCatalogStore.test.ts src/features/application/nodeCatalog/createNodeFromDescriptor.test.ts src/features/application/nodeCatalog/useLocalizedNodeCatalog.test.tsx src/views/EditorView/Layout/NodePalette.test.tsx src/features/application/editor/editorUnavailableRouting.test.tsx src/features/application/editorMutation/editorMutation.test.ts
pnpm typecheck
```

- [ ] **Step 3: Run Rust/check gates.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Do not run complete Rust or frontend suites.