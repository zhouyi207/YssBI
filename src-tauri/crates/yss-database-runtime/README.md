# Database runtime

This crate owns session-scoped dataset handles, read admission, runtime revisions, edit history,
and the storage/publication handoff. `yss-dataset-store` owns committed SQLite catalog state and
immutable Parquet generations; `yss-datafusion` owns native relational execution. Project owns
its declaration/index publication, and Application coordinates the existing owners.

## Storage and activation

Project format 5 stores `database/catalog.sqlite` and
`database/datasets/<dataset-id>/<generation-id>/part-000000.parquet`. Activation validates the
manifest before opening the catalog and rebuilds declarations only from committed entries.
Format 4 is rejected without rewriting its files. A declaration uses `Dataset {}` storage;
CSV, Parquet, Excel, and external SQL remain separate `DatabaseImportSource` variants.

A runtime instance contains `Dataset { snapshot, engine, history }` or `Failed`. Snapshots keep
exact Arrow types, column identities, category metadata, stable RowId and independent
DisplayOrder. All host queries share a DataFusion RuntimeEnv. There is no mutable resident
DataFrame or host Polars conversion path in this runtime.

## Queries and display

Column reads apply projection and bounds before materialization. DataView pages include stable
row IDs; relation reads hide internal identity/order fields. Display DTOs convert unsafe
JavaScript integers to decimal strings. `DatabaseColumnFact` carries a semantic Graph type and
an independent exact display label; neither reconstructs the stored Arrow schema.

Profile queries aggregate the fixed effective snapshot in DataFusion. Numeric summaries ignore
non-finite values while reporting nulls separately; category ties sort deterministically and
empty tables return empty summaries. Plot preparation reads Arrow directly and keeps the existing
day/microsecond coordinate convention. Plugin snapshots use exact Arrow IPC batches.

A relation captures the actual dataset snapshot and the Project grant revision. Graph source,
projection, filter, and series kernels retain that handle. Native optimizer rewrites can drop
field metadata, so the adapter retains the source Arrow schema, validates native names/types,
and restores metadata at the batch boundary. GraphSemanticSnapshot remains the sole semantic
Graph authority.

## Editing and publication

The existing generic EditHistory owns before/after snapshot references. Cell edits, inserts,
deletes, column operations and undo/redo prepare new immutable views. Row IDs are never reused;
order keys determine display position. Edit targets parse directly into Arrow types, independently
of Graph's coarse vocabulary. Casts preserve the original generation for exact undo.

The handoff prepares Project authority and storage work, registers the runtime transition,
revalidates the captured session, commits the catalog using the expected head, installs the
snapshot/history, and publishes Project facts. A tracked storage handoff blocks reads while
its snapshot and runtime revision are being reconciled. A schema-changing undo also advances
the schema revision.

Before catalog commit, dropping preparation removes only its uncommitted files. After commit,
Project publication failure retains the committed data and durable handoff. Runtime recovery
consults the exact SQLite operation record to distinguish committed data from an abandoned
preparation; it does not undo a durable edit. Project activation publishes the rebuilt index
and acknowledges recovered handoffs.

Save checkpoints the current snapshot and clears history without rewriting the table. A sparse
delta that exceeds its budget is compacted into a new generation as part of the same edit.
Dataset deletion uses the same handoff and publishes a tombstone. Existing relation/result
snapshots retain their original contents. Garbage collection protects active heads, queries,
history and pending handoffs, then uses a durable retry queue for physical file removal.
SQLite connections are released between catalog operations so idle runtime handles do not
prevent a drained project from being moved or deleted on Windows.

## Import and export

SQLx readers decode supported source types directly into bounded Arrow builders. A bounded
channel supplies backpressure, cancellation/deadline checks cover the worker, and an explicit
end marker prevents a worker failure from looking like a successful partial import. Unsupported
SQL types fail explicitly. CSV/Parquet use the shared Arrow readers; Excel decoding retains its
existing calamine owner.

Exports stream a fixed relation into CSV or Parquet. Application retains its sibling temporary
file, currentness check, atomic destination replacement, and failure cleanup workflow. Project
Save As captures a standalone SQLite image and streams large files through the existing filesystem
transaction staging area. Source roots require leases; copied files must have absent destinations,
and source metadata is checked during staging. Project documents retain their typed validation.

The DuckDB and Polars adapter crates have been removed. Julia plugins also use DataFusion and
Arrow within their process boundary. The original host migration scope and validation are recorded in the
[migration acceptance](../../../docs/reviews/2026-09-10-data-engine-migration.md).
