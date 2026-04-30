//! SQL 数据库读取：PostgreSQL、MySQL/MariaDB（使用 sqlx）
//! SQLite 仍由 sqlite_reader 处理

use polars::prelude::*;
use sqlx::{Column as SqlxColumn, ConnectOptions, Row, Value, ValueRef};

use super::DatabaseEngineSql;

/// 列出 PostgreSQL 数据库中的用户表（public schema）
pub fn list_postgres_tables(connection_string: &str) -> Result<Vec<String>, String> {
    tauri::async_runtime::block_on(async {
        let opts: sqlx::postgres::PgConnectOptions = connection_string
            .parse()
            .map_err(|e: sqlx::Error| format!("Invalid PostgreSQL URL: {}", e))?;
        let mut conn = opts
            .connect()
            .await
            .map_err(|e| format!("Failed to connect to PostgreSQL: {}", e))?;

        let rows: Vec<sqlx::postgres::PgRow> = sqlx::query(
            "SELECT tablename FROM pg_tables WHERE schemaname = 'public' ORDER BY tablename",
        )
        .fetch_all(&mut conn)
        .await
        .map_err(|e| format!("Failed to list tables: {}", e))?;

        let tables: Vec<String> = rows
            .iter()
            .filter_map(|r| r.try_get::<String, _>(0).ok())
            .collect();
        Ok(tables)
    })
}

/// 列出 MySQL/MariaDB 数据库中的用户表
pub fn list_mysql_tables(connection_string: &str) -> Result<Vec<String>, String> {
    tauri::async_runtime::block_on(async {
        let opts: sqlx::mysql::MySqlConnectOptions = connection_string
            .parse()
            .map_err(|e: sqlx::Error| format!("Invalid MySQL URL: {}", e))?;
        let mut conn = opts
            .connect()
            .await
            .map_err(|e| format!("Failed to connect to MySQL: {}", e))?;

        let rows: Vec<sqlx::mysql::MySqlRow> = sqlx::query(
            "SELECT table_name FROM information_schema.tables WHERE table_schema = DATABASE() ORDER BY table_name",
        )
        .fetch_all(&mut conn)
        .await
        .map_err(|e| format!("Failed to list tables: {}", e))?;

        let tables: Vec<String> = rows
            .iter()
            .filter_map(|r| r.try_get::<String, _>(0).ok())
            .collect();
        Ok(tables)
    })
}

/// PostgreSQL 行值转 Polars AnyValue
fn pg_value_to_anyvalue(
    row: &sqlx::postgres::PgRow,
    i: usize,
) -> Result<AnyValue<'static>, String> {
    let val_ref = row
        .try_get_raw(i)
        .map_err(|e| format!("Column {}: {}", i, e))?;

    if val_ref.is_null() {
        return Ok(AnyValue::Null);
    }

    let owned = ValueRef::to_owned(&val_ref);
    if let Ok(v) = owned.try_decode::<i64>() {
        return Ok(AnyValue::Int64(v));
    }
    if let Ok(v) = owned.try_decode::<f64>() {
        return Ok(AnyValue::Float64(v));
    }
    if let Ok(v) = owned.try_decode::<String>() {
        return Ok(AnyValue::StringOwned(v.into()));
    }
    if let Ok(v) = owned.try_decode::<bool>() {
        return Ok(AnyValue::Boolean(v));
    }
    if let Ok(v) = owned.try_decode::<Vec<u8>>() {
        return Ok(AnyValue::StringOwned(format!("<{} bytes>", v.len()).into()));
    }
    Ok(AnyValue::Null)
}

/// MySQL 行值转 Polars AnyValue
fn mysql_value_to_anyvalue(
    row: &sqlx::mysql::MySqlRow,
    i: usize,
) -> Result<AnyValue<'static>, String> {
    let val_ref = row
        .try_get_raw(i)
        .map_err(|e| format!("Column {}: {}", i, e))?;

    if val_ref.is_null() {
        return Ok(AnyValue::Null);
    }

    let owned = ValueRef::to_owned(&val_ref);
    if let Ok(v) = owned.try_decode::<i64>() {
        return Ok(AnyValue::Int64(v));
    }
    if let Ok(v) = owned.try_decode::<f64>() {
        return Ok(AnyValue::Float64(v));
    }
    if let Ok(v) = owned.try_decode::<String>() {
        return Ok(AnyValue::StringOwned(v.into()));
    }
    if let Ok(v) = owned.try_decode::<bool>() {
        return Ok(AnyValue::Boolean(v));
    }
    if let Ok(v) = owned.try_decode::<Vec<u8>>() {
        return Ok(AnyValue::StringOwned(format!("<{} bytes>", v.len()).into()));
    }
    Ok(AnyValue::Null)
}

/// PostgreSQL 表名转 SQL 标识符（双引号）
fn quote_pg_identifier(name: &str) -> String {
    format!("\"{}\"", name.replace('"', "\"\""))
}

/// MySQL 表名转 SQL 标识符（反引号）
fn quote_mysql_identifier(name: &str) -> String {
    format!("`{}`", name.replace('`', "``"))
}

/// 从 PostgreSQL 表读取数据并构建 Polars DataFrame
pub fn read_postgres_table_to_dataframe(
    connection_string: &str,
    table: &str,
) -> Result<DataFrame, String> {
    tauri::async_runtime::block_on(async {
        let opts: sqlx::postgres::PgConnectOptions = connection_string
            .parse()
            .map_err(|e: sqlx::Error| format!("Invalid PostgreSQL URL: {}", e))?;
        let mut conn = opts
            .connect()
            .await
            .map_err(|e| format!("Failed to connect to PostgreSQL: {}", e))?;

        let quoted = quote_pg_identifier(table);
        let sql = format!("SELECT * FROM {}", quoted);

        let rows: Vec<sqlx::postgres::PgRow> = sqlx::query(&sql)
            .fetch_all(&mut conn)
            .await
            .map_err(|e| format!("Failed to execute query: {}", e))?;

        build_dataframe_from_pg_rows(rows)
    })
}

/// 从 MySQL 表读取数据并构建 Polars DataFrame
pub fn read_mysql_table_to_dataframe(
    connection_string: &str,
    table: &str,
) -> Result<DataFrame, String> {
    tauri::async_runtime::block_on(async {
        let opts: sqlx::mysql::MySqlConnectOptions = connection_string
            .parse()
            .map_err(|e: sqlx::Error| format!("Invalid MySQL URL: {}", e))?;
        let mut conn = opts
            .connect()
            .await
            .map_err(|e| format!("Failed to connect to MySQL: {}", e))?;

        let quoted = quote_mysql_identifier(table);
        let sql = format!("SELECT * FROM {}", quoted);

        let rows: Vec<sqlx::mysql::MySqlRow> = sqlx::query(&sql)
            .fetch_all(&mut conn)
            .await
            .map_err(|e| format!("Failed to execute query: {}", e))?;

        build_dataframe_from_mysql_rows(rows)
    })
}

fn build_dataframe_from_pg_rows(rows: Vec<sqlx::postgres::PgRow>) -> Result<DataFrame, String> {
    if rows.is_empty() {
        let columns: Vec<polars::prelude::Column> = Vec::new();
        return DataFrame::new(0, columns).map_err(|e| format!("Failed to build DataFrame: {}", e));
    }

    let column_count = rows[0].len();
    let column_names: Vec<String> = rows[0]
        .columns()
        .iter()
        .map(|c| SqlxColumn::name(c).to_string())
        .collect();

    let mut columns_data: Vec<Vec<AnyValue<'static>>> =
        (0..column_count).map(|_| Vec::new()).collect();

    for row in &rows {
        for (i, col_data) in columns_data.iter_mut().enumerate() {
            let av = pg_value_to_anyvalue(row, i)?;
            col_data.push(av);
        }
    }

    build_dataframe(column_names, columns_data)
}

fn build_dataframe_from_mysql_rows(rows: Vec<sqlx::mysql::MySqlRow>) -> Result<DataFrame, String> {
    if rows.is_empty() {
        let columns: Vec<polars::prelude::Column> = Vec::new();
        return DataFrame::new(0, columns).map_err(|e| format!("Failed to build DataFrame: {}", e));
    }

    let column_count = rows[0].len();
    let column_names: Vec<String> = rows[0]
        .columns()
        .iter()
        .map(|c| SqlxColumn::name(c).to_string())
        .collect();

    let mut columns_data: Vec<Vec<AnyValue<'static>>> =
        (0..column_count).map(|_| Vec::new()).collect();

    for row in &rows {
        for (i, col_data) in columns_data.iter_mut().enumerate() {
            let av = mysql_value_to_anyvalue(row, i)?;
            col_data.push(av);
        }
    }

    build_dataframe(column_names, columns_data)
}

fn build_dataframe(
    column_names: Vec<String>,
    columns_data: Vec<Vec<AnyValue<'static>>>,
) -> Result<DataFrame, String> {
    let series: Vec<Series> = column_names
        .iter()
        .zip(columns_data.iter())
        .map(|(name, data)| {
            let name_ss: PlSmallStr = name.as_str().into();
            Series::from_any_values(name_ss.clone(), data, false)
                .unwrap_or_else(|_| Series::new_null(name_ss, data.len()))
        })
        .collect();

    let columns: Vec<polars::prelude::Column> = series
        .into_iter()
        .map(polars::prelude::Column::from)
        .collect();
    let height = columns_data.first().map(|d| d.len()).unwrap_or(0);
    DataFrame::new(height, columns).map_err(|e| format!("Failed to build DataFrame: {}", e))
}

/// 根据引擎类型列出表
pub fn list_tables(
    engine: &DatabaseEngineSql,
    connection_string: &str,
) -> Result<Vec<String>, String> {
    match engine {
        DatabaseEngineSql::Sqlite { .. } => super::sqlite_reader::list_tables(connection_string),
        DatabaseEngineSql::Postgres { .. } => list_postgres_tables(connection_string),
        DatabaseEngineSql::Mysql { .. } => list_mysql_tables(connection_string),
    }
}

/// 根据引擎类型读取表到 DataFrame
pub fn read_table_to_dataframe(
    engine: &DatabaseEngineSql,
    connection_string: &str,
    table: &str,
) -> Result<DataFrame, String> {
    match engine {
        DatabaseEngineSql::Sqlite { .. } => {
            super::sqlite_reader::read_table_to_dataframe(connection_string, table)
        }
        DatabaseEngineSql::Postgres { .. } => {
            read_postgres_table_to_dataframe(connection_string, table)
        }
        DatabaseEngineSql::Mysql { .. } => read_mysql_table_to_dataframe(connection_string, table),
    }
}
