# Database runtime

## Crate responsibilities

| Crate                   | Responsibility                                                           |
| ----------------------- | ------------------------------------------------------------------------ |
| `yss-database-contract` | Public database identities, declarations and operation contracts         |
| `yss-database-schema`   | Database schema facts exposed to consumers                               |
| `yss-database-engine`   | SQL and relational execution using DataFusion                            |
| `yss-database-store`    | Dataset catalog, immutable snapshots, sparse patches and persistence     |
| `yss-database-source`   | Read-only external SQLite, PostgreSQL and MySQL sources                  |
| `yss-database-io`       | CSV, Parquet and Arrow IPC encoding/decoding, and Excel input            |
| `yss-database-arrow`    | Arrow value conversion, semantic validation and database schema metadata |
| `yss-database-runtime`  | Session-scoped instances, edit history and publication handoff           |

Backend-neutral scalar values and ordered table literals belong to `yss-data-contract`.
They are shared by graph documents, node kernels and database adapters. They do not represent
independently managed cells. Arrow conversion remains a shared adapter consumed by both
database operations and node kernels; the pure data contract does not depend on this adapter.

## Runtime ownership

This crate owns session-scoped dataset handles, read admission, runtime revisions, edit history,
and the storage/publication handoff. `yss-database-store` owns committed SQLite catalog state and
immutable Parquet generations; `yss-database-engine` owns native relational execution. Project owns
its declaration/index publication, and Application coordinates the existing owners.

## Storage and activation

Application opens the Dataset Store at the Project root and supplies bound dataset instances.
Session-open requests carry session identity, generation, declarations and their observations;
empty and populated sessions share the same validation and initialization path.

Project format 5 stores `database/catalog.sqlite` and
`database/datasets/<dataset-id>/<generation-id>/part-000000.parquet`. Activation validates the
manifest before opening the catalog and rebuilds declarations only from committed entries.
Format 4 is rejected without rewriting its files. A declaration uses `Dataset {}` storage;
CSV, Parquet, Excel, and external SQL remain separate `DatabaseImportSource` variants.

A runtime instance contains `Dataset { snapshot, engine, history }` or `Failed`. Snapshots keep
exact Arrow types, column identities, category metadata, stable RowId and independent
DisplayOrder. All host queries share a DataFusion RuntimeEnv. There is no mutable resident
DataFrame in this runtime.

## Queries and display

Metadata snapshots expose the Dataset Store's existing `data_revision`. Semantic-only changes preserve
this value; row edits, physical casts and column membership changes advance it. It identifies the
positional row payload for an already admitted page, while query admission still checks the full
Project/Runtime revision basis.

Column snapshots use Arrow and apply projection and bounds before materialization. DataView pages include stable
row IDs; relation reads hide internal identity/order fields. Display DTOs convert unsafe
JavaScript integers to decimal strings. `DatabaseColumnFact` carries a semantic Graph type and
an independent display label, exact Physical label, the field's Semantic configuration and supported
Semantic choices derived by the Arrow adapter from the exact Physical type;
none reconstructs the stored Arrow schema. The seven Semantic types and conversion constraints
are owned by the [dataset metadata contract](../yss-database-store/README.md#field-meaning-and-physical-conversion).

Boolean, string, date and timestamp edit targets use `Bool`, `Utf8`, `Date` and explicit
`Datetime(s/ms/us/ns)` names. The Arrow edit parser does not accept aliases (`Boolean`, `String`, `Date32`) or
infer a timestamp unit from bare `Datetime`/`DateTime`.

`DatabasePageSnapshot::into_parts` transfers the owned table and stable row IDs to a consumer
without cloning the page. Borrowed access remains available for callers that retain the snapshot.
Application revalidates the captured query basis and application session after building its result.
`DatabaseQueryBasis` captures the existing declaration observation together with runtime/schema
revisions. Revalidation checks all three facts. Its declaration revision lets Application compare
the runtime read to a caller's Project resource revision without adding a counter or a separate
observation index. Numeric column-pair reads require an expected declaration revision and check
it against the captured basis before materializing Arrow columns.
When both plot axes name the same column, the plot adapter projects and converts that column
once and shares its immutable numeric values between the axes, preserving null positions and
axis metadata. Distinct columns are read together from the same relation.

`DatabaseDeclarationObservationSet::get` exposes borrowed lookup through the contract's existing
database ID index. Runtime preparation, commit and compensation, and Application mutation setup use
that lookup directly; they do not duplicate linear-search helpers or maintain another observation map.
Revision and fingerprint comparisons remain with the existing session and mutation owners.

Profile queries aggregate the fixed effective snapshot in DataFusion. Numeric summaries ignore
non-finite values while reporting nulls separately; category ties sort deterministically and
empty tables return empty summaries. Plot preparation reads Arrow directly and keeps the existing
day/microsecond coordinate convention. Plugin snapshots use exact Arrow IPC batches.

Harness profile inspection calls `session_api::dataset_overview_with_control` with the caller's
cancellation and query budget. Chart histograms use `session_api::column_distributions` through
the Application revision-checked read boundary.

Semantic mapping initialization uses `session_api::column_values`. It projects the requested user column and reuses the engine's bounded `distinct_labels` stream over the complete effective snapshot. Arrow supplies canonical strings (including exact wide integers), nulls are omitted and values have deterministic lexical order. The query retains the existing 30-second/16 MiB budget and rejects domains above 65,536 entries rather than returning a partial mapping; it does not modify field metadata or data.

A relation captures the actual dataset snapshot and the Project grant revision. Graph source,
projection, filter, and series kernels retain that handle. Native optimizer rewrites can drop
field metadata, so the adapter retains the source Arrow schema, validates native names/types,
and restores metadata at the batch boundary. GraphSemanticSnapshot remains the sole semantic
Graph authority.

Graph data preparation uses the neutral requests in `yss-relational-contract`; native
`LogicalPlan` and `Expr` values remain in `yss-database-engine`. Projection, filtering,
sorting, deduplication, fixed-schema pivot/unpivot, resampling, encoding and grid alignment
compose native plans. Series transformations retain their row domain and expressions;
nested windows are lowered into successive native Window operators and restore source
order. Repeated differences use a bounded DataFusion window evaluator, preserving repeated
subtraction and null propagation without collecting columns in node kernels.
Numeric reductions and arithmetic windows, including sum, difference and percent change,
require Numeric field meaning as well as numeric Arrow storage. Integer Identifier fields
are rejected; integer Numeric sums retain the exact Decimal128 aggregation path.

Single imputation uses native mean, exact median, mode windows or a constant over
the full input row domain. Numeric modes break ties by smallest value. Nonempty
all-null inputs require a constant; invalid or non-finite results fail when read.
The transformed series preserves row identity and order and uses Float64 storage.

Literal series are imported once as immutable Arrow arrays and exposed through expressions
over explicit position coordinates; generated integer ranges use the same coordinates.
Equal-length literal/range series in one engine can align by those coordinates. Independent
dataset or table relations never acquire alignment from matching lengths. Row-changing
operations establish a new domain. Scalar reductions consume only native aggregate results;
data-dependent column dropping consumes one aggregate row and retains its deferred schema.
Concurrent consumers of the same deferred relation reuse the successful cached plan, including
its row domain. Cancellation and failed resolution do not populate that cache.

## Editing and publication

An edit request identifies a row, a column and a replacement value in a captured snapshot.
The store resolves the column name to its stable column identity and validates the value
against the exact Arrow field. Persistent patches contain `column_id`, `row_ids` and typed
Arrow values. Column renames preserve that identity, and row IDs are independent of display
positions. No separate cell identity or cell object lifecycle is maintained.

The private `edit_history::EditHistory` owns before/after snapshot references. Its public
projection, `EditState`, belongs to `yss-database-contract`; Application and IPC consume that
contract without accessing the history container. `DatabaseInstance` keeps its runtime state
private to this crate. Cell edits, inserts,
deletes, column operations and undo/redo prepare new immutable views. Row IDs are never reused;
order keys determine display position. Edit targets parse directly into Arrow types, independently
of Graph's semantic vocabulary. Casts preserve the original generation for exact undo. Semantic
changes validate current values, retain the physical representation, advance the schema revision,
and use the same history/publication path. Casts retain Semantic and reject precision loss.

The handoff prepares Project authority and storage work, registers the runtime transition,
revalidates the captured session, commits the catalog using the expected head, installs the
snapshot/history, and publishes Project facts. Runtime preparation requires the physical
proposal and its storage recovery record. Schema invalidation compares the proposal's Arrow
schema with its captured snapshot; the operation name does not infer a second schema effect.
Declaration observations own the current fingerprints. A storage handoff blocks reads while
its snapshot and runtime revision are being reconciled. A schema-changing undo also advances
the schema revision.
Physical handoffs retain the prepared edit and store handle, then move the committed instance
into runtime state. They do not retain extra copies of the before/after instances or histories.

Before catalog commit, dropping preparation removes only its uncommitted files. After commit,
Project publication failure retains the committed data and durable handoff. Runtime recovery
consults the exact SQLite operation record to distinguish committed data from an abandoned
preparation; it does not undo a durable edit. Project activation publishes the rebuilt index
and acknowledges recovered handoffs.
`resolve_storage_recoveries` owns recovery after session admission closes. Physical mutations
report whether a commit requires recovery; dropping an uncommitted preparation cleans its files.
Runtime compensation is allowed only when storage has neither committed nor reported an uncertain
commit. Runtime errors retain only their classification, operation and resource identity;
query, export and storage failures use the existing Dataset Store error contract internally.

Save checkpoints the current snapshot, shares its persisted patch blobs and clears history
without rewriting the table. A sparse
delta that exceeds its budget is compacted into a new generation as part of the same edit.
Dataset deletion uses the same handoff and publishes a tombstone. Existing relation/result
snapshots retain their original contents. Garbage collection protects active heads, queries,
history and pending handoffs, then uses a durable retry queue for physical file removal.
Ordinary confirmed mutations batch the collection sweep at the store's 64-mutation interval;
Save and dataset deletion collect immediately after releasing the mutation's temporary snapshot references.
SQLite connections are released between catalog operations so idle runtime handles do not
prevent a drained project from being moved or deleted on Windows.

## Import and export

SQLx readers decode supported source types directly into bounded Arrow builders. A bounded
channel supplies backpressure, cancellation/deadline checks cover the worker, and an explicit
end marker prevents a worker failure from looking like a successful partial import. Unsupported
SQL types fail explicitly. CSV/Parquet use the shared Arrow readers; Excel decoding retains its
existing calamine owner.
Excel calendar serials use Calamine's 1900/1904 conversion to timezone-free ISO text with
millisecond precision before CSV inference. Date-only and time-only cells retain the calendar
and clock components Calamine exposes; the adapter does not infer a separate time-only type.
When millisecond rounding reaches midnight, the existing workspace Chrono date operation
advances the ordinary calendar day; Calamine retains ownership of the special 1900 calendar.
Elapsed durations remain numeric serial days, and ISO date/duration cells retain their text.
Excel's fictitious 1900-02-29 also retains its calendar text under the existing CSV inference
and validation rules. Serials outside Excel's calendar range retain their numeric representation.

Exports stream a fixed relation into CSV or Parquet. Application retains its sibling temporary
file, currentness check, atomic destination replacement, and failure cleanup workflow. Project
Save As captures a standalone SQLite image and streams large files through the existing filesystem
transaction staging area. Source roots require leases; copied files must have absent destinations,
and source metadata is checked during staging. Project documents retain their typed validation.

Julia plugins also use DataFusion and Arrow within their process boundary. Storage and graph
integration contracts are described in
[Dataset store](../yss-database-store/README.md) and
[Graph and Execution](../yss-application/src/graph/README.md).
