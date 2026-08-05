# Relational Filter and Project Lineage Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver Catalog-creatable, UI-configurable Project DataFrame and structured Filter Rows nodes with strict Rust parameter authority, typed schema validation, one DataFrame-native relational island, safe metadata lineage, and production demand execution.

**Architecture:** Rust nominal codecs validate persisted JSON, typed schema facts drive editor projection and compile diagnostics, and frontend shadcn editors only render Rust-issued options and send atomic mutations. Validated parameters lower to existing structured relational operators; metadata-only hints are inferred and verified from the operator tree. Runtime converts tabular bridge input once, keeps DataFrames through Project/Filter/Rename/Limit, and converts only selected roots at result boundaries.

**Tech Stack:** Rust, serde, Tauri DTOs/services, React/TypeScript, shadcn/ui, schema analyzer, relational planner, Polars DataFrame runtime, ProjectState production tests.

## Global Constraints

- Work directly on `shadcn`; no worktree, branch, commit, or tag.
- Preserve unrelated dirty work.
- Add exactly `yssbi.dataframe.project` and `yssbi.dataframe.filter.rows`; preserve existing external-mask Filter and Decompose.
- Rust owns nominal parameter codecs, schema/type/operator choices, Catalog descriptors, projection DTOs, lowering, planning, and runtime.
- Frontend forwards exact descriptors and parameter mutations; views never call `invoke` and do not recreate compatibility rules.
- Persist Project columns as `string[]`; persist Filter predicate with exact `column`, `operator`, and tagged literal object.
- Integer/decimal values use canonical strings; no lossy JS number identity or blanket i64→f64 comparison.
- `ParameterizedStatic` permits editable missing parameters but missing/invalid values block compilation.
- Source→Filter→Project→Rename→Limit is one backend island with no intermediate materialization under final-only demand.
- Projection/predicate hints are metadata-only and semantically inert; backend validates exact inference before scan.
- Project/Filter/Rename/Limit remain DataFrame-native; RuntimeValue/IPC tabular transport is unchanged and does not claim dtype/order preservation.
- Preserve demand specialization, cancellation, resources, publication, History, structured control, Resource Catalog, and RunRegistry.
- RED-GREEN TDD; Rust commands serial with `CARGO_BUILD_JOBS=1` and `--test-threads=1`.
- Update `.superpowers/sdd/2026-08-03-relational-filter-project-lineage/progress.md` and `TODO.md` after every independent review. Keep Phase 7 at 95% until final review and fresh verification.
- Existing 18 `unused_must_use` test warnings and LF→CRLF notices are non-failing pre-existing warnings.

## File structure

- Create `src-tauri/src/node_system/parameter_types/mod.rs` and `parameter_types/dataframe.rs`: nominal codecs and typed structures that depend on protocol value/ID primitives; protocol has no reverse dependency, while document/compiler/Registry/Catalog consume protocol plus parameter_types one-way.
- Modify protocol type/schema files and `node_system/plan/model.rs`: nominal registrations, typed schema fields, Filter parameter dependency, Decimal literal, Predicate hint.
- Modify document mutation/compiler normalization/registry validation: invoke strict codecs at authority boundaries.
- Modify DataFrame Catalog and tests: stable protocols, ParameterizedStatic metadata, localization/docs.
- Modify analysis projection and command DTOs: schema-aware editor DTO variants.
- Modify frontend node-system DTOs/services/application editor and shadcn editor components: exact descriptor forwarding and atomic configuration.
- Modify compiler schema/lowering/relational planner/tests: typed diagnostics, exact fragments, deterministic metadata lineage and demand.
- Create `runtime/relational_dataframe.rs`; modify relational error/evaluator: ingress conversion, DataFrame-native operators, stable errors/checkpoints.
- Modify `project/project_state.rs` only for test-only relational backend/checkpoint injection.
- Modify `project/production_tests.rs`: final-only chain, preview, determinism, cancellation/resource proofs.

---

### Task 1: Freeze nominal parameters and typed schema authority

**Files:**
- Create: `src-tauri/src/node_system/parameter_types/mod.rs`
- Create: `src-tauri/src/node_system/parameter_types/dataframe.rs`
- Modify: `src-tauri/src/node_system/mod.rs`
- Modify: `src-tauri/src/node_system/protocol/types.rs`
- Modify: `src-tauri/src/node_system/plan/model.rs`
- Modify: `src-tauri/src/node_system/document/mutation.rs`
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs`
- Modify: `src-tauri/src/node_system/compiler/schema_analysis.rs`
- Modify: `src-tauri/src/node_system/analysis/model.rs` and semantic graph types owning AnalysisSnapshot fields
- Modify: `src-tauri/src/node_system/analysis/projection.rs`
- Modify: `src-tauri/src/node_system/registry/mod.rs` and `validation.rs`
- Modify: `src-tauri/src/project/project_state.rs` dtype resolver only
- Test: protocol/document/compiler/registry focused tests
- Update: `TODO.md`

**Produces:**

```rust
ProjectColumns(Box<[Box<str>]>);
FilterPredicate { column, operator, value };
FilterLiteral::{Boolean, Integer(i64), Decimal(CanonicalDecimal), String};
RelationalScalarType::{Boolean, Int64, Float64, String, Date, DateTime, Unknown};
SchemaField { name, scalar_type };
RelationalLiteral::Decimal(CanonicalDecimal);
SchemaExpr::Filter { input, predicate: ParameterKey };
```

- [ ] Write codec RED tests for exact persisted JSON, unknown fields/tags, string-encoded integer/decimal, empty/duplicate Project columns, and operator/value shape.
- [ ] Run exact tests and confirm failures are missing authority behavior.
- [ ] Implement strict shared codecs and nominal type IDs `yssbi.dataframe.project_columns` / `yssbi.dataframe.filter_predicate` in the lower-level parameter_types module. Add a generic NodeRegistry nominal validator registration/lookup path, duplicate/missing built-in validator errors, and proofs that unrelated custom types are unaffected.
- [ ] Add editor mutation and direct GraphDocument/compiler normalization RED tests proving malformed nominal JSON cannot pass as an unknown concrete type.
- [ ] Wire nominal validators into mutation/compiler boundaries without changing unrelated custom types.
- [ ] Add typed schema/dtype normalization RED tests for every specified database dtype alias and Unknown.
- [ ] Replace name-only schema facts with typed fields; make Project/Rename preserve them and Filter reference exact predicate parameter. Publish resolved typed fact maps in AnalysisSnapshot and ValidatedSemanticGraph keyed by stable port address, update serialization/fingerprint tests, and prove Project/Rename-transformed typed fields reach editor projection.
- [ ] Add parameter-/port-aware diagnostics and RED tests for missing schema, Project fields, Filter column/operator/literal/type, and Rename-aware names.
- [ ] Add Decimal relational IR serde/validation tests and exact comparison compatibility matrix; prohibit new Filter path from legacy blanket numeric conversion.
- [ ] Run protocol/document/schema/registry suites plus rust check/fmt/diff.
- [ ] After independent review, update ledger/TODO; Phase 7 stays 95%.

---

### Task 2: Add ParameterizedStatic Catalog and schema-aware frontend editors

**Files:**
- Modify: `src-tauri/src/node_system/catalog/dataframe/families.rs`
- Modify: `src-tauri/src/node_system/catalog/dataframe/mod.rs`
- Modify: `src-tauri/src/node_system/catalog/localization.rs`
- Modify: Catalog creation descriptor/DTO and document creation mutation files
- Modify: `src-tauri/src/node_system/analysis/projection.rs`
- Modify: command DTO serialization tests
- Modify: frontend node-system DTO/service files under `src/services/`
- Modify/Create: application parameter-edit workflow under `src/features/application/editor/`
- Create: focused shadcn Project columns and Filter predicate editor components in the existing parameter editor boundary
- Test: Rust Catalog/projection/mutation and frontend DTO/application/component tests
- Update: `TODO.md`

**Produces:**

```rust
NodeCreationDescriptor::ParameterizedStatic {
    node_type_id,
    required_parameters,
}
```

and Rust-issued `ProjectColumnsEditorDto` / `FilterPredicateEditorDto` with typed column/operator options.

- [ ] Write Catalog RED tests: both nodes visible/localized/documented/searchable, descriptor is ParameterizedStatic, exact required parameter keys, existing Filter/Decompose unchanged.
- [ ] Implement strict descriptor serde and creation of editable empty-parameter nodes; compilation remains blocked until configured. At mutation, derive the authoritative descriptor from frozen Registry/Catalog and require exact kind/required keys. Add omitted/extra/duplicate/cross-node/forged descriptor zero-effect tests.
- [ ] Write projection RED tests for no-schema unavailable state and typed schema options/operator matrix.
- [ ] Implement Rust projection DTOs from typed schema facts; do not duplicate compatibility in frontend.
- [ ] Write frontend DTO strict-wire and exact descriptor forwarding RED tests.
- [ ] Implement service/application forwarding with one atomic parameter mutation and pending-echo discipline.
- [ ] Write component RED tests for Project ordered multi-select, Filter column/operator/tagged literal, unavailable source, large integer/decimal strings, and validation errors.
- [ ] Implement shadcn editors with narrow store selectors and no direct invoke/global listener.
- [ ] Run focused Rust Catalog/projection and frontend tests plus `pnpm typecheck`, rust check/fmt/diff.
- [ ] After independent review, update ledger/TODO; Phase 7 stays 95%.

---

### Task 3: Lower exact fragments and derive deterministic metadata lineage

**Files:**
- Modify: `src-tauri/src/node_system/catalog/dataframe/mod.rs`
- Modify: `src-tauri/src/node_system/catalog/dataframe/tests.rs`
- Modify: `src-tauri/src/node_system/plan/model.rs`
- Modify: `src-tauri/src/node_system/plan/validation.rs`
- Modify: `src-tauri/src/node_system/compiler/relational.rs`
- Modify: `src-tauri/src/node_system/compiler/tests.rs`
- Test: plan/relational/compiler focused suites
- Update: `TODO.md`

**Produces:** ProjectLowerer, FilterRowsLowerer, `RelationalPushdownHint::Predicate`, exact metadata inference.

- [ ] Write exact lowerer RED tests for ordered Project columns and every Filter operator/literal including Decimal and IsNotNull→Not(IsNull).
- [ ] Implement defensive-codec lowerers with exact Input/root/result metadata and no schema lookup.
- [ ] Write full-chain planner RED tests varying fragment registration order directly; assert one island/source, zero bridge, continuous indices, exact roots.
- [ ] Implement only generic remapping fixes exposed by real fragments.
- [ ] Write metadata RED tests for Project(Filter(Source)), predicate dependency columns, Rename, multiple roots, bridge/Union boundaries, and semantic equivalence with hints removed.
- [ ] Add Predicate hint and deterministic exact-vector inference. Projection/Predicate remain metadata-only; existing Limit stays operational. Removing hints must preserve values but may change Limit scan counts.
- [ ] Add pure `ExecutionPlan::validate()` forged/stale hint tests here; defer source-scan observation to Task 4 runtime.
- [ ] Add demand RED tests for final-only root and stable Filter/Project intermediate GraphOutputRef prefixes/suffix pruning.
- [ ] Run lowerer/planner/plan/compiler/demand suites and rust check/fmt/diff.
- [ ] After independent review, update ledger/TODO; Phase 7 stays 95%.

---

### Task 4: Make relational operators DataFrame-native with explicit ingress

**Files:**
- Create: `src-tauri/src/node_system/runtime/relational_dataframe.rs`
- Modify: `src-tauri/src/node_system/runtime/mod.rs`
- Modify: `src-tauri/src/node_system/runtime/relational.rs`
- Modify: `src-tauri/src/node_system/runtime/production_relational.rs`
- Modify: run executor/RunError/terminal event-result mapping files that propagate relational codes
- Modify: `src-tauri/src/commands/command_node_system.rs` error-code mapping tests
- Modify: `src-tauri/src/project/project_state.rs` test-only backend/checkpoint injection
- Test: runtime helper/production relational/project checkpoint focused tests
- Update: `TODO.md`

**Produces:** stable-coded `RelationalError`, tabular ingress adapter, DataFrame Project/Filter/Rename helpers, test checkpoints/observer before result conversion.

- [ ] Write RelationalError code/serde/mapping RED tests with sanitized messages, then prove HintInvalid/TypeMismatch/Cancelled propagate through RunError, terminal event/result, ProjectState, and command mapping without string flattening.
- [ ] Implement stable error codes while preserving cancellation classification.
- [ ] Write ingress RED tests accepting only Scalar(Object(columns)) with equal-length lists and Artifact with exactly one such object; reject non-tabular scalar, empty/multi-value Artifact, Stream, unequal columns, and cover bridge-to-Project/Filter/Rename.
- [ ] Implement exactly-once ingress conversion at relational Input boundary.
- [ ] Write DataFrame Project/Filter/Rename RED tests covering order, dtype, nulls, native i64, checked Float64 decimal/integer, strings/booleans, missing/conflicts/type mismatch.
- [ ] Implement Polars-native helpers without whole-DataFrame Value conversion.
- [ ] Integrate evaluator DataFrame branches; only bridge/result roots convert to RuntimeValue.
- [ ] Add backend exact-hint validation before source scan and zero-scan forged-hint tests.
- [ ] Add predicate/result checkpoints and pre-conversion observer; add test-only ProjectState backend/checkpoint factory with bounded channels.
- [ ] Prove cancellation publishes no result and releases run/resources; prove only selected fragment roots materialize.
- [ ] Run runtime relational/cancellation/resource suites and rust check/fmt/diff.
- [ ] After independent review, update ledger/TODO; Phase 7 stays 95%.

---

### Task 5: Complete production chain, frontend/Rust verification, and Phase 7

**Files:**
- Modify: `src-tauri/src/project/production_tests.rs`
- Modify: focused Catalog/compiler/runtime tests for integration corrections
- Modify: frontend structured editor/application tests
- Modify: `TODO.md`
- Update: `.superpowers/sdd/2026-08-03-relational-filter-project-lineage/progress.md`

- [ ] Build a real built-in Registry/database fixture with Source→Filter Rows→Project→Rename→Limit and persisted exact parameters.
- [ ] Write final-only demand RED test using `include_default_results: false`; assert one island, no bridge/intermediate publication, exact final values, one lease/completion, no run leak.
- [ ] Use pre-conversion observer to assert internal column order, dtype, null counts, row order; external result assertions cover values only.
- [ ] Add stable Filter and Project preview demands proving exact prefixes and pruned suffix non-execution.
- [ ] Add production determinism with different UUID sort orders and normalized operator/result/resource/operation comparison.
- [ ] Add bounded predicate/materialization cancellation and defensive hint/type failure with zero result/completion and exact cleanup.
- [ ] Prove ParameterizedStatic UI configuration production route and existing external-mask Filter/Decompose/Source→Rename→Limit regressions.
- [ ] Run focused Rust protocol/Catalog/schema/lowering/planner/runtime/production/demand/cancellation/resource/History/structured-control/RunRegistry suites and focused frontend DTO/application/component tests; record counts.
- [ ] Run `pnpm typecheck`, `pnpm rust:check`, `pnpm rust:fmt:check`, and `git diff --check`.
- [ ] Run `pnpm verify` because the slice changes frontend and Rust contracts.
- [ ] Request independent final whole-slice review covering stable IDs/wire, Rust-owned editors, schema/type diagnostics, exact lowering, hints, ingress, DataFrame runtime, final/intermediate demand, cancellation/resources, and existing-node regressions.
- [ ] After clean review and fresh verification, update Phase 7 from 95% to 100%. Keep external mask alignment, typed tabular transport, operational database pushdown, Join/GroupBy/Sort/Window, multi-backend rewrite, runtime cache, scheduler parallelism, and deadlines as future work.
