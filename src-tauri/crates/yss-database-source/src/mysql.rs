use arrow::array::*;
use futures_util::StreamExt;
use sqlx::mysql::{MySqlConnectOptions, MySqlRow};
use sqlx::{AssertSqlSafe, ConnectOptions, Executor, Row, SqlSafeStr, Statement, Value, ValueRef};

use crate::batch::{BatchBuilder, ColumnKind, ColumnSpec, SqlSourceError, raw_column_metadata};
use crate::reader::BatchSender;
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

pub(crate) async fn read_table(
    connection_string: &str,
    charset: &str,
    table: &str,
    output: &BatchSender,
) -> Result<(), SqlSourceError> {
    let mut connection = connect(connection_string, charset).await?;
    let sql = format!("SELECT * FROM {}", quote_identifier(table));
    let statement = connection
        .prepare(AssertSqlSafe(sql).into_sql_str())
        .await
        .map_err(|source| SqlSourceError::query(ENGINE, "prepare table read", source))?;
    let columns = column_specs(raw_column_metadata::<sqlx::MySql>(statement.columns()))?;
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

fn decode_value(
    row: &MySqlRow,
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
        ColumnKind::Int8 => append_value!(
            builder,
            Int8Builder,
            value.try_decode::<i8>().map_err(error),
            is_null,
            |_: &i8| std::mem::size_of::<i8>() + 1
        ),
        ColumnKind::Int16 => append_value!(
            builder,
            Int16Builder,
            value.try_decode::<i16>().map_err(error),
            is_null,
            |_: &i16| std::mem::size_of::<i16>() + 1
        ),
        ColumnKind::Int32 => append_value!(
            builder,
            Int32Builder,
            value.try_decode::<i32>().map_err(error),
            is_null,
            |_: &i32| std::mem::size_of::<i32>() + 1
        ),
        ColumnKind::Int64 => append_value!(
            builder,
            Int64Builder,
            value.try_decode::<i64>().map_err(error),
            is_null,
            |_: &i64| std::mem::size_of::<i64>() + 1
        ),
        ColumnKind::UInt8 => append_value!(
            builder,
            UInt8Builder,
            value.try_decode::<u8>().map_err(error),
            is_null,
            |_: &u8| std::mem::size_of::<u8>() + 1
        ),
        ColumnKind::UInt16 => append_value!(
            builder,
            UInt16Builder,
            value.try_decode::<u16>().map_err(error),
            is_null,
            |_: &u16| std::mem::size_of::<u16>() + 1
        ),
        ColumnKind::UInt32 => append_value!(
            builder,
            UInt32Builder,
            value.try_decode::<u32>().map_err(error),
            is_null,
            |_: &u32| std::mem::size_of::<u32>() + 1
        ),
        ColumnKind::UInt64 => append_value!(
            builder,
            UInt64Builder,
            value.try_decode::<u64>().map_err(error),
            is_null,
            |_: &u64| std::mem::size_of::<u64>() + 1
        ),
        ColumnKind::Float32 => append_value!(
            builder,
            Float32Builder,
            value.try_decode::<f32>().map_err(error),
            is_null,
            |_: &f32| std::mem::size_of::<f32>() + 1
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
            value.try_decode_unchecked::<String>().map_err(error),
            is_null,
            |value: &String| value.len().saturating_add(8)
        ),
        ColumnKind::Binary => append_value!(
            builder,
            BinaryBuilder,
            value.try_decode_unchecked::<Vec<u8>>().map_err(error),
            is_null,
            |value: &Vec<u8>| value.len().saturating_add(8)
        ),
    }
}

#[cfg(test)]
pub(crate) fn quote_identifier_for_test(name: &str) -> String {
    quote_identifier(name)
}
