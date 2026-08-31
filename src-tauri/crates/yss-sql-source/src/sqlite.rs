use std::path::PathBuf;

use polars::prelude::AnyValue;
use sqlx::sqlite::{SqliteConnectOptions, SqliteRow};
use sqlx::{AssertSqlSafe, ConnectOptions, Executor, Row, SqlSafeStr, Statement, Value, ValueRef};

use crate::dataframe::{
    ColumnKind, ColumnSpec, SqlSourceError, build_dataframe, empty_column_data, raw_column_metadata,
};
use crate::runtime;

const ENGINE: &str = "SQLite";

pub(crate) fn list_tables(
    database_path: &str,
    auto_create: bool,
) -> Result<Vec<String>, SqlSourceError> {
    let database_path = PathBuf::from(database_path);
    runtime::run(async move {
        let mut connection = connect(database_path, auto_create).await?;
        sqlx::query_scalar::<_, String>(
            "SELECT name FROM sqlite_master \
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .fetch_all(&mut connection)
        .await
        .map_err(|source| SqlSourceError::query(ENGINE, "list tables", source))
    })
}

pub(crate) fn read_table(
    database_path: &str,
    auto_create: bool,
    table: &str,
) -> Result<polars::prelude::DataFrame, SqlSourceError> {
    let database_path = PathBuf::from(database_path);
    let table = table.to_string();
    runtime::run(async move {
        let mut connection = connect(database_path, auto_create).await?;
        let sql = format!("SELECT * FROM {}", quote_identifier(&table));
        let statement = connection
            .prepare(AssertSqlSafe(sql).into_sql_str())
            .await
            .map_err(|source| SqlSourceError::query(ENGINE, "prepare table read", source))?;
        let columns = column_specs(raw_column_metadata::<sqlx::Sqlite>(statement.columns()))?;
        let rows: Vec<SqliteRow> = statement
            .query()
            .fetch_all(&mut connection)
            .await
            .map_err(|source| SqlSourceError::query(ENGINE, "read table", source))?;
        rows_to_dataframe(&columns, rows)
    })
}

async fn connect(
    database_path: PathBuf,
    auto_create: bool,
) -> Result<sqlx::SqliteConnection, SqlSourceError> {
    SqliteConnectOptions::new()
        .filename(database_path)
        .create_if_missing(auto_create)
        .read_only(!auto_create)
        .connect()
        .await
        .map_err(|source| SqlSourceError::connect(ENGINE, source))
}

fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

fn column_specs(metadata: Vec<(String, String)>) -> Result<Vec<ColumnSpec>, SqlSourceError> {
    metadata
        .into_iter()
        .map(|(name, source_type)| {
            let kind = match source_type.to_ascii_uppercase().as_str() {
                "BOOLEAN" => ColumnKind::Boolean,
                "INTEGER" => ColumnKind::Int64,
                "REAL" => ColumnKind::Float64,
                "TEXT" => ColumnKind::String,
                "BLOB" => ColumnKind::Binary,
                _ => return Err(SqlSourceError::unsupported(ENGINE, name, source_type)),
            };
            Ok(ColumnSpec::new(&name, &source_type, kind))
        })
        .collect()
}

fn rows_to_dataframe(
    columns: &[ColumnSpec],
    rows: Vec<SqliteRow>,
) -> Result<polars::prelude::DataFrame, SqlSourceError> {
    let mut data = empty_column_data(columns.len());
    for row in &rows {
        if row.len() != columns.len() {
            return Err(SqlSourceError::InconsistentRowShape);
        }
        for (index, (column, values)) in columns.iter().zip(&mut data).enumerate() {
            values.push(decode_value(row, index, column)?);
        }
    }
    build_dataframe(columns, data)
}

fn decode_value(
    row: &SqliteRow,
    index: usize,
    column: &ColumnSpec,
) -> Result<AnyValue<'static>, SqlSourceError> {
    let value_ref = row
        .try_get_raw(index)
        .map_err(|source| SqlSourceError::decode(ENGINE, column, source))?;
    if value_ref.is_null() {
        return Ok(AnyValue::Null);
    }
    let value = ValueRef::to_owned(&value_ref);
    let decoded = match column.kind {
        ColumnKind::Boolean => value.try_decode::<bool>().map(AnyValue::Boolean),
        ColumnKind::Int64 => value.try_decode::<i64>().map(AnyValue::Int64),
        ColumnKind::Float64 => value.try_decode::<f64>().map(AnyValue::Float64),
        ColumnKind::String => value
            .try_decode::<String>()
            .map(|value| AnyValue::StringOwned(value.into())),
        ColumnKind::Binary => value.try_decode::<Vec<u8>>().map(AnyValue::BinaryOwned),
        unsupported => {
            return Err(SqlSourceError::unsupported(
                ENGINE,
                &column.name,
                format!("{} ({unsupported:?})", column.source_type),
            ));
        }
    };
    decoded.map_err(|source| SqlSourceError::decode(ENGINE, column, source))
}

#[cfg(test)]
pub(crate) async fn execute_fixture_sql(
    database_path: PathBuf,
    sql: &'static str,
) -> Result<(), SqlSourceError> {
    let mut connection = SqliteConnectOptions::new()
        .filename(database_path)
        .create_if_missing(true)
        .connect()
        .await
        .map_err(|source| SqlSourceError::connect(ENGINE, source))?;
    sqlx::raw_sql(sql)
        .execute(&mut connection)
        .await
        .map_err(|source| SqlSourceError::query(ENGINE, "create test fixture", source))?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn quote_identifier_for_test(name: &str) -> String {
    quote_identifier(name)
}
