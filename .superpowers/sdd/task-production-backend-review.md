# Production backend task re-review

## Verdict

1. **Spec compliance: ❌**
2. **Code quality: Issues**

Current severity count: **Critical 4, Important 5**.

The remediation materially improves the backend cut: normalized lifecycle commands now exist, result-source IPC is bounded and releasable, legacy project-index schemas are rejected, old graph DTO event modules are removed, and pre-run tracking closes the originally identified post-compile registration window. However, the implementation is still not production-correct. Normalized rename/duplication can leave incorrect resource references, the new statistics “adapters” silently implement unrelated algorithms, variable reads during Run are live rather than basis-bound snapshots, and schema v2 now accepts two different function-resource shapes. Additional concurrency and IPC synchronization issues remain.

## Previous finding disposition

| Previous finding | Status | Re-review conclusion |
|---|---|---|
| Critical 1 — graph lifecycle missing | **Partially fixed** | Commands and state methods now exist, but duplicate/rename do not preserve normalized references and current in-memory authority in all cases. |
| Critical 2 — runtime adapters missing | **Partially fixed** | Error stubs were replaced and function/relational wiring exists, but statistics kernels return silently incorrect generic calculations; lazy project DuckDB execution is still not covered/wired. |
| Critical 3 — project-session race | **Partially fixed** | Pre-run registration and session recheck close the original late-registration race, but function-resource disk loading occurs before protection and compile cancellation is not connected to the registered token. |
| Critical 4 — result IPC/leak | **Resolved for backend scope** | Descriptor/value/page/release/run-release commands exist and retained source count is capped. |
| Important 1 — legacy schemas hidden | **Resolved** | Index and local-variable reads now propagate malformed/unsupported graph errors. |
| Important 2 — undo/redo unreachable | **Partially fixed** | Commands exist, but they emit no authoritative delta/projection invalidation; function signature IPC is not revisioned. |
| Important 3 — obsolete DTO/events | **Resolved** | Old graph/function/node/connection event modules and variants are removed from the production event boundary. |
| Important 4 — replacement coverage | **Partially fixed** | Coverage is broader, but it proves availability rather than correctness for several high-risk paths and misses the remaining races/reference defects. |

## Findings

### Critical 1 — Rename and duplication do not preserve normalized resource identity

`ProjectState::rename_graph_resource` writes the target and cascades references only in files on disk (`src-tauri/src/project/project_state.rs:331-355`; `src-tauri/src/project/project_io.rs:184-240`). It does not update already-loaded caller documents or loaded variable scopes in `ProjectData`.

This breaks the required authority model:

1. caller graph A and function B are loaded;
2. B is renamed;
3. disk files are rewritten to the new path, but loaded A still contains B's old path;
4. `ProjectData`, which must be the current normalized authority, now disagrees with disk;
5. executing or later saving A can fail or restore the stale reference.

Function duplication is also incorrect. `duplicate_project_graph_file` copies the normalized document and changes only the file/name; it does not rebind the duplicated function's managed Entry/Return `function` parameters from the source path to the new path (`src-tauri/src/project/project_io.rs:411-451`). The lifecycle test duplicates only an Event with no resource references and therefore does not detect this (`src-tauri/src/project/production_tests.rs:37-70`).

The lifecycle remediation is not complete until path changes are applied transactionally to both loaded authoritative documents and persisted unloaded documents, and duplicated function self-identities are rebound before writing.

### Critical 2 — Statistics kernels silently execute the wrong algorithms

The former adapter errors were replaced, but the replacements do not preserve domain behavior. In `src-tauri/src/node_system/runtime/kernels/statistics/mod.rs:68-113`, many distinct operations are collapsed into generic branches:

- `OlsFit`, `GlsFit`, `LogitFit`, `PraisFit`, `ProbitFit`, `VecFit`, and `WlsFit` all call the same one-predictor straight-line fit;
- `LinearPredict`, `LogitPredict`, and `ProbitPredict` all use the same linear prediction;
- `AdfTest`, `VarLagOrder`, and `VecRankTest` all use the same generic test path;
- the selected `ScientificApi` is explicitly ignored (`let _ = self.api`).

`fit_outputs` is a simple univariate OLS slope/intercept calculation (`src-tauri/src/node_system/runtime/kernels/statistics/mod.rs:176-237`). Returning that result under Logit, Probit, GLS, VEC, WLS, and Prais node identities is materially worse than a fail-fast adapter error because users receive plausible but incorrect scientific output.

The focused test only checks that a fit returns a model/fitted/residual shape; it does not verify any operation-specific semantics. This fails production code quality and does not constitute behavioral replacement for the deleted scientific execution tests.

### Critical 3 — Runtime variable values are not bound to the compiled resource basis

`snapshot_project_resources` computes a resource-version basis from cloned variables, but runtime resources store `AuthorityVariableAccess` handles pointing back to live `ProjectData` (`src-tauri/src/project/project_state.rs:1169-1262`). During Run, variable get/set acquires the live project lock and reads or mutates the current variable (`src-tauri/src/project/project_state.rs:1134-1162`).

A variable can change after the stale-resource check and before its kernel reads it. The plan and `RunEvent` still carry the old resource version while execution consumes the new value. A variable-set node can likewise mutate authoritative state mid-run without updating the basis, project history, persistence, or the ordinary variable event boundary.

This violates the requirement that execution snapshot document/resources before compilation and execute against that exact basis. Reads need immutable run snapshots. If writes are intentionally supported, they need an explicit application-side effect/commit path with revision/event semantics rather than a live authority handle masquerading as a snapshot resource.

### Critical 4 — Schema v2 accepts two function-resource formats

The remediation added `FunctionDocument` without changing the graph schema version. To remain deserializable, both persistence and in-memory resource types model it as optional and default missing values:

- `GraphDocument.function: Option<FunctionDocument>` uses `#[serde(default, skip_serializing_if = "Option::is_none")]` (`src-tauri/src/project/project_io.rs:41-50`);
- `GraphFileHeader.function` also defaults (`src-tauri/src/project/project_io.rs:669-677`);
- `GraphResourceDocument.function` is optional (`src-tauri/src/project/project_data.rs:8-16`).

`read_graph_document` validates schema version and directory kind but never enforces `Function => Some(function)` and `Event => None` (`src-tauri/src/project/project_io.rs:600-624`). Therefore a schema-v2 function file from the earlier implementation, with no function document, is still accepted; an Event carrying a function document is also accepted. Those are dual schema-v2 representations and an implicit compatibility fallback, contrary to the explicit “no fallback/dual format” requirement.

The canonical schema must reject kind/function mismatches. If adding `FunctionDocument` changed the required schema, use a new schema version rather than accepting both shapes under v2.

### Important 1 — Pre-run protection starts after project-dependent disk I/O and does not cancel compilation

`execute_graph` calls `load_function_resources()` before it snapshots the session and registers the pre-run lease (`src-tauri/src/project/project_state.rs:621-650`). `load_function_resources` reads the active project index/files and inserts graphs into `ProjectData` (`src-tauri/src/project/project_state.rs:599-619`). A concurrent project replacement can therefore occur during these reads; the old project's function resources can be inserted after the new project is activated.

After registration, compilation receives newly created `CompileCancellationToken` instances rather than the registered runtime cancellation token (`src-tauri/src/project/project_state.rs:653-679`, `1303-1336`). Project drain sets the runtime token, but compilation continues until completion; drain merely waits for the pre-run guard. This contradicts the report's claim that the same cancellation token covers compilation-to-run handoff.

The original late-run race is closed, but all project-dependent preparation must be inside the protected session, and compiler cancellation must be bridged to the registered token.

### Important 2 — Shared function-plan publication is not atomic across concurrent executions

Every execution mutates the project-global `FunctionPlanStore` by calling `set_current_basis`, `clear`, and then publishing plans one at a time (`src-tauri/src/project/project_state.rs:1303-1361`). `FunctionPlanStore` protects current basis and plans with separate locks (`src-tauri/src/node_system/runtime/function_plan.rs:18-28`, `49-95`).

Two concurrent executions can interleave as follows:

1. run A clears and starts publishing;
2. run B clears A's plans and starts publishing;
3. run A begins execution while B's store is only partially populated;
4. A's function call receives `None`/stale failure despite compiling against a valid complete snapshot.

Publishing a basis plus its complete function plan map needs one atomic generation/snapshot, or function plans should be run-local immutable providers. The focused test checks one sequential execution only.

### Important 3 — History IPC mutations do not participate in projection/event synchronization

`undo_graph_document` and `redo_graph_document` call Rust history but return `()` and emit no `GraphDelta`, projection replacement, or project-index invalidation (`src-tauri/src/commands/command_node_system.rs:195-203`). Clients therefore have no authoritative event telling them which graph/function revision changed.

`update_function_signature` accepts a bare `FunctionSignature`, not a revisioned `MutationRequest`, generates its operation ID internally, emits no delta, and returns only one English projection (`src-tauri/src/commands/command_node_system.rs:145-158`; `src-tauri/src/project/project_state.rs:497-536`). This does not meet the revisioned mutation IPC rule for function-resource writes.

The current frontend call contract also remains incompatible: `src/services/graph/graphService.ts:63-83` sends `inputs`/`outputs` and expects old `GraphInstanceDTO` caller graphs, while the backend requires `signature` and returns `EditorGraphProjectionDto`. Although this task is backend-scoped, the command is not usable by the current production application without the corresponding frontend cut.

### Important 4 — Relational coverage still bypasses the real lazy project database path

`snapshot_project_resources` only includes DataFrames already in `DatabaseState::Loaded` (`src-tauri/src/project/project_state.rs:1180-1193`, `1248-1252`). The new relational test manually constructs a `ProjectResourceProvider` with an in-memory DataFrame and invokes `ProductionRelationalBackend` directly (`src-tauri/src/project/production_tests.rs:272-330`).

It does not execute a graph through `ProjectState::execute_graph`, does not start from a project DuckDB declaration, and does not verify lazy acquisition/materialization. A declared but not already-loaded project database still reaches runtime without a corresponding resource lease and fails acquisition. The original report concern about the lazy DuckDB path remains Important; the remediation report overstates it as closed.

### Important 5 — Remediation coverage and report status overstate closure

The added tests are useful smoke coverage, but they do not cover:

- function duplication rebinding managed self-paths;
- rename while caller graphs/local variables are loaded;
- schema-v2 function files missing `FunctionDocument` or Event files carrying one;
- operation-specific statistical correctness;
- variable changes between basis validation and kernel read;
- concurrent `FunctionPlanStore` publication/execution;
- project replacement during pre-registration function loading;
- actual compiler cancellation during drain;
- undo/redo/function signature projection events and revision conflict behavior;
- lazy DuckDB graph execution through the production command path.

Accordingly, report status `DONE` and “Remaining blockers and concerns: None” are not supported by the current code.

## Focus-area conclusions

### ProjectData as the unique normalized authority

**Partial.** `ProjectData.graphs` still stores only normalized `GraphResourceDocument`/`GraphDocument`/`FunctionDocument`, never `GraphInstance`. Normal mutation/projection/execution starts from it. However, rename updates unloaded disk documents without synchronizing loaded authoritative documents, and runtime variable handles bypass immutable resource snapshots by reading/writing live `ProjectData` during Run.

### Legacy GraphInstance / Registry / Executor production reachability

**Pass for command/runtime authority.** The old graph commands are not registered, old event DTO modules are removed, and no production construction of the legacy `GraphInstance`, mutable registry, or old `Executor` was found. The old modules remain privately compiled only because non-node database/scientific/shared-value code still depends on types under them, which is allowed by the brief.

### Fallbacks, aliases, and dual formats

**Fail.** No old graph reader/converter or old-ID alias path was found, and old schema versions are now rejected at index/load boundaries. But optional/default `FunctionDocument` creates two accepted schema-v2 function shapes, which is a dual format/fallback.

### Mutation, history, and project locks

**Partial.** Graph patches remain revisioned and Rust-history-owned. Filesystem I/O, compile, projection construction, event emission, and long execution are outside project locks. Function signature, undo, and redo IPC do not expose revision conflict/delta semantics. Variable kernels acquire live project locks and mutate authority during Run outside project history/event synchronization.

### Projection IPC

**Partial.** Load/hydrate return localized `EditorGraphProjectionDto`, and old DTO hydration is gone. Graph patch emits `GraphDelta`. Function signature/undo/redo do not emit equivalent projection/delta synchronization, and function signature hardcodes `en-US`.

### Execution stale basis and cleanup

**Fail on basis; pass on result retention mechanics.** Session recheck and pre-run tracking improve stale protection, and result sources now have read/release IPC plus count-bounded eviction. Live variable reads violate resource basis; pre-registration function loading and non-atomic shared function plan publication remain unsafe. The source cap bounds count, not total bytes, but deterministic release and project-store teardown prevent the original unbounded-source-count defect.

### Deleted tests and replacement coverage

**Partial.** Smoke coverage now exists for every previously listed category, but several tests validate only existence/shape and do not replace domain correctness or concurrency/lifecycle invariants. In particular, the statistics test allows incorrect algorithms to pass.

### Report concerns

- Previous concern 1 (function plans): sequential publication/call now exists, but concurrent publication remains Important.
- Previous concern 2 (DuckDB): still Important; the real lazy project database path is not covered or supplied.
- Full `pnpm rust:test` omission: not Important under repository policy.
- LF→CRLF notices: not Important.

## Code quality assessment

**Issues.** The structural direction is improved, and several former blockers have clean targeted fixes. Approval is blocked by silent scientific miscalculation, basis/authority violations, and non-atomic lifecycle/concurrency behavior. These are root correctness issues, not cosmetic concerns.

## Review validation

This re-review used the current mixed staged/unstaged/untracked workspace. I reread the brief, appended report, previous review, current diff inventory, and the affected production lifecycle, persistence, history, projection, execution, runtime resource, function-plan, result-store, event, tests, and frontend service code. I did not rerun builds or tests, per the request not to repeat heavy validation. No production code was modified; only this requested review document was overwritten.
