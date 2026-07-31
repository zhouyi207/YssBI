# Compile Publication Owner Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make one graph-scoped production `CompileCoordinator` publish and reuse the current analysis, semantic graph, and execution plan by exact compilation basis.

**Architecture:** The existing generic coordinator remains the concurrency primitive. Its analysis payload becomes `PublishedCompileAnalysis`; `ProjectState` owns a replaceable coordinator generation so old-project work cannot become current. Editor projection and execution share a compile helper and never hold project locks during compilation or waiting.

**Tech Stack:** Rust, Tauri ProjectState, existing compiler/analysis/plan IR, serde, pnpm Rust scripts.

## Global Constraints

- Work on `shadcn`; no worktree, commit, compatibility adapter, or second compiler path.
- Preserve unrelated changes.
- Key slots by `GraphResourcePath`, never tab ID/title/locale.
- Compare complete `CompilationBasis`: graph revision, Registry fingerprint, and all resource versions.
- Blocking analysis clears the executable plan; stale completion can never restore it.
- Resource/Registry changes initially invalidate all graph slots because the basis contains the full resource version set.
- Use focused serial Rust tests only; no complete suite in this slice.

---

## File Structure

- Modify `src-tauri/src/node_system/compiler/pipeline.rs`: retain semantic product.
- Modify `src-tauri/src/node_system/compiler/mod.rs`: public aliases.
- Modify `src-tauri/src/node_system/compiler/coordinator.rs`: reuse, joining, waiting, invalidation.
- Modify `src-tauri/src/node_system/plan/model.rs`: serde for complete plan object graph.
- Create `src-tauri/src/project/compile_publication.rs`: coherent compile capture/publication helper.
- Modify `src-tauri/src/project/mod.rs`: module registration.
- Modify `src-tauri/src/project/project_state.rs`: coordinator generation, lifecycle invalidation, projection/execution reuse.
- Test compiler/coordinator and `src-tauri/src/project/production_tests.rs`.

### Task 1: Retain and serialize exact compile products

**Interfaces:**

```rust
pub struct PublishedCompileAnalysis {
    pub analysis: CompilerAnalysis,
    pub semantic: Option<CompilerSemanticGraph>,
}

pub type ProjectCompileCoordinator =
    CompileCoordinator<PublishedCompileAnalysis, ExecutionPlan>;
```

- [ ] **Step 1: Add failing compiler tests**: successful compilation has semantic+plan with equal basis; blocking analysis has neither semantic nor plan.
- [ ] **Step 2: Run exact tests.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::compiler::tests::valid_constant_graph_produces_plan_with_same_basis --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::compiler::tests::unknown_node_returns_analysis_without_plan --exact --test-threads=1
```

- [ ] **Step 3: Make `CompilerSemanticGraph` public**, add `CompileResult.semantic`, and retain the validated semantic graph after lowering. If lowering adds a blocking diagnostic, return `semantic: None` and `plan: None`.
- [ ] **Step 4: Add serde derives** to the complete `ExecutionPlan` graph, including index/opaque ID macros, control regions, relational plans, resources, and results.
- [ ] **Step 5: Re-run exact tests** plus the dynamic-interface blocking test.

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::compiler::tests_dynamic_pipeline::full_compile_keeps_complete_projection_when_interface_diagnostics_block_lowering --exact --test-threads=1
```

### Task 2: Complete coordinator reuse, joining, and invalidation

**Interfaces:**
- `get_current(graph_path, basis) -> Option<(CompileProjection<PublishedCompileAnalysis>, Option<CompileProjection<ExecutionPlan>>)>`.
- `invalidate(graph_path)` and `invalidate_all()` cancel work, clear products, and wake waiters.

- [ ] **Step 1: Add failing coordinator tests** for matching published reuse, same-basis join without cancellation, latest-different-basis coalescing, waiter wake on publish/invalidate, graph invalidation, all invalidation, and stale plan non-restoration.
- [ ] **Step 2: Run coordinator tests.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::compiler::coordinator::tests --test-threads=1
```

- [ ] **Step 3: Add slot state and `Condvar` notification** without holding the coordinator mutex during compilation. Same active basis joins; a different basis cancels active and keeps exactly one latest pending task.
- [ ] **Step 4: Add cloneable exact-basis getters** requiring analysis and plan projections to share graph path, basis, and compile ID.
- [ ] **Step 5: Add invalidation APIs** that cancel active/pending tasks, remove published products, and notify waiters.
- [ ] **Step 6: Re-run coordinator tests** and review for lock-order inversions.

### Task 3: Add replaceable ProjectState compile ownership

**Interfaces:**

```rust
compile_coordinator: Arc<RwLock<Arc<ProjectCompileCoordinator>>>
```

`compile_publication.rs` owns coherent input capture and `get_or_compile_current(graph_path)`; callers receive cloned current projections.

- [ ] **Step 1: Add failing production tests** for projection→execution reuse and execution→projection reuse, asserting one compile ID on an unchanged basis.
- [ ] **Step 2: Run the tests and confirm current double compilation.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::projection_and_execution_reuse_one_compile_product --exact --test-threads=1
```

- [ ] **Step 3: Create `compile_publication.rs`**. Capture document, Registry `Arc`, resource snapshot, project session, and exact basis under short locks; release locks; request/join/compile; re-read the authoritative basis; CAS-publish; return only current products.
- [ ] **Step 4: Add current project session ID to projection compilation** so a plan first produced by editor projection is safe for execution.
- [ ] **Step 5: Route `ProjectionSourceSnapshot::graph_projection`** through published analysis and `ProjectState::execute_graph` through the published plan. Runtime resource acquisition, function-plan generation, compiled parameters, effects, and result publication remain per-run.
- [ ] **Step 6: Re-run reuse tests** and assert projection DTO/IPC signatures did not change.

### Task 4: Wire authoritative lifecycle invalidation

- [ ] **Step 1: Add failing tests** for blocking recompile plan clearing, graph unload invalidation, stale compile non-restoration, and project replacement detaching old generation.
- [ ] **Step 2: Run exact filters.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::blocking_recompile_clears_published_execution_plan --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::stale_compile_cannot_restore_an_older_plan --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::graph_unload_invalidates_compile_slot --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::production_tests::project_replacement_detaches_old_compile_generation --exact --test-threads=1
```

- [ ] **Step 3: Invalidate one slot** at authoritative graph mutation/patch/insert/unload/remove/move boundaries.
- [ ] **Step 4: Invalidate all slots** after function, variable, database, Registry, history, and committed variable-effect changes.
- [ ] **Step 5: Replace the inner coordinator `Arc` atomically** during project activation/clear. Detached snapshots may finish only into the detached generation.
- [ ] **Step 6: Re-run lifecycle tests.**

### Task 5: Add differential determinism coverage

- [ ] **Step 1: Construct two fixed-ID documents** with reversed node, connection, parameter, and input-state insertion orders and a fixed `CompileId`.
- [ ] **Step 2: Compile both** against identical Registry/resources/session and assert byte-identical `serde_json::to_vec` for analysis, semantic graph, and execution plan.
- [ ] **Step 3: Assert explicit sequence equality** for diagnostics, semantic dependencies, relational subplans, fragment order, and bridges.
- [ ] **Step 4: Run the determinism test.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::compiler::tests::semantically_identical_documents_serialize_identically --exact --test-threads=1
```

- [ ] **Step 5: If it fails, sort only at the authoritative compiler finalization boundary**; do not hide nondeterminism by sorting serialized JSON.

### Task 6: Slice verification

- [ ] Run focused coordinator/compiler/project tests, then:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Do not run the complete Rust suite.