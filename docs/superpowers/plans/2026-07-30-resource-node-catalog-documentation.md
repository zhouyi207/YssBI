# Resource Node Catalog and Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add authoritative function, variable, and database-backed DataFrame descriptors, validate them during graph mutation, deliver localized documentation, and remove legacy frontend Catalog inference.

**Architecture:** ProjectState builds a coherent resource snapshot with opaque backend paths and revisions. Rust localizes discovery/documentation and revalidates descriptors at mutation time. React carries descriptors unchanged through palette/sidebar/drop paths and renders Catalog-projected docs; it never synthesizes resource parameters or pins.

**Tech Stack:** Rust/Tauri ProjectState and Node Catalog, React/TypeScript/Zustand/Vitest, existing graph mutation coordinator.

## Global Constraints

- Slice 4 static Catalog must be complete first; extend it rather than adding another store/service.
- Work on `shadcn`; no worktree or commit.
- Resource paths are backend-issued opaque identities; display names/locales never form identity.
- Do not use `DatabaseDecl.schema_version` as resource revision.
- `resourcePublicationRevision` remains the sole Catalog cache watermark. Every database discovery change must advance it through the existing canonical resource transaction/publication path with a matching event; do not add a second Catalog revision.
- Remove legacy production `NodeDefinition`, Call pin generation, contextual Catalog inference, all-locale docs search, and unavailable placeholders.
- Preserve actual canvas pin compatibility operating on Rust-projected pins.
- Focused explicit tests only; no complete suites.

---

## File Structure

- Modify `node_system/catalog/localization.rs` and `mod.rs`: resource descriptors and docs projections.
- Modify `project/project_reads.rs`, `project_state.rs`, and `project_state_database.rs`: coherent resources and revisions.
- Modify `commands/command_node_system.rs`: resource-aware Catalog command.
- Modify `node_system/document/mutation.rs` and frontend editor mutation DTO: descriptor-based creation.
- Extend Slice 4 frontend Catalog modules.
- Modify palette/sidebar/DnD/application creation paths.
- Modify `NodeDocumentationModal.tsx` and locale files.
- Delete only proven-dead legacy Catalog/definition modules and update exports.

### Task 1: Establish opaque resource identities and revisions

**Interfaces:**

```rust
pub struct CatalogResourcePath(Box<str>);

NodeCreationDescriptor::ResourceBound {
    node_type_id: NodeTypeId,
    resource_path: CatalogResourcePath,
    resource_revision: ResourceRevision,
    create_args: ResourceBoundCreateArgsDto,
}
```

- [ ] **Step 1: Add failing Catalog serialization tests** for function, variable, and database descriptors with exact camelCase paths/revisions.
- [ ] **Step 2: Add failing database revision tests** for add, rename, delete, and project replacement.
- [ ] **Step 3: Run focused tests.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::catalog::tests::resource_catalog_serializes_opaque_paths_and_revisions --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_state_database::tests --test-threads=1
```

- [ ] **Step 4: Implement `CatalogResourcePath`** conventions: function canonical graph path, `variables/{uuid}`, `databases/{id}`.
- [ ] **Step 5: Add `database_revisions` to ProjectState activation/swap/garbage paths.** Route database add, rename, delete, and schema/data identity changes through the existing canonical resource transaction receipt so `MutationPublication.resource_revision` advances with its matching project event.
- [ ] **Step 6: Re-run tests.**

### Task 2: Build coherent authoritative resource snapshots

**Interfaces:**

```rust
pub struct CatalogProjectSnapshot {
    pub project_instance_id: ProjectInstanceId,
    pub resource_publication_revision: u64,
    pub registry: Arc<NodeRegistry>,
    pub catalog: Arc<BuiltinCatalog>,
    pub resources: Vec<CatalogResourceEntry>,
}
```

- [ ] **Step 1: Add failing `project_reads` tests** for unloaded functions/local variables, loaded revision overlays, database runtime entries, deterministic ordering, stale identity, and project replacement coherence.
- [ ] **Step 2: Run the snapshot filter.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_reads::tests::catalog_snapshot --test-threads=1
```

- [ ] **Step 3: Implement `ProjectState::catalog_snapshot` in `project_reads.rs`** using project index filesystem reading plus loaded authoritative overlays. Hold the filesystem lease as required, but release state locks before localization.
- [ ] **Step 4: Emit entries** for function Call, variable Get/Set, and database-backed `yssbi.dataframe.source.get`; sort by stable identity.
- [ ] **Step 5: Validate the project session again before returning** and re-run tests.

### Task 3: Project localized documentation and resource items

- [ ] **Step 1: Add failing tests** for localized resource names/protocol docs, localized port/parameter projections with stable keys, search exclusion of docs/descriptions, and deterministic output under shuffled resources.
- [ ] **Step 2: Extend `LocalizedCatalogItemDto`** with focused localized ports and parameters, not the complete protocol AST.
- [ ] **Step 3: Change the Catalog command** to call `localize_with_resources` on the coherent snapshot and return the exact snapshot metadata.
- [ ] **Step 4: Run Catalog/command filters.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::catalog::tests::resource_catalog --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- commands::command_node_system::tests::localized_catalog --test-threads=1
```

### Task 4: Revalidate descriptors at graph mutation time

**Target create shape:**

```rust
EditorGraphMutationDto::CreateNode {
    descriptor: NodeCreationDescriptor,
    position: NodePosition,
    user_label: Option<String>,
}
```

- [ ] **Step 1: Add failing document/application tests** for valid function/variable/database parameter materialization and wrong tuple, malformed path, stale revision, missing resource, out-of-scope variable, parameter injection, and zero effects on rejection.
- [ ] **Step 2: Run focused tests.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::document::tests::editor_mutation_validation::resource_descriptor --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::resource_descriptor --test-threads=1
```

- [ ] **Step 3: Replace arbitrary create parameters with the descriptor** at the Rust/TypeScript wire boundary; do not retain a compatibility field.
- [ ] **Step 4: Capture immutable mutation validation resources in ProjectState**; perform no disk I/O inside `into_patch`.
- [ ] **Step 5: Materialize only allowed bindings:** function `target`, variable `variable`, database source `dataframe`. Re-run normal scope/parameter validation afterward.
- [ ] **Step 6: Map stale/missing revision to `catalog_resource_stale` and malformed tuples to `catalog_descriptor_invalid`; preserve revision/history/events on rejection.**
- [ ] **Step 7: Re-run tests.**

### Task 5: Route every supported frontend creation path through descriptors

- [ ] **Step 1: Update frontend tests** for static/function/variable/database forwarding, exact revisions, stale refresh, and no fallback synthesis.
- [ ] **Step 2: Change `createNodeFromDescriptor` and mutation DTOs** to forward descriptors unchanged.
- [ ] **Step 3: Replace DnD `NodeSpawnTemplate`** with `{ title?, descriptor }`; remove variable/function/dataframe override reconstruction.
- [ ] **Step 4: Route palette, sidebar, and canvas drop** through exact descriptors. Shift-drop looks up the current descriptor by opaque resource path and rejects the drop while refreshing the Catalog when no exact current descriptor exists; it never synthesizes one.
- [ ] **Step 5: Keep duplicate/paste disabled** and run explicit tests.

```sh
pnpm test -- src/features/application/nodeCatalog/createNodeFromDescriptor.test.ts src/features/application/dataManagement/useNodeManagement.test.tsx src/features/application/editor/canvasDrop/spawnFromTemplate.test.ts src/views/EditorView/Layout/NodePalette.test.tsx
```

### Task 6: Implement localized Node documentation

- [ ] **Step 1: Add `NodeDocumentationModal.test.tsx`** for current-locale title/docs, ports/parameters/stable IDs, resource path/revision, docs-excluded search, locale switch, empty state, close, and `OverlayScrollbar`.
- [ ] **Step 2: Implement the modal** from the current Catalog hook/store only. Reuse/extract the existing Markdown renderer; add no UI library.
- [ ] **Step 3: Update en-US/zh-CN copy** so it does not promise all-language/documentation-body search.
- [ ] **Step 4: Enable only documentation capability** after tests pass and retain F1 behavior.

```sh
pnpm test -- src/views/EditorView/Layout/NodeDocumentationModal.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx
```

### Task 7: Remove legacy production paths and add audits

- [ ] **Step 1: Add a failing frontend architecture test** scanning production source for forbidden Catalog `NodeDefinition`, `resolveEffectiveDefinition`, dynamic Call pin generation, contextual builders, all-language docs search, unavailable messages, and descriptor synthesis.
- [ ] **Step 2: Remove dead Catalog-specific modules and exports** only after checking consumers. Keep actual Rust-projected pin connection compatibility.
- [ ] **Step 3: Remove node-definition detail placeholder** in favor of `NodeDocumentationModal`.
- [ ] **Step 4: Add a Rust boundary audit** ensuring new Catalog/document/command paths do not import old `crate::graph::NodeDefinition`, placeholder/pin resolver, or legacy inference.
- [ ] **Step 5: Run architecture tests.**

```sh
pnpm test -- src/services/nodeSystem/nodeCatalogArchitectureContract.test.ts
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::catalog::tests::production_catalog --test-threads=1
```

### Task 8: Slice verification

```sh
pnpm test -- src/services/nodeSystem/catalogService.test.ts src/features/core/nodeCatalog/nodeCatalogStore.test.ts src/features/core/nodeCatalog/localizedSearchIndex.test.ts src/features/application/nodeCatalog/createNodeFromDescriptor.test.ts src/features/application/dataManagement/useNodeManagement.test.tsx src/features/application/editor/canvasDrop/spawnFromTemplate.test.ts src/views/EditorView/Layout/NodeDocumentationModal.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx src/services/nodeSystem/nodeCatalogArchitectureContract.test.ts
pnpm typecheck
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Do not run complete suites.