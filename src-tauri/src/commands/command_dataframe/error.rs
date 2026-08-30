use serde::Serialize;

use crate::application::database::{DatabaseApplicationError, DatabaseApplicationOperation};
use crate::error::CommandError;
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::ResourceRevision;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseResourceDetails<'a> {
    database_id: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectIdentityDetails<'a> {
    project_instance_id: &'a ProjectInstanceId,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseRevisionDetails<'a> {
    database_id: &'a str,
    expected_revision: ResourceRevision,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseOperationDetails<'a> {
    database_id: &'a str,
    operation: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseRowLimitDetails<'a> {
    database_id: &'a str,
    operation: &'static str,
    requested_rows: usize,
    max_rows: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EngineDetails<'a> {
    engine: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportFormatDetails<'a> {
    format: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseNameDetails<'a> {
    database_id: &'a str,
    requested_name: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseInputDetails<'a> {
    database_id: &'a str,
    operation: &'static str,
    field: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectDatabaseDetails<'a> {
    project_instance_id: &'a ProjectInstanceId,
    #[serde(skip_serializing_if = "Option::is_none")]
    database_id: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    recovery_required: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CommandErrorSummary<'a> {
    code: &'static str,
    incident_id: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CleanupErrorDetails<'a> {
    cleanup_error: CommandErrorSummary<'a>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PrimaryErrorDetails<'a> {
    primary_error: CommandErrorSummary<'a>,
}

pub(super) fn database_command_error(error: DatabaseApplicationError) -> CommandError {
    match error {
        DatabaseApplicationError::NotFound { database_id } => {
            CommandError::expected("database_not_found").with_details(DatabaseResourceDetails {
                database_id: &database_id,
            })
        }
        DatabaseApplicationError::StaleProject {
            project_instance_id,
        } => {
            CommandError::expected("stale_project_lifecycle").with_details(ProjectIdentityDetails {
                project_instance_id: &project_instance_id,
            })
        }
        DatabaseApplicationError::StaleRevision {
            database_id,
            expected_revision,
        } => CommandError::expected("stale_database_revision").with_details(
            DatabaseRevisionDetails {
                database_id: &database_id,
                expected_revision,
            },
        ),
        DatabaseApplicationError::InvalidAccess {
            database_id,
            operation,
        } => CommandError::expected("database_access_failed").with_details(
            DatabaseOperationDetails {
                database_id: &database_id,
                operation: operation_name(operation),
            },
        ),
        DatabaseApplicationError::RowLimitExceeded {
            database_id,
            operation,
            requested_rows,
            max_rows,
        } => CommandError::expected("database_row_limit_exceeded").with_details(
            DatabaseRowLimitDetails {
                database_id: &database_id,
                operation: operation_name(operation),
                requested_rows,
                max_rows,
            },
        ),
        DatabaseApplicationError::ExportUnsupported { format } => {
            CommandError::expected("database_export_unsupported")
                .with_details(ExportFormatDetails { format: &format })
        }
        DatabaseApplicationError::SqlEngineUnsupported { engine } => {
            CommandError::expected("unsupported_sql_engine")
                .with_details(EngineDetails { engine: &engine })
        }
        DatabaseApplicationError::ImportUnsupported { engine } => {
            CommandError::expected("unsupported_database_import")
                .with_details(EngineDetails { engine })
        }
        DatabaseApplicationError::InvalidName {
            database_id,
            requested_name,
        } => CommandError::expected("invalid_database_name").with_details(DatabaseNameDetails {
            database_id: &database_id,
            requested_name: &requested_name,
        }),
        DatabaseApplicationError::NameConflict {
            database_id,
            requested_name,
        } => CommandError::expected("database_name_conflict").with_details(DatabaseNameDetails {
            database_id: &database_id,
            requested_name: &requested_name,
        }),
        DatabaseApplicationError::AlreadyExists { database_id } => {
            let error = CommandError::expected("database_already_exists");
            match database_id.as_deref() {
                Some(database_id) => error.with_details(DatabaseResourceDetails { database_id }),
                None => error,
            }
        }
        DatabaseApplicationError::InvalidInput {
            database_id,
            operation,
            field,
        } => CommandError::expected("invalid_database_input").with_details(DatabaseInputDetails {
            database_id: &database_id,
            operation: operation_name(operation),
            field,
        }),
        DatabaseApplicationError::OperationUnsupported {
            database_id,
            operation,
        } => {
            let error = CommandError::expected("database_operation_unsupported");
            match database_id.as_deref() {
                Some(database_id) => error.with_details(DatabaseOperationDetails {
                    database_id,
                    operation: operation_name(operation),
                }),
                None => error,
            }
        }
        DatabaseApplicationError::InvalidExportDestination => {
            CommandError::expected("database_export_temp_reservation_failed")
        }
        DatabaseApplicationError::Project {
            project_instance_id,
            database_id,
            source,
        } => {
            let code = source.code();
            let recovery_required = source.recovery_required().then_some(true);
            CommandError::expected(code).with_details(ProjectDatabaseDetails {
                project_instance_id: &project_instance_id,
                database_id: database_id.as_deref(),
                recovery_required,
            })
        }
        DatabaseApplicationError::CleanupAfterFailure { primary, cleanup } => {
            map_cleanup_failure(*primary, *cleanup)
        }
        DatabaseApplicationError::Internal(error) => {
            let code = internal_error_code(error.operation());
            if code == "internal_error" {
                CommandError::internal(error)
            } else {
                CommandError::diagnosed(code, error)
            }
        }
    }
}

fn map_cleanup_failure(
    primary: DatabaseApplicationError,
    cleanup: DatabaseApplicationError,
) -> CommandError {
    let primary = database_command_error(primary);
    let cleanup = database_command_error(cleanup);
    if primary.code() == "stale_project_lifecycle" {
        let details = CleanupErrorDetails {
            cleanup_error: CommandErrorSummary {
                code: cleanup.code(),
                incident_id: cleanup.incident_id(),
            },
        };
        primary.with_details(details)
    } else {
        let details = PrimaryErrorDetails {
            primary_error: CommandErrorSummary {
                code: primary.code(),
                incident_id: primary.incident_id(),
            },
        };
        cleanup.with_details(details)
    }
}

fn internal_error_code(operation: DatabaseApplicationOperation) -> &'static str {
    match operation {
        DatabaseApplicationOperation::ListTables => "database_table_list_failed",
        DatabaseApplicationOperation::ListSheets => "database_sheet_list_failed",
        DatabaseApplicationOperation::ReadMetadata
        | DatabaseApplicationOperation::ReadRows
        | DatabaseApplicationOperation::ColumnStatistics
        | DatabaseApplicationOperation::ColumnDistribution
        | DatabaseApplicationOperation::DatasetOverview
        | DatabaseApplicationOperation::ExportRead => "database_computation_failed",
        DatabaseApplicationOperation::ExportSerialize => "database_export_serialization_failed",
        DatabaseApplicationOperation::ExportReserve => "database_export_temp_reservation_failed",
        DatabaseApplicationOperation::ExportPublish => "database_export_publication_failed",
        DatabaseApplicationOperation::ExportCleanup => "database_export_cleanup_failed",
        DatabaseApplicationOperation::Load
        | DatabaseApplicationOperation::ReadEditState
        | DatabaseApplicationOperation::EditCell
        | DatabaseApplicationOperation::AddRow
        | DatabaseApplicationOperation::DeleteRows
        | DatabaseApplicationOperation::AddColumn
        | DatabaseApplicationOperation::DeleteColumn
        | DatabaseApplicationOperation::CastColumn
        | DatabaseApplicationOperation::RenameColumn
        | DatabaseApplicationOperation::UndoEdit
        | DatabaseApplicationOperation::RedoEdit
        | DatabaseApplicationOperation::Save
        | DatabaseApplicationOperation::Rename
        | DatabaseApplicationOperation::Delete => "internal_error",
    }
}

fn operation_name(operation: DatabaseApplicationOperation) -> &'static str {
    match operation {
        DatabaseApplicationOperation::Load => "load",
        DatabaseApplicationOperation::ListTables => "listTables",
        DatabaseApplicationOperation::ListSheets => "listSheets",
        DatabaseApplicationOperation::ReadMetadata => "readMetadata",
        DatabaseApplicationOperation::ReadRows => "readRows",
        DatabaseApplicationOperation::ColumnStatistics => "columnStatistics",
        DatabaseApplicationOperation::ColumnDistribution => "columnDistribution",
        DatabaseApplicationOperation::DatasetOverview => "datasetOverview",
        DatabaseApplicationOperation::ReadEditState => "readEditState",
        DatabaseApplicationOperation::EditCell => "editCell",
        DatabaseApplicationOperation::AddRow => "addRow",
        DatabaseApplicationOperation::DeleteRows => "deleteRows",
        DatabaseApplicationOperation::AddColumn => "addColumn",
        DatabaseApplicationOperation::DeleteColumn => "deleteColumn",
        DatabaseApplicationOperation::CastColumn => "castColumn",
        DatabaseApplicationOperation::RenameColumn => "renameColumn",
        DatabaseApplicationOperation::UndoEdit => "undoEdit",
        DatabaseApplicationOperation::RedoEdit => "redoEdit",
        DatabaseApplicationOperation::Save => "save",
        DatabaseApplicationOperation::Rename => "rename",
        DatabaseApplicationOperation::Delete => "delete",
        DatabaseApplicationOperation::ExportRead => "exportRead",
        DatabaseApplicationOperation::ExportSerialize => "exportSerialize",
        DatabaseApplicationOperation::ExportReserve => "exportReserve",
        DatabaseApplicationOperation::ExportPublish => "exportPublish",
        DatabaseApplicationOperation::ExportCleanup => "exportCleanup",
    }
}
