use super::DatabaseAccess;
use super::DatabaseDecl;
use super::DatabaseState;
use super::DatabaseView;
use polars::prelude::*;
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
    /// 把 `Pending` 状态实际化为 `Lazy`（或 `Failed`）。Pending 的引擎可能是
    /// SQL / Excel 这类同步读取器；此调用会真的去拉数据。其他状态保持不变。
    fn realize_pending(&mut self) -> PolarsResult<()> {
        if matches!(self.state, DatabaseState::Pending) {
            match self.decl.engine.build_lazy() {
                Ok(lazy_frame) => {
                    self.state = DatabaseState::Lazy { lazy_frame };
                }
                Err(e) => {
                    let msg = e.to_string();
                    self.state = DatabaseState::Failed {
                        error: msg.clone(),
                    };
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    pub fn ensure_loaded(&mut self) -> PolarsResult<&DataFrame> {
        self.realize_pending()?;

        let need_load = matches!(self.state, DatabaseState::Lazy { .. });

        if need_load {
            let lazy = match &self.state {
                DatabaseState::Lazy { lazy_frame } => lazy_frame.clone(),
                _ => unreachable!(),
            };

            let df = lazy.collect()?;
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
            _ => unreachable!("realize_pending+lazy collect should reach Loaded"),
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

        self.realize_pending()?;

        let df = match &self.state {
            DatabaseState::Pending => unreachable!("realize_pending leaves Pending"),
            DatabaseState::Lazy { lazy_frame } => lazy_frame.clone().limit(n).collect()?,
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

    pub fn save_changes(&mut self) -> Result<EditState, String> {
        self.ensure_loaded().map_err(|e| e.to_string())?;

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
