//! DuckDB 项目库读写：Polars ↔ Arrow ↔ DuckDB。
//!
//! Categorical 列通过 DuckDB `ENUM`（类型名 `_yssbi_enum_*`）持久化；读写 Arrow 物理类型不对称，
//! 详见 [`README.md`](./README.md) 中「Categorical / ENUM 类型映射」一节。
use std::path::Path;

use duckdb::Connection;
use duckdb::arrow::array::{Array, make_array};
use duckdb::arrow::compute::cast;
use duckdb::arrow::datatypes::{DataType as ArrowDataType, Field, Schema};
use duckdb::arrow::ffi::{FFI_ArrowArray, FFI_ArrowSchema, from_ffi, to_ffi};
use duckdb::arrow::record_batch::RecordBatch;
use polars::prelude::*;
use polars_arrow::datatypes::Field as PlField;
use polars_arrow::ffi::{
    ArrowArray, ArrowSchema, export_array_to_c, export_field_to_c, import_array_from_c,
    import_field_from_c,
};
use polars_dtype::categorical::{CatSize, FrozenCategories};

use super::{quote_duckdb_identifier, quote_duckdb_string_literal};

pub const DEFAULT_DUCKDB_TABLE: &str = "data";
pub const YSSBI_META_TABLE: &str = "_yssbi_meta";
pub const YSSBI_ENUM_PREFIX: &str = "_yssbi_enum_";

/// Polars 列名：分页查询 `SELECT rowid, ...` 后用于提取 / drop。
pub const DUCKDB_ROWID_COL: &str = "rowid";

fn open_project_duckdb(duckdb_path: &Path) -> Result<Connection, String> {
    if let Some(parent) = duckdb_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Connection::open(duckdb_path).map_err(|e| e.to_string())
}

fn ensure_meta_table(conn: &Connection) -> Result<(), String> {
    let table = quote_duckdb_identifier(YSSBI_META_TABLE);
    conn.execute_batch(&format!(
        r#"CREATE TABLE IF NOT EXISTS {table} (
            table_id VARCHAR PRIMARY KEY,
            display_name VARCHAR NOT NULL
        );"#
    ))
    .map_err(|e| format!("Failed to ensure meta table: {e}"))
}

fn drop_user_table(conn: &Connection, table: &str) -> Result<(), String> {
    let table_sql = quote_duckdb_identifier(table);
    conn.execute_batch(&format!("DROP TABLE IF EXISTS {table_sql};"))
        .map_err(|e| format!("Failed to drop table '{table}': {e}"))
}

pub fn is_user_data_table(table: &str) -> bool {
    !table.starts_with('_') && table != YSSBI_META_TABLE
}

/// 列出项目 DuckDB 内所有用户数据表（排除 `_yssbi_meta` 等内部表）。
pub fn list_data_tables(duckdb_path: &Path) -> Result<Vec<String>, String> {
    if !duckdb_path.is_file() {
        return Ok(Vec::new());
    }
    let conn = Connection::open(duckdb_path).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT table_name FROM information_schema.tables \
             WHERE table_schema = 'main' AND table_type = 'BASE TABLE'",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    let mut tables = Vec::new();
    for row in rows {
        let name = row.map_err(|e| e.to_string())?;
        if is_user_data_table(&name) {
            tables.push(name);
        }
    }
    tables.sort();
    Ok(tables)
}

pub fn drop_data_table(duckdb_path: &Path, table: &str) -> Result<(), String> {
    if !duckdb_path.is_file() {
        return Ok(());
    }
    let conn = Connection::open(duckdb_path).map_err(|e| e.to_string())?;
    drop_user_table(&conn, table)?;
    let meta = quote_duckdb_identifier(YSSBI_META_TABLE);
    let table_id = quote_duckdb_string_literal(table);
    conn.execute(
        &format!("DELETE FROM {meta} WHERE table_id = {table_id}"),
        [],
    )
    .map_err(|e| format!("Failed to remove meta for table '{table}': {e}"))?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct DuckDbColumnMeta {
    pub name: String,
    pub dtype: String,
}

#[derive(Debug, Clone)]
pub struct DuckDbTableMeta {
    pub row_count: usize,
    pub columns: Vec<DuckDbColumnMeta>,
}

pub fn duckdb_path_literal(path: &Path) -> String {
    quote_duckdb_string_literal(&path.to_string_lossy().replace('\\', "/"))
}

/// 将 DuckDB `DESCRIBE` / `information_schema` 中的列类型映射为 Polars 逻辑类型名（schema 展示用）。
///
/// - 存储为 `ENUM` 或 `_yssbi_enum_*` → `"Categorical"`
/// - 固定宽度整数保留其有符号性与宽度
pub fn duckdb_type_to_raw_string(duckdb_type: &str) -> String {
    if is_duckdb_enum_storage_type(duckdb_type) {
        return "Categorical".to_string();
    }

    let upper = duckdb_type.to_uppercase();
    if upper.contains("DECIMAL") || upper.contains("NUMERIC") {
        return "Float64".to_string();
    }
    match upper.as_str() {
        "BOOLEAN" | "BOOL" => "Boolean".to_string(),
        "TINYINT" => "Int8".to_string(),
        "SMALLINT" => "Int16".to_string(),
        "INTEGER" | "INT" => "Int32".to_string(),
        "BIGINT" | "HUGEINT" => "Int64".to_string(),
        "UTINYINT" => "UInt8".to_string(),
        "USMALLINT" => "UInt16".to_string(),
        "UINTEGER" => "UInt32".to_string(),
        "UBIGINT" => "UInt64".to_string(),
        "FLOAT" | "REAL" => "Float32".to_string(),
        "DOUBLE" => "Float64".to_string(),
        "VARCHAR" | "TEXT" | "STRING" | "UUID" => "String".to_string(),
        "DATE" => "Date".to_string(),
        "TIME" => "Time".to_string(),
        "TIMESTAMP" | "TIMESTAMP WITH TIME ZONE" | "TIMESTAMPTZ" => "DateTime".to_string(),
        other => other.to_string(),
    }
}

pub fn is_yssbi_enum_type(type_name: &str) -> bool {
    type_name.starts_with(YSSBI_ENUM_PREFIX)
}

fn is_duckdb_enum_storage_type(storage_type: &str) -> bool {
    is_yssbi_enum_type(storage_type) || storage_type.trim().to_uppercase().starts_with("ENUM(")
}

fn parse_inline_enum_categories(storage_type: &str) -> Option<Vec<String>> {
    let trimmed = storage_type.trim();
    let upper = trimmed.to_uppercase();
    if !upper.starts_with("ENUM(") || !trimmed.ends_with(')') {
        return None;
    }

    let inner = trimmed[5..trimmed.len() - 1].trim();
    if inner.is_empty() {
        return Some(Vec::new());
    }

    let mut categories = Vec::new();
    let mut chars = inner.chars().peekable();
    while chars.peek().is_some() {
        while matches!(chars.peek(), Some(' ' | ',')) {
            chars.next();
        }
        if chars.peek().is_none() {
            break;
        }
        if chars.next() != Some('\'') {
            return None;
        }

        let mut value = String::new();
        while let Some(ch) = chars.next() {
            if ch == '\'' {
                if chars.peek() == Some(&'\'') {
                    chars.next();
                    value.push('\'');
                } else {
                    break;
                }
            } else {
                value.push(ch);
            }
        }
        categories.push(value);
    }

    Some(categories)
}

fn resolve_enum_categories(conn: &Connection, storage_type: &str) -> Result<Vec<String>, String> {
    if let Some(categories) = parse_inline_enum_categories(storage_type) {
        return Ok(categories);
    }
    if is_yssbi_enum_type(storage_type) {
        return fetch_enum_categories(conn, storage_type);
    }
    Err(format!("Unsupported enum storage type: {storage_type}"))
}

fn yssbi_enum_type_name(table: &str, column: &str) -> String {
    fn sanitize(value: &str) -> String {
        value
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '_'
                }
            })
            .collect()
    }

    format!(
        "{YSSBI_ENUM_PREFIX}{}_{}",
        sanitize(table),
        sanitize(column)
    )
}

#[derive(Debug, Clone)]
struct EnumColumnSpec {
    type_name: String,
    categories: Vec<String>,
}

fn extract_categorical_categories(series: &Series) -> Result<Vec<String>, String> {
    if let Ok(ca) = series.cat8() {
        return collect_categories_from_categorical_chunked(ca);
    }
    if let Ok(ca) = series.cat16() {
        return collect_categories_from_categorical_chunked(ca);
    }
    if let Ok(ca) = series.cat32() {
        return collect_categories_from_categorical_chunked(ca);
    }

    Err(format!("Column '{}' is not categorical", series.name()))
}

fn collect_categories_from_categorical_chunked<T: PolarsCategoricalType>(
    ca: &CategoricalChunked<T>,
) -> Result<Vec<String>, String> {
    match ca.dtype() {
        DataType::Enum(fcats, _) => Ok(fcats
            .categories()
            .values_iter()
            .map(|s| s.to_string())
            .collect()),
        DataType::Categorical(_, mapping) => {
            let n = mapping.num_cats_upper_bound();
            Ok((0..n)
                .filter_map(|i| mapping.cat_to_str(i as CatSize).map(str::to_string))
                .collect())
        }
        other => Err(format!("Unexpected categorical dtype: {other:?}")),
    }
}

fn plan_enum_columns(df: &DataFrame, table: &str) -> Result<Vec<(String, EnumColumnSpec)>, String> {
    let mut specs = Vec::new();
    for col in df.columns() {
        let series = col.as_materialized_series();
        if !matches!(
            series.dtype(),
            DataType::Categorical(_, _) | DataType::Enum(_, _)
        ) {
            continue;
        }

        let categories = extract_categorical_categories(series)?;
        if categories.is_empty() {
            continue;
        }

        let col_name = series.name().to_string();
        specs.push((
            col_name.clone(),
            EnumColumnSpec {
                type_name: yssbi_enum_type_name(table, &col_name),
                categories,
            },
        ));
    }
    Ok(specs)
}

fn describe_table_storage_types(
    conn: &Connection,
    table: &str,
) -> Result<Vec<(String, String)>, String> {
    let table_sql = quote_duckdb_identifier(table);
    let describe_sql = format!("DESCRIBE {table_sql}");
    let mut stmt = conn
        .prepare(&describe_sql)
        .map_err(|e| format!("Failed to describe table '{table}': {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            let name: String = row.get(0)?;
            let dtype: String = row.get(1)?;
            Ok((name, dtype))
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn drop_table_enum_types(conn: &Connection, table: &str) -> Result<(), String> {
    let columns = describe_table_storage_types(conn, table).unwrap_or_default();
    for (_, storage_type) in columns {
        if is_yssbi_enum_type(&storage_type) {
            let type_sql = quote_duckdb_identifier(&storage_type);
            conn.execute_batch(&format!("DROP TYPE IF EXISTS {type_sql} CASCADE;"))
                .map_err(|e| format!("Failed to drop enum type '{storage_type}': {e}"))?;
        }
    }
    Ok(())
}

fn drop_user_table_and_enum_types(conn: &Connection, table: &str) -> Result<(), String> {
    drop_table_enum_types(conn, table)?;
    drop_user_table(conn, table)
}

fn create_enum_type(conn: &Connection, spec: &EnumColumnSpec) -> Result<(), String> {
    let type_sql = quote_duckdb_identifier(&spec.type_name);
    let values = spec
        .categories
        .iter()
        .map(|value| quote_duckdb_string_literal(value))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "DROP TYPE IF EXISTS {type_sql} CASCADE;\nCREATE TYPE {type_sql} AS ENUM ({values});"
    );
    conn.execute_batch(&sql)
        .map_err(|e| format!("Failed to create enum type '{}': {e}", spec.type_name))
}

fn fetch_enum_categories(conn: &Connection, enum_type: &str) -> Result<Vec<String>, String> {
    let type_sql = quote_duckdb_identifier(enum_type);
    let sql = format!("SELECT unnest(enum_range(NULL::{type_sql}))");
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to read enum categories for '{enum_type}': {e}"))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn string_series_to_categorical(series: &Series, categories: &[String]) -> Result<Series, String> {
    let fcats = FrozenCategories::new(categories.iter().map(|s| s.as_str()))
        .map_err(|e| format!("Failed to rebuild categorical categories: {e}"))?;
    let dtype = DataType::Enum(fcats.clone(), fcats.mapping().clone());
    series.cast(&dtype).map_err(|e| {
        format!(
            "Failed to cast column '{}' to categorical: {e}",
            series.name()
        )
    })
}

fn restore_categorical_columns_with_conn(
    conn: &Connection,
    table: &str,
    df: &mut DataFrame,
) -> Result<(), String> {
    let columns = describe_table_storage_types(conn, table)?;

    for (col_name, storage_type) in columns {
        if !is_duckdb_enum_storage_type(&storage_type) {
            continue;
        }

        let categories = resolve_enum_categories(conn, &storage_type)?;
        if categories.is_empty() {
            continue;
        }

        let col_idx = df
            .get_column_index(&col_name)
            .ok_or_else(|| format!("Column '{col_name}' missing while restoring categorical"))?;
        let series = df
            .column(&col_name)
            .map_err(|e| e.to_string())?
            .as_materialized_series()
            .clone();
        let restored = string_series_to_categorical(&series, &categories)?;
        df.replace_column(col_idx, Column::from(restored))
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

pub fn ingest_csv_to_duckdb(
    csv_path: &Path,
    duckdb_path: &Path,
    table: &str,
    delimiter: char,
    has_header: bool,
    infer_schema_length: Option<usize>,
) -> Result<DuckDbTableMeta, String> {
    if !csv_path.is_file() {
        return Err(format!("CSV file not found: {}", csv_path.display()));
    }

    if let Some(parent) = duckdb_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let conn = open_project_duckdb(duckdb_path)?;
    ensure_meta_table(&conn)?;
    drop_user_table(&conn, table)?;

    let csv_literal = duckdb_path_literal(csv_path);
    let header = if has_header { "true" } else { "false" };
    let sample_size = infer_schema_length
        .map(|n| n.to_string())
        .unwrap_or_else(|| "-1".to_string());

    let table_sql = quote_duckdb_identifier(table);
    let delimiter_literal = quote_duckdb_string_literal(&delimiter.to_string());
    let sql = format!(
        "CREATE TABLE {table_sql} AS SELECT * FROM read_csv({csv_literal}, header={header}, delim={delimiter_literal}, auto_detect=true, sample_size={sample_size})"
    );

    conn.execute_batch(&sql)
        .map_err(|e| format!("Failed to ingest CSV into DuckDB: {e}"))?;
    drop(conn);

    read_table_meta(duckdb_path, table)
}

pub fn ingest_parquet_to_duckdb(
    parquet_path: &Path,
    duckdb_path: &Path,
    table: &str,
    columns: Option<&[String]>,
) -> Result<DuckDbTableMeta, String> {
    if !parquet_path.is_file() {
        return Err(format!(
            "Parquet file not found: {}",
            parquet_path.display()
        ));
    }

    if let Some(parent) = duckdb_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let conn = open_project_duckdb(duckdb_path)?;
    ensure_meta_table(&conn)?;
    drop_user_table(&conn, table)?;

    let parquet_literal = duckdb_path_literal(parquet_path);
    let table_sql = quote_duckdb_identifier(table);

    let select_list = match columns {
        None | Some([]) => "*".to_string(),
        Some(cols) => cols
            .iter()
            .map(|column| quote_duckdb_identifier(column))
            .collect::<Vec<_>>()
            .join(", "),
    };

    let sql = format!(
        "CREATE TABLE {table_sql} AS SELECT {select_list} FROM read_parquet({parquet_literal})"
    );

    conn.execute_batch(&sql)
        .map_err(|e| format!("Failed to ingest Parquet into DuckDB: {e}"))?;
    drop(conn);

    read_table_meta(duckdb_path, table)
}

/// 将 Polars `DataFrame` 写入项目 DuckDB（Arrow RecordBatch + Appender，无临时 Parquet）。
///
/// Categorical / Enum 列：先 `CREATE TYPE _yssbi_enum_{table}_{col}`，建 ENUM 列，再经
/// [`polars_series_to_arrow_array`]（`for_enum_ingest = true`）cast 为 **Utf8** 后 append。
/// Appender 不接受 Arrow Dictionary 直写 ENUM，见 `database/README.md`。
pub fn ingest_dataframe_to_duckdb(
    df: &mut DataFrame,
    duckdb_path: &Path,
    table: &str,
) -> Result<DuckDbTableMeta, String> {
    if df.height() == 0 && df.width() == 0 {
        return Err("Cannot ingest empty DataFrame".into());
    }

    if let Some(parent) = duckdb_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    df.rechunk_mut();
    let enum_columns = plan_enum_columns(df, table)?;
    let enum_by_column: std::collections::HashMap<String, EnumColumnSpec> = enum_columns
        .into_iter()
        .map(|(name, spec)| (name, spec))
        .collect();

    let conn = open_project_duckdb(duckdb_path)?;
    ensure_meta_table(&conn)?;
    drop_user_table_and_enum_types(&conn, table)?;

    for spec in enum_by_column.values() {
        create_enum_type(&conn, spec)?;
    }

    create_table_for_ingest(&conn, table, df, &enum_by_column)?;

    let height = df.height();
    if height > 0 {
        use super::duckdb_editing::INGEST_CHUNK_ROWS;
        let mut appender = conn
            .appender(table)
            .map_err(|e| format!("Failed to open DuckDB appender for '{table}': {e}"))?;
        for start in (0..height).step_by(INGEST_CHUNK_ROWS) {
            let chunk_len = INGEST_CHUNK_ROWS.min(height - start);
            let chunk = df.slice(start as i64, chunk_len);
            let batch = dataframe_to_record_batch(&chunk, &enum_by_column)?;
            appender
                .append_record_batch(batch)
                .map_err(|e| format!("Failed to append Arrow batch to '{table}': {e}"))?;
        }
    }

    drop(conn);
    read_table_meta(duckdb_path, table)
}

fn create_table_for_ingest(
    conn: &Connection,
    table: &str,
    df: &DataFrame,
    enum_by_column: &std::collections::HashMap<String, EnumColumnSpec>,
) -> Result<(), String> {
    let table_sql = quote_duckdb_identifier(table);
    let column_defs = df
        .columns()
        .iter()
        .map(|col| {
            let series = col.as_materialized_series();
            let col_name = series.name().to_string();
            let sql_type = if let Some(spec) = enum_by_column.get(&col_name) {
                quote_duckdb_identifier(&spec.type_name)
            } else {
                let arrow_array = polars_series_to_arrow_array(series, false)?;
                arrow_dtype_to_create_table_sql(arrow_array.data_type())?
            };
            Ok(format!(
                "{} {sql_type}",
                quote_duckdb_identifier(series.name().as_str())
            ))
        })
        .collect::<Result<Vec<_>, String>>()?;

    let sql = format!("CREATE TABLE {table_sql} ({})", column_defs.join(", "));
    conn.execute_batch(&sql)
        .map_err(|e| format!("Failed to create table '{table}': {e}"))
}

fn dataframe_to_record_batch(
    df: &DataFrame,
    enum_by_column: &std::collections::HashMap<String, EnumColumnSpec>,
) -> Result<RecordBatch, String> {
    let mut arrays = Vec::with_capacity(df.width());
    let mut fields = Vec::with_capacity(df.width());

    for col in df.columns() {
        let series = col.as_materialized_series();
        let use_enum = enum_by_column.contains_key(series.name().as_str());
        let arrow_array = polars_series_to_arrow_array(series, use_enum)?;
        fields.push(Field::new(
            series.name().as_str(),
            arrow_array.data_type().clone(),
            true,
        ));
        arrays.push(arrow_array);
    }

    let schema = std::sync::Arc::new(Schema::new(fields));
    RecordBatch::try_new(schema, arrays)
        .map_err(|e| format!("Failed to build Arrow RecordBatch: {e}"))
}

/// Polars 列 → Arrow 数组（FFI），供建表推断或 Appender 写入。
///
/// `for_enum_ingest == true`（写入 DuckDB ENUM 列）时，**必须先 cast 为 String → Arrow Utf8**。
/// DuckDB `appender-arrow` 对 ENUM 目标列只接受 Utf8 字面量，不接受 Dictionary；
/// 读侧 `query_arrow` 则返回 `Dictionary(UInt8, Utf8)`。
fn polars_series_to_arrow_array(
    series: &Series,
    for_enum_ingest: bool,
) -> Result<std::sync::Arc<dyn Array>, String> {
    let series = if for_enum_ingest {
        // ENUM Appender 仅接受 Utf8；Polars Categorical 的 Arrow 形态是 Dictionary，直接 append 会失败。
        series.cast(&DataType::String).map_err(|e| {
            format!(
                "Failed to cast categorical column '{}' for enum ingest: {e}",
                series.name()
            )
        })?
    } else if series.n_chunks() > 1 {
        series.rechunk()
    } else {
        series.clone()
    };

    let pl_field = PlField::new(
        series.name().clone(),
        series.dtype().to_arrow(CompatLevel::newest()),
        true,
    );
    let pl_array = series.to_arrow(0, CompatLevel::newest());
    let c_array = export_array_to_c(pl_array);
    let c_schema = export_field_to_c(&pl_field);

    let ffi_array: FFI_ArrowArray = unsafe { std::mem::transmute(c_array) };
    let ffi_schema: FFI_ArrowSchema = unsafe { std::mem::transmute(c_schema) };

    let array_data = unsafe { from_ffi(ffi_array, &ffi_schema) }.map_err(|e| {
        format!(
            "Failed to import Polars column '{}' to Arrow: {e}",
            series.name()
        )
    })?;
    let arrow_array = make_array(array_data);

    if !for_enum_ingest && matches!(arrow_array.data_type(), ArrowDataType::Dictionary(_, _)) {
        cast(arrow_array.as_ref(), &ArrowDataType::LargeUtf8).map_err(|e| {
            format!(
                "Failed to cast dictionary column '{}' to string: {e}",
                series.name()
            )
        })
    } else {
        Ok(arrow_array)
    }
}

fn arrow_dtype_to_create_table_sql(dtype: &ArrowDataType) -> Result<String, String> {
    Ok(match dtype {
        ArrowDataType::Null => "VARCHAR".to_string(),
        ArrowDataType::Boolean => "BOOLEAN".to_string(),
        ArrowDataType::Int8 => "TINYINT".to_string(),
        ArrowDataType::Int16 => "SMALLINT".to_string(),
        ArrowDataType::Int32 => "INTEGER".to_string(),
        ArrowDataType::Int64 => "BIGINT".to_string(),
        ArrowDataType::UInt8 => "UTINYINT".to_string(),
        ArrowDataType::UInt16 => "USMALLINT".to_string(),
        ArrowDataType::UInt32 => "UINTEGER".to_string(),
        ArrowDataType::UInt64 => "UBIGINT".to_string(),
        ArrowDataType::Float16 | ArrowDataType::Float32 => "FLOAT".to_string(),
        ArrowDataType::Float64 => "DOUBLE".to_string(),
        ArrowDataType::Utf8 | ArrowDataType::LargeUtf8 | ArrowDataType::Utf8View => {
            "VARCHAR".to_string()
        }
        ArrowDataType::Binary | ArrowDataType::LargeBinary | ArrowDataType::BinaryView => {
            "BLOB".to_string()
        }
        ArrowDataType::Date32 | ArrowDataType::Date64 => "DATE".to_string(),
        ArrowDataType::Time32(_) | ArrowDataType::Time64(_) => "TIME".to_string(),
        ArrowDataType::Timestamp(_, _) => "TIMESTAMP".to_string(),
        ArrowDataType::Decimal128(precision, scale)
        | ArrowDataType::Decimal256(precision, scale) => {
            format!("DECIMAL({precision},{scale})")
        }
        ArrowDataType::Dictionary(_, value_type) => arrow_dtype_to_create_table_sql(value_type)?,
        other => {
            return Err(format!(
                "Unsupported Arrow type for DuckDB ingest: {other:?}"
            ));
        }
    })
}

pub fn ingest_excel_to_duckdb(
    excel_path: &Path,
    sheet: &str,
    duckdb_path: &Path,
    table: &str,
) -> Result<DuckDbTableMeta, String> {
    if !excel_path.is_file() {
        return Err(format!("Excel file not found: {}", excel_path.display()));
    }

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let temp_csv = std::env::temp_dir().join(format!("yssbi_excel_{table}_{stamp}.csv"));
    super::excel_reader::export_sheet_to_csv(
        excel_path.to_string_lossy().as_ref(),
        sheet,
        &temp_csv,
    )?;
    let meta = ingest_csv_to_duckdb(&temp_csv, duckdb_path, table, ',', true, None);
    let _ = std::fs::remove_file(&temp_csv);
    meta
}

pub fn read_table_meta(duckdb_path: &Path, table: &str) -> Result<DuckDbTableMeta, String> {
    if !duckdb_path.is_file() {
        return Err(format!("DuckDB file not found: {}", duckdb_path.display()));
    }

    let conn = Connection::open(duckdb_path).map_err(|e| e.to_string())?;
    let table_sql = quote_duckdb_identifier(table);

    let mut columns = Vec::new();
    let describe_sql = format!("DESCRIBE {table_sql}");
    let mut stmt = conn
        .prepare(&describe_sql)
        .map_err(|e| format!("Failed to describe table '{table}': {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            let name: String = row.get(0)?;
            let dtype: String = row.get(1)?;
            Ok(DuckDbColumnMeta {
                name,
                dtype: duckdb_type_to_raw_string(&dtype),
            })
        })
        .map_err(|e| e.to_string())?;
    for row in rows {
        columns.push(row.map_err(|e| e.to_string())?);
    }

    let count_sql = format!("SELECT COUNT(*) FROM {table_sql}");
    let row_count: i64 = conn
        .query_row(&count_sql, [], |row| row.get(0))
        .map_err(|e| format!("Failed to count rows in '{table}': {e}"))?;

    Ok(DuckDbTableMeta {
        row_count: row_count.max(0) as usize,
        columns,
    })
}

pub fn write_display_name(duckdb_path: &Path, table_id: &str, name: &str) -> Result<(), String> {
    let conn = open_project_duckdb(duckdb_path)?;
    ensure_meta_table(&conn)?;
    let meta = quote_duckdb_identifier(YSSBI_META_TABLE);
    let table_key = quote_duckdb_string_literal(table_id);
    let value = quote_duckdb_string_literal(name);
    let sql = format!(
        "DELETE FROM {meta} WHERE table_id = {table_key};\nINSERT INTO {meta} (table_id, display_name) VALUES ({table_key}, {value});"
    );
    conn.execute_batch(&sql)
        .map_err(|e| format!("Failed to write display name: {e}"))
}

pub fn read_display_name(duckdb_path: &Path, table_id: &str) -> Option<String> {
    if !duckdb_path.is_file() {
        return None;
    }
    let conn = Connection::open(duckdb_path).ok()?;
    let meta = quote_duckdb_identifier(YSSBI_META_TABLE);
    let table_key = quote_duckdb_string_literal(table_id);
    let sql = format!("SELECT display_name FROM {meta} WHERE table_id = {table_key}");
    conn.query_row(&sql, [], |row| row.get(0)).ok()
}

pub fn query_to_dataframe(duckdb_path: &Path, sql: &str) -> Result<DataFrame, String> {
    query_to_dataframe_for_table(duckdb_path, sql, None)
}

pub fn query_to_dataframe_for_table(
    duckdb_path: &Path,
    sql: &str,
    table: Option<&str>,
) -> Result<DataFrame, String> {
    let conn = Connection::open(duckdb_path).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("Failed to prepare DuckDB query: {e}"))?;
    let batches: Vec<RecordBatch> = stmt
        .query_arrow([])
        .map_err(|e| format!("Failed to execute DuckDB query: {e}"))?
        .collect();
    let mut df = record_batches_to_dataframe(batches)?;
    if let Some(table) = table {
        restore_categorical_columns_with_conn(&conn, table, &mut df)?;
    }
    Ok(df)
}

/// DuckDB `query_arrow` batches → Polars via Arrow C Data Interface (no temp files).
fn record_batches_to_dataframe(batches: Vec<RecordBatch>) -> Result<DataFrame, String> {
    if batches.is_empty() {
        return Ok(DataFrame::empty());
    }
    let mut frames: Vec<DataFrame> = batches
        .iter()
        .map(record_batch_to_dataframe)
        .collect::<Result<Vec<_>, _>>()?;
    if frames.len() == 1 {
        return Ok(frames.remove(0));
    }
    let lazy: Vec<LazyFrame> = frames.into_iter().map(|df| df.lazy()).collect();
    concat(&lazy, UnionArgs::default())
        .map_err(|e| e.to_string())?
        .collect()
        .map_err(|e| e.to_string())
}

fn record_batch_to_dataframe(batch: &RecordBatch) -> Result<DataFrame, String> {
    let schema = batch.schema();
    let mut columns = Vec::with_capacity(batch.num_columns());
    for i in 0..batch.num_columns() {
        let field = schema.field(i);
        columns.push(arrow_rs_array_to_series(
            field.name(),
            batch.column(i).as_ref(),
        )?);
    }
    DataFrame::new_infer_height(
        columns
            .into_iter()
            .map(|s| s.into_column())
            .collect::<Vec<_>>(),
    )
    .map_err(|e| e.to_string())
}

fn arrow_rs_array_to_series(name: &str, array: &dyn Array) -> Result<Series, String> {
    let data = array.to_data();
    let (ffi_array, ffi_schema) = to_ffi(&data).map_err(|e| e.to_string())?;

    let pl_field = unsafe {
        import_field_from_c(&*(&ffi_schema as *const FFI_ArrowSchema as *const ArrowSchema))
    }
    .map_err(|e| e.to_string())?;

    let pl_array = unsafe {
        import_array_from_c(
            std::mem::transmute_copy::<FFI_ArrowArray, ArrowArray>(&ffi_array),
            pl_field.dtype.clone(),
        )
    }
    .map_err(|e| e.to_string())?;

    // `import_array_from_c` took ownership of the exported buffers.
    std::mem::forget((ffi_array, ffi_schema));

    unsafe { Series::_try_from_arrow_unchecked(name.into(), vec![pl_array], &pl_field.dtype) }
        .map_err(|e| e.to_string())
}

pub fn duckdb_table_sql(table: &str) -> String {
    quote_duckdb_identifier(table)
}

pub fn query_columns_to_dataframe(
    duckdb_path: &Path,
    table: &str,
    columns: &[&str],
) -> Result<DataFrame, String> {
    if columns.is_empty() {
        return query_to_dataframe_for_table(
            duckdb_path,
            &format!("SELECT * FROM {} LIMIT 0", duckdb_table_sql(table)),
            Some(table),
        );
    }

    let select_list = columns
        .iter()
        .map(|column| quote_duckdb_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("SELECT {} FROM {}", select_list, duckdb_table_sql(table));
    query_to_dataframe_for_table(duckdb_path, &sql, Some(table))
}

pub fn query_page_to_dataframe(
    duckdb_path: &Path,
    table: &str,
    offset: usize,
    limit: usize,
) -> Result<DataFrame, String> {
    Ok(query_page_with_rowids(duckdb_path, table, offset, limit)?.dataframe)
}

#[derive(Debug, Clone)]
pub struct PageQueryResult {
    pub dataframe: DataFrame,
    pub row_ids: Vec<i64>,
}

pub fn query_page_with_rowids(
    duckdb_path: &Path,
    table: &str,
    offset: usize,
    limit: usize,
) -> Result<PageQueryResult, String> {
    use super::duckdb_sql::DUCKDB_ROWID_SQL;

    let meta = read_table_meta(duckdb_path, table)?;
    let user_cols = meta
        .columns
        .iter()
        .map(|column| quote_duckdb_identifier(&column.name))
        .collect::<Vec<_>>();
    let table_sql = duckdb_table_sql(table);

    if limit == 0 {
        return Ok(PageQueryResult {
            dataframe: query_to_dataframe_for_table(
                duckdb_path,
                &format!("SELECT {} FROM {} LIMIT 0", user_cols.join(", "), table_sql),
                Some(table),
            )?,
            row_ids: Vec::new(),
        });
    }

    let select_list = if user_cols.is_empty() {
        DUCKDB_ROWID_SQL.to_string()
    } else {
        format!("{DUCKDB_ROWID_SQL}, {}", user_cols.join(", "))
    };
    let sql = format!(
        "SELECT {select_list} FROM {table_sql} ORDER BY {DUCKDB_ROWID_SQL} LIMIT {limit} OFFSET {offset}"
    );
    let df = query_to_dataframe_for_table(duckdb_path, &sql, Some(table))?;
    let row_ids = df
        .column(DUCKDB_ROWID_COL)
        .map_err(|e| e.to_string())?
        .i64()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|v| v.unwrap_or(0))
        .collect::<Vec<_>>();
    let user_df = df.drop(DUCKDB_ROWID_COL).map_err(|e| e.to_string())?;
    Ok(PageQueryResult {
        dataframe: user_df,
        row_ids,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn iris_page_rowids_are_stable() {
        let csv_path = PathBuf::from("tests/data/iris.csv");
        let duckdb_path = PathBuf::from(format!(
            "target/test_iris_rowids_{}.duckdb",
            uuid::Uuid::new_v4()
        ));
        ingest_csv_to_duckdb(
            &csv_path,
            &duckdb_path,
            DEFAULT_DUCKDB_TABLE,
            ',',
            true,
            Some(100),
        )
        .expect("ingest");
        let page = query_page_with_rowids(&duckdb_path, DEFAULT_DUCKDB_TABLE, 0, 3).expect("page");
        assert!(!page.row_ids.is_empty());
        for w in page.row_ids.windows(2) {
            assert!(w[0] < w[1], "rowids must be ascending: {:?}", page.row_ids);
        }
        let _ = std::fs::remove_file(&duckdb_path);
    }

    #[test]
    fn ingest_categorical_enum_roundtrip() {
        use crate::database::cast_column;

        let mut df = df!(
            "city" => &["北京", "上海", "北京"],
            "value" => &[1i64, 2, 3],
        )
        .expect("df");
        cast_column(&mut df, "city", "Categorical", true).expect("cast");

        let duckdb_path = PathBuf::from(format!(
            "target/test_duckdb_categorical_enum_{}.duckdb",
            uuid::Uuid::new_v4()
        ));

        ingest_dataframe_to_duckdb(&mut df, &duckdb_path, "test_table").expect("ingest");

        let meta = read_table_meta(&duckdb_path, "test_table").expect("meta");
        let city_meta = meta
            .columns
            .iter()
            .find(|col| col.name == "city")
            .expect("city meta");
        assert_eq!(city_meta.dtype, "Categorical");

        let loaded = query_page_to_dataframe(&duckdb_path, "test_table", 0, 10).expect("query");
        let city = loaded.column("city").expect("city");
        assert!(matches!(
            city.dtype(),
            DataType::Categorical(_, _) | DataType::Enum(_, _)
        ));

        let _ = std::fs::remove_file(&duckdb_path);
    }

    #[test]
    fn ingest_dataframe_via_arrow_roundtrip() {
        let mut df = df!(
            "name" => &["a", "b"],
            "value" => &[1i64, 2],
        )
        .expect("df");
        let duckdb_path = PathBuf::from(format!(
            "target/test_duckdb_arrow_ingest_{}.duckdb",
            uuid::Uuid::new_v4()
        ));

        ingest_dataframe_to_duckdb(&mut df, &duckdb_path, "test_table").expect("ingest");

        let meta = read_table_meta(&duckdb_path, "test_table").expect("meta");
        assert_eq!(meta.row_count, 2);
        assert_eq!(meta.columns.len(), 2);
        assert!(
            meta.columns.iter().all(|c| c.name != "_yssbi_rowid"),
            "ingest must not add _yssbi_rowid column"
        );

        let loaded = query_to_dataframe(
            &duckdb_path,
            r#"SELECT * FROM "test_table" ORDER BY "value""#,
        )
        .expect("query");
        assert_eq!(loaded.height(), 2);
        assert_eq!(
            loaded
                .column("name")
                .expect("name")
                .str()
                .expect("str")
                .get(0),
            Some("a")
        );

        let _ = std::fs::remove_file(&duckdb_path);
    }
}
