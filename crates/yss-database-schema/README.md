# Database schema projections

> Status: Current
> Scope: Backend-neutral database column facts and typed runtime/schema revisions
> Canonical owners: This crate owns read projection types; Data Contract owns column names and semantic types; Database Runtime owns revision state
> Update when: Column facts, revision projections or consuming boundaries change

`DatabaseColumnFact` carries a validated column name, Graph value type, nullability,
display and physical labels, field meaning and supported semantic choices. These
read facts do not reconstruct the exact storage schema. The
[Arrow adapter](../yss-database-arrow/README.md) projects source fields, and
[Database Runtime](../yss-database-runtime/README.md) supplies their captured revisions.
Application and the native host consume the same projection.

Run focused checks from the repository root:

```sh
cargo check -p yss-database-schema --lib
cargo test -p yss-database-arrow --lib storage_schema_preserves_identity_exact_types_and_category_domain
cargo check -p yss-desktop-gpui --bin yss-desktop-gpui
```
