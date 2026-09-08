use super::error::DatabaseExportError;
use crate::DatabaseState;
use yss_database_contract::{DatabaseDecl, DatabaseExportFormat};
use yss_database_edit::{EditOperation, EditState};

use polars::prelude::*;
use std::path::{Path, PathBuf};
use yss_database_schema::DatabaseSchemaFact;
use yss_dataset_profile::{ColumnDistribution, ColumnStats, DatasetOverview};
use yss_duckdb::{
    DatasetProfileColumnRef, DuckDbColumnMeta, PageQueryResult, add_row_with_operation,
    apply_edit_on_duckdb,
    compute_all_column_distributions as compute_all_column_distributions_duckdb,
    compute_all_column_stats as compute_all_column_stats_duckdb,
    compute_dataset_overview as compute_dataset_overview_duckdb, delete_column_with_snapshot,
    delete_rows_with_operations, edit_cell_with_operation, export_duckdb_table,
    query_columns_to_dataframe, query_page_with_rowids, refresh_duckdb_meta,
    reverse_edit_on_duckdb, write_display_name,
};
use yss_tabular_polars::{dtype_from_string, dtype_to_string};

fn duckdb_profile_columns(columns: &[DuckDbColumnMeta]) -> Vec<DatasetProfileColumnRef<'_>> {
    columns
        .iter()
        .map(|column| DatasetProfileColumnRef::new(&column.name, &column.dtype))
        .collect()
}

#[derive(Clone)]
pub struct DatabaseInstance {
    pub decl: DatabaseDecl,
    pub state: DatabaseState,
}

impl DatabaseInstance {
    pub fn data_schema(&mut self) -> PolarsResult<DatabaseSchemaFact> {
        let fact = match &self.state {
            DatabaseState::DuckDb { columns, .. } => {
                DatabaseSchemaFact::from_duckdb(&self.decl.id, columns)
            }
            DatabaseState::Failed { error } => {
                return Err(PolarsError::ComputeError(error.clone().into()));
            }
        }
        .map_err(|error| PolarsError::ComputeError(error.to_string().into()))?;

        Ok(fact)
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
            .columns()
            .iter()
            .map(|c| c.name().as_str().to_string())
            .collect())
    }

    pub fn export_to_path(
        &self,
        path: &Path,
        format: DatabaseExportFormat,
    ) -> Result<(), DatabaseExportError> {
        match &self.state {
            DatabaseState::DuckDb {
                duckdb_path, table, ..
            } => {
                export_duckdb_table(Path::new(duckdb_path), table, path, format).map_err(Into::into)
            }
            DatabaseState::Failed { .. } => Err(DatabaseExportError::unavailable()),
        }
    }

    pub fn rename_display_name(&mut self, name: &str) -> Result<EditState, String> {
        if let DatabaseState::DuckDb {
            duckdb_path, table, ..
        } = &self.state
        {
            write_display_name(Path::new(duckdb_path), table, name)?;
        }
        self.decl.name = name.to_owned().into_boxed_str();
        Ok(self.edit_state())
    }

    /// 列统计：DuckDB 走 SQL 聚合。
    pub fn compute_column_stats(&mut self) -> PolarsResult<Vec<ColumnStats>> {
        match &self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                columns,
                ..
            } => {
                let columns = duckdb_profile_columns(columns);
                compute_all_column_stats_duckdb(Path::new(duckdb_path), table, &columns)
                    .map_err(|e| PolarsError::ComputeError(e.into()))
            }
            DatabaseState::Failed { error } => Err(PolarsError::ComputeError(error.clone().into())),
        }
    }

    /// 列分布：DuckDB 走 SQL 聚合。
    pub fn compute_column_distributions(&mut self) -> PolarsResult<Vec<ColumnDistribution>> {
        match &self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                columns,
                ..
            } => {
                let columns = duckdb_profile_columns(columns);
                compute_all_column_distributions_duckdb(Path::new(duckdb_path), table, &columns)
                    .map_err(|e| PolarsError::ComputeError(e.into()))
            }
            DatabaseState::Failed { error } => Err(PolarsError::ComputeError(error.clone().into())),
        }
    }

    /// 数据集概览：DuckDB 用缓存元数据 + SQL null 统计。
    pub fn compute_dataset_overview(&mut self) -> PolarsResult<DatasetOverview> {
        match &self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                columns,
                row_count,
                ..
            } => {
                let columns = duckdb_profile_columns(columns);
                compute_dataset_overview_duckdb(Path::new(duckdb_path), table, &columns, *row_count)
                    .map_err(|e| PolarsError::ComputeError(e.into()))
            }
            DatabaseState::Failed { error } => Err(PolarsError::ComputeError(error.clone().into())),
        }
    }

    pub fn edit_cell(
        &mut self,
        row: usize,
        col_name: &str,
        new_value: serde_json::Value,
        row_id: Option<i64>,
    ) -> Result<EditState, String> {
        match &mut self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                history,
                ..
            } => {
                let path = PathBuf::from(duckdb_path.clone());
                let table_name = table.clone();
                let operation =
                    edit_cell_with_operation(&path, &table_name, row, row_id, col_name, new_value)?;
                history.push(operation);
                Ok(history.state())
            }
            DatabaseState::Failed { error } => Err(error.clone()),
        }
    }

    pub fn add_row(&mut self, index: Option<usize>) -> Result<EditState, String> {
        match &mut self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                history,
                row_count,
                ..
            } => {
                let path = PathBuf::from(duckdb_path.clone());
                let table_name = table.clone();
                let idx = index.unwrap_or(*row_count);
                let operation = add_row_with_operation(&path, &table_name, idx)?;
                history.push(operation);
                *row_count += 1;
                Ok(history.state())
            }
            DatabaseState::Failed { error } => Err(error.clone()),
        }
    }

    pub fn delete_rows(
        &mut self,
        indices: &[usize],
        row_ids: Option<&[i64]>,
    ) -> Result<EditState, String> {
        match &mut self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                history,
                row_count,
                ..
            } => {
                let path = PathBuf::from(duckdb_path.clone());
                let table_name = table.clone();
                let operations = delete_rows_with_operations(&path, &table_name, indices, row_ids)?;
                let deleted_count = operations.len();
                for operation in operations {
                    history.push(operation);
                }
                *row_count = row_count.saturating_sub(deleted_count);
                Ok(history.state())
            }
            DatabaseState::Failed { error } => Err(error.clone()),
        }
    }

    pub fn add_column(&mut self, name: &str, dtype: &str) -> Result<EditState, String> {
        let dtype = dtype_to_string(&dtype_from_string(dtype)?)?;

        match &mut self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                history,
                columns,
                ..
            } => {
                let mut op = EditOperation::AddColumn {
                    name: name.to_string(),
                    dtype: dtype.to_string(),
                };
                apply_edit_on_duckdb(Path::new(duckdb_path), table, &mut op)?;
                columns.push(DuckDbColumnMeta {
                    name: name.to_string(),
                    dtype: dtype.to_string(),
                });
                history.push(op);
                Ok(history.state())
            }
            DatabaseState::Failed { error } => Err(error.clone()),
        }
    }

    pub fn delete_column(&mut self, name: &str) -> Result<EditState, String> {
        match &mut self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                history,
                columns,
                ..
            } => {
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
                Ok(history.state())
            }
            DatabaseState::Failed { error } => Err(error.clone()),
        }
    }

    pub fn rename_column(&mut self, old_name: &str, new_name: &str) -> Result<EditState, String> {
        match &mut self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                history,
                columns,
                ..
            } => {
                let mut op = EditOperation::RenameColumn {
                    old_name: old_name.to_string(),
                    new_name: new_name.to_string(),
                };
                apply_edit_on_duckdb(Path::new(duckdb_path), table, &mut op)?;
                if let Some(col) = columns.iter_mut().find(|c| c.name == old_name) {
                    col.name = new_name.to_string();
                }
                history.push(op);
                Ok(history.state())
            }
            DatabaseState::Failed { error } => Err(error.clone()),
        }
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

        match &mut self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                history,
                columns,
                ..
            } => {
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
                Ok(history.state())
            }
            DatabaseState::Failed { error } => Err(error.clone()),
        }
    }

    pub fn undo_edit(&mut self) -> Result<EditState, String> {
        match &mut self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                history,
                row_count,
                columns,
                ..
            } => {
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
                Ok(history.state())
            }
            DatabaseState::Failed { error } => Err(error.clone()),
        }
    }

    pub fn redo_edit(&mut self) -> Result<EditState, String> {
        match &mut self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                history,
                row_count,
                columns,
                ..
            } => {
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
                Ok(history.state())
            }
            DatabaseState::Failed { error } => Err(error.clone()),
        }
    }

    pub fn save_changes(&mut self) -> Result<EditState, String> {
        match &mut self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                history,
                row_count,
                columns,
            } => {
                let (count, cols) = refresh_duckdb_meta(Path::new(duckdb_path), table)?;
                *row_count = count;
                *columns = cols;
                history.clear();
                Ok(history.state())
            }
            DatabaseState::Failed { error } => Err(error.clone()),
        }
    }

    pub fn edit_state(&self) -> EditState {
        match &self.state {
            DatabaseState::DuckDb { history, .. } => history.state(),
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
