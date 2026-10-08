# SQLite project registry

> Status: Current
> Scope: SQLite persistence for project registration records
> Canonical owners: This crate owns the SQLite schema, queries and row decoding; Registry Contract owns records and the storage port
> Update when: Storage initialization, query behavior or persisted fields change

`SqliteProjectRegistryStore` implements the
[Registry storage port](../yss-project-registry-contract/README.md). It opens
`db/projects.sqlite` under the supplied application directory using SQLx native filename
options, preserving percent signs and other filename characters. The pool uses one connection
with WAL and normal synchronous mode.

The registration ID primary key and metadata path unique index serve individual reads.
Queries bind values through SQLx; catalog loading and individual reads share one row decoder.
Unknown root identity states and non-boolean favorite values return `StorageFailed`.
Upsert conflicts and SQL failures use the same typed failure; removing an absent registration
returns `Unavailable`. SQL diagnostics go to structured tracing rather than localized UI text.

[Registry workflows](../yss-project-registry/README.md) own path admission, native root checks,
listing order and cleanup. This adapter persists those records without a second project model.

```sh
cargo test -p yss-project-registry-sqlite --lib
cargo clippy -p yss-project-registry-sqlite --lib --tests --no-deps -- -D warnings
cargo fmt -p yss-project-registry-sqlite -- --check
```
