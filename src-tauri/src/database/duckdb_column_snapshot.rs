use std::hash::{Hash, Hasher};
use std::path::Path;

use duckdb::Connection;

use yss_duckdb::{
    DUCKDB_ROWID_SQL, duckdb_table_sql, editable_dtype_to_duckdb_sql, quote_duckdb_identifier,
    quote_duckdb_string_literal,
};

pub const MAX_DELETE_COLUMN_SNAPSHOT_ROWS: usize = 50_000;
pub const MAX_DELETE_COLUMN_SNAPSHOT_BYTES: usize = 16 * 1024 * 1024;

pub struct DuckDbColumnSnapshot {
    pub dtype: String,
    pub row_ids: Vec<i64>,
    pub row_fingerprints: Vec<u64>,
    pub data: Vec<serde_json::Value>,
}

fn open_conn(duckdb_path: &Path) -> Result<Connection, String> {
    Connection::open(duckdb_path).map_err(|e| e.to_string())
}

pub(super) fn user_column_names(conn: &Connection, table: &str) -> Result<Vec<String>, String> {
    let table_literal = quote_duckdb_string_literal(table);
    conn.prepare(&format!(
        "SELECT column_name FROM information_schema.columns \
         WHERE table_schema = 'main' AND table_name = {table_literal} \
         ORDER BY ordinal_position",
    ))
    .map_err(|e| e.to_string())?
    .query_map([], |row| row.get::<_, String>(0))
    .map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())
}

pub(super) fn duckdb_storage_type(
    conn: &Connection,
    table: &str,
    column: &str,
) -> Result<String, String> {
    let table_literal = quote_duckdb_string_literal(table);
    let column_literal = quote_duckdb_string_literal(column);
    conn.query_row(
        &format!(
            "SELECT data_type FROM information_schema.columns \
             WHERE table_schema = 'main' AND table_name = {table_literal} \
             AND column_name = {column_literal}"
        ),
        [],
        |row| row.get(0),
    )
    .map_err(|e| format!("Failed to read storage type for column '{column}': {e}"))
}

fn editable_dtype_from_duckdb_storage(storage: &str) -> Result<String, String> {
    let dtype = match storage.trim().to_uppercase().as_str() {
        "BOOLEAN" | "BOOL" => "Boolean",
        "TINYINT" => "Int8",
        "SMALLINT" => "Int16",
        "INTEGER" | "INT" => "Int32",
        "BIGINT" => "Int64",
        "UTINYINT" => "UInt8",
        "USMALLINT" => "UInt16",
        "UINTEGER" => "UInt32",
        "UBIGINT" => "UInt64",
        "FLOAT" | "REAL" => "Float32",
        "DOUBLE" => "Float64",
        "VARCHAR" | "TEXT" | "STRING" => "String",
        "DATE" => "Date",
        "TIMESTAMP" => "DateTime",
        _ => {
            return Err(format!(
                "DuckDB column type '{storage}' cannot be restored exactly after deletion"
            ));
        }
    };
    Ok(dtype.to_string())
}

fn snapshot_size_bytes(row_id: i64, value: &serde_json::Value) -> Result<usize, String> {
    let value_bytes = match value {
        serde_json::Value::Null => 1,
        serde_json::Value::String(value) => value.len(),
        other => other.to_string().len(),
    };
    std::mem::size_of_val(&row_id)
        .checked_add(value_bytes)
        .ok_or_else(|| "Delete-column snapshot size overflow".to_string())
}

fn identity_select(columns: &[String]) -> String {
    let select = columns
        .iter()
        .map(|name| format!("CAST({} AS VARCHAR)", quote_duckdb_identifier(name)))
        .collect::<Vec<_>>()
        .join(", ");
    if select.is_empty() {
        String::new()
    } else {
        format!(", {select}")
    }
}

fn capture_column_snapshot(
    conn: &Connection,
    table: &str,
    column: &str,
) -> Result<DuckDbColumnSnapshot, String> {
    let table_sql = duckdb_table_sql(table);
    let column_sql = quote_duckdb_identifier(column);
    let row_count: i64 = conn
        .query_row(&format!("SELECT COUNT(*) FROM {table_sql}"), [], |row| {
            row.get(0)
        })
        .map_err(|e| format!("Failed to count rows before deleting column '{column}': {e}"))?;
    let row_count = usize::try_from(row_count)
        .map_err(|_| format!("Invalid row count before deleting column '{column}'"))?;
    if row_count > MAX_DELETE_COLUMN_SNAPSHOT_ROWS {
        return Err(format!(
            "Cannot delete column '{column}': {row_count} rows exceed the reversible snapshot limit of {MAX_DELETE_COLUMN_SNAPSHOT_ROWS}"
        ));
    }

    let storage = duckdb_storage_type(conn, table, column)?;
    let dtype = editable_dtype_from_duckdb_storage(&storage)?;
    let mut identity_columns = user_column_names(conn, table)?
        .into_iter()
        .filter(|name| name != column)
        .collect::<Vec<_>>();
    identity_columns.sort();
    let identity_select = identity_select(&identity_columns);
    let sql = format!(
        "SELECT {DUCKDB_ROWID_SQL}, CAST({column_sql} AS VARCHAR){identity_select} \
         FROM {table_sql} ORDER BY {DUCKDB_ROWID_SQL}"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare snapshot for column '{column}': {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            let row_id = row.get::<_, i64>(0)?;
            let value = row.get::<_, Option<String>>(1)?;
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            let mut identity_bytes = 0usize;
            row_id.hash(&mut hasher);
            for (offset, name) in identity_columns.iter().enumerate() {
                let identity_value = row.get::<_, Option<String>>(offset + 2)?;
                identity_bytes = identity_bytes
                    .saturating_add(name.len())
                    .saturating_add(identity_value.as_ref().map_or(1, String::len));
                name.hash(&mut hasher);
                identity_value.hash(&mut hasher);
            }
            Ok((row_id, value, hasher.finish(), identity_bytes))
        })
        .map_err(|e| format!("Failed to read snapshot for column '{column}': {e}"))?;

    let mut row_ids = Vec::with_capacity(row_count);
    let mut row_fingerprints = Vec::with_capacity(row_count);
    let mut data = Vec::with_capacity(row_count);
    let mut snapshot_bytes = 0usize;
    for row in rows {
        let (row_id, value, fingerprint, identity_bytes) =
            row.map_err(|e| format!("Failed to read snapshot row for column '{column}': {e}"))?;
        let value = value
            .map(serde_json::Value::String)
            .unwrap_or(serde_json::Value::Null);
        snapshot_bytes = snapshot_bytes
            .checked_add(snapshot_size_bytes(row_id, &value)?)
            .and_then(|bytes| bytes.checked_add(std::mem::size_of_val(&fingerprint)))
            .and_then(|bytes| bytes.checked_add(identity_bytes))
            .ok_or_else(|| "Delete-column snapshot size overflow".to_string())?;
        if snapshot_bytes > MAX_DELETE_COLUMN_SNAPSHOT_BYTES {
            return Err(format!(
                "Cannot delete column '{column}': snapshot exceeds the {MAX_DELETE_COLUMN_SNAPSHOT_BYTES}-byte safety limit"
            ));
        }
        row_ids.push(row_id);
        row_fingerprints.push(fingerprint);
        data.push(value);
    }
    if data.len() != row_count {
        return Err(format!(
            "Cannot delete column '{column}': snapshot read {} of {row_count} rows",
            data.len()
        ));
    }

    Ok(DuckDbColumnSnapshot {
        dtype,
        row_ids,
        row_fingerprints,
        data,
    })
}

pub fn delete_column_with_snapshot(
    duckdb_path: &Path,
    table: &str,
    column: &str,
) -> Result<DuckDbColumnSnapshot, String> {
    let mut conn = open_conn(duckdb_path)?;
    let transaction = conn
        .transaction()
        .map_err(|e| format!("Failed to start delete-column transaction: {e}"))?;
    let snapshot = capture_column_snapshot(&transaction, table, column)?;
    transaction
        .execute_batch(&format!(
            "ALTER TABLE {} DROP COLUMN {};",
            duckdb_table_sql(table),
            quote_duckdb_identifier(column)
        ))
        .map_err(|e| format!("Failed to drop column: {e}"))?;
    transaction
        .commit()
        .map_err(|e| format!("Failed to commit column deletion: {e}"))?;
    Ok(snapshot)
}

fn snapshot_value_sql(value: &serde_json::Value, sql_type: &str) -> Result<String, String> {
    if value.is_null() {
        return Ok("NULL".to_string());
    }
    let literal = match value {
        serde_json::Value::Bool(value) => value.to_string().to_uppercase(),
        serde_json::Value::Number(value) => value.to_string(),
        serde_json::Value::String(value) => quote_duckdb_string_literal(value),
        _ => return Err("Unsupported value in delete-column snapshot".to_string()),
    };
    Ok(format!("CAST({literal} AS {sql_type})"))
}

fn current_row_fingerprints(
    conn: &Connection,
    table: &str,
    expected_rows: usize,
) -> Result<(Vec<i64>, Vec<u64>), String> {
    let table_sql = duckdb_table_sql(table);
    let current_rows: i64 = conn
        .query_row(&format!("SELECT COUNT(*) FROM {table_sql}"), [], |row| {
            row.get(0)
        })
        .map_err(|e| format!("Failed to validate snapshot row count: {e}"))?;
    if usize::try_from(current_rows).ok() != Some(expected_rows) {
        return Err("Table row count no longer matches the delete-column snapshot".to_string());
    }

    let mut identity_columns = user_column_names(conn, table)?;
    identity_columns.sort();
    let identity_select = identity_select(&identity_columns);
    let sql = format!(
        "SELECT {DUCKDB_ROWID_SQL}{identity_select} \
         FROM {table_sql} ORDER BY {DUCKDB_ROWID_SQL}"
    );
    let mut stmt = conn
        .prepare(&sql)
        .map_err(|e| format!("Failed to prepare row-identity validation: {e}"))?;
    let rows = stmt
        .query_map([], |row| {
            let row_id = row.get::<_, i64>(0)?;
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            let mut identity_bytes = 0usize;
            row_id.hash(&mut hasher);
            for (offset, name) in identity_columns.iter().enumerate() {
                let identity_value = row.get::<_, Option<String>>(offset + 1)?;
                identity_bytes = identity_bytes
                    .saturating_add(name.len())
                    .saturating_add(identity_value.as_ref().map_or(1, String::len));
                name.hash(&mut hasher);
                identity_value.hash(&mut hasher);
            }
            Ok((row_id, hasher.finish(), identity_bytes))
        })
        .map_err(|e| format!("Failed to validate row identities: {e}"))?;

    let mut row_ids = Vec::with_capacity(expected_rows);
    let mut fingerprints = Vec::with_capacity(expected_rows);
    let mut identity_bytes = 0usize;
    for row in rows {
        let (row_id, fingerprint, row_identity_bytes) =
            row.map_err(|e| format!("Failed to read row identity: {e}"))?;
        identity_bytes = identity_bytes
            .checked_add(row_identity_bytes)
            .ok_or_else(|| "Row-identity snapshot size overflow".to_string())?;
        if identity_bytes > MAX_DELETE_COLUMN_SNAPSHOT_BYTES {
            return Err("Row-identity validation exceeds the snapshot byte limit".to_string());
        }
        row_ids.push(row_id);
        fingerprints.push(fingerprint);
    }
    Ok((row_ids, fingerprints))
}

pub fn restore_deleted_column(
    duckdb_path: &Path,
    table: &str,
    name: &str,
    dtype: &str,
    row_ids: &[i64],
    row_fingerprints: &[u64],
    data: &[serde_json::Value],
) -> Result<(), String> {
    if row_ids.len() != data.len() || row_fingerprints.len() != data.len() {
        return Err(format!(
            "Cannot restore column '{name}': snapshot row IDs and values differ in length"
        ));
    }
    if data.len() > MAX_DELETE_COLUMN_SNAPSHOT_ROWS {
        return Err(format!(
            "Cannot restore column '{name}': snapshot exceeds the reversible row limit"
        ));
    }
    let mut snapshot_bytes = 0usize;
    for ((&row_id, &fingerprint), value) in row_ids.iter().zip(row_fingerprints).zip(data) {
        snapshot_bytes = snapshot_bytes
            .checked_add(snapshot_size_bytes(row_id, value)?)
            .and_then(|bytes| bytes.checked_add(std::mem::size_of_val(&fingerprint)))
            .ok_or_else(|| "Delete-column snapshot size overflow".to_string())?;
    }
    if snapshot_bytes > MAX_DELETE_COLUMN_SNAPSHOT_BYTES {
        return Err(format!(
            "Cannot restore column '{name}': snapshot exceeds the reversible byte limit"
        ));
    }

    let sql_type = editable_dtype_to_duckdb_sql(dtype)?;
    let table_sql = duckdb_table_sql(table);
    let column_sql = quote_duckdb_identifier(name);
    let mut conn = open_conn(duckdb_path)?;
    let transaction = conn
        .transaction()
        .map_err(|e| format!("Failed to start column-restore transaction: {e}"))?;
    let (current_row_ids, current_fingerprints) =
        current_row_fingerprints(&transaction, table, data.len())?;
    if current_row_ids.len() != row_ids.len() {
        return Err(format!(
            "Cannot restore column '{name}': table row count no longer matches its snapshot"
        ));
    }
    for (index, ((current_id, current_fingerprint), (expected_id, expected_fingerprint))) in
        current_row_ids
            .iter()
            .zip(&current_fingerprints)
            .zip(row_ids.iter().zip(row_fingerprints))
            .enumerate()
    {
        if current_id != expected_id || current_fingerprint != expected_fingerprint {
            return Err(format!(
                "Cannot restore column '{name}': rowid {expected_id} no longer matches snapshot row {index}"
            ));
        }
    }

    transaction
        .execute_batch(&format!(
            "ALTER TABLE {table_sql} ADD COLUMN {column_sql} {sql_type};"
        ))
        .map_err(|e| format!("Failed to restore column '{name}': {e}"))?;
    for (&row_id, value) in row_ids.iter().zip(data) {
        let value_sql = snapshot_value_sql(value, sql_type)?;
        let changed = transaction
            .execute(
                &format!(
                    "UPDATE {table_sql} SET {column_sql} = {value_sql} \
                     WHERE {DUCKDB_ROWID_SQL} = ?"
                ),
                [row_id],
            )
            .map_err(|e| format!("Failed to restore column '{name}' at rowid {row_id}: {e}"))?;
        if changed != 1 {
            return Err(format!(
                "Failed to restore column '{name}': rowid {row_id} was not updated"
            ));
        }
    }
    transaction
        .commit()
        .map_err(|e| format!("Failed to commit column restore: {e}"))
}
