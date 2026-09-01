use crate::error::CommandError;
use crate::event::{Event, EventProject, emit_project_event_result};
use crate::schema::application_event::ResourceMutationResultDto;
use serde::Serialize;
use tauri::{AppHandle, State};
use yss_application::chart::ChartApplicationError;
use yss_application::chart_plot::{ChartPlotApplicationError, ChartPlotQuery};
use yss_application::execution::{ApplicationState, SessionCaptureError};
use yss_chart_document::{ChartDocument, ChartResourcePath};
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

fn chart_application_command_error(error: &ChartApplicationError) -> CommandError {
    match error {
        ChartApplicationError::SessionCapture(error) => session_capture_command_error(*error),
        ChartApplicationError::Project(error) => {
            crate::commands::project_failure::application_project_command_error(error.clone())
        }
        ChartApplicationError::SessionChanged(error) => {
            CommandError::diagnosed("chart_session_changed", error)
        }
        ChartApplicationError::SessionRefresh(error) => {
            CommandError::diagnosed("chart_session_refresh_failed", error)
        }
    }
}

fn emit_chart_application_result(
    app: &AppHandle,
    result: &ResourceMutationResultDto,
) -> Result<(), CommandError> {
    emit_project_event_result(
        app,
        &Event::Project(EventProject::ResourceMutationCommitted {
            result: result.clone(),
        }),
    )
    .map_err(|error| CommandError::diagnosed("chart_event_emit_failed", error))
}

#[tauri::command]
pub fn create_chart(
    app: AppHandle,
    application: State<yss_application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    name: String,
    database_id: Option<String>,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .create_chart_resource(project_instance_id, operation_id, name, database_id)
        .map_err(|error| chart_application_command_error(&error))?;
    let result = crate::schema::application_event::resource_mutation_to_transport(&result);
    emit_chart_application_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn duplicate_chart(
    app: AppHandle,
    application: State<yss_application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    chart_path: ChartResourcePath,
    expected_revision: ResourceRevision,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .duplicate_chart_resource(
            project_instance_id,
            operation_id,
            chart_path,
            expected_revision,
        )
        .map_err(|error| chart_application_command_error(&error))?;
    let result = crate::schema::application_event::resource_mutation_to_transport(&result);
    emit_chart_application_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn load_chart(
    application: State<yss_application::execution::ApplicationState>,
    project_instance_id: String,
    chart_path: ChartResourcePath,
) -> Result<ChartDocument, CommandError> {
    let project_instance_id =
        yss_project_identity::ProjectInstanceId::from_existing(project_instance_id);
    application
        .load_chart_resource(project_instance_id, chart_path)
        .map_err(|error| chart_application_command_error(&error))
}

#[tauri::command]
pub fn save_chart(
    app: AppHandle,
    application: State<yss_application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    chart_path: ChartResourcePath,
    expected_revision: ResourceRevision,
    document: ChartDocument,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .save_chart_resource(
            project_instance_id,
            operation_id,
            chart_path,
            expected_revision,
            document,
        )
        .map_err(|error| chart_application_command_error(&error))?;
    let result = crate::schema::application_event::resource_mutation_to_transport(&result);
    emit_chart_application_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn rename_chart_resource(
    app: AppHandle,
    application: State<yss_application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    chart_path: ChartResourcePath,
    expected_revision: ResourceRevision,
    new_name: String,
    lifecycle_token: u64,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .rename_chart_resource(
            project_instance_id,
            operation_id,
            chart_path,
            expected_revision,
            new_name,
            lifecycle_token,
        )
        .map_err(|error| chart_application_command_error(&error))?;
    let result = crate::schema::application_event::resource_mutation_to_transport(&result);
    emit_chart_application_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn remove_chart(
    app: AppHandle,
    application: State<yss_application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    chart_path: ChartResourcePath,
    expected_revision: ResourceRevision,
) -> Result<ResourceMutationResultDto, CommandError> {
    let result = application
        .remove_chart_resource(
            project_instance_id,
            operation_id,
            chart_path,
            expected_revision,
        )
        .map_err(|error| chart_application_command_error(&error))?;
    let result = crate::schema::application_event::resource_mutation_to_transport(&result);
    emit_chart_application_result(&app, &result)?;
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
        .map_err(database_computation_error)?;
    let y_column = yss_tabular_contract::TabularColumnName::try_from(y_col.as_str())
        .map_err(database_computation_error)?;
    let result = state
        .query_chart_plot(ChartPlotQuery {
            project_instance_id,
            database_id: yss_database_contract::DatabaseId::from_existing(database_id.into()),
            x_column,
            y_column,
            max_points,
        })
        .map_err(chart_plot_command_error)?;
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

fn chart_plot_command_error(error: ChartPlotApplicationError) -> CommandError {
    match error {
        ChartPlotApplicationError::SessionCapture(error) => {
            session_capture_command_error(error)
        }
        ChartPlotApplicationError::SessionChanged
        | ChartPlotApplicationError::ProjectIdentityMismatch { .. }
        | ChartPlotApplicationError::ProjectAuthorityChanged { .. } => {
            CommandError::expected("stale_project_lifecycle")
        }
        ChartPlotApplicationError::Database(error) => match error.kind() {
            yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::AdmissionClosed => {
                CommandError::expected("project_lifecycle_admission_closed")
            }
            yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::SessionMismatch
            | yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::RuntimeRevisionMismatch
            | yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::SchemaRevisionMismatch => {
                CommandError::expected("stale_project_lifecycle")
            }
            yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::DatabaseNotFound => {
                CommandError::expected("database_not_found")
            }
            yss_database_runtime::plot_query::DatabasePlotQueryErrorKind::ColumnMaterializationFailed => {
                CommandError::diagnosed("database_computation_failed", error)
            }
        },
        ChartPlotApplicationError::PlotDataEmpty => {
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

fn plot_axis_format(format: yss_application::chart_plot::PlotAxisFormat) -> String {
    match format {
        yss_application::chart_plot::PlotAxisFormat::Number => "number",
        yss_application::chart_plot::PlotAxisFormat::Date => "date",
        yss_application::chart_plot::PlotAxisFormat::Datetime => "datetime",
    }
    .to_owned()
}
