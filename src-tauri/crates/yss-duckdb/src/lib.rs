//! DuckDB table storage, engine primitives, physical profiling, and export.

mod export;
mod profile;
mod sql;
mod table;

pub use export::{DuckDbExportError, DuckDbExportPhase, export_duckdb_table};
pub use profile::{
    DatasetProfileColumnRef, compute_all_column_distributions, compute_all_column_stats,
    compute_dataset_overview,
};
pub use sql::{
    DUCKDB_ROWID_SQL, duckdb_table_sql, editable_dtype_to_duckdb_sql, quote_duckdb_identifier,
    quote_duckdb_string_literal,
};
pub use table::{
    DEFAULT_DUCKDB_TABLE, DUCKDB_ROWID_COL, DuckDbColumnMeta, DuckDbTableMeta, INGEST_CHUNK_ROWS,
    PageQueryResult, YSSBI_ENUM_PREFIX, YSSBI_META_TABLE, drop_data_table, duckdb_path_literal,
    duckdb_type_to_raw_string, ingest_csv_to_duckdb, ingest_dataframe_to_duckdb,
    ingest_excel_to_duckdb, ingest_parquet_to_duckdb, is_user_data_table, is_yssbi_enum_type,
    list_data_tables, query_columns_to_dataframe, query_page_to_dataframe, query_page_with_rowids,
    query_to_dataframe, query_to_dataframe_for_table, read_display_name, read_table_meta,
    write_display_name,
};
