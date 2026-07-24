# Production backend cut report

## Status

**DONE_WITH_CONCERNS**

Backend production authority has been switched from the legacy `GraphInstance` command/persistence/execution entry points to the normalized `node_system` document/compiler/runtime path. The required focused checks pass. Two production coverage concerns remain under **Unfinished blockers and concerns**.

## Implemented changes

### Project authority and storage

- `ProjectData.graphs` now stores `GraphResourceDocument`, whose graph body is `node_system::document::GraphDocument`; it no longer stores `graph::GraphInstance`.
- `ProjectState` owns normalized graph insertion, loading, revisioned patch application, Rust-authoritative undo/redo, localized projection, compile/staleness checks, and plan-only execution.
- `ProjectState::insert_graph` is the single in-memory insertion path used by production loading/insertion.
- Project reload/clear resets `ProjectHistory` and drains project-scoped runs before replacing state.
- Graph compilation snapshots the normalized document, variables, database declarations, and immutable runtime handles under short locks. Compilation, localization projection construction, event emission, and execution occur after locks are released.
- Resource versions are derived outside locks from cloned authoritative variable/database documents. Runtime snapshots contain variables and already-loaded in-memory DataFrames.

### Project store

- Replaced the legacy mutable `graph::register::NodeRegistry` field with immutable production assets:
  - `node_system::registry::NodeRegistry`
  - built-in localized catalog
  - production kernel registry
  - compiled parameter store ownership
  - function plan store ownership
  - new result store
  - project run registry/session identity
- Built-in registration is validated and frozen through `NodeRegistryBuilder`.

### Persistence

- Graph file schema is now version `2`.
- Graph files serialize resource metadata (`kind`, path-derived `name`) plus normalized `document` and scoped local variables.
- Fixed protocol ports are not persisted; documents contain stable `PortAddress` references only.
- Schema-v1/legacy graph files are rejected with an unsupported-schema error; no converter, fallback reader, alias, or dual format remains.
- Project/database/worksheet support remains in the project IO module; graph-specific legacy scanning and pin persistence were removed.

### Mutation and history

- Production mutation IPC accepts `MutationRequest<GraphDocumentPatch>`.
- Base revision mismatches return a structured `graph_revision_conflict` application error.
- Successful mutations are applied through `ProjectHistoryTransaction`, advance monotonic revisions, and emit a `GraphDelta` project event after the project lock is released.
- Undo/redo operate on Rust-owned project history and preserve document identities.
- No frontend snapshot is accepted as a replacement graph authority.

### Projection and catalog

- Project graph loading/hydration returns localized `EditorGraphProjectionDto` rather than `GraphInstanceDTO`/`PinInstanceDTO`.
- Added localized catalog IPC backed by the frozen `node_system` registry/catalog.
- Projection compilation uses the real graph resource path and a resource-version snapshot.

### Execution

- Added production execution IPC using a Tauri channel of new `RunEvent` values.
- Execution:
  1. snapshots normalized document/project resources and immutable runtime assets under short locks;
  2. compiles with `GraphCompiler` using the real graph resource path;
  3. rejects blocking diagnostics;
  4. rejects stale graph, registry, or resource bases;
  5. builds run-local compiled constant parameters;
  6. executes only through `RunExecutor`;
  7. publishes through the new `ResultStore` and run event sink;
  8. releases run resources through RAII cleanup.
- Runtime execution does not query Registry, i18n, display roles, or editor state.

## Main files added or changed

### Production wiring

- `src-tauri/src/lib.rs`
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/commands/command_node_system.rs`
- `src-tauri/src/commands/command_project/lifecycle.rs`
- `src-tauri/src/commands/command_project/mod.rs`
- `src-tauri/src/commands/command_project/query.rs`
- `src-tauri/src/commands/command_project/registry.rs`
- `src-tauri/src/event/event_project.rs`

### Project authority, IO, and tests

- `src-tauri/src/project/mod.rs`
- `src-tauri/src/project/project_data.rs`
- `src-tauri/src/project/project_io.rs`
- `src-tauri/src/project/project_state.rs`
- `src-tauri/src/project/project_state_variable.rs`
- `src-tauri/src/project/project_store.rs`
- `src-tauri/src/project/production_tests.rs`

### Node-system production assets

The current production cut includes the new modules under:

- `src-tauri/src/node_system/protocol/`
- `src-tauri/src/node_system/registry/`
- `src-tauri/src/node_system/document/`
- `src-tauri/src/node_system/analysis/`
- `src-tauri/src/node_system/compiler/`
- `src-tauri/src/node_system/plan/`
- `src-tauri/src/node_system/catalog/`
- `src-tauri/src/node_system/runtime/`
- `src-tauri/src/node_system/testing/`

Notable production runtime additions include:

- `src-tauri/src/node_system/runtime/artifact.rs`
- `src-tauri/src/node_system/runtime/execution_event.rs`
- `src-tauri/src/node_system/runtime/function_plan.rs`
- `src-tauri/src/node_system/runtime/project_resource.rs`
- `src-tauri/src/node_system/runtime/project_run.rs`
- `src-tauri/src/node_system/runtime/result_store.rs`
- `src-tauri/src/node_system/runtime/production_tests.rs`

### Legacy-domain isolation edits

- `src-tauri/src/execution/context/node_execution_context.rs`
- `src-tauri/src/execution/engine/executor/mod.rs`
- `src-tauri/src/graph/core/graph_runtime.rs`
- `src-tauri/src/graph/core/graph_instance/lifecycle.rs`
- `src-tauri/src/graph/core/graph_instance/persistence.rs`
- `src-tauri/src/schema/history.rs`
- `src/features/core/sync/types.ts`
- `src/services/graph/graphService.ts`

## Deleted legacy production entries

### Tauri commands

- `src-tauri/src/commands/command_execution/`
- `src-tauri/src/commands/command_graph/`
- `src-tauri/src/commands/command_schema.rs`
- `src-tauri/src/commands/command_resource.rs`
- `src-tauri/src/commands/command_project/types.rs`

This removes the registered old executor, graph snapshot/history commands, node/pin-ID mutation commands, old schema catalog command, and `GraphInstanceDTO` graph hydrate boundary.

### Project graph/execution modules

- `src-tauri/src/project/execution_cancel.rs`
- `src-tauri/src/project/execution_graph_bundle.rs`
- `src-tauri/src/project/function_call_site_index.rs`
- `src-tauri/src/project/function_signature_table.rs`
- `src-tauri/src/project/graph_events.rs`
- `src-tauri/src/project/project_execution.rs`
- `src-tauri/src/project/project_state_graph.rs`
- `src-tauri/src/project/project_state_graph_mut.rs`

### Obsolete GraphInstance integration tests

- `src-tauri/tests/common/`
- `src-tauri/tests/function_call_test.rs`
- `src-tauri/tests/logic_test.rs`
- `src-tauri/tests/shell_node_test.rs`
- `src-tauri/tests/type_convert_test.rs`

These tests exercised the deliberately removed display-name/pin/GraphInstance production architecture. Replacement coverage lives in `node_system` tests and `project/production_tests.rs`.

## TDD evidence

### RED: project persistence

Command:

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib project::project_io::tests::production_graph_file_uses_normalized_document_without_persisted_pins -- --exact --nocapture
```

Observed expected failure before implementation:

```text
assertion failed: schemaVersion left Number(1), right Number(2)
```

### RED: project history integration

The focused mutation test initially failed to compile because production `ProjectState::undo_last_transaction` did not exist. The implementation added project-level history application and undo/redo without restoring legacy graph APIs.

### RED: production execution

Command:

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib project::production_tests::project_execution_runs_valid_plan_through_run_executor -- --exact --nocapture
```

Observed expected failure before parameter wiring:

```text
operation 0 failed: compiled parameter store is unavailable for 'node.<uuid>'
```

The GREEN implementation builds a run-local compiled parameter snapshot outside project locks and injects it into `RunExecutor`.

## Final sequential verification

All test commands used `CARGO_BUILD_JOBS=1` and were run sequentially. `pnpm rust:test` was not used because it compiles every integration target; the focused `pnpm exec cargo test ... --lib` form keeps validation scoped as required.

1. Rust check

```text
CARGO_BUILD_JOBS=1 pnpm rust:check
```

Result: **PASS**

```text
Finished `dev` profile ... in 6.28s
```

2. Focused project IO tests

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib project::project_io::tests::production_graph_io -- --nocapture
```

Result: **PASS** — 2 passed

- normalized graph round-trip and fixed ports absent
- legacy graph schema rejection

3. Focused project graph mutation test

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib project::production_tests::project_mutation_rejects_stale_revision_and_records_undo_history -- --exact --nocapture
```

Result: **PASS** — 1 passed

4. Focused projection hydrate test

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib project::production_tests::project_projection_hydrates_localized_editor_dto -- --exact --nocapture
```

Result: **PASS** — 1 passed

5. Focused production execution tests

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib project::production_tests::project_execution -- --nocapture
```

Result: **PASS** — 2 passed

- blocking analysis refusal
- valid plan execution through `RunExecutor`

6. Focused resource cleanup test

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib node_system::runtime::tests::successful_run_releases_all_resources -- --exact --nocapture
```

Result: **PASS** — 1 passed

7. Rust formatting

```text
pnpm rust:fmt:check
```

Result: **PASS**

8. Diff whitespace validation

```text
git --no-pager diff --check
```

Result: **PASS** with Windows checkout line-ending notices only; no whitespace errors.

## Unfinished blockers and concerns

No compile or focused-test blocker remains.

Concerns retained for explicit follow-up:

1. `FunctionPlanStore` is now production-owned, but this focused cut does not include an end-to-end project test that publishes and executes a persisted function graph through that store. The removed GraphInstance function-call tests are not valid coverage for the new architecture; a normalized FunctionDocument/function-plan production fixture should be added in the frontend/function-resource phase.
2. Runtime resource snapshots include all authoritative variable values and resource versions, plus DataFrames already in `DatabaseState::Loaded`. Project DuckDB resources remain lazy and are not materialized while holding project locks; an end-to-end relational/DuckDB plan test should verify the selected relational backend acquires those resources through its intended lazy backend path.
3. Full `pnpm rust:test` was intentionally not run because the task and `AGENTS.md` require focused Rust tests by default. Unrelated database/science integration suites were not validated in this task.
4. `git diff --check` reports informational LF→CRLF checkout warnings on Windows. It reports no whitespace errors.

## Commit state

No commit was created.

---

# Production backend review remediation

## Status

**DONE**

The four Critical and four Important review findings have been remediated without restoring legacy graph commands, snapshot authority, fallback readers, or dual graph formats.

## Review finding remediation

### Critical 1 — Normalized production graph lifecycle

- Added normalized create event/function, remove, unload, save, duplicate, rename, and function-signature application paths on `ProjectState`.
- Registered the replacement Tauri commands under their production command names.
- Creation and duplication call `ProjectState::insert_graph`; persisted function resources are also loaded through that insertion path before execution.
- Function resources now persist their normalized `FunctionDocument` signature with the normalized graph document.
- Function creation seeds normalized Entry/Return shell nodes, stable resource parameters, and the default control connection without persisted fixed ports.
- Rename cascades stable graph-path references and removes the old file instead of retaining aliases.

Focused test:

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib project::production_tests::normalized_graph_lifecycle_routes_every_insert_through_project_state -- --exact --nocapture
```

Result: **PASS** — 1 passed.

RED evidence: the test initially failed to compile because all six normalized lifecycle state methods were absent.

### Critical 2 — Production runtime adapters

- Replaced project-variable get/set error kernels with run-scoped resource adapters over authoritative project variables. The compiler now lowers normalized `Resource` parameters into shared/exclusive plan requirements.
- Replaced all DataFrame adapter-error kernels with materialized protocol-value implementations for project sources, decomposition/composition, filtering, selection, ranges, aggregates, comparisons, standardization, inverse standardization, time-series transforms, and panel transforms.
- Replaced all statistics adapter-error kernels with executable configuration, fit, prediction, test, and summary paths over materialized protocol series/model values.
- Added a production relational backend for source/project/filter/rename/limit/union plans and inject it into `RunExecutor`.
- Added a run-scoped production plot sink resource.
- `execute_graph` now loads persisted function resources, compiles them with built-in interface resolvers, updates the function basis, publishes current plans, and supplies their compiled parameters before running the caller.
- No production runtime kernel retains an `adapter required` error implementation.

Focused tests:

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib project::production_tests::project_execution_publishes_persisted_function_plans -- --exact --nocapture
```

Result: **PASS** — persisted function graph loaded, published, called, and executed.

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib node_system::runtime::builtin_tests::dataframe_integer_range_executes_through_production_registry -- --exact --nocapture
```

Result: **PASS** — DataFrame family execution returned the expected series.

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib node_system::runtime::builtin_tests::statistics_fit_executes_instead_of_returning_an_adapter_error -- --exact --nocapture
```

Result: **PASS** — fit returned model, fitted values, and residuals.

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib project::production_tests::project_variable_get_executes_against_authoritative_resource -- --exact --nocapture
```

Result: **PASS** — production variable resource acquired and executed.

RED evidence: this test first failed with `bound project variable resource is unavailable`; compiler resource lowering fixed the root cause.

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib project::production_tests::production_relational_backend_executes_project_dataframe_source -- --exact --nocapture
```

Result: **PASS** — production relational source and limit executed over a project DataFrame.

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib project::production_tests::production_resource_snapshot_supplies_plot_sink -- --exact --nocapture
```

Result: **PASS** — plot resource acquisition and publication succeeded.

### Critical 3 — Project-session replacement race

- Added pre-run registrations to `ProjectRunRegistry`.
- `execute_graph` registers before compilation starts and uses the same cancellation token for compilation-to-run handoff.
- Project drain now cancels and waits for both pre-run compilation leases and active runs.
- Post-compile validation also compares the current project session before run registration.

Focused test:

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib node_system::runtime::project_run::tests::project_drain_cancels_and_waits_for_pre_run_compilation -- --exact --nocapture
```

Result: **PASS** — drain cancelled compilation and remained blocked until the pre-run lease was released.

RED evidence: the test initially failed to compile because `track_pre_run` did not exist.

### Critical 4 — Result-source IPC and bounded ownership

- Registered descriptor, value, page, single-source release, and run-source release commands over the normalized `ResultStore`.
- Added a hard production source capacity; publishing beyond it evicts the oldest source and releases its artifact hold.
- Explicit per-source and per-run release remain available for deterministic client cleanup.

Focused tests:

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib node_system::runtime::result_store::tests::descriptor_page_and_release_replace_result_source_store_reads -- --exact --nocapture
```

Result: **PASS** — descriptor/page/release lifecycle works.

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib node_system::runtime::result_store::tests::result_sources_evict_oldest_entry_at_capacity -- --exact --nocapture
```

Result: **PASS** — retained source count remained bounded and the oldest artifact hold was released.

RED evidence: the capacity test initially failed to compile because bounded construction did not exist.

### Important 1 — Unsupported graph schema at project index boundary

- Project graph-index header failures now propagate.
- Schema-version and resource-kind mismatches return explicit project-format errors.
- Local-variable indexing no longer hides unsupported or malformed graph documents.

Focused test:

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib project::project_io::tests::production_project_index_rejects_legacy_graph_schema -- --exact --nocapture
```

Result: **PASS** — project indexing rejected schema version 1.

RED evidence: the corrected test observed `read_project_index` returning `Ok` with an empty graph list before the fix.

### Important 2 — Production undo/redo reachability

- Registered normalized undo and redo Tauri commands.
- Function signatures now participate in the same Rust `ProjectHistory` authority as graph patches.
- Undo/redo restore normalized graph and function documents while advancing revisions.

Focused tests:

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib project::production_tests::normalized_function_signature_update_is_undoable -- --exact --nocapture
```

Result: **PASS** — normalized function signature update and undo use one authority.

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib project::production_tests::project_mutation_rejects_stale_revision_and_records_undo_history -- --exact --nocapture
```

Result: **PASS** — revision conflict and graph undo remain correct.

RED evidence: the signature test initially failed to compile because the normalized state update method did not exist.

### Important 3 — Obsolete graph DTO/event production references

- Removed legacy Event/Function/Node/Connection event variants and modules from the production event boundary.
- Removed the obsolete variable-to-pin-inference event path.
- Legacy `execution`, `graph`, and `schema` internals are no longer public crate exports; remaining private compilation is limited to non-node database/scientific/shared-value dependencies.
- Static review found no `GraphInstanceDTO`, `PinInstanceDTO`, `EventEvent`, `EventFunction`, `EventNode`, or `EventConnection` references under `src-tauri/src/event/`.

Focused validation: `CARGO_BUILD_JOBS=1 pnpm rust:check` — **PASS**.

### Important 4 — Behavioral replacement coverage

Added focused production behavior coverage for:

- normalized create/save/load/unload/duplicate/rename/remove lifecycle;
- persisted function-plan publication and call execution;
- project variable resource execution;
- DataFrame and statistics kernel execution;
- production relational backend execution;
- plot-sink resource injection;
- project replacement during pre-run compilation;
- result descriptor/page/release and bounded retention;
- shell-node scope and singleton enforcement.

Shell-focused command:

```text
CARGO_BUILD_JOBS=1 pnpm exec cargo test --manifest-path src-tauri/Cargo.toml -p yssbi --lib project::production_tests::production_compiler_rejects_wrong_scope_and_duplicate_shell_nodes -- --exact --nocapture
```

Result: **PASS** — wrong-scope and duplicate managed shell nodes produced blocking diagnostics.

## Final sequential verification after remediation

All Rust tests below were run one at a time with `CARGO_BUILD_JOBS=1`; no test commands were run concurrently.

1. `CARGO_BUILD_JOBS=1 pnpm rust:check` — **PASS**, finished in 0.66s.
2. Normalized graph round-trip/fixed-port absence — **PASS**, 1 passed.
3. Direct legacy graph schema rejection — **PASS**, 1 passed.
4. Project-index legacy schema rejection — **PASS**, 1 passed.
5. Normalized graph lifecycle — **PASS**, 1 passed.
6. Normalized function signature/history — **PASS**, 1 passed.
7. Graph revision conflict/history — **PASS**, 1 passed.
8. Localized projection hydrate — **PASS**, 1 passed.
9. Persisted function plan/call execution — **PASS**, 1 passed.
10. Shell scope/singleton validation — **PASS**, 1 passed.
11. DataFrame production kernel — **PASS**, 1 passed.
12. Statistics production kernel — **PASS**, 1 passed.
13. Project variable resource execution — **PASS**, 1 passed.
14. Relational backend execution — **PASS**, 1 passed.
15. Plot sink resource — **PASS**, 1 passed.
16. Blocking analysis refusal — **PASS**, 1 passed.
17. Valid plan execution — **PASS**, 1 passed.
18. Pre-run project drain race — **PASS**, 1 passed.
19. Result descriptor/page/release — **PASS**, 1 passed.
20. Bounded result retention — **PASS**, 1 passed.
21. Generic run resource cleanup — **PASS**, 1 passed.
22. `pnpm rust:fmt:check` — **PASS**.
23. `git --no-pager diff --check` — **PASS** with Windows LF→CRLF notices only; no whitespace errors.

## Remaining blockers and concerns

None for the reviewed backend production cut. Full `pnpm rust:test` remains intentionally out of scope under `AGENTS.md`; focused tests and `rust:check` are the required default workflow.

## Commit state after remediation

No commit was created.

## Post-report verification refresh

After the report append and final comment/test visibility cleanup:

1. `pnpm rust:fmt:check` — **PASS**.
2. `CARGO_BUILD_JOBS=1 pnpm rust:check` — **PASS**, finished in 5.84s.
3. `git --no-pager diff --check` — **PASS** with informational Windows LF→CRLF notices only and no whitespace errors.

## Critical 3 follow-up — compiled variable snapshot and effect commit

- Runtime variable access is bound to the immutable `ProjectResourceSnapshot`; reads never retain a live `ProjectData` handle.
- Variable-set kernels record typed `VariableWriteEffect` values containing the snapshot resource revision and do not mutate project authority.
- Successful runs revalidate the project session and every variable revision before applying one `ProjectHistoryTransaction` of `VariableDocumentPatch` changes.
- The commit returns authoritative `ResourceDeltaEvent` values, advances revisions, remains undoable, and the command boundary emits `ResourceMutationCommitted` plus the existing variable projection event.
- Run errors and cancellation return before the application commit path, so pending effects are discarded. Session/revision conflicts are represented by `VariableEffectCommitError` and do not partially apply a transaction.

TDD and focused sequential verification:

1. RED: `project::production_tests::variable_effect_commit_is_revisioned_and_undoable` initially failed to compile because session validation, delta results, and structured commit errors did not exist.
2. `node_system::runtime::production_tests::variable_reads_stay_on_the_snapshot_and_writes_become_effects` — **PASS**, proving a write remains pending while subsequent reads retain the captured value.
3. `project::production_tests::variable_effect_commit_is_revisioned_and_undoable` — **PASS**, proving success advances revision and records undo history.
4. `project::production_tests::concurrent_variable_effect_commit_returns_structured_revision_conflict` — **PASS**, proving a winning commit makes the stale effect fail without overwriting authority.
5. `CARGO_BUILD_JOBS=1 pnpm rust:check` — **PASS**, finished in 10.56s.
6. `project::production_tests::project_variable_get_executes_against_authoritative_resource` — **BLOCKED BY EXISTING FIXTURE** before variable execution: `execute_graph` returned `项目尚未加载` because the test does not establish a project path.

No statistics, DuckDB, persistence schema, or schema-version code was changed. No commit was created.

---

# Targeted backend re-review remediation

## Scope and status

**TARGETED_FINDINGS_DONE**

This pass addresses only re-review Critical 1, Critical 4, and Important 3. It does not change statistics kernels, runtime variable execution/snapshot behavior, or function-plan publication. The current frontend service contract is intentionally unchanged for the later frontend task.

## Critical 1 — normalized rename and duplicate identity

- Rename stages the renamed resource together with all loaded caller documents and loaded variable scopes, then replaces `ProjectData` with the remapped authoritative snapshot after persistence succeeds.
- Function duplication rewrites normalized self-path parameters, including managed Entry/Return `function` bindings, before writing the duplicate.
- Regression coverage now proves all three required cases in one production lifecycle fixture: loaded caller rewrite, loaded function-local variable scope rewrite, and duplicate managed self-path rebinding.

The additional loaded-variable assertion passed immediately because the current mixed workspace already contained the corresponding authority fix; no further production change was made for this finding in this pass.

## Critical 4 — one strict function-resource schema

- `SCHEMA_VERSION` is `3`; older graph documents are rejected.
- Schema v3 requires `Function => Some(FunctionDocument)` and `Event => None` at both full graph loading and project-index header reading.
- Missing function metadata and Event documents carrying function metadata are rejected rather than accepted as alternate v3 shapes.

The strict mismatch and old-schema focused tests were already GREEN in the current mixed workspace and were retained as regression coverage.

## Important 3 — revisioned history/signature synchronization

### Mutation requests

`update_function_signature`, `undo_graph_document`, and `redo_graph_document` now accept revisioned mutation envelopes. Undo/redo use:

```text
MutationRequest<HistoryMutation> {
  resource,
  baseRevision,
  operationId,
  payload: {}
}
```

The resource is only the concurrency anchor. Its current revision must equal `baseRevision`; a stale anchor returns `history_revision_conflict` without consuming or applying the history entry. A successful request still applies the complete project history transaction atomically under the project write transaction. Every returned delta uses the undo/redo request's `operationId`, not the original history operation ID.

Function signature writes continue to use `MutationRequest<FunctionDocumentPatch>` and return `function_revision_conflict` on a stale base revision.

### Atomic backend result/event DTO

Function signature, undo, and redo return the same explicit DTO that is emitted once through `ResourceMutationCommitted`:

```text
ResourceMutationResultDto {
  deltas: ResourceDeltaEvent[],
  projectionReplacements: GraphProjectionReplacementDto[]
}

GraphProjectionReplacementDto {
  graphPath: string,
  projection: EditorGraphProjectionDto
}
```

All transaction deltas and all graph/function resources touched by those deltas are collected before the single event is emitted. Projection replacements are built from committed authoritative state and deduplicated by `graphPath`. Ordinary graph document patches continue to return and emit `GraphDeltaEvent<GraphDocumentPatch>`.

## TDD evidence

### RED: revisioned signature/history request

The new focused test initially failed to compile as expected:

```text
unresolved import `HistoryMutation`
this method takes 0 arguments but 1 argument was supplied
expected `HistoryError`, found `MutationConflict`
```

The GREEN implementation added the explicit history payload, anchor revision validation, request operation-ID propagation, and revisioned undo/redo state APIs.

### RED: atomic mutation result wire contract

The wire-contract test initially failed to compile as expected:

```text
cannot find struct, variant or union type `ResourceMutationResultDto`
```

The GREEN implementation added the shared result/replacement DTOs and replaced per-delta event emission with one atomic `ResourceMutationCommitted` event.

## Final sequential verification

All Cargo commands used one build job. Focused tests were run one command at a time with `--test-threads=1`.

1. `CARGO_BUILD_JOBS=1 pnpm rust:check` — **PASS**, finished in 15.83s.
2. Loaded caller/local-variable rename and function duplicate self-path — **PASS**, 1 passed.
3. Strict function/Event schema mismatch rejection — **PASS**, 1 passed.
4. Legacy graph schema rejection — **PASS**, 1 passed.
5. Signature/undo/redo revision conflict and delta behavior — **PASS**, 1 passed.
6. Atomic result DTO wire fields — **PASS**, 1 passed.
7. Committed projection replacement — **PASS**, 1 passed.
8. Cross-resource function/caller history atomicity — **PASS**, 1 passed.
9. Existing graph mutation conflict/history regression — **PASS**, 1 passed.
10. Existing normalized function signature undo regression — **PASS**, 1 passed.
11. Existing variable-effect history regression — **PASS**, 1 passed.
12. `pnpm rust:fmt:check` — **PASS**.
13. `git --no-pager diff --check` — **PASS** with informational Windows LF→CRLF notices only; no whitespace errors.

## Commit state

No commit was created.

---

# Targeted execution concurrency remediation

## Scope and status

**TARGETED_FINDINGS_DONE**

This pass addresses only re-review Important 1 and Important 2. It does not modify statistics, variable, schema, or history behavior.

## Important 1 — pre-run protection and drain cancellation

- `execute_graph` snapshots the current project session and registers its pre-run lease before obtaining the project path or performing function index/file loading.
- Function loading checks the lease's runtime cancellation token before index I/O, around each function file load, and immediately before insertion into `ProjectData`.
- The `CompileCancellationToken` is constructed from the same shared cancellation flag registered in `ProjectRunRegistry`; project drain therefore reaches `GraphCompiler` checkpoints instead of only waiting for compilation to finish.
- A test-only loading checkpoint makes the replacement race deterministic without adding a production delay or alternate loading path.

Regression coverage:

1. `project_replacement_during_function_loading_cancels_before_old_resource_insert` pauses after old-project function file I/O, replaces the project concurrently, observes drain cancellation, and proves the old function is not inserted into the replacement authority.
2. `project_drain_cancels_graph_compiler_with_the_pre_run_token` drains a registered pre-run lease and passes its shared cancellation source into a real `GraphCompiler::compile_snapshot` call, which returns cancellation while drain still waits for lease release.

## Important 2 — run-local immutable function plan generations

- `FunctionPlanStore` is configuration/factory state only; it contains no project-global mutable basis or plan map.
- `generation` validates every entry and returns one complete immutable `FunctionPlanGeneration` containing its basis and complete plan map.
- Each `execute_graph` call retains its own generation and supplies it directly to `RunExecutor`, so another execution cannot clear or partially replace its function plans.
- `concurrent_function_plan_publication_and_calls_keep_run_local_generations` starts two concurrent generations for different versions of the same function and repeatedly resolves calls from both while yielding between reads; each run continues to observe only its own complete version.

## TDD evidence

The replacement test initially failed to compile because the deterministic test checkpoint was not exposed to the sibling production-test module (`Arc` import/type inference and method visibility). After correcting only the test seam, the behavioral test passed against the protected loading implementation. The compiler-drain and concurrent generation tests then passed against the existing shared-token and immutable-generation implementation in the mixed workspace; no additional production synchronization mechanism was required.

## Sequential verification

All Cargo commands used `CARGO_BUILD_JOBS=1` and were run one at a time.

1. Project replacement during function loading — **PASS**, 1 passed (61.00s build, 0.19s test).
2. GraphCompiler cancellation during project drain — **PASS**, 1 passed (0.05s test).
3. Concurrent function plan publication/call — **PASS**, 1 passed (0.00s test).
4. `pnpm rust:fmt:check` — **PASS**.
5. `CARGO_BUILD_JOBS=1 pnpm rust:check` — **PASS**, finished in 6.26s.
6. Existing persisted function plan/call execution regression — **PASS**, 1 passed (0.09s test).
7. `git --no-pager diff --check` — **PASS** with informational Windows LF→CRLF notices only; no whitespace errors.

## Commit state

No commit was created.
