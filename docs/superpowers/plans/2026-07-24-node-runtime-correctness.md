# Node Runtime Correctness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Eliminate the remaining known silent-correctness hazards in project-variable execution and migrated statistics kernels.

**Architecture:** Keep immutable variable reads exactly as implemented by `ProjectResourceSnapshot`, but reject exclusive variable requirements during plan validation until durable revisioned Run-side writes have a complete commit coordinator. For statistics, preserve working operation-specific `yss-sci` adapters, add strict semantic validation at the adapter boundary, and replace `VarSummary`'s incorrect lag-selection call with a real VAR fit.

**Tech Stack:** Rust, serde JSON protocol values, `yss-sci`, Tauri project runtime, pnpm Cargo scripts.

## Global Constraints

- `ProjectState.project_data` remains authoritative.
- Do not reintroduce live `ProjectData` access from runtime kernels.
- Do not hold project locks during I/O, compilation, or execution.
- Unsupported scientific semantics must return an explicit error; they must never fall back to another algorithm.
- Add each regression test before its implementation and observe the expected RED result.
- Run Rust commands sequentially with `CARGO_BUILD_JOBS=1`.
- Do not run the full Rust test suite by default.
- Preserve unrelated changes and do not commit.

---

### Task 1: Reject unsafe Run-side variable writes

**Files:**
- Modify: `src-tauri/src/node_system/runtime/production_tests.rs`
- Modify: `src-tauri/src/node_system/runtime/project_resource.rs`

**Interfaces:**
- Consumes: `ProjectResourceProvider::validate_plan(&CompileProvenance, &[CompiledResourceRequirement])`.
- Produces: early `ResourceError` for an `ExternalArtifact` variable with `ResourceAccess::Exclusive`; shared variable reads remain valid and immutable.

- [x] **Step 1: Add the failing exclusive-access test**

Add `project_variable_exclusive_access_is_rejected_before_acquisition` beside the existing snapshot test. Build a snapshot containing `variables/<id>`, construct matching provenance and an exclusive requirement, call `validate_plan`, and assert the exact error:

```rust
assert_eq!(
    error.to_string(),
    format!(
        "project variable '{}' does not support Run-side writes until durable revisioned commits are available",
        resource.as_str()
    )
);
```

Also retain a shared requirement assertion that `validate_plan` succeeds.

- [x] **Step 2: Run the focused test and verify RED**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::production_tests::project_variable_exclusive_access_is_rejected_before_acquisition --exact --test-threads=1
```

Expected: FAIL because exclusive variable access currently returns `Ok(())`.

- [x] **Step 3: Add the validation guard**

In `ProjectResourceProvider::validate_plan`, before version comparison, reject only requirements satisfying all three conditions:

```rust
requirement.kind == ResourceKind::ExternalArtifact
    && requirement.access == ResourceAccess::Exclusive
    && self.snapshot.variables.contains_key(&requirement.resource)
```

Return the stable error asserted by the test. Do not change shared reads or non-variable exclusive-resource behavior.

- [x] **Step 4: Separate the existing read characterization from writes**

Rename `variable_reads_stay_on_the_snapshot_and_writes_become_effects` to `variable_reads_stay_on_the_snapshot`. Acquire the variable with `ResourceAccess::Shared`, assert the cloned value remains `Int64(1)`, and remove assertions that normalize Run-side writes as supported behavior.

- [x] **Step 5: Verify GREEN and related runtime coverage**

Run sequentially:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::production_tests::project_variable_exclusive_access_is_rejected_before_acquisition --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::production_tests::variable_reads_stay_on_the_snapshot --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::tests::cancellation_stops_run_and_releases_resources --exact --test-threads=1
```

Expected: PASS.

---

### Task 2: Enforce strict statistics semantic guards

**Files:**
- Modify: `src-tauri/src/node_system/runtime/kernels/statistics/mod.rs`
- Modify: `src-tauri/src/sci/api/node_statistics.rs`

**Interfaces:**
- Consumes: protocol model objects with `family` and `coefficients`; ADF `regression` strings.
- Produces: family-compatible prediction only; explicit ADF rejection for unknown regression specifications.

- [x] **Step 1: Add the failing prediction-family test**

Add `prediction_rejects_incompatible_model_family` to the statistics test module. Construct an object containing `family = "ols"` and numeric coefficients, invoke `LogitPredict`, and assert:

```rust
assert_eq!(
    error.to_string(),
    "statistics prediction LogitPredict requires model family 'logit', got 'ols'"
);
```

Repeat for a `logit` model passed to `ProbitPredict` so all nonlinear predictors prove they do not accept arbitrary coefficient objects.

- [x] **Step 2: Run the prediction test and verify RED**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::kernels::statistics::tests::prediction_rejects_incompatible_model_family --exact --test-threads=1
```

Expected: FAIL because `prediction` currently ignores `family`.

- [x] **Step 3: Implement model-family validation**

In `prediction`, derive the required family from the operation:

```rust
let expected_family = match operation {
    StatisticsOperation::LinearPredict => "ols",
    StatisticsOperation::LogitPredict => "logit",
    StatisticsOperation::ProbitPredict => "probit",
    _ => unreachable!("prediction operation"),
};
```

Read `family` as `Value::String`, reject missing/non-string/mismatched values, then continue with the existing coefficient and link-function logic.

- [x] **Step 4: Add the failing unknown-ADF-specification test**

Add a unit test in `src-tauri/src/sci/api/node_statistics.rs` that calls:

```rust
augmented_dickey_fuller(&series, 1, "unexpected")
```

and expects:

```text
unsupported ADF regression 'unexpected'
```

- [x] **Step 5: Run the ADF test and verify RED**

Run the exact new test with `CARGO_BUILD_JOBS=1 pnpm rust:test -- ... --exact --test-threads=1`.

Expected: FAIL because unknown values currently silently select constant-only ADF.

- [x] **Step 6: Make ADF regression parsing exhaustive**

Change `augmented_dickey_fuller` to accept only:

```rust
"none" | "no_constant" => (false, false),
"constant" => (true, false),
"trend" => (true, true),
other => return Err(format!("unsupported ADF regression '{other}'")),
```

- [x] **Step 7: Verify the semantic guards**

Run sequentially:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::kernels::statistics::tests::prediction_rejects_incompatible_model_family --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- sci::api::node_statistics --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::kernels::statistics::tests::operation_specific_statistics_match_sci_golden_fixtures --exact --test-threads=1
```

Expected: PASS.

---

### Task 3: Execute real VAR estimation for `VarSummary`

**Files:**
- Modify: `src-tauri/src/node_system/runtime/kernels/statistics/mod.rs`
- Modify: `src-tauri/src/sci/api/node_statistics.rs`

**Interfaces:**
- Consumes: two or more equal-length numeric series and an explicit positive VAR lag count.
- Produces: serialized `yss_sci::ts::var::VARResult` with `equations`, `coefficients`, residual covariance, and diagnostics; `VarLagOrder` remains a separate `VARSocResult` operation.

- [x] **Step 1: Add the failing VAR summary test**

Add `var_summary_runs_estimation_instead_of_lag_selection` to the statistics tests. Invoke `VarSummary` with two stable fixture series and:

```rust
StatisticsKernelParameters {
    lags: Some(1),
    max_lags: Some(2),
    rank: None,
    trend: Some("constant".into()),
}
```

Assert the first result object contains `equations` and `coefficients`, does not contain `rows`, and the report says `VAR estimation from yss_sci`.

- [x] **Step 2: Run the VAR summary test and verify RED**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::kernels::statistics::tests::var_summary_runs_estimation_instead_of_lag_selection --exact --test-threads=1
```

Expected: FAIL because `VarSummary` currently calls `var_lag_order` and returns a `VARSocResult`.

- [x] **Step 3: Add a focused `var_fit` adapter**

In `src-tauri/src/sci/api/node_statistics.rs`, import `VAR` and `VARConfig` and add:

```rust
pub fn var_fit(series: Vec<Vec<f64>>, lags: usize) -> Result<serde_json::Value, String>
```

Validate `lags > 0`, convert via the existing `multivariate_series`, and run:

```rust
VAR {
    y,
    exog: None,
    config: VARConfig {
        constant: true,
        lags: (1..=lags).collect(),
        step: 8,
        dfk: false,
        mlag: 2,
        sample_start_offset: None,
        skip_extras: false,
    },
    var_names: None,
    exog_names: None,
    regression_times: None,
}
.fit()
```

Serialize the real `VARResult` with `serde_json::to_value`.

- [x] **Step 4: Route `VarSummary` to `var_fit`**

Keep `require_constant_trend`, pass `parameters.lags.unwrap_or(1)` to `var_fit`, and change the report to exactly:

```text
VAR estimation from yss_sci
```

Do not change `VarLagOrder`, which must continue to use `max_lags` and `var_lag_order`.

- [x] **Step 5: Verify GREEN and operation separation**

Run sequentially:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::kernels::statistics::tests::var_summary_runs_estimation_instead_of_lag_selection --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::kernels::statistics::tests::operation_specific_statistics_match_sci_golden_fixtures --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test:sci -- ts::var --test-threads=1
```

Expected: PASS.

---

### Task 4: Update execution ledger and verify the slice

**Files:**
- Modify: `.superpowers/sdd/progress.md`
- Modify: `.superpowers/sdd/task-production-backend-report.md`

**Interfaces:**
- Consumes: completed Task 1–3 test evidence.
- Produces: an accurate ledger that marks immutable reads as already present, Run-side writes as intentionally rejected, and statistics safety improvements without claiming all statistics configuration migration is complete.

- [x] **Step 1: Record exact scope and remaining limitations**

Append a report section containing:

- immutable variable reads were already implemented before this slice;
- exclusive Run variable access now fails before resource acquisition;
- durable revisioned variable write commits remain future work;
- prediction family and ADF specification validation are strict;
- `VarSummary` now performs real VAR estimation;
- OLS covariance configuration, non-identity GLS, panel estimator dispatch, DID inference, and summary formatting remain open and are not reported as complete.

Update `.superpowers/sdd/progress.md` so the current task accurately points to the next production-cut item rather than claiming the whole architecture is done.

- [x] **Step 2: Run focused and static verification sequentially**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::production_tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::kernels::statistics::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:fmt:check
git --no-pager diff --check
```

Expected: all commands PASS with no whitespace errors.

- [x] **Step 3: Review the final diff**

Confirm the diff contains only the planned runtime, scientific adapter, regression tests, progress/report, and this plan file. Do not commit.
