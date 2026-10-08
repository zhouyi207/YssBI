# Dataset profile values

> Status: Current
> Scope: Typed dataset overviews, column statistics/distributions and histogram labels
> Canonical owners: This crate owns read values and label formatting; Database Engine computes profiles; Runtime owns snapshot admission; Harness owns model output
> Update when: Profile fields, histogram defaults or consumer projections change

`ColumnStats` and `ColumnDistribution` use Rust enum variants to distinguish numeric
and string/category results. Counts, summaries and histogram labels are computed by
[Database Engine](../yss-database-engine/README.md) from a fixed effective dataset.
[Database Runtime](../yss-database-runtime/README.md) admits selected queries and captures
their snapshots. These values retain no mutable dataset or execution state.

The native host consumes the typed values directly. Application's existing
[Harness profile projection](../yss-application/src/automation/resources/database/read/profile.rs)
maps requested metrics into model contracts at the output boundary. This crate has
no serialization or GUI dependency.

Run focused consumer checks from the repository root:

```sh
cargo test -p yss-database-store --lib native_profiles_follow_the_snapshot_and_preserve_finite_mode_and_empty_table_semantics
cargo test -p yss-application --lib automation::resources::tests::database_reads::
cargo check -p yss-desktop-gpui --bin yss-desktop-gpui
```
