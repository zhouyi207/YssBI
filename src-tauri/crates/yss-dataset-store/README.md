# Dataset store

This adapter owns the dataset catalog and immutable data generations used by production Project
activation, DataView editing, and relation sources. Validation and scope are recorded in the
[migration acceptance](../../../docs/reviews/2026-09-10-data-engine-migration.md).

The project layout is `database/catalog.sqlite` plus
`database/datasets/<dataset-id>/<generation-id>/part-000000.parquet`. SQLite owns the committed
dataset index, active snapshot, exact Arrow schema, file membership, data/schema revisions,
monotonic row allocation, and pending publication records. Sparse cell edits use bounded typed
Arrow batches keyed by column identity and RowId; inserted rows and tombstones are committed
with the same snapshot.
Graph/Chart documents, Project registry and Graph history keep their existing owners.

`prepare_import` reserves a new generation directory, assigns stable column/row identities,
preserves a separate display order, and writes bounded Arrow batches to Parquet parts. Parts are
closed and synced before the preparation can be committed. Nothing is entered in the dataset
index during file preparation. Dropping a preparation before attempting a catalog commit removes
only its own generated directory.

Each managed generation part is ordered by DisplayOrder, then RowId, ascending. Import assigns
monotonic order keys; compaction and casts stream that same order into their parts. This is the
ordering guarantee of `DatasetRelationInput`, including existing generations. DataFusion receives
the per-file order explicitly; file membership order alone is not a global ordering guarantee.
Small prefixes of one base file without filters or overlays use a single query partition so LIMIT
can stop the scan without parallel TopK/merge buffers. Other queries retain budgeted parallelism.
Arbitrary Parquet relations do not acquire the managed-file ordering guarantee.

The shared Parquet writer uses BYTE_STREAM_SPLIT with ZSTD for Float32/Float64 columns and
disables their Parquet dictionaries. Other column types retain their existing encoding policy;
Arrow schema metadata and category identities remain unchanged.

User columns are nullable because DataView admits blank cells and blank inserted rows. This is
an explicit editable-dataset policy; integer width/sign, decimal precision/scale, time units,
time zones, column identities, and category domains remain exact. Internal RowId and DisplayOrder
columns stay non-null. Row IDs increase monotonically; insertion creates an independent order
key between adjacent rows. Renaming a column changes its label and keeps its identity.

Imported string dictionaries use managed Int32 keys. Existing category domains remain metadata;
when an external Arrow/Parquet source has no domain metadata, import collects its actual dictionary
labels under a byte budget. Keys can differ across batches without changing category identity.
An ordered source with incompatible dictionary orders is rejected rather than reordered silently.

`DatasetSnapshot::query` composes the effective view in native DataFusion plans. Base/inserted
batches align by column identity and row tombstones use an anti join. Column edits divide this
view into unchanged and changed RowIds with native anti/semi joins; only the changed branch
passes through the patch joins. An explicit RowId range also permits pruning the changed source
without expanding the delta into a large literal list. Patch presence tests make a null edit
replace the old value. Native UNION allows filters to reach unchanged base values while filters
on edited columns apply after their patches. Both branches retain inserted rows, deletions and
exact column identities; limits and display ordering apply to the combined effective view.
Relation projections carry the exact source Arrow schema and reattach it to streamed batches;
native CASE/projection optimization must not erase column identity or category metadata.
Profiles use native aggregate/top-group queries over the same snapshot and return the existing
`yss-dataset-profile` DTOs. They exclude non-finite values from numeric summaries, count nulls
separately, order tied categories deterministically, and handle empty tables.

Compaction and casts stream the effective view into new immutable generations. A cast retains
the original generation for undo instead of attempting to cast changed values back. When an edit
would exceed the sparse-delta limit, preparation materializes that edit into a new generation
and publishes it in one catalog transaction. `prepare_restore` publishes chosen immutable
content as a new head with monotonic revisions and row allocation. Dataset deletion publishes a
tombstone; existing snapshots remain readable. Runtime undo/redo and publication admission remain
with DatabaseRuntime and Project, respectively.

`commit` checks the expected head and operation identity in an immediate SQLite transaction, then
publishes the snapshot, its files and the publication handoff together. Once commit has been
attempted, a failed or interrupted caller retains the files: an uncertain commit outcome must be
resolved from the catalog, not by deleting data that may have committed. An unacknowledged
publication remains discoverable after reopening. Project must acknowledge it only after its
runtime/index publication succeeds.

`open` verifies the catalog application ID and version through a read-only connection before
opening it for normal use. It reads only committed catalog entries, never discovers datasets by
scanning Parquet directories, and never imports an old DuckDB project implicitly. Snapshot handles
retain the immutable generation while queries and Results use it. `collect_garbage` protects
active heads, pending publication handoffs, live snapshot handles and prepared restores.
Store handles for the same canonical root share the host's lease registry. The collector queues
unreferenced files durably before physical removal and retries interrupted removals. It also
cleans abandoned generated parts while retaining live preparations. Directory scanning never
enrolls a dataset; only the catalog determines visibility.

The adapter uses the repository's SQLx/SQLite and tabular I/O implementations. Public synchronous
methods use the existing blocking workers. Calls made inside a Tokio lifecycle task use a scoped
thread so the private SQL runtime never nests inside the caller's runtime. Idle SQLite handles
are closed after each catalog operation to permit Windows project moves and deletion.
Catalog and generation paths reject symbolic links and Windows reparse points before reads,
writes and cleanup. SQL operations are parameterized, and no DataFusion or Polars plan enters
the catalog. Arrow and Parquet versions come from the workspace manifest.
