# Structured Control Production Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove Branch, Loop, Call, and effect semantics through real built-in protocols, authoritative GraphDocuments, and `ProjectState::execute_graph`.

**Architecture:** Keep the existing control IR and scheduler. Close authoring gaps with stable `PortRef`/dynamic-member identities, correct the cross-frame Call ABI, add explicit built-in effect ports, and add production tests rather than a second control engine.

**Tech Stack:** Rust Node Protocol, GraphDocument, GraphCompiler, FunctionPlanGeneration, RunExecutor, ProjectState.

## Global Constraints

- Work on `shadcn`; no worktree or commit.
- Use only built-in Registry nodes in final ProjectState tests; synthetic protocols remain unit fixtures only.
- Never persist compiler-local `ValueRef` indices or match bindings by labels/insertion order.
- Keep recursion limit 64 and existing structured `RunError` variants.
- Defer parallelism, forced kernel termination, retries, and retry-policy DSL.
- Run focused serial tests only; no complete Rust suite.

---

## File Structure

- Modify `src-tauri/src/node_system/catalog/control.rs`: stable Branch/Loop authoring contracts.
- Modify `src-tauri/src/node_system/catalog/core_nodes/control.rs`: effect ports for real built-ins.
- Modify `src-tauri/src/node_system/compiler/control.rs`: stable binding resolution.
- Modify `src-tauri/src/node_system/compiler/project.rs`: function ABI identities.
- Modify `src-tauri/src/node_system/compiler/pipeline.rs`: retain control/ABI mappings.
- Modify `src-tauri/src/node_system/plan/model.rs` and `validation.rs`: direction-specific Call bindings.
- Modify `src-tauri/src/node_system/runtime/function_plan.rs`: publish plan ABI.
- Modify `src-tauri/src/node_system/runtime/scheduler.rs`: execute corrected ABI and normalize cancellation.
- Modify `src-tauri/src/project/project_state.rs`: publish current run-local function generation.
- Create `src-tauri/src/project/structured_control_production_tests.rs`.
- Modify `src-tauri/src/project/mod.rs` to register tests.

### Task 1: Freeze authorable built-in control/effect protocols

- [ ] **Step 1: Add failing Catalog tests** asserting Branch has stable result members, Loop has independently addressable carried members, and Do/Sleep expose declared effect input/output ports.
- [ ] **Step 2: Run the protocol test.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::catalog::tests::project_and_control_nodes_freeze_with_complete_protocol_contracts --exact --test-threads=1
```

- [ ] **Step 3: Implement protocol shapes** using declared or user-created `PortRef::Instance` identities. Preserve `StructuralNodeRole` IDs and localization completeness.
- [ ] **Step 4: Re-run Catalog tests** and freeze the exact stable keys before compiler changes.

### Task 2: Compile stable Branch and Loop bindings

- [ ] **Step 1: Add real-built-in compiler tests** for one Branch result, multiple Loop carried instances, Branch continuation, insertion-order independence, and blocking diagnostics for missing/ambiguous instances.
- [ ] **Step 2: Run the new focused filters** and confirm current key/first-match resolution fails.
- [ ] **Step 3: Replace `RegionBuilder::resolve_value` persistence assumptions** with stable `PortAddress`/instance references resolved to `ValueRef` only inside compilation.
- [ ] **Step 4: Ensure Branch uses `with_continuation`** when downstream operations consume its result.
- [ ] **Step 5: Extend plan validation** for exact Branch/Loop value bounds and duplicate/missing binding identities.
- [ ] **Step 6: Run compiler control filters.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::compiler::tests::branch_builds_exclusive_true_and_false_regions --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::compiler::tests::loop_uses_explicit_condition_limit_and_carried_bindings --exact --test-threads=1
```

### Task 3: Correct the cross-frame function Call ABI

**Interfaces:**

```rust
pub struct CallArgumentBinding {
    pub caller_source: ValueRef,
    pub callee_destination: ValueRef,
}

pub struct CallResultBinding {
    pub callee_source: ValueRef,
    pub caller_destination: ValueRef,
}
```

- [ ] **Step 1: Add failing compiler/runtime tests** where caller and callee numeric layouts deliberately differ but dynamic member locators match.
- [ ] **Step 2: Assert** argument transfer, result transfer, independent frames, stale ABI rejection, and current generation usage.
- [ ] **Step 3: Replace ambiguous `RegionValueBinding` in Call** with direction-specific bindings. Derive callee refs from compiled Entry/Return ABI keyed by `DynamicMemberLocator::FunctionParameter`; never resolve both sides in the caller plan.
- [ ] **Step 4: Publish ABI with each `FunctionPlanGeneration`** and validate it against the same session, Registry fingerprint, and resource versions as the plan.
- [ ] **Step 5: Update scheduler Call execution** to copy caller→callee arguments and callee→caller results explicitly.
- [ ] **Step 6: Run focused tests.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::tests::call_uses_an_independent_frame --exact --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- node_system::runtime::tests::recursive_calls_stop_at_the_configured_limit --exact --test-threads=1
```

### Task 4: Add production Branch coverage

- [ ] **Step 1: Create `structured_control_production_tests.rs`** with helpers for built-in node IDs, stable ports/instances, deterministic connections, temporary project activation, and recording run events.
- [ ] **Step 2: Add tests** `builtin_branch_executes_only_selected_effect_branch_and_binds_result`, false-path binding, and insertion-order independence.
- [ ] **Step 3: Build documents only from real built-ins**, execute through `ProjectState::execute_graph`, route result to a project variable, and assert only selected effect operations emit events.
- [ ] **Step 4: Run the Branch filters.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::structured_control_production_tests::builtin_branch --test-threads=1
```

### Task 5: Add production Loop coverage

- [ ] **Step 1: Add tests** for initial/next/result carried values, structured iteration-limit error, and project-drain cancellation between iterations.
- [ ] **Step 2: Use `ProjectRunRegistry::cancel_and_drain`** after an observed body completion; do not use arbitrary sleeps.
- [ ] **Step 3: Normalize a native kernel error to `RunError::Cancelled`** when the cancellation token is already cancelled.
- [ ] **Step 4: Assert** exact activation count, no completed result after cancellation, and zero active runs after drain.
- [ ] **Step 5: Run Loop filters.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::structured_control_production_tests::builtin_loop --test-threads=1
```

### Task 6: Add production Call and recursion coverage

- [ ] **Step 1: Add a persisted function fixture** with one parameter and result; materialize Entry/Return/Call derived ports by exact `FunctionParameterId` locator.
- [ ] **Step 2: Add tests** for argument/result binding, two independent calls, current function generation after body replacement, and recursive limit 64.
- [ ] **Step 3: Execute only through `ProjectState::execute_graph`** and assert authoritative variable results/error code.
- [ ] **Step 4: Run Call filters.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::structured_control_production_tests::builtin_call --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::structured_control_production_tests::builtin_recursive_call_stops_at_project_recursion_limit --exact --test-threads=1
```

### Task 7: Close effect ordering, no-retry, and cleanup

- [ ] **Step 1: Add production tests** where document insertion is `after,before` but explicit effect edge requires `before,after`; repeat with reversed maps.
- [ ] **Step 2: Add failure/cancellation tests** asserting one operation attempt and release of every acquired project resource. Add only a `#[cfg(test)]` lease-drop observer.
- [ ] **Step 3: Preserve `frame.attempted` semantics** and RAII `RunResourceSet`; do not add retries or production callbacks.
- [ ] **Step 4: Run effect filters.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::structured_control_production_tests::builtin_effect --test-threads=1
```

### Task 8: Slice verification

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project::structured_control_production_tests --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

Do not run the complete Rust suite.