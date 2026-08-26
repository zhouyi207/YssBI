use super::DatabaseDecl;
use super::DatabaseEngine;
use super::DatabaseExportFormat;
use super::DatabaseState;

use super::{
    EditHistory, EditOperation, EditState, anyvalue_to_json, apply_operation, capture_column_data,
    capture_row_data, cast_column as sci_cast_column, dtype_from_string, dtype_to_string,
    export_dataframe, export_duckdb_table, reverse_operation,
};
use super::{
    PageQueryResult, apply_edit_on_duckdb, delete_column_with_snapshot, duckdb_table_sql,
    fetch_cell_json, fetch_row_json, ingest_dataframe_to_duckdb, query_columns_to_dataframe,
    query_page_with_rowids, query_to_dataframe_for_table, refresh_duckdb_meta,
    resolve_row_id_by_index, resolve_row_ids_by_indices, reverse_edit_on_duckdb,
    should_use_in_memory_editing, sql_add_row,
};
use super::{
    compute_all_column_distributions_duckdb, compute_all_column_stats_duckdb,
    compute_dataset_overview_duckdb,
};
use crate::database::database_schema::{dataframe_to_schema, duckdb_columns_to_schema};
use crate::graph::schema::DataSchema;
use polars::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
pub struct DatabaseInstance {
    pub decl: DatabaseDecl,
    pub state: DatabaseState,
}

impl DatabaseInstance {
    pub fn data_schema(&mut self) -> PolarsResult<DataSchema> {
        match &self.state {
            DatabaseState::DuckDb { columns, .. } => Ok(duckdb_columns_to_schema(columns)),
            DatabaseState::Loaded { dataframe, .. } => Ok(dataframe_to_schema(dataframe)),
            DatabaseState::Failed { error } => Err(PolarsError::ComputeError(error.clone().into())),
        }
    }

    /// 分页读取行数据。DuckDB 走 `LIMIT/OFFSET`，不触发整表物化。
    pub fn query_page(&mut self, offset: usize, limit: usize) -> PolarsResult<DataFrame> {
        self.query_page_with_rowids(offset, limit)
            .map(|page| page.dataframe)
    }

    /// 分页读取，附带 DuckDB `rowid`（供 DataView 编辑）。
    pub fn query_page_with_rowids(
        &mut self,
        offset: usize,
        limit: usize,
    ) -> PolarsResult<PageQueryResult> {
        match &self.state {
            DatabaseState::DuckDb {
                duckdb_path, table, ..
            } => query_page_with_rowids(Path::new(duckdb_path), table, offset, limit)
                .map_err(|e| PolarsError::ComputeError(e.into())),
            DatabaseState::Loaded { dataframe, .. } => {
                let total = dataframe.height();
                let start = offset.min(total);
                let count = limit.min(total.saturating_sub(start));
                let slice = dataframe.slice(start as i64, count);
                let row_ids = (start as i64..start as i64 + count as i64).collect();
                Ok(PageQueryResult {
                    dataframe: slice,
                    row_ids,
                })
            }
            DatabaseState::Failed { error } => Err(PolarsError::ComputeError(error.clone().into())),
        }
    }

    /// 按列名列表加载窄 DataFrame。DuckDB 走 `SELECT col1, col2, ...`，不整表物化。
    pub fn load_columns(&mut self, columns: &[&str]) -> PolarsResult<DataFrame> {
        match &self.state {
            DatabaseState::DuckDb {
                duckdb_path, table, ..
            } => query_columns_to_dataframe(Path::new(duckdb_path), table, columns)
                .map_err(|e| PolarsError::ComputeError(e.into())),
            DatabaseState::Loaded { dataframe, .. } => {
                Ok(dataframe.clone().select(columns.to_vec())?)
            }
            DatabaseState::Failed { error } => Err(PolarsError::ComputeError(error.clone().into())),
        }
    }

    /// 加载单列 Series，优先走列裁剪路径。
    pub fn load_column_series(&mut self, column: &str) -> PolarsResult<Series> {
        let df = self.load_columns(&[column])?;
        Ok(df.column(column)?.clone().take_materialized_series())
    }

    /// 列出列名（不触发整表加载）。
    pub fn list_column_names(&mut self) -> PolarsResult<Vec<String>> {
        Ok(self
            .data_schema()?
            .columns
            .iter()
            .map(|c| c.name.clone())
            .collect())
    }

    pub fn export_to_path(&self, path: &Path, format: DatabaseExportFormat) -> Result<(), String> {
        match &self.state {
            DatabaseState::DuckDb {
                duckdb_path, table, ..
            } => export_duckdb_table(Path::new(duckdb_path), table, path, format),
            DatabaseState::Loaded { dataframe, .. } => {
                let mut dataframe = dataframe.as_ref().clone();
                export_dataframe(&mut dataframe, path, format)
            }
            DatabaseState::Failed { error } => Err(error.clone()),
        }
    }

    /// 列统计：DuckDB 走 SQL 聚合，其它状态 fallback 到 Polars 整表。
    pub fn compute_column_stats(&mut self) -> PolarsResult<Vec<super::ColumnStats>> {
        match &self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                columns,
                row_count,
                ..
            } => {
                compute_all_column_stats_duckdb(Path::new(duckdb_path), table, columns, *row_count)
                    .map_err(|e| PolarsError::ComputeError(e.into()))
            }
            DatabaseState::Loaded { dataframe, .. } => {
                Ok(super::compute_all_column_stats(dataframe))
            }
            DatabaseState::Failed { error } => Err(PolarsError::ComputeError(error.clone().into())),
        }
    }

    /// 列分布：DuckDB 走 SQL 聚合。
    pub fn compute_column_distributions(&mut self) -> PolarsResult<Vec<super::ColumnDistribution>> {
        match &self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                columns,
                ..
            } => compute_all_column_distributions_duckdb(Path::new(duckdb_path), table, columns)
                .map_err(|e| PolarsError::ComputeError(e.into())),
            DatabaseState::Loaded { dataframe, .. } => {
                Ok(super::compute_all_column_distributions(dataframe))
            }
            DatabaseState::Failed { error } => Err(PolarsError::ComputeError(error.clone().into())),
        }
    }

    /// 数据集概览：DuckDB 用缓存元数据 + SQL null 统计。
    pub fn compute_dataset_overview(&mut self) -> PolarsResult<super::DatasetOverview> {
        match &self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                columns,
                row_count,
                ..
            } => {
                compute_dataset_overview_duckdb(Path::new(duckdb_path), table, columns, *row_count)
                    .map_err(|e| PolarsError::ComputeError(e.into()))
            }
            DatabaseState::Loaded { dataframe, .. } => {
                Ok(super::compute_dataset_overview(dataframe))
            }
            DatabaseState::Failed { error } => Err(PolarsError::ComputeError(error.clone().into())),
        }
    }

    pub fn ensure_loaded(&mut self) -> PolarsResult<&DataFrame> {
        if matches!(self.state, DatabaseState::DuckDb { .. }) {
            let row_count = match &self.state {
                DatabaseState::DuckDb { row_count, .. } => *row_count,
                _ => unreachable!(),
            };
            if !should_use_in_memory_editing(row_count) {
                return Err(PolarsError::ComputeError(
                    format!(
                        "Table has {row_count} rows; exceeds in-memory limit for full load. \
                         Use SQL editing or column-scoped graph nodes."
                    )
                    .into(),
                ));
            }
            let df = match &self.state {
                DatabaseState::DuckDb {
                    duckdb_path, table, ..
                } => {
                    let sql = format!("SELECT * FROM {}", duckdb_table_sql(table));
                    query_to_dataframe_for_table(Path::new(duckdb_path), &sql, Some(table))
                        .map_err(|e| PolarsError::ComputeError(e.into()))?
                }
                _ => unreachable!(),
            };
            let arc_df = Arc::new(df);

            self.state = DatabaseState::Loaded {
                dataframe: arc_df.clone(),
                original: arc_df,
                history: EditHistory::new(),
            };
        }

        match &self.state {
            DatabaseState::Loaded { dataframe, .. } => Ok(dataframe),
            DatabaseState::Failed { error } => Err(PolarsError::ComputeError(error.clone().into())),
            _ => Err(PolarsError::ComputeError(
                "Database must be DuckDb or Loaded before materialization".into(),
            )),
        }
    }

    pub fn edit_cell(
        &mut self,
        row: usize,
        col_name: &str,
        new_value: serde_json::Value,
        row_id: Option<i64>,
    ) -> Result<EditState, String> {
        if let DatabaseState::DuckDb {
            duckdb_path,
            table,
            history,
            ..
        } = &mut self.state
        {
            let path = PathBuf::from(duckdb_path.clone());
            let table_name = table.clone();
            let conn = duckdb::Connection::open(&path).map_err(|e| e.to_string())?;
            let rid = match row_id {
                Some(id) => id,
                None => resolve_row_id_by_index(&conn, &table_name, row)?,
            };
            let old_value = fetch_cell_json(&conn, &table_name, rid, col_name)?;
            drop(conn);
            let mut op = EditOperation::EditCell {
                row,
                row_id: Some(rid),
                col: col_name.to_string(),
                old_value,
                new_value,
            };
            apply_edit_on_duckdb(&path, &table_name, &mut op)?;
            history.push(op);
            return Ok(history.state());
        }

        self.ensure_loaded().map_err(|e| e.to_string())?;

        let (dataframe, history) = match &mut self.state {
            DatabaseState::Loaded {
                dataframe, history, ..
            } => (dataframe, history),
            _ => return Err("Database not loaded".into()),
        };

        let df = Arc::make_mut(dataframe);
        let col_idx = df
            .get_column_index(col_name)
            .ok_or_else(|| format!("Column '{}' not found", col_name))?;
        let old_value = df.columns()[col_idx]
            .get(row)
            .map(|v| anyvalue_to_json(v))
            .unwrap_or(serde_json::Value::Null);

        let op = EditOperation::EditCell {
            row,
            row_id,
            col: col_name.to_string(),
            old_value,
            new_value,
        };

        apply_operation(df, &op)?;
        history.push(op);
        Ok(history.state())
    }

    pub fn add_row(&mut self, index: Option<usize>) -> Result<EditState, String> {
        if let DatabaseState::DuckDb {
            duckdb_path,
            table,
            history,
            row_count,
            ..
        } = &mut self.state
        {
            let path = PathBuf::from(duckdb_path.clone());
            let table_name = table.clone();
            let conn = duckdb::Connection::open(&path).map_err(|e| e.to_string())?;
            let idx = index.unwrap_or(*row_count);
            let new_id = sql_add_row(&conn, &table_name)?;
            drop(conn);
            let op = EditOperation::AddRow {
                index: idx,
                row_id: Some(new_id),
            };
            history.push(op);
            *row_count += 1;
            return Ok(history.state());
        }

        self.ensure_loaded().map_err(|e| e.to_string())?;

        let (dataframe, history) = match &mut self.state {
            DatabaseState::Loaded {
                dataframe, history, ..
            } => (dataframe, history),
            _ => return Err("Database not loaded".into()),
        };

        let df = Arc::make_mut(dataframe);
        let idx = index.unwrap_or(df.height());
        let op = EditOperation::AddRow {
            index: idx,
            row_id: None,
        };

        apply_operation(df, &op)?;
        history.push(op);
        Ok(history.state())
    }

    pub fn delete_rows(
        &mut self,
        indices: &[usize],
        row_ids: Option<&[i64]>,
    ) -> Result<EditState, String> {
        if let DatabaseState::DuckDb {
            duckdb_path,
            table,
            history,
            row_count,
            ..
        } = &mut self.state
        {
            let path = PathBuf::from(duckdb_path.clone());
            let table_name = table.clone();
            let conn = duckdb::Connection::open(&path).map_err(|e| e.to_string())?;

            let mut sorted_indices = indices.to_vec();
            sorted_indices.sort_unstable();
            sorted_indices.dedup();

            let ids: Vec<i64> = if let Some(ids) = row_ids {
                if ids.len() != sorted_indices.len() {
                    return Err("rowIds length must match indices".into());
                }
                ids.to_vec()
            } else {
                resolve_row_ids_by_indices(&conn, &table_name, &sorted_indices)?
            };

            let mut ops = Vec::with_capacity(sorted_indices.len());
            for (&idx, &rid) in sorted_indices.iter().zip(ids.iter()) {
                let data = fetch_row_json(&conn, &table_name, rid)?;
                ops.push(EditOperation::DeleteRow {
                    index: idx,
                    row_id: Some(rid),
                    data,
                });
            }
            drop(conn);
            for mut op in ops {
                apply_edit_on_duckdb(&path, &table_name, &mut op)?;
                history.push(op);
            }
            *row_count = row_count.saturating_sub(sorted_indices.len());
            return Ok(history.state());
        }

        self.ensure_loaded().map_err(|e| e.to_string())?;

        let (dataframe, history) = match &mut self.state {
            DatabaseState::Loaded {
                dataframe, history, ..
            } => (dataframe, history),
            _ => return Err("Database not loaded".into()),
        };

        let df = Arc::make_mut(dataframe);
        let mut sorted_indices = indices.to_vec();
        sorted_indices.sort_unstable();
        sorted_indices.dedup();

        for (offset, &idx) in sorted_indices.iter().enumerate() {
            let actual_idx = idx - offset;
            let data = capture_row_data(df, actual_idx);
            let op = EditOperation::DeleteRow {
                index: actual_idx,
                row_id: None,
                data,
            };
            apply_operation(df, &op)?;
            history.push(op);
        }

        Ok(history.state())
    }

    pub fn add_column(&mut self, name: &str, dtype: &str) -> Result<EditState, String> {
        let dtype = dtype_to_string(&dtype_from_string(dtype)?)?;

        if let DatabaseState::DuckDb {
            duckdb_path,
            table,
            history,
            columns,
            ..
        } = &mut self.state
        {
            let mut op = EditOperation::AddColumn {
                name: name.to_string(),
                dtype: dtype.to_string(),
            };
            apply_edit_on_duckdb(Path::new(duckdb_path), table, &mut op)?;
            columns.push(super::DuckDbColumnMeta {
                name: name.to_string(),
                dtype: dtype.to_string(),
            });
            history.push(op);
            return Ok(history.state());
        }

        self.ensure_loaded().map_err(|e| e.to_string())?;

        let (dataframe, history) = match &mut self.state {
            DatabaseState::Loaded {
                dataframe, history, ..
            } => (dataframe, history),
            _ => return Err("Database not loaded".into()),
        };

        let df = Arc::make_mut(dataframe);
        let op = EditOperation::AddColumn {
            name: name.to_string(),
            dtype: dtype.to_string(),
        };

        apply_operation(df, &op)?;
        history.push(op);
        Ok(history.state())
    }

    pub fn delete_column(&mut self, name: &str) -> Result<EditState, String> {
        if let DatabaseState::DuckDb {
            duckdb_path,
            table,
            history,
            columns,
            ..
        } = &mut self.state
        {
            let snapshot = delete_column_with_snapshot(Path::new(duckdb_path), table, name)?;
            let op = EditOperation::DeleteColumn {
                name: name.to_string(),
                dtype: snapshot.dtype,
                row_ids: snapshot.row_ids,
                row_fingerprints: snapshot.row_fingerprints,
                data: snapshot.data,
            };
            columns.retain(|c| c.name != name);
            history.push(op);
            return Ok(history.state());
        }

        self.ensure_loaded().map_err(|e| e.to_string())?;

        let (dataframe, history) = match &mut self.state {
            DatabaseState::Loaded {
                dataframe, history, ..
            } => (dataframe, history),
            _ => return Err("Database not loaded".into()),
        };

        let df = Arc::make_mut(dataframe);
        let column = df
            .column(name)
            .map_err(|_| format!("Column '{name}' not found"))?;
        let dtype = dtype_to_string(column.dtype())?;
        let data = capture_column_data(df, name);
        let op = EditOperation::DeleteColumn {
            name: name.to_string(),
            dtype,
            row_ids: vec![],
            row_fingerprints: vec![],
            data,
        };

        apply_operation(df, &op)?;
        history.push(op);
        Ok(history.state())
    }

    pub fn rename_column(&mut self, old_name: &str, new_name: &str) -> Result<EditState, String> {
        if let DatabaseState::DuckDb {
            duckdb_path,
            table,
            history,
            columns,
            ..
        } = &mut self.state
        {
            let mut op = EditOperation::RenameColumn {
                old_name: old_name.to_string(),
                new_name: new_name.to_string(),
            };
            apply_edit_on_duckdb(Path::new(duckdb_path), table, &mut op)?;
            if let Some(col) = columns.iter_mut().find(|c| c.name == old_name) {
                col.name = new_name.to_string();
            }
            history.push(op);
            return Ok(history.state());
        }

        self.ensure_loaded().map_err(|e| e.to_string())?;

        let (dataframe, history) = match &mut self.state {
            DatabaseState::Loaded {
                dataframe, history, ..
            } => (dataframe, history),
            _ => return Err("Database not loaded".into()),
        };

        let df = Arc::make_mut(dataframe);
        let op = EditOperation::RenameColumn {
            old_name: old_name.to_string(),
            new_name: new_name.to_string(),
        };

        apply_operation(df, &op)?;
        history.push(op);
        Ok(history.state())
    }

    pub fn cast_column(
        &mut self,
        col_name: &str,
        new_dtype: &str,
        force: bool,
    ) -> Result<EditState, String> {
        let new_dtype = dtype_to_string(&dtype_from_string(new_dtype)?)?;
        if force && matches!(&self.state, DatabaseState::DuckDb { .. }) {
            return Err("DuckDB force casting is not supported".into());
        }

        if let DatabaseState::DuckDb {
            duckdb_path,
            table,
            history,
            columns,
            ..
        } = &mut self.state
        {
            let old_dtype = columns
                .iter()
                .find(|c| c.name == col_name)
                .map(|c| c.dtype.clone())
                .ok_or_else(|| format!("Column '{col_name}' not found"))?;
            let mut op = EditOperation::CastColumn {
                col: col_name.to_string(),
                old_data: vec![],
                old_dtype: old_dtype.clone(),
                new_dtype: new_dtype.clone(),
            };
            apply_edit_on_duckdb(Path::new(duckdb_path), table, &mut op)?;
            if let Some(col) = columns.iter_mut().find(|c| c.name == col_name) {
                col.dtype = new_dtype.clone();
            }
            history.push(op);
            return Ok(history.state());
        }

        self.ensure_loaded().map_err(|e| e.to_string())?;

        let (dataframe, history) = match &mut self.state {
            DatabaseState::Loaded {
                dataframe, history, ..
            } => (dataframe, history),
            _ => return Err("Database not loaded".into()),
        };

        let df = Arc::make_mut(dataframe);
        let old_data = capture_column_data(df, col_name);
        let col_idx = df
            .get_column_index(col_name)
            .ok_or_else(|| format!("Column '{}' not found", col_name))?;
        let old_dtype = dtype_to_string(df.columns()[col_idx].dtype())?;

        sci_cast_column(df, col_name, &new_dtype, force)?;

        let op = EditOperation::CastColumn {
            col: col_name.to_string(),
            old_data,
            old_dtype,
            new_dtype,
        };
        history.push(op);
        Ok(history.state())
    }

    pub fn undo_edit(&mut self) -> Result<EditState, String> {
        if let DatabaseState::DuckDb {
            duckdb_path,
            table,
            history,
            row_count,
            columns,
            ..
        } = &mut self.state
        {
            let mut op = history.pop_undo().ok_or("Nothing to undo")?;
            let path = PathBuf::from(duckdb_path.clone());
            let table_name = table.clone();
            if let Err(error) = reverse_edit_on_duckdb(&path, &table_name, &mut op) {
                history.push_undo(op);
                return Err(error);
            }
            history.push_redo(op);
            let (count, cols) = refresh_duckdb_meta(&path, &table_name)?;
            *row_count = count;
            *columns = cols;
            return Ok(history.state());
        }

        let (dataframe, history) = match &mut self.state {
            DatabaseState::Loaded {
                dataframe, history, ..
            } => (dataframe, history),
            _ => return Err("Database not loaded".into()),
        };

        let op = history.pop_undo().ok_or("Nothing to undo")?;
        let mut candidate = dataframe.as_ref().clone();
        if let Err(error) = reverse_operation(&mut candidate, &op) {
            history.push_undo(op);
            return Err(error);
        }
        *dataframe = Arc::new(candidate);
        history.push_redo(op);
        Ok(history.state())
    }

    pub fn redo_edit(&mut self) -> Result<EditState, String> {
        if let DatabaseState::DuckDb {
            duckdb_path,
            table,
            history,
            row_count,
            columns,
            ..
        } = &mut self.state
        {
            let mut op = history.pop_redo().ok_or("Nothing to redo")?;
            let path = PathBuf::from(duckdb_path.clone());
            let table_name = table.clone();
            if let Err(error) = apply_edit_on_duckdb(&path, &table_name, &mut op) {
                history.push_redo(op);
                return Err(error);
            }
            history.push_undo(op);
            let (count, cols) = refresh_duckdb_meta(&path, &table_name)?;
            *row_count = count;
            *columns = cols;
            return Ok(history.state());
        }

        let (dataframe, history) = match &mut self.state {
            DatabaseState::Loaded {
                dataframe, history, ..
            } => (dataframe, history),
            _ => return Err("Database not loaded".into()),
        };

        let op = history.pop_redo().ok_or("Nothing to redo")?;
        let mut candidate = dataframe.as_ref().clone();
        if let Err(error) = apply_operation(&mut candidate, &op) {
            history.push_redo(op);
            return Err(error);
        }
        *dataframe = Arc::new(candidate);
        history.push_undo(op);
        Ok(history.state())
    }

    pub fn save_changes(&mut self, project_root: Option<&Path>) -> Result<EditState, String> {
        if let DatabaseEngine::DuckDb { path, table } = &self.decl.engine {
            let root = project_root.ok_or_else(|| "请先打开或创建项目后再保存数据".to_string())?;
            let duckdb_abs = root.join(path);
            let table_id = table.clone();

            if let DatabaseState::DuckDb {
                duckdb_path,
                history,
                row_count,
                columns,
                ..
            } = &mut self.state
            {
                let (count, cols) = refresh_duckdb_meta(Path::new(duckdb_path), &table_id)?;
                *row_count = count;
                *columns = cols;
                history.clear();
                return Ok(EditState {
                    can_undo: false,
                    can_redo: false,
                    is_modified: false,
                    undo_count: 0,
                    redo_count: 0,
                });
            }

            self.ensure_loaded().map_err(|e| e.to_string())?;
            let meta = match &mut self.state {
                DatabaseState::Loaded { dataframe, .. } => {
                    ingest_dataframe_to_duckdb(Arc::make_mut(dataframe), &duckdb_abs, &table_id)?
                }
                _ => return Err("Database not loaded".into()),
            };
            self.state = DatabaseState::DuckDb {
                duckdb_path: duckdb_abs.to_string_lossy().to_string(),
                table: table_id,
                row_count: meta.row_count,
                columns: meta.columns,
                history: EditHistory::new(),
            };
            return Ok(EditState {
                can_undo: false,
                can_redo: false,
                is_modified: false,
                undo_count: 0,
                redo_count: 0,
            });
        }

        match &mut self.state {
            DatabaseState::Loaded {
                dataframe,
                original,
                history,
            } => {
                *original = dataframe.clone();
                history.clear();
                Ok(history.state())
            }
            _ => Err("Database not loaded".into()),
        }
    }

    pub fn edit_state(&self) -> EditState {
        match &self.state {
            DatabaseState::DuckDb { history, .. } | DatabaseState::Loaded { history, .. } => {
                history.state()
            }
            _ => EditState {
                can_undo: false,
                can_redo: false,
                is_modified: false,
                undo_count: 0,
                redo_count: 0,
            },
        }
    }
}
