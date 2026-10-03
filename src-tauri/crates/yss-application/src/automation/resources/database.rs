use super::*;
use crate::database::DatabaseMutation;
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
pub(super) fn edit_operation(edit: DatasetOperation) -> DatabaseMutation {
    match edit {
        DatasetOperation::EditCell {
            row,
            column,
            value,
            row_id,
        } => DatabaseMutation::EditCell {
            row,
            column,
            value,
            row_id,
        },
        DatasetOperation::AddRow { index } => DatabaseMutation::AddRow { index },
        DatasetOperation::DeleteRows { indices, row_ids } => {
            DatabaseMutation::DeleteRows { indices, row_ids }
        }
        DatasetOperation::AddColumn { name, dtype } => DatabaseMutation::AddColumn { name, dtype },
        DatasetOperation::DeleteColumn { name } => DatabaseMutation::DeleteColumn { name },
        DatasetOperation::CastColumn {
            column,
            dtype,
            force,
        } => DatabaseMutation::CastColumn {
            column,
            dtype,
            force,
        },
        DatasetOperation::RenameColumn { old_name, new_name } => {
            DatabaseMutation::RenameColumn { old_name, new_name }
        }
        DatasetOperation::SetColumnSemantic { column, semantic } => {
            DatabaseMutation::SetColumnSemantic {
                column,
                semantic: yss_data_contract::ColumnSemantic {
                    kind: match semantic.kind {
                        DatasetSemanticKind::Numeric => yss_data_contract::SemanticType::Numeric,
                        DatasetSemanticKind::Categorical => {
                            yss_data_contract::SemanticType::Categorical
                        }
                        DatasetSemanticKind::Ordinal => yss_data_contract::SemanticType::Ordinal,
                        DatasetSemanticKind::Binary => yss_data_contract::SemanticType::Binary,
                        DatasetSemanticKind::Datetime => yss_data_contract::SemanticType::Datetime,
                        DatasetSemanticKind::Text => yss_data_contract::SemanticType::Text,
                        DatasetSemanticKind::Identifier => {
                            yss_data_contract::SemanticType::Identifier
                        }
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
                },
            }
        }
        DatasetOperation::Undo => DatabaseMutation::Undo,
        DatasetOperation::Redo => DatabaseMutation::Redo,
    }
}

pub(super) fn inspect_database(
    application: &ApplicationState,
    session: &ApplicationSession,
    request: &InspectResourceRequest,
    expected_revision: ResourceRevision,
) -> Result<(ResourceContent, bool)> {
    let id = request.resource.id.clone();
    let database = DatabaseId::from_existing(id.clone().into_boxed_str());
    let basis = session
        .database()
        .capture_query_basis(&database)
        .map_err(|_| unavailable())?;
    let schema = crate::automation::inspect_dataset_schema(
        session,
        InspectDatasetSchemaRequest {
            database_id: id.clone(),
        },
    )?;
    let project = session.project_instance_id().clone();
    let metadata = application
        .query_database_meta_for_application(project.clone(), id.clone(), expected_revision)
        .map_err(database_error)?;
    let page = application
        .query_database_rows_for_application(
            project.clone(),
            id.clone(),
            expected_revision,
            request.offset,
            request.limit,
        )
        .map_err(database_error)?;
    let state = application
        .query_database_edit_state_for_application(project, id, expected_revision)
        .map_err(database_error)?;
    let rows = (0..page.rows.row_count())
        .map(|row| {
            page.rows
                .columns()
                .iter()
                .map(|column| {
                    serde_json::to_value(&column.values()[row])
                        .map_err(|_| CapabilityFailure::new(CapabilityFailureCode::InternalFailure))
                })
                .collect::<Result<Vec<_>>>()
        })
        .collect::<Result<Vec<_>>>()?;
    session_api::revalidate_query_basis(session.database(), &basis).map_err(|_| conflict())?;
    let end = request.offset.saturating_add(rows.len());
    Ok((
        ResourceContent::Database {
            schema,
            rows,
            row_ids: page.row_ids,
            next_offset: (end < metadata.row_count).then_some(end),
            can_undo: state.can_undo,
            can_redo: state.can_redo,
        },
        state.is_modified,
    ))
}

pub(in crate::automation) fn export_dataset(
    application: &ApplicationState,
    session: &ApplicationSession,
    request: ExportDatasetRequest,
    control: &CapabilityControl,
) -> Result<DatasetExported> {
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
    Ok(DatasetExported {
        resource: request.resource,
        path: request.path,
    })
}
