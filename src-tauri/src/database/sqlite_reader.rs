//! SQLite 读取：列出表、读取表数据转 Polars DataFrame（使用 sqlx）

use polars::prelude::*;
use sqlx::sqlite::{SqliteConnectOptions, SqliteRow};
use sqlx::{Column as SqlxColumn, ConnectOptions, Row, Value, ValueRef};

/// 构建 SQLite 连接 URL（只读）
fn sqlite_url(db_path: &str) -> String {
    let path = db_path.replace('\\', "/");
    let url = if path.len() >= 2
        && path.chars().next().map(|c| c.is_ascii_alphabetic()).unwrap_or(false)
        && path.chars().nth(1) == Some(':')
    {
        format!("sqlite:///{}?mode=ro", path)
    } else {
        format!("sqlite://{}?mode=ro", path)
    };
    url
}

/// 列出 SQLite 数据库中的用户表（排除 sqlite_ 前缀的系统表）
pub fn list_tables(db_path: &str) -> Result<Vec<String>, String> {
    let url = sqlite_url(db_path);
    tauri::async_runtime::block_on(async {
        let opts: SqliteConnectOptions = url
            .parse()
            .map_err(|e: sqlx::Error| format!("Invalid SQLite URL: {}", e))?;
        let mut conn = opts
            .connect()
            .await
            .map_err(|e| format!("Failed to open SQLite: {}", e))?;

        let rows: Vec<SqliteRow> = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
            .fetch_all(&mut conn)
            .await
            .map_err(|e| format!("Failed to list tables: {}", e))?;

        let tables: Vec<String> = rows
            .into_iter()
            .filter_map(|r| r.try_get::<String, _>(0).ok())
            .collect();
        Ok(tables)
    })
}

/// sqlx SqliteValueRef 转 Polars AnyValue（通过 try_decode 尝试多种类型）
fn sqlite_value_to_anyvalue(row: &SqliteRow, i: usize) -> Result<AnyValue<'static>, String> {
    let val_ref = row
        .try_get_raw(i)
        .map_err(|e| format!("Column {}: {}", i, e))?;

    if val_ref.is_null() {
        return Ok(AnyValue::Null);
    }

    let owned = val_ref.to_owned();
    if let Ok(v) = owned.try_decode::<i64>() {
        return Ok(AnyValue::Int64(v));
    }
    if let Ok(v) = owned.try_decode::<f64>() {
        return Ok(AnyValue::Float64(v));
    }
    if let Ok(v) = owned.try_decode::<String>() {
        return Ok(AnyValue::StringOwned(v.into()));
    }
    if let Ok(v) = owned.try_decode::<Vec<u8>>() {
        return Ok(AnyValue::StringOwned(
            format!("<{} bytes>", v.len()).into(),
        ));
    }
    Ok(AnyValue::Null)
}

/// 从 SQLite 表读取数据并构建 Polars DataFrame
pub fn read_table_to_dataframe(db_path: &str, table: &str) -> Result<DataFrame, String> {
    let url = sqlite_url(db_path);
    tauri::async_runtime::block_on(async {
        let opts: SqliteConnectOptions = url
            .parse()
            .map_err(|e: sqlx::Error| format!("Invalid SQLite URL: {}", e))?;
        let mut conn = opts
            .connect()
            .await
            .map_err(|e| format!("Failed to open SQLite: {}", e))?;

        let escaped_table = format!("\"{}\"", table.replace('"', "\"\""));
        let sql = format!("SELECT * FROM {}", escaped_table);

        let rows: Vec<SqliteRow> = sqlx::query(&sql)
            .fetch_all(&mut conn)
            .await
            .map_err(|e| format!("Failed to execute query: {}", e))?;

        if rows.is_empty() {
            let pragma_sql = format!("PRAGMA table_info({})", escaped_table);
            let pragma_rows: Vec<SqliteRow> = sqlx::query(&pragma_sql)
                .fetch_all(&mut conn)
                .await
                .map_err(|e| format!("Failed to get table info: {}", e))?;
            let column_names: Vec<String> = pragma_rows
                .iter()
                .filter_map(|r| r.try_get::<String, _>(1).ok())
                .collect();
            let series: Vec<Series> = column_names
                .iter()
                .map(|name| {
                    let name_ss: PlSmallStr = name.as_str().into();
                    Series::new_null(name_ss, 0)
                })
                .collect();
            let columns: Vec<polars::prelude::Column> =
                series.into_iter().map(polars::prelude::Column::from).collect();
            return DataFrame::new(columns).map_err(|e| format!("Failed to build DataFrame: {}", e));
        }

        let column_count = rows[0].len();
        let column_names: Vec<String> = rows[0]
            .columns()
            .iter()
            .map(|c| SqlxColumn::name(c).to_string())
            .collect();

        if column_names.is_empty() {
            return Err("Table has no columns".into());
        }

        let mut columns_data: Vec<Vec<AnyValue<'static>>> = (0..column_count)
            .map(|_| Vec::new())
            .collect();

        for row in &rows {
            for (i, col_data) in columns_data.iter_mut().enumerate() {
                let av = sqlite_value_to_anyvalue(row, i)?;
                col_data.push(av);
            }
        }

        let series: Vec<Series> = column_names
            .iter()
            .zip(columns_data.iter())
            .map(|(name, data)| {
                let name_ss: PlSmallStr = name.as_str().into();
                Series::from_any_values(name_ss.clone(), data, false)
                    .unwrap_or_else(|_| Series::new_null(name_ss, data.len()))
            })
            .collect();

        let columns: Vec<polars::prelude::Column> =
            series.into_iter().map(polars::prelude::Column::from).collect();
        DataFrame::new(columns).map_err(|e| format!("Failed to build DataFrame: {}", e))
    })
}
