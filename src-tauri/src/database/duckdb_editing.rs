//! DuckDB 侧 DataView 编辑：SQL 增量写入，避免 `SELECT *` 整表进 Polars。
//! 行定位使用 DuckDB 内置 `rowid` 伪列（非物理用户列）。

use std::path::Path;

use duckdb::Connection;

use super::{
    DuckDbColumnMeta, EditOperation, EditState, duckdb_table_sql, read_table_meta,
    sql_escape_literal,
};

/// DuckDB `rowid` 伪列（SQL 关键字，不加引号）。
pub const DUCKDB_ROWID_SQL: &str = "rowid";

pub const MAX_IN_MEMORY_EDIT_ROWS: usize = 50_000;
pub const INGEST_CHUNK_ROWS: usize = 50_000;
pub const MAX_GET_DATAFRAME_ROWS: usize = 500_000;

pub fn should_use_in_memory_editing(row_count: usize) -> bool {
    row_count <= MAX_IN_MEMORY_EDIT_ROWS
}

fn open_conn(duckdb_path: &Path) -> Result<Connection, String> {
    Connection::open(duckdb_path).map_err(|e| e.to_string())
}

fn quote_ident(name: &str) -> String {
    format!(r#""{}""#, sql_escape_literal(name))
}

fn user_column_names(conn: &Connection, table: &str) -> Result<Vec<String>, String> {
    conn.prepare(&format!(
        "SELECT column_name FROM information_schema.columns \
             WHERE table_schema = 'main' AND table_name = '{}' \
             ORDER BY ordinal_position",
        sql_escape_literal(table),
    ))
    .map_err(|e| e.to_string())?
    .query_map([], |row| row.get::<_, String>(0))
    .map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())
}

pub fn refresh_duckdb_meta(
    duckdb_path: &Path,
    table: &str,
) -> Result<(usize, Vec<DuckDbColumnMeta>), String> {
    let meta = read_table_meta(duckdb_path, table)?;
    Ok((meta.row_count, meta.columns))
}

pub fn resolve_row_id_by_index(
    conn: &Connection,
    table: &str,
    row_index: usize,
) -> Result<i64, String> {
    let table_sql = duckdb_table_sql(table);
    conn.query_row(
        &format!(
            "SELECT {DUCKDB_ROWID_SQL} FROM {table_sql} ORDER BY {DUCKDB_ROWID_SQL} LIMIT 1 OFFSET ?"
        ),
        [row_index as i64],
        |row| row.get(0),
    )
    .map_err(|e| format!("Failed to resolve rowid at index {row_index}: {e}"))
}

pub fn resolve_row_ids_by_indices(
    conn: &Connection,
    table: &str,
    indices: &[usize],
) -> Result<Vec<i64>, String> {
    indices
        .iter()
        .map(|&idx| resolve_row_id_by_index(conn, table, idx))
        .collect()
}

fn duckdb_storage_type(conn: &Connection, table: &str, col: &str) -> Result<String, String> {
    conn.query_row(
        &format!(
            "SELECT data_type FROM information_schema.columns \
                  WHERE table_schema = 'main' AND table_name = '{}' AND column_name = '{}'",
            sql_escape_literal(table),
            sql_escape_literal(col)
        ),
        [],
        |row| row.get(0),
    )
    .map_err(|e| format!("Failed to read storage type for column '{col}': {e}"))
}

fn json_to_sql_literal(
    conn: &Connection,
    table: &str,
    col: &str,
    value: &serde_json::Value,
) -> Result<String, String> {
    if value.is_null() {
        return Ok("NULL".to_string());
    }

    let storage = duckdb_storage_type(conn, table, col)?;
    let upper = storage.to_uppercase();

    if upper.contains("ENUM") || storage.starts_with(super::YSSBI_ENUM_PREFIX) {
        let s = value
            .as_str()
            .ok_or_else(|| format!("ENUM column '{col}' requires string value"))?;
        return Ok(format!("'{}'", sql_escape_literal(s)));
    }

    match value {
        serde_json::Value::Bool(b) => Ok(if *b {
            "TRUE".to_string()
        } else {
            "FALSE".to_string()
        }),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(i.to_string())
            } else if let Some(f) = n.as_f64() {
                Ok(f.to_string())
            } else {
                Err(format!("Invalid number for column '{col}'"))
            }
        }
        serde_json::Value::String(s) => Ok(format!("'{}'", sql_escape_literal(s))),
        _ => Err(format!("Unsupported JSON value for column '{col}'")),
    }
}

pub fn fetch_cell_json(
    conn: &Connection,
    table: &str,
    row_id: i64,
    col: &str,
) -> Result<serde_json::Value, String> {
    let table_sql = duckdb_table_sql(table);
    let col_sql = quote_ident(col);
    let sql =
        format!("SELECT CAST({col_sql} AS VARCHAR) FROM {table_sql} WHERE {DUCKDB_ROWID_SQL} = ?");
    let raw: Option<String> = conn
        .query_row(&sql, [row_id], |row| row.get(0))
        .map_err(|e| format!("Failed to read cell ({col}, rowid={row_id}): {e}"))?;
    Ok(match raw {
        None => serde_json::Value::Null,
        Some(s) => serde_json::Value::String(s),
    })
}

pub fn fetch_row_json(
    conn: &Connection,
    table: &str,
    row_id: i64,
) -> Result<Vec<serde_json::Value>, String> {
    user_column_names(conn, table)?
        .into_iter()
        .map(|col| fetch_cell_json(conn, table, row_id, &col))
        .collect()
}

pub fn sql_edit_cell(
    conn: &Connection,
    table: &str,
    row_id: i64,
    col: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    let table_sql = duckdb_table_sql(table);
    let col_sql = quote_ident(col);
    let literal = json_to_sql_literal(conn, table, col, value)?;
    let sql = format!("UPDATE {table_sql} SET {col_sql} = {literal} WHERE {DUCKDB_ROWID_SQL} = ?");
    conn.execute(&sql, [row_id])
        .map_err(|e| format!("Failed to update cell: {e}"))?;
    Ok(())
}

pub fn sql_delete_rows(conn: &Connection, table: &str, row_ids: &[i64]) -> Result<(), String> {
    if row_ids.is_empty() {
        return Ok(());
    }
    for id in row_ids {
        if !try_delete_row_id(conn, table, *id)? {
            return Err(format!("No row deleted for rowid {id}"));
        }
    }
    Ok(())
}

fn try_delete_row_id(conn: &Connection, table: &str, row_id: i64) -> Result<bool, String> {
    let table_sql = duckdb_table_sql(table);
    let changed = conn
        .execute(
            &format!("DELETE FROM {table_sql} WHERE {DUCKDB_ROWID_SQL} = ?"),
            [row_id],
        )
        .map_err(|e| format!("Failed to delete row {row_id}: {e}"))?;
    Ok(changed > 0)
}

pub fn sql_add_row(conn: &Connection, table: &str, index: Option<usize>) -> Result<i64, String> {
    let table_sql = duckdb_table_sql(table);
    let user_cols = user_column_names(conn, table)?;
    if let Some(idx) = index {
        // DuckDB 无原生行序 insert；新行追加。index 仅作兼容记录。
        let _ = idx;
    }
    let insert_sql = if user_cols.is_empty() {
        format!("INSERT INTO {table_sql} DEFAULT VALUES")
    } else {
        let col_list = user_cols
            .iter()
            .map(|c| quote_ident(c))
            .collect::<Vec<_>>()
            .join(", ");
        let val_list = user_cols
            .iter()
            .map(|_| "NULL".to_string())
            .collect::<Vec<_>>()
            .join(", ");
        format!("INSERT INTO {table_sql} ({col_list}) VALUES ({val_list})")
    };
    conn.execute(&insert_sql, [])
        .map_err(|e| format!("Failed to insert row: {e}"))?;
    conn.query_row(
        &format!(
            "SELECT {DUCKDB_ROWID_SQL} FROM {table_sql} ORDER BY {DUCKDB_ROWID_SQL} DESC LIMIT 1"
        ),
        [],
        |row| row.get(0),
    )
    .map_err(|e| format!("Failed to read inserted rowid: {e}"))
}

fn row_data_predicate(
    conn: &Connection,
    table: &str,
    cols: &[String],
    data: &[serde_json::Value],
) -> Result<String, String> {
    let clauses = cols
        .iter()
        .enumerate()
        .map(|(idx, col)| {
            let col_sql = quote_ident(col);
            match data.get(idx).unwrap_or(&serde_json::Value::Null) {
                serde_json::Value::Null => Ok(format!("{col_sql} IS NULL")),
                value => {
                    let literal = json_to_sql_literal(conn, table, col, value)?;
                    Ok(format!("{col_sql} IS NOT DISTINCT FROM {literal}"))
                }
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(if clauses.is_empty() {
        "TRUE".to_string()
    } else {
        clauses.join(" AND ")
    })
}

fn resolve_row_id_by_data(
    conn: &Connection,
    table: &str,
    data: &[serde_json::Value],
) -> Result<Option<i64>, String> {
    let cols = user_column_names(conn, table)?;
    let predicate = row_data_predicate(conn, table, &cols, data)?;
    let table_sql = duckdb_table_sql(table);
    let sql = format!(
        "SELECT {DUCKDB_ROWID_SQL} FROM {table_sql} WHERE {predicate} ORDER BY {DUCKDB_ROWID_SQL} DESC LIMIT 1"
    );
    match conn.query_row(&sql, [], |row| row.get(0)) {
        Ok(id) => Ok(Some(id)),
        Err(duckdb::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Failed to resolve row by data: {e}")),
    }
}

fn resolve_all_null_row_id(conn: &Connection, table: &str) -> Result<Option<i64>, String> {
    let cols = user_column_names(conn, table)?;
    let data = vec![serde_json::Value::Null; cols.len()];
    resolve_row_id_by_data(conn, table, &data)
}

pub fn apply_edit_on_duckdb(
    duckdb_path: &Path,
    table: &str,
    op: &mut EditOperation,
) -> Result<(), String> {
    let conn = open_conn(duckdb_path)?;
    match op {
        EditOperation::EditCell {
            row_id: Some(rid),
            col,
            new_value,
            ..
        } => sql_edit_cell(&conn, table, *rid, col, new_value),
        EditOperation::EditCell {
            row,
            col,
            new_value,
            ..
        } => {
            let rid = resolve_row_id_by_index(&conn, table, *row)?;
            sql_edit_cell(&conn, table, rid, col, new_value)
        }
        EditOperation::DeleteRow {
            row_id,
            index,
            data,
            ..
        } => {
            if let Some(rid) = *row_id {
                if try_delete_row_id(&conn, table, rid)? {
                    return Ok(());
                }
            }

            if let Some(rid) = resolve_row_id_by_data(&conn, table, data)? {
                *row_id = Some(rid);
                return sql_delete_rows(&conn, table, &[rid]);
            }

            let rid = resolve_row_id_by_index(&conn, table, *index)?;
            *row_id = Some(rid);
            sql_delete_rows(&conn, table, &[rid])
        }
        EditOperation::AddRow { row_id, index, .. } => {
            let new_id = sql_add_row(&conn, table, Some(*index))?;
            *row_id = Some(new_id);
            Ok(())
        }
        EditOperation::AddColumn { name, dtype } => {
            let col_sql = quote_ident(name);
            let sql_type = polars_dtype_to_duckdb_sql(dtype);
            conn.execute_batch(&format!(
                "ALTER TABLE {} ADD COLUMN {} {};",
                duckdb_table_sql(table),
                col_sql,
                sql_type
            ))
            .map_err(|e| format!("Failed to add column: {e}"))
        }
        EditOperation::DeleteColumn { name, .. } => conn
            .execute_batch(&format!(
                "ALTER TABLE {} DROP COLUMN {};",
                duckdb_table_sql(table),
                quote_ident(name)
            ))
            .map_err(|e| format!("Failed to drop column: {e}")),
        EditOperation::RenameColumn { old_name, new_name } => conn
            .execute_batch(&format!(
                "ALTER TABLE {} RENAME COLUMN {} TO {};",
                duckdb_table_sql(table),
                quote_ident(old_name),
                quote_ident(new_name)
            ))
            .map_err(|e| format!("Failed to rename column: {e}")),
        EditOperation::CastColumn { col, new_dtype, .. } => {
            let sql_type = polars_dtype_to_duckdb_sql(new_dtype);
            conn.execute_batch(&format!(
                "ALTER TABLE {} ALTER COLUMN {} TYPE {};",
                duckdb_table_sql(table),
                quote_ident(col),
                sql_type
            ))
            .map_err(|e| format!("Failed to cast column: {e}"))
        }
    }
}

pub fn reverse_edit_on_duckdb(
    duckdb_path: &Path,
    table: &str,
    op: &mut EditOperation,
) -> Result<(), String> {
    let conn = open_conn(duckdb_path)?;
    match op {
        EditOperation::EditCell {
            row_id,
            row,
            col,
            old_value,
            ..
        } => {
            let rid = match row_id {
                Some(id) => *id,
                None => resolve_row_id_by_index(&conn, table, *row)?,
            };
            sql_edit_cell(&conn, table, rid, col, old_value)
        }
        EditOperation::AddRow { row_id, index, .. } => {
            if let Some(id) = *row_id {
                if try_delete_row_id(&conn, table, id)? {
                    return Ok(());
                }
            }

            let rid = resolve_all_null_row_id(&conn, table)?
                .unwrap_or(resolve_row_id_by_index(&conn, table, *index)?);
            *row_id = Some(rid);
            sql_delete_rows(&conn, table, &[rid])
        }
        EditOperation::DeleteRow {
            row_id,
            index: _,
            data,
            ..
        } => {
            let new_id = sql_add_row(&conn, table, None)?;
            *row_id = Some(new_id);
            let cols = user_column_names(&conn, table)?;
            for (col_idx, val) in data.iter().enumerate() {
                if col_idx < cols.len() {
                    sql_edit_cell(&conn, table, new_id, &cols[col_idx], val)?;
                }
            }
            if let Some(rid) = resolve_row_id_by_data(&conn, table, data)? {
                *row_id = Some(rid);
            }
            Ok(())
        }
        EditOperation::AddColumn { name, .. } => {
            let mut temp = EditOperation::DeleteColumn {
                name: name.clone(),
                data: vec![],
            };
            apply_edit_on_duckdb(duckdb_path, table, &mut temp)
        }
        EditOperation::DeleteColumn { name, data } => {
            let sql_type = polars_dtype_to_duckdb_sql("String");
            conn.execute_batch(&format!(
                "ALTER TABLE {} ADD COLUMN {} {};",
                duckdb_table_sql(table),
                quote_ident(name),
                sql_type
            ))
            .map_err(|e| format!("Failed to restore column: {e}"))?;
            for (row_idx, val) in data.iter().enumerate() {
                if let Ok(rid) = resolve_row_id_by_index(&conn, table, row_idx) {
                    let _ = sql_edit_cell(&conn, table, rid, name, val);
                }
            }
            Ok(())
        }
        EditOperation::RenameColumn { old_name, new_name } => {
            let mut temp = EditOperation::RenameColumn {
                old_name: new_name.clone(),
                new_name: old_name.clone(),
            };
            apply_edit_on_duckdb(duckdb_path, table, &mut temp)
        }
        EditOperation::CastColumn { col, old_dtype, .. } => {
            let mut temp = EditOperation::CastColumn {
                col: col.clone(),
                old_data: vec![],
                old_dtype: old_dtype.clone(),
                new_dtype: old_dtype.clone(),
            };
            apply_edit_on_duckdb(duckdb_path, table, &mut temp)
        }
    }
}

fn polars_dtype_to_duckdb_sql(dtype: &str) -> &'static str {
    match dtype {
        "Boolean" => "BOOLEAN",
        "Int8" => "TINYINT",
        "Int16" => "SMALLINT",
        "Int32" => "INTEGER",
        "Int64" => "BIGINT",
        "UInt8" => "UTINYINT",
        "UInt16" => "USMALLINT",
        "UInt32" => "UINTEGER",
        "UInt64" => "UBIGINT",
        "Float32" => "REAL",
        "Float64" => "DOUBLE",
        "Date" => "DATE",
        "DateTime" => "TIMESTAMP",
        "Categorical" | "String" => "VARCHAR",
        _ => "VARCHAR",
    }
}

pub fn empty_edit_state() -> EditState {
    EditState {
        can_undo: false,
        can_redo: false,
        is_modified: false,
        undo_count: 0,
        redo_count: 0,
    }
}
