# Relational Filter and Project Lineage Design

## Status

Approved revised design for completing the remaining Phase 7 relational lineage
slice in `docs/plan/node-architecture.md`.

## Problem

The relational IR and production backend contain partial Project/Filter support,
but built-in DataFrame protocols lower only Source, Rename, and Limit. Existing
`yssbi.dataframe.filter` consumes an external Boolean Series and cannot safely
be rewritten as a same-relation predicate. Existing Decompose emits dynamic
Series outputs and is not a DataFrame projection.

The current schema facts discard column dtypes, complex parameters have no
strict nominal validation, static Catalog creation assumes defaultable
parameters, and Project/Filter/Rename currently drift through scalar object
materialization inside the relational evaluator.

## Goals

- Add stable `yssbi.dataframe.project` and
  `yssbi.dataframe.filter.rows` built-ins.
- Make both nodes Catalog-visible, parameterized, and configurable through
  schema-aware frontend editors driven only by Rust projection DTOs.
- Freeze exact persisted parameter shapes and strict nominal Rust codecs.
- Preserve normalized column types through schema analysis.
- Compile Source → Filter → Project → Rename → Limit into one backend island.
- Keep Project, Filter, Rename, and Limit DataFrame-native inside the island.
- Add deterministic structured predicate/projection lineage hints that are
  metadata-only and backend-validated.
- Preserve demand specialization, cancellation, resources, and publication.

## Non-goals

- Migrating the external Boolean-Series Filter.
- External mask alignment, joins, aggregation, sorting, windows, arbitrary SQL,
  cross-source predicates, derived projections, or multi-backend rewriting.
- Operational database predicate/projection pushdown in this slice.
- Typed tabular RuntimeValue/IPC transport; that belongs with Phase 8 streaming
  and materialization.
- Runtime memoization, scheduler parallelism, or deadlines.

## Stable node protocols

### Project DataFrame

Stable ID:

```text
yssbi.dataframe.project
```

Ports:

```text
source: DataFrame input, Streaming
result: DataFrame output, Streaming
```

Required parameter:

```text
columns: yssbi.dataframe.project_columns
```

Persisted JSON:

```json
["b", "a"]
```

Rules:

- list contains at least one non-empty column name;
- names are unique and preserve user order;
- every name exists in source schema;
- output contains only direct column references in exact parameter order;
- no rename or derived expression occurs.

### Filter Rows

Stable ID:

```text
yssbi.dataframe.filter.rows
```

Ports:

```text
source: DataFrame input, Streaming
result: DataFrame output, Streaming
```

Required parameter:

```text
predicate: yssbi.dataframe.filter_predicate
```

Persisted JSON:

```json
{
  "column": "amount",
  "operator": "greaterThan",
  "value": {
    "type": "decimal",
    "value": "10.5"
  }
}
```

Operators:

```text
equal
notEqual
lessThan
lessThanOrEqual
greaterThan
greaterThanOrEqual
isNull
isNotNull
```

Tagged literal wire forms:

```json
{ "type": "boolean", "value": true }
{ "type": "integer", "value": "9007199254740993" }
{ "type": "decimal", "value": "10.5" }
{ "type": "string", "value": "paid" }
```

Integer and decimal use canonical strings so JavaScript does not lose identity.
`isNull` and `isNotNull` forbid `value`; all comparisons require it. Strict
codecs reject unknown fields/tags, non-canonical numbers, nested literals, and
wrong field types.

## Nominal parameter authority

Register:

```text
yssbi.dataframe.project_columns
yssbi.dataframe.filter_predicate
```

GraphDocument still stores raw JSON parameter values. Authoritative codecs and typed structures live below Catalog in `node_system/parameter_types/dataframe`, which depends on protocol value/ID primitives such as `CanonicalDecimal` and `TypeId`. Protocol does not depend on parameter_types. Document, compiler, Registry, and Catalog depend one-way on protocol plus parameter_types, avoiding a cycle. A generic nominal validator registry binds nominal TypeId to a validator and is frozen into NodeRegistry. Missing validators for the two built-in nominal IDs are registry errors; unrelated custom types retain their existing behavior.

The nominal type registration binds those parameter keys to strict Rust codecs used by:

- editor mutation validation;
- compiler parameter normalization;
- schema analysis;
- defensive lowerer decoding.

Mutation validation cannot treat unknown concrete/applied types as sufficient
for these two nominal types.

Codec ownership:

- codec validates JSON shape, tag, empty Project list/name, duplicate Project
  columns, and operator/value presence;
- schema analyzer validates source-dependent column and type compatibility;
- lowerer performs only defensive decode.

## Parameterized Catalog creation

Static creation cannot supply schema-independent valid defaults. Extend the
creation descriptor:

```rust
enum NodeCreationDescriptor {
    Static {
        node_type_id: NodeTypeId,
    },
    ParameterizedStatic {
        node_type_id: NodeTypeId,
        required_parameters: Box<[ParameterKey]>,
    },
    ResourceBound { /* unchanged */ },
}
```

`ParameterizedStatic` creates a document node with missing required parameters.
That is a legal editable document state but compile-blocking. The backend does
not invent placeholder columns or predicates.

The descriptor remains Rust-issued and strict-wire serialized. Frontend drag,
drop, and quick-create routes forward the exact descriptor without inference. On mutation, the backend derives the authoritative descriptor from the frozen Registry/Catalog and requires exact node kind and required-parameter keys; omitted, extra, duplicate, cross-node, or forged ParameterizedStatic descriptors have zero effects.

## Rust-owned editor projection

Once source schema is available, Rust projects schema-aware editor DTOs:

```ts
type ProjectColumnsEditorDto = {
  kind: 'projectColumns'
  options: Array<{ name: string; dataType: DataTypeDto }>
  value: string[]
}

type FilterPredicateEditorDto = {
  kind: 'filterPredicate'
  columns: Array<{
    name: string
    dataType: DataTypeDto
    operators: FilterOperatorDto[]
  }>
  value: FilterPredicateDto | null
}
```

Without source schema, the DTO contains no fabricated options and marks the
editor unavailable with a localized “connect DataFrame input” reason.

Frontend uses shadcn controls and an application workflow that sends one atomic
parameter mutation. Views do not call `invoke`; frontend does not implement
type/operator compatibility.

## Typed schema fields

Replace name-only schema fields with typed fields:

```rust
struct SchemaField {
    name: SchemaColumnRef,
    scalar_type: RelationalScalarType,
}

enum RelationalScalarType {
    Boolean,
    Int64,
    Float64,
    String,
    Date,
    DateTime,
    Unknown,
}
```

Normalize database dtype strings in one Rust helper:

```text
BOOLEAN → Boolean
TINYINT/SMALLINT/INTEGER/BIGINT/INT64 → Int64
FLOAT/DOUBLE/REAL/FLOAT64 → Float64
VARCHAR/TEXT/STRING → String
DATE → Date
TIMESTAMP/DATETIME → DateTime
other → Unknown
```

Project, Filter, and Rename preserve typed fields. Unknown fields remain visible but cannot participate in typed comparison. Resolved typed schema facts are published in `AnalysisSnapshot` and `ValidatedSemanticGraph` as an authoritative map keyed by stable port address; editor projection consumes this map rather than attempting to reconstruct dtypes from SchemaExpr. Analysis serialization/fingerprint and projection-delta tests include the typed facts.

Project reuses existing `ColumnSelectionExpr::FromParameter(ParameterKey)`.
Filter becomes:

```rust
SchemaExpr::Filter {
    input: Box<SchemaExpr>,
    predicate: ParameterKey,
}
```

Registry validation requires the parameter key and exact nominal type.

Missing source schema emits only a source-port/dependency diagnostic. Project
and Filter parameter diagnostics point to their exact parameter key.

## Comparison matrix

Add:

```rust
RelationalLiteral::Decimal(CanonicalDecimal)
```

Supported compile-time combinations:

| Column | Literal | Operators |
|---|---|---|
| Boolean | Boolean | equal, notEqual |
| Int64 | Integer | all comparisons, native i64 |
| Float64 | Integer | all comparisons after exact checked f64 conversion |
| Float64 | Decimal | all comparisons after finite f64 conversion |
| String | String | all comparisons |
| Date/DateTime | none | isNull, isNotNull only |
| Unknown | any | none |

Large integers must not travel through the legacy blanket i64→f64 comparison
path. Decimal physical columns are not introduced; canonical decimals are
stable Float64 literals only.

## Diagnostics

Compile diagnostics use stable codes:

```text
compiler.schema.project_empty
compiler.schema.project_field_duplicate
compiler.schema.project_field_missing
compiler.relational.filter_column_missing
compiler.relational.filter_operator_invalid
compiler.relational.filter_literal_missing
compiler.relational.filter_literal_forbidden
compiler.relational.filter_literal_type
compiler.relational.planning_failed
```

Schema analysis gains parameter-aware and port-aware issue helpers.
Compile-blocking diagnostics prevent plan publication.

Runtime relational errors gain a stable code:

```rust
enum RelationalErrorCode {
    OperatorInvalid,
    ColumnMissing,
    TypeMismatch,
    InputShapeInvalid,
    HintInvalid,
    Cancelled,
}
```

Messages are sanitized and contain no row values, SQL, or internal paths. Codes propagate through relational execution, RunError, terminal run events/results, and ProjectState/command error mapping without flattening to an unstructured string; cancellation retains its existing typed cancellation classification.

## Lowering

Project lowers to Input plus ordered direct-column Project. Filter lowers to
Input plus a structured Filter predicate. `isNotNull` lowers as
`Not(IsNull(Column))`.

Lowerers consume validated compiled parameters and do not inspect labels or
resolve schema.

## Island compilation and determinism

Source → Filter Rows → Project DataFrame → Rename → Limit compiles to:

- one backend subplan;
- one Source;
- no intermediate bridge;
- continuous local operator indices;
- one selected final root for final-only demand.

Planner tests vary fragment registration order directly. Production determinism
uses semantically equivalent graphs with different UUID sort order and compares
normalized operator structure, results, resource traces, and operation counts.
It does not treat reversing insertion into a BTreeMap as evidence.

## Structured metadata-only hints

Add:

```rust
RelationalPushdownHint::Predicate {
    source: RelationalOperatorIndex,
    predicate: RelationalExpression,
}
```

Projection and Predicate hints are metadata-only in this slice. Existing Limit hints remain an operational, result-preserving source-scan optimization. Removing any hints preserves result values, although removing Limit may change scan counts/performance. Source scan APIs gain no projection/predicate execution in this slice.

Backend validates the exact inferred hint vector against the compiled operator
tree before source scan. Forged/stale hints fail with `HintInvalid`.

Lineage rules:

- direct Project columns are projection lineage;
- predicate-referenced columns remain required even when Project omits them;
- Rename rewrites downstream names but does not leak stale names upstream;
- hints do not cross bridges, Union, unsupported expressions, or backend
  boundaries;
- multiple roots union their required lineage deterministically.

## DataFrame runtime

Create focused helpers:

```rust
fn project_dataframe(DataFrame, projections) -> Result<DataFrame, RelationalError>;
fn filter_dataframe(DataFrame, predicate) -> Result<DataFrame, RelationalError>;
fn rename_dataframe(DataFrame, renames) -> Result<DataFrame, RelationalError>;
```

They preserve row order, selected column order, dtype, null semantics, and
source immutability. Project rejects missing/duplicate/derived expressions
defensively. Filter ordinary comparisons do not retain null rows; IsNull and
IsNotNull have explicit null semantics.

### Island ingress

A relational Input may carry RuntimeValue across a bridge. Convert tabular input
to DataFrame exactly once at island ingress:

```rust
fn tabular_runtime_to_dataframe(RuntimeValue) -> Result<DataFrame, RelationalError>;
```

Accept exactly `RuntimeValue::Scalar(Value::Object(columns))` when every value is an equal-length column list, and an Artifact containing exactly one such normalized DataFrame object. Reject non-tabular scalars, empty or multi-value artifacts, uncollected streams, unequal column lengths, and unsupported shapes. Add bridge-to-Project/Filter/Rename tests.

### Cancellation and materialization

Check cancellation around predicate evaluation and result conversion. Add a
test-only ProjectState backend/checkpoint factory so production tests can pause
without sleeps.

Project, Filter, Rename, and Limit remain DataFrames internally. Conversion to
RuntimeValue occurs only at an explicit bridge/result boundary.

## External result boundary

This slice does not change RuntimeValue/IPC tabular transport. A test-only
backend observer immediately before result conversion captures internal:

- column order;
- dtype;
- null counts;
- row count and order.

External results assert only representable values; they do not claim dtype or
column-order preservation. Typed tabular transport remains Phase 8 work.

## Demand and results

The full-chain production run uses:

```rust
ExecutionDemand::Outputs {
    outputs: [final_limit_output],
    include_default_results: false,
}
```

Only the final root is materialized/published. Separate stable GraphOutputRef
requests for Filter and Project prove prefix retention and suffix pruning.

## Existing-node boundaries

- `yssbi.dataframe.filter` remains native DataFrame + Boolean Series filtering.
- `yssbi.dataframe.decompose` remains dynamic Series output.
- Existing Source→Rename→Limit remains one relational island.
- No compatibility alias maps existing nodes to the new stable IDs.

## Verification matrix

### Protocol, persistence, Catalog, and frontend

- nominal type registrations and strict GraphDocument/editor mutation shapes;
- integer/decimal tagged strict wire;
- ParameterizedStatic descriptor strict serde and exact forwarding;
- Catalog localization/search/docs;
- schema-aware Project multi-select and Filter structured editor;
- one atomic application parameter mutation;
- unavailable editor without source schema;
- existing Filter/Decompose unchanged.

### Schema and lowering

- typed dtype normalization;
- Project reorder/empty/duplicate/missing;
- Filter column/operator/value/type matrix;
- Rename-aware identity and exact locations;
- exact Project/Filter fragments and decimal IR serialization.

### Planner

- one full chain island, zero intermediate bridges;
- fragment registration-order determinism;
- metadata-only Projection/Predicate hints and operational result-preserving Limit hints; hint removal preserves values but may change Limit scan counts;
- dependency columns and Rename handling;
- pure plan forged-hint rejection;
- final and intermediate demand specialization.

### Runtime

- DataFrame Project/Filter/Rename types/order/nulls;
- native integer and checked Float64 comparisons;
- ingress bridge conversion and invalid shapes;
- runtime stable codes;
- forged hints rejected before scan;
- cancellation/no result/resource cleanup;
- only selected roots materialize.

### Production

- real built-in Registry and database resource chain;
- final-only output demand and exact values;
- internal observer dtypes/order/nulls;
- stable Filter/Project previews;
- UUID-order determinism;
- no run/resource leak;
- Resource Catalog, History, structured control, and RunRegistry regressions.

Run Rust tests serially with `CARGO_BUILD_JOBS=1` and `--test-threads=1`.
Because this slice changes frontend and Rust contracts, run focused frontend
tests, `pnpm typecheck`, Rust check/fmt, `git diff --check`, and final
`pnpm verify` before delivery.

## Completion criteria

- Both nodes are Catalog-creatable and UI-configurable through Rust-owned DTOs.
- Malformed persisted/editor parameter shapes are rejected by strict Rust
  codecs.
- Typed schema validation blocks invalid plans before lowering.
- Source→Filter→Project→Rename→Limit is one DataFrame-native island with no
  intermediate materialization under final-only demand.
- Hints are structured, deterministic, and validated; Projection/Predicate are metadata-only while existing Limit remains an operational result-preserving scan optimization.
- Existing external-mask Filter and Decompose remain unchanged.
- External result transport remains explicitly unchanged and unclaimed.
- Independent review has no Critical/Important finding and fresh verification
  passes.
- `TODO.md` Phase 7 advances from 95% to 100% only after final review.
