use polars::prelude::AnyValue;
use sqlx::mysql::{MySqlConnectOptions, MySqlRow};
use sqlx::{AssertSqlSafe, ConnectOptions, Executor, Row, SqlSafeStr, Statement, Value, ValueRef};

use crate::dataframe::{
    ColumnKind, ColumnSpec, SqlSourceError, build_dataframe, empty_column_data, raw_column_metadata,
};
use crate::runtime;

const ENGINE: &str = "MySQL";

pub(crate) fn list_tables(
    connection_string: &str,
    charset: &str,
) -> Result<Vec<String>, SqlSourceError> {
    let connection_string = connection_string.to_string();
    let charset = charset.to_string();
    runtime::run(async move {
        let mut connection = connect(&connection_string, &charset).await?;
        sqlx::query_scalar::<_, String>(
            "SELECT table_name FROM information_schema.tables \
             WHERE table_schema = DATABASE() ORDER BY table_name",
        )
        .fetch_all(&mut connection)
        .await
        .map_err(|source| SqlSourceError::query(ENGINE, "list tables", source))
    })
}

pub(crate) fn read_table(
    connection_string: &str,
    charset: &str,
    table: &str,
) -> Result<polars::prelude::DataFrame, SqlSourceError> {
    let connection_string = connection_string.to_string();
    let charset = charset.to_string();
    let table = table.to_string();
    runtime::run(async move {
        let mut connection = connect(&connection_string, &charset).await?;
        let sql = format!("SELECT * FROM {}", quote_identifier(&table));
        let statement = connection
            .prepare(AssertSqlSafe(sql).into_sql_str())
            .await
            .map_err(|source| SqlSourceError::query(ENGINE, "prepare table read", source))?;
        let columns = column_specs(raw_column_metadata::<sqlx::MySql>(statement.columns()))?;
        let rows: Vec<MySqlRow> = statement
            .query()
            .fetch_all(&mut connection)
            .await
            .map_err(|source| SqlSourceError::query(ENGINE, "read table", source))?;
        rows_to_dataframe(&columns, rows)
    })
}

async fn connect(
    connection_string: &str,
    charset: &str,
) -> Result<sqlx::MySqlConnection, SqlSourceError> {
    let options: MySqlConnectOptions = connection_string
        .parse()
        .map_err(|source| SqlSourceError::invalid_connection(ENGINE, source))?;
    options
        .charset(charset)
        .connect()
        .await
        .map_err(|source| SqlSourceError::connect(ENGINE, source))
}

fn quote_identifier(name: &str) -> String {
    format!("`{}`", name.replace('`', "``"))
}

pub(crate) fn column_specs(
    metadata: Vec<(String, String)>,
) -> Result<Vec<ColumnSpec>, SqlSourceError> {
    metadata
        .into_iter()
        .map(|(name, source_type)| {
            let kind = match source_type.to_ascii_uppercase().as_str() {
                "BOOLEAN" => ColumnKind::Boolean,
                "TINYINT" => ColumnKind::Int8,
                "SMALLINT" => ColumnKind::Int16,
                "INT" | "MEDIUMINT" => ColumnKind::Int32,
                "BIGINT" => ColumnKind::Int64,
                "TINYINT UNSIGNED" => ColumnKind::UInt8,
                "SMALLINT UNSIGNED" => ColumnKind::UInt16,
                "INT UNSIGNED" | "MEDIUMINT UNSIGNED" => ColumnKind::UInt32,
                "BIGINT UNSIGNED" => ColumnKind::UInt64,
                "FLOAT" => ColumnKind::Float32,
                "DOUBLE" => ColumnKind::Float64,
                "CHAR" | "VARCHAR" | "TINYTEXT" | "TEXT" | "MEDIUMTEXT" | "LONGTEXT" | "ENUM"
                | "SET" | "JSON" => ColumnKind::String,
                "BINARY" | "VARBINARY" | "TINYBLOB" | "BLOB" | "MEDIUMBLOB" | "LONGBLOB"
                | "BIT" => ColumnKind::Binary,
                _ => return Err(SqlSourceError::unsupported(ENGINE, name, source_type)),
            };
            Ok(ColumnSpec::new(&name, &source_type, kind))
        })
        .collect()
}

fn rows_to_dataframe(
    columns: &[ColumnSpec],
    rows: Vec<MySqlRow>,
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
    row: &MySqlRow,
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
        ColumnKind::UInt8 => value.try_decode::<u8>().map(AnyValue::UInt8),
        ColumnKind::UInt16 => value.try_decode::<u16>().map(AnyValue::UInt16),
        ColumnKind::UInt32 => value.try_decode::<u32>().map(AnyValue::UInt32),
        ColumnKind::UInt64 => value.try_decode::<u64>().map(AnyValue::UInt64),
        ColumnKind::Float32 => value.try_decode::<f32>().map(AnyValue::Float32),
        ColumnKind::Float64 => value.try_decode::<f64>().map(AnyValue::Float64),
        // Metadata has already restricted these branches to textual and binary MySQL types.
        // SQLx intentionally excludes JSON/SET and BIT from String/Vec<u8> compatibility even
        // though their wire payloads are UTF-8 and bytes respectively, so decode deterministically
        // after the explicit source-type classification instead of attempting fallback guesses.
        ColumnKind::String => value
            .try_decode_unchecked::<String>()
            .map(|value| AnyValue::StringOwned(value.into())),
        ColumnKind::Binary => value
            .try_decode_unchecked::<Vec<u8>>()
            .map(AnyValue::BinaryOwned),
    };
    decoded.map_err(|source| SqlSourceError::decode(ENGINE, column, source))
}

#[cfg(test)]
pub(crate) fn quote_identifier_for_test(name: &str) -> String {
    quote_identifier(name)
}
