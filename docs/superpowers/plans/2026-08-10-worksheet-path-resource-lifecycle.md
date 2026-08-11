# Worksheet Path Resource Lifecycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace UUID worksheet identity with readable canonical worksheet paths and route worksheet create, duplicate, save, rename, remove, history, publication, and recovery through the same revisioned resource lifecycle used by graphs.

**Architecture:** Rust owns resource-name validation, canonical paths, revisions, filesystem transactions, history, and display-name projection. Worksheet mutations use `ResourceMutationResultDto.deltas` and `.moves`; the worksheet-specific delta side channel is deleted. React stores and tabs use opaque `worksheetPath` keys and install only Rust-authoritative DTOs.

**Tech Stack:** Rust, Serde, Tauri 2, TypeScript, React 19, Zustand, Vitest, Cargo tests invoked through `pnpm`.

## Global Constraints

- The approved design is `docs/superpowers/specs/2026-08-10-worksheet-path-resource-lifecycle-design.md`.
- Canonical worksheet identity is exactly `worksheets/{validated-name}.yssbi-worksheet`.
- `worksheetPath` is the sole production identity; do not retain `worksheetId`, UUID aliases, fallback scanners, migration shims, or dual wire formats.
- Frontend code treats all resource paths as opaque and never derives names from paths.
- Rust returns display names in index, lifecycle, and move DTOs.
- Event, function, and worksheet names use one Rust `ResourceName` validator.
- Allowed name characters are Unicode letters/numbers, ASCII space, `-`, `_`, `(`, and `)`; enforce all rejection and NFC rules from the spec.
- All platforms use portable NFC-aware case-insensitive uniqueness.
- Create and duplicate allocate `Name`, `Name 2`, `Name 3`; rename rejects a conflicting target without silently suffixing it.
- Worksheet documents persist no `id` and no `name`.
- Remove `WorksheetDeltaDto` and `worksheetDeltas` directly; do not add compatibility branches.
- Tests and golden fixture names must not contain the application software version. `schemaVersion` remains independent persistence metadata.
- Preserve filesystem redirect, rollback, recovery, project identity, revision conflict, and ABA/tombstone protections.
- Do not parse or construct resource paths in views or application hooks; IPC invokes remain in `src/services/`.
- Keep existing large-file structure unless a new focused boundary is explicitly named below.
- Run Rust commands from the repository root through `pnpm`; do not invoke ad-hoc Cargo or create `src-tauri/target/`.
- Preserve unrelated dirty-tree changes. Do not stage or commit unless the user explicitly asks during execution.

---

### Task 1: Shared Resource Name and Worksheet Path Types

**Files:**
- Create: `src-tauri/src/project/resource_name.rs`
- Create: `src-tauri/src/project/worksheet_resource_path.rs`
- Modify: `src-tauri/src/project/graph_resource_path.rs`
- Modify: `src-tauri/src/project/project_error.rs`
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/Cargo.toml`
- Modify: `Cargo.lock`
- Test: `src-tauri/src/project/resource_name.rs`
- Test: `src-tauri/src/project/worksheet_resource_path.rs`
- Test: `src-tauri/src/project/graph_resource_path.rs`

**Interfaces:**
- Produces:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResourceNameError {
    Empty,
    NotNfc,
    ForbiddenCharacter(char),
    InvalidSpacing,
    Reserved,
    TooLong,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResourceName(String);

impl ResourceName {
    pub fn parse(input: &str) -> Result<Self, ResourceNameError>;
    pub fn as_str(&self) -> &str;
    pub fn portable_key(&self) -> String;
}

pub fn allocate_unique_resource_name<'a>(
    base: &ResourceName,
    existing: impl IntoIterator<Item = &'a ResourceName>,
) -> ResourceName;
```

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorksheetResourcePathError {
    WrongDirectory,
    Nested,
    WrongExtension,
    InvalidName(ResourceNameError),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct WorksheetResourcePath(String);

impl WorksheetResourcePath {
    pub fn parse(value: &str) -> Result<Self, WorksheetResourcePathError>;
    pub fn from_name(name: &ResourceName) -> Self;
    pub fn as_str(&self) -> &str;
    pub fn display_name(&self) -> &ResourceName;
    pub fn relative_path(&self) -> &Path;
}
```

- `GraphResourcePath::event(&ResourceName)` and `GraphResourcePath::function(&ResourceName)` become the only unchecked-name-free construction paths for new resources.
- Parsing persisted paths validates their filename stem with `ResourceName`; it must not sanitize it.
- `ProjectFilesystemError` receives structured resource-name/path variants whose IPC codes match the spec.

- [ ] **Step 1: Add RED name-validation tests**

Add exact tests named:

```rust
resource_name_accepts_portable_unicode_names
resource_name_rejects_forbidden_characters_and_controls
resource_name_rejects_spacing_punctuation_and_emoji
resource_name_rejects_non_nfc_reserved_and_overlong_names
resource_name_portable_key_is_case_insensitive
unique_resource_name_uses_first_free_numeric_suffix
```

Use semantic fixtures such as `销售分析 2`, `Report_2026`, and `Revenue (Net)`; do not include application version strings.

- [ ] **Step 2: Run the RED tests**

Run:

```sh
pnpm rust:test resource_name_
```

Expected: FAIL because `ResourceName` and its validation/allocation API do not exist.

- [ ] **Step 3: Implement `ResourceName` and structured errors**

Add `unicode-normalization = "0.1.24"` and `unicode-casefold = "0.2.0"` to `src-tauri/Cargo.toml`, updating the root `Cargo.lock` through the normal `pnpm rust:*` command path. Use them for NFC validation and full Unicode case folding. Reject input whose NFC form differs rather than silently replacing it. Enforce the approved allowed character set, 80-character limit, reserved-name set, and portable key.

Do not use `trim`, filename sanitization, or replacement characters in the validator.

- [ ] **Step 4: Add RED path tests**

Add:

```rust
worksheet_path_round_trips_canonical_resource_identity
worksheet_path_rejects_nested_wrong_extension_and_invalid_stem
all_resource_path_kinds_use_shared_name_validation
graph_path_parsing_does_not_sanitize_invalid_names
```

Assert that the canonical worksheet path is exactly:

```text
worksheets/Sales Report.yssbi-worksheet
```

- [ ] **Step 5: Run the path tests RED**

Run:

```sh
pnpm rust:test worksheet_resource_path
pnpm rust:test all_resource_path_kinds_use_shared_name_validation
```

Expected: FAIL because `WorksheetResourcePath` and graph constructors are absent.

- [ ] **Step 6: Implement strict path construction and parsing**

Implement the interfaces above. Keep path parsing inside Rust. Convert name/path errors to stable structured `ProjectFilesystemError` details; do not change frontend code in this task.

- [ ] **Step 7: Run focused GREEN checks**

Run:

```sh
pnpm rust:test resource_name_
pnpm rust:test worksheet_resource_path
pnpm rust:test graph_resource_path
pnpm rust:fmt:check
pnpm rust:check
```

Expected: all focused tests pass; fmt/check exit 0 with no new warning attributable to this task.

- [ ] **Step 8: Review checkpoint**

Confirm no production code outside the new path constructors still calls a sanitizer for a newly supplied resource name. Do not stage or commit.

---

### Task 2: Path-Keyed Worksheet Persistence and Project Index

**Files:**
- Modify: `src-tauri/src/project/worksheet_io.rs`
- Modify: `src-tauri/src/project/project_data.rs`
- Modify: `src-tauri/src/project/project_io.rs`
- Modify: `src-tauri/src/project/project_reads.rs`
- Modify: `src-tauri/src/project/project_activation.rs`
- Modify: `src-tauri/src/project/project_lifecycle.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Modify: `src-tauri/src/project/project_writers.rs`
- Modify: `src-tauri/src/project/resource_patch.rs`
- Modify: `src-tauri/src/project/production_tests.rs`
- Modify: `src-tauri/src/commands/command_worksheet.rs`
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/src/project/graph_resource_index.rs`
- Test: `src-tauri/src/project/worksheet_io.rs`
- Test: `src-tauri/src/project/project_reads.rs`
- Test: `src-tauri/src/project/project_activation.rs`
- Test: `src-tauri/src/project/project_lifecycle.rs`
- Test: `src-tauri/src/project/project_writers.rs`
- Test: `src-tauri/src/project/production_tests.rs`
- Test: `src-tauri/src/commands/command_worksheet.rs`

**Interfaces:**
- Consumes: `ResourceName`, `WorksheetResourcePath`, and portable uniqueness from Task 1.
- Produces:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorksheetDocument {
    pub schema_version: u32,
    pub revision: ResourceRevision,
    pub database_id: String,
    pub chart_type: String,
    pub encodings: WorksheetEncodings,
}
```

```rust
pub struct ProjectWorksheetIndexEntry {
    pub worksheet_path: WorksheetResourcePath,
    pub name: String,
    pub database_id: String,
    pub chart_type: String,
    pub revision: ResourceRevision,
}
```

```rust
pub fn serialize_worksheet(
    path: &WorksheetResourcePath,
    document: &WorksheetDocument,
) -> Result<(PathBuf, Vec<u8>), ProjectError>;

pub fn load_worksheet_from_file(
    root: &Path,
    worksheet_path: &WorksheetResourcePath,
) -> Result<WorksheetDocument, ProjectError>;
```

- `ProjectData.worksheets` becomes `HashMap<WorksheetResourcePath, WorksheetDocument>`.
- Worksheet revision ledgers use `WorksheetResourcePath`, not `String` IDs.
- Until Task 4 removes the old command wrapper, its compile-safe transitional shape is explicit and path-only:

```rust
pub struct WorksheetMutationResultDto {
    pub operation_id: OperationId,
    pub result: ResourceMutationResultDto,
    pub worksheet_path: WorksheetResourcePath,
    pub document: WorksheetDocument,
}
```

This is not a UUID compatibility path and must be deleted, not aliased, in Task 4.

- [ ] **Step 1: Write RED canonical persistence tests**

Replace the UUID-based positive test with:

```rust
canonical_name_path_is_shared_by_activation_index_and_direct_load
```

Add:

```rust
worksheet_document_rejects_legacy_identity_fields
worksheet_activation_rejects_uuid_filename
worksheet_activation_rejects_noncanonical_and_casefold_duplicate_paths
worksheet_index_derives_name_from_path_and_includes_revision
```

Keep existing nested-file and redirect/junction rejection tests.

- [ ] **Step 2: Run RED persistence tests**

Run:

```sh
pnpm rust:test worksheet_io::tests
```

Expected: new tests fail because persistence is still UUID/document-name based and Serde accepts unknown legacy fields.

- [ ] **Step 3: Convert worksheet persistence to canonical path identity**

Delete persisted `id` and `name`; add `deny_unknown_fields`. Make the scanner validate each filename as a `WorksheetResourcePath`, reject UUID/noncanonical/nested/case-fold duplicates, and return path/document pairs. Do not add a legacy scan branch.

The fixture writer must require the path explicitly:

```rust
pub(crate) fn write_worksheet(
    root: &Path,
    path: &WorksheetResourcePath,
    document: &WorksheetDocument,
) -> Result<(), ProjectError>;
```

- [ ] **Step 4: Convert ProjectData, activation, save-as, and reads**

Update all worksheet maps, authority patches, writer helpers, command adapters, test fixtures, and revision maps to path keys so the Rust crate remains compilable at this checkpoint. Existing mutation wrappers may continue only until Task 4, but they must carry `worksheet_path` rather than reconstruct identity from the document. Change authoritative reads to:

```rust
pub fn load_worksheet_document(
    &self,
    expected_project_instance_id: &ProjectInstanceId,
    worksheet_path: &WorksheetResourcePath,
) -> Result<WorksheetDocument, ProjectFilesystemError>;
```

Project index projection derives `name` in Rust and includes `revision`. Save-as/copy writes `(path, document)` pairs and validates destination canonical paths.

- [ ] **Step 5: Tighten graph discovery to the shared validator**

Update `graph_resource_index.rs` so persisted event/function stems use `ResourceName` and portable case-fold collision checks. Preserve existing redirect behavior. Do not change graph runtime loading order.

- [ ] **Step 6: Run focused GREEN tests**

Run:

```sh
pnpm rust:test worksheet_io::tests
pnpm rust:test worksheet_load_reads_current_authority_without_disk_fallback
pnpm rust:test project_index_overlays_functions_and_globals_from_one_authoritative_snapshot
pnpm rust:test activation_replaces_old_session_revision_tombstones
pnpm rust:test save_as_builds_destination_from_one_authoritative_snapshot
pnpm rust:test graph_resource_index
pnpm rust:fmt:check
pnpm rust:check
```

Expected: canonical path tests pass; old UUID fixtures have been replaced or are negative rejection fixtures only.

- [ ] **Step 7: Source audit checkpoint**

Search production Rust for `worksheet.id`, `worksheet_id`, and UUID-based worksheet filename formatting. Every remaining hit must be either an explicitly negative test or unrelated prose. Do not add aliases.

---

### Task 3: General Resource Lifecycle, Worksheet Delta, and Strict Wire Contract

**Files:**
- Modify: `src-tauri/src/node_system/document/history.rs`
- Modify: `src-tauri/src/node_system/document/mod.rs`
- Modify: `src-tauri/src/event/event_project.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Modify: `src-tauri/src/project/project_state_database.rs`
- Modify: `src-tauri/src/project/project_writers.rs`
- Modify: `src/features/application/editorMutation/resourceMutationResult.ts`
- Modify: `src/features/application/editorMutation/projectPublicationCoordinator.ts`
- Modify: `src/shared/types/domain/worksheet.ts`
- Modify: `src/shared/types/dto/editorMutation.ts`
- Modify: `src/features/core/sync/utils/resourceMutationWireValidator.ts`
- Modify: `src/features/core/sync/utils/resourceMutationResultWireParser.ts`
- Modify: `src/features/core/sync/utils/projectEventWireParser.ts`
- Test: `src-tauri/src/event/event_project.rs`
- Test: `src-tauri/src/node_system/document/tests.rs`
- Test: `src/features/core/sync/utils/resourceMutationWireValidator.test.ts`
- Test: `src/features/core/sync/utils/projectEventWireParser.test.ts`
- Test: `src/features/application/editorMutation/editorMutation.test.ts`

**Interfaces:**
- Consumes: path-keyed `WorksheetDocument` from Task 2.
- Produces:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceLifecycleKind { Event, Function, Worksheet }

pub struct ResourceLifecycleState {
    pub revision: ResourceRevision,
    pub path: Box<str>,
    pub kind: ResourceLifecycleKind,
    pub name: String,
}

pub struct WorksheetDocumentState {
    pub database_id: String,
    pub chart_type: String,
    pub encodings: WorksheetEncodings,
}

pub struct WorksheetDocumentPatch {
    pub before: WorksheetDocumentState,
    pub after: WorksheetDocumentState,
}
```

```rust
pub enum ResourceDocumentPatch {
    Graph(GraphDocumentPatch),
    Function(FunctionDocumentPatch),
    Worksheet(WorksheetDocumentPatch),
    ResourceLifecycle(ResourceLifecyclePatch),
    ResourceMove(ResourcePathMovePatch),
    Variable(VariableDocumentPatch),
    VariableScopeMove(ResourcePathMovePatch),
    Database(DatabaseDocumentPatch),
}
```

- `ResourceMutationResultDto` has no `worksheet_deltas` field.
- Rust `ResourceMoveDto.kind` is `ResourceLifecycleKind`, serialized as `event`, `function`, or `worksheet`.
- TypeScript `ResourceMoveDto.kind` is `'event' | 'function' | 'worksheet'`.
- TypeScript validators check exact DTO structure and non-empty opaque path strings; they do not duplicate Rust filename validation.

- [ ] **Step 1: Write RED Rust wire tests**

Add exact tests:

```rust
worksheet_document_delta_uses_common_resource_delta_wire
worksheet_lifecycle_delta_carries_rust_derived_name
worksheet_move_uses_common_resource_move_wire
resource_mutation_result_has_no_worksheet_side_channel
```

Update graph lifecycle wire expectations to the generalized `resource_lifecycle` and `resource_move` variants and required `name`.

- [ ] **Step 2: Run Rust wire tests RED**

Run:

```sh
pnpm rust:test event_project::tests::worksheet_
pnpm rust:test resource_lifecycle_delta_serializes_explicit_optional_states
```

Expected: FAIL on missing generalized variants and obsolete `worksheet_deltas`.

- [ ] **Step 3: Implement Rust protocol generalization**

Replace graph-only lifecycle/move variants directly. Add worksheet patch inversion and `ResourceDocumentPatch::kind() == ResourceKind::Worksheet`. Remove all aggregate initializers of `worksheet_deltas`, including database mutation publication.

Do not yet add full worksheet history routing; Task 6 supplies durable application. This task freezes the serializable contract.

- [ ] **Step 4: Write RED TypeScript strict-parser tests**

Add canonical worksheet lifecycle/document/move envelopes and explicit negative inputs containing:

```json
{ "worksheetDeltas": [] }
{ "id": "legacy-id" }
{ "name": "persisted document name" }
```

The latter two are invalid inside a worksheet document, while `name` remains required in lifecycle/index/move DTOs.

- [ ] **Step 5: Run TypeScript parser tests RED**

Run:

```sh
pnpm test -- src/features/core/sync/utils/resourceMutationWireValidator.test.ts src/features/core/sync/utils/projectEventWireParser.test.ts src/features/application/editorMutation/editorMutation.test.ts
```

Expected: FAIL because parsers still require `worksheetDeltas` and graph-only variant tags.

- [ ] **Step 6: Implement TypeScript DTOs and parsers**

Delete `WorksheetDeltaDto`. Parse identity-free worksheet documents only under `ResourceKey.kind === 'worksheet'`. Require exact allowed keys and reject unknown legacy fields. Accept opaque worksheet paths as non-empty strings without parsing their syntax. Update current worksheet writers to emit common resource deltas, and update publication preparation to consume those deltas immediately so `pnpm typecheck` passes at this checkpoint; do not retain an internal adapter that recreates `worksheetDeltas`.

- [ ] **Step 7: Run focused GREEN checks**

Run:

```sh
pnpm rust:test event_project::tests
pnpm rust:test history_persistence_policies_round_trip
pnpm test -- src/features/core/sync/utils/resourceMutationWireValidator.test.ts src/features/core/sync/utils/projectEventWireParser.test.ts src/features/application/editorMutation/editorMutation.test.ts
pnpm typecheck
pnpm rust:fmt:check
pnpm rust:check
```

Expected: Rust and TypeScript agree on the common variants; no production DTO exposes `worksheetDeltas`.

---

### Task 4: Authoritative Worksheet Create, Duplicate, Save, and Remove

**Files:**
- Modify: `src-tauri/src/project/resource_mutations.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Modify: `src-tauri/src/project/project_writers.rs`
- Modify: `src-tauri/src/project/resource_patch.rs`
- Modify: `src-tauri/src/commands/command_worksheet.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src/services/worksheet/worksheetService.ts`
- Modify: `src/features/core/worksheet/worksheetStore.ts`
- Test: `src-tauri/src/project/project_writers.rs`
- Test: `src-tauri/src/project/production_tests.rs`
- Test: `src-tauri/src/commands/command_worksheet.rs`
- Test: `src-tauri/src/project/filesystem/source_audit_tests.rs`
- Test: `src/services/worksheet/worksheetService.test.ts`
- Test: `src/features/core/worksheet/worksheetStore.test.ts`

**Interfaces:**
- Consumes: Task 3 common mutation aggregate.
- Produces Rust transactions:

```rust
pub fn create_worksheet_resource_transaction(
    &self,
    project_instance_id: &ProjectInstanceId,
    name: &ResourceName,
    database_id: Option<String>,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, ProjectFilesystemError>;

pub fn duplicate_worksheet_resource_transaction(
    &self,
    project_instance_id: &ProjectInstanceId,
    source: &WorksheetResourcePath,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, ProjectFilesystemError>;

pub fn save_worksheet_document(
    &self,
    project_instance_id: &ProjectInstanceId,
    worksheet_path: &WorksheetResourcePath,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    document: WorksheetDocument,
) -> Result<ResourceMutationResultDto, ProjectFilesystemError>;

pub fn remove_worksheet_resource_transaction(
    &self,
    project_instance_id: &ProjectInstanceId,
    worksheet_path: &WorksheetResourcePath,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, ProjectFilesystemError>;
```

- Commands are `create_worksheet`, `duplicate_worksheet`, `save_worksheet`, and `remove_worksheet`; delete the old `delete_worksheet` registration.
- Services return strictly parsed `ResourceMutationResultDto`.

- [ ] **Step 1: Add RED transaction tests**

Add:

```rust
worksheet_create_publishes_resource_lifecycle_delta
worksheet_duplicate_allocates_first_free_authoritative_path
worksheet_save_publishes_document_delta
worksheet_remove_publishes_resource_lifecycle_delta
worksheet_delete_recreate_preserves_tombstone_revision
worksheet_mutation_failures_have_zero_authoritative_effects
```

Create and duplicate expectations are `Report`, then `Report 2`, then `Report 3`. Remove must retain a revision tombstone so same-path recreate advances generation.

- [ ] **Step 2: Run transaction tests RED**

Run:

```sh
pnpm rust:test worksheet_create_publishes_resource_lifecycle_delta
pnpm rust:test worksheet_duplicate_allocates_first_free_authoritative_path
pnpm rust:test worksheet_save_publishes_document_delta
pnpm rust:test worksheet_remove_publishes_resource_lifecycle_delta
pnpm rust:test worksheet_delete_recreate_preserves_tombstone_revision
```

Expected: FAIL because current writers still return `WorksheetMutationResultDto`, lack the four authoritative transaction methods, and discard remove revisions.

- [ ] **Step 3: Implement transaction methods and remove legacy writer paths**

Use `reserve_resource_operation`, project root lease, expected revision validation, filesystem prepare/commit, authority patch, rollback/recovery, and exactly one common mutation result. Keep worksheet revision tombstones after removal. Save accepts no name and cannot rename.

Remove old uncorrelated `upsert_worksheet_document`, `remove_worksheet_document`, and `WorksheetMutationResultDto` production paths rather than wrapping them.

- [ ] **Step 4: Add RED command and service tests**

Assert exact invoke/request fields for all four commands. Assert create requires a name string, database ID is optional, save/remove require expected revision, and every direct result passes through the strict parser.

- [ ] **Step 5: Run command/service tests RED**

Run:

```sh
pnpm rust:test command_worksheet::tests
pnpm test -- src/services/worksheet/worksheetService.test.ts src/features/core/worksheet/worksheetStore.test.ts
```

Expected: FAIL on old command names and result wrappers.

- [ ] **Step 6: Implement thin commands, services, and path-keyed store save**

The store API becomes explicit:

```ts
upsertDocument(worksheetPath: string, document: WorksheetDocument): void
removeDocument(worksheetPath: string): void
updateDocument(worksheetPath: string, patch: Partial<WorksheetDocument>): WorksheetDocument | null
saveDocument(worksheetPath: string): Promise<boolean>
```

The store must not synthesize index entries from documents because documents no longer contain names. Save settles only through the common publication result and preserves newer dirty edits.

- [ ] **Step 7: Run focused GREEN checks**

Run:

```sh
pnpm rust:test command_worksheet::tests
pnpm rust:test worksheet_mutation
pnpm rust:test worksheet_delete_recreate_preserves_tombstone_revision
pnpm rust:test production_project_document_io_is_owned_by_filesystem_modules
pnpm test -- src/services/worksheet/worksheetService.test.ts src/features/core/worksheet/worksheetStore.test.ts
pnpm typecheck
pnpm rust:fmt:check
pnpm rust:check
```

Expected: create/duplicate/save/remove use canonical paths and common deltas; no old command remains registered.

---

### Task 5: Worksheet Rename, Shared Lifecycle Ownership, and Case-Only Filesystem Move

**Files:**
- Rename: `src-tauri/src/project/graph_lifecycle.rs` to `src-tauri/src/project/resource_lifecycle.rs`
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Modify: `src-tauri/src/project/resource_mutations.rs`
- Modify: `src-tauri/src/project/filesystem/transaction.rs`
- Modify: `src-tauri/src/project/filesystem/tests.rs`
- Modify: `src-tauri/src/commands/command_worksheet.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src/features/application/editor/worksheetLifecycleCoordinator.ts`
- Modify: `src/services/worksheet/worksheetService.ts`
- Modify: `src/features/application/resource/resourceActions.ts`
- Modify: `src/features/application/sidebar/sidebarResourceActions.ts`
- Test: `src-tauri/src/project/resource_lifecycle.rs`
- Test: `src-tauri/src/project/filesystem/tests.rs`
- Test: `src-tauri/src/project/production_tests.rs`
- Test: `src/services/worksheet/worksheetService.test.ts`
- Test: `src/features/application/editor/worksheetLifecycle.test.tsx`

**Interfaces:**
- Produces:

```rust
pub enum LifecycleResourcePath {
    Graph(GraphResourcePath),
    Worksheet(WorksheetResourcePath),
}

pub fn rename_worksheet_resource_transaction(
    &self,
    project_instance_id: &ProjectInstanceId,
    worksheet_path: &WorksheetResourcePath,
    expected_revision: ResourceRevision,
    new_name: &ResourceName,
    lifecycle_token: u64,
    operation_id: OperationId,
) -> Result<ResourceMutationResultDto, ProjectFilesystemError>;
```

```rust
StagedFilesystemMutation::MoveFile {
    from: PathBuf,
    to: PathBuf,
}
```

The transaction implementation, not its caller, derives any case-only temporary
path and keeps it out of public mutation state.

- Frontend coordinator produces a monotonic token scoped to project instance and worksheet path; it does not load graph runtime state.
- `rename_worksheet_resource` command shape matches graph rename except for the typed worksheet path.

- [ ] **Step 1: Write RED shared lifecycle tests**

Add tests proving independent graph/worksheet owners, monotonic token rejection, project clear cleanup, and no token pollution across resources.

- [ ] **Step 2: Run lifecycle tests RED**

Run:

```sh
pnpm rust:test resource_lifecycle
```

Expected: FAIL because the registry is graph-only.

- [ ] **Step 3: Generalize lifecycle ownership without compatibility aliases**

Rename the module and types directly. Keep graph behavior unchanged. Map stale owner/token failures to the approved structured stale-resource lifecycle error while preserving stale project identity errors.

- [ ] **Step 4: Write RED filesystem move tests**

Add:

```rust
file_move_commits_source_to_destination_atomically
file_move_rollback_restores_source_and_destination
case_only_file_move_uses_internal_temporary_path
file_move_rejects_existing_portable_conflict
```

Assert temporary paths never appear in the mutation result, history, or project index.

- [ ] **Step 5: Run filesystem tests RED**

Run:

```sh
pnpm rust:test file_move_
pnpm rust:test case_only_file_move_
```

Expected: FAIL because no move primitive exists.

- [ ] **Step 6: Implement staged move and worksheet rename**

Rename validates the exact requested name, rejects a conflicting target instead of suffixing it, increments the worksheet revision, performs the staged move, moves ProjectState authority, and emits one `ResourceMoveDto { kind: "worksheet", name, from, to }` plus common deltas.

Use the same `ResourceName` behavior for graph rename. Remove sanitization and silent suffixing from rename only; retain deterministic allocation for graph/worksheet create and duplicate.

- [ ] **Step 7: Write RED frontend rename tests**

Assert exact service payload, lifecycle token capture before invoke, expected revision capture, stale project/token rejection, and no load/edit/save rename fallback.

- [ ] **Step 8: Implement frontend rename command path**

Route worksheet rename through `resourceActions` and publication submission. Delete the document-name patch/save implementation. Use `formatErrorMessage` for logs/toasts.

- [ ] **Step 9: Run focused GREEN checks**

Run:

```sh
pnpm rust:test resource_lifecycle
pnpm rust:test file_move_
pnpm rust:test case_only_file_move_
pnpm rust:test worksheet_rename
pnpm test -- src/services/worksheet/worksheetService.test.ts src/features/application/editor/worksheetLifecycle.test.tsx
pnpm typecheck
pnpm rust:fmt:check
pnpm rust:check
```

Expected: rename is path-based, revisioned, token-guarded, and portable; graph rename regressions remain green.

---

### Task 6: Durable Worksheet History, Undo/Redo, and Recovery

**Files:**
- Modify: `src-tauri/src/node_system/document/history.rs`
- Modify: `src-tauri/src/node_system/document/mod.rs`
- Modify: `src-tauri/src/project/history_hydration.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Modify: `src-tauri/src/project/resource_patch.rs`
- Modify: `src-tauri/src/project/resource_mutations.rs`
- Modify: `src-tauri/src/project/production_tests.rs`
- Test: `src-tauri/src/node_system/document/tests.rs`
- Test: `src-tauri/src/project/production_tests.rs`

**Interfaces:**
- Consumes: generalized lifecycle/move patches and authoritative worksheet transactions.
- Produces:

```rust
pub enum ResourceLifecycleHistoryPayload {
    Graph { persisted_document: serde_json::Value },
    Worksheet { document: WorksheetDocument },
}

pub struct ResourceLifecycleHistoryPatch {
    pub forward: ResourceLifecyclePatch,
    pub payload: ResourceLifecycleHistoryPayload,
}

pub enum ResourceMoveHistoryPayload {
    Graph { persisted_move_payload: serde_json::Value },
    Worksheet { document: WorksheetDocument },
}

pub struct ResourceMoveHistoryPatch {
    pub from: Box<str>,
    pub to: Box<str>,
    pub kind: ResourceLifecycleKind,
    pub payload: ResourceMoveHistoryPayload,
}
```

- `ProjectHistoryTransaction.resource_lifecycle: Option<ResourceLifecycleHistoryPatch>` stores durable create/delete restoration data.
- `ProjectHistoryTransaction.resource_move: Option<ResourceMoveHistoryPatch>` replaces `graph_resource_move` directly.
- `ProjectDocumentState` includes path-keyed worksheet documents and revisions.
- Lifecycle create/delete history has enough before/after document payload to produce durable filesystem writes/removals during undo/redo.

- [ ] **Step 1: Write RED history model tests**

Add:

```rust
worksheet_document_patch_round_trips_and_inverts
worksheet_resource_move_history_round_trips_without_graph_payload
worksheet_lifecycle_history_preserves_document_for_restore
history_state_tracks_worksheet_paths_and_revisions
```

- [ ] **Step 2: Run model tests RED**

Run:

```sh
pnpm rust:test worksheet_document_patch_round_trips_and_inverts
pnpm rust:test worksheet_resource_move_history_round_trips_without_graph_payload
pnpm rust:test history_state_tracks_worksheet_paths_and_revisions
```

Expected: FAIL because history ignores worksheet keys and move payloads are graph-specific.

- [ ] **Step 3: Generalize history types and application**

Replace graph-only move history fields, add worksheet documents to project history state, and implement worksheet patch/lifecycle/move application. Do not keep serde defaults or aliases for the removed graph-only field.

- [ ] **Step 4: Write RED durable undo/redo and recovery tests**

Add:

```rust
worksheet_create_delete_save_and_rename_undo_redo_are_durable
worksheet_history_rejects_stale_project_before_filesystem_commit
worksheet_history_filesystem_failure_has_zero_authoritative_effects
worksheet_history_publication_failure_enters_authoritative_recovery
```

Verify both undo and redo filesystem targets, revisions, history status, moves/deltas, and project instance identity.

- [ ] **Step 5: Run durable tests RED**

Run:

```sh
pnpm rust:test worksheet_create_delete_save_and_rename_undo_redo_are_durable
pnpm rust:test worksheet_history_
```

Expected: FAIL because history hydration and durable filesystem planning omit worksheets.

- [ ] **Step 6: Implement history hydration and durable filesystem plans**

`discover_touched_resources`, document revision lookup, project document snapshots, replacement, durable mutation construction, and history publication must include worksheets. Reuse the staged file move from Task 5. Keep graph reference-rewrite payload behavior inside the graph payload variant only.

- [ ] **Step 7: Run focused GREEN and graph regression tests**

Run:

```sh
pnpm rust:test worksheet_history_
pnpm rust:test worksheet_create_delete_save_and_rename_undo_redo_are_durable
pnpm rust:test undo_redo_return_atomic_replacements_and_current_history_status
pnpm rust:test history_commands_reject_stale_project_identity
pnpm rust:test history_lifecycle_typing_
pnpm rust:test history_persistence_policies_round_trip
pnpm rust:fmt:check
pnpm rust:check
```

Expected: worksheet and graph history both pass with no legacy graph-only move field.

---

### Task 7: Generic Frontend Publication, Worksheet Moves, and Recovery

**Files:**
- Modify: `src/features/application/editorMutation/resourceMutationResult.ts`
- Modify: `src/features/application/editorMutation/projectPublicationCoordinator.ts`
- Modify: `src/features/application/editorMutation/projectPublicationMovePlan.ts`
- Modify: `src/features/application/editorMutation/projectPublicationRecovery.ts`
- Modify: `src/features/application/editor/cascadeGraphPathReferences.ts`
- Modify: `src/features/core/worksheet/worksheetStore.ts`
- Modify: `src/features/core/resource/documentStateQueries.ts`
- Modify: `src/services/worksheet/worksheetPreviewCache.ts`
- Test: `src/features/application/editorMutation/resourceMutationResult.test.ts`
- Test: `src/features/application/editorMutation/resourceLifecyclePublication.test.ts`
- Test: `src/features/application/editorMutation/projectPublicationMovePlan.test.ts`
- Test: `src/features/application/editorMutation/projectPublicationProductionStores.test.ts`
- Test: `src/features/application/editorMutation/projectPublicationRecovery.test.ts`
- Test: `src/features/application/editorMutation/projectPublicationCoordinator.test.ts`
- Test: `src/services/worksheet/worksheetPreviewCache.test.ts`

**Interfaces:**
- Consumes: common worksheet deltas/moves and path-keyed store.
- Produces a kind-aware publication plan:

```ts
type PreparedResourceMove =
  | PreparedGraphResourceMove
  | PreparedWorksheetResourceMove;

interface PreparedWorksheetResourceMove {
  kind: 'worksheet';
  from: string;
  to: string;
  name: string;
  documents: Record<string, WorksheetDocument>;
  index: WorksheetIndexEntry[];
  resources: Record<ResourceKey, ProjectResourceMeta>;
  documentStates: Record<ResourceKey, DocumentState>;
  tabs: EditorTabMemento;
}
```

- Worksheet move preparation never requires graph entities, graph sessions, projection replacements, or viewport state.

- [ ] **Step 1: Write RED atomic worksheet-publication tests**

Extend the common lifecycle/document delta cases introduced in Task 3 with save revision advancement, direct-result/event-echo exactly-once behavior, and atomic worksheet index/document/resource-state commits.

- [ ] **Step 2: Run publication tests RED**

Run:

```sh
pnpm test -- src/features/application/editorMutation/resourceMutationResult.test.ts src/features/application/editorMutation/resourceLifecyclePublication.test.ts src/features/application/editorMutation/projectPublicationProductionStores.test.ts
```

Expected: FAIL on missing atomic index/resource-state preparation or revision advancement; no test may reintroduce `worksheetDeltas`.

- [ ] **Step 3: Implement common worksheet delta application**

Consolidate the common worksheet-delta preparation introduced in Task 3 into the production aggregate commit. Apply worksheet content/lifecycle deltas by `delta.resource.key`. Use lifecycle `name` to create/remove index and resource metadata without parsing the path; confirm no worksheet-side-channel coordinator type remains.

- [ ] **Step 4: Write RED worksheet move tests**

Assert migration of documents, index, resource metadata, dirty/stale/conflict state, active/selected tab IDs, detail focus, and preview cache invalidation. Assert graph projection/session/viewport state is untouched.

- [ ] **Step 5: Run move/recovery tests RED**

Run:

```sh
pnpm test -- src/features/application/editorMutation/projectPublicationMovePlan.test.ts src/features/application/editorMutation/projectPublicationRecovery.test.ts src/features/application/editorMutation/projectPublicationCoordinator.test.ts src/services/worksheet/worksheetPreviewCache.test.ts
```

Expected: FAIL because move preparation is graph-only and recovery remaps only graph paths.

- [ ] **Step 6: Implement kind-aware move and recovery**

Generalize non-viewport UI remapping or add a focused worksheet sibling. Recovery reconciles worksheet paths against the Rust index and loaded documents, but does not hydrate graph projection state for worksheets. Cache keys receive `worksheetPath` explicitly and are migrated or invalidated atomically.

- [ ] **Step 7: Run focused GREEN checks**

Run:

```sh
pnpm test -- src/features/application/editorMutation/resourceMutationResult.test.ts src/features/application/editorMutation/resourceLifecyclePublication.test.ts src/features/application/editorMutation/projectPublicationMovePlan.test.ts src/features/application/editorMutation/projectPublicationProductionStores.test.ts src/features/application/editorMutation/projectPublicationRecovery.test.ts src/features/application/editorMutation/projectPublicationCoordinator.test.ts src/services/worksheet/worksheetPreviewCache.test.ts
pnpm typecheck
```

Expected: all worksheet publication uses common deltas/moves and graph publication regressions pass.

---

### Task 8: Project Hydration, Worksheet Workflows, Tabs, Sidebar, and Detail UI

**Files:**
- Modify: `src/services/project/projectService.ts`
- Modify: `src/features/core/dataStore/authoritativeProjectLoadPlan.ts`
- Modify: `src/features/core/dataStore/projectIOStore.ts`
- Modify: `src/features/application/editor/useWorksheetManagement.ts`
- Modify: `src/features/application/editor/closeEditorTab.ts`
- Modify: `src/features/application/editor/saveAllDirtyGraphs.ts`
- Modify: `src/features/application/editor/useProjectOperations.ts`
- Modify: `src/features/application/editor/resolveTabDisplayName.ts`
- Modify: `src/features/application/editor/switchEditorTab.ts`
- Modify: `src/features/core/layout/layoutTabModel.ts`
- Modify: `src/features/core/editor/detail/types.ts`
- Modify: `src/features/core/editor/detail/clearDetailFocusForClosedTab.ts`
- Modify: `src/features/core/sidebar/flatRows/buildChartsSidebarModel.ts`
- Modify: `src/features/core/sidebar/flatRows/types.ts`
- Modify: `src/views/EditorView/Worksheet/WorksheetEditor.tsx`
- Modify: `src/views/EditorView/Layout/Detail/Detail.tsx`
- Modify: `src/views/EditorView/Layout/Detail/useDetailPanelModel.ts`
- Modify: `src/views/EditorView/Layout/Detail/panels/WorksheetDetailPanel.tsx`
- Modify: `src/views/EditorView/Layout/sidebar/tabs/SidebarChartsTab.tsx`
- Modify: `src/views/EditorView/Layout/sidebar/sections/SidebarFlatRowItem.tsx`
- Modify: `src/views/EditorView/Layout/sidebar/sections/SidebarFlatRowPanel.tsx`
- Modify: `src/views/EditorView/Layout/sidebar/sections/sidebarFlatRowContext.ts`
- Modify: `src/views/EditorView/Layout/sidebar/useSidebarResourceActions.ts`
- Modify: `src/views/EditorView/Layout/sidebarContextMenu/sidebarContextMenuTypes.ts`
- Modify: `src/views/EditorView/Layout/sidebarContextMenu/buildSidebarContextMenuSections.tsx`
- Modify: `src/views/EditorView/Layout/Sidebar.tsx`
- Modify fixtures only as required: `src/services/worksheet/worksheetDataService.lifecycle.test.ts`, `src/views/EditorView/Worksheet/WorksheetChartPreview.databaseIdentity.test.tsx`
- Test: `src/services/project/projectService.test.ts`
- Test: `src/features/core/dataStore/authoritativeProjectLoadPlan.test.ts`
- Test: `src/features/core/dataStore/projectIOStore.test.ts`
- Test: `src/features/application/editor/worksheetLifecycle.test.tsx`
- Test: `src/features/application/editor/saveAllDirtyWorksheets.test.ts`
- Test: `src/features/core/layout/layoutTabModel.test.ts`
- Test: `src/features/core/sidebar/flatRows/sidebarPanelModel.test.ts`
- Test: `src/views/EditorView/Layout/Detail/panels/WorksheetDetailPanel.databaseIdentity.test.tsx`

**Interfaces:**
- `ProjectWorksheetIndexRow` exactly matches `{ worksheetPath, name, databaseId, chartType, revision }` and is strictly parsed.
- Detail target is `{ kind: 'worksheet'; worksheetPath: string }`.
- `buildWorksheetLayoutTab(worksheetPath: string)` retains the opaque path unchanged.
- Views receive `worksheetPath` and display `name` separately.

- [ ] **Step 1: Write RED project-index and hydration tests**

Add strict index rejection for legacy `id`, missing revision, and unknown fields. Add project reload tests that retain valid worksheet tabs by authoritative path, remove absent paths, and preserve valid active/selected IDs.

- [ ] **Step 2: Run hydration tests RED**

Run:

```sh
pnpm test -- src/services/project/projectService.test.ts src/features/core/dataStore/authoritativeProjectLoadPlan.test.ts src/features/core/dataStore/projectIOStore.test.ts
```

Expected: FAIL because worksheet rows are cast rather than strictly parsed and project load currently deletes worksheet tabs.

- [ ] **Step 3: Implement strict index hydration and tab reconciliation**

Hydrate resource metadata with authoritative revision and name. Replace `loadedWorksheetIds` with path sets. Restore only worksheet tabs whose opaque paths exist in the current project index; do not restore stale project tabs.

- [ ] **Step 4: Write RED application workflow tests**

Assert:

- create opens the path from Rust lifecycle state;
- load/upsert uses `worksheetPath`;
- successful remove closes only after publication;
- failed remove preserves tab/document;
- duplicate invokes `duplicate_worksheet` and opens the Rust-returned path;
- rename uses dedicated command and Rust move result;
- display names come from index/lifecycle/move DTOs;
- dirty save-all keeps stale saves dirty.

- [ ] **Step 5: Run workflow tests RED**

Run:

```sh
pnpm test -- src/features/application/editor/worksheetLifecycle.test.tsx src/features/application/editor/saveAllDirtyWorksheets.test.ts src/features/core/layout/layoutTabModel.test.ts
```

Expected: FAIL on document `id/name` assumptions and old delete/save rename paths.

- [ ] **Step 6: Implement application and tab workflows**

Propagate explicit `worksheetPath` names through hooks and callbacks. Delete UUID parameter names and document-name fallbacks. `closeWorksheetTab` and remove actions capture current project context and expected revision before IPC and do not close before authoritative success. Add worksheet duplicate to the sidebar action/context-menu path and open only the authoritative destination returned by publication.

- [ ] **Step 7: Write RED sidebar/detail tests**

Use a path containing spaces and a Rust-provided name that cannot be inferred by test code. Assert row identity is the path, label is the DTO name, content updates do not contain a name, and name editing invokes rename.

- [ ] **Step 8: Implement sidebar/detail/view propagation**

Pass path and name as separate props. Do not introduce `worksheetPathToName`, filename parsing, or browser path utilities. Keep chart preview content logic identity-free except for an explicit cache path argument.

- [ ] **Step 9: Run focused GREEN checks**

Run:

```sh
pnpm test -- src/services/project/projectService.test.ts src/features/core/dataStore/authoritativeProjectLoadPlan.test.ts src/features/core/dataStore/projectIOStore.test.ts src/features/application/editor/worksheetLifecycle.test.tsx src/features/application/editor/saveAllDirtyWorksheets.test.ts src/features/core/layout/layoutTabModel.test.ts src/features/core/sidebar/flatRows/sidebarPanelModel.test.ts src/views/EditorView/Layout/Detail/panels/WorksheetDetailPanel.databaseIdentity.test.tsx src/services/worksheet/worksheetDataService.lifecycle.test.ts src/views/EditorView/Worksheet/WorksheetChartPreview.databaseIdentity.test.tsx
pnpm typecheck
```

Expected: every frontend worksheet key is the opaque canonical path and no view parses it.

---

### Task 9: Golden Contracts, Architecture Audit, and Full Verification

**Files:**
- Modify: `src-tauri/src/node_system/testing/contracts.rs`
- Modify: `src/tests/fixtures/node-system-contracts/project-events.json`
- Modify: `src/features/core/sync/utils/projectEventWireParser.test.ts`
- Modify: `src-tauri/src/project/filesystem/source_audit_tests.rs`

- Test: all focused files from Tasks 1–8

**Interfaces:**
- The checked-in Rust-to-TypeScript project-event contract contains semantic worksheet create/save/rename/remove/history examples inside the existing aggregate fixture inventory.
- Do not create application-version-named fixtures.
- Architecture audits prohibit production `worksheetId`, `WorksheetDeltaDto`, `worksheetDeltas`, UUID worksheet filenames, frontend resource-path parsing, and direct view-layer `invoke`.

- [ ] **Step 1: Make golden-contract RED assertions**

Extend the Rust contract inventory and TypeScript parser test to require all worksheet lifecycle, document delta, move, direct result, event envelope, undo, and redo variants. Keep the existing aggregate fixture unless a separate fixture is required by the contract harness; any new fixture uses semantic names only.

- [ ] **Step 2: Run golden tests RED**

Run:

```sh
pnpm rust:test checked_in_node_system_contracts_match_rust
pnpm rust:test execution_and_project_event_contract_inventories_are_complete
pnpm test -- src/features/core/sync/utils/projectEventWireParser.test.ts
```

Expected: FAIL until the checked-in fixture and parser expectations include every worksheet variant.

- [ ] **Step 3: Regenerate/update semantic golden data**

Update the fixture from Rust serialization, not hand-authored TypeScript assumptions. Confirm no package/Cargo application version appears in the fixture or test names.

- [ ] **Step 4: Add architecture audit assertions**

The audit must search production code, excluding explicit negative fixtures, for:

```text
worksheetId
WorksheetDeltaDto
worksheetDeltas
worksheets/{UUID}.yssbi-worksheet
```

Also assert frontend production code has no worksheet path parser/name extractor and views do not invoke Tauri directly.

- [ ] **Step 5: Run focused architecture and regression matrices**

Run:

```sh
pnpm rust:test production_project_document_io_is_owned_by_filesystem_modules
pnpm rust:test checked_in_node_system_contracts_match_rust
pnpm rust:test worksheet_
pnpm test -- src/features/core/sync/utils/resourceMutationWireValidator.test.ts src/features/core/sync/utils/projectEventWireParser.test.ts src/services/worksheet/worksheetService.test.ts src/features/core/worksheet/worksheetStore.test.ts src/features/application/editorMutation/resourceLifecyclePublication.test.ts src/features/application/editorMutation/projectPublicationMovePlan.test.ts src/features/application/editorMutation/projectPublicationProductionStores.test.ts src/features/application/editorMutation/projectPublicationRecovery.test.ts src/features/application/editor/worksheetLifecycle.test.tsx src/features/core/dataStore/authoritativeProjectLoadPlan.test.ts src/features/core/dataStore/projectIOStore.test.ts
```

Expected: all focused Rust and frontend tests pass.

- [ ] **Step 6: Run required broader verification**

Run:

```sh
pnpm typecheck
pnpm test
pnpm rust:fmt:check
pnpm rust:check
pnpm rust:test --lib -- --test-threads=1
git diff --check
```

Expected: all commands exit 0; report existing unrelated warnings separately and do not modify unrelated code to silence them.

- [ ] **Step 7: Run canonical cross-stack verification**

Ensure the Tauri application is closed, then run:

```sh
pnpm verify
```

Expected: exit 0. If Windows reports a transient DuckDB file lock, follow systematic debugging: rerun the exact failed test through `pnpm rust:test`, classify only with evidence, and perform at most one fresh canonical rerun. Do not weaken tests.

- [ ] **Step 8: Final self-audit**

Confirm the completion criteria in the approved spec line by line. Run:

```sh
git --no-pager diff --check
git --no-pager diff --cached --name-only
git --no-optional-locks status --short
```

Expected: no whitespace errors, no staged files unless the user explicitly requested staging, and all unrelated dirty-tree changes preserved.
