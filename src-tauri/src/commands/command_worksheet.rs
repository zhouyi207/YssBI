use crate::application::execution::{ApplicationState, SessionCaptureError};
use crate::application::worksheet::WorksheetApplicationError;
use crate::application::worksheet_plot::{WorksheetPlotApplicationError, WorksheetPlotQuery};
use crate::error::CommandError;
use crate::event::{Event, EventProject, emit_project_event_result};
use crate::project::{ProjectFilesystemError, WorksheetDocument, WorksheetResourcePath};
use crate::schema::application_event::ResourceMutationResultDto;
use serde::Serialize;
use tauri::{AppHandle, State};
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::{OperationId, ResourceRevision};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryRequiredDetails {
    recovery_required: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlotPoint {
    x: f64,
    y: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlotColumnPairPayload {
    data: Vec<PlotPoint>,
    x_label: Option<String>,
    y_label: Option<String>,
    x_format: String,
    y_format: String,
}

fn database_computation_error(error: impl std::fmt::Display + std::fmt::Debug) -> CommandError {
    CommandError::diagnosed("database_computation_failed", error)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WorksheetErrorDetails<'a> {
    resource_kind: &'static str,
    resource_path: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    recovery_required: Option<bool>,
}

fn worksheet_command_error(
    worksheet_path: &WorksheetResourcePath,
    error: ProjectFilesystemError,
) -> CommandError {
    let code = match &error {
        ProjectFilesystemError::TransactionPrepareFailed { .. } => "filesystem_prepare_failed",
        ProjectFilesystemError::TransactionCommitFailed { .. } => "filesystem_commit_failed",
        ProjectFilesystemError::TransactionRollbackFailed { .. }
        | ProjectFilesystemError::ProjectRecoveryRequired { .. } => "publication_recovery_required",
        _ => error.code(),
    };
    let recovery_required = error.recovery_required().then_some(true);
    let details = WorksheetErrorDetails {
        resource_kind: "worksheet",
        resource_path: worksheet_path.as_str(),
        recovery_required,
    };
    let command_error = match error {
        error @ (ProjectFilesystemError::TransactionPrepareFailed { .. }
        | ProjectFilesystemError::TransactionCommitFailed { .. }
        | ProjectFilesystemError::TransactionRollbackFailed { .. }) => {
            CommandError::diagnosed(code, error)
        }
        _ => CommandError::expected(code),
    };
    command_error.with_details(details)
}

fn worksheet_application_command_error(error: &WorksheetApplicationError) -> CommandError {
    match error {
        WorksheetApplicationError::SessionCapture(error) => session_capture_command_error(*error),
        WorksheetApplicationError::Project(error) => CommandError::from(error.clone()),
        WorksheetApplicationError::SessionChanged(error) => {
            CommandError::diagnosed("worksheet_session_changed", error)
        }
        WorksheetApplicationError::SessionRefresh(error) => {
            CommandError::diagnosed("worksheet_session_refresh_failed", error)
        }
    }
}

fn emit_worksheet_application_result(
    app: &AppHandle,
    result: &ResourceMutationResultDto,
) -> Result<(), CommandError> {
    emit_project_event_result(
        app,
        &Event::Project(EventProject::ResourceMutationCommitted {
            result: result.clone(),
        }),
    )
    .map_err(|error| CommandError::diagnosed("worksheet_event_emit_failed", error))
}

#[tauri::command]
pub fn create_worksheet(
    app: AppHandle,
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    name: String,
    database_id: Option<String>,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .create_worksheet_resource(project_instance_id, operation_id, name, database_id)
        .map_err(|error| worksheet_application_command_error(&error))?;
    let result = crate::schema::application_event::resource_mutation_to_transport(&result);
    emit_worksheet_application_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn duplicate_worksheet(
    app: AppHandle,
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    worksheet_path: WorksheetResourcePath,
    expected_revision: ResourceRevision,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .duplicate_worksheet_resource(
            project_instance_id,
            operation_id,
            worksheet_path,
            expected_revision,
        )
        .map_err(|error| worksheet_application_command_error(&error))?;
    let result = crate::schema::application_event::resource_mutation_to_transport(&result);
    emit_worksheet_application_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn load_worksheet(
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: String,
    worksheet_path: WorksheetResourcePath,
) -> Result<WorksheetDocument, CommandError> {
    let project_instance_id =
        yss_project_identity::ProjectInstanceId::from_existing(project_instance_id);
    application
        .load_worksheet_resource(project_instance_id, worksheet_path)
        .map_err(|error| worksheet_application_command_error(&error))
}

#[tauri::command]
pub fn save_worksheet(
    app: AppHandle,
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    worksheet_path: WorksheetResourcePath,
    expected_revision: ResourceRevision,
    document: WorksheetDocument,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .save_worksheet_resource(
            project_instance_id,
            operation_id,
            worksheet_path,
            expected_revision,
            document,
        )
        .map_err(|error| worksheet_application_command_error(&error))?;
    let result = crate::schema::application_event::resource_mutation_to_transport(&result);
    emit_worksheet_application_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn rename_worksheet_resource(
    app: AppHandle,
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    worksheet_path: WorksheetResourcePath,
    expected_revision: ResourceRevision,
    new_name: String,
    lifecycle_token: u64,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .rename_worksheet_resource(
            project_instance_id,
            operation_id,
            worksheet_path,
            expected_revision,
            new_name,
            lifecycle_token,
        )
        .map_err(|error| worksheet_application_command_error(&error))?;
    let result = crate::schema::application_event::resource_mutation_to_transport(&result);
    emit_worksheet_application_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn remove_worksheet(
    app: AppHandle,
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    worksheet_path: WorksheetResourcePath,
    expected_revision: ResourceRevision,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .remove_worksheet_resource(
            project_instance_id,
            operation_id,
            worksheet_path,
            expected_revision,
        )
        .map_err(|error| worksheet_application_command_error(&error))?;
    let result = crate::schema::application_event::resource_mutation_to_transport(&result);
    emit_worksheet_application_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn get_plot_column_pair(
    state: State<'_, ApplicationState>,
    project_instance_id: ProjectInstanceId,
    database_id: String,
    x_col: String,
    y_col: String,
    max_points: Option<usize>,
) -> Result<PlotColumnPairPayload, CommandError> {
    let x_column = yss_tabular_contract::TabularColumnName::try_from(x_col.as_str())
        .map_err(|error| database_computation_error(error))?;
    let y_column = yss_tabular_contract::TabularColumnName::try_from(y_col.as_str())
        .map_err(|error| database_computation_error(error))?;
    let result = state
        .query_worksheet_plot(WorksheetPlotQuery {
            project_instance_id,
            database_id: yss_database_contract::DatabaseId::from_existing(database_id.into()),
            x_column,
            y_column,
            max_points,
        })
        .map_err(worksheet_plot_command_error)?;
    Ok(PlotColumnPairPayload {
        data: result
            .data
            .into_iter()
            .map(|point| PlotPoint {
                x: point.x,
                y: point.y,
            })
            .collect(),
        x_label: result.x_label.map(Into::into),
        y_label: result.y_label.map(Into::into),
        x_format: plot_axis_format(result.x_format),
        y_format: plot_axis_format(result.y_format),
    })
}

fn worksheet_plot_command_error(error: WorksheetPlotApplicationError) -> CommandError {
    match error {
        WorksheetPlotApplicationError::SessionCapture(error) => {
            session_capture_command_error(error)
        }
        WorksheetPlotApplicationError::SessionChanged
        | WorksheetPlotApplicationError::ProjectIdentityMismatch { .. }
        | WorksheetPlotApplicationError::ProjectAuthorityChanged { .. } => {
            CommandError::expected("stale_project_lifecycle")
        }
        WorksheetPlotApplicationError::Database(error) => match error.kind() {
            crate::database::plot_query::DatabasePlotQueryErrorKind::AdmissionClosed => {
                CommandError::expected("project_lifecycle_admission_closed")
            }
            crate::database::plot_query::DatabasePlotQueryErrorKind::SessionMismatch
            | crate::database::plot_query::DatabasePlotQueryErrorKind::GenerationMismatch
            | crate::database::plot_query::DatabasePlotQueryErrorKind::RuntimeRevisionMismatch
            | crate::database::plot_query::DatabasePlotQueryErrorKind::SchemaRevisionMismatch => {
                CommandError::expected("stale_project_lifecycle")
            }
            crate::database::plot_query::DatabasePlotQueryErrorKind::DatabaseNotFound => {
                CommandError::expected("database_not_found")
            }
            crate::database::plot_query::DatabasePlotQueryErrorKind::ColumnMaterializationFailed => {
                CommandError::diagnosed("database_computation_failed", error)
            }
        },
        WorksheetPlotApplicationError::PlotDataEmpty => {
            CommandError::expected("plot_data_empty")
        }
    }
}

fn session_capture_command_error(error: SessionCaptureError) -> CommandError {
    match error {
        SessionCaptureError::Inactive => CommandError::expected("stale_project_lifecycle"),
        SessionCaptureError::Replacing => {
            CommandError::expected("project_lifecycle_admission_closed")
        }
        SessionCaptureError::Recovering => CommandError::expected("project_recovery_required")
            .with_details(RecoveryRequiredDetails {
                recovery_required: true,
            }),
    }
}

fn plot_axis_format(format: crate::application::worksheet_plot::PlotAxisFormat) -> String {
    match format {
        crate::application::worksheet_plot::PlotAxisFormat::Number => "number",
        crate::application::worksheet_plot::PlotAxisFormat::Date => "date",
        crate::application::worksheet_plot::PlotAxisFormat::Datetime => "datetime",
    }
    .to_owned()
}
