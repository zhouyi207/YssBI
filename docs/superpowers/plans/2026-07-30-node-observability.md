# Node Observability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace production no-op-only tracing with a bounded project-scoped compile/run trace buffer, read-only IPC, and a focused developer details projection.

**Architecture:** A bounded sink implements the existing `TraceSink`; ProjectStore owns one sink per active project and injects it into compiler, function compiler, scheduler, and relational execution. ProjectState exposes lifecycle-validated read APIs; frontend service/hook/view remain read-only and keep no authoritative trace store.

**Tech Stack:** Rust TraceSink/Tauri, VecDeque/Mutex, React/TypeScript/Vitest, existing detail sidebar.

## Global Constraints

- Work on `shadcn`; no worktree or commit.
- Bounded in-memory traces only; no persistence, external telemetry, clear/delete command, or frontend mutation.
- Never record runtime values, data rows, credentials, or graph documents.
- One FIFO per active project, not per tab/graph.
- Preserve compiler/runtime trace interfaces and correlation identities.
- Focused explicit tests only; the complete Rust suite runs later, once after all six slices.

---

## File Structure

- Create `src-tauri/src/node_system/analysis/trace_store.rs`.
- Modify `analysis/observability.rs` and `analysis/mod.rs`.
- Modify `runtime/scheduler.rs` for relational spans.
- Modify `project/project_store.rs` and `project_state.rs` for ownership/injection.
- Create `project/project_traces.rs` and `commands/command_trace.rs`.
- Modify command/module/lib registrations.
- Create frontend trace DTO/service/hook/view files and tests.
- Modify Event/Function detail panels and locale files.

### Task 1: Add bounded sink and complete span vocabulary

**Interfaces:**

```rust
pub const DEFAULT_PROJECT_TRACE_CAPACITY: usize = 4096;

pub struct TraceRecord {
    pub sequence: u64,
    pub event: SpanEvent,
}

pub struct BoundedTraceSink { /* Mutex<VecDeque<TraceRecord>> */ }
```

- [ ] **Step 1: Add failing sink tests** for capacity, oldest eviction, monotonic order, exact graph/run filtering, and zero-capacity rejection.
- [ ] **Step 2: Add failing scheduler tests** for `SpanKind::RelationalBackend` success/failure/cancellation and full correlation.
- [ ] **Step 3: Run focused filters.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- bounded_trace_sink --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- relational_backend_trace --test-threads=1
```

- [ ] **Step 4: Implement `BoundedTraceSink`** with sequence assignment and eviction under one short mutex. Query methods clone retained records oldest-first.
- [ ] **Step 5: Add `SpanKind::RelationalBackend`** and wrap backend execution with started/terminal events using operation correlation. Public fields may contain backend/subplan IDs only.
- [ ] **Step 6: Re-run tests.**

### Task 2: Own and inject one sink per active project

- [ ] **Step 1: Add failing production tests** proving real `execute_graph` emits compile/run spans with current session and project replacement installs an empty distinct sink.
- [ ] **Step 2: Add `Arc<BoundedTraceSink>` to `ProjectStore::default`** so project activation replacement naturally drops old traces.
- [ ] **Step 3: Snapshot the Arc under a short store lock** in `execute_graph`; inject it into main GraphCompiler, `publish_function_plans`, and `RunExecutor::with_trace_sink`.
- [ ] **Step 4: Change `publish_function_plans`** to accept `&dyn TraceSink`; remove production hard-coded `NOOP_TRACE_SINK` sites.
- [ ] **Step 5: Run production observability tests.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- production_observability --test-threads=1
```

### Task 3: Add lifecycle-safe read-only query and IPC

**Interfaces:**

```rust
pub fn ProjectState::list_graph_traces(
    &self,
    expected_project_instance_id: &ProjectInstanceId,
    graph_path: &GraphResourcePath,
) -> Result<Vec<TraceRecord>, TraceQueryError>;

pub fn ProjectState::get_run_trace(
    &self,
    expected_project_instance_id: &ProjectInstanceId,
    run_id: RunId,
) -> Result<Vec<TraceRecord>, TraceQueryError>;
```

- [ ] **Step 1: Add failing project tests** for exact graph/run filtering, empty graph result, evicted run not found, stale project rejection, and replacement isolation.
- [ ] **Step 2: Implement `project_traces.rs`** using capture/validate session before and after sink snapshot. Graph traces may remain visible after unload while retained.
- [ ] **Step 3: Add explicit serializable DTOs** for sequence, kind/status, full correlation, and redacted values.
- [ ] **Step 4: Implement thin commands** `list_graph_traces` and `get_run_trace`, mapping `trace_project_stale` and `trace_not_found`; register in `lib.rs`.
- [ ] **Step 5: Run project/command filters.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- project_trace_query --test-threads=1
CARGO_BUILD_JOBS=1 pnpm rust:test -- command_trace --test-threads=1
```

### Task 4: Add frontend service and stale-safe application hook

- [ ] **Step 1: Add service tests** asserting exact command names and camelCase arguments.
- [ ] **Step 2: Add hook tests** for initial graph query, refresh, run selection, stale project suppression, and evicted run handling.
- [ ] **Step 3: Implement `src/shared/types/dto/trace.ts`** with explicit span/status/value unions.
- [ ] **Step 4: Implement `TraceService`** as sole IPC owner.
- [ ] **Step 5: Implement `useGraphTraceDetails(graphPath)`** using `captureProjectCommandContext`; retain only component-local loading/error/selection state and discard stale completions.
- [ ] **Step 6: Run exact tests.**

```sh
pnpm test -- src/services/nodeSystem/traceService.test.ts src/features/application/observability/useGraphTraceDetails.test.tsx
```

### Task 5: Add focused read-only developer projection

- [ ] **Step 1: Add `GraphTraceDetails.test.tsx`** for collapsed state, refresh, sequence/status/correlation, run selection, public fields, redacted marker, and absence of mutation controls.
- [ ] **Step 2: Implement `GraphTraceDetails`** with shadcn controls inside the existing detail layout contract.
- [ ] **Step 3: Embed it collapsed-by-default** in Event and Function detail panels; do not add a Zustand trace store or global panel.
- [ ] **Step 4: Update localized labels** in en-US and zh-CN.
- [ ] **Step 5: Run exact view tests.**

```sh
pnpm test -- src/views/EditorView/Layout/Detail/observability/GraphTraceDetails.test.tsx
```

### Task 6: Repair the production execution service contract before trace UI acceptance

The existing frontend invokes `execute_project`, while Rust registers `execute_graph_document`. Do not add a compatibility command.

- [ ] **Step 1: Add/update the project service wire test** to expect `execute_graph_document` with the current graph path and channel DTO.
- [ ] **Step 2: Change the service invoke name/payload** to the registered canonical command and keep views free of direct IPC.
- [ ] **Step 3: Run the exact service test** plus trace service tests.

### Task 7: Slice and final delivery verification

- [ ] **Step 1: Run all Slice 6 frontend files explicitly.**

```sh
pnpm test -- src/services/nodeSystem/traceService.test.ts src/features/application/observability/useGraphTraceDetails.test.tsx src/views/EditorView/Layout/Detail/observability/GraphTraceDetails.test.tsx
pnpm typecheck
```

- [ ] **Step 2: Run focused Rust/check gates.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

- [ ] **Step 3: Only after all six slice plans are complete and focused gates pass, run the complete Rust suite exactly once.**

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test -- --test-threads=1
```

If it OOMs, stalls, or times out, record the result and do not retry.