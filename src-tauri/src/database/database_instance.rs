use super::DatabaseAccess;
use super::DatabaseDecl;
use super::DatabaseEngine;
use super::DatabaseState;
use super::DatabaseView;
use super::{duckdb_table_sql, ingest_dataframe_to_duckdb, query_columns_to_dataframe, query_page_to_dataframe, query_to_dataframe};
use super::{
    compute_all_column_distributions_duckdb, compute_all_column_stats_duckdb,
    compute_dataset_overview_duckdb,
};
use crate::database::database_schema::{
    dataframe_to_schema, duckdb_columns_to_schema,
};
use crate::graph::node::DataSchema;
use polars::prelude::*;
use std::path::Path;
use std::sync::Arc;
use yss_sci::api::database::{
    anyvalue_to_json, apply_operation, capture_column_data, capture_row_data,
    cast_column as sci_cast_column, dtype_to_string, reverse_operation, EditHistory, EditOperation,
    EditState,
};

pub struct DatabaseInstance {
    pub decl: DatabaseDecl,
    pub state: DatabaseState,
}

impl DatabaseInstance {
    pub fn data_schema(&mut self) -> PolarsResult<DataSchema> {
        match &self.state {
            DatabaseState::DuckDb { columns, .. } => Ok(duckdb_columns_to_schema(columns)),
            DatabaseState::Loaded { dataframe, .. } => Ok(dataframe_to_schema(dataframe)),
            DatabaseState::Failed { error } => {
                Err(PolarsError::ComputeError(error.clone().into()))
            }
        }
    }

    /// 分页读取行数据。DuckDB 走 `LIMIT/OFFSET`，不触发整表物化。
    pub fn query_page(&mut self, offset: usize, limit: usize) -> PolarsResult<DataFrame> {
        match &self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                ..
            } => query_page_to_dataframe(Path::new(duckdb_path), table, offset, limit)
                .map_err(|e| PolarsError::ComputeError(e.into())),
            DatabaseState::Loaded { dataframe, .. } => {
                let total = dataframe.height();
                let start = offset.min(total);
                let count = limit.min(total.saturating_sub(start));
                Ok(dataframe.slice(start as i64, count))
            }
            DatabaseState::Failed { error } => {
                Err(PolarsError::ComputeError(error.clone().into()))
            }
        }
    }

    /// 按列名列表加载窄 DataFrame。DuckDB 走 `SELECT col1, col2, ...`，不整表物化。
    pub fn load_columns(&mut self, columns: &[&str]) -> PolarsResult<DataFrame> {
        match &self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                ..
            } => query_columns_to_dataframe(Path::new(duckdb_path), table, columns)
                .map_err(|e| PolarsError::ComputeError(e.into())),
            DatabaseState::Loaded { dataframe, .. } => {
                Ok(dataframe.clone().select(columns.to_vec())?)
            }
            DatabaseState::Failed { error } => {
                Err(PolarsError::ComputeError(error.clone().into()))
            }
        }
    }

    /// 加载单列 Series，优先走列裁剪路径。
    pub fn load_column_series(&mut self, column: &str) -> PolarsResult<Series> {
        let df = self.load_columns(&[column])?;
        Ok(df
            .column(column)?
            .clone()
            .take_materialized_series())
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

    /// 列统计：DuckDB 走 SQL 聚合，其它状态 fallback 到 Polars 整表。
    pub fn compute_column_stats(&mut self) -> PolarsResult<Vec<yss_sci::database::ColumnStats>> {
        match &self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                columns,
                row_count,
            } => compute_all_column_stats_duckdb(
                Path::new(duckdb_path),
                table,
                columns,
                *row_count,
            )
            .map_err(|e| PolarsError::ComputeError(e.into())),
            DatabaseState::Loaded { dataframe, .. } => {
                Ok(yss_sci::database::compute_all_column_stats(dataframe))
            }
            DatabaseState::Failed { error } => {
                Err(PolarsError::ComputeError(error.clone().into()))
            }
        }
    }

    /// 列分布：DuckDB 走 SQL 聚合。
    pub fn compute_column_distributions(
        &mut self,
    ) -> PolarsResult<Vec<yss_sci::database::ColumnDistribution>> {
        match &self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                columns,
                ..
            } => compute_all_column_distributions_duckdb(
                Path::new(duckdb_path),
                table,
                columns,
            )
            .map_err(|e| PolarsError::ComputeError(e.into())),
            DatabaseState::Loaded { dataframe, .. } => {
                Ok(yss_sci::database::compute_all_column_distributions(dataframe))
            }
            DatabaseState::Failed { error } => {
                Err(PolarsError::ComputeError(error.clone().into()))
            }
        }
    }

    /// 数据集概览：DuckDB 用缓存元数据 + SQL null 统计。
    pub fn compute_dataset_overview(
        &mut self,
    ) -> PolarsResult<yss_sci::database::DatasetOverview> {
        match &self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                columns,
                row_count,
            } => compute_dataset_overview_duckdb(
                Path::new(duckdb_path),
                table,
                columns,
                *row_count,
            )
            .map_err(|e| PolarsError::ComputeError(e.into())),
            DatabaseState::Loaded { dataframe, .. } => {
                Ok(yss_sci::database::compute_dataset_overview(dataframe))
            }
            DatabaseState::Failed { error } => {
                Err(PolarsError::ComputeError(error.clone().into()))
            }
        }
    }

    pub fn ensure_loaded(&mut self) -> PolarsResult<&DataFrame> {
        if matches!(self.state, DatabaseState::DuckDb { .. }) {
            let df = match &self.state {
                DatabaseState::DuckDb {
                    duckdb_path,
                    table,
                    ..
                } => {
                    let sql = format!("SELECT * FROM {}", duckdb_table_sql(table));
                    query_to_dataframe(Path::new(duckdb_path), &sql)
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

    pub fn access(&mut self, access: DatabaseAccess) -> PolarsResult<DatabaseView> {
        match access {
            DatabaseAccess::Preview => self.preview_view(),
            DatabaseAccess::Execution => self.execution_view(),
        }
    }

    fn preview_view(&mut self) -> PolarsResult<DatabaseView> {
        let n = 100;

        let df = match &self.state {
            DatabaseState::DuckDb {
                duckdb_path,
                table,
                ..
            } => {
                let sql = format!(
                    "SELECT * FROM {} LIMIT {}",
                    duckdb_table_sql(table),
                    n
                );
                query_to_dataframe(Path::new(duckdb_path), &sql)
                    .map_err(|e| PolarsError::ComputeError(e.into()))?
            }
            DatabaseState::Loaded { dataframe, .. } => dataframe.head(Some(n as usize)),
            DatabaseState::Failed { error } => {
                return Err(PolarsError::NoData(error.clone().into()))
            }
        };

        Ok(DatabaseView::new(df))
    }

    fn execution_view(&mut self) -> PolarsResult<DatabaseView> {
        let df = self.ensure_loaded()?;
        Ok(DatabaseView::new(df.clone()))
    }

    pub fn edit_cell(
        &mut self,
        row: usize,
        col_name: &str,
        new_value: serde_json::Value,
    ) -> Result<EditState, String> {
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
            col: col_name.to_string(),
            old_value,
            new_value,
        };

        apply_operation(df, &op)?;
        history.push(op);
        Ok(history.state())
    }

    pub fn add_row(&mut self, index: Option<usize>) -> Result<EditState, String> {
        self.ensure_loaded().map_err(|e| e.to_string())?;

        let (dataframe, history) = match &mut self.state {
            DatabaseState::Loaded {
                dataframe, history, ..
            } => (dataframe, history),
            _ => return Err("Database not loaded".into()),
        };

        let df = Arc::make_mut(dataframe);
        let idx = index.unwrap_or(df.height());
        let op = EditOperation::AddRow { index: idx };

        apply_operation(df, &op)?;
        history.push(op);
        Ok(history.state())
    }

    pub fn delete_rows(&mut self, indices: &[usize]) -> Result<EditState, String> {
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
                data,
            };
            apply_operation(df, &op)?;
            history.push(op);
        }

        Ok(history.state())
    }

    pub fn add_column(&mut self, name: &str, dtype: &str) -> Result<EditState, String> {
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
        self.ensure_loaded().map_err(|e| e.to_string())?;

        let (dataframe, history) = match &mut self.state {
            DatabaseState::Loaded {
                dataframe, history, ..
            } => (dataframe, history),
            _ => return Err("Database not loaded".into()),
        };

        let df = Arc::make_mut(dataframe);
        let data = capture_column_data(df, name);
        let op = EditOperation::DeleteColumn {
            name: name.to_string(),
            data,
        };

        apply_operation(df, &op)?;
        history.push(op);
        Ok(history.state())
    }

    pub fn rename_column(&mut self, old_name: &str, new_name: &str) -> Result<EditState, String> {
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
        let old_dtype = dtype_to_string(df.columns()[col_idx].dtype());

        sci_cast_column(df, col_name, new_dtype, force)?;

        let op = EditOperation::CastColumn {
            col: col_name.to_string(),
            old_data,
            old_dtype,
            new_dtype: new_dtype.to_string(),
        };
        history.push(op);
        Ok(history.state())
    }

    pub fn undo_edit(&mut self) -> Result<EditState, String> {
        let (dataframe, history) = match &mut self.state {
            DatabaseState::Loaded {
                dataframe, history, ..
            } => (dataframe, history),
            _ => return Err("Database not loaded".into()),
        };

        let op = history.pop_undo().ok_or("Nothing to undo")?;
        let df = Arc::make_mut(dataframe);
        reverse_operation(df, &op)?;
        history.push_redo(op);
        Ok(history.state())
    }

    pub fn redo_edit(&mut self) -> Result<EditState, String> {
        let (dataframe, history) = match &mut self.state {
            DatabaseState::Loaded {
                dataframe, history, ..
            } => (dataframe, history),
            _ => return Err("Database not loaded".into()),
        };

        let op = history.pop_redo().ok_or("Nothing to redo")?;
        let df = Arc::make_mut(dataframe);
        apply_operation(df, &op)?;
        history.push_undo(op);
        Ok(history.state())
    }

    pub fn save_changes(&mut self, project_root: Option<&Path>) -> Result<EditState, String> {
        self.ensure_loaded().map_err(|e| e.to_string())?;

        if let DatabaseEngine::DuckDb { path, table } = &self.decl.engine {
            let root = project_root.ok_or_else(|| {
                "请先打开或创建项目后再保存数据".to_string()
            })?;
            let duckdb_abs = root.join(path);
            let meta = match &mut self.state {
                DatabaseState::Loaded { dataframe, .. } => {
                    ingest_dataframe_to_duckdb(Arc::make_mut(dataframe), &duckdb_abs, table)?
                }
                _ => return Err("Database not loaded".into()),
            };
            self.state = DatabaseState::DuckDb {
                duckdb_path: duckdb_abs.to_string_lossy().to_string(),
                table: table.clone(),
                row_count: meta.row_count,
                columns: meta.columns,
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
            DatabaseState::Loaded { history, .. } => history.state(),
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
