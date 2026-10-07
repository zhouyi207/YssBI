use std::path::PathBuf;

use arrow::array::*;
use futures_util::StreamExt;
use sqlx::sqlite::{SqliteConnectOptions, SqliteRow};
use sqlx::{AssertSqlSafe, ConnectOptions, Executor, Row, SqlSafeStr, Statement, Value, ValueRef};

use crate::batch::{BatchBuilder, ColumnKind, ColumnSpec, SqlSourceError, raw_column_metadata};
use crate::reader::BatchSender;
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

pub(crate) async fn read_table(
    database_path: &str,
    auto_create: bool,
    table: &str,
    output: &BatchSender,
) -> Result<(), SqlSourceError> {
    let mut connection = connect(PathBuf::from(database_path), auto_create).await?;
    let sql = format!("SELECT * FROM {}", quote_identifier(table));
    let statement = connection
        .prepare(AssertSqlSafe(sql).into_sql_str())
        .await
        .map_err(|source| SqlSourceError::query(ENGINE, "prepare table read", source))?;
    let columns = column_specs(raw_column_metadata::<sqlx::Sqlite>(statement.columns()))?;
    let mut builder = BatchBuilder::new(columns);
    output.schema(builder.schema.clone()).await?;
    let mut rows = statement.query().fetch(&mut connection);
    while let Some(row) = rows.next().await {
        let row = row.map_err(|source| SqlSourceError::query(ENGINE, "read table", source))?;
        builder.append_row(&row, decode_value)?;
        if builder.bytes > output.max_bytes() {
            return Err(yss_relational_contract::RelationError::MemoryLimitExceeded.into());
        }
        if builder.rows >= 50_000 || builder.bytes >= output.max_bytes() / 2 {
            output.batch(builder.finish()?).await?;
        }
    }
    if builder.rows > 0 {
        output.batch(builder.finish()?).await?;
    }
    Ok(())
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

fn decode_value(
    row: &SqliteRow,
    index: usize,
    column: &ColumnSpec,
    builder: &mut dyn ArrayBuilder,
) -> Result<usize, SqlSourceError> {
    let error = |source| SqlSourceError::decode(ENGINE, column, source);
    let value_ref = row.try_get_raw(index).map_err(error)?;
    let is_null = value_ref.is_null();
    let value = ValueRef::to_owned(&value_ref);
    match column.kind {
        ColumnKind::Boolean => append_value!(
            builder,
            BooleanBuilder,
            value.try_decode::<bool>().map_err(error),
            is_null,
            |_: &bool| std::mem::size_of::<bool>() + 1
        ),
        ColumnKind::Int64 => append_value!(
            builder,
            Int64Builder,
            value.try_decode::<i64>().map_err(error),
            is_null,
            |_: &i64| std::mem::size_of::<i64>() + 1
        ),
        ColumnKind::Float64 => append_value!(
            builder,
            Float64Builder,
            value.try_decode::<f64>().map_err(error),
            is_null,
            |_: &f64| std::mem::size_of::<f64>() + 1
        ),
        ColumnKind::String => append_value!(
            builder,
            StringBuilder,
            value.try_decode::<String>().map_err(error),
            is_null,
            |value: &String| value.len().saturating_add(8)
        ),
        ColumnKind::Binary => append_value!(
            builder,
            BinaryBuilder,
            value.try_decode::<Vec<u8>>().map_err(error),
            is_null,
            |value: &Vec<u8>| value.len().saturating_add(8)
        ),
        _ => Err(SqlSourceError::unsupported(
            ENGINE,
            &column.name,
            &column.source_type,
        )),
    }
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
