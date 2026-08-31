//! Transactional DuckDB DataView editing without whole-table Polars materialization.
//!
//! Rows are addressed through DuckDB's built-in `rowid` pseudo-column, never a persisted user
//! column. Each public mutation either commits the complete operation or leaves the table intact.

use std::collections::HashSet;
use std::path::Path;

use duckdb::Connection;

use crate::column_snapshot::{duckdb_storage_type, restore_deleted_column, user_column_names};
use crate::{
    DUCKDB_ROWID_SQL, DuckDbColumnMeta, YSSBI_ENUM_PREFIX, duckdb_table_sql,
    editable_dtype_to_duckdb_sql, quote_duckdb_identifier, quote_duckdb_string_literal,
    read_table_meta,
};
use yss_database_edit::EditOperation;

pub const MAX_IN_MEMORY_EDIT_ROWS: usize = 50_000;
pub const MAX_GET_DATAFRAME_ROWS: usize = 500_000;

pub fn should_use_in_memory_editing(row_count: usize) -> bool {
    row_count <= MAX_IN_MEMORY_EDIT_ROWS
}

fn open_conn(duckdb_path: &Path) -> Result<Connection, String> {
    Connection::open(duckdb_path).map_err(|e| e.to_string())
}

pub fn refresh_duckdb_meta(
    duckdb_path: &Path,
    table: &str,
) -> Result<(usize, Vec<DuckDbColumnMeta>), String> {
    let meta = read_table_meta(duckdb_path, table)?;
    Ok((meta.row_count, meta.columns))
}

fn resolve_row_id_by_index(
    conn: &Connection,
    table: &str,
    row_index: usize,
) -> Result<i64, String> {
    let table_sql = duckdb_table_sql(table);
    let offset = i64::try_from(row_index)
        .map_err(|_| format!("Row index {row_index} exceeds DuckDB's supported offset range"))?;
    conn.query_row(
        &format!(
            "SELECT {DUCKDB_ROWID_SQL} FROM {table_sql} ORDER BY {DUCKDB_ROWID_SQL} LIMIT 1 OFFSET ?"
        ),
        [offset],
        |row| row.get(0),
    )
    .map_err(|e| format!("Failed to resolve rowid at index {row_index}: {e}"))
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

    if upper.contains("ENUM") || storage.starts_with(YSSBI_ENUM_PREFIX) {
        let s = value
            .as_str()
            .ok_or_else(|| format!("ENUM column '{col}' requires string value"))?;
        return Ok(quote_duckdb_string_literal(s));
    }

    match value {
        serde_json::Value::Bool(b) => Ok(if *b {
            "TRUE".to_string()
        } else {
            "FALSE".to_string()
        }),
        // `Number::to_string` preserves unsigned integers above `i64::MAX`; routing those values
        // through `as_f64` first would silently round valid DuckDB `UBIGINT` inputs.
        serde_json::Value::Number(number) => Ok(number.to_string()),
        serde_json::Value::String(s) => Ok(quote_duckdb_string_literal(s)),
        _ => Err(format!("Unsupported JSON value for column '{col}'")),
    }
}

fn fetch_cell_json(
    conn: &Connection,
    table: &str,
    row_id: i64,
    col: &str,
) -> Result<serde_json::Value, String> {
    let table_sql = duckdb_table_sql(table);
    let col_sql = quote_duckdb_identifier(col);
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

fn fetch_row_json(
    conn: &Connection,
    table: &str,
    row_id: i64,
) -> Result<Vec<serde_json::Value>, String> {
    user_column_names(conn, table)?
        .into_iter()
        .map(|col| fetch_cell_json(conn, table, row_id, &col))
        .collect()
}

fn sql_edit_cell(
    conn: &Connection,
    table: &str,
    row_id: i64,
    col: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    let table_sql = duckdb_table_sql(table);
    let col_sql = quote_duckdb_identifier(col);
    let literal = json_to_sql_literal(conn, table, col, value)?;
    let sql = format!("UPDATE {table_sql} SET {col_sql} = {literal} WHERE {DUCKDB_ROWID_SQL} = ?");
    let changed = conn
        .execute(&sql, [row_id])
        .map_err(|e| format!("Failed to update cell: {e}"))?;
    match changed {
        1 => Ok(()),
        0 => Err(format!("No row updated for rowid {row_id}")),
        count => Err(format!(
            "Expected one row update for rowid {row_id}, but DuckDB reported {count}"
        )),
    }
}

fn sql_delete_rows(conn: &Connection, table: &str, row_ids: &[i64]) -> Result<(), String> {
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
    match changed {
        0 => Ok(false),
        1 => Ok(true),
        count => Err(format!(
            "Expected at most one row deletion for rowid {row_id}, but DuckDB reported {count}"
        )),
    }
}

fn sql_add_row(conn: &Connection, table: &str) -> Result<i64, String> {
    let table_sql = duckdb_table_sql(table);
    let user_cols = user_column_names(conn, table)?;
    let insert_sql = if user_cols.is_empty() {
        format!("INSERT INTO {table_sql} DEFAULT VALUES")
    } else {
        let col_list = user_cols
            .iter()
            .map(|column| quote_duckdb_identifier(column))
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
            let col_sql = quote_duckdb_identifier(col);
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

fn normalize_delete_targets(
    indices: &[usize],
    row_ids: Option<&[i64]>,
) -> Result<Vec<(usize, Option<i64>)>, String> {
    if let Some(row_ids) = row_ids
        && row_ids.len() != indices.len()
    {
        return Err("rowIds length must match indices".to_string());
    }

    let mut targets = indices
        .iter()
        .copied()
        .enumerate()
        .map(|(position, index)| (index, row_ids.map(|ids| ids[position])))
        .collect::<Vec<_>>();
    targets.sort_unstable_by_key(|(index, _)| *index);

    let mut normalized: Vec<(usize, Option<i64>)> = Vec::with_capacity(targets.len());
    for target in targets {
        if let Some(previous) = normalized.last()
            && previous.0 == target.0
        {
            if previous.1 != target.1 {
                return Err(format!(
                    "Row index {} was paired with conflicting row IDs",
                    target.0
                ));
            }
            continue;
        }
        normalized.push(target);
    }
    Ok(normalized)
}

pub fn edit_cell_with_operation(
    duckdb_path: &Path,
    table: &str,
    row: usize,
    row_id: Option<i64>,
    column: &str,
    new_value: serde_json::Value,
) -> Result<EditOperation, String> {
    let mut conn = open_conn(duckdb_path)?;
    let transaction = conn
        .transaction()
        .map_err(|error| format!("Failed to start cell-edit transaction: {error}"))?;
    let row_id = match row_id {
        Some(row_id) => row_id,
        None => resolve_row_id_by_index(&transaction, table, row)?,
    };
    let old_value = fetch_cell_json(&transaction, table, row_id, column)?;
    let mut operation = EditOperation::EditCell {
        row,
        row_id: Some(row_id),
        col: column.to_string(),
        old_value,
        new_value,
    };
    apply_edit_on_connection(&transaction, table, &mut operation)?;
    transaction
        .commit()
        .map_err(|error| format!("Failed to commit cell edit: {error}"))?;
    Ok(operation)
}

pub fn add_row_with_operation(
    duckdb_path: &Path,
    table: &str,
    index: usize,
) -> Result<EditOperation, String> {
    let mut conn = open_conn(duckdb_path)?;
    let transaction = conn
        .transaction()
        .map_err(|error| format!("Failed to start row-add transaction: {error}"))?;
    let mut operation = EditOperation::AddRow {
        index,
        row_id: None,
    };
    apply_edit_on_connection(&transaction, table, &mut operation)?;
    transaction
        .commit()
        .map_err(|error| format!("Failed to commit row addition: {error}"))?;
    Ok(operation)
}

pub fn delete_rows_with_operations(
    duckdb_path: &Path,
    table: &str,
    indices: &[usize],
    row_ids: Option<&[i64]>,
) -> Result<Vec<EditOperation>, String> {
    let targets = normalize_delete_targets(indices, row_ids)?;
    if targets.is_empty() {
        return Ok(Vec::new());
    }

    let mut conn = open_conn(duckdb_path)?;
    let transaction = conn
        .transaction()
        .map_err(|error| format!("Failed to start row-delete transaction: {error}"))?;
    let mut seen_row_ids = HashSet::with_capacity(targets.len());
    let mut operations = Vec::with_capacity(targets.len());
    for (index, supplied_row_id) in targets {
        let row_id = match supplied_row_id {
            Some(row_id) => row_id,
            None => resolve_row_id_by_index(&transaction, table, index)?,
        };
        if !seen_row_ids.insert(row_id) {
            return Err(format!("rowid {row_id} was selected more than once"));
        }
        let data = fetch_row_json(&transaction, table, row_id)?;
        operations.push(EditOperation::DeleteRow {
            index,
            row_id: Some(row_id),
            data,
        });
    }
    for operation in &mut operations {
        apply_edit_on_connection(&transaction, table, operation)?;
    }
    transaction
        .commit()
        .map_err(|error| format!("Failed to commit row deletion: {error}"))?;
    Ok(operations)
}

fn apply_edit_on_connection(
    conn: &Connection,
    table: &str,
    op: &mut EditOperation,
) -> Result<(), String> {
    match op {
        EditOperation::EditCell {
            row_id: Some(rid),
            col,
            new_value,
            ..
        } => sql_edit_cell(conn, table, *rid, col, new_value),
        EditOperation::EditCell {
            row,
            col,
            new_value,
            ..
        } => {
            let rid = resolve_row_id_by_index(conn, table, *row)?;
            sql_edit_cell(conn, table, rid, col, new_value)
        }
        EditOperation::DeleteRow {
            row_id,
            index,
            data,
            ..
        } => {
            if let Some(rid) = *row_id
                && try_delete_row_id(conn, table, rid)?
            {
                return Ok(());
            }

            if let Some(rid) = resolve_row_id_by_data(conn, table, data)? {
                *row_id = Some(rid);
                return sql_delete_rows(conn, table, &[rid]);
            }

            let rid = resolve_row_id_by_index(conn, table, *index)?;
            *row_id = Some(rid);
            sql_delete_rows(conn, table, &[rid])
        }
        EditOperation::AddRow { row_id, .. } => {
            let new_id = sql_add_row(conn, table)?;
            *row_id = Some(new_id);
            Ok(())
        }
        EditOperation::AddColumn { name, dtype } => {
            let col_sql = quote_duckdb_identifier(name);
            let sql_type = editable_dtype_to_duckdb_sql(dtype)?;
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
                quote_duckdb_identifier(name)
            ))
            .map_err(|e| format!("Failed to drop column: {e}")),
        EditOperation::RenameColumn { old_name, new_name } => conn
            .execute_batch(&format!(
                "ALTER TABLE {} RENAME COLUMN {} TO {};",
                duckdb_table_sql(table),
                quote_duckdb_identifier(old_name),
                quote_duckdb_identifier(new_name)
            ))
            .map_err(|e| format!("Failed to rename column: {e}")),
        EditOperation::CastColumn { col, new_dtype, .. } => {
            let sql_type = editable_dtype_to_duckdb_sql(new_dtype)?;
            conn.execute_batch(&format!(
                "ALTER TABLE {} ALTER COLUMN {} TYPE {};",
                duckdb_table_sql(table),
                quote_duckdb_identifier(col),
                sql_type
            ))
            .map_err(|e| format!("Failed to cast column: {e}"))
        }
    }
}

pub fn apply_edit_on_duckdb(
    duckdb_path: &Path,
    table: &str,
    operation: &mut EditOperation,
) -> Result<(), String> {
    let original = operation.clone();
    let mut conn = open_conn(duckdb_path)?;
    let transaction = conn
        .transaction()
        .map_err(|error| format!("Failed to start edit transaction: {error}"))?;
    if let Err(error) = apply_edit_on_connection(&transaction, table, operation) {
        *operation = original;
        return Err(error);
    }
    if let Err(error) = transaction.commit() {
        *operation = original;
        return Err(format!("Failed to commit edit: {error}"));
    }
    Ok(())
}

fn reverse_edit_on_connection(
    conn: &Connection,
    table: &str,
    op: &mut EditOperation,
) -> Result<(), String> {
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
                None => resolve_row_id_by_index(conn, table, *row)?,
            };
            sql_edit_cell(conn, table, rid, col, old_value)
        }
        EditOperation::AddRow { row_id, index, .. } => {
            if let Some(id) = *row_id
                && try_delete_row_id(conn, table, id)?
            {
                return Ok(());
            }

            let rid = match resolve_all_null_row_id(conn, table)? {
                Some(row_id) => row_id,
                None => resolve_row_id_by_index(conn, table, *index)?,
            };
            *row_id = Some(rid);
            sql_delete_rows(conn, table, &[rid])
        }
        EditOperation::DeleteRow { row_id, data, .. } => {
            let new_id = sql_add_row(conn, table)?;
            *row_id = Some(new_id);
            let columns = user_column_names(conn, table)?;
            for (column_index, value) in data.iter().enumerate() {
                if let Some(column) = columns.get(column_index) {
                    sql_edit_cell(conn, table, new_id, column, value)?;
                }
            }
            if let Some(resolved_id) = resolve_row_id_by_data(conn, table, data)? {
                *row_id = Some(resolved_id);
            }
            Ok(())
        }
        EditOperation::AddColumn { name, .. } => conn
            .execute_batch(&format!(
                "ALTER TABLE {} DROP COLUMN {};",
                duckdb_table_sql(table),
                quote_duckdb_identifier(name)
            ))
            .map_err(|error| format!("Failed to reverse column addition: {error}")),
        EditOperation::DeleteColumn { .. } => {
            Err("Delete-column reversal requires its bounded snapshot transaction".to_string())
        }
        EditOperation::RenameColumn { old_name, new_name } => conn
            .execute_batch(&format!(
                "ALTER TABLE {} RENAME COLUMN {} TO {};",
                duckdb_table_sql(table),
                quote_duckdb_identifier(new_name),
                quote_duckdb_identifier(old_name)
            ))
            .map_err(|error| format!("Failed to reverse column rename: {error}")),
        EditOperation::CastColumn { col, old_dtype, .. } => {
            let sql_type = editable_dtype_to_duckdb_sql(old_dtype)?;
            conn.execute_batch(&format!(
                "ALTER TABLE {} ALTER COLUMN {} TYPE {};",
                duckdb_table_sql(table),
                quote_duckdb_identifier(col),
                sql_type
            ))
            .map_err(|error| format!("Failed to reverse column cast: {error}"))
        }
    }
}

pub fn reverse_edit_on_duckdb(
    duckdb_path: &Path,
    table: &str,
    operation: &mut EditOperation,
) -> Result<(), String> {
    if let EditOperation::DeleteColumn {
        name,
        dtype,
        row_ids,
        row_fingerprints,
        data,
    } = operation
    {
        return restore_deleted_column(
            duckdb_path,
            table,
            name,
            dtype,
            row_ids,
            row_fingerprints,
            data,
        );
    }

    let original = operation.clone();
    let mut conn = open_conn(duckdb_path)?;
    let transaction = conn
        .transaction()
        .map_err(|error| format!("Failed to start reverse-edit transaction: {error}"))?;
    if let Err(error) = reverse_edit_on_connection(&transaction, table, operation) {
        *operation = original;
        return Err(error);
    }
    if let Err(error) = transaction.commit() {
        *operation = original;
        return Err(format!("Failed to commit reverse edit: {error}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DATABASE: AtomicU64 = AtomicU64::new(0);

    struct TestDatabase(PathBuf);

    impl TestDatabase {
        fn create(label: &str, sql: &str) -> Self {
            let sequence = NEXT_DATABASE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "yssbi-duckdb-edit-{label}-{}-{sequence}.duckdb",
                std::process::id()
            ));
            let connection = Connection::open(&path).expect("open test database");
            connection.execute_batch(sql).expect("create test fixture");
            drop(connection);
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn connection(&self) -> Connection {
            Connection::open(&self.0).expect("open test database")
        }
    }

    impl Drop for TestDatabase {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    fn row_ids(connection: &Connection, table: &str) -> Vec<i64> {
        connection
            .prepare(&format!(
                "SELECT {DUCKDB_ROWID_SQL} FROM {} ORDER BY {DUCKDB_ROWID_SQL}",
                duckdb_table_sql(table)
            ))
            .expect("prepare row IDs")
            .query_map([], |row| row.get(0))
            .expect("query row IDs")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect row IDs")
    }

    #[test]
    fn delete_rows_keeps_unsorted_indices_paired_with_their_row_ids() {
        let database = TestDatabase::create(
            "paired-delete",
            "CREATE TABLE records (value VARCHAR); \
             INSERT INTO records VALUES ('a'), ('b'), ('c');",
        );
        let connection = database.connection();
        let ids = row_ids(&connection, "records");
        drop(connection);

        let operations = delete_rows_with_operations(
            database.path(),
            "records",
            &[2, 0],
            Some(&[ids[2], ids[0]]),
        )
        .expect("delete selected rows");

        assert_eq!(
            operations,
            vec![
                EditOperation::DeleteRow {
                    index: 0,
                    row_id: Some(ids[0]),
                    data: vec![serde_json::json!("a")],
                },
                EditOperation::DeleteRow {
                    index: 2,
                    row_id: Some(ids[2]),
                    data: vec![serde_json::json!("c")],
                },
            ]
        );
        let remaining: String = database
            .connection()
            .query_row("SELECT value FROM records", [], |row| row.get(0))
            .expect("remaining row");
        assert_eq!(remaining, "b");
    }

    #[test]
    fn multi_row_delete_rolls_back_when_a_later_delete_fails() {
        let database = TestDatabase::create(
            "atomic-delete",
            "CREATE TABLE parent_records (id INTEGER PRIMARY KEY, value VARCHAR); \
             CREATE TABLE child_records (parent_id INTEGER REFERENCES parent_records(id)); \
             INSERT INTO parent_records VALUES (1, 'free'), (2, 'referenced'); \
             INSERT INTO child_records VALUES (2);",
        );
        let connection = database.connection();
        let ids = row_ids(&connection, "parent_records");
        drop(connection);

        let error =
            delete_rows_with_operations(database.path(), "parent_records", &[0, 1], Some(&ids))
                .expect_err("referenced row must reject the transaction");

        assert!(error.to_lowercase().contains("constraint"));
        let count: i64 = database
            .connection()
            .query_row("SELECT COUNT(*) FROM parent_records", [], |row| row.get(0))
            .expect("parent row count");
        assert_eq!(count, 2, "the earlier deletion must be rolled back");
    }

    #[test]
    fn add_row_reverse_uses_a_found_null_row_without_evaluating_index_fallback() {
        let database = TestDatabase::create(
            "lazy-add-row-reverse",
            "CREATE TABLE records (value BIGINT); INSERT INTO records VALUES (NULL);",
        );
        let mut operation = EditOperation::AddRow {
            index: usize::MAX,
            row_id: None,
        };

        reverse_edit_on_duckdb(database.path(), "records", &mut operation)
            .expect("reverse the null row without resolving the impossible index");

        let count: i64 = database
            .connection()
            .query_row("SELECT COUNT(*) FROM records", [], |row| row.get(0))
            .expect("row count");
        assert_eq!(count, 0);
    }

    #[test]
    fn failed_edit_restores_the_operation_and_leaves_the_table_unchanged() {
        let database = TestDatabase::create(
            "failed-edit",
            "CREATE TABLE records (value BIGINT); INSERT INTO records VALUES (7);",
        );
        let mut operation = EditOperation::EditCell {
            row: 0,
            row_id: Some(9_999),
            col: "value".to_string(),
            old_value: serde_json::json!(7),
            new_value: serde_json::json!(8),
        };
        let original = operation.clone();

        let error = apply_edit_on_duckdb(database.path(), "records", &mut operation)
            .expect_err("missing rowid must fail");

        assert!(error.contains("No row updated"));
        assert_eq!(operation, original);
        let value: i64 = database
            .connection()
            .query_row("SELECT value FROM records", [], |row| row.get(0))
            .expect("unchanged value");
        assert_eq!(value, 7);
    }

    #[test]
    fn edit_cell_preserves_the_full_unsigned_json_integer() {
        let database = TestDatabase::create(
            "unsigned-integer",
            "CREATE TABLE records (value UBIGINT); INSERT INTO records VALUES (0);",
        );

        edit_cell_with_operation(
            database.path(),
            "records",
            0,
            None,
            "value",
            serde_json::json!(u64::MAX),
        )
        .expect("write the exact unsigned integer");

        let value: u64 = database
            .connection()
            .query_row("SELECT value FROM records", [], |row| row.get(0))
            .expect("read the exact unsigned integer");
        assert_eq!(value, u64::MAX);
    }
}
