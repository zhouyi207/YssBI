# Relational execution engine

> Status: Current
> Scope: DataFusion plans, controlled Arrow execution, relational/series operations and dataset queries
> Canonical owners: This crate owns query plans and execution; yss-relational-contract owns neutral handles and control
> Update when: Query entry points, row domains, snapshot lifetime or execution limits change

`DataFusionRuntime` implements the shared `RelationFactory` and owns the query memory/spill
environment. Dataset queries read catalog-committed immutable batches; relation and series
plans preserve their captured bindings, source leases, schema metadata and row domains.
Runtime session publication belongs to [Database Runtime](../yss-database-runtime/README.md).

`page` resolves the selected relation, applies offset and a one-row continuation probe, and
streams a bounded `RelationPage`. Arrow conversion uses the existing `TabularScalar` carriers,
preserving signed/unsigned integers at full width. The page limit accounts for scalar containers
and owned text bytes as well as each source batch. Stream waits use the shared cancellation and
deadline control. JSON encoding belongs to the consumer's output boundary.

Exact Arrow snapshots, aggregate/profile queries and numeric materialization retain their
separate entry points. Display pages do not replace the committed or computed Arrow schema.

Run focused checks from the repository root:

```sh
cargo test -p yss-database-engine --lib
cargo clippy -p yss-database-engine --lib --tests --no-deps -- -D warnings
cargo fmt -p yss-database-engine -- --check
```
