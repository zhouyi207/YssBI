use polars::prelude::AnyValue;
use sqlx::postgres::{PgConnectOptions, PgRow, PgSslMode};
use sqlx::{AssertSqlSafe, ConnectOptions, Executor, Row, SqlSafeStr, Statement, Value, ValueRef};

use crate::dataframe::{
    ColumnKind, ColumnSpec, SqlSourceError, build_dataframe, empty_column_data, raw_column_metadata,
};
use crate::runtime;

const ENGINE: &str = "PostgreSQL";

pub(crate) fn list_tables(
    connection_string: &str,
    ssl: bool,
) -> Result<Vec<String>, SqlSourceError> {
    let connection_string = connection_string.to_string();
    runtime::run(async move {
        let mut connection = connect(&connection_string, ssl).await?;
        sqlx::query_scalar::<_, String>(
            "SELECT tablename FROM pg_tables \
             WHERE schemaname = 'public' ORDER BY tablename",
        )
        .fetch_all(&mut connection)
        .await
        .map_err(|source| SqlSourceError::query(ENGINE, "list tables", source))
    })
}

pub(crate) fn read_table(
    connection_string: &str,
    ssl: bool,
    table: &str,
) -> Result<polars::prelude::DataFrame, SqlSourceError> {
    let connection_string = connection_string.to_string();
    let table = table.to_string();
    runtime::run(async move {
        let mut connection = connect(&connection_string, ssl).await?;
        let sql = format!("SELECT * FROM {}", quote_identifier(&table));
        let statement = connection
            .prepare(AssertSqlSafe(sql).into_sql_str())
            .await
            .map_err(|source| SqlSourceError::query(ENGINE, "prepare table read", source))?;
        let columns = column_specs(raw_column_metadata::<sqlx::Postgres>(statement.columns()))?;
        let rows: Vec<PgRow> = statement
            .query()
            .fetch_all(&mut connection)
            .await
            .map_err(|source| SqlSourceError::query(ENGINE, "read table", source))?;
        rows_to_dataframe(&columns, rows)
    })
}

async fn connect(connection_string: &str, ssl: bool) -> Result<sqlx::PgConnection, SqlSourceError> {
    let options: PgConnectOptions = connection_string
        .parse()
        .map_err(|source| SqlSourceError::invalid_connection(ENGINE, source))?;
    let options = options.ssl_mode(if ssl {
        PgSslMode::Require
    } else {
        PgSslMode::Disable
    });
    options
        .connect()
        .await
        .map_err(|source| SqlSourceError::connect(ENGINE, source))
}

fn quote_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

pub(crate) fn column_specs(
    metadata: Vec<(String, String)>,
) -> Result<Vec<ColumnSpec>, SqlSourceError> {
    metadata
        .into_iter()
        .map(|(name, source_type)| {
            let kind = match source_type.to_ascii_uppercase().as_str() {
                "BOOL" => ColumnKind::Boolean,
                "CHAR" => ColumnKind::Int8,
                "INT2" => ColumnKind::Int16,
                "INT4" => ColumnKind::Int32,
                "INT8" => ColumnKind::Int64,
                "OID" => ColumnKind::UInt32,
                "FLOAT4" => ColumnKind::Float32,
                "FLOAT8" => ColumnKind::Float64,
                "BPCHAR" | "NAME" | "TEXT" | "VARCHAR" => ColumnKind::String,
                "BYTEA" => ColumnKind::Binary,
                _ => return Err(SqlSourceError::unsupported(ENGINE, name, source_type)),
            };
            Ok(ColumnSpec::new(&name, &source_type, kind))
        })
        .collect()
}

fn rows_to_dataframe(
    columns: &[ColumnSpec],
    rows: Vec<PgRow>,
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
    row: &PgRow,
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
        ColumnKind::Int8 => value.try_decode::<i8>().map(AnyValue::Int8),
        ColumnKind::Int16 => value.try_decode::<i16>().map(AnyValue::Int16),
        ColumnKind::Int32 => value.try_decode::<i32>().map(AnyValue::Int32),
        ColumnKind::Int64 => value.try_decode::<i64>().map(AnyValue::Int64),
        ColumnKind::UInt32 => value
            .try_decode::<sqlx::postgres::types::Oid>()
            .map(|value| AnyValue::UInt32(value.0)),
        ColumnKind::Float32 => value.try_decode::<f32>().map(AnyValue::Float32),
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
pub(crate) fn quote_identifier_for_test(name: &str) -> String {
    quote_identifier(name)
}
