# Canonical DataSeries Runtime Design

## Status

Approved design. This document specifies the complete replacement of the
project's incompatible DataSeries protocol and runtime representations with one
canonical type and one canonical runtime value.

## Problem

The project currently has three mutually incompatible DataSeries protocol
representations:

- `tabular.series`
- nominal IDs such as `core.data_series.float64`,
  `core.data_series.int64`, and `core.data_series.date`
- parameterized `core.data_series<T>`

The compiler compares these representations by exact `TypeExpr` identity. The
editor projection, however, maps several of them to the same frontend
`DataSeries<T>` shape. This creates type islands and contradictory behavior:
the UI can show compatible-looking ports while the backend rejects the
connection.

Runtime representation is also split. DataFrame, statistics, plot, and
distribution kernels commonly use `RuntimeValue::Scalar(Value::List/Object)`;
math and conversion kernels use `RuntimeValue::Artifact`. Unifying only the
static type would therefore create graphs that compile but fail at runtime.

The migration must replace both layers together. This is a 0.x project, so no
legacy aliases, compatibility shims, or permanent scalar-list adapters will
remain.

## Scope

This design covers:

- canonical DataSeries `TypeExpr` representation;
- numeric and string type separation;
- type assignment, projection, and editor connection preflight;
- one Artifact-based DataSeries runtime representation;
- migration of every DataSeries producer and consumer;
- correction of confirmed node protocol/runtime mismatches;
- project-authoritative numeric tolerance and missing-value policy;
- node-level overrides for statistical convergence and missing values;
- project settings UI for computation settings;
- regression, runtime, persistence, and frontend verification.

This design does not introduce UInt64 as a graph-visible type. `Number` means
`Int64 | Float64` only. It also does not retain old project execution paths.

## 1. Canonical type architecture

### 1.1 DataSeries representation

The only production DataSeries representation is:

```text
core.data_series<T>
```

Canonical examples are:

```text
core.data_series<core.int64>
core.data_series<core.float64>
core.data_series<core.string>
core.data_series<core.bool>
core.data_series<core.date>
core.data_series<core.datetime>
core.data_series<core.categorical>
```

The migration removes these production type IDs:

```text
tabular.series
core.data_series.int64
core.data_series.float64
core.data_series.date
core.data_series.string
core.data_series.bool
core.data_series.categorical
```

Their registrations, projection branches, tests, and runtime assumptions are
deleted. No aliasing rule is added to the compiler.

### 1.2 Number

The backend retains exact scalar types:

```text
core.int64
core.float64
```

`core.numeric` is a type class whose members are `core.int64` and
`core.float64`. Generic protocols may express `T implements core.numeric`.
Where current protocol constraints cannot express the required relationship,
a numeric series input uses an outer union:

```text
core.data_series<core.int64>
|
core.data_series<core.float64>
```

It does not use:

```text
core.data_series<core.int64 | core.float64>
```

The outer union means one homogeneous integer series or one homogeneous float
series. A union inside the element type would permit mixed element types.

`Number` and `DataSeries<Number>` are frontend display names only. They do not
introduce a nominal `core.number` protocol type and do not erase a resolved
`Int64` or `Float64` type.

### 1.3 String separation

String remains a separate scalar and series family:

```text
core.string
core.data_series<core.string>
```

Arithmetic, numeric statistics, and numeric comparisons reject String.
Crossing the Number/String boundary requires an explicit conversion node.

### 1.4 Assignment semantics

Assignment follows value-set containment:

```text
Assignable(source, target) iff Values(source) is a subset of Values(target)
```

Rules:

- every source-union member must be assignable to the target;
- a target union accepts a source when one complete target member accepts it;
- `core.data_series<T>` is covariant in `T`;
- canonical unions are flattened, deduplicated, deterministically ordered, and
  reduced when they contain one member;
- empty unions are invalid rather than interpreted as Any;
- `Unknown` means unresolved information and yields an indeterminate
  compatibility result;
- `Unknown` is not an explicit dynamic Any type.

Compiler assignment, catalog compatibility, mutation validation, projection,
and frontend preflight share one conformance matrix.

### 1.5 Unknown projection

An unresolved type is projected as unresolved:

```text
resolved: false
dataType: null
```

The UI displays `Unresolved` or `DataSeries<Unresolved>`, not `Any` or
`DataSeries<any>`. If an explicit dynamic Any type is needed later, it receives
a separate protocol representation and runtime contract.

## 2. Canonical Artifact runtime

### 2.1 Runtime representation

Every DataSeries producer returns and every DataSeries consumer accepts:

```rust
RuntimeValue::Artifact
```

Production DataSeries values are never represented by:

```rust
RuntimeValue::Scalar(Value::List(_))
RuntimeValue::Scalar(Value::Object { .. })
```

Receiving one of those forms at a DataSeries kernel boundary is an internal
runtime contract error, not an implicit conversion opportunity.

### 2.2 Metadata

A DataSeries Artifact exposes authoritative metadata equivalent to:

```rust
struct DataSeriesMetadata {
    element_type: DataSeriesElementType,
    length: usize,
    null_count: usize,
    name: Option<String>,
}

enum DataSeriesElementType {
    Int64,
    Float64,
    String,
    Boolean,
    Date,
    Datetime,
    Categorical,
}
```

Storage may be in-memory, Polars-backed, spilled, or the materialized result of
a stream. Kernels access it through a common reader API rather than depending
on a storage implementation.

### 2.3 Typed readers

The runtime provides focused readers:

```rust
numeric_series(artifact, null_policy)
string_series(artifact, null_policy)
boolean_series(artifact, null_policy)
```

Numeric reading preserves the element type:

```rust
enum NumericSeriesView<'a> {
    Int64(Int64SeriesView<'a>),
    Float64(Float64SeriesView<'a>),
}
```

Algorithms that require floating-point math explicitly and safely promote an
integer view to `f64`.

### 2.4 `num_traits`

`num-traits` is already a direct workspace dependency. It is used only after
RuntimeValue and Artifact validation, inside pure numeric algorithms.

Preferred narrow traits include:

- `Zero + AddAssign` for sums;
- `PrimInt` for counts and indices;
- `Float` for floating-point algorithms and approximate equality;
- `ToPrimitive`, `FromPrimitive`, or `NumCast` at explicit conversion
  boundaries.

The serialized graph type system never contains a Rust trait object or a
`num_traits` trait name. A broad `T: Num` bound is avoided when the algorithm
requires narrower semantics such as checked integer arithmetic, floating-point
special values, or division.

### 2.5 Null storage

Null is represented independently from element values, for example through a
validity bitmap. It is not converted to NaN. The runtime distinguishes:

- Null;
- NaN;
- positive infinity;
- negative infinity.

Readers apply an explicit policy:

```rust
enum NullPolicy {
    Propagate,
    Skip,
    Reject,
}
```

Default operation policies are:

- element-wise operations: `Propagate`;
- aggregations: `Skip`;
- comparisons: `Propagate`;
- filter masks: Null behaves as false, documented explicitly;
- statistics: project/node statistical missing-value policy;
- length: total rows;
- count: non-null rows.

### 2.6 Planner boundary

`InputConsumption` and `OutputProduction` continue to drive streaming,
materialization, fan-out, spill, and Artifact lifecycle. The planner does not
convert scalar lists to Artifacts because only the Artifact representation
remains.

Artifact metadata is checked against the compiled port contract. A mismatch is
an internal runtime contract error.

## 3. Project computation settings

### 3.1 Authority and persistence

Computation settings are Rust-authoritative project data, not local frontend
preferences. They are persisted in the project manifest under a stable
`computationSettings` field with serde defaults for projects created before the
field existed.

The conceptual model is:

```rust
struct ProjectComputationSettings {
    numeric: NumericSettings,
    missing_values: MissingValueSettings,
}

struct NumericSettings {
    tolerance: NumericTolerance,
}

struct NumericTolerance {
    absolute: f64,
    relative: f64,
}

struct MissingValueSettings {
    statistics: StatisticalMissingValuePolicy,
}

enum StatisticalMissingValuePolicy {
    Listwise,
    Reject,
}
```

Defaults are:

```text
absolute tolerance: 1e-12
relative tolerance: 1e-9
statistics missing-value policy: Listwise
```

Validation requires finite, non-negative absolute and relative tolerances, and
they cannot both be zero. Node convergence overrides must be finite and greater
than zero.

### 3.2 Tolerance semantics

Approximate equality is:

```text
abs(left - right)
<= max(
  absoluteTolerance,
  relativeTolerance * max(abs(left), abs(right))
)
```

Rules:

- Int64/Int64 equality is exact;
- Float64 equality and inequality use project tolerance;
- floating-point zero tests use absolute tolerance;
- `<`, `>`, sorting, range boundaries, grouping, joins, hashes, and cache
  identity do not use tolerance;
- NaN is not approximately equal to any value, including itself;
- equal signed infinities compare equal, while opposite infinities do not;
- mixed Int64/Float64 comparison uses a checked conversion;
- an Int64 outside the exact `f64` integer range is not silently compared after
  a lossy conversion.

Only iterative statistical nodes can override convergence tolerance. Ordinary
comparison nodes always use the project setting. Effective settings and their
source (`project` or `node`) enter compiled parameters and cache identity.

### 3.3 Statistical missing values

The effective policy is:

```text
node override > project default
```

Supported policies are `Listwise` and `Reject`.

Listwise deletion builds one validity mask across every input used by the
model, including response, predictors, weights, entity, time, treatment,
endogenous variables, and instruments. A row containing Null or NaN in any
participating input is removed. Infinity is rejected rather than silently
removed.

Reject stops at the first Null or NaN and reports the input/column, row, and
value kind.

Statistical output and reports record:

```text
originalObservationCount
usedObservationCount
droppedNullCount
droppedNaNCount
missingValuePolicy
effectiveTolerance
toleranceSource
```

Listwise deletion is the default. Nodes may override it with Reject.

### 3.4 Mutation and cache invalidation

Updating computation settings is atomic:

1. parse and validate the complete replacement;
2. write a temporary manifest;
3. atomically replace the manifest;
4. update `ProjectState.project_data`;
5. advance project authority generation;
6. emit a project settings event;
7. invalidate compilation products whose nodes read computation settings.

Failures preserve the previous disk and memory state. The settings are exposed
as a versioned compile resource so plans using tolerance or missing-value
policy cannot reuse stale execution bases. Operations unrelated to computation
settings do not need invalidation.

### 3.5 Project settings UI

The existing project settings page gains a backend-backed Computation section:

```text
Project settings
  Basic information
  Computation
    Numeric comparison
      Absolute tolerance
      Relative tolerance
      Formula help
    Missing values
      Statistics default: Listwise | Reject
```

The service layer reads and writes the Rust-authoritative values. The page is
disabled when no project is open. Frontend validation gives immediate feedback,
but Rust performs final validation. The UI updates from the confirmed response
or project event, and an application confirmation modal protects unsaved edits.
A reset action restores the recommended project defaults.

Node detail editors expose `Inherit project setting` versus `Node override` for
eligible statistical convergence and missing-value parameters.

## 4. Node family migration

### 4.1 DataFrame and DataSeries

Required contracts include:

| Operation | Input | Output | Null behavior |
|---|---|---|---|
| Decompose | DataFrame | schema-derived `DataSeries<T>` | preserve |
| Combine | dynamic `DataSeries<T>` | DataFrame | preserve |
| Filter | DataFrame + `DataSeries<Boolean>` | DataFrame | Null mask is false |
| Length | `DataSeries<T>` | Int64 scalar | total rows |
| Count | `DataSeries<T>` | Int64 scalar | non-null rows |
| Sum | numeric series | Int64 or Float64 scalar | skip |
| Mean | numeric series | Float64 scalar | skip |
| Lag | `DataSeries<T>` | `DataSeries<T>` | propagate |
| Standardize | numeric series | `DataSeries<Float64>` | propagate |
| Difference | numeric series | `DataSeries<Float64>` | propagate |
| Numeric comparison | numeric series/scalar | `DataSeries<Boolean>` | propagate |
| String comparison | string series/scalar | `DataSeries<Boolean>` | propagate |

Integer sum uses checked arithmetic. Mean and operations requiring division
produce Float64. Filter no longer accepts an untyped series mask.

### 4.2 Statistics

Numeric statistical inputs accept homogeneous Int64 or Float64 series.
Calculated series outputs are Float64:

```text
fitted -> DataSeries<Float64>
residuals -> DataSeries<Float64>
prediction -> DataSeries<Float64>
```

Model types are family-specific, such as OLS, Logit, and Probit model types, so
an incompatible model/predictor pairing fails statically. IV dynamic-port
cardinalities match what the kernel consumes. Effective convergence and
missing-value settings are compiled and reported.

### 4.3 Plot

Plot inputs use parameterized numeric series. The current kernels do not
implement date-axis conversion, so Date support is removed from Scatter and
Line until a dedicated temporal-axis implementation exists. Correlogram accepts
both Int64 and Float64 numeric series when its runtime algorithm supports both.

### 4.4 Distribution

Continuous distributions produce `DataSeries<Float64>` Artifacts. Discrete
distributions produce `DataSeries<Int64>` Artifacts. Integer parameters remain
strict Int64 to prevent decimal truncation. Float parameter widening is only
performed where the protocol and conversion semantics explicitly guarantee it.

### 4.5 Numeric series math

Numeric series math uses precise promotion rules:

```text
Int64 + Int64 -> Int64
any operation involving Float64 -> Float64
division -> Float64
```

The corresponding DataSeries result uses the promoted element type. Integer
arithmetic is checked. A series-math node requires at least one DataSeries
operand; scalar-only arithmetic uses scalar numeric nodes.

### 4.6 Conversion

Series conversion nodes retain explicit source and target types. Float64 to
Int64 does not silently truncate: non-integral and out-of-range values are
errors unless a future explicit Round/Floor/Ceil node is used. String/Number
parsing behavior must be an explicit node parameter rather than an implicit
fallback. The general scalar conversion node resolves its output type from its
parameter through an interface resolver instead of remaining Unknown.

### 4.7 Variables and functions

DataSeries variables, function parameters, and function results use canonical
`core.data_series<T>` types and Artifact values. Function ABI and variable
resource projection preserve the element type. No scalar-list compatibility
path is retained.

## 5. Connection and error model

### 5.1 Three-state preflight

Frontend connection preflight returns:

```text
Compatible
Incompatible
Indeterminate
```

- Compatible: complete types prove assignment.
- Incompatible: complete types prove rejection.
- Indeterminate: unresolved generics, pending resolvers, or unsupported
  projection prevent a proof.

Indeterminate lets the user attempt a backend-authoritative mutation but does
not claim compatibility. Missing frontend `dataType` no longer fails open as
compatible.

Backend diagnostics contain source/target ports, source/target canonical types,
and a reason. Friendly diagnostics may describe `DataSeries<Number>` while
retaining exact canonical types in details.

### 5.2 Error categories

Compile-time errors include incompatible element types, non-Boolean masks,
wrong statistical model families, invalid dynamic-port cardinality, and
required unresolved constraints.

User-data runtime errors include Reject missing values, insufficient samples
after listwise deletion, non-integral Float64-to-Int64 conversion, integer
overflow, infinity in statistical input, length mismatch, and approximate-zero
divisors.

Internal contract errors include scalar-list DataSeries values, Artifact
metadata/storage disagreement, a kernel returning the wrong element type,
planner contract failures, and resolver output using an unregistered
constructor.

## 6. Migration sequence

The implementation is staged internally but delivered with no production
compatibility path:

1. canonical type helpers, numeric class, union normalization, Unknown handling,
   and three-state compatibility;
2. DataSeries Artifact metadata, typed readers, builders, null policies, and
   runtime contract validation;
3. project computation settings, persistence, authority, IPC, and project UI;
4. migration of all DataSeries producers;
5. migration of all DataSeries consumers;
6. correction of node protocol/runtime mismatches;
7. deletion of old type IDs, registrations, projections, scalar-list branches,
   adapters, and tests.

Producer migration precedes consumer migration so no final connection is
statically enabled before its runtime representation is supported.

## 7. Verification

### 7.1 Type conformance matrix

Compiler, catalog compatibility, mutation validation, projection, and frontend
preflight must agree on at least:

| Source | Target | Expected |
|---|---|---|
| `DataSeries<Int64>` | `DataSeries<Number>` | compatible |
| `DataSeries<Float64>` | `DataSeries<Number>` | compatible |
| numeric series outer union | `DataSeries<Number>` | compatible |
| `DataSeries<String>` | `DataSeries<Number>` | incompatible |
| `DataSeries<Boolean>` | filter mask | compatible |
| `DataSeries<Int64>` | filter mask | incompatible |
| `DataSeries<Int64> | DataSeries<String>` | `DataSeries<Int64>` | incompatible |
| Unknown | `DataSeries<Float64>` | indeterminate |
| `DataSeries<Unknown>` | `DataSeries<Float64>` | indeterminate |
| OLS model | Logit Predict | incompatible |
| OLS model | OLS Predict | compatible |

### 7.2 Runtime matrix

Each migrated family covers Int64, Float64, String, Boolean, Null, NaN,
infinity, empty series, one-element series, spill-backed series, length
mismatch, metadata/storage mismatch, and erroneous scalar-list input as
applicable.

### 7.3 Tolerance matrix

Tests cover absolute and relative dominance, signed zero, NaN, infinities,
exact Int64 comparison, checked mixed comparison, integers beyond `2^53`,
ordered comparisons unaffected by tolerance, project-setting invalidation, and
node override precedence.

### 7.4 Missing-value matrix

Tests cover listwise union masks, separate Null/NaN counts, Reject diagnostics,
infinity errors, empty/insufficient samples, project defaults, node overrides,
reports, and execution logs.

### 7.5 Frontend matrix

Tests cover Number display, unresolved display, three-state compatibility,
removal of missing-type fail-open behavior, project computation setting reads
and writes, backend rollback behavior, disabled state without a project, and
node inheritance/override presentation.

### 7.6 Completion criteria

The migration is complete only when:

1. no production source contains a removed DataSeries type ID;
2. no production kernel represents DataSeries as scalar List/Object;
3. every DataSeries protocol uses `core.data_series<T>`;
4. frontend and backend compatibility matrices agree;
5. statistics accepts numeric series and rejects string series;
6. computation settings persist with the project and participate in authority;
7. Listwise and Reject produce complete diagnostics and report metadata;
8. focused suites, relevant frontend suites, `pnpm verify`, and
   `git diff --check` pass, with any pre-existing unrelated failure reported
   separately.
