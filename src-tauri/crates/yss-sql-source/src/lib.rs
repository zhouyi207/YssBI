//! Read-only SQLite, PostgreSQL, and MySQL table sources.
//!
//! This crate owns external SQL connection configuration, table discovery, identifier quoting,
//! strict SQLx value decoding, and Polars materialization. It never owns project state or import
//! publication; callers decide how the returned [`polars::prelude::DataFrame`] is persisted.

mod dataframe;
mod mysql;
mod postgres;
mod runtime;
mod sqlite;

use polars::prelude::DataFrame;
use yss_database_contract::DatabaseEngineSql;

pub use dataframe::SqlSourceError;

/// List user tables exposed by an external SQL source.
pub fn list_tables(
    engine: &DatabaseEngineSql,
    connection: &str,
) -> Result<Vec<String>, SqlSourceError> {
    match engine {
        DatabaseEngineSql::Sqlite { auto_create } => sqlite::list_tables(connection, *auto_create),
        DatabaseEngineSql::Postgres { ssl } => postgres::list_tables(connection, *ssl),
        DatabaseEngineSql::Mysql { charset } => mysql::list_tables(connection, charset),
    }
}

/// Materialize one external SQL table into a typed Polars DataFrame.
pub fn read_table_to_dataframe(
    engine: &DatabaseEngineSql,
    connection: &str,
    table: &str,
) -> Result<DataFrame, SqlSourceError> {
    match engine {
        DatabaseEngineSql::Sqlite { auto_create } => {
            sqlite::read_table(connection, *auto_create, table)
        }
        DatabaseEngineSql::Postgres { ssl } => postgres::read_table(connection, *ssl, table),
        DatabaseEngineSql::Mysql { charset } => mysql::read_table(connection, charset, table),
    }
}

#[cfg(test)]
mod tests;
