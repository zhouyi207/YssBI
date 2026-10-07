use super::*;

mod read;
use crate::database::DatabaseMutation;
pub(in crate::automation) use read::read_database;
use yss_database_contract::{DatabaseEngineSql, DatabaseId, DatabaseImportSource};
use yss_database_runtime::session_api;

pub(in crate::automation) fn semantic_to_contract(
    value: &yss_data_contract::ColumnSemantic,
) -> DatasetColumnSemantic {
    DatasetColumnSemantic {
        kind: match value.kind {
            yss_data_contract::SemanticType::Numeric => DatasetSemanticKind::Numeric,
            yss_data_contract::SemanticType::Categorical => DatasetSemanticKind::Categorical,
            yss_data_contract::SemanticType::Ordinal => DatasetSemanticKind::Ordinal,
            yss_data_contract::SemanticType::Binary => DatasetSemanticKind::Binary,
            yss_data_contract::SemanticType::Datetime => DatasetSemanticKind::Datetime,
            yss_data_contract::SemanticType::Text => DatasetSemanticKind::Text,
            yss_data_contract::SemanticType::Identifier => DatasetSemanticKind::Identifier,
        },
        values: value
            .values
            .iter()
            .map(|value| DatasetSemanticValue {
                value: value.value.clone(),
                label: value.label.clone(),
            })
            .collect(),
        positive_value: value.positive_value.clone(),
        numeric: value
            .numeric
            .as_ref()
            .map(|value| DatasetNumericConstraints {
                integer: value.integer,
                minimum: value.minimum.clone(),
                maximum: value.maximum.clone(),
            }),
    }
}

pub(super) fn import_source(source: DatasetImportSource) -> DatabaseImportSource {
    match source {
        DatasetImportSource::Csv {
            path,
            delimiter,
            has_header,
            infer_schema_length,
        } => DatabaseImportSource::Csv {
            path,
            delimiter,
            has_header,
            infer_schema_length,
        },
        DatasetImportSource::Parquet { path, columns } => {
            DatabaseImportSource::Parquet { path, columns }
        }
        DatasetImportSource::Excel { path, sheet } => DatabaseImportSource::Excel { path, sheet },
        DatasetImportSource::Sql {
            engine,
            connection_string,
            table,
        } => DatabaseImportSource::Sql {
            engine: match engine {
                DatasetSqlEngine::Sqlite { auto_create } => {
                    DatabaseEngineSql::Sqlite { auto_create }
                }
                DatasetSqlEngine::Postgres { ssl } => DatabaseEngineSql::Postgres { ssl },
                DatasetSqlEngine::Mysql { charset } => DatabaseEngineSql::Mysql { charset },
            },
            connection_string,
            table,
        },
    }
}
fn semantic_from_contract(semantic: DatasetColumnSemantic) -> yss_data_contract::ColumnSemantic {
    yss_data_contract::ColumnSemantic {
        kind: match semantic.kind {
            DatasetSemanticKind::Numeric => yss_data_contract::SemanticType::Numeric,
            DatasetSemanticKind::Categorical => yss_data_contract::SemanticType::Categorical,
            DatasetSemanticKind::Ordinal => yss_data_contract::SemanticType::Ordinal,
            DatasetSemanticKind::Binary => yss_data_contract::SemanticType::Binary,
            DatasetSemanticKind::Datetime => yss_data_contract::SemanticType::Datetime,
            DatasetSemanticKind::Text => yss_data_contract::SemanticType::Text,
            DatasetSemanticKind::Identifier => yss_data_contract::SemanticType::Identifier,
        },
        values: semantic
            .values
            .into_iter()
            .map(|value| yss_data_contract::SemanticValue {
                value: value.value,
                label: value.label,
            })
            .collect(),
        positive_value: semantic.positive_value,
        numeric: semantic
            .numeric
            .map(|value| yss_data_contract::NumericConstraints {
                integer: value.integer,
                minimum: value.minimum,
                maximum: value.maximum,
            }),
    }
}

pub(super) fn edited_columns(edit: &ResourceEdit) -> Vec<String> {
    match edit {
        ResourceEdit::CreateColumns { columns } => {
            columns.iter().map(|column| column.name.clone()).collect()
        }
        ResourceEdit::RenameColumns { columns } => {
            columns.iter().map(|column| column.name.clone()).collect()
        }
        ResourceEdit::DeleteColumns { columns } => columns.clone(),
        ResourceEdit::CastColumns { columns } => {
            columns.iter().map(|column| column.column.clone()).collect()
        }
        ResourceEdit::SetColumnSemantics { columns } => {
            columns.iter().map(|column| column.column.clone()).collect()
        }
        _ => vec![],
    }
}

pub(super) fn edit_operation(edit: ResourceEdit) -> Result<(DatabaseMutation, usize)> {
    Ok(match edit {
        ResourceEdit::InsertRows {
            rows,
            before_row_id,
        } => {
            let count = rows.len();
            (
                DatabaseMutation::InsertRows {
                    rows,
                    before_row_id,
                },
                count,
            )
        }
        ResourceEdit::UpdateCells { cells } => {
            let count = cells.len();
            let updates = cells
                .into_iter()
                .map(|cell| session_api::DatabaseCellUpdate {
                    row_id: cell.row_id,
                    column: cell.column.into(),
                    value: cell.value,
                })
                .collect();
            (DatabaseMutation::UpdateCells { updates }, count)
        }
        ResourceEdit::DeleteRows { row_ids } => {
            let count = row_ids.len();
            (DatabaseMutation::DeleteRowIds { row_ids }, count)
        }
        ResourceEdit::CreateColumns { columns } => {
            let count = columns.len();
            (
                DatabaseMutation::CreateColumns {
                    columns: columns
                        .into_iter()
                        .map(|column| (column.name, column.dtype))
                        .collect(),
                },
                count,
            )
        }
        ResourceEdit::RenameColumns { columns } => {
            let count = columns.len();
            (
                DatabaseMutation::RenameColumns {
                    columns: columns
                        .into_iter()
                        .map(|column| (column.column, column.name))
                        .collect(),
                },
                count,
            )
        }
        ResourceEdit::DeleteColumns { columns } => {
            let count = columns.len();
            (DatabaseMutation::DeleteColumns { columns }, count)
        }
        ResourceEdit::CastColumns { columns } => {
            let count = columns.len();
            (
                DatabaseMutation::CastColumns {
                    columns: columns
                        .into_iter()
                        .map(|column| (column.column, column.dtype, column.force))
                        .collect(),
                },
                count,
            )
        }
        ResourceEdit::SetColumnSemantics { columns } => {
            let count = columns.len();
            (
                DatabaseMutation::SetColumnSemantics {
                    columns: columns
                        .into_iter()
                        .map(|column| (column.column, semantic_from_contract(column.semantic)))
                        .collect(),
                },
                count,
            )
        }
        _ => {
            return Err(CapabilityFailure::new(
                CapabilityFailureCode::InvalidRequest,
            ));
        }
    })
}

pub(in crate::automation) fn export_database(
    application: &ApplicationState,
    session: &ApplicationSession,
    request: ExportDatabaseRequest,
    control: &CapabilityControl,
) -> Result<DatabaseExported> {
    check_version(session, &request.resource, &request.version)?;
    control.check()?;
    application
        .export_database_for_application(
            session.project_instance_id().clone(),
            request.resource.id.clone(),
            request.path.clone(),
            match request.format {
                DatasetExportFormat::Csv => "csv",
                DatasetExportFormat::Parquet => "parquet",
            }
            .into(),
            Some(ResourceRevision::new(request.version.revision)),
        )
        .map_err(database_error)?;
    Ok(DatabaseExported {
        resource: request.resource,
        path: request.path,
    })
}
