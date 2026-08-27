pub mod column_distribution;
pub mod column_stats;
pub mod error;
pub mod runtime;
pub mod session_api;

pub mod database_instance;
pub mod database_state;
pub mod project_storage;
pub mod schema_snapshot;

pub mod dataset_overview;
pub mod duckdb_analytics;
pub mod duckdb_column_snapshot;
pub mod duckdb_editing;
pub mod duckdb_reader; // 类型映射见 duckdb_reader 与 database/README.md
pub mod duckdb_sql;
pub mod edit_operation;
pub mod excel_reader;
pub mod export;
pub mod sql_reader;
pub mod sqlite_reader;
pub mod tabular_io;

pub use column_distribution::*;
pub use column_stats::*;

pub use database_instance::*;
pub use database_state::*;
pub use project_storage::*;

pub use dataset_overview::*;
pub use duckdb_analytics::*;
pub use duckdb_column_snapshot::*;
pub use duckdb_editing::*;
pub use duckdb_reader::*;
pub use duckdb_sql::*;
pub use edit_operation::*;
pub use export::*;

#[cfg(test)]
mod foundation_tests;
