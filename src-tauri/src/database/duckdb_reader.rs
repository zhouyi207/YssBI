use std::path::{Path, PathBuf};

use duckdb::arrow::array::Array;
use duckdb::arrow::ffi::{to_ffi, FFI_ArrowArray, FFI_ArrowSchema};
use duckdb::arrow::record_batch::RecordBatch;
use duckdb::Connection;
use polars::prelude::*;
use polars_arrow::ffi::{import_array_from_c, import_field_from_c, ArrowArray, ArrowSchema};
use uuid::Uuid;

pub const DEFAULT_DUCKDB_TABLE: &str = "data";
pub const YSSBI_META_TABLE: &str = "_yssbi_meta";

fn open_project_duckdb(duckdb_path: &Path) -> Result<Connection, String> {
    if let Some(parent) = duckdb_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    Connection::open(duckdb_path).map_err(|e| e.to_string())
}

fn ensure_meta_table(conn: &Connection) -> Result<(), String> {
    let table = sql_escape_literal(YSSBI_META_TABLE);
    conn.execute_batch(&format!(
        r#"CREATE TABLE IF NOT EXISTS "{table}" (
            table_id VARCHAR PRIMARY KEY,
            display_name VARCHAR NOT NULL
        );"#
    ))
    .map_err(|e| format!("Failed to ensure meta table: {e}"))
}

fn drop_user_table(conn: &Connection, table: &str) -> Result<(), String> {
    let table_literal = sql_escape_literal(table);
    conn.execute_batch(&format!(r#"DROP TABLE IF EXISTS "{table_literal}";"#))
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
    let meta = sql_escape_literal(YSSBI_META_TABLE);
    let table_id = sql_escape_literal(table);
    conn.execute(
        &format!(r#"DELETE FROM "{meta}" WHERE table_id = '{table_id}'"#),
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

pub fn sql_escape_literal(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\'', "''")
}

pub fn duckdb_path_literal(path: &Path) -> String {
    sql_escape_literal(&path.to_string_lossy().replace('\\', "/"))
}

pub fn duckdb_type_to_raw_string(duckdb_type: &str) -> String {
    let upper = duckdb_type.to_uppercase();
    if upper.contains("DECIMAL") || upper.contains("NUMERIC") {
        return "Float64".to_string();
    }
    match upper.as_str() {
        "BOOLEAN" | "BOOL" => "Boolean".to_string(),
        "TINYINT" | "SMALLINT" | "INTEGER" | "INT" | "BIGINT" | "HUGEINT" => "Int64".to_string(),
        "UTINYINT" | "USMALLINT" | "UINTEGER" | "UBIGINT" => "Int64".to_string(),
        "FLOAT" | "REAL" => "Float32".to_string(),
        "DOUBLE" => "Float64".to_string(),
        "VARCHAR" | "TEXT" | "STRING" | "UUID" => "String".to_string(),
        "DATE" => "Date".to_string(),
        "TIME" => "Time".to_string(),
        "TIMESTAMP" | "TIMESTAMP WITH TIME ZONE" | "TIMESTAMPTZ" => {
            "Datetime(Microseconds, None)".to_string()
        }
        other => other.to_string(),
    }
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

    let sql = format!(
        r#"CREATE TABLE "{}" AS SELECT * FROM read_csv('{}', header={}, delim='{}', auto_detect=true, sample_size={})"#,
        sql_escape_literal(table),
        csv_literal,
        header,
        sql_escape_literal(&delimiter.to_string()),
        sample_size,
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
        return Err(format!("Parquet file not found: {}", parquet_path.display()));
    }

    if let Some(parent) = duckdb_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let conn = open_project_duckdb(duckdb_path)?;
    ensure_meta_table(&conn)?;
    drop_user_table(&conn, table)?;

    let parquet_literal = duckdb_path_literal(parquet_path);
    let table_literal = sql_escape_literal(table);

    let select_list = match columns {
        None | Some([]) => "*".to_string(),
        Some(cols) => cols
            .iter()
            .map(|c| format!(r#""{}""#, sql_escape_literal(c)))
            .collect::<Vec<_>>()
            .join(", "),
    };

    let sql = format!(
        r#"CREATE TABLE "{}" AS SELECT {} FROM read_parquet('{}')"#,
        table_literal, select_list, parquet_literal
    );

    conn.execute_batch(&sql)
        .map_err(|e| format!("Failed to ingest Parquet into DuckDB: {e}"))?;
    drop(conn);

    read_table_meta(duckdb_path, table)
}

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

    let tmp = std::env::temp_dir().join(format!("yssbi-ingest-{}.parquet", Uuid::new_v4()));
    let file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    ParquetWriter::new(file)
        .finish(df)
        .map_err(|e| format!("Failed to write ingest Parquet: {e}"))?;

    let result = ingest_parquet_to_duckdb(&tmp, duckdb_path, table, None);
    let _ = std::fs::remove_file(&tmp);
    result
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

    let mut df = super::excel_reader::read_sheet_to_dataframe(
        excel_path.to_string_lossy().as_ref(),
        sheet,
    )?;
    ingest_dataframe_to_duckdb(&mut df, duckdb_path, table)
}

pub fn read_table_meta(duckdb_path: &Path, table: &str) -> Result<DuckDbTableMeta, String> {
    if !duckdb_path.is_file() {
        return Err(format!("DuckDB file not found: {}", duckdb_path.display()));
    }

    let conn = Connection::open(duckdb_path).map_err(|e| e.to_string())?;
    let table_literal = sql_escape_literal(table);

    let mut columns = Vec::new();
    let describe_sql = format!(r#"DESCRIBE "{}""#, table_literal);
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

    let count_sql = format!(r#"SELECT COUNT(*) FROM "{}""#, table_literal);
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
    let meta = sql_escape_literal(YSSBI_META_TABLE);
    let table_key = sql_escape_literal(table_id);
    let value = sql_escape_literal(name);
    let sql = format!(
        r#"DELETE FROM "{meta}" WHERE table_id = '{table_key}';
           INSERT INTO "{meta}" (table_id, display_name) VALUES ('{table_key}', '{value}');"#
    );
    conn.execute_batch(&sql)
        .map_err(|e| format!("Failed to write display name: {e}"))
}

pub fn read_display_name(duckdb_path: &Path, table_id: &str) -> Option<String> {
    if !duckdb_path.is_file() {
        return None;
    }
    let conn = Connection::open(duckdb_path).ok()?;
    let meta = sql_escape_literal(YSSBI_META_TABLE);
    let table_key = sql_escape_literal(table_id);
    let sql = format!(
        r#"SELECT display_name FROM "{meta}" WHERE table_id = '{table_key}'"#
    );
    conn.query_row(&sql, [], |row| row.get(0)).ok()
}

pub fn query_to_dataframe(duckdb_path: &Path, sql: &str) -> Result<DataFrame, String> {
    let conn = Connection::open(duckdb_path).map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare(sql)
        .map_err(|e| format!("Failed to prepare DuckDB query: {e}"))?;
    let batches: Vec<RecordBatch> = stmt
        .query_arrow([])
        .map_err(|e| format!("Failed to execute DuckDB query: {e}"))?
        .collect();
    record_batches_to_dataframe(batches)
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
        columns.push(arrow_rs_array_to_series(field.name(), batch.column(i).as_ref())?);
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
    format!(r#""{}""#, sql_escape_literal(table))
}

pub fn query_columns_to_dataframe(
    duckdb_path: &Path,
    table: &str,
    columns: &[&str],
) -> Result<DataFrame, String> {
    if columns.is_empty() {
        return query_to_dataframe(
            duckdb_path,
            &format!("SELECT * FROM {} LIMIT 0", duckdb_table_sql(table)),
        );
    }

    let select_list = columns
        .iter()
        .map(|c| format!(r#""{}""#, sql_escape_literal(c)))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!("SELECT {} FROM {}", select_list, duckdb_table_sql(table));
    query_to_dataframe(duckdb_path, &sql)
}

pub fn query_page_to_dataframe(
    duckdb_path: &Path,
    table: &str,
    offset: usize,
    limit: usize,
) -> Result<DataFrame, String> {
    if limit == 0 {
        return query_to_dataframe(
            duckdb_path,
            &format!("SELECT * FROM {} LIMIT 0", duckdb_table_sql(table)),
        );
    }
    let sql = format!(
        "SELECT * FROM {} LIMIT {} OFFSET {}",
        duckdb_table_sql(table),
        limit,
        offset
    );
    query_to_dataframe(duckdb_path, &sql)
}

pub fn resolve_duckdb_file(project_root: &Path, relative_path: &str) -> PathBuf {
    project_root.join(relative_path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ingest_csv_and_read_meta() {
        let csv_path = PathBuf::from("tests/data/iris.csv");
        let duckdb_path = PathBuf::from("target/test_duckdb_ingest.duckdb");
        let _ = std::fs::remove_file(&duckdb_path);

        let meta = ingest_csv_to_duckdb(
            &csv_path,
            &duckdb_path,
            DEFAULT_DUCKDB_TABLE,
            ',',
            true,
            Some(100),
        )
        .expect("ingest");

        assert_eq!(meta.row_count, 150);
        assert!(meta.columns.len() >= 5);
        assert_eq!(meta.columns[0].name, "sepal_length");

        let df = query_to_dataframe(
            &duckdb_path,
            &format!(
                "SELECT * FROM {} LIMIT 10",
                duckdb_table_sql(DEFAULT_DUCKDB_TABLE)
            ),
        )
        .expect("query");
        assert_eq!(df.height(), 10);

        let page = query_page_to_dataframe(&duckdb_path, DEFAULT_DUCKDB_TABLE, 5, 7).expect("page");
        assert_eq!(page.height(), 7);
        assert!(df.width() >= 5);
        let _ = std::fs::remove_file(&duckdb_path);
    }
}
