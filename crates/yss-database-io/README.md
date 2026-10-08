# Database I/O

> Status: Current
> Scope: Arrow IPC/Parquet files, bounded CSV decoding and XLSX input
> Canonical owners: [file I/O](src/lib.rs), [CSV reader](src/csv.rs), [Excel conversion](src/excel.rs)
> Update when: Codec, schema, resource limits, calendar conversion or error phases change

This crate supplies synchronous codecs over files and Arrow record batches. Callers own
resource authorization, snapshots, cancellation, staging paths and final publication.
Dataset persistence belongs to [Dataset Store](../yss-database-store/README.md);
Application owns atomic destination replacement. Database CSV exports use the existing
[Runtime export](../yss-database-runtime/src/database_instance.rs), including headers for
empty relations. This crate does not maintain a second batch CSV export entry.

`read_csv_batches` infers a schema from a byte-bounded sample, rewinds the same file and
decodes bounded batches. Quoted multiline records remain intact; oversized records or
batches fail and terminate the reader. Row limits control batching, while the CSV byte
limits are owned by `csv.rs`. Timestamp cells are decoded as text and converted through
[Database Arrow](../yss-database-arrow/README.md) before input offsets can alter their
calendar or wall-clock fields.

IPC readers accept optional column projection. Parquet readers retain complete file
schema metadata on the reader and every batch, including projected fields in file order.
The opened-file entry retains a caller's previously validated file handle. IPC writes
normalize category dictionaries through Database Arrow and use the current Julia reader's
required eight-byte message alignment. Parquet writes preserve schema metadata and use
the same `ParquetBatchWriter` for paths and caller-owned file reservations. That writer
controls row-group size and flushing; callers remain responsible for input batch bounds.
Successful IPC and Parquet writers sync content; they do not publish business state.

Excel input uses Calamine's XLSX reader. Sheet conversion writes a staging CSV while
preserving quoted text, 1900/1904 calendar components and millisecond midnight carry.
Elapsed durations retain serial days. Calendar and Arrow conversion contracts remain
with their existing owners; no project-format migration is performed here.

`TabularIoError` exposes operation, format and phase while retaining its source.
`ExcelIoError` exposes the workbook/conversion phase. Application maps those failures to
its own stable error contract rather than reading or logging input cells.

Focused validation from the repository root:

```sh
cargo test -p yss-database-io --lib
cargo clippy -p yss-database-io --lib --tests --no-deps -- -D warnings
cargo fmt -p yss-database-io --check
```

Changes to reader, schema or writer contracts also assess Store, Runtime, Engine,
Application imports and plugin consumers before selecting affected checks.
