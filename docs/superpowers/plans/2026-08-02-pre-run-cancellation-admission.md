# Pre-run Cancellation Admission Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Register graph execution with the current project run registry before function resource loading so project activation can cancel and drain the complete preparation lifecycle.

**Architecture:** Reuse `ProjectRunRegistry::track_pre_run` as the single admission and cancellation authority. Capture the registry, session, and trace sink under one short `project_store` read lock, register the token before releasing that lock, and retain the RAII registration through loading, compilation, execution, and finalization.

**Tech Stack:** Rust, `ProjectState`, `ProjectRunRegistry`, RAII lifecycle guards, focused serial Cargo tests through pnpm.

## Global Constraints

- Work directly on `shadcn`; no worktree, branch, or commit.
- Preserve all unrelated dirty work, especially `.gitignore`, `TODO.md`, observability, Static Catalog, structured-control, and relational slices.
- Rust remains authoritative for project lifecycle, cancellation, resource loading, and execution.
- Keep `ProjectRunRegistry` as the only run/pre-run lifecycle authority; do not add a second registry or generation polling.
- Hold `project_store` only for snapshot and admission; never hold it during filesystem I/O, compilation, waits, or execution.
- Preserve existing run-scoped cancellation, active registration, finalization, and result publication behavior.
- Use the two existing failing production tests as the TDD RED contract; do not weaken their assertions.
- Run focused serial Rust tests only; do not rerun the known-red complete Rust suite.
- After independent review, update only the relevant rows in `TODO.md` under `## node_architecture 进度`.

---

## File Structure

- Modify `src-tauri/src/project/project_state.rs` only for `ProjectState::execute_graph` admission ordering.
- Reuse existing tests in `src-tauri/src/project/production_tests.rs` and `src-tauri/src/project/project_activation.rs`; no new fixture is needed because both tests already fail for the exact missing behavior.
- Create `.superpowers/sdd/2026-08-02-pre-run-cancellation-admission/progress.md` as the durable task ledger.
- Modify `TODO.md` only after implementation and independent review are complete.

### Task 1: Admit execution before function resource loading

**Files:**
- Modify: `src-tauri/src/project/project_state.rs:4932-5061`
- Verify: `src-tauri/src/project/production_tests.rs:2507-2579`
- Verify: `src-tauri/src/project/project_activation.rs:267-324`
- Create: `.superpowers/sdd/2026-08-02-pre-run-cancellation-admission/progress.md`
- Modify after review: `TODO.md` under `## node_architecture 进度`

**Interfaces:**
- Consumes: `ProjectRunRegistry::track_pre_run(ProjectSessionId, CancellationToken) -> Result<ProjectPreRunRegistration<'_>, ProjectRunRegistrationError>`.
- Produces: one `ProjectPreRunRegistration` whose lifetime covers function loading, compilation, runtime setup, execution, and success finalization.
- Preserves: `ProjectPreRunRegistration::begin_finalization`, `RunExecutor::with_run_registry`, and the existing cancellation token shared with compile/runtime operations.

- [ ] **Step 1: Record the existing RED evidence in the ledger**

Create the ledger with the plan identity, constraints, and the observed failures:

```markdown
# SDD ledger — plan: docs/superpowers/plans/2026-08-02-pre-run-cancellation-admission.md

Execution constraints:
- Directly use shadcn; no worktree/branch/commit.
- Preserve unrelated dirty work and use the existing ProjectRunRegistry only.
- Focused serial Rust tests only; no complete Rust suite.
- Update TODO.md node_architecture progress only after reviewed completion.

Task 1: RED — project replacement test failed because function-load checkpoint observed an uncancelled token
Task 1: RED — activation deadlock test failed for the same pre-load registration gap
```

- [ ] **Step 2: Preserve the exact RED commands and expected failures**

Run serially if fresh RED evidence is not already available in the controller session:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::project_replacement_during_function_loading_cancels_before_old_resource_insert -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_activation::tests::activation_and_pre_run_function_loading_complete_without_deadlock -- --exact --test-threads=1
```

Expected: each runs one test and fails because the function-load checkpoint sees `cancellation.is_cancelled() == false`.

- [ ] **Step 3: Move pre-run admission before function resource loading**

In `ProjectState::execute_graph`, replace the initial token/load/session capture sequence with this ordering:

```rust
let cancellation = crate::node_system::runtime::CancellationToken::new();
let store = self.project_store.read().unwrap();
let runs = Arc::clone(&store.runs);
let session_id = store.project_session_id.clone();
let trace_sink = Arc::clone(&store.trace_sink);
let pre_run = runs
    .track_pre_run(session_id.clone(), cancellation.clone())
    .map_err(|error| error.to_string())?;
drop(store);

self.load_function_resources(&cancellation)?;
```

Keep `runs` and `pre_run` in the function scope until execution returns. Do not hold `store` beyond admission.

- [ ] **Step 4: Remove the delayed duplicate admission**

Delete only the later block immediately after `validate_execution_authority`:

```rust
let pre_run = execution
    .runs
    .track_pre_run(execution.session_id.clone(), cancellation.clone())
    .map_err(|error| error.to_string())?;
```

Keep the existing `pre_run.begin_finalization(cancellation)` call in the success finalizer unchanged. Keep `execution.runs` passed to `RunExecutor::with_run_registry` unchanged.

- [ ] **Step 5: Run the two exact GREEN regressions**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::project_replacement_during_function_loading_cancels_before_old_resource_insert -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_activation::tests::activation_and_pre_run_function_loading_complete_without_deadlock -- --exact --test-threads=1
```

Expected: each runs one test and passes; activation publishes no old-project function resource and neither thread deadlocks.

- [ ] **Step 6: Run focused lifecycle and registry coverage serially**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::runtime::project_run::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_activation::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_lifecycle::tests -- --test-threads=1
```

Expected: all selected tests pass with zero failures.

- [ ] **Step 7: Run required quality gates**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Expected: all commands exit 0. Existing LF/CRLF notices and the known `unused_must_use` test warnings are non-failing baseline output.

- [ ] **Step 8: Complete independent task review**

Review the scoped diff against the design and verify:

- admission occurs before `load_function_resources`;
- registry/session pairing is captured and admitted under the same short `project_store` read lock;
- no project/global lock survives into loading or execution;
- early returns drop `pre_run` and unblock drains;
- no duplicate registry or cancellation path was added;
- existing finalization and active run behavior remains intact.

Resolve every Critical or Important finding and rerun its covering focused tests before marking the task complete.

- [ ] **Step 9: Publish reviewed progress**

Append exact GREEN counts, gate results, review verdict, and the contract to the ledger. Then update only the relevant percentages/status text in the `TODO.md` `## node_architecture 进度` table. The status must state that pre-run cancellation now covers function resource loading and activation drain without deadlock.
