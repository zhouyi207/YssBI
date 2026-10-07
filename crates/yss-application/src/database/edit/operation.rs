use crate::database::{DatabaseApplicationOperation, DatabaseOperationError, DatabaseUseCaseError};

/// Editable DataView operations admitted by the database runtime.
pub enum DatabaseMutation {
    UpdateCells {
        updates: Vec<yss_database_runtime::session_api::DatabaseCellUpdate>,
    },
    InsertRows {
        rows: Vec<std::collections::BTreeMap<String, yss_data_contract::TabularScalar>>,
        before_row_id: Option<i64>,
    },
    DeleteRowIds {
        row_ids: Vec<i64>,
    },
    EditCell {
        row: usize,
        column: String,
        value: serde_json::Value,
        row_id: Option<i64>,
    },
    AddRow {
        index: Option<usize>,
    },
    DeleteRows {
        indices: Vec<usize>,
        row_ids: Option<Vec<i64>>,
    },
    CreateColumns {
        columns: Vec<(String, String)>,
    },
    RenameColumns {
        columns: Vec<(String, String)>,
    },
    DeleteColumns {
        columns: Vec<String>,
    },
    CastColumns {
        columns: Vec<(String, String, bool)>,
    },
    SetColumnSemantics {
        columns: Vec<(String, yss_data_contract::ColumnSemantic)>,
    },
    AddColumn {
        name: String,
        dtype: String,
    },
    DeleteColumn {
        name: String,
    },
    CastColumn {
        column: String,
        dtype: String,
        force: bool,
    },
    SetColumnSemantic {
        column: String,
        semantic: yss_data_contract::ColumnSemantic,
    },
    RenameColumn {
        old_name: String,
        new_name: String,
    },
    Undo,
    Redo,
}

impl DatabaseMutation {
    pub fn operation(&self) -> DatabaseApplicationOperation {
        match self {
            Self::UpdateCells { .. } => DatabaseApplicationOperation::UpdateCells,
            Self::InsertRows { .. } => DatabaseApplicationOperation::InsertRows,
            Self::DeleteRowIds { .. } => DatabaseApplicationOperation::DeleteRows,
            Self::EditCell { .. } => DatabaseApplicationOperation::EditCell,
            Self::AddRow { .. } => DatabaseApplicationOperation::AddRow,
            Self::DeleteRows { .. } => DatabaseApplicationOperation::DeleteRows,
            Self::CreateColumns { .. } => DatabaseApplicationOperation::CreateColumns,
            Self::RenameColumns { .. } => DatabaseApplicationOperation::RenameColumns,
            Self::DeleteColumns { .. } => DatabaseApplicationOperation::DeleteColumns,
            Self::CastColumns { .. } => DatabaseApplicationOperation::CastColumns,
            Self::SetColumnSemantics { .. } => DatabaseApplicationOperation::SetColumnSemantics,
            Self::AddColumn { .. } => DatabaseApplicationOperation::AddColumn,
            Self::DeleteColumn { .. } => DatabaseApplicationOperation::DeleteColumn,
            Self::CastColumn { .. } => DatabaseApplicationOperation::CastColumn,
            Self::SetColumnSemantic { .. } => DatabaseApplicationOperation::SetColumnSemantic,
            Self::RenameColumn { .. } => DatabaseApplicationOperation::RenameColumn,
            Self::Undo => DatabaseApplicationOperation::UndoEdit,
            Self::Redo => DatabaseApplicationOperation::RedoEdit,
        }
    }
}

pub(super) fn runtime_database_mutation(
    database_id: &str,
    mutation: DatabaseMutation,
) -> Result<yss_database_runtime::session_api::DatabaseMutationOperation, DatabaseUseCaseError> {
    use yss_database_runtime::session_api::DatabaseMutationOperation;
    let operation = mutation.operation();
    let invalid = |field| {
        DatabaseUseCaseError::Database(DatabaseOperationError::InvalidInput {
            database_id: database_id.to_owned(),
            operation,
            field,
        })
    };
    match mutation {
        DatabaseMutation::UpdateCells { updates } => {
            Ok(DatabaseMutationOperation::UpdateCells { updates })
        }
        DatabaseMutation::InsertRows {
            rows,
            before_row_id,
        } => Ok(DatabaseMutationOperation::InsertRows {
            rows,
            before_row_id,
        }),
        DatabaseMutation::DeleteRowIds { row_ids } => {
            Ok(DatabaseMutationOperation::DeleteRowIds { row_ids })
        }
        DatabaseMutation::EditCell {
            row,
            column,
            value,
            row_id,
        } => {
            let value = serde_json::from_value(value).map_err(|_| invalid("value"))?;
            Ok(DatabaseMutationOperation::EditCell {
                row,
                column: column.into_boxed_str(),
                value,
                row_id,
            })
        }
        DatabaseMutation::AddRow { index } => Ok(DatabaseMutationOperation::AddRow {
            index: index.unwrap_or(usize::MAX),
        }),
        DatabaseMutation::DeleteRows { indices, row_ids } => {
            let mut distinct_indices = indices;
            distinct_indices.sort_unstable();
            distinct_indices.dedup();
            if let Some(row_ids) = &row_ids
                && row_ids.len() != distinct_indices.len()
            {
                return Err(invalid("rowIds"));
            }
            Ok(DatabaseMutationOperation::DeleteRows {
                indices: distinct_indices.into_boxed_slice(),
                row_ids: row_ids.map(Vec::into_boxed_slice),
            })
        }
        DatabaseMutation::CreateColumns { columns } => {
            Ok(DatabaseMutationOperation::CreateColumns {
                columns: columns
                    .into_iter()
                    .map(|(name, dtype)| {
                        yss_database_arrow::editable_data_type(&dtype)
                            .map(|dtype| (name.into(), dtype))
                            .map_err(|_| invalid("dtype"))
                    })
                    .collect::<Result<_, _>>()?,
            })
        }
        DatabaseMutation::RenameColumns { columns } => {
            Ok(DatabaseMutationOperation::RenameColumns {
                columns: columns
                    .into_iter()
                    .map(|(from, to)| (from.into(), to.into()))
                    .collect(),
            })
        }
        DatabaseMutation::DeleteColumns { columns } => {
            Ok(DatabaseMutationOperation::DeleteColumns {
                columns: columns.into_iter().map(Into::into).collect(),
            })
        }
        DatabaseMutation::CastColumns { columns } => Ok(DatabaseMutationOperation::CastColumns {
            columns: columns
                .into_iter()
                .map(|(name, dtype, force)| {
                    yss_database_arrow::editable_data_type(&dtype)
                        .map(|dtype| (name.into(), dtype, force))
                        .map_err(|_| invalid("dtype"))
                })
                .collect::<Result<_, _>>()?,
        }),
        DatabaseMutation::SetColumnSemantics { columns } => {
            Ok(DatabaseMutationOperation::SetColumnSemantics {
                columns: columns
                    .into_iter()
                    .map(|(name, semantic)| (name.into(), semantic))
                    .collect(),
            })
        }
        DatabaseMutation::AddColumn { name, dtype } => Ok(DatabaseMutationOperation::AddColumn {
            name: name.into_boxed_str(),
            data_type: yss_database_arrow::editable_data_type(&dtype)
                .map_err(|_| invalid("dtype"))?,
        }),
        DatabaseMutation::DeleteColumn { name } => Ok(DatabaseMutationOperation::DeleteColumn {
            name: name.into_boxed_str(),
        }),
        DatabaseMutation::CastColumn {
            column,
            dtype,
            force,
        } => Ok(DatabaseMutationOperation::CastColumn {
            name: column.into_boxed_str(),
            data_type: yss_database_arrow::editable_data_type(&dtype)
                .map_err(|_| invalid("dtype"))?,
            force,
        }),
        DatabaseMutation::SetColumnSemantic { column, semantic } => {
            Ok(DatabaseMutationOperation::SetColumnSemantic {
                name: column.into_boxed_str(),
                semantic,
            })
        }
        DatabaseMutation::RenameColumn { old_name, new_name } => {
            Ok(DatabaseMutationOperation::RenameColumn {
                old_name: old_name.into_boxed_str(),
                new_name: new_name.into_boxed_str(),
            })
        }
        DatabaseMutation::Undo => Ok(DatabaseMutationOperation::Undo),
        DatabaseMutation::Redo => Ok(DatabaseMutationOperation::Redo),
    }
}
