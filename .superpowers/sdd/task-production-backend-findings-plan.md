# Production Backend Findings Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix only backend review Critical 1, Critical 4, and Important 3 with regression coverage and explicit revisioned IPC contracts.

**Architecture:** Keep `ProjectState.project_data` authoritative. Rename and duplicate normalize every in-memory and persisted path identity; schema v3 is the sole accepted graph format; function signatures and history commands use `MutationRequest`, with history requests guarding one anchor revision while atomically applying the complete project transaction. Graph patches continue to use `GraphDeltaEvent`; function/history mutations return and emit one atomic synchronization envelope containing all `ResourceDeltaEvent` values and graph projection replacements.

**Tech Stack:** Rust, serde, Tauri commands/events, pnpm/Cargo test scripts.

## Global Constraints

- Do not change statistics kernels, runtime variable execution, or function-plan publication.
- Do not update the current frontend contract; backend DTOs must be explicit and serializable.
- Preserve unrelated user changes and do not commit.
- Add focused regression tests before implementation changes.
- Run all validation sequentially with `CARGO_BUILD_JOBS=1` for Rust commands.

---

### Task 1: Normalized rename and duplicate identity

**Files:**
- Modify: `src-tauri/src/project/production_tests.rs`
- Modify only if RED proves necessary: `src-tauri/src/project/project_state.rs`
- Modify only if RED proves necessary: `src-tauri/src/project/project_io.rs`

**Interfaces:**
- Consumes: `ProjectState::rename_graph_resource`, `ProjectState::duplicate_graph_resource`.
- Produces: loaded caller references and loaded local-variable scopes rewritten to the renamed path; duplicated function Entry/Return managed `function` parameters rebound to the duplicate path.

- [ ] Extend `function_duplicate_rebinds_self_identity_and_loaded_rename_is_authoritative` with a loaded function-scoped `VariableInstance` and assertions that its scope changes from the old function path to the renamed function path.
- [ ] Run the exact focused test and confirm RED if the invariant is not already satisfied.
- [ ] Make the minimal authority/persistence change needed; do not alter runtime variable behavior.
- [ ] Re-run the exact test and confirm PASS.

### Task 2: Strict graph schema v3

**Files:**
- Modify: `src-tauri/src/project/project_io.rs`

**Interfaces:**
- Consumes: persisted `GraphDocument { schema_version, kind, function, ... }`.
- Produces: schema version `3`; `Function` requires `Some(FunctionDocument)` and `Event` requires `None` at full-load and index-header boundaries.

- [ ] Verify focused tests cover missing function metadata, event-with-function metadata, and older schema rejection.
- [ ] Run `project::project_io::tests::production_graph_io_rejects_function_shape_mismatches` and confirm current behavior.
- [ ] If required, centralize strict validation so both `read_graph_document` and `read_graph_file_header` reject mismatches.
- [ ] Re-run strict-shape and legacy-schema focused tests sequentially.

### Task 3: Revisioned function/history mutation synchronization

**Files:**
- Modify: `src-tauri/src/node_system/document/history.rs`
- Modify: `src-tauri/src/node_system/document/mod.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Modify: `src-tauri/src/commands/command_node_system.rs`
- Modify: `src-tauri/src/event/event_project.rs`
- Modify: `src-tauri/src/project/production_tests.rs`

**Interfaces:**
- Add: `HistoryMutation` as an explicit empty serde payload for `MutationRequest<HistoryMutation>`.
- Change: `ProjectState::undo_last_transaction(request)` and `redo_last_transaction(request)` validate `request.resource` at `request.base_revision`, use `request.operation_id` for emitted deltas, then atomically apply the complete history transaction.
- Add: `GraphProjectionReplacementDto { graph_path, projection }`.
- Add: `ResourceMutationResultDto { deltas, projection_replacements }` returned by signature/undo/redo commands and emitted as one project event.

- [ ] Add focused tests proving stale anchor revisions reject undo and redo without mutation, successful undo/redo return all transaction deltas with the request operation ID, and function signature stale revisions conflict while successful updates return a delta.
- [ ] Run the focused tests and confirm RED.
- [ ] Implement anchor revision lookup and request validation before history application while retaining complete transaction atomicity.
- [ ] Replace signature/history command results with the explicit synchronization envelope and emit the same envelope once as an atomic event.
- [ ] Re-run focused tests and confirm PASS.

### Task 4: Sequential verification and report

**Files:**
- Modify: `.superpowers/sdd/task-production-backend-report.md`

- [ ] Run `CARGO_BUILD_JOBS=1 pnpm rust:check`.
- [ ] Run the focused normalized identity test.
- [ ] Run the focused strict schema tests.
- [ ] Run the focused history/signature revision and delta tests.
- [ ] Run `pnpm rust:fmt:check`.
- [ ] Run `git --no-pager diff --check`.
- [ ] Append exact remediation scope, backend DTO contract, TDD evidence, sequential command results, untouched findings, and no-commit status to the report.
