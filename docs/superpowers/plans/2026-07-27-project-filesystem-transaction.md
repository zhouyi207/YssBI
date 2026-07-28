# Project Filesystem Transaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the temporary project root mutex, full-snapshot fingerprint, rename rollback, and lifecycle code with one normalized-root filesystem transaction protocol that preserves project-instance ownership, narrow authoritative commits, and revisioned frontend results across every project-document reader and writer.

**Architecture:** `ProjectState.project_data` remains authoritative while a dedicated `ProjectFilesystemCoordinator` serializes filesystem access by normalized project root without owning domain state. Operations capture a `ProjectSession` and resource revisions before waiting, acquire one deterministic lease set, stage and validate precise file mutations, commit or roll back only those paths, then revalidate and apply a narrow `ResourceDocumentPatch` under the existing short publication boundary. Existing temporary code in `project_state.rs`, command modules, and `project_io.rs` is moved into focused filesystem, lifecycle, activation, and mutation modules as its callers migrate; no parallel transaction path remains after any task.

**Tech Stack:** Rust 2024, std synchronization and filesystem APIs, serde/serde_json, Tauri 2, TypeScript 5.8, React 19, Zustand, Vitest 4, pnpm.

## Global Constraints

- `ProjectState.project_data` is the authoritative project, graph, function, variable, and worksheet-related domain state; disk reads never overwrite current authority.
- `ProjectState::insert_graph` remains the only graph insertion path for load, create, duplicate, import, and restore runtime setup.
- Every project-document filesystem read or write acquires `ProjectFilesystemCoordinator` leases keyed by `NormalizedProjectRoot`.
- Normalize, sort, and deduplicate all roots before multi-root acquisition; release them in reverse order.
- Never hold `mutation_publication`, `project_path`, `project_data`, `graph_lifecycle`, history, runtime-store, or registry state locks while waiting for a filesystem lease or performing filesystem I/O.
- Capture immutable payloads and expected revisions before lease acquisition; revalidate project identity, root, lifecycle ownership, and revisions after acquisition and again before authoritative publication.
- Lifecycle cancellation takes precedence over filesystem errors produced by an obsolete project session.
- Stage under `<project>/.yssbi-transaction/<operation-id>/`; validate serialized documents before changing live files.
- Journal precise target before-images and directory topology only; never snapshot or restore entire `events`, `functions`, `worksheets`, or variables trees.
- Never publish a `ProjectData` clone captured before I/O; authoritative commits use narrow `ResourceDocumentPatch` values against current state.
- Successful direct results and events include the required `projectInstanceId`, publication revision, correlated resource deltas, exact projection membership or invalidations, and backend history status.
- Frontend services always send a required `projectInstanceId`; application coordinators capture a project epoch and reject stale completions before correlation, store access, event handling, toast, navigation, or index refresh.
- Graph resources use `events/...` and `functions/...` paths; UUIDs identify nodes, pins, connections, variables, worksheets, operation IDs, and project instances only.
- Keep Tauri commands thin; filesystem workflows live under `src-tauri/src/project/`, and frontend invokes remain under `src/services/`.
- Migrate and delete the temporary `filesystem_transactions`, `with_project_filesystem_transaction`, `with_current_project_filesystem_transaction`, `ProjectFilesystemSnapshot`, `GraphRenameDiskRollback`, and whole-`ProjectData` rename commit code; do not wrap or preserve them.
- Remove direct project-document writer exports once their callers migrate. There are no compatibility commands, legacy adapters, fallback writers, or dual writes.
- Preserve unrelated working-tree changes. Do not reset, overwrite, stage, commit, or otherwise rewrite work outside the files named by the active task.
- Do not create commits in any task.
- Strict TDD applies: add the named regression tests first, run the exact RED commands and observe the expected failure, make the minimum production change, then run the exact GREEN commands.
- During Tasks 1–8, Rust execution is limited to the listed focused filters with `CARGO_BUILD_JOBS=1` and `--test-threads=1`, plus `CARGO_BUILD_JOBS=1 pnpm rust:check`; do not run unfiltered `cargo test`, unfiltered `pnpm rust:test`, `pnpm rust:test:sci`, `pnpm verify:rust`, or `pnpm verify`.
- Frontend tests use only the explicit `pnpm exec vitest run <file...>` commands in this plan; do not run unqualified `pnpm test` or `pnpm verify:frontend`.
- Task 9 runs `CARGO_BUILD_JOBS=1 pnpm rust:test -- --test-threads=1` exactly once. If it stalls or exhausts memory, do not retry; record the exact command, exit/termination, and last output.

## Planned File Structure

- `src-tauri/src/project/filesystem/mod.rs`: public transaction boundary and re-exports; no domain authority.
- `src-tauri/src/project/filesystem/root.rs`: `NormalizedProjectRoot` construction for existing and not-yet-created roots.
- `src-tauri/src/project/filesystem/coordinator.rs`: deterministic single-/multi-root admission and RAII release.
- `src-tauri/src/project/filesystem/transaction.rs`: transaction context, staged mutation set, precise before-images, commit, rollback, cleanup, and structured failures.
- `src-tauri/src/project/filesystem/tests.rs`: focused coordinator and transaction fault-injection tests.
- `src-tauri/src/project/project_session.rs`: strong project identity/root snapshots and ownership revalidation.
- `src-tauri/src/project/resource_patch.rs`: narrow authoritative graph/function/variable patch types and revision checks.
- `src-tauri/src/project/graph_lifecycle.rs`: project-instance + graph-path + token + intent ownership currently embedded in `project_state.rs`.
- `src-tauri/src/project/project_reads.rs`: leased project index, graph/function, and worksheet reads with coherent authoritative overlays.
- `src-tauri/src/project/project_activation.rs`: prepare/drain/atomic-publication workflow.
- `src-tauri/src/project/project_writers.rs`: flush, graph/function save, global-variable persistence, and worksheet persistence.
- `src-tauri/src/project/resource_mutations.rs`: create, duplicate, remove, rename, and reference cascades using staged narrow patches.
- `src-tauri/src/project/project_lifecycle.rs`: save-as/copy, project creation, and registered-project deletion under deterministic leases.
- `src-tauri/src/project/filesystem/source_audit_tests.rs`: Rust production-source audit preventing direct project-document filesystem bypasses.
- `src/services/project/projectIdentity.ts`: required frontend project identity and epoch capture/assertion.
- `src/services/project/projectFilesystemContract.test.ts`: service payload and production-source audit for required identity.

---

### Task 1: Normalize roots and coordinate deterministic leases

**Files:**
- Create: `src-tauri/src/project/filesystem/mod.rs`
- Create: `src-tauri/src/project/filesystem/root.rs`
- Create: `src-tauri/src/project/filesystem/coordinator.rs`
- Create: `src-tauri/src/project/filesystem/tests.rs`
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/src/project/project_state.rs:349-431`
- Test: `src-tauri/src/project/filesystem/tests.rs`

**Interfaces:**
- Consumes: `project_root_from_path(&str) -> PathBuf`, platform filesystem path rules, and `std::sync::{Mutex, Condvar}`.
- Produces:

```rust
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NormalizedProjectRoot(PathBuf);

impl NormalizedProjectRoot {
    pub fn from_project_path(path: impl AsRef<Path>) -> Result<Self, ProjectFilesystemError>;
    pub fn as_path(&self) -> &Path;
}

#[derive(Clone, Default)]
pub struct ProjectFilesystemCoordinator {
    registry: Arc<RootLeaseRegistry>,
}

impl ProjectFilesystemCoordinator {
    pub fn acquire(
        &self,
        root: NormalizedProjectRoot,
    ) -> Result<ProjectFilesystemLeaseSet, ProjectFilesystemError>;

    pub fn acquire_many<I>(
        &self,
        roots: I,
    ) -> Result<ProjectFilesystemLeaseSet, ProjectFilesystemError>
    where
        I: IntoIterator<Item = NormalizedProjectRoot>;
}

pub struct ProjectFilesystemLeaseSet {
    coordinator: ProjectFilesystemCoordinator,
    roots: Vec<NormalizedProjectRoot>,
}

impl ProjectFilesystemLeaseSet {
    pub fn roots(&self) -> &[NormalizedProjectRoot];
    pub fn contains(&self, root: &NormalizedProjectRoot) -> bool;
}
```

- `ProjectState` gains `filesystem: ProjectFilesystemCoordinator`; this replaces, rather than accompanies, `filesystem_transactions`.

- [ ] **Step 1: Add failing normalized-root and lease-order tests**

Add exact tests:

```text
project::filesystem::tests::equivalent_existing_and_missing_root_spellings_share_one_lease
project::filesystem::tests::metadata_and_directory_paths_normalize_to_the_same_root
project::filesystem::tests::reverse_order_multi_root_acquisition_is_sorted_deduplicated_and_deadlock_free
project::filesystem::tests::lease_set_releases_roots_in_reverse_order
```

The first test covers `.`/`..`, slash direction, metadata-file versus directory spelling, case folding on Windows, and a destination whose final directory does not exist. The deadlock test starts `[A, B]` and `[B, A]` acquisitions behind a barrier and requires both threads to finish within two seconds.

- [ ] **Step 2: Run the exact tests and verify RED**

Run sequentially:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::filesystem::tests::equivalent_existing_and_missing_root_spellings_share_one_lease --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::filesystem::tests::metadata_and_directory_paths_normalize_to_the_same_root --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::filesystem::tests::reverse_order_multi_root_acquisition_is_sorted_deduplicated_and_deadlock_free --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::filesystem::tests::lease_set_releases_roots_in_reverse_order --exact --test-threads=1
```

Expected: compilation fails because `project::filesystem`, `NormalizedProjectRoot`, and `ProjectFilesystemCoordinator` do not exist.

- [ ] **Step 3: Implement lexical and ancestor-aware root normalization**

Implement `NormalizedProjectRoot::from_project_path` so it:

1. trims and rejects empty input;
2. converts a `metadata.yssbi` input to its parent root without requiring the file to exist;
3. makes relative input absolute from `std::env::current_dir()`;
4. canonicalizes the deepest existing ancestor, appends normalized missing components, and removes `.`/resolves `..` without crossing the canonical ancestor;
5. normalizes Windows drive-letter/case identity consistently with existing `normalize_existing_path` behavior;
6. stores an absolute root path and never performs project I/O after construction.

Return structured `ProjectFilesystemError::InvalidRoot { path, message }`, not a free-form lifecycle string.

- [ ] **Step 4: Implement deterministic lease admission and RAII release**

Use one short registry mutex plus a condition variable. `acquire_many` sorts and deduplicates before waiting, atomically reserves the whole set, and stores the sorted roots in `ProjectFilesystemLeaseSet`; `Drop` removes reservations in reverse order and notifies waiters. Never acquire one root, wait, then acquire a second root.

- [ ] **Step 5: Replace the temporary coordinator and helper API immediately**

Add `filesystem: ProjectFilesystemCoordinator` to `ProjectState`, initialize it in `ProjectState::new`, and expose only:

```rust
pub(crate) fn filesystem(&self) -> &ProjectFilesystemCoordinator;
```

Delete `filesystem_transactions`, `filesystem_transaction_for_path`, `with_project_filesystem_transaction`, and `with_current_project_filesystem_transaction` in this task. Update every current caller to normalize its root and acquire through `state.filesystem().acquire(...)` directly, releasing any state snapshot locks before acquisition. Tasks 2–7 replace those still-direct leased workflows with full transaction/read/application modules; no old lease wrapper or second registry survives Task 1.

- [ ] **Step 6: Run Task 1 GREEN checks**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::filesystem::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Expected: the four focused tests pass, Rust checks pass, and the diff has no whitespace errors. Do not run a full Rust suite.

---

### Task 2: Add transaction context, staging, precise rollback, and narrow authoritative patches

**Files:**
- Create: `src-tauri/src/project/project_session.rs`
- Create: `src-tauri/src/project/resource_patch.rs`
- Create: `src-tauri/src/project/filesystem/transaction.rs`
- Modify: `src-tauri/src/project/project_data.rs`
- Modify: `src-tauri/src/project/filesystem/mod.rs`
- Modify: `src-tauri/src/project/filesystem/tests.rs`
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/src/project/project_state.rs:246-289, 438-499, 784-833`
- Modify: `src-tauri/src/project/project_error.rs`
- Test: `src-tauri/src/project/filesystem/tests.rs`
- Test: `src-tauri/src/project/production_tests.rs`

**Interfaces:**
- Consumes: `NormalizedProjectRoot`, `ProjectFilesystemLeaseSet`, existing `OperationId`, `ResourceKey`, `ResourceRevision`, `GraphResourceDocument`, `VariableInstance`, and `MutationPublication`.
- Produces:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectSession {
    pub instance_id: ProjectInstanceId,
    pub root: NormalizedProjectRoot,
}

#[derive(Clone, Debug)]
pub struct ProjectTransactionContext {
    pub session: ProjectSession,
    pub operation_id: OperationId,
    pub affected_resources: Vec<ResourceKey>,
    pub expected_revisions: BTreeMap<ResourceKey, ResourceRevision>,
}

#[derive(Clone, Debug)]
pub enum StagedFilesystemMutation {
    Write { relative_path: PathBuf, contents: Vec<u8> },
    RemoveFile { relative_path: PathBuf },
    CreateDirectory { relative_path: PathBuf },
    RemoveDirectoryIfEmpty { relative_path: PathBuf },
}

pub struct ProjectFilesystemTransaction {
    context: ProjectTransactionContext,
    lease: ProjectFilesystemLeaseSet,
    staging_root: PathBuf,
    mutations: Vec<StagedFilesystemMutation>,
}

impl ProjectFilesystemTransaction {
    pub fn prepare(
        context: ProjectTransactionContext,
        lease: ProjectFilesystemLeaseSet,
        mutations: Vec<StagedFilesystemMutation>,
    ) -> Result<PreparedProjectFilesystemTransaction, ProjectFilesystemError>;
}

impl PreparedProjectFilesystemTransaction {
    pub fn commit(self) -> Result<CommittedFilesystemMutation, ProjectFilesystemError>;
}

#[derive(Clone, Debug)]
pub enum ResourceDocumentPatch {
    InsertGraph { path: GraphResourcePath, resource: GraphResourceDocument },
    RemoveGraph { path: GraphResourcePath },
    MoveGraph {
        from: GraphResourcePath,
        to: GraphResourcePath,
        moved: GraphResourceDocument,
        referenced_graphs: BTreeMap<GraphResourcePath, GraphResourceDocument>,
        referenced_variables: BTreeMap<VariableId, VariableInstance>,
    },
    PatchVariables {
        updates: BTreeMap<VariableId, VariableInstance>,
        removals: BTreeSet<VariableId>,
    },
    UpsertWorksheet { id: String, document: WorksheetDocument },
    RemoveWorksheet { id: String },
}

impl ProjectState {
    pub fn capture_project_session(&self) -> Result<ProjectSession, ProjectFilesystemError>;
    pub fn validate_project_session(&self, session: &ProjectSession) -> Result<(), ProjectFilesystemError>;
    pub fn apply_resource_document_patch(
        &self,
        context: &ProjectTransactionContext,
        patch: ResourceDocumentPatch,
    ) -> Result<ResourceMutationResultDto, ProjectFilesystemError>;
}
```

`ProjectInstanceId` is a strong newtype serialized transparently as the existing UUID string. Add `pub worksheets: HashMap<String, WorksheetDocument>` to `ProjectData`; activation hydrates it and worksheet reads/writes project from it rather than treating disk as authority. `CommittedFilesystemMutation` owns only the before-images and topology needed to restore its explicit mutation set and exposes `rollback(self) -> Result<(), ProjectFilesystemError>` until authoritative publication succeeds.

- [ ] **Step 1: Add failing transaction and patch tests**

Add exact tests:

```text
project::filesystem::tests::prepare_serializes_every_document_before_touching_live_files
project::filesystem::tests::commit_failure_restores_only_touched_files_and_directory_topology
project::filesystem::tests::rollback_failure_reports_transaction_rollback_failed_with_recovery_requirement
project::filesystem::tests::staging_directory_is_removed_after_commit_and_rollback
project::production_tests::narrow_graph_move_patch_preserves_unrelated_concurrent_mutation
project::production_tests::stale_transaction_context_has_zero_authoritative_effects
project::production_tests::worksheet_patch_preserves_unrelated_concurrent_project_data
```

Use fault injection at: staged serialization, first live replacement, second live replacement, rollback restore, and staging cleanup. The narrow-patch test inserts an unrelated variable after context capture and asserts it survives graph move publication.

- [ ] **Step 2: Run the exact tests and verify RED**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::filesystem::tests::prepare_serializes_every_document_before_touching_live_files --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::filesystem::tests::commit_failure_restores_only_touched_files_and_directory_topology --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::filesystem::tests::rollback_failure_reports_transaction_rollback_failed_with_recovery_requirement --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::filesystem::tests::staging_directory_is_removed_after_commit_and_rollback --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::narrow_graph_move_patch_preserves_unrelated_concurrent_mutation --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::stale_transaction_context_has_zero_authoritative_effects --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::worksheet_patch_preserves_unrelated_concurrent_project_data --exact --test-threads=1
```

Expected: compilation fails on missing context, staging, and patch APIs.

- [ ] **Step 3: Implement strong session/context capture and revalidation**

Capture `ProjectInstanceId`, normalized root, immutable payloads, and affected resource revisions while holding only short state reads. Release all state locks before calling `acquire`/`acquire_many`. Validation returns these exact codes through `ProjectFilesystemError`/`AppError` mapping:

```text
stale_project_lifecycle
resource_revision_conflict
filesystem_transaction_busy
transaction_prepare_failed
transaction_commit_failed
transaction_rollback_failed
```

When both stale ownership and an I/O error exist, return `stale_project_lifecycle`.

- [ ] **Step 4: Implement staged mutation validation and precise before-images**

Reject absolute paths, parent traversal, duplicate targets, and `.yssbi-transaction` targets. Write every `Write` payload beneath `.yssbi-transaction/<operation-id>/prepared/`, deserialize it back into its expected document type through a caller-supplied validator, then collect before-images only for explicit live targets. Commit each target with same-directory temporary replacement where supported. The journal records whether each target was absent, a file with bytes, or a directory with the exact affected child topology.

- [ ] **Step 5: Implement rollback and recovery semantics**

Rollback runs while the lease set is still owned, restores only journaled paths in reverse mutation order, and always attempts staging cleanup. If filesystem commit succeeded but `apply_resource_document_patch` cannot publish, rollback first; if rollback fails, return `transaction_rollback_failed` with `recovery_required = true`. A caller receiving that flag must activate a fresh authoritative reload before emitting an ordinary mutation result.

- [ ] **Step 6: Implement narrow authoritative patch publication**

Under the established short order—`mutation_publication`, `project_path`, `graph_lifecycle` when relevant, then `project_data`—revalidate context/session/revisions, mutate only the named current `ProjectData.graphs`, `ProjectData.variables`, or `ProjectData.worksheets` entries, update history/publication revision, snapshot projections after the patch, and return one `ResourceMutationResultDto`. Route every graph insertion arm through `ProjectState::insert_graph` or an internal lock-aware helper used solely by `insert_graph` and patch publication; do not assign an old full `ProjectData` clone or replace the complete variable/worksheet maps.

- [ ] **Step 7: Delete full-snapshot transaction authority**

Delete `ProjectFilesystemSnapshot`, `data_fingerprint`, `project_filesystem_snapshot`, and `validate_project_filesystem_snapshot`. Keep no JSON fingerprint freshness path.

- [ ] **Step 8: Run Task 2 GREEN checks**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::filesystem::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::narrow_graph_move_patch_preserves_unrelated_concurrent_mutation --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::stale_transaction_context_has_zero_authoritative_effects --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::worksheet_patch_preserves_unrelated_concurrent_project_data --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Expected: focused transaction and patch tests pass. Do not run a full Rust suite.

---

### Task 3: Migrate project index, graph/function, and worksheet reads with consolidated graph lifecycle ownership

**Files:**
- Create: `src-tauri/src/project/graph_lifecycle.rs`
- Create: `src-tauri/src/project/project_reads.rs`
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/src/project/project_state.rs:30-122, 446-499, 1186-1560, 3541-4150`
- Modify: `src-tauri/src/project/project_io.rs:293-425`
- Modify: `src-tauri/src/project/worksheet_io.rs:60-160`
- Modify: `src-tauri/src/commands/command_project/query.rs`
- Modify: `src-tauri/src/commands/command_node_system.rs:57-70`
- Modify: `src-tauri/src/commands/command_worksheet.rs:173-185`
- Modify: `src-tauri/src/project/production_tests.rs`
- Test: `src-tauri/src/project/project_reads.rs`
- Test: `src-tauri/src/project/graph_lifecycle.rs`
- Test: `src-tauri/src/commands/command_project/query.rs`

**Interfaces:**
- Consumes: Task 2 `ProjectSession`, transaction context validation, coordinator leases, existing projection building, `LifecycleToken = u64`, and `GraphResourcePath`.
- Produces:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphLifecycleIntent { Load, Unload, Rename }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphLifecycleOwner {
    pub project_instance_id: ProjectInstanceId,
    pub graph_path: GraphResourcePath,
    pub token: u64,
    pub intent: GraphLifecycleIntent,
}

impl GraphLifecycleRegistry {
    pub fn register(
        &self,
        session: &ProjectSession,
        graph_path: &GraphResourcePath,
        token: u64,
        intent: GraphLifecycleIntent,
    ) -> Result<GraphLifecycleGuard, ProjectFilesystemError>;

    pub fn validate(&self, owner: &GraphLifecycleOwner) -> Result<(), ProjectFilesystemError>;
    pub fn clear_for_project(&self, project_instance_id: &ProjectInstanceId);
}

impl ProjectState {
    pub fn read_project_index(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
    ) -> Result<ProjectIndex, ProjectFilesystemError>;

    pub fn load_graph_projection(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        lifecycle_token: u64,
        locale: &str,
    ) -> Result<EditorGraphProjectionDto, ProjectFilesystemError>;

    pub fn load_worksheet_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        worksheet_id: &str,
    ) -> Result<WorksheetDocument, ProjectFilesystemError>;
}
```

Commands require `project_instance_id: String` for index, graph load/unload, and worksheet load, parse it once, call these methods, and map DTOs/errors only.

- [ ] **Step 1: Add failing coherent-read and lifecycle tests**

Add exact tests:

```text
project::project_reads::tests::delayed_project_index_read_has_zero_effects_after_project_replacement
project::project_reads::tests::project_index_overlays_functions_and_globals_from_one_authoritative_snapshot
project::project_reads::tests::project_index_waits_for_resource_writer_and_returns_committed_layout
project::project_reads::tests::worksheet_load_rejects_replaced_project_before_returning_document
project::graph_lifecycle::tests::old_project_load_unload_and_rename_tokens_never_match_replacement_project
project::graph_lifecycle::tests::load_returns_projection_from_its_owned_committed_snapshot
project::graph_lifecycle::tests::unload_and_rename_intents_exclude_load_for_the_same_owner
```

Move the assertions from existing query and embedded `graph_lifecycle_tests` into these focused owners before deleting duplicated old tests.

- [ ] **Step 2: Run exact RED tests**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_reads::tests::delayed_project_index_read_has_zero_effects_after_project_replacement --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_reads::tests::project_index_overlays_functions_and_globals_from_one_authoritative_snapshot --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_reads::tests::project_index_waits_for_resource_writer_and_returns_committed_layout --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_reads::tests::worksheet_load_rejects_replaced_project_before_returning_document --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::graph_lifecycle::tests::old_project_load_unload_and_rename_tokens_never_match_replacement_project --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::graph_lifecycle::tests::load_returns_projection_from_its_owned_committed_snapshot --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::graph_lifecycle::tests::unload_and_rename_intents_exclude_load_for_the_same_owner --exact --test-threads=1
```

Expected: missing modules/signatures or failures because current commands do not require identity for every read.

- [ ] **Step 3: Move lifecycle types and guards out of `project_state.rs`**

Move `GraphLifecycleIntent`, key/ownership/operation logic, rename ownership unwinding, and lifecycle test hooks into `graph_lifecycle.rs`. Replace string IDs in lifecycle keys with `ProjectInstanceId`. Keep one registry field in `ProjectState`; remove the embedded duplicate structs and helper implementations.

- [ ] **Step 4: Implement the leased read protocol**

For each read: capture/validate the requested session, release state locks, acquire the normalized root lease, revalidate, perform disk reads, reacquire the short publication/read boundary, revalidate, clone one coherent `ProjectData`, overlay loaded functions and globals, and build the response. `read_project_index` must not call mutation-producing flatten helpers; move layout normalization into explicit write transactions in Tasks 5–7.

- [ ] **Step 5: Migrate graph load/unload**

Register lifecycle ownership before waiting for the root lease, revalidate it under the lease, load disk data, release the lease, then publish through `insert_graph` only if session and owner still match. Return the projection from the operation-owned committed snapshot. Unload applies a narrow in-memory removal only after required identity/token validation and returns its project-scoped result; no stale request removes graph-local variables from a replacement project.

- [ ] **Step 6: Thin the read commands and remove direct readers**

Change exact command inputs:

```rust
pub fn get_project_index(
    state: State<ProjectState>,
    project_instance_id: String,
) -> Result<ProjectIndex, AppError>;

pub fn load_project_graph(
    state: State<ProjectState>,
    project_instance_id: String,
    graph_path: String,
    locale: Option<String>,
    lifecycle_token: u64,
) -> Result<EditorGraphProjectionDto, AppError>;

pub fn load_worksheet(
    state: State<ProjectState>,
    project_instance_id: String,
    worksheet_id: String,
) -> Result<WorksheetDocument, AppError>;
```

Remove `get_project_index_with_reader`, `project_index_ownership`, `project_index_is_owned`, `overlay_project_index_if_owned`, and `load_graph_from_current_project` compatibility entry points after all callers use `project_reads`.

- [ ] **Step 7: Run Task 3 GREEN checks**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_reads::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::graph_lifecycle::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- commands::command_project::query::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::project_replacement_during_function_loading_cancels_before_old_resource_insert --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Expected: focused reads/lifecycle tests pass and stale reads have zero effects. Do not run a full Rust suite.

---

### Task 4: Separate activation preparation, run drain, and atomic publication

**Files:**
- Create: `src-tauri/src/project/project_activation.rs`
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/src/project/project_state.rs:258-265, 668-760, 835-843, 3961-4040`
- Modify: `src-tauri/src/node_system/runtime/project_run.rs`
- Modify: `src-tauri/src/node_system/runtime/project_resource.rs`
- Modify: `src-tauri/src/node_system/runtime/project_run.rs`
- Modify: `src-tauri/src/commands/command_project/lifecycle.rs:28-63, 143-150`
- Modify: `src-tauri/src/project/production_tests.rs`
- Test: `src-tauri/src/project/project_activation.rs`
- Test: `src-tauri/src/node_system/runtime/project_run.rs`

**Interfaces:**
- Consumes: leased project reads from Task 3, `ProjectRunRegistry::begin_drain`, `PreparedProjectActivation`, `ProjectStore`, graph lifecycle registry, history, and publication state.
- Produces:

```rust
pub struct PreparedProjectActivation {
    pub session_root: Option<NormalizedProjectRoot>,
    pub data: ProjectData,
    pub store: ProjectStore,
    pub variable_revisions: HashMap<VariableId, ResourceRevision>,
}

impl ProjectState {
    pub fn prepare_project_activation(
        &self,
        path: Option<&Path>,
    ) -> Result<PreparedProjectActivation, ProjectFilesystemError>;

    pub fn activate_prepared_project(
        &self,
        prepared: PreparedProjectActivation,
    ) -> Result<ProjectSession, ProjectFilesystemError>;

    pub fn activate_project_from_path(
        &self,
        path: &Path,
    ) -> Result<ProjectSession, ProjectFilesystemError>;

    pub fn clear_project(&self) -> Result<ProjectInstanceId, ProjectFilesystemError>;
}
```

`ProjectRunDrainGuard` remains count-safe and denies both pre-run and run admission for the drained session until atomic publication completes.

- [ ] **Step 1: Add failing activation/drain tests**

Add exact tests:

```text
project::project_activation::tests::activation_and_pre_run_function_loading_complete_without_deadlock
project::project_activation::tests::activation_waits_for_old_pre_runs_without_state_or_filesystem_locks
project::project_activation::tests::concurrent_activations_publish_only_complete_sessions
project::project_activation::tests::failed_preparation_leaves_current_identity_path_data_lifecycle_and_runtime_unchanged
project::project_activation::tests::same_root_reactivation_invalidates_old_graph_lifecycle_owners
node_system::runtime::project_run::tests::nested_drain_guards_keep_admission_closed_until_last_drop
```

The deadlock test pauses function loading after pre-run registration, starts activation, then releases loading and requires both threads to finish. Hooks assert the root lease and state publication locks are not held while waiting for drain.

- [ ] **Step 2: Run exact RED tests**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_activation::tests::activation_and_pre_run_function_loading_complete_without_deadlock --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_activation::tests::activation_waits_for_old_pre_runs_without_state_or_filesystem_locks --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_activation::tests::concurrent_activations_publish_only_complete_sessions --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_activation::tests::failed_preparation_leaves_current_identity_path_data_lifecycle_and_runtime_unchanged --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_activation::tests::same_root_reactivation_invalidates_old_graph_lifecycle_owners --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::project_run::tests::nested_drain_guards_keep_admission_closed_until_last_drop --exact --test-threads=1
```

Expected: missing activation module/signatures or lock-order regression failures.

- [ ] **Step 3: Move preparation out of `ProjectState`**

Under the destination root lease, read manifest, globals, database declarations, and every worksheet document into `ProjectData.worksheets`, then build `PreparedProjectActivation` outside state locks. Release the root lease before run drain. Model clear/new-project preparation with `session_root: None` and empty authoritative data rather than publishing path and data separately.

- [ ] **Step 4: Drain run admission without publication or filesystem locks**

Serialize activations with the dedicated activation mutex. Snapshot the currently published `Arc<ProjectRunRegistry>` and `ProjectSessionId`, release runtime-store locks, call `begin_drain`, and wait with no filesystem lease or state lock. Keep the guard alive through publication.

- [ ] **Step 5: Publish the complete session atomically**

Under the established short publication order, allocate a new `ProjectInstanceId`, replace path/root, `project_data`, graph lifecycle registry contents, runtime store, variable revisions, and history as one operation. No hook or event may observe a new path with old/default data. Return the new `ProjectSession`; commands emit only after this succeeds.

- [ ] **Step 6: Remove old activation entry points**

Delete `activate_loaded_project`, public `set_path` usage in production, and the old `activate_project_from_file` implementation. Update tests/helpers to activate through `activate_prepared_project` or a `#[cfg(test)]` fixture that constructs a complete prepared activation without I/O.

- [ ] **Step 7: Run Task 4 GREEN checks**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_activation::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::project_run::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::project_replacement_during_function_loading_cancels_before_old_resource_insert --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Expected: activation/drain tests pass without deadlock. Do not run a full Rust suite.

---

### Task 5: Migrate save, flush, global/function, and worksheet writers

**Files:**
- Create: `src-tauri/src/project/project_writers.rs`
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/src/project/project_state.rs:845-852, 1031-1050`
- Modify: `src-tauri/src/project/project_io.rs:119-165, 257-291, 1123-1130`
- Modify: `src-tauri/src/project/project_state_variable.rs`
- Modify: `src-tauri/src/project/worksheet_io.rs:132-145, 330-337`
- Modify: `src-tauri/src/commands/command_project/lifecycle.rs:132-141`
- Modify: `src-tauri/src/commands/command_node_system.rs:72-82`
- Modify: `src-tauri/src/commands/command_variable/mod.rs`
- Modify: `src-tauri/src/commands/command_worksheet.rs:151-209`
- Modify: `src-tauri/src/project/production_tests.rs`
- Test: `src-tauri/src/project/project_writers.rs`
- Test: `src-tauri/src/commands/command_worksheet.rs`

**Interfaces:**
- Consumes: Task 2 transaction context/staging, Task 3 project reads, current authoritative document snapshots/revisions, and existing revisioned mutation result DTOs.
- Produces:

```rust
impl ProjectState {
    pub fn flush_project_documents(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        operation_id: OperationId,
    ) -> Result<ProjectSaveResultDto, ProjectFilesystemError>;

    pub fn save_graph_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<ProjectSaveResultDto, ProjectFilesystemError>;

    pub fn persist_global_variables(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        expected_revisions: BTreeMap<ResourceKey, ResourceRevision>,
        operation_id: OperationId,
    ) -> Result<ProjectSaveResultDto, ProjectFilesystemError>;

    pub fn create_worksheet_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        name: Option<String>,
        database_id: Option<String>,
        operation_id: OperationId,
    ) -> Result<WorksheetMutationResultDto, ProjectFilesystemError>;

    pub fn save_worksheet_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        document: WorksheetDocument,
        operation_id: OperationId,
    ) -> Result<WorksheetMutationResultDto, ProjectFilesystemError>;

    pub fn delete_worksheet_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        worksheet_id: &str,
        operation_id: OperationId,
    ) -> Result<WorksheetMutationResultDto, ProjectFilesystemError>;
}
```

`ProjectSaveResultDto` and `WorksheetMutationResultDto` include `project_instance_id`, publication revision, affected resources, and exact index invalidation; commands emit only their returned committed event.

- [ ] **Step 1: Add failing writer transaction tests**

Add exact tests:

```text
project::project_writers::tests::graph_save_revalidates_revision_after_waiting_for_rename
project::project_writers::tests::flush_writes_one_coherent_authoritative_snapshot_without_recreating_removed_graphs
project::project_writers::tests::global_variable_writer_cannot_be_overwritten_by_rename_rollback
project::project_writers::tests::function_save_persists_signature_and_graph_at_one_revision
project::project_writers::tests::worksheet_create_rechecks_unique_name_under_root_lease
project::project_writers::tests::worksheet_commit_failure_restores_file_and_nested_directory_topology
project::project_writers::tests::stale_writer_emits_no_result_or_event
```

- [ ] **Step 2: Run exact RED tests**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_writers::tests::graph_save_revalidates_revision_after_waiting_for_rename --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_writers::tests::flush_writes_one_coherent_authoritative_snapshot_without_recreating_removed_graphs --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_writers::tests::global_variable_writer_cannot_be_overwritten_by_rename_rollback --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_writers::tests::function_save_persists_signature_and_graph_at_one_revision --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_writers::tests::worksheet_create_rechecks_unique_name_under_root_lease --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_writers::tests::worksheet_commit_failure_restores_file_and_nested_directory_topology --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_writers::tests::stale_writer_emits_no_result_or_event --exact --test-threads=1
```

Expected: missing writer methods/results or stale current direct writers fail assertions.

- [ ] **Step 3: Centralize pure serialization**

Convert `project_io.rs` and `worksheet_io.rs` writer internals into pure byte builders and path planners:

```rust
pub fn serialize_project_manifest(data: &ProjectData) -> Result<Vec<u8>, ProjectError>;
pub fn serialize_global_variables(data: &ProjectData) -> Result<Vec<u8>, ProjectError>;
pub fn serialize_graph_document(
    data: &ProjectData,
    graph_path: &GraphResourcePath,
) -> Result<(PathBuf, Vec<u8>), ProjectError>;
pub fn serialize_worksheet(document: &WorksheetDocument) -> Result<(PathBuf, Vec<u8>), ProjectError>;
```

These functions perform no filesystem writes. Keep raw `write_json` private to the transaction staging implementation or test fixtures.

- [ ] **Step 4: Implement save/flush/global/function writers**

Snapshot exact authoritative payloads and revisions, acquire the root lease without state locks, revalidate, stage all bytes, commit, and report success. Flush plans manifest, globals, and loaded graph/function targets from one coherent snapshot but never deletes unknown/unloaded resource files. Graph/function save writes only its document and exact local variables. Global variable commands persist only `variables.yssbi-vars`, not a full project rewrite.

- [ ] **Step 5: Implement worksheet writers**

Move unique-name selection and default-database snapshot into `ProjectState` application code. Recheck destination names while leased, stage only the worksheet file and required directories, then publish `UpsertWorksheet` or `RemoveWorksheet` against current `ProjectData.worksheets` and return a project-scoped mutation result. Remove filesystem workflows from `command_worksheet.rs`; disk worksheet contents never replace current authority after activation.

- [ ] **Step 6: Change command contracts to required identity/revision/correlation**

Add `project_instance_id` and `operation_id` to flush/global/worksheet command inputs; add `expected_revision` to graph/function save inputs. Commands parse values, call one state method, emit one returned event, and map DTOs.

- [ ] **Step 7: Remove migrated direct writers**

Remove production exports/usages of `save_project_to_file`, `save_project_graph_to_file`, `save_worksheet_to_file`, and `delete_worksheet_from_file`. Retain explicit `#[cfg(test)]` fixture helpers under the test module rather than public production writers. Delete `persist_current_project`, `save_graph_resource_to`, and `save_graph_resource`.

- [ ] **Step 8: Run Task 5 GREEN checks**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_writers::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- commands::command_worksheet::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- commands::command_variable::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Expected: focused writers pass and no migrated direct writer remains in production callers. Do not run a full Rust suite.

---

### Task 6: Migrate create, duplicate, remove, and rename to one resource transaction path

**Files:**
- Create: `src-tauri/src/project/resource_mutations.rs`
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/src/project/project_state.rs:892-1030, 1052-1309, 4042-4380`
- Modify: `src-tauri/src/project/project_io.rs:167-255, 414-474`
- Modify: `src-tauri/src/commands/command_node_system.rs:83-181, 480-572`
- Modify: `src-tauri/src/event/event_resource.rs`
- Modify: `src-tauri/src/project/production_tests.rs`
- Test: `src-tauri/src/project/resource_mutations.rs`
- Test: `src-tauri/src/commands/command_node_system.rs`

**Interfaces:**
- Consumes: staged filesystem transaction, `ResourceDocumentPatch`, graph lifecycle `Rename` guard, revisioned result/event DTOs, pure graph serializers, and `ProjectState::insert_graph`.
- Produces:

```rust
impl ProjectState {
    pub fn create_graph_resource_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        name: &str,
        kind: GraphDocumentKind,
        operation_id: OperationId,
    ) -> Result<ResourceMutationResultDto, ProjectFilesystemError>;

    pub fn duplicate_graph_resource_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        source: &GraphResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<ResourceMutationResultDto, ProjectFilesystemError>;

    pub fn remove_graph_resource_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<ResourceMutationResultDto, ProjectFilesystemError>;

    pub fn rename_graph_resource_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        expected_revision: ResourceRevision,
        new_name: &str,
        lifecycle_token: u64,
        operation_id: OperationId,
    ) -> Result<ResourceMutationResultDto, ProjectFilesystemError>;
}
```

Create/duplicate allocate persistent graph/node/connection/variable identities in Rust. Rename returns a `ResourceDocumentPatch::MoveGraph` and exact projection replacements/invalidations; its event carries `projectInstanceId` and publication revision.

- [ ] **Step 1: Add failing resource transaction tests**

Add exact tests:

```text
project::resource_mutations::tests::create_rechecks_destination_under_lease_and_routes_insert_through_project_state
project::resource_mutations::tests::duplicate_rechecks_destination_and_allocates_persistent_identities_in_rust
project::resource_mutations::tests::remove_rolls_back_file_when_authoritative_revision_changed
project::resource_mutations::tests::rename_stages_complete_reference_cascade_before_live_mutation
project::resource_mutations::tests::rename_rollback_restores_only_target_graph_global_and_worksheet_paths
project::resource_mutations::tests::rename_narrow_patch_preserves_unrelated_graph_variable_and_history_mutations
project::resource_mutations::tests::save_flush_and_index_cannot_enter_during_rename_commit_or_rollback
project::resource_mutations::tests::old_project_create_duplicate_remove_and_rename_have_zero_effects
commands::command_node_system::tests::resource_commands_emit_one_project_scoped_committed_result
```

The destination race tests pause after initial planning, create the candidate file from a competing leased operation, and require a newly suffixed path or `destination_not_empty` without overwrite.

- [ ] **Step 2: Run exact RED tests**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::resource_mutations::tests::create_rechecks_destination_under_lease_and_routes_insert_through_project_state --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::resource_mutations::tests::duplicate_rechecks_destination_and_allocates_persistent_identities_in_rust --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::resource_mutations::tests::remove_rolls_back_file_when_authoritative_revision_changed --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::resource_mutations::tests::rename_stages_complete_reference_cascade_before_live_mutation --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::resource_mutations::tests::rename_rollback_restores_only_target_graph_global_and_worksheet_paths --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::resource_mutations::tests::rename_narrow_patch_preserves_unrelated_graph_variable_and_history_mutations --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::resource_mutations::tests::save_flush_and_index_cannot_enter_during_rename_commit_or_rollback --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::resource_mutations::tests::old_project_create_duplicate_remove_and_rename_have_zero_effects --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- commands::command_node_system::tests::resource_commands_emit_one_project_scoped_committed_result --exact --test-threads=1
```

Expected: missing transaction methods or failures from current direct writes/full-state rename replacement.

- [ ] **Step 3: Implement create and duplicate through staged insertion**

Plan names/paths from authoritative state, then repeat destination availability checks under the lease against disk and current revisions. Build graph shells and duplicate rebinding in memory, serialize to staging, commit, and apply `InsertGraph` through `ProjectState::insert_graph`. If resources remain lazy after creation/duplication, publish insertion/runtime setup first and then apply the explicit lifecycle unload patch; never insert directly into `ProjectData.graphs` from command or I/O code.

- [ ] **Step 4: Implement remove through a precise deletion transaction**

Capture resource ownership/revision and local variables, stage a `RemoveFile`, commit under lease, then publish `RemoveGraph`. On stale authoritative publication, restore only the removed file from its before-image. Return `resource_revision_conflict` with no event if freshness fails before commit.

- [ ] **Step 5: Implement rename and reference cascades as a prepared mutation set**

Load every affected unloaded graph and global-variable document under the same root lease, compute target graph contents, call-function references, self references, and variable scope changes in memory, serialize and validate all outputs, then commit the exact write/remove list. Apply `MoveGraph` to current authority with only the moved graph, changed loaded callers, and changed variables. Preserve unrelated graphs, variables, databases, history entries, and concurrent revisions.

- [ ] **Step 6: Delete temporary rename and direct mutation paths**

Delete `GraphRenameDiskRollback`, `register_graph_rename` staging clone return, `commit_graph_rename(ProjectData)`, `cascade_graph_path_references_on_disk`, `duplicate_project_graph_file`, `remove_project_graph_from_file`, and old `create_graph_resource`/`duplicate_graph_resource`/`remove_graph_resource`/`rename_graph_resource`. Move reusable pure remapping functions to `resource_mutations.rs` and keep them I/O-free.

- [ ] **Step 7: Thin resource commands and events**

Require project identity, expected revision, lifecycle token for rename, and operation ID. Emit only after the returned authoritative result exists. Remove the standalone unrevisioned `GraphResourceMoved` emission if the revisioned resource mutation event fully replaces it; do not emit both.

- [ ] **Step 8: Run Task 6 GREEN checks**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::resource_mutations::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- commands::command_node_system::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::normalized_graph_lifecycle_routes_every_insert_through_project_state --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::function_duplicate_rebinds_self_identity_and_loaded_rename_is_authoritative --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Expected: resource mutations pass through one transaction path and whole-tree rollback/full-state commit code is gone. Do not run a full Rust suite.

---

### Task 7: Migrate save-as, project creation/copy, and registered-project deletion

**Files:**
- Create: `src-tauri/src/project/project_lifecycle.rs`
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/src/project/project_io.rs:499-591`
- Modify: `src-tauri/src/project/project_registry.rs:472-518`
- Modify: `src-tauri/src/commands/command_project/lifecycle.rs`
- Modify: `src-tauri/src/commands/command_project/registry.rs:62-104`
- Modify: `src-tauri/src/project/project_activation.rs`
- Modify: `src-tauri/src/project/production_tests.rs`
- Test: `src-tauri/src/project/project_lifecycle.rs`
- Test: `src-tauri/src/commands/command_project/lifecycle.rs`
- Test: `src-tauri/src/commands/command_project/registry.rs`

**Interfaces:**
- Consumes: deterministic multi-root coordinator, transaction staging, activation drain/publication, coherent authoritative snapshots, and project registry record APIs.
- Produces:

```rust
impl ProjectState {
    pub fn save_project_as_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        destination: &Path,
        operation_id: OperationId,
    ) -> Result<PreparedProjectCopy, ProjectFilesystemError>;

    pub fn create_project_transaction(
        &self,
        name: &str,
        destination: &Path,
        operation_id: OperationId,
    ) -> Result<CreatedProject, ProjectFilesystemError>;

    pub fn delete_project_transaction(
        &self,
        root: &Path,
        expected_active_instance_id: Option<&ProjectInstanceId>,
        operation_id: OperationId,
    ) -> Result<ProjectDeletionResult, ProjectFilesystemError>;
}

pub struct PreparedProjectCopy {
    pub metadata_path: PathBuf,
    pub prepared_activation: PreparedProjectActivation,
}

pub struct CreatedProject {
    pub metadata_path: PathBuf,
    pub project_name: String,
}

pub struct ProjectDeletionResult {
    pub deleted_root: NormalizedProjectRoot,
    pub cleared_project_instance_id: Option<ProjectInstanceId>,
}
```

Save-as acquires source and destination in one sorted lease set, builds destination from a coherent authoritative source snapshot, and activates only after complete filesystem commit. Deletion drains the active session when roots match and excludes all readers/writers for the root.

- [ ] **Step 1: Add failing multi-root/project lifecycle tests**

Add exact tests:

```text
project::project_lifecycle::tests::save_as_reverse_root_order_cannot_deadlock
project::project_lifecycle::tests::save_as_rechecks_destination_emptiness_under_both_leases
project::project_lifecycle::tests::save_as_builds_destination_from_one_authoritative_snapshot_and_publishes_after_commit
project::project_lifecycle::tests::failed_save_as_leaves_source_session_and_destination_unchanged
project::project_lifecycle::tests::create_project_rechecks_destination_policy_under_lease
project::project_lifecycle::tests::registered_project_deletion_excludes_index_load_save_rename_and_worksheet_operations
project::project_lifecycle::tests::active_project_deletion_drains_then_clears_before_registry_removal_event
project::project_lifecycle::tests::stale_active_identity_cannot_delete_replacement_project
```

- [ ] **Step 2: Run exact RED tests**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_lifecycle::tests::save_as_reverse_root_order_cannot_deadlock --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_lifecycle::tests::save_as_rechecks_destination_emptiness_under_both_leases --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_lifecycle::tests::save_as_builds_destination_from_one_authoritative_snapshot_and_publishes_after_commit --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_lifecycle::tests::failed_save_as_leaves_source_session_and_destination_unchanged --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_lifecycle::tests::create_project_rechecks_destination_policy_under_lease --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_lifecycle::tests::registered_project_deletion_excludes_index_load_save_rename_and_worksheet_operations --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_lifecycle::tests::active_project_deletion_drains_then_clears_before_registry_removal_event --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_lifecycle::tests::stale_active_identity_cannot_delete_replacement_project --exact --test-threads=1
```

Expected: missing lifecycle methods or failures because current save-as nests source/destination locks and deletion bypasses the coordinator.

- [ ] **Step 3: Implement save-as with one deterministic lease set**

Capture the source session and coherent authoritative payload, normalize both roots, reject equal roots, call `acquire_many([source, destination])`, revalidate source and destination emptiness/policy, stage the complete destination under its transaction directory, and commit. Do not flush the source as a side effect. Build `PreparedProjectActivation` from committed destination bytes while still protected, release leases, then drain and atomically publish the destination session.

- [ ] **Step 4: Implement project creation under a destination lease**

Validate name/path before waiting and repeat destination emptiness/policy after acquiring the normalized destination lease. Stage manifest, empty globals, required directories, and database directory as one transaction. Register the project only after filesystem commit. A registry failure leaves the valid created project on disk and returns a structured registry error; it does not silently delete user data.

- [ ] **Step 5: Implement registered-project deletion under root ownership**

Fetch the registry record without holding registry locks across filesystem work, then hold the dedicated activation mutex for the deletion lifecycle. Normalize and lease the root, validate the directory and expected active identity, snapshot the active run registry/session, and release the root lease. Establish the run drain with no filesystem or state lock held. Reacquire the same root lease while the activation mutex still prevents replacement, revalidate the directory and expected identity, atomically publish cleared authority under the short publication boundary, release state locks, move the directory to the recycle bin while retaining only the root lease, then release drain/lease/activation guards. Remove the registry record and emit `ProjectCleared` only after successful deletion/clear.

- [ ] **Step 6: Remove direct copy/delete and temporary helper APIs**

Delete production `copy_project_directory`, `save_project_as_to_directory`, `delete_project_directory`, and `paths_refer_to_same_project`. Confirm the old root-lease wrappers removed in Task 1 have not been reintroduced.

- [ ] **Step 7: Run Task 7 GREEN checks**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_lifecycle::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- commands::command_project::lifecycle::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- commands::command_project::registry::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Expected: project lifecycle tests pass and every project-root mutation uses the coordinator. Do not run a full Rust suite.

---

### Task 8: Enforce frontend project identity and audit all production filesystem sources

**Files:**
- Create: `src/services/project/projectIdentity.ts`
- Create: `src/services/project/projectFilesystemContract.test.ts`
- Create: `src-tauri/src/project/filesystem/source_audit_tests.rs`
- Modify: `src-tauri/src/project/filesystem/mod.rs`
- Modify: `src/services/project/projectService.ts`
- Modify: `src/services/graph/graphService.ts`
- Modify: `src/features/core/dataStore/projectIOStore.ts`
- Modify: `src/features/core/dataStore/projectIOStore.test.ts`
- Modify: `src/features/application/editorProjection/graphProjectionCoordinator.ts`
- Modify: `src/features/application/editor/graphDocumentUnload.ts`
- Modify: `src/features/application/editor/graphDocumentUnload.test.ts`
src/features/application/editorMutation/projectPublicationMovePlan.ts
src/features/application/editorMutation/projectPublicationMovePlan.test.ts
- Modify: `src/features/core/sync/handlers/ResourceEventHandler.ts`
- Modify: `src/features/core/sync/handlers/ResourceEventHandler.test.ts`
- Modify: `src/features/core/sync/handlers/ProjectMutationEventHandler.ts`
- Modify: `src/features/core/sync/handlers/ProjectMutationEventHandler.test.ts`
- Modify: `src/features/application/editorMutation/resourceMutationResult.ts`
- Modify: `src/shared/types/dto/editorMutation.ts`
- Test: `src/services/project/projectFilesystemContract.test.ts`
- Test: `src/features/core/dataStore/projectIOStore.test.ts`
- Test: named editor/event test files above
- Test: `src-tauri/src/project/filesystem/source_audit_tests.rs`

**Interfaces:**
- Consumes: required backend identity command fields/results/events from Tasks 3–7 and existing resource mutation publication ordering.
- Produces:

```ts
export interface ProjectIdentitySnapshot {
  projectInstanceId: string;
  epoch: number;
}

export function captureProjectIdentity(): ProjectIdentitySnapshot;
export function isCurrentProjectIdentity(identity: ProjectIdentitySnapshot): boolean;
export function assertCurrentProjectIdentity(identity: ProjectIdentitySnapshot): void;
```

Service signatures become:

```ts
ProjectService.getProjectIndex(projectInstanceId: string): Promise<ProjectIndexRow>
ProjectService.flushProject(projectInstanceId: string, operationId: string): Promise<ProjectSaveResultDto>
ProjectService.saveProjectAs(projectInstanceId: string, operationId: string): Promise<ProjectRecordRow | null>
ProjectService.deleteRegisteredProjectFiles(id: string, expectedActiveProjectInstanceId: string | null, operationId: string): Promise<void>
GraphService.createEvent(projectInstanceId: string, graphName: string, operationId: string): Promise<ResourceMutationResultDto>
GraphService.createFunction(projectInstanceId: string, graphName: string, operationId: string): Promise<ResourceMutationResultDto>
GraphService.saveProjectGraph(projectInstanceId: string, graphPath: string, expectedRevision: number, operationId: string): Promise<ProjectSaveResultDto>
GraphService.duplicateGraph(projectInstanceId: string, graphPath: string, expectedRevision: number, operationId: string): Promise<ResourceMutationResultDto>
GraphService.removeGraph(projectInstanceId: string, graphPath: string, expectedRevision: number, operationId: string): Promise<ResourceMutationResultDto>
GraphService.renameGraphResource(projectInstanceId: string, graphPath: string, expectedRevision: number, newName: string, lifecycleToken: number, operationId: string): Promise<ResourceMutationResultDto>
```

- [ ] **Step 1: Add failing frontend identity and source-audit tests**

Add exact Vitest cases:

```text
projectFilesystemContract > sends projectInstanceId for every active-project read and write
projectFilesystemContract > contains no direct invoke outside services for project filesystem commands
projectFilesystemContract > rejects stale direct results before any frontend side effect
projectFilesystemContract > rejects stale events before correlation or store access
projectFilesystemContract > contains no optional projectInstanceId in active-project service contracts
projectIOStore > rejects an index completion from a replaced project before resetting or hydrating stores
projectIOStore > captures one identity epoch for path databases and index hydration
ResourceEventHandler > ignores stale resource events before index invalidation or path migration
ProjectMutationEventHandler > ignores stale project events before pending-operation correlation
```

Add exact Rust audit test:

```text
project::filesystem::source_audit_tests::production_project_document_io_is_owned_by_filesystem_modules
```

The Rust audit scans production `.rs` files and allows project-document `std::fs`/`trash::delete` calls only in `project/filesystem/`, approved read-only fixture-free scanners, and DuckDB-specific modules. It rejects removed helper names and direct writer symbols.

- [ ] **Step 2: Run frontend and Rust audit tests and verify RED**

```sh
pnpm exec vitest run src/services/project/projectFilesystemContract.test.ts src/features/core/dataStore/projectIOStore.test.ts src/features/application/editor/graphDocumentUnload.test.ts src/features/application/editorMutation/projectPublicationMovePlan.test.ts src/features/core/sync/handlers/ResourceEventHandler.test.ts src/features/core/sync/handlers/ProjectMutationEventHandler.test.ts
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::filesystem::source_audit_tests::production_project_document_io_is_owned_by_filesystem_modules --exact --test-threads=1
```

Expected: frontend payload/epoch assertions and Rust source audit fail on current optional/missing identity or bypassing writers.

- [ ] **Step 3: Implement one frontend identity epoch owner**

Store `projectInstanceId` and monotonically increasing `epoch` together in `projectIOStore`; increment epoch before clearing/hydrating a replacement. `captureProjectIdentity` throws when no active identity exists. Every application workflow captures once before invoke and checks the same snapshot immediately after await and before any side effect.

- [ ] **Step 4: Update thin services and callers**

Make all active-project service methods require identity in TypeScript and send exact camelCase fields. Services remain invoke-only. Update project load/index, graph load/unload/save/create/duplicate/remove/rename, global variable persistence, worksheet operations, flush, save-as, and active deletion callers. Project activation itself takes a path and returns the new identity; it does not require the old identity.

- [ ] **Step 5: Gate direct results and events before all effects**

At the first line after parsing a result/event, compare `projectInstanceId` to the captured/current identity. Only then perform operation correlation, publication-order checks, graph-path migration, projection replacement, store updates, index invalidation, toasts, or navigation. Keep graph state changes exclusively through authoritative projection replacement.

- [ ] **Step 6: Implement source audits and remove stale contracts**

The frontend audit scans production `src/**/*.{ts,tsx}` and rejects project filesystem command names outside `src/services/`, optional `projectInstanceId` declarations for active-project calls, and direct `invoke` in views/features. The Rust audit rejects:

```text
with_project_filesystem_transaction
with_current_project_filesystem_transaction
filesystem_transactions
ProjectFilesystemSnapshot
GraphRenameDiskRollback
save_project_to_file
save_project_graph_to_file
save_project_as_to_directory
duplicate_project_graph_file
remove_project_graph_from_file
cascade_graph_path_references_on_disk
delete_project_directory
```

Allow these strings only inside the audit’s own deny-list. Confirm `get_editor_schema_command`, legacy schema/node-registry/global-type-system stores, and catalog-dependent creation UI are not restored.

- [ ] **Step 7: Run Task 8 GREEN checks**

```sh
pnpm exec vitest run src/services/project/projectFilesystemContract.test.ts src/features/core/dataStore/projectIOStore.test.ts src/features/application/editor/graphDocumentUnload.test.ts src/features/application/editorMutation/projectPublicationMovePlan.test.ts src/features/core/sync/handlers/ResourceEventHandler.test.ts src/features/core/sync/handlers/ProjectMutationEventHandler.test.ts
pnpm typecheck
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::filesystem::source_audit_tests::production_project_document_io_is_owned_by_filesystem_modules --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Expected: explicit frontend files pass, the Rust audit finds no bypass, typecheck/check/format/diff pass. Do not run a full Rust suite, any unqualified frontend suite, or `pnpm verify`.

---

### Task 9: Run focused preflight and the single complete Rust checkpoint

**Files:**
- Modify only if a preflight failure is caused by this slice: the exact failing production file and its focused regression test from Tasks 1–8
- Modify: `.superpowers/sdd/progress.md`
- Modify: `.superpowers/sdd/task-production-backend-report.md`
- No production file is pre-authorized for cleanup or unrelated fixes

**Interfaces:**
- Consumes: completed Tasks 1–8 with no temporary lease/writer path remaining.
- Produces: a recorded focused preflight, one complete Rust-suite result, and accurate implementation ledgers without a commit.

- [ ] **Step 1: Confirm scope and forbidden-path removal before tests**

Run:

```sh
git --no-optional-locks status --short
git --no-pager diff --check
git grep -n -E "with_project_filesystem_transaction|with_current_project_filesystem_transaction|filesystem_transactions|ProjectFilesystemSnapshot|GraphRenameDiskRollback|save_project_as_to_directory|duplicate_project_graph_file|remove_project_graph_from_file|cascade_graph_path_references_on_disk|delete_project_directory" -- src-tauri/src ':!src-tauri/src/project/filesystem/source_audit_tests.rs'
```

Expected: status preserves unrelated dirty files, diff check passes, and grep returns no production matches. If grep finds a migrated compatibility path, return to its owning task and remove it before continuing.

- [ ] **Step 2: Run the final explicit frontend preflight**

```sh
pnpm exec vitest run src/services/project/projectFilesystemContract.test.ts src/features/core/dataStore/projectIOStore.test.ts src/features/application/editor/graphDocumentUnload.test.ts src/features/application/editorMutation/projectPublicationMovePlan.test.ts src/features/core/sync/handlers/ResourceEventHandler.test.ts src/features/core/sync/handlers/ProjectMutationEventHandler.test.ts
pnpm typecheck
```

Expected: all named files and typecheck pass. Do not run `pnpm test`, `pnpm verify:frontend`, or `pnpm verify`.

- [ ] **Step 3: Run serial focused Rust preflight**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::filesystem::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_reads::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::graph_lifecycle::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_activation::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_writers::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::resource_mutations::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::project_lifecycle::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::filesystem::source_audit_tests::production_project_document_io_is_owned_by_filesystem_modules --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Expected: every focused slice passes. Fix slice-caused failures with a new failing focused regression and rerun only that focused filter; do not start the complete suite until the whole preflight is green.

- [ ] **Step 4: Run the complete Rust suite exactly once**

Run once with a generous bounded timeout:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- --test-threads=1
```

Do not run any other complete Rust command. Do not run `pnpm rust:test:sci`, `pnpm verify:rust`, or `pnpm verify`. If this process stalls, times out, or exhausts memory, do not retry it in this task or session; preserve and record the exact exit/termination and final output.

- [ ] **Step 5: Record results and inspect final scope without rerunning Rust**

Update `.superpowers/sdd/progress.md` and `.superpowers/sdd/task-production-backend-report.md` with:

- focused frontend command and result;
- each focused Rust command and result;
- `pnpm typecheck`, `pnpm rust:check`, `pnpm rust:fmt:check`, and `git diff --check` results;
- the single complete Rust command, exit status or termination, and whether it passed, failed, stalled, or exhausted memory;
- confirmation that no complete Rust retry occurred;
- remaining unrelated dirty files and pre-existing failures, if any.

Then run only:

```sh
git --no-optional-locks status --short
git --no-pager diff --stat
git diff --check
```

Confirm no commit was created, no unrelated user change was reverted, `ProjectState.project_data` remains authoritative, no state lock spans lease wait/I/O, no direct/dual writer survives, and the final diff stays within the approved filesystem transaction, lifecycle, identity, tests, and ledger scope.
