# Relational Production Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Execute a real built-in `DataFrame Source -> Limit` graph as one lazy `relational.default` island with pushdown, exact bridge values, cancellation, and `ProjectState::execute_graph` coverage.

**Architecture:** Built-in Source and Limit lower to existing relational IR. The compiler emits one executable operation per island, while the production backend owns lazy operator evaluation and database scanning. The scheduler publishes declared fragment outputs and passes exact materialization bridges; no eager complete protocol value is created between Source and Limit.

**Tech Stack:** Rust, Tauri, existing Node Protocol/Registry/compiler/runtime, DuckDB/Polars, pnpm Rust scripts.

## Global Constraints

- Work directly on branch `shadcn`; do not create a worktree or commit.
- Preserve unrelated changes, especially `src-tauri/tests/database_test.rs`.
- `ProjectState.project_data` remains authoritative and `ProjectState::insert_graph` remains the sole insertion path.
- Reuse `LoweredKernel::Relational`, `RelationalPlanner`, `ExecutionPlan`, and `RunExecutor`; do not add a second executor.
- Support only `DataFrame Source -> Limit` as the first built-in relational pipeline.
- Defer parallel scheduling, cross-run cache, timeout, forced cancellation, federation, and cost optimization.
- Run Rust tests serially with `CARGO_BUILD_JOBS=1` and `--test-threads=1`; do not run the complete Rust suite in this slice.

---

## File Structure

- Modify `src-tauri/src/node_system/catalog/dataframe/families.rs`: stable Limit inventory.
- Modify `src-tauri/src/node_system/catalog/dataframe/mod.rs`: Source/Limit protocols and relational lowerers.
- Modify `src-tauri/src/node_system/catalog/builtin.rs`: focused custom lowerer registration only if required.
- Modify `src-tauri/src/node_system/compiler/pipeline.rs`: one executable operation per relational island.
- Modify `src-tauri/src/node_system/compiler/relational.rs`: deterministic island/root/bridge metadata.
- Modify `src-tauri/src/node_system/plan/model.rs`: explicit island root and fragment-output bindings.
- Modify `src-tauri/src/node_system/plan/validation.rs`: relational ownership and bridge validation.
- Create `src-tauri/src/node_system/runtime/production_relational.rs`: lazy production backend.
- Modify `src-tauri/src/node_system/runtime/mod.rs`: export production backend.
- Modify `src-tauri/src/node_system/runtime/project_resource.rs`: bounded source scan.
- Modify `src-tauri/src/node_system/runtime/scheduler.rs`: one island invocation and fragment publication.
- Modify `src-tauri/src/project/project_state.rs`: backend registration only; remove embedded backend implementation.
- Test `src-tauri/src/node_system/catalog/dataframe/tests.rs`.
- Test `src-tauri/src/node_system/compiler/tests.rs` and `compiler/relational.rs`.
- Test `src-tauri/src/node_system/runtime/tests.rs` and new backend-local tests.
- Test `src-tauri/src/project/production_tests.rs`.

### Task 1: Freeze built-in Source and Limit relational contracts

**Interfaces:**
- Consumes: `NodeLowerer::lower(&LoweringContext) -> Result<LoweredNode, LoweringError>`.
- Produces: Source and Limit `LoweredKernel::Relational(RelationalNodeFragment)` targeting `relational.default`.

- [ ] **Step 1: Add failing Catalog tests** asserting `yssbi.dataframe.source.get` and new `yssbi.dataframe.limit` freeze, lower relationally, and expose streaming-compatible ports. Limit must have a bounded positive `rows` parameter with a protocol default.
- [ ] **Step 2: Run the failing tests.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::catalog::dataframe::tests --test-threads=1
```

Expected: failure because Limit is absent and Source still lowers as native.

- [ ] **Step 3: Implement focused lowerers.** Source emits a source fragment with the database resource parameter; Limit emits an input-bound limit fragment. Keep every other DataFrame node on its current native lowerer.
- [ ] **Step 4: Re-run the exact test filter** and require all Catalog/protocol assertions to pass.

### Task 2: Compile one operation per relational island

**Interfaces:**
- Consumes: `RelationalPlanner::plan(&[RelationalFragment], &[RelationalConnection])`.
- Produces: exactly one `PlannedKernel::Relational(subplan_index)` operation for a merged Source→Limit island, with deterministic graph-value/root bindings.

- [ ] **Step 1: Strengthen compiler tests** to assert one subplan, one executable relational operation, one backend root, no internal materialization bridge, and stable ordering under reversed document insertion.
- [ ] **Step 2: Run focused compiler tests and observe the current duplicate operation failure.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::compiler::tests::compiler_plans_relational_islands_with_valid_local_indices --exact --test-threads=1
```

- [ ] **Step 3: Refactor `lower_graph`** so relational fragments are collected first and converted into island-owned pending operations after planning. Preserve operation source-node correlation using the deterministic island root node; preserve every graph result/value mapping.
- [ ] **Step 4: Extend `ExecutionPlan::validate`** to reject duplicate island owners, invalid roots, unknown fragment IDs, and mismatched producer/consumer subplan bridges.
- [ ] **Step 5: Run relational planner and compiler filters.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::compiler::relational::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::compiler::tests::compiler_plans_relational_islands_with_valid_local_indices --exact --test-threads=1
```

### Task 3: Add a lazy, pushdown-aware production backend

**Interfaces:**
- Consumes: `RelationalBackend::execute(context, plan, operation_inputs, bridge_inputs)`.
- Produces: `RelationalExecution { outputs, fragment_outputs }` without eager Source materialization before Limit.

- [ ] **Step 1: Add backend-local failing tests** with an observable source scanner. Assert the scan receives the pushed row bound, not merely that final output is truncated.
- [ ] **Step 2: Run the backend test filter** and verify failure because the existing backend ignores hints.

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::production_relational::tests --test-threads=1
```

- [ ] **Step 3: Create `production_relational.rs`** with a private lazy expression enum for Source, Input, and Limit. Build expressions from `CompiledRelationalPlan.operators`; evaluate only roots and requested fragment outputs.
- [ ] **Step 4: Add bounded source scanning** to `ProjectDatabaseSnapshot`/`ProjectResourceLease`. Loaded frames slice before conversion; DuckDB executes a quoted table query with `LIMIT n`. Keep existing eager `load_dataframe` for native kernels.
- [ ] **Step 5: Apply `RelationalPushdownHint::Limit`** while constructing/evaluating Source so the scan bound is `min` of applicable limits.
- [ ] **Step 6: Re-run backend tests** and verify pushed scan count and output count.

### Task 4: Publish exact fragment outputs and consume exact bridges

**Interfaces:**
- Produces: fragment values keyed by `RelationalFragmentId`; bridge lookup keyed by the full `PlannedMaterializationBridge` identity.

- [ ] **Step 1: Add a compiler/runtime failing test** using a focused test Registry consumer whose input requires `FullyMaterialized`. Do not add a user-visible built-in only for this test.
- [ ] **Step 2: Assert** two islands, one declared bridge, producer `fragment_outputs` containing the exact producer fragment, and downstream `bridge_inputs` containing the exact bridge/value pair.
- [ ] **Step 3: Run the focused bridge tests.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::compiler::tests::compiler_derives_materialization_bridge_from_consumer_contract --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::tests::materialization_bridges_preserve_their_explicit_semantics --exact --test-threads=1
```

- [ ] **Step 4: Implement requested fragment evaluation** in the backend and exact bridge resolution for Input operators. Never guess by vector position or display name.
- [ ] **Step 5: Re-run both filters** and require exact value equality.

### Task 5: Close cancellation and result-publication semantics

- [ ] **Step 1: Add deterministic tests** for cancellation at source scan, operator evaluation, bridge materialization, and final result materialization. Use checkpoint hooks under `#[cfg(test)]`, not timing sleeps.
- [ ] **Step 2: Assert** `RunError::Cancelled`, a cancellation event, no completion event, and no result-store publication.
- [ ] **Step 3: Add `CancellationToken::check()`** at every required checkpoint in the backend and immediately before scheduler result publication.
- [ ] **Step 4: Run runtime-focused filters.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::production_relational::tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::tests::relational_operation_executes_compiled_subplan_by_index --exact --test-threads=1
```

### Task 6: Prove the real ProjectState vertical slice

- [ ] **Step 1: Add `project_execute_graph_runs_builtin_dataframe_source_limit`** using a temporary project/database, real built-in Registry/Catalog, authoritative `GraphDocument`, `ProjectState::insert_graph`, and `ProjectState::execute_graph`.
- [ ] **Step 2: Assert** one island, one backend invocation through a test-only observer, no internal bridge, pushed scan bound, and bounded DataFrame result.
- [ ] **Step 3: Run the new E2E test** and fix only production-path defects it exposes.

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::project_execute_graph_runs_builtin_dataframe_source_limit --exact --test-threads=1
```

- [ ] **Step 4: Run slice gates.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Do not run the complete Rust suite.