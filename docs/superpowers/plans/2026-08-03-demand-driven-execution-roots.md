# Demand-Driven Execution Roots Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make canonical graph execution honor stable requested outputs and `EvaluationPolicy`, pruning unrequested pure work and resources while preserving structured control, effects, relational integrity, and frontend pin preview identity.

**Architecture:** Rust caches one full basis-level compiler product and derives validated execution plans from normalized demand keys. Compiler pruning occurs before relational grouping and final operation indexing. Canonical IPC and frontend preview use stable graph/port identities, never compiler-local value indices.

**Tech Stack:** Rust, Serde, Tauri, TypeScript, React/Zustand, Vitest, pnpm.

## Global Constraints

- Work directly on `shadcn`; do not create a worktree, branch, commit, or tag.
- Preserve unrelated dirty work.
- Rust owns graph protocol, semantic analysis, compilation, resources, and execution.
- Never expose or persist `ValueRef`, `OperationIndex`, or `valueIndex` as request/result identity.
- Requested outputs do not enter `CompilationBasis`.
- Full graph analysis/diagnostics remain authoritative; only executable plans are demand-specialized.
- Preserve existing Branch, Loop, Call, effect ordering, cancellation, finalization, resource cleanup, recursion limit 64, and current function-generation semantics.
- No nested function preview, callee specialization, cross-run cache, scheduler parallelism, timeout policy, Filter migration, automatic terminal inference, or legacy execution-stack rewrite.
- Run focused Rust tests serially with `CARGO_BUILD_JOBS=1`; do not run complete suites by default.
- Update `.superpowers/sdd/2026-08-03-demand-driven-execution-roots/progress.md` and `TODO.md` after every independently reviewed task.

---

## File Structure

### Stable demand protocol

- Modify `src-tauri/src/node_system/plan/model.rs`: own `GraphOutputRef`, demand/result metadata if plan-domain placement fits existing boundaries.
- Modify `src-tauri/src/node_system/document/mutation.rs` or the existing execution DTO module only to reuse/convert `PortAddressDto`; do not duplicate port address grammar.
- Modify `src-tauri/src/commands/node_system_execution_dto.rs`: strict execution demand and stable output-ready DTOs.
- Modify `src/shared/types/dto/runEvent.ts` and create `src/shared/types/dto/executionDemand.ts` plus `executionDemand.test.ts`.

### Compiler specialization

- Modify `src-tauri/src/node_system/compiler/pipeline.rs`: retain full lowering facts and derive demand closure.
- Modify `src-tauri/src/node_system/compiler/lowering.rs`: associate results/resources with stable outputs/owners.
- Modify `src-tauri/src/node_system/compiler/control.rs`: project retained structured regions without changing control semantics.
- Modify `src-tauri/src/node_system/compiler/relational.rs`: consume retained fragment roots and preserve bridge/cardinality contracts.
- Modify `src-tauri/src/node_system/plan/validation.rs`: validate stable results and pruned references.

### Project execution and cache

- Modify `src-tauri/src/node_system/compiler/coordinator.rs` and `src-tauri/src/project/compile_publication.rs`: retain one basis product and bounded demand variants.
- Modify `src-tauri/src/project/project_state.rs`: accept execution demand and select/derive the plan before resource preflight.
- Modify `src-tauri/src/commands/command_node_system.rs`: parse strict demand and emit stable output events.

### Frontend execution

- Modify `src/services/project/projectService.ts`: send exact demand.
- Modify `src/features/application/editor/useProjectOperations.ts`: use default demand for ordinary run.
- Modify `src/features/application/editor/observeGraphRunEvent.ts`: consume stable output events.
- Modify existing pin preview/application/store modules under `src/features/core/execution/` and `src/features/application/editor/` to request and index `(graphPath, PortAddressDto)`.

---

### Task 1: Freeze stable output identity and execution-demand contracts

**Files:**

- Modify: `src-tauri/src/node_system/plan/model.rs`
- Modify: `src-tauri/src/commands/node_system_execution_dto.rs`
- Modify: existing `PortAddressDto` conversion owner under `src-tauri/src/node_system/document/`
- Modify: `src/shared/types/dto/runEvent.ts`
- Modify: focused Rust serde/plan-model tests and frontend DTO tests

**Interfaces:**

```rust
pub struct GraphOutputRef {
    pub graph_path: GraphResourcePath,
    pub port: PortAddress,
}

pub enum ExecutionDemand {
    Default,
    Outputs {
        outputs: Box<[GraphOutputRef]>,
        include_default_results: bool,
    },
}
```

```ts
type ExecutionDemandDto =
  | { type: 'default' }
  | {
      type: 'outputs';
      outputs: Array<{ graphPath: string; port: PortAddressDto }>;
      includeDefaultResults: boolean;
    };
```

- [ ] **Step 1: Add RED strict-serde and identity tests**

Cover default, declared output, dynamic instance output, empty outputs, duplicate caller order as accepted input, missing/extra fields, invalid tags, and explicit absence of `valueIndex`/operation-index request fields.

- [ ] **Step 2: Run RED focused tests serially**

Run:

```sh
CARGO_BUILD_JOBS=1 pnpm rust:test --lib execution_demand -- --test-threads=1
pnpm test src/shared/types/dto/executionDemand.test.ts
```

Confirm failures are due to missing demand/output contracts.

- [ ] **Step 3: Implement stable types and strict conversions**

Reuse `PortAddressDto` and its declared/instance conversion. Do not define a second frontend/backend port-address grammar.

- [ ] **Step 4: Extend plan result metadata**

Add stable output identity to requested/default graph results while retaining `ValueRef` as an internal plan value. Update plan validation for duplicate stable output identities, duplicate names, and invalid values.

- [ ] **Step 5: Add stable `OutputReady` DTO**

The canonical event carries `GraphOutputRefDto` and `sourceId`. Existing compiler-local events must not become request identity. Update strict frontend event parsing/types.

- [ ] **Step 6: Run GREEN tests and gates**

Run focused Rust model/DTO/plan validation tests and frontend DTO tests, then:

```sh
pnpm typecheck
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

- [ ] **Step 7: Independent review and publication**

Reviewer verifies stable identity, strict serde, no duplicate port grammar, no compiler-local wire identity, and validation completeness. After approval, update ledger and `TODO.md` Phase 6.

---

### Task 2: Derive pure/eager/effect demand closure and owned resources

**Files:**

- Modify: `src-tauri/src/node_system/compiler/pipeline.rs`
- Modify: `src-tauri/src/node_system/compiler/lowering.rs`
- Modify: `src-tauri/src/node_system/plan/model.rs`
- Modify: `src-tauri/src/node_system/plan/validation.rs`
- Test: focused compiler and plan tests

**Interfaces:**

The basis-level lowering product must retain:

```text
stable PortAddress → internal value lookup
operation evaluation/purity/effect semantics
operation inputs/outputs
operation-owned resource requirements
default FragmentMetadata results
value/effect dependencies
```

It produces a normalized demand key and a pruned intermediate operation/value/resource set before relational grouping.

- [ ] **Step 1: Add RED independent-chain tests**

Create two disconnected pure chains with observable counting kernels/resources. Request only chain A. Assert the derived plan excludes chain B operations, results, and resources.

- [ ] **Step 2: Add RED demand normalization/default-result tests**

Assert `[A, B]` and `[B, A, A]` produce the same normalized key/plan. Cover `Default`, `Outputs` without defaults, and `Outputs` unioned with defaults.

- [ ] **Step 3: Preserve protocol execution semantics in lowering facts**

Record `EvaluationPolicy`, `Purity`, and effect semantics directly from `NodeProtocol.execution`. Do not infer evaluation from purity or connections.

- [ ] **Step 4: Implement reverse value closure**

Build producer/incoming indexes, retain demanded producers, and recursively demand required inputs until fixed point. Reject invalid requested ports before plan/resource construction.

- [ ] **Step 5: Implement eager/effect closure**

Retain `EagerWhenRegionEntered` operations in their region and retain transitive effect-order predecessors. Filter effect dependencies only after both endpoints are retained.

- [ ] **Step 6: Implement operation-owned resource aggregation**

Associate lowerer resource requirements with their owning intermediate operation/fragment. Aggregate only retained resources with deterministic deduplication.

- [ ] **Step 7: Add resource and zero-work production proofs**

Tests prove an unavailable resource used only by pruned chain B is neither validated nor acquired, while retained chain A resources preserve preflight/RAII behavior.

- [ ] **Step 8: Run focused GREEN tests and gates**

Run compiler, plan, runtime counter/resource filters serially, then Rust check/fmt and diff check.

- [ ] **Step 9: Independent review and publication**

Reviewer verifies evaluation semantics are authoritative, closure direction is correct, unrequested pure work/resources disappear, eager/effect roots remain, and full diagnostics are unchanged. Update ledger/TODO after approval.

---

### Task 3: Preserve structured-control and relational integrity during pruning

**Files:**

- Modify: `src-tauri/src/node_system/compiler/control.rs`
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs`
- Modify: `src-tauri/src/node_system/compiler/relational.rs`
- Modify: `src-tauri/src/node_system/plan/validation.rs`
- Test: compiler/plan tests and existing structured-control/relational production fixtures

- [ ] **Step 1: Add RED `If` closure tests**

Demand a branch result and assert condition, both result sources, retained arm structure, and branch bindings remain. Add unrelated branch-local pure work that must be removed and branch-local eager work that must remain in its arm.

- [ ] **Step 2: Add RED `Loop` closure tests**

Demand a loop result and assert initial/body-input/next/result/condition closure. Remove unrelated body-local pure work while preserving eager body work and iteration semantics.

- [ ] **Step 3: Add RED `Call` closure tests**

Demand a caller result and retain Call plus all required arguments. Verify the callee plan remains complete and recursion/function-generation behavior is unchanged.

- [ ] **Step 4: Project retained structured regions**

Prune using temporary node/operation identities, then assign dense `OperationIndex` values. Never delete a structured binding source still referenced by the retained region.

- [ ] **Step 5: Prune before relational grouping**

Feed only retained fragments/outputs to the relational planner. Preserve cross-island bridge roots, deterministic grouping, and owner-output/compiled-root cardinality/order.

- [ ] **Step 6: Strengthen derived-plan validation**

Reject references to deleted operations/values, incomplete branch/loop/call bindings, broken effect edges, and relational cardinality mismatches.

- [ ] **Step 7: Run existing production regressions**

Run focused Branch/Loop/Call/effect tests, Source→Rename→Limit relational tests, compiler tests, and plan tests serially. Do not run complete Rust suite.

- [ ] **Step 8: Independent review and publication**

Reviewer verifies no compile-time branch selection, Loop carried semantics, Call frame/ABI preservation, effect ordering, relational bridge/root integrity, and insertion-order determinism. Update ledger/TODO after approval.

---

### Task 4: Add basis-level demand variants to ProjectState execution

**Files:**

- Modify: `src-tauri/src/node_system/compiler/coordinator.rs`
- Modify: `src-tauri/src/project/compile_publication.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Modify: `src-tauri/src/commands/command_node_system.rs`
- Modify: `src-tauri/src/commands/node_system_execution_dto.rs`
- Test: compiler coordinator, project production, command execution, runtime event tests

**Interfaces:**

- One current full compiler product remains keyed by `CompilationBasis`.
- Derived variants are keyed by deterministic normalized demand key/digest.
- Variant cache is bounded and cannot overwrite current analysis/projection.
- `ProjectState::execute_graph` accepts demand before resource preflight.

- [ ] **Step 1: Add RED compile-reuse tests**

Default run plus two preview demands at one basis perform one full analysis/lowering capture. Variants have distinct deterministic selection digests and do not replace current compile analysis.

- [ ] **Step 2: Add RED invalidation tests**

Graph revision, registry fingerprint, resource versions, project replacement, and authority mismatch invalidate/reject old variants through the existing basis/session gates.

- [ ] **Step 3: Implement bounded variant cache**

Normalize demand before lookup. Eviction must not affect correctness. Do not place demand in `CompilationBasis` or create a second publication stream.

- [ ] **Step 4: Select/derive before resource preflight**

`ProjectState::execute_graph` validates demand against the authoritative full product, derives/selects the plan, then captures/acquires only retained resources.

- [ ] **Step 5: Update canonical command request**

Require strict demand DTO; ordinary callers send `Default`. Remove old command overloads and update every Rust call site/test.

- [ ] **Step 6: Emit stable output events**

Map retained `PlanResult.output` to `OutputReady`. Preserve terminal event ordering, channel drain, cancellation, run registry, finalization, and resource mutation publication.

- [ ] **Step 7: Run project/runtime/command GREEN tests**

Run exact compile-publication, project execution, RunExecutor, run-registry, structured-control, cancellation, and command filters serially.

- [ ] **Step 8: Independent review and publication**

Reviewer verifies full-product reuse, bounded variants, basis separation, authority validation before resources, stable events, and no run/finalization regression. Update ledger/TODO after approval.

---

### Task 5: Route default execution and top-level pin preview through stable demands

**Files:**

- Modify: `src/services/project/projectService.ts`
- Modify: `src/features/application/editor/useProjectOperations.ts`
- Modify: `src/features/application/editor/observeGraphRunEvent.ts`
- Modify: `src/features/core/execution/useExecutionStore.ts`
- Modify: `src/features/core/execution/pinResultIndex.ts`
- Modify: `src/features/core/execution/index.ts`
- Create: `src/features/application/editor/requestPinPreview.ts`
- Test: `src/services/project/projectService.execution.test.ts`
- Test: `src/features/application/editor/observeGraphRunEvent.test.ts`
- Test: `src/features/application/editor/requestPinPreview.test.ts`
- Test: `src/features/core/execution/pinResultIndex.test.ts`
- Test: `src/features/core/execution/useExecutionStore.lifecycle.test.ts`

- [ ] **Step 1: Add RED service wire tests**

Assert ordinary run sends `{ type: 'default' }`; declared and dynamic preview requests send exact `GraphOutputRefDto`; malformed/compiler-local identities are not accepted by types/adapters.

- [ ] **Step 2: Add RED stable output-event tests**

Assert `OutputReady` updates only the matching `(graphPath, PortAddressDto)` preview entry, including dynamic instances. Stale project/run completions cannot overwrite newer preview state.

- [ ] **Step 3: Update service and ordinary run call sites**

`ProjectService.executeGraphDocument` requires demand. `useProjectOperations` always sends `Default` for ordinary execution.

- [ ] **Step 4: Implement top-level pin preview application flow**

Build the request only from Rust-projected pin `PortAddressDto`. Reject non-data-output, orphan, nested-function, or missing-session previews before IPC using existing toast/error patterns.

- [ ] **Step 5: Consume stable output events**

Index preview results by stable graph/port identity. Do not use `valueIndex`, result name, label, insertion order, or resource-path parsing.

- [ ] **Step 6: Preserve channel terminal semantics**

Existing terminal event drain, command rejection precedence, cancellation, and execution-store finalization tests remain green.

- [ ] **Step 7: Run focused GREEN tests and typecheck**

Run service execution, project operations, event observation, pin preview/index/store, graph session, and stale lifecycle tests, followed by typecheck and diff check.

- [ ] **Step 8: Independent review and publication**

Reviewer verifies exact stable forwarding, top-level scope, stale suppression, no direct view invoke, service dependency direction, and no compiler-local identity. Update ledger/TODO after approval.

---

### Task 6: Final focused verification and whole-slice review

**Files:**

- Modify: `.superpowers/sdd/2026-08-03-demand-driven-execution-roots/progress.md`
- Modify: `TODO.md`

- [ ] **Step 1: Run all explicit frontend demand/preview/lifecycle files**

Run this explicit set in one command without an extra `--` separator:

```sh
pnpm test src/shared/types/dto/executionDemand.test.ts src/services/project/projectService.execution.test.ts src/features/application/editor/observeGraphRunEvent.test.ts src/features/application/editor/requestPinPreview.test.ts src/features/core/execution/pinResultIndex.test.ts src/features/core/execution/useExecutionStore.lifecycle.test.ts src/features/core/projectLifecycle/projectLifecycleAuthority.test.ts src/services/project/projectFilesystemContract.test.ts src/services/database/databaseService.test.ts src/features/application/dataManagement/useDatabaseManagement.test.tsx src/features/application/resource/resourceActions.test.ts src/features/core/dataStore/projectIOStore.test.ts src/features/core/sync/utils/resourceMutationWireValidator.test.ts src/features/application/editorMutation/projectPublicationCoordinator.test.ts src/features/application/editorMutation/projectPublicationProductionStores.test.ts src/features/core/sync/handlers/ProjectMutationEventHandler.test.ts src/services/nodeSystem/catalogService.test.ts src/features/core/nodeCatalog/nodeCatalogStore.test.ts src/features/application/nodeCatalog/useLocalizedNodeCatalog.test.tsx src/features/application/nodeCatalog/createNodeFromDescriptor.test.ts src/features/application/dataManagement/useNodeManagement.test.tsx src/features/application/editor/canvasDrop/spawnFromTemplate.test.ts src/views/EditorView/Layout/NodeDocumentationModal.test.tsx src/views/EditorView/Layout/NodePalette.test.tsx src/services/nodeSystem/nodeCatalogArchitectureContract.test.ts src/features/application/projectCommandContext.test.ts src/features/application/dataManagement/databaseMutation.test.ts src/features/application/dataManagement/variableActions.test.ts src/features/application/editorMutation/functionSignatureCoordinator.test.ts
```

- [ ] **Step 2: Run all focused Rust filters serially**

Include compiler, plan, relational, runtime, project production, structured-control, command execution, compile-publication, run registry, cancellation, Resource Catalog, ProjectIndex, and database integration filters touched by Tasks 1–5.

- [ ] **Step 3: Run final gates**

```sh
pnpm typecheck
CARGO_BUILD_JOBS=1 pnpm rust:check
pnpm rust:fmt:check
git diff --check
```

- [ ] **Step 4: Dispatch independent whole-slice review**

Reviewer must explicitly verify:

```text
stable GraphOutputRef identity
no ValueRef/OperationIndex/valueIndex request identity
CompilationBasis independent from DemandKey
full diagnostics with demand-specialized plans
pure pruning and eager/effect closure
operation-owned resource pruning
If/Loop/Call/effect behavior preservation
relational root/bridge integrity
bounded variant cache and authority invalidation
stable OutputReady frontend preview
```

- [ ] **Step 5: Publish only with fresh controller evidence**

Append exact counts and review verdict. Raise Phase 6 only when no Critical or Important findings remain. Record future CachePolicy/timeout/parallel/Filter work separately rather than claiming it in this slice.
