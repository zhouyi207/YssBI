# Rust Suite Contract Stabilization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Repair the fifteen deterministic stale Rust tests and strengthen unsupported resource-access preflight without changing valid production behavior.

**Architecture:** Replace implementation-text assertions with behavioral coverage, repair fixtures through authoritative ProjectState APIs, and normalize only non-semantic test values. `ProjectResourceProvider` gains one focused validation rule so unsupported known-resource access fails before acquisition and remains classified as `InvalidPlan`.

**Tech Stack:** Rust, ProjectState lifecycle/history, Node runtime resources, relational plan validation, focused serial Cargo tests through pnpm.

## Global Constraints

- Work directly on `shadcn`; no worktree, branch, or commit.
- Preserve all unrelated dirty work and completed catalog, observability, relational, cancellation, and revision-tombstone slices.
- Do not change valid numerical output, compile-ID allocation, recovery display strings, IPC mappings, or variable exclusive-access semantics.
- Do not globally activate `state_with_empty_graph`; use existing active helpers only where authoritative behavior is required.
- Do not replace deleted source-text tests with new string or AST matching.
- Resource preflight rejects unsupported access only for known project resources that cannot satisfy the requested mode.
- Use focused serial Rust tests only. Do not rerun the complete Rust suite until every task is reviewed/green and the user explicitly authorizes it.
- After each reviewed task and fresh controller verification, immediately update only relevant rows in `TODO.md` under `## node_architecture 进度`.

---

## File Structure

- Modify `src-tauri/src/commands/command_node_system.rs` to remove three obsolete source-text tests.
- Modify `src-tauri/src/commands/command_project/query.rs` for an active replacement fixture.
- Modify `src-tauri/src/project/production_tests.rs` for canonical active/function/variable fixtures, semantic capture counts, typed recovery errors, and relational fixture verification.
- Modify `src-tauri/src/node_system/runtime/production_tests.rs` for scheduler validation classification and provider preflight coverage.
- Modify `src-tauri/src/node_system/runtime/project_resource.rs` for unsupported known-resource access validation.
- Modify `src-tauri/src/node_system/runtime/builtin_tests.rs` for approximate decimal-list assertions.
- Modify `src-tauri/src/node_system/testing/snapshots.rs` for compile-ID-only snapshot normalization.
- Create `.superpowers/sdd/2026-08-02-rust-suite-contract-stabilization/progress.md` as the durable ledger.

### Task 1: Remove obsolete source-text architecture tests

**Files:**
- Modify: `src-tauri/src/commands/command_node_system.rs` tests module
- Verify: `src-tauri/src/project/production_tests.rs`
- Verify: `src-tauri/src/project/project_activation.rs`
- Create: `.superpowers/sdd/2026-08-02-rust-suite-contract-stabilization/progress.md`
- Modify after review: `TODO.md` node architecture table

**Interfaces:**
- Removes only the three diagnosed implementation-text tests.
- Relies on existing behavioral contracts for completion totality, captured metadata, stale authority, coherent activation, and panic recovery.

- [ ] **Step 1: Record baseline and exact obsolete tests**

Create the ledger with plan identity and record the three previously isolated failures:

```markdown
# SDD ledger — plan: docs/superpowers/plans/2026-08-02-rust-suite-contract-stabilization.md

Execution constraints:
- Directly use shadcn; no worktree/branch/commit.
- Focused serial Rust tests only; no complete suite without explicit authorization.
- Preserve production behavior and unrelated dirty work.
- Update TODO.md node_architecture progress after each reviewed task.

Task 1: baseline — three source-text tests fail on obsolete private function spelling/ownership
Task 2: baseline — four authoritative fixture tests do not reach intended behavior
Task 3: baseline — resource/relational validation fixtures are stale; provider preflight gap confirmed
Task 4: baseline — six numerical/identity/count/error-display assertions are brittle
```

- [ ] **Step 2: Delete exactly these tests**

Remove the complete `#[test] fn ...` blocks named:

```rust
committed_resource_completion_source_is_total_and_state_independent
projection_environment_capture_lock_order_is_activation_compatible
projection_environment_capture_rejects_mixed_activation_generation
```

Do not alter adjacent command tests or production code.

- [ ] **Step 3: Run replacement behavioral coverage serially**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::committed_signature_undo_redo_return_and_observe_after_recovery_marker -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::committed_projection_uses_precommit_database_metadata_after_removal -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::committed_source_cannot_rebind_after_authority_generation_aba -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::projection_environment_capture_is_activation_ordered_and_coherent -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::projection_environment_capture_rejects_store_from_overlapping_activation -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_activation::tests::publication_panic_restores_even_generation_and_preserves_complete_session -- --exact --test-threads=1
```

Expected: six tests pass. Completion may report an explicit incomplete projection when live compile authority is stale; it must not recapture authoritative domain/database metadata or rebind stale authority.

- [ ] **Step 4: Run Task 1 gates, review, and publish progress**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Independent review must verify exact deletion scope and adequate behavioral replacement coverage. After controller verification, update Phase 5 and Phase 9 status without claiming the complete suite is green.

### Task 2: Repair authoritative project fixtures

**Files:**
- Modify: `src-tauri/src/commands/command_project/query.rs`
- Modify: `src-tauri/src/project/production_tests.rs`
- Modify after review: `TODO.md` node architecture table

**Interfaces:**
- Uses `activate_project_fixture` or existing active-state helpers before authoritative mutation.
- Uses `create_graph_resource_fixture` for canonical managed function shells.
- Uses `ProjectState::add_variable` for authoritative variable state/revisions.

- [ ] **Step 1: Run the four exact RED regressions before changes**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib commands::command_project::query::tests::project_index_during_activation_observes_only_the_previous_complete_lifecycle -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::execution_rejects_function_body_change_after_main_plan_before_run -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::production_compiler_rejects_wrong_scope_and_duplicate_shell_nodes -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::project_variable_get_executes_against_authoritative_resource -- --exact --test-threads=1
```

Expected: failures occur before the intended assertions because replacement/graph/variable fixtures are inactive or malformed.

- [ ] **Step 2: Activate the project-index replacement fixture**

Before calling `replacement_source.add_variable(...)`, establish a unique active fixture root and activate it:

```rust
let replacement_root = std::env::temp_dir().join(format!(
    "yssbi-project-index-replacement-{}",
    uuid::Uuid::new_v4()
));
replacement_source.activate_project_fixture(
    replacement_root.to_string_lossy().into_owned(),
    ProjectData::new(),
);
```

Clean up the root after joining worker threads. Keep all concurrency expectations unchanged.

- [ ] **Step 3: Use a canonical function resource in the authority-gate test**

Replace direct insertion of an empty function document with:

```rust
let function_path = state
    .create_graph_resource_fixture("Authority", GraphDocumentKind::Function)
    .unwrap();
```

Use `function_path` for the later body mutation. Preserve the hook, body-change timing, stale-authority expectation, bounded waits, and worker join.

- [ ] **Step 4: Use the active graph helper for compiler diagnostics**

Replace the inactive fixture with:

```rust
let (state, root) = active_state_with_empty_graph("compiler-shell-diagnostics");
```

Preserve exact scope-mismatch and duplicate-shell diagnostic assertions, then remove `root` at test end.

- [ ] **Step 5: Create the project variable authoritatively**

Use:

```rust
let (state, root) = active_state_with_empty_graph("project-variable-execution");
let variable = state
    .add_variable(
        "authoritative",
        crate::graph::value::DataType::Int64,
        crate::graph::value::DataValue::Int64(41),
        "",
        crate::variable::VariableScope::Global,
        Vec::new(),
    )
    .unwrap();
```

Bind the node parameter to `variable.id`, preserve execution/result assertions, and clean up `root`.

- [ ] **Step 6: Run GREEN and owner suites**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib commands::command_project::query::tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::execution_rejects_function_body_change_after_main_plan_before_run -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::production_compiler_rejects_wrong_scope_and_duplicate_shell_nodes -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::project_variable_get_executes_against_authoritative_resource -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::project_state_variable::tests -- --test-threads=1
```

Expected: all selected tests pass and reach their intended behavioral assertions.

- [ ] **Step 7: Run gates, review, and publish progress**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Independent review must ensure no global helper behavior changed and all temporary roots are panic-safe or unconditionally cleaned up. After controller verification, update Phase 2, Phase 5, and Phase 6 as warranted.

### Task 3: Repair relational/resource fixtures and strengthen provider preflight

**Files:**
- Modify: `src-tauri/src/node_system/runtime/production_tests.rs`
- Modify: `src-tauri/src/node_system/runtime/project_resource.rs`
- Verify or minimally modify: `src-tauri/src/project/production_tests.rs` relational source fixture
- Modify after review: `TODO.md` node architecture table

**Interfaces:**
- `ProjectResourceProvider::validate_plan` returns `ResourceError::unsupported_access` for a known non-variable resource requesting `ResourceAccess::Exclusive`.
- Exclusive access to a known project variable remains valid.
- `RunExecutor` maps validation `UnsupportedAccess` to `RunError::InvalidPlan`.

- [ ] **Step 1: Add provider RED coverage before production changes**

Add a focused test using a known database resource:

```rust
#[test]
fn project_resource_provider_rejects_unsupported_access_during_validation() {
    let session = ProjectSessionId::new("project-a");
    let database = resource_id("databases/main");
    let resource_versions = versions(&[(database.as_str(), "1")]);
    let provider = ProjectResourceProvider::new(
        ProjectResourceSnapshot::new(session.clone(), resource_versions.clone())
            .with_database(database.clone(), Arc::new(polars::prelude::DataFrame::default())),
    );
    let provenance = empty_plan(
        &session,
        "events/main",
        &RegistryFingerprint::from_bytes([7; 32]),
        resource_versions,
    )
    .provenance;
    let requirement = CompiledResourceRequirement {
        resource: database,
        kind: ResourceKind::DatabaseConnection,
        access: ResourceAccess::Exclusive,
        optional: false,
    };

    let error = provider.validate_plan(&provenance, &[requirement]).unwrap_err();
    assert_eq!(
        error.kind(),
        crate::node_system::runtime::ResourceErrorKind::UnsupportedAccess
    );
}
```

Reuse the file's existing `empty_plan`, `resource_id`, and `versions` helpers exactly as shown; do not add a second plan/provenance constructor.

- [ ] **Step 2: Verify RED and the stale scheduler regression**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::runtime::production_tests::project_resource_provider_rejects_unsupported_access_during_validation -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::runtime::production_tests::run_executor_classifies_resource_plan_validation_errors -- --exact --test-threads=1
```

Expected: provider validation currently accepts the database requirement; the scheduler test's variable/external-artifact fixture is not a valid unsupported-access case.

- [ ] **Step 3: Add minimal preflight validation**

Inside `ProjectResourceProvider::validate_plan`, after session validation and before version checks, add:

```rust
for requirement in requirements {
    if self.snapshot_contains(requirement)
        && requirement.access == ResourceAccess::Exclusive
        && !self.snapshot.variables.contains_key(&requirement.resource)
    {
        return Err(ResourceError::unsupported_access(format!(
            "project resource '{}' does not support exclusive access",
            requirement.resource.as_str()
        )));
    }
    // existing version validation follows
}
```

Retain the acquire-time defense. Do not reject exclusive access for known project variables.

- [ ] **Step 4: Isolate scheduler error mapping with a test provider**

Add a test-only provider whose `validate_plan` deterministically returns `ResourceError::unsupported_access` and whose `acquire` is unreachable. Use it for the first case in `run_executor_classifies_resource_plan_validation_errors`. Keep the existing `ProjectResourceProvider` for stale-session and stale-version cases.

- [ ] **Step 5: Normalize the hand-built relational fixture**

In `production_relational_backend_executes_project_dataframe_source`, construct the `operators` collection first and set:

```rust
let pushdown_hints =
    crate::node_system::plan::infer_relational_pushdown_hints(&operators).into_boxed_slice();
```

Then use the same `operators` and `pushdown_hints` in `CompiledRelationalPlan`. If the current dirty-tree fixture already does this or already supplies the exact inferred hint, do not rewrite it; run the exact regression and document that no additional change was needed.

- [ ] **Step 6: Run GREEN and focused suites**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::runtime::production_tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::production_relational_backend_executes_project_dataframe_source -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::runtime::production_relational::tests -- --test-threads=1
```

Expected: provider preflight, scheduler classification, stale snapshot classification, relational execution, and existing legal variable-exclusive coverage pass.

- [ ] **Step 7: Run gates, review, and publish progress**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Independent review must verify known-resource-only rejection, legal variable exclusive access, validation-before-acquisition, strict relational metadata, and unchanged runtime error mapping. After controller verification, update Phase 6 and Phase 7.

### Task 4: Replace brittle numerical, identity, retry-count, and recovery assertions

**Files:**
- Modify: `src-tauri/src/node_system/runtime/builtin_tests.rs`
- Modify: `src-tauri/src/node_system/testing/snapshots.rs`
- Modify: `src-tauri/src/project/production_tests.rs`
- Modify after review: `TODO.md` node architecture table

**Interfaces:**
- Numerical tests compare decimal results at scaled tolerance `1e-12`.
- `plan_debug_snapshot` normalizes only `provenance.compile_id`.
- Capture retry tests require at least one retry and retain authoritative result assertions.
- Test-only `load_graph` preserves typed `ProjectFilesystemError`.

- [ ] **Step 1: Add a file-local decimal-list assertion**

Near `decimal`, add:

```rust
fn assert_decimal_list_approx_eq(actual: &RuntimeValue, expected: &[f64]) {
    let RuntimeValue::Scalar(Value::List(actual)) = actual else {
        panic!("expected scalar decimal list, got {actual:?}");
    };
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        let Value::Decimal(actual) = actual else {
            panic!("expected decimal list member, got {actual:?}");
        };
        let actual = actual.as_str().parse::<f64>().unwrap();
        let tolerance = 1e-12 * expected.abs().max(1.0);
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected} ± {tolerance}, got {actual}"
        );
    }
}
```

Use the actual `CanonicalDecimal` string accessor available in the type; do not change production conversion or round values.

- [ ] **Step 2: Replace exact statistics equality**

Replace the exact predicted list assertion with:

```rust
assert_decimal_list_approx_eq(&result.values["value_6"], &[1.0, 2.0, 3.0, 4.0]);
```

- [ ] **Step 3: Normalize only compile identity in snapshots**

Change `plan_debug_snapshot` to:

```rust
pub fn plan_debug_snapshot(plan: &ExecutionPlan) -> String {
    let mut canonical = plan.clone();
    canonical.provenance.compile_id = crate::node_system::analysis::CompileId::new(0);
    format!("{canonical:#?}")
}
```

Update its comment to state that all semantic plan/provenance fields remain, while process-monotonic `compile_id` is normalized.

- [ ] **Step 4: Make capture-count assertions semantic**

In both compile and graph-projection retry tests, preserve result/source-revision assertions and replace exact `2` with:

```rust
let captures = capture_count.load(Ordering::Acquire);
assert!(
    captures >= 2,
    "expected invalidated capture to be retried, observed {captures} capture(s)"
);
```

- [ ] **Step 5: Preserve typed load errors in tests**

Change the local helper signature to:

```rust
fn load_graph(
    state: &ProjectState,
    graph_path: &GraphResourcePath,
) -> Result<GraphResourceDocument, ProjectFilesystemError> {
    let project_instance_id = state.capture_project_session()?.instance_id;
    state.load_graph_resource(&project_instance_id, graph_path, 1)
}
```

Adapt existing unwrap callers without behavior changes. In both recovery tests assert:

```rust
let error = load_graph(&state, &graph).unwrap_err();
assert_eq!(error.code(), "project_recovery_required");
assert!(error.recovery_required());
```

Use the existing local graph variable name in each test.

- [ ] **Step 6: Run all six exact regressions and owner suites**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::runtime::builtin_tests::statistics_fit_executes_instead_of_returning_an_adapter_error -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::testing::tests::randomized_btree_insertion_order_is_semantically_equivalent -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::compile_capture_retries_when_authority_changes_during_metadata_capture -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::graph_projection_retries_when_authority_changes_during_metadata_capture -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::recovery_required_blocks_authoritative_entry_points_until_activation -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib project::production_tests::recovery_required_gate_blocks_project_authority_until_activation -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::runtime::builtin_tests -- --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test --lib node_system::testing::tests -- --test-threads=1
```

Expected: all selected tests pass without changing production output or identity allocation.

- [ ] **Step 7: Run all fifteen original exact regressions serially**

Run the exact filters listed across Tasks 1-4. For deleted source-text tests, verify their names no longer match any test and run the six replacement behavioral tests instead. Record exact counts in the ledger.

- [ ] **Step 8: Run final gates and whole-plan review**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Dispatch final independent review over all four task-only diffs and the ledger. Resolve every Critical/Important finding. Update the relevant TODO rows immediately after controller verification.

- [ ] **Step 9: Stop before complete-suite retry**

Record that all fifteen focused regressions and owner suites are green. Ask the user for explicit authorization before rerunning the complete Rust suite; do not infer authorization from plan execution.
