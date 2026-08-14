# Canonical DataSeries Runtime Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace every legacy DataSeries protocol/runtime representation with canonical `core.data_series<T>` Artifacts, add project-authoritative numeric and missing-value settings, and make backend/frontend connection compatibility agree.

**Architecture:** Rust remains authoritative for canonical types, Artifact payload contracts, project computation settings, connection validation, and execution semantics. React consumes exact projections through services and uses a three-state compatibility preflight; all DataSeries kernels exchange one metadata-bearing Artifact representation, while `num_traits` is limited to pure numeric algorithms after runtime validation.

**Tech Stack:** Rust 2024, Tauri 2, serde, `num-traits 0.2.19`, Polars 0.53, React 19, TypeScript 5.8, Zustand, Vitest, shadcn/ui.

## Global Constraints

- Canonical production DataSeries type: `core.data_series<T>` only.
- Remove `tabular.series` and every `core.data_series.*` nominal type; do not add aliases or runtime compatibility shims.
- `Number` means `Int64 | Float64` and is a UI display name, not a nominal backend type.
- Numeric series inputs use an outer union or a numeric-constrained generic; do not use `DataSeries<Int64 | Float64>`.
- DataSeries runtime values use `RuntimeValue::Artifact` only; scalar List/Object inputs are internal contract errors.
- `ArtifactKind` continues to describe materialization strategy, not business payload type.
- Project defaults: absolute tolerance `1e-12`, relative tolerance `1e-9`, statistical missing-value policy `Listwise`.
- Tolerance applies only to Float64 equality/inequality, approximate-zero tests, and iterative convergence; ordered comparisons, hashes, grouping, joins, and cache identity remain exact.
- Statistical nodes may override convergence tolerance and missing-value policy; ordinary comparisons may not.
- Null remains distinct from NaN and infinity; statistical Listwise deletion reports removed Null and NaN rows separately, while infinity is rejected.
- Computation settings are Rust-authoritative project data persisted in `metadata.yssbi`; do not add them to localStorage-backed `ProjectSettings`.
- Computation-setting mutations are atomic and advance project authority; they are not added to graph undo/redo history in this implementation.
- Run project commands from the repository root through `pnpm`; use `pnpm rust:test --lib <filter>` for focused Rust unit tests.
- Do not run full `pnpm rust:test` by default; use focused Rust suites, `pnpm rust:check`, and `pnpm verify` at final delivery.
- Preserve unrelated user changes. Do not create commits unless the user explicitly authorizes commits during execution.

---

## File Structure Map

### Canonical types and compatibility

- Modify `src-tauri/src/node_system/protocol/types.rs`: canonical union construction and normalization.
- Create `src-tauri/src/node_system/protocol/data_series.rs`: canonical DataSeries constructors and constants.
- Modify `src-tauri/src/node_system/protocol/mod.rs`: export canonical DataSeries APIs.
- Modify `src-tauri/src/node_system/compiler/type_analysis.rs`: three-state conformance and Unknown propagation.
- Modify `src-tauri/src/node_system/compatibility.rs`: consume three-state conformance for catalog filtering.
- Modify `src-tauri/src/node_system/analysis/projection.rs`: remove legacy mappings and stop projecting Unknown as Any.
- Modify `src-tauri/src/node_system/document/mutation.rs`: reject proven incompatibility and allow indeterminate submissions for backend analysis.
- Modify `src/shared/types/domain/dataType.ts`: deterministic unions and Number display.
- Modify `src/shared/utils/pinCompatibility.ts`: frontend three-state compatibility.
- Modify `src/features/core/canvas/useCanvasInteraction.ts`: block only proven incompatibility.

### Artifact runtime and plan contracts

- Create `src-tauri/src/node_system/runtime/data_series.rs`: payload metadata, typed readers, builders, Null policies, checked numeric conversion.
- Modify `src-tauri/src/node_system/runtime/run.rs`: add payload metadata to Artifact storage without changing `ArtifactKind`.
- Modify `src-tauri/src/node_system/runtime/materialization.rs`: preserve DataSeries metadata through collect/spill/replay.
- Modify `src-tauri/src/node_system/runtime/artifact.rs`: publish/page DataSeries descriptors.
- Modify `src-tauri/src/node_system/runtime/mod.rs`: export DataSeries runtime APIs.
- Modify `src-tauri/src/node_system/plan/model.rs`: add planned value representation contracts.
- Modify `src-tauri/src/node_system/plan/validation.rs`: validate value-kind continuity.
- Modify `src-tauri/src/node_system/compiler/pipeline.rs`: lower resolved TypeExpr into planned value contracts.
- Modify `src-tauri/src/node_system/compiler/specialization/finalization.rs`: preserve contracts through specialization/adapters.
- Modify `src-tauri/src/node_system/runtime/function_plan.rs`: validate DataSeries function ABI contracts.

### Project computation settings

- Create `src-tauri/src/project/computation_settings.rs`: validated tolerance and missing-value domain types.
- Modify `src-tauri/src/project/project_data.rs`: authoritative settings and revision.
- Modify `src-tauri/src/project/project_io.rs`: manifest persistence/defaults.
- Modify `src-tauri/src/project/project_state.rs`: atomic settings mutation and compile-resource authority.
- Create `src-tauri/src/commands/command_project/settings.rs`: thin Tauri query/update commands and DTOs.
- Modify `src-tauri/src/commands/command_project/mod.rs` and `src-tauri/src/lib.rs`: command exports/registration.
- Modify `src-tauri/src/event/event_project.rs`: committed computation-settings event.
- Create `src/shared/types/dto/projectComputationSettings.ts`: strict frontend DTOs/parsers.
- Modify `src/services/project/projectService.ts`: settings query/update invokes.
- Create `src/features/application/projectSettings/useProjectComputationSettings.ts`: project-scoped loading/save workflow.
- Modify `src/views/EditorView/Layout/SettingsView.tsx`: Computation section UI.

### Kernel migrations

- Modify `src-tauri/src/node_system/catalog/dataframe/mod.rs` and runtime counterpart `runtime/kernels/dataframe/mod.rs`.
- Modify `src-tauri/src/node_system/catalog/distribution/mod.rs` and runtime counterpart `runtime/kernels/distribution/mod.rs`.
- Modify `src-tauri/src/node_system/catalog/statistics/mod.rs` and runtime counterpart `runtime/kernels/statistics/mod.rs`.
- Modify `src-tauri/src/node_system/catalog/plot/mod.rs` and runtime counterpart `runtime/kernels/plot/mod.rs`.
- Modify `src-tauri/src/node_system/catalog/core_nodes/math.rs`, `value.rs`, and runtime counterparts.
- Modify project variable/function/resource projection and result-source files only where they exchange DataSeries values.

---

### Task 1: Canonical DataSeries Type Constructors and Union Normalization

**Files:**
- Create: `src-tauri/src/node_system/protocol/data_series.rs`
- Modify: `src-tauri/src/node_system/protocol/mod.rs`
- Modify: `src-tauri/src/node_system/protocol/types.rs`
- Modify: `src-tauri/src/node_system/catalog/builtin.rs`
- Test: `src-tauri/src/node_system/protocol/tests.rs`
- Test: `src-tauri/src/node_system/catalog/statistics/tests.rs`

**Interfaces:**
- Consumes: existing `TypeExpr`, `TypeId`, `TypeConstructorId`, and registered `core.data_series` constructor.
- Produces:

```rust
pub const DATA_SERIES_CONSTRUCTOR_ID: &str = "core.data_series";
pub const NUMERIC_TYPE_CLASS_ID: &str = "core.numeric";
pub fn data_series_type(element: TypeExpr) -> TypeExpr;
pub fn numeric_data_series_type() -> TypeExpr;
pub fn normalize_type_expr(value: TypeExpr) -> Result<TypeExpr, TypeNormalizationError>;
```

- `numeric_data_series_type()` returns an outer union of Int64 and Float64 series.
- `src-tauri/src/node_system/catalog/builtin.rs` assigns `TypeClassId("core.numeric")` to exactly the `core.int64` and `core.float64` `TypeRegistration.classes` sets; String and Boolean remain outside the class.

- [ ] **Step 1: Write failing canonicalization tests**

Add tests that assert exact canonical shapes:

```rust
#[test]
fn numeric_data_series_is_an_outer_union_of_homogeneous_series() {
    assert_eq!(
        numeric_data_series_type(),
        TypeExpr::Union(vec![
            data_series_type(TypeExpr::Concrete(TypeId::new("core.float64").unwrap())),
            data_series_type(TypeExpr::Concrete(TypeId::new("core.int64").unwrap())),
        ])
    );
}

#[test]
fn type_union_is_flattened_deduplicated_and_deterministically_sorted() {
    let int = TypeExpr::Concrete(TypeId::new("core.int64").unwrap());
    let float = TypeExpr::Concrete(TypeId::new("core.float64").unwrap());
    let normalized = normalize_type_expr(TypeExpr::Union(vec![
        int.clone(),
        TypeExpr::Union(vec![float.clone(), int.clone()]),
    ]))
    .unwrap();
    assert_eq!(normalized, TypeExpr::Union(vec![float, int]));
}

#[test]
fn empty_type_union_is_rejected() {
    assert!(matches!(
        normalize_type_expr(TypeExpr::Union(Vec::new())),
        Err(TypeNormalizationError::EmptyUnion)
    ));
}

#[test]
fn numeric_type_class_contains_only_int64_and_float64() {
    let registry = production_type_registry();
    assert_eq!(
        registry.type_class_members("core.numeric").unwrap(),
        ["core.float64", "core.int64"],
    );
}
```

Define `production_type_registry` in the protocol test module using the existing production registry constructor; do not create a test-only registry with different registrations.

Update the OLS Summary protocol test to expect `numeric_data_series_type()` rather than an inner union.

- [ ] **Step 2: Run focused tests to verify RED**

Run:

```sh
pnpm rust:test --lib numeric_data_series_is_an_outer_union_of_homogeneous_series
```

Expected: compile failure because the canonical DataSeries APIs do not exist.

- [ ] **Step 3: Implement canonical constructors and normalization**

Create `data_series.rs` with exact constructor IDs and outer-union construction. Add a stable `TypeExpr` sort key based on variant and canonical identifier, flatten nested unions recursively, deduplicate equal members, collapse one-member unions, and reject empty unions.

The central implementation must preserve Unknown and Generic values rather than converting them to Any:

```rust
pub fn data_series_type(element: TypeExpr) -> TypeExpr {
    TypeExpr::Applied {
        constructor: TypeConstructorId::new(DATA_SERIES_CONSTRUCTOR_ID)
            .expect("canonical DataSeries constructor ID"),
        arguments: vec![element],
    }
}

pub fn numeric_data_series_type() -> TypeExpr {
    normalize_type_expr(TypeExpr::Union(vec![
        data_series_type(TypeExpr::Concrete(TypeId::new("core.int64").unwrap())),
        data_series_type(TypeExpr::Concrete(TypeId::new("core.float64").unwrap())),
    ]))
    .expect("canonical numeric DataSeries union")
}
```

- [ ] **Step 4: Run focused tests to verify GREEN**

Run each separately:

```sh
pnpm rust:test --lib numeric_data_series_is_an_outer_union_of_homogeneous_series
pnpm rust:test --lib type_union_is_flattened_deduplicated_and_deterministically_sorted
pnpm rust:test --lib empty_type_union_is_rejected
pnpm rust:test --lib numeric_type_class_contains_only_int64_and_float64
```

Expected: each reports `1 passed; 0 failed`.

- [ ] **Step 5: Run protocol validation**

Run:

```sh
pnpm rust:test --lib catalog::statistics::tests::statistics_protocols_have_unique_ports_and_valid_bindings
pnpm rust:check
```

Expected: both exit 0.

- [ ] **Step 6: Commit checkpoint only if commits were explicitly authorized**

```sh
git add src-tauri/src/node_system/protocol src-tauri/src/node_system/catalog/statistics/tests.rs
git commit -m "Add canonical DataSeries types"
```

---

### Task 2: Three-State Backend Type Conformance

**Files:**
- Modify: `src-tauri/src/node_system/compiler/type_analysis.rs`
- Modify: `src-tauri/src/node_system/compiler/mod.rs`
- Modify: `src-tauri/src/node_system/compatibility.rs`
- Modify: `src-tauri/src/node_system/document/mutation.rs`
- Test: `src-tauri/src/node_system/compiler/tests.rs`
- Test: `src-tauri/src/node_system/document/tests/editor_mutation_validation.rs`

**Interfaces:**
- Consumes: Task 1 normalized `TypeExpr` values.
- Produces:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeCompatibility {
    Compatible,
    Incompatible,
    Indeterminate,
}

pub fn type_exprs_compatibility(
    source: &TypeExpr,
    target: &TypeExpr,
    source_type_parameters: &[TypeParameterId],
    target_type_parameters: &[TypeParameterId],
) -> TypeCompatibility;
```

- Compiler analysis still emits blocking diagnostics only for proven incompatibility. Indeterminate constraints retain unresolved facts without claiming success.

- [ ] **Step 1: Write failing conformance tests**

```rust
#[test]
fn type_conformance_requires_every_source_union_member() {
    let source = TypeExpr::Union(vec![concrete("core.int64"), concrete("core.string")]);
    assert_eq!(
        type_exprs_compatibility(&source, &concrete("core.int64"), &[], &[]),
        TypeCompatibility::Incompatible
    );
}

#[test]
fn type_conformance_accepts_numeric_series_members() {
    let target = numeric_data_series_type();
    assert_eq!(
        type_exprs_compatibility(
            &data_series_type(concrete("core.int64")),
            &target,
            &[],
            &[],
        ),
        TypeCompatibility::Compatible
    );
}

#[test]
fn type_conformance_reports_unknown_as_indeterminate() {
    assert_eq!(
        type_exprs_compatibility(
            &TypeExpr::Unknown,
            &data_series_type(concrete("core.float64")),
            &[],
            &[],
        ),
        TypeCompatibility::Indeterminate
    );
}
```

Add mutation tests asserting a proven String-series-to-numeric-series connection is rejected before publication while `DataSeries<Unknown>` can be submitted for backend analysis.

- [ ] **Step 2: Run RED tests separately**

```sh
pnpm rust:test --lib type_conformance_reports_unknown_as_indeterminate
pnpm rust:test --lib editor_mutation_rejects_proven_incompatible_connection
```

Expected: compile failure for missing `TypeCompatibility` or behavior failure because Unknown currently succeeds.

- [ ] **Step 3: Implement three-state solving**

Change internal assignability outcomes to distinguish mismatch from insufficient information. Keep existing source-union `all` and target-union trial semantics. Make Unknown and unresolved generics return Indeterminate. Use separate recursive combinators:

```text
source union (every member required):
  any Incompatible -> Incompatible
  else any Indeterminate -> Indeterminate
  else Compatible

target union (one member sufficient):
  any Compatible -> Compatible
  else any Indeterminate -> Indeterminate
  else Incompatible

Applied(core.data_series, [source_element]) -> Applied(core.data_series, [target_element]):
  recurse covariantly on the one element argument
```

Update catalog filtering so:

- Compatible candidates are included;
- Incompatible candidates are excluded;
- Indeterminate candidates remain available but are marked unresolved at the call site rather than claimed compatible.

Update connection mutation validation to reject only `Incompatible` before applying the patch.

- [ ] **Step 4: Run GREEN tests separately**

```sh
pnpm rust:test --lib type_conformance_requires_every_source_union_member
pnpm rust:test --lib type_conformance_accepts_numeric_series_members
pnpm rust:test --lib type_conformance_reports_unknown_as_indeterminate
pnpm rust:test --lib editor_mutation_rejects_proven_incompatible_connection
pnpm rust:test --lib editor_mutation_allows_indeterminate_connection_for_backend_analysis
```

Expected: each passes independently.

- [ ] **Step 5: Run broader type-analysis coverage**

```sh
pnpm rust:test --lib compatibility_uses_exact_type_expr_ids_and_compiler_source_union_semantics
pnpm rust:test --lib compiler::tests::implements_and_element_of_solve_registered_type_shapes
pnpm rust:check
```

Replace legacy-ID assertions in the compatibility test with canonical Applied-type assertions.

- [ ] **Step 6: Commit checkpoint only if authorized**

```sh
git add src-tauri/src/node_system/compiler src-tauri/src/node_system/compatibility.rs src-tauri/src/node_system/document
git commit -m "Add three-state type conformance"
```

---

### Task 3: Exact Projection and Frontend Three-State Preflight

**Files:**
- Modify: `src-tauri/src/node_system/analysis/projection.rs`
- Modify: `src/shared/types/domain/dataType.ts`
- Modify: `src/shared/utils/pinCompatibility.ts`
- Modify: `src/features/core/canvas/useCanvasInteraction.ts`
- Modify: `src/features/core/dataStore/graphDataStore.ts`
- Test: `src/shared/utils/pinCompatibility.test.ts`
- Test: `src/features/core/dataStore/graphDataStore.test.ts`
- Test: `src/shared/types/dto/editorMutationWireParser.test.ts`

**Interfaces:**
- Consumes: Task 2 `TypeCompatibility` semantics and existing `TypeSummaryDto` fields.
- Produces:

```ts
export type TypeCompatibility =
  | 'compatible'
  | 'incompatible'
  | 'indeterminate';

export function getDataTypeCompatibility(
  source: DataType | null | undefined,
  target: DataType | null | undefined,
  typeSystem?: TypeSystemSnapshot,
): TypeCompatibility;

export function getPinCompatibility(
  source: ConnectionCandidatePin,
  target: ConnectionCandidatePin,
  typeSystem?: TypeSystemSnapshot,
): TypeCompatibility;
```

- [ ] **Step 1: Write failing Rust projection tests**

```rust
#[test]
fn projection_projects_unknown_without_any() {
    let summary = project_type_summary(&TypeExpr::Unknown);
    assert!(!summary.resolved);
    assert_eq!(summary.data_type, None);
}

#[test]
fn projection_projects_numeric_series_union_without_legacy_ids() {
    let summary = project_type_summary(&numeric_data_series_type());
    assert!(summary.resolved);
    assert_eq!(summary.display.as_ref(), "core.data_series<core.float64> | core.data_series<core.int64>");
}
```

- [ ] **Step 2: Write failing frontend compatibility tests**

```ts
it('requires every source union member to be assignable', () => {
  expect(getDataTypeCompatibility(
    { kind: 'OneOf', inner: [{ kind: 'Int64' }, { kind: 'String' }] },
    { kind: 'Int64' },
  )).toBe('incompatible');
});

it('returns indeterminate when either projected type is missing', () => {
  expect(getDataTypeCompatibility(null, { kind: 'Float64' }))
    .toBe('indeterminate');
});

it('accepts homogeneous numeric series into DataSeries Number union', () => {
  const target = { kind: 'OneOf', inner: [
    { kind: 'DataSeries', inner: { kind: 'Int64' } },
    { kind: 'DataSeries', inner: { kind: 'Float64' } },
  ] } satisfies DataType;
  expect(getDataTypeCompatibility(
    { kind: 'DataSeries', inner: { kind: 'Int64' } },
    target,
  )).toBe('compatible');
});
```

- [ ] **Step 3: Run RED tests**

```sh
pnpm rust:test --lib projection_projects_unknown_without_any
pnpm test -- src/shared/utils/pinCompatibility.test.ts
```

Expected: Rust behavior assertion fails and TypeScript compile/tests fail because three-state APIs do not exist.

- [ ] **Step 4: Implement exact projection and frontend conformance**

Remove every legacy DataSeries projection branch. Map only `Applied(core.data_series, [T])`. Add exact mappings for `core.time`, `core.object`, and `core.array<T>` if the constructor is registered during Task 1; otherwise return unresolved rather than fail-open.

In TypeScript:

- source `OneOf` uses `every`;
- target `OneOf` uses `some`;
- missing types return `indeterminate`;
- unresolved pins are never labeled compatible;
- Number display is recognized only for exact Int64/Float64 unions and corresponding DataSeries outer unions;
- canvas blocks only `'incompatible'`, while `'indeterminate'` proceeds to backend validation.

- [ ] **Step 5: Run GREEN frontend tests**

```sh
pnpm test -- src/shared/utils/pinCompatibility.test.ts
pnpm test -- src/features/core/dataStore/graphDataStore.test.ts
pnpm test -- src/shared/types/dto/editorMutationWireParser.test.ts
pnpm typecheck
```

Expected: all pass.

- [ ] **Step 6: Run GREEN Rust projection tests**

```sh
pnpm rust:test --lib projection_projects_unknown_without_any
pnpm rust:test --lib projection_projects_numeric_series_union_without_legacy_ids
pnpm rust:check
```

- [ ] **Step 7: Commit checkpoint only if authorized**

```sh
git add src-tauri/src/node_system/analysis src/shared/types/domain src/shared/utils src/features/core
git commit -m "Align projected type compatibility"
```

---

### Task 4: DataSeries Artifact Payload and Typed Readers

**Files:**
- Create: `src-tauri/src/node_system/runtime/data_series.rs`
- Modify: `src-tauri/src/node_system/runtime/run.rs`
- Modify: `src-tauri/src/node_system/runtime/materialization.rs`
- Modify: `src-tauri/src/node_system/runtime/artifact.rs`
- Modify: `src-tauri/src/node_system/runtime/mod.rs`
- Test: `src-tauri/src/node_system/runtime/tests.rs`
- Test: `src-tauri/src/node_system/runtime/artifact.rs`

**Interfaces:**
- Consumes: existing Artifact storage, cursor, spill, replay, and `protocol::Value`.
- Produces:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactValueKind { Sequence, DataSeries }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataSeriesElementType {
    Int64, Float64, String, Boolean, Date, Datetime, Categorical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DataSeriesMetadata {
    pub element_type: DataSeriesElementType,
    pub length: usize,
    pub null_count: usize,
    pub name: Option<Box<str>>,
    pub format: Option<Box<str>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NullPolicy { Propagate, Skip, Reject }

pub enum NumericSeriesView<'a> {
    Int64(Int64SeriesView<'a>),
    Float64(Float64SeriesView<'a>),
}

pub fn require_data_series(value: &RuntimeValue) -> Result<&Artifact, KernelError>;
pub fn numeric_series(artifact: &Artifact, policy: NullPolicy) -> Result<NumericSeriesView<'_>, KernelError>;
pub fn string_series(artifact: &Artifact, policy: NullPolicy) -> Result<StringSeriesView<'_>, KernelError>;
pub fn boolean_series(artifact: &Artifact, policy: NullPolicy) -> Result<BooleanSeriesView<'_>, KernelError>;
```

- [ ] **Step 1: Write failing Artifact contract tests**

```rust
#[test]
fn data_series_artifact_preserves_metadata_through_spill_and_replay() {
    let metadata = DataSeriesMetadata {
        element_type: DataSeriesElementType::Float64,
        length: 3,
        null_count: 1,
        name: Some("x".into()),
        format: None,
    };
    let artifact = materialize_test_data_series(
        metadata.clone(),
        [decimal("1"), Value::Null, decimal("3")],
        1,
    );
    assert_eq!(artifact.value_kind(), ArtifactValueKind::DataSeries);
    assert_eq!(artifact.data_series_metadata(), Some(&metadata));
    assert_eq!(artifact.cursor().unwrap().collect::<Result<Vec<_>, _>>().unwrap().len(), 3);
}

#[test]
fn scalar_list_is_rejected_as_data_series() {
    let value = RuntimeValue::Scalar(Value::List(vec![Value::Integer(1)]));
    assert_eq!(
        require_data_series(&value).unwrap_err().message(),
        "expected DataSeries Artifact, received scalar"
    );
}
```

- [ ] **Step 2: Run RED tests separately**

```sh
pnpm rust:test --lib data_series_artifact_preserves_metadata_through_spill_and_replay
pnpm rust:test --lib scalar_list_is_rejected_as_data_series
```

Expected: compile failure for missing DataSeries Artifact APIs.

- [ ] **Step 3: Implement payload metadata without changing ArtifactKind**

Extend Artifact storage with `ArtifactValueKind` and optional `DataSeriesMetadata`. Add constructors:

```rust
pub fn new_data_series(
    kind: ArtifactKind,
    metadata: DataSeriesMetadata,
    values: impl Into<Box<[Value]>>,
) -> Result<Self, DataSeriesContractError>;
```

Validate metadata length/null count and per-element type. Preserve metadata when materialization adapters produce collected, spilled, buffered, or replayable artifacts. `ArtifactKind` remains unchanged.

- [ ] **Step 4: Implement typed readers and builders**

Readers verify `ArtifactValueKind::DataSeries`, metadata element type, cursor values, and NullPolicy. Numeric views preserve Int64/Float64 until explicit promotion. Add checked Int64-to-f64 conversion rejecting values outside `-(2^53)..=2^53` where exact comparison is required.

- [ ] **Step 5: Run GREEN tests and existing materialization tests**

```sh
pnpm rust:test --lib data_series_artifact_preserves_metadata_through_spill_and_replay
pnpm rust:test --lib scalar_list_is_rejected_as_data_series
pnpm rust:test --lib materialization_matrix_executes_all_fifteen_cells_with_declared_io_contracts
pnpm rust:test --lib spill_replay_supports_two_independent_passes
pnpm rust:check
```

Expected: all pass.

- [ ] **Step 6: Commit checkpoint only if authorized**

```sh
git add src-tauri/src/node_system/runtime
git commit -m "Add typed DataSeries artifacts"
```

---

### Task 5: Planned Value Contracts and Function ABI Preservation

**Files:**
- Modify: `src-tauri/src/node_system/plan/model.rs`
- Modify: `src-tauri/src/node_system/plan/validation.rs`
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs`
- Modify: `src-tauri/src/node_system/compiler/specialization/mod.rs`
- Modify: `src-tauri/src/node_system/compiler/specialization/finalization.rs`
- Modify: `src-tauri/src/node_system/runtime/function_plan.rs`
- Test: `src-tauri/src/node_system/compiler/tests.rs`
- Test: `src-tauri/src/node_system/plan/mod.rs`

**Interfaces:**
- Consumes: Task 1 canonical TypeExpr and Task 4 Artifact payload kind.
- Produces:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlannedValueKind { Scalar, DataSeries, DataFrame, Opaque }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedValueContract {
    pub kind: PlannedValueKind,
    pub type_expr: TypeExpr,
}
```

- `PlannedInput`, `PlannedOutput`, external/control value sources, and function ABI bindings carry contracts.

- [ ] **Step 1: Write failing plan tests**

```rust
#[test]
fn data_series_contract_survives_materialization_adapter_insertion() {
    let plan = compile_connected_series_graph(OutputProduction::Streaming, InputConsumption::FullyMaterialized);
    let adapter = plan.operations.iter().find(|operation| matches!(operation.kernel, PlannedKernel::Adapter(_))).unwrap();
    assert!(adapter.inputs.iter().all(|input| input.contract.kind == PlannedValueKind::DataSeries));
    assert!(adapter.outputs.iter().all(|output| output.contract.kind == PlannedValueKind::DataSeries));
}

#[test]
fn function_plan_store_rejects_data_series_kind_mismatch() {
    let (mut plan, abi) = structured_data_series_function_plan();
    plan.operations[0].outputs[0].contract.kind = PlannedValueKind::Scalar;
    assert!(matches!(
        publish_function(plan, abi),
        Err(FunctionPlanStoreError::AbiValueContractMismatch { .. })
    ));
}
```

- [ ] **Step 2: Run RED tests**

```sh
pnpm rust:test --lib data_series_contract_survives_materialization_adapter_insertion
pnpm rust:test --lib function_plan_store_rejects_data_series_kind_mismatch
```

Expected: compile failure for missing contract fields/types.

- [ ] **Step 3: Lower contracts from resolved port TypeExpr**

Add `planned_value_contract(type_expr)` in compiler lowering:

```text
Applied(core.data_series, [_]) -> DataSeries
Concrete(tabular.dataframe) -> DataFrame
registered scalar IDs -> Scalar
registered model/report/config nominal IDs -> Opaque
Unknown/Generic required at lowering -> compiler diagnostic
```

Thread contracts through operations, value sources, adapters, control bindings, function calls, and ABI. Adapter insertion copies contracts unchanged.

- [ ] **Step 4: Validate contract continuity**

Plan validation rejects dependency, adapter, publication, or function binding edges whose source and destination `PlannedValueKind` differ. Runtime operation admission verifies a DataSeries contract receives a DataSeries Artifact before executing a kernel.

- [ ] **Step 5: Run GREEN tests and broader plan checks**

```sh
pnpm rust:test --lib data_series_contract_survives_materialization_adapter_insertion
pnpm rust:test --lib function_plan_store_rejects_data_series_kind_mismatch
pnpm rust:test --lib materialization_adapter_matrix_covers_every_contract_pair
pnpm rust:test --lib call_binds_exact_function_locators_across_different_value_layouts
pnpm rust:check
```

- [ ] **Step 6: Commit checkpoint only if authorized**

```sh
git add src-tauri/src/node_system/plan src-tauri/src/node_system/compiler src-tauri/src/node_system/runtime/function_plan.rs
git commit -m "Track planned value contracts"
```

---

### Task 6: Project Computation Settings Domain and Manifest Persistence

**Files:**
- Create: `src-tauri/src/project/computation_settings.rs`
- Modify: `src-tauri/src/project/mod.rs`
- Modify: `src-tauri/src/project/project_data.rs`
- Modify: `src-tauri/src/project/project_io.rs`
- Test: `src-tauri/src/project/project_io.rs`

**Interfaces:**
- Produces:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumericTolerance { pub absolute: f64, pub relative: f64 }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StatisticalMissingValuePolicy { Listwise, Reject }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NumericSettings {
    pub tolerance: NumericTolerance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingValueSettings {
    pub statistics: StatisticalMissingValuePolicy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectComputationSettings {
    pub numeric: NumericSettings,
    pub missing_values: MissingValueSettings,
}
```

- Defaults: `1e-12`, `1e-9`, `Listwise`.

- [ ] **Step 1: Write failing domain and manifest tests**

```rust
#[test]
fn legacy_manifest_defaults_computation_settings() {
    let manifest = serde_json::from_value::<ProjectManifest>(json!({
        "schemaVersion": 3,
        "projectName": "Legacy",
        "exportTime": "2026-08-12T00:00:00Z"
    })).unwrap();
    assert_eq!(manifest.computation_settings, ProjectComputationSettings::default());
}

#[test]
fn computation_settings_reject_non_finite_or_zero_pair() {
    assert!(NumericTolerance { absolute: f64::NAN, relative: 1e-9 }.validate().is_err());
    assert!(NumericTolerance { absolute: 0.0, relative: 0.0 }.validate().is_err());
}

#[test]
fn project_manifest_round_trips_computation_settings() {
    let mut data = ProjectData::new();
    data.computation_settings.numeric.tolerance.absolute = 1e-10;
    let manifest = serialize_project_manifest(&data).unwrap();
    let decoded: ProjectManifest = serde_json::from_slice(&manifest).unwrap();
    assert_eq!(decoded.computation_settings, data.computation_settings);
}
```

- [ ] **Step 2: Run RED tests**

```sh
pnpm rust:test --lib legacy_manifest_defaults_computation_settings
pnpm rust:test --lib computation_settings_reject_non_finite_or_zero_pair
```

Expected: compile failure because settings types/fields are missing.

- [ ] **Step 3: Implement validated settings and serde defaults**

Add `ProjectData.computation_settings` with `#[serde(default)]`. Extend `ProjectManifest`, serialization, project creation, save, load, and missing-manifest defaults. Do not add the settings to frontend localStorage DTOs.

- [ ] **Step 4: Run GREEN tests and existing manifest checks**

```sh
pnpm rust:test --lib legacy_manifest_defaults_computation_settings
pnpm rust:test --lib computation_settings_reject_non_finite_or_zero_pair
pnpm rust:test --lib project_manifest_round_trips_computation_settings
pnpm rust:test --lib project_manifest_omits_application_version
pnpm rust:check
```

- [ ] **Step 5: Commit checkpoint only if authorized**

```sh
git add src-tauri/src/project/computation_settings.rs src-tauri/src/project/project_data.rs src-tauri/src/project/project_io.rs src-tauri/src/project/mod.rs
git commit -m "Persist project computation settings"
```

---

### Task 7: Atomic Computation Settings Mutation, IPC, Events, and Settings UI

**Files:**
- Modify: `src-tauri/src/project/project_state.rs`
- Create: `src-tauri/src/commands/command_project/settings.rs`
- Modify: `src-tauri/src/commands/command_project/mod.rs`
- Modify: `src-tauri/src/event/event_project.rs`
- Modify: `src-tauri/src/lib.rs`
- Create: `src/shared/types/dto/projectComputationSettings.ts`
- Modify: `src/services/project/projectService.ts`
- Create: `src/features/application/projectSettings/useProjectComputationSettings.ts`
- Modify: `src/views/EditorView/Layout/SettingsView.tsx`
- Test: `src-tauri/src/project/production_tests.rs`
- Test: `src/services/project/projectService.test.ts`
- Create: `src/features/application/projectSettings/useProjectComputationSettings.test.ts`
- Create: `src/views/EditorView/Layout/SettingsView.test.tsx`

**Interfaces:**
- Consumes: Task 6 settings types.
- Produces commands `get_project_computation_settings` and `update_project_computation_settings`.
- Produces strict DTOs with project instance ID, operation ID, expected revision, settings revision, publication revision, and confirmed settings.

- [ ] **Step 1: Write failing Rust atomicity tests**

Add tests named `computation_settings_mutation_commits_disk_and_authority_atomically`, `stale_computation_settings_revision_changes_nothing`, `computation_settings_disk_failure_preserves_memory_and_manifest`, `computation_settings_publication_failure_rolls_back_manifest_and_memory`, and `computation_settings_commit_emits_exactly_one_event`. The success assertion must reload `metadata.yssbi` and compare it to `ProjectState.project_data`; both failure tests must compare pre/post manifest bytes, in-memory settings, settings revision, publication revision, and authority generation.

```rust
#[test]
fn computation_settings_mutation_commits_disk_and_authority_atomically() {
    let project = temp_project_with_empty_graph("computation-settings");
    let request = settings_request(&project, 0, NumericTolerance { absolute: 1e-8, relative: 1e-6 });
    let receipt = project.state().update_computation_settings_transaction(request).unwrap();
    let reloaded = load_project_from_file(project.root_string().as_str()).unwrap();
    assert_eq!(reloaded.computation_settings, receipt.settings);
    assert_eq!(project.state().get_data().unwrap().computation_settings, receipt.settings);
}
```

- [ ] **Step 2: Run Rust RED test**

```sh
pnpm rust:test --lib computation_settings_mutation_commits_disk_and_authority_atomically
```

Expected: compile failure for missing mutation API.

- [ ] **Step 3: Implement atomic backend mutation**

Use the existing project filesystem transaction/coordinator pattern:

```text
capture project/session/settings revision
build next manifest
stage and validate metadata.yssbi outside global locks
revalidate authority
commit disk
install ProjectData settings + revision + publication revision under short locks
rollback committed disk if publication fails
finalize and emit one event
```

The mutation advances authority generation and exposes a versioned compile resource key `project/computation-settings`. Compilation records this resource only for nodes that consume tolerance or missing-value settings; the tests compile one affected and one unrelated graph and prove only the affected basis becomes stale after mutation.

- [ ] **Step 4: Implement thin commands and committed event**

Commands parse DTOs, call `ProjectState`, map receipts, and emit `ComputationSettingsChanged`. They do no filesystem work directly.

- [ ] **Step 5: Write failing frontend service/hook/UI tests**

```ts
it('loads project computation settings from the backend instead of localStorage', async () => {
  invokeMock.mockResolvedValue(settingsDto());
  renderHook(() => useProjectComputationSettings(), { wrapper: projectWrapper() });
  await waitFor(() => expect(invokeMock).toHaveBeenCalledWith(
    'get_project_computation_settings',
    { projectInstanceId: 'project-1' },
  ));
});

it('disables computation settings when no project is open', () => {
  render(<SettingsView />);
  expect(screen.getByRole('group', { name: /computation/i })).toHaveAttribute('aria-disabled', 'true');
});

it('asks before discarding a dirty computation draft', async () => {
  invokeMock.mockResolvedValue(settingsDto());
  render(<SettingsView />);
  await userEvent.clear(screen.getByLabelText(/absolute tolerance/i));
  await userEvent.type(screen.getByLabelText(/absolute tolerance/i), '1e-8');
  await userEvent.click(screen.getByRole('button', { name: /close settings/i }));
  expect(screen.getByRole('dialog', { name: /discard computation changes/i }))
    .toBeVisible();
});
```

Use the existing application confirmation modal API in the test wrapper; do not use `window.confirm`.

- [ ] **Step 6: Run frontend RED tests**

```sh
pnpm test -- src/services/project/projectService.test.ts
pnpm test -- src/features/application/projectSettings/useProjectComputationSettings.test.ts
```

Expected: tests fail because service/hook APIs do not exist.

- [ ] **Step 7: Implement service, application hook, and shadcn UI**

Add strict parsers. The hook owns project-scoped confirmed state and local draft state, rejects stale project responses, and reconciles direct receipt/event echo by operation ID. `SettingsView` adds absolute tolerance, relative tolerance, Listwise/Reject select, formula help, Apply, and Restore Recommended Values. It uses the existing application confirmation modal before closing, changing section, or changing project with a dirty computation draft. It does not write these fields to `useSettingsStore` and never calls browser `alert`, `prompt`, or `confirm`.

- [ ] **Step 8: Run GREEN tests**

```sh
pnpm rust:test --lib computation_settings_mutation_commits_disk_and_authority_atomically
pnpm rust:test --lib stale_computation_settings_revision_changes_nothing
pnpm rust:test --lib computation_settings_disk_failure_preserves_memory_and_manifest
pnpm rust:test --lib computation_settings_publication_failure_rolls_back_manifest_and_memory
pnpm rust:test --lib computation_settings_commit_emits_exactly_one_event
pnpm rust:test --lib computation_settings_invalidation_only_stales_consuming_plans
pnpm test -- src/services/project/projectService.test.ts
pnpm test -- src/features/application/projectSettings/useProjectComputationSettings.test.ts
pnpm test -- src/views/EditorView/Layout/SettingsView.test.tsx
pnpm typecheck
pnpm rust:check
```

- [ ] **Step 9: Commit checkpoint only if authorized**

```sh
git add src-tauri/src/project src-tauri/src/commands/command_project src-tauri/src/event src-tauri/src/lib.rs src/shared/types/dto src/services/project src/features/application/projectSettings src/views/EditorView/Layout/SettingsView.tsx
git commit -m "Add project computation settings"
```

---

### Task 8: Shared Numeric Tolerance and Missing-Value Algorithms

**Files:**
- Create: `src-tauri/src/node_system/runtime/numeric.rs`
- Modify: `src-tauri/src/node_system/runtime/mod.rs`
- Modify: `src-tauri/src/node_system/runtime/data_series.rs`
- Test: `src-tauri/src/node_system/runtime/tests.rs`
- Modify: `src-tauri/src/node_system/runtime/kernel.rs` to expose effective settings through `KernelContext`.

**Interfaces:**
- Consumes: Task 6 `NumericTolerance` and `StatisticalMissingValuePolicy`; Task 4 typed readers.
- Produces:

```rust
pub enum NumericValue { Int64(i64), Float64(f64) }
pub fn approximately_equal(left: f64, right: f64, tolerance: NumericTolerance) -> bool;
pub fn approximately_zero(value: f64, tolerance: NumericTolerance) -> bool;
pub fn numeric_equal(left: NumericValue, right: NumericValue, tolerance: NumericTolerance) -> Result<bool, NumericError>;
pub fn numeric_ordering(left: NumericValue, right: NumericValue) -> Result<std::cmp::Ordering, NumericError>;
pub fn listwise_numeric_rows(inputs: &[NumericSeriesView<'_>]) -> Result<ListwiseRows, KernelError>;
```

- [ ] **Step 1: Write failing tolerance tests**

Cover absolute dominance, relative dominance, NaN, same/opposite infinity, signed zero, exact Int64, safe mixed comparison, and >`2^53` rejection.

```rust
#[test]
fn approximate_equality_handles_special_values_and_exact_ints() {
    let tolerance = NumericTolerance { absolute: 1e-12, relative: 1e-9 };
    assert!(approximately_equal(1.0, 1.0 + 1e-10, tolerance));
    assert!(!approximately_equal(f64::NAN, f64::NAN, tolerance));
    assert!(approximately_equal(f64::INFINITY, f64::INFINITY, tolerance));
    assert!(!approximately_equal(f64::INFINITY, f64::NEG_INFINITY, tolerance));
    assert!(numeric_equal(NumericValue::Int64(7), NumericValue::Int64(7), tolerance).unwrap());
}
```

- [ ] **Step 2: Write failing listwise/reject tests**

Use Int64 and Float64 Artifact fixtures with independent validity bitmaps and NaNs. Assert combined mask counts Null and NaN separately, rejects infinity, and Reject reports input/row/kind.

- [ ] **Step 3: Run RED tests**

```sh
pnpm rust:test --lib approximate_equality_handles_special_values_and_exact_ints
pnpm rust:test --lib listwise_rows_combines_all_model_inputs
```

Expected: compile failure for missing numeric APIs.

- [ ] **Step 4: Implement with narrow `num_traits` bounds**

Use `Float` for generic approximate equality and `ToPrimitive` only at explicit checked conversion boundaries. `numeric_equal` uses tolerance only when Float64 participates; `numeric_ordering` never uses tolerance. Keep Int64 arithmetic/comparison exact, reject mixed comparison when an Int64 cannot be represented exactly as `f64`, and test that ordered comparison remains unchanged inside the tolerance window. Add effective settings to `KernelContext` snapshots; do not consult mutable global project state during a kernel run.

- [ ] **Step 5: Run GREEN tests**

```sh
pnpm rust:test --lib approximate_equality_handles_special_values_and_exact_ints
pnpm rust:test --lib mixed_numeric_comparison_rejects_lossy_int64_conversion
pnpm rust:test --lib numeric_ordering_ignores_tolerance
pnpm rust:test --lib listwise_rows_combines_all_model_inputs
pnpm rust:test --lib reject_missing_value_reports_input_and_row
pnpm rust:check
```

- [ ] **Step 6: Commit checkpoint only if authorized**

```sh
git add src-tauri/src/node_system/runtime/numeric.rs src-tauri/src/node_system/runtime/data_series.rs src-tauri/src/node_system/runtime/kernel.rs src-tauri/src/node_system/runtime/mod.rs
git commit -m "Add project numeric execution policy"
```

---

### Task 9: Distribution, Math, and Conversion Artifact Producers

**Files:**
- Modify: `src-tauri/src/node_system/catalog/distribution/mod.rs`
- Modify: `src-tauri/src/node_system/runtime/kernels/distribution/mod.rs`
- Modify: `src-tauri/src/node_system/catalog/core_nodes/math.rs`
- Modify: `src-tauri/src/node_system/runtime/kernels/core_nodes/math.rs`
- Modify: `src-tauri/src/node_system/catalog/core_nodes/value.rs`
- Modify: `src-tauri/src/node_system/runtime/kernels/core_nodes/value.rs`
- Test: `src-tauri/src/node_system/runtime/builtin_tests.rs`

**Interfaces:**
- Consumes: canonical types, typed Artifact builders, numeric policies.
- Produces continuous `DataSeries<Float64>` and discrete `DataSeries<Int64>` distribution outputs; typed math/conversion Artifact outputs.

- [ ] **Step 1: Write failing producer tests**

```rust
#[test]
fn continuous_distribution_returns_float_data_series_artifact() {
    let output = execute_distribution("yssbi.distribution.normal", normal_inputs(4)).unwrap();
    let artifact = require_data_series(&output[0]).unwrap();
    assert_eq!(artifact.data_series_metadata().unwrap().element_type, DataSeriesElementType::Float64);
    assert_eq!(artifact.data_series_metadata().unwrap().length, 4);
}

#[test]
fn series_conversion_rejects_scalar_list_and_preserves_nulls() {
    let source = int_series_artifact([Some(1), None, Some(3)]);
    let converted = execute_series_conversion(source, ConvertTarget::Float64).unwrap();
    assert_eq!(converted.metadata.null_count, 1);
    assert!(execute_series_conversion(RuntimeValue::Scalar(Value::List(vec![])), ConvertTarget::Float64).is_err());
}
```

- [ ] **Step 2: Run RED tests separately**

```sh
pnpm rust:test --lib continuous_distribution_returns_float_data_series_artifact
pnpm rust:test --lib series_conversion_rejects_scalar_list_and_preserves_nulls
```

Expected: distribution assertion fails because it returns scalar Object; conversion contract test fails until metadata APIs are used.

- [ ] **Step 3: Migrate distribution outputs**

Replace `series_value()` Object encoding with `DataSeriesArtifactBuilder`. Preserve name/format in metadata. Keep integer parameters strict and validate positive sample count.

- [ ] **Step 4: Converge math and conversion on shared readers/builders**

Remove generic Artifact acceptance; require DataSeries payload kind. Implement promotion:

```text
Int64 + Int64 -> Int64 with checked arithmetic
any Float64 operand -> Float64
division -> Float64
```

Require at least one series operand for series math. Float64-to-Int64 conversion rejects non-integral/out-of-range values.

- [ ] **Step 5: Run GREEN and existing family tests**

```sh
pnpm rust:test --lib continuous_distribution_returns_float_data_series_artifact
pnpm rust:test --lib discrete_distribution_returns_int_data_series_artifact
pnpm rust:test --lib series_conversion_rejects_scalar_list_and_preserves_nulls
pnpm rust:test --lib series_conversion_kernels_cover_every_legacy_conversion
pnpm rust:test --lib unary_math_kernels_execute_each_legacy_operation
pnpm rust:check
```

- [ ] **Step 6: Commit checkpoint only if authorized**

```sh
git add src-tauri/src/node_system/catalog/distribution src-tauri/src/node_system/catalog/core_nodes src-tauri/src/node_system/runtime/kernels/distribution src-tauri/src/node_system/runtime/kernels/core_nodes
git commit -m "Migrate numeric series producers"
```

---

### Task 10: DataFrame and DataSeries Kernel Migration

**Files:**
- Modify: `src-tauri/src/node_system/catalog/dataframe/mod.rs`
- Modify: `src-tauri/src/node_system/catalog/dataframe/tests.rs`
- Modify: `src-tauri/src/node_system/compiler/dataframe.rs`
- Modify: `src-tauri/src/node_system/runtime/kernels/dataframe/mod.rs`
- Test: `src-tauri/src/node_system/runtime/builtin_tests.rs`

**Interfaces:**
- Consumes: Tasks 1, 4, 8 Artifact/type APIs.
- Produces schema-derived DataSeries Artifacts and strict operation contracts listed in the design.

- [ ] **Step 1: Write failing protocol tests**

Assert:

```text
Filter condition -> DataSeries<Boolean>
Length output -> Int64
Sum/Mean/Standardize/Difference inputs -> numeric outer union
Lag input/output -> one shared generic T
Decompose dynamic output -> schema-derived DataSeries<T>
```

- [ ] **Step 2: Write failing runtime tests**

Add one focused test per behavior group:

Implement four tests with fixtures defined in the same test module:

1. `dataframe_filter_consumes_boolean_series_artifact` filters a three-row DataFrame with `[true, null, false]` and asserts only row zero remains.
2. `dataframe_length_returns_int64_scalar` supplies a three-value Artifact containing one Null and asserts `RuntimeValue::Scalar(Value::Integer(3))`.
3. `dataframe_standardize_returns_float_artifact_and_propagates_nulls` supplies `[1, null, 3]`, asserts Float64 metadata with length three/null count one, and verifies the middle validity bit remains unset.
4. `dataframe_kernel_rejects_scalar_list_series_input` supplies a scalar list and asserts `expected DataSeries Artifact, received scalar`.

The fixtures must invoke production kernels with canonical Artifact builders; they must not recreate the kernel algorithm inside the test.

- [ ] **Step 3: Run RED tests separately**

```sh
pnpm rust:test --lib dataframe_filter_consumes_boolean_series_artifact
pnpm rust:test --lib dataframe_length_returns_int64_scalar
pnpm rust:test --lib dataframe_standardize_returns_float_artifact_and_propagates_nulls
```

Expected: behavior failures under scalar-list implementation.

- [ ] **Step 4: Replace all DataFrame catalog legacy series types**

Use canonical constructors:

```rust
fn series_type(element: TypeExpr) -> TypeExpr { data_series_type(element) }
fn numeric_series_type() -> TypeExpr { numeric_data_series_type() }
```

Remove `tabular.series` registration. Make dynamic schema outputs wrap resolved field types in `core.data_series<T>`.

- [ ] **Step 5: Migrate producers and consumers**

Producers returning Artifacts: Decompose, Select, IntegerRange, numeric comparisons, string comparisons, Standardize, InverseStandardize, Difference, PercentChange, RollingMean, Lag, PanelDifference.

Consumers requiring Artifacts: Combine, Filter mask, Length, Count, Sum, Mean, numeric comparisons, string comparisons, standardization, time-series and panel operations.

Keep numeric and string comparison protocols/kernels separate: numeric accepts only Int64/Float64 series/scalars and uses Task 8 equality semantics; string accepts only String series/scalars and exact string equality. Both return Boolean Artifacts and propagate Null. Apply the remaining exact output and Null policies from the design. Filter validates Boolean metadata before evaluating Null as false.

- [ ] **Step 6: Run GREEN tests and production catalog checks**

```sh
pnpm rust:test --lib dataframe_filter_consumes_boolean_series_artifact
pnpm rust:test --lib dataframe_length_returns_int64_scalar
pnpm rust:test --lib dataframe_standardize_returns_float_artifact_and_propagates_nulls
pnpm rust:test --lib dataframe_numeric_and_string_comparisons_remain_separate
pnpm rust:test --lib dataframe_kernel_rejects_scalar_list_series_input
pnpm rust:test --lib dataframe_protocols_have_unique_ports_and_valid_bindings
pnpm rust:test --lib production_decompose_projects_database_column_metadata
pnpm rust:check
```

- [ ] **Step 7: Commit checkpoint only if authorized**

```sh
git add src-tauri/src/node_system/catalog/dataframe src-tauri/src/node_system/compiler/dataframe.rs src-tauri/src/node_system/runtime/kernels/dataframe
git commit -m "Migrate DataFrame series artifacts"
```

---

### Task 11: Statistics Artifact, Tolerance, Missing Values, and Model Contracts

**Files:**
- Modify: `src-tauri/src/node_system/catalog/statistics/mod.rs`
- Modify: `src-tauri/src/node_system/catalog/statistics/families.rs`
- Modify: `src-tauri/src/node_system/catalog/statistics/tests.rs`
- Modify: `src-tauri/src/node_system/runtime/kernels/statistics/mod.rs`
- Modify: `src-tauri/src/execution/presentation.rs`
- Modify: `src-tauri/src/sci/api/node_statistics.rs`
- Modify: `src-tauri/src/sci/models/regression.rs`
- Modify: `src-tauri/src/sci/models/panel_did.rs` only if its result participates in the shared observation metadata contract.
- Modify: `src/views/EditorView/Layout/Detail/node/parameterEditors/NodeParameterEditor.tsx`
- Test: `src-tauri/src/node_system/runtime/builtin_tests.rs`
- Test: unit-test modules in `src-tauri/src/sci/api/node_statistics.rs`, `src-tauri/src/sci/models/regression.rs`, and `src-tauri/src/sci/models/panel_did.rs` for APIs changed by this task.
- Test: `src/views/EditorView/Layout/Detail/node/parameterEditors/NodeParameterEditor.test.tsx`.

**Interfaces:**
- Consumes: numeric Artifact readers, project settings snapshot, listwise/reject APIs.
- Produces family-specific model types and Float64 fitted/residual/prediction Artifacts.
- Produces projected optional override parameters where omission means `Inherit project setting`; the detail editor can set or clear convergence tolerance and `Listwise | Reject` missing-value overrides.

- [ ] **Step 1: Write failing protocol/model tests**

Assert numeric outer-union inputs, Float64 series outputs, family-specific model types, and exact IV cardinality. Assert OLS model cannot connect to Logit Predict.

- [ ] **Step 2: Write failing runtime Artifact tests**

```rust
#[test]
fn statistics_fit_consumes_artifacts_and_returns_float_series_artifacts() {
    let result = execute_ols_fit(int_series([1, 2, 3]), vec![float_series([1.0, 2.0, 4.0])]);
    assert_eq!(series_element_type(&result.fitted), DataSeriesElementType::Float64);
    assert_eq!(series_element_type(&result.residuals), DataSeriesElementType::Float64);
}

#[test]
fn statistics_listwise_reports_removed_null_and_nan_rows() {
    let result = execute_ols_with_policy(response_with_null(), predictor_with_nan(), StatisticalMissingValuePolicy::Listwise).unwrap();
    assert_eq!(result.metadata.original_observation_count, 4);
    assert_eq!(result.metadata.used_observation_count, 2);
    assert_eq!(result.metadata.dropped_null_count, 1);
    assert_eq!(result.metadata.dropped_nan_count, 1);
}
```

Add Reject and infinity tests with exact port/row diagnostics. In the same test module, define every fixture/helper used by these tests (`int_series`, `float_series`, `series_element_type`, `response_with_null`, `predictor_with_nan`, and the execution helpers); they must build canonical Artifacts and invoke the production statistics kernel rather than bypassing it.

Add a frontend editor test that renders each eligible projected override parameter, selects `Inherit project setting`, and asserts `setNodeParameters` receives a removal/`null` representation accepted by the backend parameter mutation contract. Re-render with an explicit override and assert the effective value and `Node override` state are visible. Define the projected `ParameterEditorDto` fixtures in `NodeParameterEditor.test.tsx`.

- [ ] **Step 3: Run RED tests**

```sh
pnpm rust:test --lib statistics_fit_consumes_artifacts_and_returns_float_series_artifacts
pnpm rust:test --lib statistics_listwise_reports_removed_null_and_nan_rows
pnpm test -- src/views/EditorView/Layout/Detail/node/parameterEditors/NodeParameterEditor.test.tsx
```

Expected: failures because statistics currently consumes/produces scalar lists and lacks settings metadata.

- [ ] **Step 4: Migrate statistics protocols and runtime**

Consume typed numeric Artifacts. Represent absent override parameters as inheritance; validate explicit convergence tolerance as finite and greater than zero and explicit missing policy as `Listwise | Reject`. Resolve effective tolerance/missing policy from node override then project snapshot, and include both value and source in compiled parameters/cache identity. Apply the listwise mask before passing dense `f64` arrays into scientific APIs. Reject infinity. Build Float64 Artifacts for fitted/residual/prediction. Keep model/config/report/result as scalar/opaque values.

Extend the existing projected parameter editor contract with an optional inherited-value presentation rather than creating a statistics-specific settings store. `NodeParameterEditor` renders `Inherit project setting` and `Node override`, clears the parameter to inherit, and edits an explicit tolerance/policy only in override mode.

- [ ] **Step 5: Add report and execution metadata**

Serialize observation counts, missing policy, effective tolerance, and source. Emit one execution log per model fit summarizing rows used/dropped without per-element noise.

- [ ] **Step 6: Run GREEN and model-family tests**

```sh
pnpm rust:test --lib statistics_fit_consumes_artifacts_and_returns_float_series_artifacts
pnpm rust:test --lib statistics_listwise_reports_removed_null_and_nan_rows
pnpm rust:test --lib statistics_reject_reports_port_and_row
pnpm rust:test --lib prediction_rejects_incompatible_model_family
pnpm rust:test --lib operation_specific_statistics_match_sci_golden_fixtures
pnpm rust:test --lib ols_summary_accepts_numeric_data_series_and_rejects_string_series
pnpm test -- src/views/EditorView/Layout/Detail/node/parameterEditors/NodeParameterEditor.test.tsx
pnpm typecheck
pnpm rust:check
```

- [ ] **Step 7: Commit checkpoint only if authorized**

```sh
git add src-tauri/src/node_system/catalog/statistics src-tauri/src/node_system/runtime/kernels/statistics src-tauri/src/execution src-tauri/src/sci src/views/EditorView/Layout/Detail/node/parameterEditors
git commit -m "Migrate statistics to series artifacts"
```

---

### Task 12: Plot Artifact Consumers and Result Source Projection

**Files:**
- Modify: `src-tauri/src/node_system/catalog/plot/mod.rs`
- Modify: `src-tauri/src/node_system/runtime/kernels/plot/mod.rs`
- Modify: `src-tauri/src/node_system/runtime/artifact.rs`
- Modify: `src-tauri/src/node_system/runtime/result_store.rs`
- Modify: `src-tauri/src/commands/node_system_execution_dto.rs`
- Test: `src-tauri/src/node_system/runtime/builtin_tests.rs`
- Test: `src-tauri/src/node_system/runtime/tests.rs`

**Interfaces:**
- Consumes: numeric DataSeries Artifacts and metadata.
- Produces DataSeries-aware result descriptors/pages; Plot consumes only Artifacts.

- [ ] **Step 1: Write failing plot tests**

Add three tests using production-kernel fixtures defined in `builtin_tests.rs`:

1. `scatter_consumes_two_numeric_data_series_artifacts` builds named Int64 and Float64 Artifacts, executes Scatter, and asserts the published payload contains all input points in order.
2. `plot_rejects_scalar_list_series_input` supplies `RuntimeValue::Scalar(Value::List(_))` and asserts the exact `expected DataSeries Artifact, received scalar` internal contract error.
3. `plot_preserves_data_series_name_and_format_metadata` supplies `name` and `format` metadata and asserts the projected plot labels retain both fields.

Define all Artifact, kernel-context, and publication-store fixtures in the same test module. Add a protocol test proving Date series is not accepted by Scatter/Line until a temporal kernel exists, and Correlogram accepts Int64/Float64 numeric series.

- [ ] **Step 2: Run RED tests**

```sh
pnpm rust:test --lib scatter_consumes_two_numeric_data_series_artifacts
pnpm rust:test --lib plot_rejects_scalar_list_series_input
```

Expected: failures under scalar List/Object decoder.

- [ ] **Step 3: Migrate Plot and result descriptors**

Delete List/Object decoder. Read numeric values and metadata from Artifact. Add `DataSeries` result-source kind and optional metadata DTO so pagination/inspection preserves business type through in-memory/spill/replay snapshots.

- [ ] **Step 4: Run GREEN and paging tests**

```sh
pnpm rust:test --lib scatter_consumes_two_numeric_data_series_artifacts
pnpm rust:test --lib plot_rejects_scalar_list_series_input
pnpm rust:test --lib plot_preserves_data_series_name_and_format_metadata
pnpm rust:test --lib spilled_data_series_is_pageable_as_data_series
pnpm rust:test --lib result_store_paging_propagates_spill_read_failures
pnpm rust:check
```

- [ ] **Step 5: Commit checkpoint only if authorized**

```sh
git add src-tauri/src/node_system/catalog/plot src-tauri/src/node_system/runtime/kernels/plot src-tauri/src/node_system/runtime/artifact.rs src-tauri/src/node_system/runtime/result_store.rs src-tauri/src/commands/node_system_execution_dto.rs
git commit -m "Consume DataSeries artifacts in plots"
```

---

### Task 13: Variables, Functions, Cross-Family Flows, and Legacy Removal

**Files:**
- Modify: `src-tauri/src/variable/variable_instance.rs`
- Modify: `src-tauri/src/variable/mod.rs`
- Modify: `src-tauri/src/project/project_state_variable.rs`
- Modify: `src-tauri/src/project/project_state.rs`
- Modify: `src-tauri/src/node_system/compatibility.rs`
- Modify: `src-tauri/src/node_system/compiler/pipeline.rs`
- Modify: `src-tauri/src/node_system/runtime/function_plan.rs`
- Modify: `src-tauri/src/node_system/analysis/projection.rs`
- Modify: `src-tauri/src/node_system/catalog/dataframe/mod.rs`
- Modify: `src-tauri/src/node_system/catalog/distribution/mod.rs`
- Modify: `src-tauri/src/node_system/catalog/plot/mod.rs`
- Test: `src-tauri/src/project/production_tests.rs`
- Test: `src-tauri/src/node_system/runtime/builtin_tests.rs`
- Test: `src-tauri/src/node_system/compiler/tests.rs`

**Interfaces:**
- Consumes: all prior canonical type/runtime APIs.
- Produces end-to-end DataSeries Artifact continuity through variables, functions, memoization, publication, and cross-family chains.

- [ ] **Step 1: Write failing cross-family integration tests**

Add these exact flows:

```text
Distribution<Float64> -> OLS/plot
DataFrame Decompose<Int64> -> conversion -> OLS Summary
DataFrame Decompose<Float64> -> math -> plot
Statistics prediction -> plot
DataSeries function argument/result -> downstream math
DataSeries variable get/set -> downstream statistics
spill-backed DataSeries -> memoized consumer
```

Each test asserts both compile success and Artifact metadata at runtime.

- [ ] **Step 2: Run RED tests one at a time**

```sh
pnpm rust:test --lib distribution_series_flows_into_statistics_without_scalar_encoding
pnpm rust:test --lib dataframe_selected_series_flows_into_math_and_statistics
pnpm rust:test --lib function_call_preserves_data_series_artifact
```

Expected: failures identify remaining scalar or legacy type boundaries.

- [ ] **Step 3: Migrate variables/functions and remaining boundaries**

Ensure `DataType::DataSeries<T>` maps only to Applied `core.data_series<T>`, variable values are stored/published as Artifacts in graph execution, and function ABI contracts preserve DataSeries kind and element type. Keep persisted variable domain data serializable; create an Artifact when entering execution rather than serializing runtime Artifact internals into project files.

- [ ] **Step 4: Delete all legacy production paths**

Run scans:

```sh
git grep -n -E 'tabular\.series|core\.data_series\.(int64|float64|date|string|bool|categorical)' -- src-tauri/src src
git grep -n -E 'Scalar\(Value::(List|Object)' -- src-tauri/src/node_system/runtime/kernels
```

Expected after deletion:

- first command returns no production matches;
- second returns no DataSeries producer/consumer matches (non-DataSeries scalar list/object usage must be reviewed and documented in the test or code context).

Delete legacy registrations, projection branches, exact-ID compatibility tests, List/Object DataSeries decoder branches, and temporary migration helpers.

- [ ] **Step 5: Run GREEN cross-family tests**

```sh
pnpm rust:test --lib distribution_series_flows_into_statistics_without_scalar_encoding
pnpm rust:test --lib dataframe_selected_series_flows_into_math_and_statistics
pnpm rust:test --lib statistics_prediction_flows_into_plot
pnpm rust:test --lib function_call_preserves_data_series_artifact
pnpm rust:test --lib data_series_artifact_survives_memoization
pnpm rust:check
```

- [ ] **Step 6: Run frontend canonical compatibility tests**

```sh
pnpm test -- src/shared/utils/pinCompatibility.test.ts
pnpm test -- src/features/core/dataStore/graphDataStore.test.ts
pnpm test -- src/services/project/projectService.test.ts
pnpm typecheck
```

- [ ] **Step 7: Commit checkpoint only if authorized**

```sh
git add src-tauri/src/variable src-tauri/src/project/project_state_variable.rs src-tauri/src/project/project_state.rs src-tauri/src/node_system/compatibility.rs src-tauri/src/node_system/compiler/pipeline.rs src-tauri/src/node_system/runtime/function_plan.rs src-tauri/src/node_system/analysis/projection.rs src-tauri/src/node_system/catalog
git commit -m "Complete canonical DataSeries migration"
```

---

### Task 14: Full Verification and Documentation Closure

**Files:**
- Modify: `docs/development/LOCAL_WORKFLOW.md` only if computation-setting or focused test commands need documenting.
- Modify: `docs/superpowers/specs/2026-08-12-canonical-data-series-runtime-design.md` only if implementation reveals an approved design correction.
- Test: all focused suites from Tasks 1-13.

**Interfaces:**
- Consumes: completed migration.
- Produces: fresh verification evidence and a clean delivery report.

- [ ] **Step 1: Run canonical legacy scans**

```sh
git grep -n -E 'tabular\.series|core\.data_series\.(int64|float64|date|string|bool|categorical)' -- src-tauri/src src
git grep -n 'DataSeries<any>' -- src-tauri/src src
```

Expected: no production matches. Test fixtures that intentionally assert rejection must be renamed to canonical expressions or removed.

- [ ] **Step 2: Run focused Rust suites**

Run each filter separately to isolate failures:

```sh
pnpm rust:test --lib data_series_artifact
pnpm rust:test --lib type_conformance
pnpm rust:test --lib computation_settings
pnpm rust:test --lib dataframe_
pnpm rust:test --lib statistics_
pnpm rust:test --lib distribution_
pnpm rust:test --lib plot_
pnpm rust:test --lib function_call_preserves_data_series
```

Expected: all selected tests pass. Do not use full Rust test as the first validation.

- [ ] **Step 3: Run focused frontend suites**

```sh
pnpm test -- src/shared/utils/pinCompatibility.test.ts
pnpm test -- src/features/core/dataStore/graphDataStore.test.ts
pnpm test -- src/shared/types/dto/editorMutationWireParser.test.ts
pnpm test -- src/services/project/projectService.test.ts
pnpm test -- src/features/application/projectSettings/useProjectComputationSettings.test.ts
pnpm test -- src/views/EditorView/Layout/SettingsView.test.tsx
pnpm typecheck
```

Expected: all pass.

- [ ] **Step 4: Run project-required static checks**

```sh
pnpm rust:fmt:check
pnpm rust:check
git diff --check
```

Expected: exit 0. Existing warnings may be reported but no new errors.

- [ ] **Step 5: Run full local verification**

```sh
pnpm verify
```

Expected: exit 0. If the previously known unrelated `projectFilesystemContract` failure remains unchanged, capture its exact output and prove the offending files were not modified by this migration; do not alter unrelated code to hide it. If the known `libduckdb_sys` environment issue prevents broad Rust integration targets, report it separately with the focused `--lib` evidence.

- [ ] **Step 6: Review requirements against implementation**

Confirm explicitly:

```text
one canonical DataSeries TypeExpr
one Artifact runtime representation
Number/String separation
project tolerance and missing-value persistence
effective node overrides
Listwise/Reject metadata and errors
three-state frontend/backend compatibility
no legacy type IDs or scalar-list DataSeries paths
cross-family and function/variable Artifact continuity
```

- [ ] **Step 7: Commit final documentation only if commits were explicitly authorized**

```sh
git add docs/development/LOCAL_WORKFLOW.md docs/superpowers/specs/2026-08-12-canonical-data-series-runtime-design.md docs/superpowers/plans/2026-08-12-canonical-data-series-runtime.md
git commit -m "Document canonical DataSeries workflow"
```
