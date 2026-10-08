# Arrow tabular adapter

> Status: Current
> Scope: Arrow scalar conversion, semantic validation, physical edits, calendar normalization and schema metadata
> Canonical owners: This crate's source; shared scalar/semantic contracts belong to yss-data-contract
> Update when: Public conversion entry points, metadata or precision rules change

`RecordBatch` and its exact schema carry stored and computed columns. `array_to_scalars`
projects a bounded array to the existing `TabularScalar` contract: signed and unsigned
integers retain their carriers and full width, and finite floats retain numeric values.
Non-finite floats retain the page null representation. Decimal, calendar and category
values retain their exact text; calendar normalization preserves wall-clock fields.
JSON consumers apply the shared scalar display encoding at their output boundary.

`json_to_array` admits edited values against an explicit physical field; `to_record_batch`
materializes ordered document literals. Neither projection replaces the source schema.
Semantic conversion, category domains and schema metadata reuse the shared contracts.
Database lifecycle and query admission belong to [Database Runtime](../yss-database-runtime/README.md).

Run focused checks from the repository root:

```sh
cargo test -p yss-database-arrow --lib
cargo clippy -p yss-database-arrow --lib --tests --no-deps -- -D warnings
cargo fmt -p yss-database-arrow -- --check
```
