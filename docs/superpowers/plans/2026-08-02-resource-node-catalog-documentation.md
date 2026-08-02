# Resource Node Catalog and Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the existing Static Catalog into one authoritative resource-aware Catalog for functions, variables, and databases, with exact descriptor creation and localized documentation.

**Architecture:** Rust creates opaque revisioned descriptors from coherent project snapshots and revalidates them through immutable mutation-validation snapshots. Database discovery changes use the existing canonical resource receipt/event and Catalog watermark. React caches, displays, refreshes, and forwards descriptors unchanged while legacy inference paths are removed.

**Tech Stack:** Rust/Tauri ProjectState, Node Catalog and revisioned mutations; React/TypeScript/Zustand/Vitest; shadcn/ui and OverlayScrollbar.

## Global Constraints

- Work directly on `shadcn`; no worktree, branch, or commit.
- Preserve the green 891-test Rust baseline and all unrelated dirty work.
- `database_authority_revisions` remains the only database CAS ledger; do not add `database_revisions`.
- `MutationPublication.resource_revision` is the only public Catalog watermark; `authority_generation` is the internal coherence token.
- Database changes extend the canonical resource receipt/event; do not add a database-specific event family.
- Resource paths are backend-issued opaque identities; frontend code must not parse or synthesize them.
- Unloaded function validation uses a two-stage immutable snapshot; `into_patch` performs no I/O.
- Replace arbitrary create parameters with exact descriptors and do not retain compatibility fields.
- Remove only Catalog/docs/creation/DnD legacy inference; preserve Rust-projected canvas pin compatibility.
- Use explicit focused tests; do not run complete suites by default.
- After each independently reviewed task and controller verification, immediately update `TODO.md` under `## node_architecture 进度`.

---

## File Structure

- Modify `AGENTS.md` to document existing variable UUID resource identity.
- Modify `src-tauri/src/node_system/catalog/localization.rs`, `mod.rs`, and tests for opaque revisioned descriptors and resource localization.
- Modify `src-tauri/src/node_system/document/history.rs`, mutation DTOs, project database state, project activation, and project events for canonical database receipts.
- Modify `src-tauri/src/project/project_reads.rs` for coherent resource snapshots and immutable mutation-validation facts.
- Modify `src-tauri/src/commands/command_node_system.rs` and database commands to publish/query canonical resource state.
- Extend existing frontend Catalog domain/service/store/hook modules; do not create a second store.
- Modify palette/sidebar/DnD/application creation paths and Node documentation modal.
- Remove proven-dead Catalog-specific inference modules/exports only after consumer audits.
- Create `.superpowers/sdd/2026-08-02-resource-node-catalog-documentation/progress.md`.

### Task 1: Freeze opaque resource descriptors and canonical database publication

**Interfaces:**

```rust
pub struct CatalogResourcePath(Box<str>);

NodeCreationDescriptor::ResourceBound {
    node_type_id: NodeTypeId,
    resource_path: CatalogResourcePath,
    resource_revision: ResourceRevision,
    create_args: ResourceBoundCreateArgsDto,
}

enum ResourceBoundCreateArgsDto { Function, Variable, Database }
```

Database commits produce `ResourceKey::Database(DatabaseResourceKey)` deltas through the existing resource mutation receipt and event. Every write accepts caller-issued `projectInstanceId` and `operationId`; create/import uses expected-absent and other writes require exact expected database revision. Data-returning commands return `{ data, mutation }`; void commands return `{ data: null, mutation }`.

- [ ] Add failing serde tests for exact `resourcePath`, `resourceRevision`, and Function/Variable/Database create args; reject missing/extra malformed fields.
- [ ] Add failing database tests for add/import, rename, schema/data identity edit, save, delete, stale/failure, and activation replacement. Assert CAS change, exactly one public resource revision, exactly one matching canonical event, and zero effects on failure.
- [ ] Run RED tests:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::catalog::tests::resource_catalog_serializes_opaque_paths_and_revisions -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_state_database::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib commands::command_dataframe::tests -- --test-threads=1
```

- [ ] Implement `CatalogResourcePath`, descriptor revisions, explicit Database args, `DatabaseResourceKey`, and canonical database delta payloads. Reuse `database_authority_revisions`; allocate publication revision in the same commit critical section.
- [ ] Update activation swap/garbage reconstruction and database commands to accept revisioned caller requests and return aggregate `{ data, mutation }` results. Preserve existing domain payloads under `data`; emit the exact same `mutation` once through `ResourceMutationCommitted`.
- [ ] Update `AGENTS.md` identity rule to include variable resources; state that frontend treats paths as opaque.
- [ ] Run GREEN owner suites, `pnpm rust:check`, `pnpm rust:fmt:check`, and `git diff --check`.
- [ ] Independent review must verify one authority map, one watermark/event path, exact once publication, stale zero-effects, and no UUID migration. Publish Phase 1/2/4/9 progress.

### Task 2: Build coherent project resource and mutation-validation snapshots

**Interfaces:**

```rust
pub struct CatalogProjectSnapshot {
    pub project_instance_id: ProjectInstanceId,
    pub resource_publication_revision: u64,
    pub registry: Arc<NodeRegistry>,
    pub catalog: Arc<BuiltinCatalog>,
    pub resources: Vec<CatalogResourceEntry>,
    authority_generation: u64,
}

pub struct CatalogMutationValidationSnapshot {
    pub project_instance_id: ProjectInstanceId,
    pub authority_generation: u64,
    pub resources: BTreeMap<CatalogResourcePath, CatalogMutationResource>,
}
```

- [ ] Add RED `project_reads` tests for unloaded functions/local variables, loaded overlays, database entries, deterministic ordering, stale identity, ordinary authority change during capture, and project replacement.
- [ ] Add RED validation-snapshot tests proving unloaded function signature revision, variable scope/revision, and database authority are captured under filesystem lease with no later I/O.
- [ ] Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_reads::tests::catalog_snapshot -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_reads::tests::catalog_mutation_validation_snapshot -- --test-threads=1
```

- [ ] Implement both snapshots in `project_reads.rs`: capture session/watermark/generation, acquire root lease, read index and persisted function documents, overlay loaded state, sort identities, release locks/lease, revalidate session and generation.
- [ ] Emit function Call, variable Get/Set, and database source facts. Keep localization outside state/filesystem locks.
- [ ] Run focused reads/activation/filesystem tests and gates.
- [ ] Independent review must inspect lock order, stale retry/rejection, unloaded function authority, deterministic output, and no duplicate owner. Publish Phase 2/3/9 progress.

### Task 3: Localize resource Catalog items and documentation projections

- [ ] Add RED tests for resource display names, docs, stable port/parameter keys, current-locale title/aliases search only, docs/description exclusion, fallback, and shuffled-resource determinism.
- [ ] Extend `LocalizedCatalogItemDto` with focused localized port and parameter DTOs plus optional resource path/revision; do not expose the complete protocol AST.
- [ ] Implement `BuiltinCatalog::localize_with_resources(snapshot, locale)` and make the Catalog command use the coherent snapshot, returning exact project instance, registry fingerprint, and resource publication revision.
- [ ] Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::catalog::tests::resource_catalog -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib commands::command_node_system::tests::localized_catalog -- --test-threads=1
```

- [ ] Run Rust gates and independent review for identity/display separation, search scope, DTO minimality, and metadata coherence. Publish Phase 3/9 progress.

### Task 4: Revalidate descriptors at mutation time

**Target wire shape:**

```rust
EditorGraphMutationDto::CreateNode {
    descriptor: NodeCreationDescriptor,
    position: NodePosition,
    user_label: Option<String>,
}
```

- [ ] Add RED document/project tests for valid function/variable/database materialization, wrong node/resource/args tuple, malformed path, stale revision, missing resource, out-of-scope variable, parameter injection, authority change after snapshot, and zero effects.
- [ ] Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::document::tests::editor_mutation_validation::resource_descriptor -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::resource_descriptor -- --test-threads=1
```

- [ ] Replace `nodeTypeId + parameters` create fields with `descriptor`; remove compatibility fields at Rust/TS boundaries.
- [ ] Capture `CatalogMutationValidationSnapshot` before mutation publication; release filesystem lease; recheck project instance, authority generation, and graph revision inside publication.
- [ ] Materialize only function `target`, variable `variable`, or database `dataframe`, then run normal scope/parameter validation.
- [ ] Map stale/missing to `catalog_resource_stale`; malformed tuple/scope/injection to `catalog_descriptor_invalid`; rejection changes no state/history/event/revision/filesystem.
- [ ] Run document/project mutation suites and Rust gates.
- [ ] Independent review must verify no I/O in `into_patch`, exact tuple validation, atomic gates, and no fallback. Publish Phase 2/3/5 progress.

### Task 5: Route frontend Catalog refresh and all creation paths through descriptors

- [ ] Extend TypeScript descriptor/DTO guards with exact static and resourceBound unions; reject missing/extra fields.
- [ ] Add RED service/store/hook tests for resource items, exact metadata, publication-driven refetch, stale response suppression, locale/project replacement, and failure preservation.
- [ ] Add RED creation/DnD/palette/sidebar tests for unchanged descriptor forwarding and zero fallback synthesis.
- [ ] Implement event-driven Catalog invalidation using existing project resource events and `resourcePublicationRevision`; do not add polling or a second store.
- [ ] Change `NodeSpawnTemplate` to `{ title?: string; descriptor: NodeCreationDescriptor }`; remove function/variable/dataframe override reconstruction.
- [ ] Shift-drop finds an exact current descriptor by opaque path or rejects, refreshes, and toasts.
- [ ] Run:

```sh
pnpm test src/services/nodeSystem/catalogService.test.ts src/features/core/nodeCatalog/nodeCatalogStore.test.ts src/features/application/nodeCatalog/useLocalizedNodeCatalog.test.tsx src/features/application/nodeCatalog/createNodeFromDescriptor.test.ts src/features/application/dataManagement/useNodeManagement.test.tsx src/features/application/editor/canvasDrop/spawnFromTemplate.test.ts src/views/EditorView/Layout/NodePalette.test.tsx
pnpm typecheck
```

- [ ] Independent review must verify views never invoke directly, descriptors stay opaque, selectors are narrow, global listeners use shared utilities, and no synthesis remains. Publish Phase 3/9 progress.

### Task 6: Implement localized Node documentation

- [ ] Add RED modal tests for current-locale title/docs, stable node ID, ports/parameters, resource path/revision, docs-excluded search, locale switch, empty/close behavior, F1, and `OverlayScrollbar` layout.
- [ ] Reuse the existing Markdown renderer and shadcn primitives; read only the current Catalog hook/store.
- [ ] Replace unavailable placeholder copy and update en-US/zh-CN text so search promises title/aliases only.
- [ ] Enable documentation capability only after tests pass.
- [ ] Run:

```sh
pnpm test src/views/EditorView/Layout/NodeDocumentationModal.test.tsx src/features/application/editor/useEditorKeyboard.test.tsx
pnpm typecheck
```

- [ ] Independent review must verify current-locale-only rendering/search, no second Markdown/UI library, and OverlayScrollbar contract. Publish Phase 9 progress.

### Task 7: Remove legacy Catalog inference and add scoped audits

- [ ] Add RED frontend architecture test scanning only Catalog/docs/creation/DnD production paths for forbidden legacy `NodeDefinition` inference, `resolveEffectiveDefinition`, dynamic Call pin generation, contextual builders, all-language docs search, unavailable placeholders, and descriptor synthesis.
- [ ] Audit consumers, then remove only proven-dead Catalog-specific modules/exports. Preserve Rust-projected pin connection compatibility.
- [ ] Add Rust boundary audit ensuring Catalog/document/command paths do not import old graph definitions or placeholder/pin resolvers.
- [ ] Run:

```sh
pnpm test src/services/nodeSystem/nodeCatalogArchitectureContract.test.ts
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::catalog::tests::production_catalog -- --test-threads=1
pnpm typecheck
```

- [ ] Independent review must inspect every deletion, prevent global overreach, and verify zero alternate creation path. Publish Phase 3/9 progress.

### Task 8: Final slice verification

- [ ] Run all explicit frontend Catalog/creation/docs/architecture files from Tasks 5-7 without an extra `--` separator.
- [ ] Run all focused Rust Catalog/project/document/database filters from Tasks 1-4 serially.
- [ ] Run:

```sh
pnpm typecheck
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

- [ ] Dispatch final whole-slice review over all task reports/diffs and ledger; resolve Critical/Important findings.
- [ ] Record exact counts and update TODO. Do not run complete frontend/Rust suites unless explicitly authorized.
