//! DuckDB engine primitives, physical dataset profiling, and table export.

mod export;
mod profile;
mod sql;

pub use export::{DuckDbExportError, DuckDbExportPhase, export_duckdb_table};
pub use profile::{
    DatasetProfileColumnRef, compute_all_column_distributions, compute_all_column_stats,
    compute_dataset_overview,
};
pub use sql::{
    DUCKDB_ROWID_SQL, duckdb_table_sql, editable_dtype_to_duckdb_sql, quote_duckdb_identifier,
    quote_duckdb_string_literal,
};
