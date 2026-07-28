# Committed Resource Receipt Completion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every allocated `CommittedResourceMutation` publication revision complete into one canonical, observable `ResourceMutationResultDto` without any post-receipt error path.

**Architecture:** Move projection membership discovery and every projection-builder input into the committed receipt before publication allocation returns. `CommittedResourceMutation::complete(self, locale)` becomes an owned-data-only, non-fallible operation that logs projection construction errors and returns an incomplete projection result; signature, undo, redo, graph-move history, and variable-effect paths all complete and route that same DTO after receipt creation, even if recovery-required is marked meanwhile.

**Tech Stack:** Rust 2024, serde/serde_json, Tauri 2, pnpm Cargo scripts.

## Global Constraints

- The approved design is `docs/superpowers/specs/2026-07-27-committed-receipt-completion-design.md`; preserve its pre-commit/commit/completion boundary exactly.
- Scope is only the publication-recovery Task 1 correctness blocker. Do not implement publication-recovery Task 2, frontend coordination, or any main filesystem-plan task.
- Before a receipt exists, lifecycle, revision, history-head, normalization, cache, filesystem, rollback, and projection-membership failures remain ordinary errors and allocate no publication revision.
- After a receipt exists, there is no fallible `?`, live `ProjectState` read, state lock, filesystem I/O, lifecycle/recovery gate, or result reconstruction before canonical return/observation.
- The observer and direct response receive the same `ResourceMutationResultDto`; `RunResult.resource_mutation` carries that complete DTO without split fields or Tauri reconstruction.
- Projection failure logs the internal error and returns `ProjectionStatusDto::Incomplete` with no replacements and all receipt-owned expected graph paths invalidated.
- Preserve `MutationPublication.resource_revision` contiguity and the coherent `ProjectIndex.publication_revision` baseline.
- Preserve unrelated working-tree changes. Do not reset, overwrite, stage, or commit any file.
- Strict TDD: add the named tests, run the exact RED commands, then make the minimum production changes.
- Rust tests are limited to the exact focused `--lib` commands below with `CARGO_BUILD_JOBS=1` and `--test-threads=1`. Do not run full Rust tests, `pnpm rust:test` without `--lib`, frontend tests, `pnpm verify`, or `pnpm verify:rust`.
- Run `pnpm rust:check`, `pnpm rust:fmt:check`, and `git diff --check` only after the focused GREEN tests.
- Explicitly defer receipt allocation/copy reduction, projection-snapshot memory reduction, cache reuse, performance work, and splitting `project_state.rs` to the main `docs/superpowers/plans/2026-07-27-project-filesystem-transaction.md` plan.
- Publication-recovery Task 2 remains closed until this task has a clean focused review and the existing Task 1 blocker is then superseded in both its report and ledger.

## Planned File Structure

- `src-tauri/src/project/project_state.rs`: own complete projection inputs in `CommittedResourceMutation`, implement total receipt completion, route all receipt-producing resource mutations through it, and host private-boundary race regressions.
- `src-tauri/src/project/production_tests.rs`: verify public signature/undo/redo observer-return identity, recovery-marker behavior, incomplete projection fallback, next-mutation rejection, and publication contiguity.
- `src-tauri/src/commands/command_node_system.rs`: extend the existing canonical `RunResult` source audit to cover receipt completion and retain the exact variable-effect event DTO assertion.
- `.superpowers/sdd/2026-07-27-committed-receipt-completion/task-1-report.md`: record exact RED/GREEN evidence and scoped implementation facts.
- `.superpowers/sdd/2026-07-27-committed-receipt-completion/task-1-review-package.md`: record the focused diff/test package presented for review.
- `.superpowers/sdd/2026-07-27-committed-receipt-completion/task-1-review.md`: record the independent review verdict and findings.
- `.superpowers/sdd/2026-07-27-project-publication-recovery/task-1-report.md`: append the supersession statement only after a clean review.
- `.superpowers/sdd/2026-07-27-project-publication-recovery/progress.md`: append Task 1 completion and reopen Task 2 only after that same clean review.

---

### Task 1: Make committed receipt completion total and reopen publication recovery

**Files:**
- Modify: `src-tauri/src/project/project_state.rs:127-134, 197-204, 483-520, 716-765, 1183-1441, 2883-3675, 3962-3968, 3971-4313`
- Modify: `src-tauri/src/project/production_tests.rs:954-1095`
- Modify: `src-tauri/src/commands/command_node_system.rs:387-414, 432-471`
- Create: `.superpowers/sdd/2026-07-27-committed-receipt-completion/task-1-report.md`
- Create: `.superpowers/sdd/2026-07-27-committed-receipt-completion/task-1-review-package.md`
- Create: `.superpowers/sdd/2026-07-27-committed-receipt-completion/task-1-review.md`
- Modify after clean review only: `.superpowers/sdd/2026-07-27-project-publication-recovery/task-1-report.md:428-end`
- Modify after clean review only: `.superpowers/sdd/2026-07-27-project-publication-recovery/progress.md:5-10`

**Interfaces:**
- Consumes: `ProjectionSourceSnapshot`, `ResourceMutationResultDto`, `ProjectionStatusDto`, `HistoryStatusDto`, `ResourceDeltaEvent`, `ResourceMoveDto`, `MutationPublication::allocate_resource_revision`, `ProjectRecoveryMarker`, `VariableEffectCommitResult`, and `RunResult.resource_mutation`.
- Produces:

```rust
struct CommittedResourceMutation {
    project_instance_id: String,
    publication_revision: u64,
    moves: Vec<crate::event::ResourceMoveDto>,
    deltas: Vec<crate::node_system::document::ResourceDeltaEvent>,
    history: HistoryStatusDto,
    projection_source: ProjectionSourceSnapshot,
    expected_graph_paths: Vec<String>,
    #[cfg(test)]
    completion_test_hook: Option<CommittedResourceCompletionTestHook>,
}

type CommittedResourceCompletionTestHook = Arc<dyn Fn() + Send + Sync>;

impl CommittedResourceMutation {
    fn complete(self, locale: &str) -> crate::event::ResourceMutationResultDto;
}
```

```rust
impl ProjectionSourceSnapshot {
    fn replacements(
        &self,
        graph_paths: &[String],
        locale: &str,
    ) -> Result<Vec<crate::event::GraphProjectionReplacementDto>, String>;

    fn graph_projection(
        &self,
        graph_path: &GraphResourcePath,
        locale: &str,
    ) -> Result<EditorGraphProjectionDto, String>;
}
```

`ProjectionSourceSnapshot` also owns the cloned `ProjectionTestHook` under `#[cfg(test)]`. Its projection methods use only snapshot fields; they do not accept `&ProjectState` and do not call `ensure_project_operational`.

```rust
struct CommittedVariableEffects {
    variable_ids: Box<[crate::variable::VariableId]>,
    resource_mutation: Option<CommittedResourceMutation>,
}

impl ProjectState {
    fn commit_variable_effects_receipt(
        &self,
        expected_session_id: &crate::node_system::analysis::ProjectSessionId,
        effects: Vec<crate::node_system::runtime::VariableWriteEffect>,
    ) -> Result<CommittedVariableEffects, VariableEffectCommitError>;
}
```

`ProjectState::commit_variable_effects` retains its current external signature and converts the optional receipt with `receipt.complete("en-US")` only after `CommittedFilesystemMutation::finalize()`. `execute_graph` continues assigning that canonical DTO directly to `RunResult.resource_mutation`.

`apply_resource_document_patch_internal` returns `Result<CommittedResourceMutation, ProjectFilesystemError>`. `apply_resource_document_patch` completes that receipt with `"en-US"`; graph-move undo/redo consumes the receipt directly and no longer calls `get_data()` or reconstructs a second receipt after publication.

- [ ] **Step 1: Add the named RED behavior regressions**

In `src-tauri/src/project/production_tests.rs`, replace the broad existing projection-failure test with these exact test names while retaining its reusable function-signature setup:

```rust
#[test]
fn committed_signature_undo_redo_return_and_observe_after_recovery_marker() {
    // Use three independently initialized function states so each case can mark
    // recovery-required after its receipt without blocking setup for the next case.
    // Signature expects publication 1; undo prepares one signature transaction and
    // expects publication 2; redo prepares signature + undo and expects publication 3.
    // In each case, set the receipt-owned completion hook to mark
    // "injected recovery after committed receipt", invoke the public observed API,
    // and assert the returned DTO equals the single observed DTO.
    // Assert the expected revision, complete projection paths, and that a subsequent
    // signature mutation returns MutationConflict::RecoveryRequired.
}

#[test]
fn committed_projection_failure_after_recovery_marker_returns_incomplete() {
    // Configure a function and affected caller graph, set the receipt-owned completion
    // hook to mark recovery, and set the receipt-owned projection hook to return
    // Err("injected projection failure"). The public signature API must return Ok,
    // observe exactly the same DTO once, return no replacements, and invalidate the
    // sorted function + caller paths in ProjectionStatusDto::Incomplete.
}

#[test]
fn committed_variable_effect_returns_canonical_result_after_recovery_marker() {
    // Persist one global Int64 variable, set the receipt-owned completion hook to mark
    // recovery, commit one VariableWriteEffect, and assert resource_mutation is Some.
    // Assert publication revision 1, the variable delta, exact history status, empty
    // expected graph paths, and that the next variable-effect commit is rejected by
    // the recovery gate without allocating another result.
}

#[test]
fn committed_resource_observer_and_response_serialize_identically() {
    // Run signature, undo, and redo through public observed APIs and compare
    // serde_json::to_value(returned) with serde_json::to_value(observed[0]) for each;
    // assert each observer is invoked exactly once.
}
```

Use these exact test-only hooks in `ProjectState` so the race is deterministic without a live-state read from `CommittedResourceMutation::complete`:

```rust
#[cfg(test)]
pub(super) fn set_committed_resource_completion_test_hook(
    &self,
    hook: CommittedResourceCompletionTestHook,
);
```

The hook is cloned into each receipt while the commit inputs are captured, then invoked from the receipt solely for test scheduling. The closure may hold a cloned `ProjectRecoveryMarker`; the receipt must not hold or query `ProjectState`.

- [ ] **Step 2: Add the named RED source audit**

Extend `run_result_routes_canonical_resource_mutation_without_split_reconstruction` in `src-tauri/src/commands/command_node_system.rs` and add this separate exact test:

```rust
#[test]
fn committed_resource_completion_source_is_total_and_state_independent() {
    let project_source = include_str!("../project/project_state.rs");
    let receipt_start = project_source
        .find("impl CommittedResourceMutation {")
        .expect("committed receipt completion impl must exist");
    let receipt_end = project_source[receipt_start..]
        .find("\nimpl ProjectState {")
        .map(|offset| receipt_start + offset)
        .expect("receipt completion impl must end before ProjectState impl");
    let completion = &project_source[receipt_start..receipt_end];

    assert!(completion.contains(
        "fn complete(self, locale: &str) -> crate::event::ResourceMutationResultDto"
    ));
    for forbidden in [
        "Result<",
        "ensure_mutation_operational",
        "ensure_project_operational",
        "self.project_",
        "self.history",
        "self.mutation_publication",
        "std::fs",
        "ProjectFilesystem",
    ] {
        assert!(
            !completion.contains(forbidden),
            "committed completion contains forbidden post-receipt dependency: {forbidden}"
        );
    }

    for forbidden_api in [
        "pub fn update_function_signature(",
        "pub fn undo_last_transaction(",
        "pub fn redo_last_transaction(",
        "resource_project_instance_id",
        "resource_publication_revision",
        "resource_deltas",
        "resource_history",
    ] {
        assert!(
            !project_source.contains(forbidden_api),
            "resource publication retains split/delta-only API: {forbidden_api}"
        );
    }

    assert!(!project_source.contains("fn complete_resource_mutation("));
    assert!(!project_source.contains("let data = self\n            .get_data()"));
}
```

Keep the existing assertions that `RunResult` owns `Option<ResourceMutationResultDto>`, Tauri publishes `result.resource_mutation.as_ref()`, and no variable-effect reconstruction helper exists.

- [ ] **Step 3: Run the exact RED commands**

Run sequentially:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- project::production_tests::committed_signature_undo_redo_return_and_observe_after_recovery_marker --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- project::production_tests::committed_projection_failure_after_recovery_marker_returns_incomplete --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- project::production_tests::committed_variable_effect_returns_canonical_result_after_recovery_marker --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- commands::command_node_system::tests::committed_resource_completion_source_is_total_and_state_independent --exact --test-threads=1
```

Expected RED evidence:

- Behavior tests fail to compile because `set_committed_resource_completion_test_hook` does not exist and variable effects do not expose a receipt-owned completion boundary.
- The source audit fails because `impl CommittedResourceMutation` with non-fallible `complete` does not exist, `complete_resource_mutation` still exists, and current projection completion still depends on `ProjectState`.
- Do not change production code until all intended RED causes are recorded in the focused task report.

- [ ] **Step 4: Capture complete immutable projection inputs before returning receipts**

In `projection_source_snapshot_for_path`, clone the current test projection hook into `ProjectionSourceSnapshot` under `#[cfg(test)]`. Move `projection_replacements_from_snapshot`, `graph_projection_replacement_from_snapshot`, and the snapshot-only body of `graph_projection_from_snapshot` onto `ProjectionSourceSnapshot` with the signatures in **Interfaces**.

Delete these two live-state actions from snapshot projection construction:

```rust
self.ensure_project_operational()
self.run_projection_test_hook()
```

Run the snapshot-owned test hook instead. Keep ordinary public `ProjectState::graph_projection` lifecycle gating before snapshot capture; this change applies only to already-committed receipt completion.

At each publication allocation site that produces or feeds `CommittedResourceMutation`, compute and store `expected_graph_paths` before returning the receipt:

- function signature and in-memory undo/redo: `affected_projection_paths(&deltas, &data)` after authoritative mutation;
- variable-effect commit and durable variable undo/redo: `affected_projection_paths(&deltas, &next_data)` before installing/moving `next_data`;
- resource patch/graph move: the exact sorted `patch_projection_paths(&patch)` captured before consuming the patch.

Capture `projection_source` under the same short authority boundary and after applying the authoritative mutation, before `allocate_resource_revision()` returns. Capture the cloned completion test hook in the receipt at the same point. No receipt constructor may call `get_data()` after publication allocation.

- [ ] **Step 5: Implement total receipt completion and canonical routing**

Implement `CommittedResourceMutation::complete` as the sole receipt-to-DTO conversion:

```rust
impl CommittedResourceMutation {
    fn complete(self, locale: &str) -> crate::event::ResourceMutationResultDto {
        #[cfg(test)]
        if let Some(hook) = self.completion_test_hook.as_ref() {
            hook();
        }

        let projection_replacements =
            self.projection_source
                .replacements(&self.expected_graph_paths, locale);

        match projection_replacements {
            Ok(projection_replacements) => crate::event::ResourceMutationResultDto {
                project_instance_id: self.project_instance_id,
                publication_revision: self.publication_revision,
                moves: self.moves,
                deltas: self.deltas,
                projection_replacements,
                projection_status: crate::event::ProjectionStatusDto::Complete {
                    expected_graph_paths: self.expected_graph_paths,
                },
                history: self.history,
            },
            Err(error) => {
                tauri_plugin_log::log::error!(
                    "committed resource mutation projection completion failed: {error}"
                );
                crate::event::ResourceMutationResultDto {
                    project_instance_id: self.project_instance_id,
                    publication_revision: self.publication_revision,
                    moves: self.moves,
                    deltas: self.deltas,
                    projection_replacements: Vec::new(),
                    projection_status: crate::event::ProjectionStatusDto::Incomplete {
                        invalidated_graph_paths: self.expected_graph_paths,
                    },
                    history: self.history,
                }
            }
        }
    }
}
```

Delete `ProjectState::complete_resource_mutation`. Public signature/undo/redo methods must have this exact post-commit shape with no intervening `?`:

```rust
let receipt = self.commit_function_signature(graph_path, request)?;
let result = receipt.complete(locale);
observe(&result);
Ok(result)
```

Use the same three final statements after `commit_history_direction(...)` for undo and redo.

Refactor `apply_resource_document_patch_internal` to return a receipt instead of a DTO. `apply_resource_document_patch` completes it with `"en-US"`. `commit_graph_move_history_direction` receives that receipt, finalizes the committed filesystem mutation, and returns the receipt unchanged; remove the post-result `get_data()` snapshot and DTO-to-receipt reconstruction.

Refactor variable effects through `CommittedVariableEffects`. Empty effects retain `resource_mutation: None`. For non-empty effects, the authority closure returns an owned receipt; after it succeeds, call `committed_filesystem.finalize()`, then convert with:

```rust
Ok(VariableEffectCommitResult {
    variable_ids: committed.variable_ids,
    resource_mutation: committed
        .resource_mutation
        .map(|receipt| receipt.complete("en-US")),
})
```

Keep `execute_graph` and Tauri publication unchanged: `RunResult.resource_mutation` receives this DTO and `publish_run_resource_mutation` emits an exact clone once.

- [ ] **Step 6: Run focused GREEN tests**

Run sequentially:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- project::production_tests::committed_signature_undo_redo_return_and_observe_after_recovery_marker --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- project::production_tests::committed_projection_failure_after_recovery_marker_returns_incomplete --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- project::production_tests::committed_variable_effect_returns_canonical_result_after_recovery_marker --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- project::production_tests::committed_resource_observer_and_response_serialize_identically --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- commands::command_node_system::tests::committed_resource_completion_source_is_total_and_state_independent --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- commands::command_node_system::tests::run_result_routes_canonical_resource_mutation_without_split_reconstruction --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- commands::command_node_system::tests::run_variable_effects_publish_only_resource_mutation_committed --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- commands::command_project::query::tests::signature_undo_redo_publications_are_contiguous_and_match_project_index --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- project::production_tests::committed_resource_mutations_return_incomplete_results_when_projection_fails --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib -- variable_effect_ --test-threads=1
```

Expected: every named test passes; signature/undo/redo revisions remain `1/2/3`, observer and response JSON are identical, recovery marked after receipt does not erase the committed result, the next mutation is rejected, projection failure returns incomplete, variable-effect focused tests remain green, and the source audit finds no post-receipt fallibility or split wrapper.

If the old `committed_resource_mutations_return_incomplete_results_when_projection_fails` test was replaced rather than retained, omit only its command and record that exact replacement in the report; do not broaden the test filter.

- [ ] **Step 7: Run bounded Rust checks and prepare review evidence**

Run sequentially:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
git --no-pager diff -- src-tauri/src/project/project_state.rs src-tauri/src/project/production_tests.rs src-tauri/src/commands/command_node_system.rs
```

Expected: Rust check, format check, and whitespace check pass. Review the focused diff for these exact invariants:

1. Every `CommittedResourceMutation` constructor owns `projection_source` and sorted `expected_graph_paths` before returning.
2. `complete` returns a DTO, not `Result`, and uses no `ProjectState`, lock, lifecycle/recovery gate, or I/O.
3. No `?` occurs between receipt creation and result return/observer invocation.
4. Projection errors are logged and become incomplete results.
5. Graph-move history performs no post-publication `get_data()` receipt reconstruction.
6. Variable effects complete their receipt after non-fallible filesystem finalization and place the canonical DTO directly in `RunResult`.
7. No delta-only public wrapper or split RunResult publication field exists.
8. No optimization, module split, frontend edit, or main filesystem task has entered the diff.

Write `.superpowers/sdd/2026-07-27-committed-receipt-completion/task-1-report.md` with the exact commands, RED failure causes, GREEN counts, check output, changed interfaces, and remaining concerns. Write `task-1-review-package.md` with the three-file focused diff scope, test evidence, approved-design checklist, and explicit exclusions. Do not stage or commit either file.

- [ ] **Step 8: Require clean review before superseding the blocker**

Have a fresh reviewer compare the focused diff and report against the approved design and record one of these exact statuses in `.superpowers/sdd/2026-07-27-committed-receipt-completion/task-1-review.md`:

```text
Status: CLEAN
```

or

```text
Status: CHANGES REQUIRED
```

A clean review must explicitly confirm total completion, complete receipt ownership, recovery-marker race coverage for signature/undo/redo/variable effects, incomplete projection fallback, canonical observer/direct/RunResult routing, revision/index contiguity, source-audit coverage, and scoped verification. If any finding remains, keep publication recovery Task 1 blocked, do not edit its supersession ledger/report lines, fix only the finding, rerun the affected focused commands plus Step 7 checks, and obtain another review.

- [ ] **Step 9: Supersede the blocker and reopen publication-recovery Task 2 only after `Status: CLEAN`**

Append this exact section to `.superpowers/sdd/2026-07-27-project-publication-recovery/task-1-report.md`, replacing bracketed evidence references with the literal committed-receipt report and review paths shown here rather than free-form text:

```markdown
## Focused committed-receipt completion repair

The fix-round-5 blocker is superseded by
`.superpowers/sdd/2026-07-27-committed-receipt-completion/task-1-report.md`.
`CommittedResourceMutation` completion is now total and non-fallible; signature,
undo, redo, graph-move history, and variable effects return and publish their
canonical committed result even when recovery-required appears after receipt
creation. Projection construction failure returns an incomplete result with the
receipt-owned invalidation set.

The focused review at
`.superpowers/sdd/2026-07-27-committed-receipt-completion/task-1-review.md`
records `Status: CLEAN`. Publication recovery Task 1 is complete, and Task 2 may
now be dispatched. This does not reopen main filesystem Task 3.
```

Append this exact line to `.superpowers/sdd/2026-07-27-project-publication-recovery/progress.md`:

```text
Task 1: complete — prior five-round blocker superseded by the clean committed-receipt completion repair; Task 2 reopened for dispatch; Task 3 and main filesystem Task 3 remain gated; no commits; no full suites.
```

Do not delete or rewrite the five historical fix-round lines or the original blocker line. Do not mark publication-recovery Task 2 complete or start it in this task. Do not alter `.superpowers/sdd/2026-07-27-project-filesystem-transaction/progress.md`; the main filesystem plan remains blocked pending the publication-recovery Tasks 2–3 work and whole-Task-2 clean re-review required by its own plan.

- [ ] **Step 10: Verify documentation-only supersession scope**

Run:

```sh
git diff --check
git --no-pager diff -- .superpowers/sdd/2026-07-27-committed-receipt-completion/task-1-report.md .superpowers/sdd/2026-07-27-committed-receipt-completion/task-1-review-package.md .superpowers/sdd/2026-07-27-committed-receipt-completion/task-1-review.md .superpowers/sdd/2026-07-27-project-publication-recovery/task-1-report.md .superpowers/sdd/2026-07-27-project-publication-recovery/progress.md
```

Expected: whitespace passes; the focused report/review artifacts contain actual evidence; historical publication-recovery evidence remains append-only; Task 2 is reopened only after `Status: CLEAN`; main filesystem Task 3 remains closed. Do not run another test suite and do not commit.
