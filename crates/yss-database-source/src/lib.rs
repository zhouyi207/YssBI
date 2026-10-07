//! Read-only SQLite, PostgreSQL, and MySQL table sources.
//!
//! This crate owns external SQL connection configuration, table discovery, identifier quoting,
//! strict SQLx decoding, and bounded Arrow batches. Project state and import publication
//! remain with their existing owners.

#[macro_use]
mod batch;
mod mysql;
mod postgres;
mod reader;
mod runtime;
mod sqlite;

use yss_database_contract::DatabaseEngineSql;

pub use batch::SqlSourceError;
pub use reader::SqlBatchReader;

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

/// Stream a strictly typed SQL table with backpressure and cancellation/deadline checks.
pub fn read_table_batches(
    engine: &DatabaseEngineSql,
    connection: &str,
    table: &str,
    control: yss_relational_contract::RelationControl,
) -> Result<SqlBatchReader, SqlSourceError> {
    reader::read_table(engine, connection, table, control)
}

#[cfg(test)]
mod tests;
