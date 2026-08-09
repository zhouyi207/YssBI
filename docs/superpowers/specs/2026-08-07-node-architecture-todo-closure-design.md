# Node Architecture TODO Closure Design

**Date:** 2026-08-07
**Status:** Implemented and verified (2026-08-09)
**Scope:** Close every unchecked architecture item in `TODO.md:293-316` and remove the Windows linker-memory exception from the canonical verification workflow.

## 1. Goals

This work completes the remaining node-architecture authority, compiler, runtime, observability, search, and boundary-cleanup obligations without preserving legacy compatibility paths.

The delivered system must:

1. consume Rust-authoritative function/editor projections without rebuilding resolved pins in React;
2. remove legacy node creation and display-name identity DTOs from production TypeScript;
3. make descriptor validation and prepared patches the only production `GraphDocument` mutation route;
4. install revisioned Rust projection replacements for resource reference cascades rather than mutating graph projections in React;
5. record only resources actually read during analysis in `CompilationBasis`;
6. report all user-fixable lowerability failures during Analysis;
7. publish only explicitly demanded/default results and independent Pin previews;
8. carry `CachePolicy` into plans and implement run-scoped memoization;
9. insert explicit materialization adapters at producer/consumer contract boundaries;
10. implement bounded streaming, backpressure, workload-aware parallel scheduling, deadline propagation, and safe retry;
11. restore all Catalog search fields, including deterministic offline full-pinyin and initials indexing;
12. emit paired, timed, hierarchical trace spans;
13. remove the extra `parameter_types/` top-level boundary, localization compatibility bridge, and History legacy decoding;
14. make canonical Rust verification avoid Windows parallel-link memory exhaustion and make `pnpm verify` pass without an exception.

## 2. Delivery strategy

Implementation uses vertical, independently reviewable batches. Every batch begins with a focused RED regression or semantic architecture contract, closes the Rust authority and frontend consumer together where applicable, and returns the repository to a compilable, reviewable state.

The batches are:

1. canonical verification stabilization;
2. frontend projection and legacy identity cleanup;
3. `GraphDocument` mutation authority;
4. exact analysis dependencies and analysis-stage lowerability;
5. demand-driven publication and per-run memoization;
6. explicit materialization and bounded streaming;
7. scheduler parallelism, deadline, and retry;
8. Catalog search and trace spans;
9. module-boundary and wire-compatibility cleanup;
10. architecture audit and final verification.

Independent test or audit work may run in parallel inside a batch, but analysis/compiler/runtime interface changes remain sequential across batches.

## 3. Global constraints

- `ProjectState.project_data` remains authoritative for project state.
- Tauri commands remain thin and do not own workflows or long I/O.
- Frontend services own IPC parsing; views do not invoke Tauri directly.
- React stores remain projections and UI state, not business authority.
- No project-global lock may be held during I/O, channel waits, retry sleeps, model loading, or operation execution.
- Resource paths remain opaque.
- Stale lifecycle requests, events, channel messages, and completions have zero side effects.
- Run resources, cache entries, stream endpoints, spill files, retries, and spans are owner-scoped; no process-global mutable test/runtime state is introduced.
- No deprecated exports, optional identity overloads, wildcard exemptions, compatibility DTOs, or dual mutation paths are retained.
- Existing unrelated user changes and commit `f9fe4aa0` are preserved.
- No commit, staging, branch, worktree, tag, merge, or push is performed during implementation unless the user explicitly changes this constraint.

## 4. Canonical verification stabilization

The four integration binaries that failed with Windows `LNK1102` all link successfully with one Cargo build job. The canonical scripts pass Cargo's cross-platform `--jobs 1` option in `rust:test` and `rust:test:sci`.

The Rust test suite also owns process-global test hooks, compile counters, deadlines, and filesystem bindings. `.cargo/config.toml` therefore sets `RUST_TEST_THREADS=1` for Cargo-launched test harnesses. This does not limit `rust:check` or ordinary build parallelism, preserves filtered `pnpm rust:test` arguments, and makes canonical verification deterministic on Windows without a POSIX-only script assignment.

Verification proceeds from the four previously failing binaries to `pnpm verify:rust`, then full `pnpm verify`.

## 5. Rust-authoritative projection and legacy frontend removal

### 5.1 Function/editor projection

Rust function/editor projection is the only resolved source of function pins, data types, pin names, and resource references. TypeScript removes:

- `type_name` to `DataType` business mapping;
- unknown-type fallback to `Any`;
- parameter/return-value to Pin construction;
- fixed `Result` naming;
- recovery/load/publication reconstruction of resolved interfaces.

Wire parsers validate the authoritative projection before any store effect.

### 5.2 Legacy creation and identity types

Remove production use and exports of obsolete node creation/identity types and APIs, including the old batch creation path, `NodeInstanceParamsDTO`, display-name identity remnants, disabled `createNodes`, old clipboard payload members, and obsolete graph DTO fields.

Tests use current descriptor-backed creation contracts rather than preserving test-only compatibility imports.

### 5.3 Resource move publication

Resource move preparation no longer edits `NodeData.subGraphPath`. Rust returns revisioned graph projection replacements as part of the committed mutation result. The frontend may migrate only temporary UI state such as graph tabs, viewport, selection, and pending UI keys before installing the authoritative replacements.

## 6. `GraphDocument` mutation authority

Raw operations such as `create_node`, `delete_node`, `bind_port`, `connect`, `disconnect`, and `set_literal` become module-private or `#[cfg(test)]` helpers. Production callers use one path:

1. parse a descriptor or typed mutation request;
2. validate it against the current document and registry/resource facts;
3. build a prepared patch;
4. atomically commit the patch under the existing authority/lease rules;
5. publish the resulting revisioned replacement/delta.

A syntax-aware Rust architecture audit rejects production calls to raw helpers. Focused tests may retain explicit test builders without exposing production APIs.

## 7. Exact analysis dependencies and lowerability

### 7.1 Resource read tracking

Analysis resolvers record resource identity and revision only when a function, variable, or database is actually read:

```rust
struct AnalysisResourceReads {
    functions: BTreeMap<FunctionId, ResourceRevision>,
    variables: BTreeMap<VariableId, ResourceRevision>,
    databases: BTreeMap<DatabaseId, ResourceRevision>,
}
```

`CompilationBasis.resource_versions` is generated from this read set, not a project-wide snapshot. Mutating an unrelated resource does not invalidate the plan; mutating an actual dependency does.

### 7.2 Analysis-stage lowerability

All lowerability failures caused by user-authored graph/function/resource input are checked before producing `ValidatedSemanticGraph`. They become stable, localized Analysis diagnostics. Lowering receives a graph proven lowerable and may fail only for cancellation, deadline/resource exhaustion, or internal invariant errors.

Existing lowering diagnostics are classified: user-fixable cases move to Analysis; internal/runtime failure types remain in lowering.

## 8. Demand-driven publication

Plans carry explicit output demand:

```rust
struct ExecutionDemand {
    requested_outputs: BTreeSet<GraphOutputRef>,
    preview: Option<PinPreviewDemand>,
}
```

The compiler finalizes requested outputs and selected default graph results as `PlannedPublication::GraphResult`, or an independent Pin preview demand as `PlannedPublication::PinPreview`. `ExecutionPlan.publications` is the sole authority for scheduler result-source publication; intermediate operation outputs remain internal and are released when no longer required.

Pin preview remains an independent demand path with project/run identity checks and stale-settlement zero effects.

## 9. Cache policy and per-run memoization

Each plan operation carries an effective policy:

```rust
enum CachePolicy {
    Disabled,
    PerRun,
}
```

The compiler forces `Disabled` for effectful or non-deterministic operations. A per-run memoization key contains operation stable identity, canonical input fingerprints, relevant resource revisions, execution semantics version, and demand-sensitive configuration.

Semantics:

- concurrent requests for the same key share one producer;
- only complete successful results are cached;
- cancellation, timeout, retry exhaustion, errors, and partial streams are not cached;
- different inputs or relevant resource revisions never alias;
- entries and retained values are released by run finalization;
- the cache cannot outlive or cross a run/project lifecycle.

## 10. Materialization adapters and bounded streaming

### 10.1 Contract-based adapter insertion

Lowering compares producer `OutputProduction` with consumer `InputConsumption` and inserts explicit plan operations:

- `collect`: bounded stream to fully materialized value;
- `buffer`: bounded rate decoupling;
- `spill`: memory-budget overflow to a temporary file;
- `replay`: stable repeated consumption;
- `stream bridge`: compatible native stream conversion.

The scheduler does not infer adapters dynamically. Adapter selection is deterministic and testable from the contract matrix.

### 10.2 Runtime ownership

Streams use bounded channels. A full channel suspends the producer, providing real backpressure. Buffer sizes and in-memory materialization obey run resource budgets. Spill files and stream tasks are owned by run-scoped RAII guards and are cleaned on success, error, cancellation, timeout, panic boundaries, and replacement lifecycle drain.

## 11. Scheduler, deadline, and retry

The scheduler admits ready operations according to dependency readiness, workload class, and explicit CPU/I/O/resource budgets. Parallelism is bounded and scheduling prevents starvation between workload classes.

A single run deadline propagates through queue waiting, operation execution, channel waits, adapter I/O, result publication, and retry backoff. Cancellation has priority over retry and waiting.

Automatic retry is opt-in and requires all of:

- the operation is explicitly declared idempotent and retryable;
- the error is typed as transient/retryable;
- the maximum attempt count is not exhausted;
- the deadline allows the next bounded-backoff attempt.

Database writes, variable writes, filesystem writes, effects, and side-effecting calls are non-retryable by default and cannot be made retryable by frontend input. Operation identity remains stable across attempts; each attempt receives a distinct activation identity and child span.

## 12. Catalog search

The frontend search projection contains:

```ts
interface CatalogSearchDocument {
  nodeTypeId: string
  localizedTitle: string
  aliases: string[]
  technicalTerms: string[]
  backendSearchText: string[]
  resourceNames: string[]
  pinyinFull: string[]
  pinyinInitials: string[]
}
```

Every match maps to the stable `nodeTypeId`. Locale changes rebuild localized tokens without changing identity. Chinese title, alias, backend search text, and resource-name tokens receive deterministic offline full-pinyin and initials forms. The implementation must not use a network service and must have stable fixtures for polyphonic/unknown characters and mixed Chinese/Latin input.

## 13. Trace spans

Tracing uses paired hierarchical spans:

```rust
struct TraceSpan {
    span_id: SpanId,
    parent_span_id: Option<SpanId>,
    run_id: RunId,
    operation_id: Option<OperationId>,
    activation_id: Option<ActivationId>,
    kind: SpanKind,
    started_at: MonotonicTimestamp,
    finished_at: Option<MonotonicTimestamp>,
    outcome: Option<SpanOutcome>,
}
```

Coverage includes snapshot, analysis, lowering, run, operation attempt, resource acquisition, adapter I/O, result publication, and cleanup. Duration is computed from monotonic timestamps; wall-clock time is display metadata only. Success, error, cancellation, timeout, retry, and cleanup outcomes are explicit and wire-tested.

## 14. Boundary and compatibility cleanup

- Move protocol types, codecs, and validation from `node_system/parameter_types/` into an explicit `node_system/protocol/` submodule, update all imports, and remove the tenth top-level directory without compatibility re-exports.
- Merge `LocalizationLookup` and `LocalizationBundle` into one production trait/API and delete the blanket `Compatibility boundary` bridge.
- Remove `#[serde(default)]` legacy decoding for History persistence fields. Missing required fields fail strict decoding; the legacy acceptance test is replaced with a strict rejection test.

Architecture tests enforce the resulting one-way module dependencies and absence of the removed paths and symbols.

## 15. Test strategy

Every behavior change follows RED-GREEN:

1. add a focused regression or semantic architecture contract;
2. run it and confirm the intended failure;
3. implement the smallest complete authority-path change;
4. rerun focused tests;
5. run adjacent regression suites;
6. perform specification and code-quality review;
7. update the SDD ledger.

TypeScript service policy uses AST plus `TypeChecker`; Rust structure policy uses `syn` or Rust-native visibility tests; Rust-to-TypeScript protocols use generated golden fixtures and strict parsers.

Required coverage includes:

- authoritative projection with no frontend type fallback;
- no legacy node DTO/API symbols;
- no production raw document mutation access;
- revisioned move replacements and no frontend graph mutation;
- exact read-set freshness behavior;
- analysis-stage lowerability diagnostics;
- no publication for undemanded intermediate values;
- memoization hit, miss, concurrent deduplication, cancellation, error, and cleanup;
- adapter insertion matrix, bounded capacity, backpressure, spill/replay ordering, and cleanup;
- bounded scheduler concurrency, fairness, deadline propagation, idempotent retry, forbidden effect retry, and cancellation during backoff;
- all Catalog fields plus full-pinyin and initials matches;
- paired spans with stable hierarchy, identities, duration, and outcomes;
- absence of the extra parameter-types boundary and compatibility APIs;
- strict History decoding.

## 16. Final verification and acceptance

After all batches:

```text
pnpm typecheck
pnpm test
pnpm rust:fmt:check
pnpm rust:check
pnpm rust:test --lib -- --test-threads=1
pnpm verify
git diff --check
git diff --cached --name-only
git status --short
```

Acceptance requires:

- every unchecked functional item in `TODO.md:293-316` has implementation and regression evidence and is checked;
- all scoped specification and quality reviews report zero Critical and zero Important findings;
- canonical `pnpm verify` exits zero, with no `LNK1102` exception;
- stale lifecycle completion remains zero-effect;
- no generated build artifacts are added;
- the Git index remains empty and no repository-history operation is performed.

Final acceptance evidence (2026-08-09): a fresh controller rerun of the exact sequence passed `pnpm typecheck`, 250/250 frontend files with 1568/1568 tests, `pnpm rust:fmt:check`, `pnpm rust:check`, and the serial Rust library suite at 1340/1340. Canonical `pnpm verify` exited 0 with no `LNK1102`; its Rust matrices included 1375 passing `yssbi` tests and 43 passing `yss-sci` tests with one intentionally ignored diagnostic dump. `git diff --check` exited 0 and the Git index remained empty.
