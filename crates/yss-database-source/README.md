# External SQL sources

> Status: Current
> Scope: SQLite, PostgreSQL and MySQL table discovery and bounded Arrow reads
> Canonical owners: This crate owns external SQL adapters; Application owns import publication
> Update when: Source configuration, decoding, streaming or reader lifecycle changes

`list_tables` discovers tables using engine-specific connection settings and identifier rules.
`read_table_batches` returns a synchronous Arrow reader backed by a controlled worker. The
engine adapters own connection setup, column type admission and strict SQLx decoding;
`reader.rs` owns the shared row-to-batch loop, byte limits, backpressure and terminal messages.
`batch.rs` builds exact Arrow arrays from those decoded values. Unsupported SQL types and late
decode failures reject the read rather than silently completing a partial import.

The reader uses a two-message channel and batches at 50,000 rows or half the caller's byte
budget. Each emitted Arrow batch is checked against the full budget. Cancellation, deadline
and dropped-reader checks cover the worker and channel waits; dropping the reader joins its
worker. A synchronous call inside Tokio uses the existing scoped worker runtime.

SQLite opens existing sources read-only unless `auto_create` explicitly permits creating a
missing file. PostgreSQL SSL and MySQL charset settings remain owned by their connection
adapters. Project state and import publication belong to
[Application](../yss-application/src/database/README.md); committed dataset state belongs to
[Dataset Store](../yss-database-store/README.md).

Run focused checks from the repository root:

```sh
cargo test -p yss-database-source --lib
cargo clippy -p yss-database-source --lib --tests --no-deps -- -D warnings
cargo fmt -p yss-database-source -- --check
```
