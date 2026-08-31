use crate::application::database::{
    self, ApplicationDatabaseError, DatabaseMetaResult, DatabaseMutation, DatabaseMutationResult,
    DatabaseRowsResult, LoadDatabaseResult,
};
#[cfg(all(test, any()))]
use crate::application::database::{
    DatabaseApplicationError, cleanup_export_temporary_file,
    export_database_for_project_with_before_publish,
};
use crate::error::CommandError;
#[cfg(all(test, any()))]
use crate::event::emit_project_event;
use crate::event::{Event, EventProject, emit_project_event_result};
use crate::schema::application_event::ResourceMutationCommandResultDto;
use crate::schema::{
    DatabaseEngineDTO, DatabaseImportSourceDTO, DatabaseMetaResultDto, DatabaseRowsResultDto,
    LoadDatabaseResultDto,
};
use tauri::{AppHandle, State};
#[cfg(all(test, any()))]
use yss_database_edit::EditHistory;
use yss_database_edit::EditState;
#[cfg(all(test, any()))]
use yss_project::ProjectState;
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::{OperationId, ResourceRevision};

mod error;
#[cfg(all(test, any()))]
mod types;

use error::database_command_error;
#[cfg(all(test, any()))]
use types::dataframe_to_row_matrix;

#[cfg(all(test, any()))]
fn emit_database_result<T>(
    result: &ResourceMutationCommandResultDto<T>,
    mut emit: impl FnMut(Event),
) {
    emit(Event::Project(EventProject::ResourceMutationCommitted {
        result: result.mutation.clone(),
    }));
}

#[cfg(all(test, any()))]
fn load_database_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    engine: DatabaseImportSourceDTO,
    emit: impl FnMut(Event),
) -> Result<
    ResourceMutationCommandResultDto<crate::application::database::LoadDatabaseResult>,
    CommandError,
> {
    let result = database::load_database(state, &project_instance_id, operation_id, engine.into())
        .map_err(|error| {
            DatabaseApplicationError::from_project_database(
                error,
                database::DatabaseApplicationOperation::Load,
                &project_instance_id,
                None,
                None,
                None,
            )
        })
        .map_err(database_command_error)?;
    emit_database_result(&result, emit);
    Ok(result)
}

#[cfg(all(test, any()))]
fn mutate_database_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    id: &str,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    mutation: DatabaseMutation,
    emit: impl FnMut(Event),
) -> Result<ResourceMutationCommandResultDto<EditState>, CommandError> {
    let result = database::mutate_database_resource(
        state,
        &project_instance_id,
        id,
        expected_revision,
        operation_id,
        mutation,
    )
    .map_err(database_command_error)?;
    emit_database_result(&result, emit);
    Ok(result)
}

#[cfg(all(test, any()))]
fn save_database_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    id: &str,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    emit: impl FnMut(Event),
) -> Result<ResourceMutationCommandResultDto<EditState>, CommandError> {
    let result = database::save_database_changes(
        state,
        &project_instance_id,
        id,
        expected_revision,
        operation_id,
    )
    .map_err(|error| {
        DatabaseApplicationError::from_project_database(
            error,
            database::DatabaseApplicationOperation::Save,
            &project_instance_id,
            Some(id),
            Some(expected_revision),
            None,
        )
    })
    .map_err(database_command_error)?;
    emit_database_result(&result, emit);
    Ok(result)
}

#[cfg(all(test, any()))]
fn delete_database_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    id: &str,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    emit: impl FnMut(Event),
) -> Result<ResourceMutationCommandResultDto<()>, CommandError> {
    let result = state
        .delete_database(&project_instance_id, id, expected_revision, operation_id)
        .map_err(|error| {
            DatabaseApplicationError::from_project_database(
                error,
                database::DatabaseApplicationOperation::Delete,
                &project_instance_id,
                Some(id),
                Some(expected_revision),
                None,
            )
        })
        .map_err(database_command_error)?;
    emit_database_result(&result, emit);
    Ok(result)
}

async fn run_on_blocking_pool<F, R>(f: F) -> Result<R, CommandError>
where
    F: FnOnce() -> Result<R, CommandError> + Send + 'static,
    R: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(CommandError::internal)?
}

fn map_application_database_error(error: ApplicationDatabaseError) -> CommandError {
    match error {
        ApplicationDatabaseError::SessionCapture(error) => match error {
            crate::application::execution::SessionCaptureError::Inactive => {
                CommandError::expected("stale_project_lifecycle")
            }
            crate::application::execution::SessionCaptureError::Replacing => {
                CommandError::expected("project_lifecycle_admission_closed")
            }
            crate::application::execution::SessionCaptureError::Recovering => {
                CommandError::expected("project_recovery_required")
                    .with_details(serde_json::json!({ "recoveryRequired": true }))
            }
        },
        ApplicationDatabaseError::SessionChanged(error) => {
            CommandError::diagnosed("database_session_changed", error)
        }
        ApplicationDatabaseError::SessionRefresh(error) => {
            CommandError::diagnosed("database_session_refresh_failed", error)
        }
        ApplicationDatabaseError::Database(error) => database_command_error(error),
        ApplicationDatabaseError::Mutation(error) => {
            CommandError::diagnosed("database_mutation_failed", error)
        }
    }
}

fn emit_application_database_result<T>(
    app: &AppHandle,
    result: &ResourceMutationCommandResultDto<T>,
) -> Result<(), CommandError> {
    emit_project_event_result(
        app,
        &Event::Project(EventProject::ResourceMutationCommitted {
            result: result.mutation.clone(),
        }),
    )
    .map_err(|error| CommandError::diagnosed("database_event_emit_failed", error))
}

fn database_mutation_to_transport<T>(
    result: DatabaseMutationResult<T>,
) -> ResourceMutationCommandResultDto<T> {
    ResourceMutationCommandResultDto {
        data: result.data,
        mutation: crate::schema::application_event::resource_mutation_to_transport(
            &result.mutation,
        ),
    }
}

fn load_database_result_to_transport(
    result: DatabaseMutationResult<LoadDatabaseResult>,
) -> ResourceMutationCommandResultDto<LoadDatabaseResultDto> {
    let data = result.data;
    ResourceMutationCommandResultDto {
        data: LoadDatabaseResultDto {
            id: data.id,
            name: data.name,
            row_count: data.row_count,
            column_count: data.column_count,
            columns: crate::schema::column_info_from_schema(&data.columns),
        },
        mutation: crate::schema::application_event::resource_mutation_to_transport(
            &result.mutation,
        ),
    }
}

fn database_meta_to_transport(result: DatabaseMetaResult) -> DatabaseMetaResultDto {
    DatabaseMetaResultDto {
        id: result.id,
        name: result.name,
        row_count: result.row_count,
        column_count: result.column_count,
        columns: crate::schema::column_info_from_schema(&result.columns),
    }
}

fn database_rows_to_transport(
    result: DatabaseRowsResult,
) -> Result<DatabaseRowsResultDto, CommandError> {
    let row_count = result.rows.row_count();
    let rows = (0..row_count)
        .map(|row| {
            result
                .rows
                .columns()
                .iter()
                .map(|column| {
                    serde_json::to_value(&column.values()[row]).map_err(|error| {
                        CommandError::diagnosed("database_serialization_failed", error)
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DatabaseRowsResultDto {
        rows,
        row_ids: result.row_ids,
    })
}

fn database_engine_from_import(
    source: DatabaseImportSourceDTO,
) -> Result<yss_database_contract::DatabaseEngine, CommandError> {
    yss_database_contract::DatabaseEngine::try_from(DatabaseEngineDTO::from(source))
        .map_err(|_| CommandError::expected("invalid_database_engine"))
}

fn serialize_application_database_value<T: serde::Serialize>(
    value: T,
) -> Result<serde_json::Value, CommandError> {
    serde_json::to_value(value)
        .map_err(|error| CommandError::diagnosed("database_serialization_failed", error))
}

fn mutate_database_from_application(
    app: &AppHandle,
    application: &crate::application::execution::ApplicationState,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    mutation: DatabaseMutation,
) -> Result<ResourceMutationCommandResultDto<EditState>, CommandError> {
    let result = application
        .mutate_database_for_application(
            project_instance_id,
            id,
            expected_revision,
            operation_id,
            mutation,
        )
        .map_err(map_application_database_error)?;
    let result = database_mutation_to_transport(result);
    emit_application_database_result(app, &result)?;
    Ok(result)
}

#[cfg(all(test, any()))]
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseRowsPayload {
    rows: Vec<Vec<serde_json::Value>>,
    row_ids: Vec<i64>,
}

#[cfg(all(test, any()))]
fn serialize_database_value<T: serde::Serialize>(
    value: T,
) -> Result<serde_json::Value, CommandError> {
    serde_json::to_value(value)
        .map_err(|error| CommandError::diagnosed("database_serialization_failed", error))
}

#[cfg(all(test, any()))]
fn get_database_meta_for_project(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
) -> Result<serde_json::Value, CommandError> {
    let result = database::read_database_meta(state, project_instance_id, id)
        .map_err(database_command_error)?;
    serialize_database_value(result)
}

#[cfg(all(test, any()))]
fn get_database_rows_for_project(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
    offset: usize,
    limit: usize,
) -> Result<serde_json::Value, CommandError> {
    let page = database::read_database_rows(state, project_instance_id, id, offset, limit)
        .map_err(database_command_error)?;
    serialize_database_value(DatabaseRowsPayload {
        rows: dataframe_to_row_matrix(&page.dataframe),
        row_ids: page.row_ids,
    })
}

#[cfg(all(test, any()))]
fn get_column_stats_for_project(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
) -> Result<serde_json::Value, CommandError> {
    let stats = database::read_column_statistics(state, project_instance_id, id)
        .map_err(database_command_error)?;
    serialize_database_value(stats)
}

#[cfg(all(test, any()))]
fn get_column_distribution_for_project(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
) -> Result<serde_json::Value, CommandError> {
    let distributions = database::read_column_distributions(state, project_instance_id, id)
        .map_err(database_command_error)?;
    serialize_database_value(distributions)
}

#[cfg(all(test, any()))]
fn get_dataset_overview_for_project(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
) -> Result<serde_json::Value, CommandError> {
    let overview = database::read_dataset_overview(state, project_instance_id, id)
        .map_err(database_command_error)?;
    serialize_database_value(overview)
}

#[cfg(all(test, any()))]
fn get_edit_state_for_project(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
) -> Result<serde_json::Value, CommandError> {
    let edit_state = database::read_database_edit_state(state, project_instance_id, id)
        .map_err(database_command_error)?;
    serialize_database_value(edit_state)
}

#[tauri::command]
pub async fn load_database(
    app: AppHandle,
    application: State<'_, crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    engine: DatabaseImportSourceDTO,
) -> Result<ResourceMutationCommandResultDto<LoadDatabaseResultDto>, CommandError> {
    let application = application.inner().clone();
    run_on_blocking_pool(move || {
        let result = application
            .load_database_for_application(
                project_instance_id,
                operation_id,
                database_engine_from_import(engine)?,
            )
            .map_err(map_application_database_error)?;
        let result = load_database_result_to_transport(result);
        emit_application_database_result(&app, &result)?;
        Ok(result)
    })
    .await
}

#[tauri::command]
pub async fn list_sqlite_tables(db_path: String) -> Result<Vec<String>, CommandError> {
    run_on_blocking_pool(move || {
        database::list_sqlite_tables(&db_path).map_err(database_command_error)
    })
    .await
}

#[tauri::command]
pub async fn list_sql_tables(
    engine: String,
    connection_string: String,
) -> Result<Vec<String>, CommandError> {
    run_on_blocking_pool(move || {
        database::list_sql_tables(&engine, &connection_string).map_err(database_command_error)
    })
    .await
}

#[tauri::command]
pub async fn list_excel_sheets(file_path: String) -> Result<Vec<String>, CommandError> {
    run_on_blocking_pool(move || {
        database::list_excel_sheets(&file_path).map_err(database_command_error)
    })
    .await
}

#[tauri::command]
pub fn get_database_meta(
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
) -> Result<serde_json::Value, CommandError> {
    let result = application
        .query_database_meta_for_application(project_instance_id, id)
        .map_err(map_application_database_error)?;
    serialize_application_database_value(database_meta_to_transport(result))
}

#[tauri::command]
pub async fn delete_database(
    app: AppHandle,
    application: State<'_, crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ResourceMutationCommandResultDto<()>, CommandError> {
    let application = application.inner().clone();
    run_on_blocking_pool(move || {
        let result = application
            .delete_database_for_application(
                project_instance_id,
                id,
                expected_revision,
                operation_id,
            )
            .map_err(map_application_database_error)?;
        let result = database_mutation_to_transport(result);
        emit_application_database_result(&app, &result)?;
        Ok(result)
    })
    .await
}

#[cfg(all(test, any()))]
fn rename_database_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    id: &str,
    expected_revision: ResourceRevision,
    name: &str,
    operation_id: OperationId,
    emit: impl FnMut(Event),
) -> Result<ResourceMutationCommandResultDto<()>, CommandError> {
    let result = database::rename_database(
        state,
        &project_instance_id,
        id,
        expected_revision,
        name,
        operation_id,
    )
    .map_err(|error| {
        DatabaseApplicationError::from_project_database(
            error,
            database::DatabaseApplicationOperation::Rename,
            &project_instance_id,
            Some(id),
            Some(expected_revision),
            Some(name.trim()),
        )
    })
    .map_err(database_command_error)?;
    emit_database_result(&result, emit);
    Ok(result)
}

#[tauri::command]
pub fn rename_database(
    app: AppHandle,
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    name: String,
    operation_id: OperationId,
) -> Result<ResourceMutationCommandResultDto<()>, CommandError> {
    let result = application
        .rename_database_for_application(
            project_instance_id,
            id,
            expected_revision,
            name,
            operation_id,
        )
        .map_err(map_application_database_error)?;
    let result = database_mutation_to_transport(result);
    emit_application_database_result(&app, &result)?;
    Ok(result)
}

#[tauri::command]
pub fn get_database_rows(
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    offset: usize,
    limit: usize,
) -> Result<serde_json::Value, CommandError> {
    let result = application
        .query_database_rows_for_application(project_instance_id, id, offset, limit)
        .map_err(map_application_database_error)?;
    serialize_application_database_value(database_rows_to_transport(result)?)
}

#[tauri::command]
pub async fn get_column_stats(
    application: State<'_, crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
) -> Result<serde_json::Value, CommandError> {
    let application = application.inner().clone();
    run_on_blocking_pool(move || {
        let result = application
            .query_column_stats_for_application(project_instance_id, id)
            .map_err(map_application_database_error)?;
        serialize_application_database_value(result)
    })
    .await
}

#[tauri::command]
pub async fn get_column_distribution(
    application: State<'_, crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
) -> Result<serde_json::Value, CommandError> {
    let application = application.inner().clone();
    run_on_blocking_pool(move || {
        let result = application
            .query_column_distributions_for_application(project_instance_id, id)
            .map_err(map_application_database_error)?;
        serialize_application_database_value(result)
    })
    .await
}

#[tauri::command]
pub async fn get_dataset_overview(
    application: State<'_, crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
) -> Result<serde_json::Value, CommandError> {
    let application = application.inner().clone();
    run_on_blocking_pool(move || {
        let result = application
            .query_dataset_overview_for_application(project_instance_id, id)
            .map_err(map_application_database_error)?;
        serialize_application_database_value(result)
    })
    .await
}

// ==================== Edit Commands ====================

#[tauri::command]
pub fn edit_cell(
    app: AppHandle,
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    row: usize,
    col_name: String,
    value: serde_json::Value,
    row_id: Option<i64>,
) -> Result<ResourceMutationCommandResultDto<EditState>, CommandError> {
    mutate_database_from_application(
        &app,
        application.inner(),
        project_instance_id,
        id,
        expected_revision,
        operation_id,
        DatabaseMutation::EditCell {
            row,
            column: col_name,
            value,
            row_id,
        },
    )
}

#[tauri::command]
pub fn add_row(
    app: AppHandle,
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    index: Option<usize>,
) -> Result<ResourceMutationCommandResultDto<EditState>, CommandError> {
    mutate_database_from_application(
        &app,
        application.inner(),
        project_instance_id,
        id,
        expected_revision,
        operation_id,
        DatabaseMutation::AddRow { index },
    )
}

#[tauri::command]
pub fn delete_rows(
    app: AppHandle,
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    indices: Vec<usize>,
    row_ids: Option<Vec<i64>>,
) -> Result<ResourceMutationCommandResultDto<EditState>, CommandError> {
    mutate_database_from_application(
        &app,
        application.inner(),
        project_instance_id,
        id,
        expected_revision,
        operation_id,
        DatabaseMutation::DeleteRows { indices, row_ids },
    )
}

#[tauri::command]
pub fn add_column(
    app: AppHandle,
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    name: String,
    dtype: String,
) -> Result<ResourceMutationCommandResultDto<EditState>, CommandError> {
    mutate_database_from_application(
        &app,
        application.inner(),
        project_instance_id,
        id,
        expected_revision,
        operation_id,
        DatabaseMutation::AddColumn { name, dtype },
    )
}

#[tauri::command]
pub fn delete_column(
    app: AppHandle,
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    name: String,
) -> Result<ResourceMutationCommandResultDto<EditState>, CommandError> {
    mutate_database_from_application(
        &app,
        application.inner(),
        project_instance_id,
        id,
        expected_revision,
        operation_id,
        DatabaseMutation::DeleteColumn { name },
    )
}

#[tauri::command]
pub fn cast_column(
    app: AppHandle,
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    col_name: String,
    new_dtype: String,
    force: Option<bool>,
) -> Result<ResourceMutationCommandResultDto<EditState>, CommandError> {
    mutate_database_from_application(
        &app,
        application.inner(),
        project_instance_id,
        id,
        expected_revision,
        operation_id,
        DatabaseMutation::CastColumn {
            column: col_name,
            dtype: new_dtype,
            force: force.unwrap_or(false),
        },
    )
}

#[tauri::command]
pub fn rename_column(
    app: AppHandle,
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    old_name: String,
    new_name: String,
) -> Result<ResourceMutationCommandResultDto<EditState>, CommandError> {
    mutate_database_from_application(
        &app,
        application.inner(),
        project_instance_id,
        id,
        expected_revision,
        operation_id,
        DatabaseMutation::RenameColumn { old_name, new_name },
    )
}

#[tauri::command]
pub fn undo_edit(
    app: AppHandle,
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ResourceMutationCommandResultDto<EditState>, CommandError> {
    mutate_database_from_application(
        &app,
        application.inner(),
        project_instance_id,
        id,
        expected_revision,
        operation_id,
        DatabaseMutation::Undo,
    )
}

#[tauri::command]
pub fn redo_edit(
    app: AppHandle,
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ResourceMutationCommandResultDto<EditState>, CommandError> {
    mutate_database_from_application(
        &app,
        application.inner(),
        project_instance_id,
        id,
        expected_revision,
        operation_id,
        DatabaseMutation::Redo,
    )
}

#[tauri::command]
pub fn save_database_changes(
    app: AppHandle,
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ResourceMutationCommandResultDto<EditState>, CommandError> {
    let result = application
        .save_database_for_application(project_instance_id, id, expected_revision, operation_id)
        .map_err(map_application_database_error)?;
    let result = database_mutation_to_transport(result);
    emit_application_database_result(&app, &result)?;
    Ok(result)
}

/// Export the current dataset view (including unsaved in-memory edits) to an external file.
/// Use `save_database_changes` to persist edits into `project.duckdb`.
#[tauri::command]
pub async fn export_database(
    application: State<'_, crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    path: String,
    format: String,
) -> Result<(), CommandError> {
    let application = application.inner().clone();
    run_on_blocking_pool(move || {
        application
            .export_database_for_application(project_instance_id, id, path, format)
            .map_err(map_application_database_error)
    })
    .await
}

#[tauri::command]
pub fn get_edit_state(
    application: State<crate::application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
) -> Result<serde_json::Value, CommandError> {
    let result = application
        .query_database_edit_state_for_application(project_instance_id, id)
        .map_err(map_application_database_error)?;
    serialize_application_database_value(result)
}

#[cfg(all(test, any()))]
mod tests {
    use super::*;
    use crate::event::{Event, EventProject};
    use yss_database_contract::{DatabaseDecl, DatabaseEngine, DatabaseId};
    use yss_database_runtime::DatabaseState;
    use yss_project_identity::{OperationId, ResourceRevision};
    use yss_project_model::ProjectData;

    struct FailingSerialize;

    impl serde::Serialize for FailingSerialize {
        fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom("injected serialization failure"))
        }
    }

    #[test]
    fn database_serialization_errors_are_typed() {
        let error = serialize_database_value(FailingSerialize).unwrap_err();
        assert_eq!(error.code(), "database_serialization_failed");
    }

    #[test]
    fn database_application_errors_map_to_safe_wire_contract() {
        let expected = database_command_error(DatabaseApplicationError::RowLimitExceeded {
            database_id: "sales".into(),
            operation: database::DatabaseApplicationOperation::ReadRows,
            requested_rows: 500_001,
            max_rows: 500_000,
        });
        assert_eq!(expected.code(), "database_row_limit_exceeded");
        assert_eq!(
            expected.details(),
            serde_json::json!({
                "databaseId": "sales",
                "operation": "readRows",
                "requestedRows": 500_001,
                "maxRows": 500_000,
            })
            .as_object(),
        );
        assert!(expected.incident_id().is_none());

        let internal = database_command_error(DatabaseApplicationError::internal_for_test(
            database::DatabaseApplicationOperation::ExportSerialize,
            "sensitive backend failure",
        ));
        let wire = serde_json::to_value(&internal).unwrap();
        assert_eq!(internal.code(), "database_export_serialization_failed");
        assert!(internal.incident_id().is_some());
        assert!(!wire.to_string().contains("sensitive backend failure"));
    }

    fn assert_exact_event<T>(
        events: &[Event],
        result: &ResourceMutationCommandResultDto<T>,
        expected_count: usize,
    ) {
        assert_eq!(events.len(), expected_count);
        let Event::Project(EventProject::ResourceMutationCommitted { result: emitted }) =
            events.last().unwrap()
        else {
            panic!("database command emitted a non-canonical event")
        };
        assert_eq!(emitted, &result.mutation);
    }

    fn install_export_database(state: &ProjectState, project_name: &str) -> ProjectInstanceId {
        let mut project = ProjectData::new();
        let decl = DatabaseDecl {
            id: DatabaseId::from_existing("sales".into()),
            engine: DatabaseEngine::InMemory {
                name: "sales".into(),
            },
            schema_version: 1,
            required: false,
            name: project_name.into(),
        };
        project.databases.insert("sales".into(), decl.clone());
        state.activate_project_fixture(project_name.into(), project);
        let dataframe = polars::df!("amount" => &[1_i64, 2_i64]).unwrap();
        state.project_store.write().unwrap().databases.insert(
            "sales".into(),
            yss_database_runtime::DatabaseInstance {
                decl,
                state: yss_database_runtime::DatabaseState::Loaded {
                    dataframe: std::sync::Arc::new(dataframe.clone()),
                    original: std::sync::Arc::new(dataframe),
                    history: EditHistory::new(),
                },
            },
        );
        state.capture_project_session().unwrap().instance_id
    }

    fn assert_only_destination_exists(root: &std::path::Path, destination: &std::path::Path) {
        let entries = std::fs::read_dir(root)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(entries, vec![destination.to_path_buf()]);
    }

    #[test]
    fn database_reads_reject_stale_project_identity() {
        let state = ProjectState::new();
        let stale = install_export_database(&state, "read-original");
        install_export_database(&state, "read-replacement");

        let errors = [
            get_database_meta_for_project(&state, &stale, "sales").unwrap_err(),
            get_database_rows_for_project(&state, &stale, "sales", 0, 10).unwrap_err(),
            get_column_stats_for_project(&state, &stale, "sales").unwrap_err(),
            get_column_distribution_for_project(&state, &stale, "sales").unwrap_err(),
            get_dataset_overview_for_project(&state, &stale, "sales").unwrap_err(),
            get_edit_state_for_project(&state, &stale, "sales").unwrap_err(),
        ];

        for error in errors {
            assert_eq!(error.code(), "stale_project_lifecycle");
        }
    }

    #[test]
    fn database_export_rejects_replacement_before_publication() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-database-export-lifecycle-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let destination = root.join("sales.csv");
        std::fs::write(&destination, b"sentinel").unwrap();
        let state = ProjectState::new();
        let stale = install_export_database(&state, "export-original");
        install_export_database(&state, "export-replacement");

        let before_entry = export_database_for_project_with_before_publish(
            &state,
            &stale,
            "sales",
            destination.to_string_lossy().as_ref(),
            "csv",
            |_| {},
            |_| {},
        );
        assert_eq!(
            database_command_error(before_entry.unwrap_err()).code(),
            "stale_project_lifecycle"
        );
        assert_eq!(std::fs::read(&destination).unwrap(), b"sentinel");
        assert_only_destination_exists(&root, &destination);

        let current = state.capture_project_session().unwrap().instance_id;
        let replacement_state = state.clone();
        let before_publication = export_database_for_project_with_before_publish(
            &state,
            &current,
            "sales",
            destination.to_string_lossy().as_ref(),
            "csv",
            move |_| {
                install_export_database(&replacement_state, "export-final-replacement");
            },
            |_| {},
        );
        assert_eq!(
            database_command_error(before_publication.unwrap_err()).code(),
            "stale_project_lifecycle"
        );
        assert_eq!(std::fs::read(&destination).unwrap(), b"sentinel");
        assert_only_destination_exists(&root, &destination);

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn database_export_publication_wins_before_replacement_activation() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-database-export-publication-wins-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let destination = root.join("sales.csv");
        std::fs::write(&destination, b"sentinel").unwrap();
        let state = ProjectState::new();
        let current = install_export_database(&state, "export-publication-current");
        let (observed_tx, observed_rx) = std::sync::mpsc::channel();
        let observed_destination = destination.clone();
        state.set_project_activation_test_hook(std::sync::Arc::new(move || {
            observed_tx
                .send(std::fs::read(&observed_destination).unwrap())
                .unwrap();
        }));
        let activation = std::sync::Arc::new(std::sync::Mutex::new(None));
        let activation_for_hook = std::sync::Arc::clone(&activation);
        let replacement_state = state.clone();

        export_database_for_project_with_before_publish(
            &state,
            &current,
            "sales",
            destination.to_string_lossy().as_ref(),
            "csv",
            |_| {},
            move |_| {
                *activation_for_hook.lock().unwrap() = Some(std::thread::spawn(move || {
                    install_export_database(&replacement_state, "export-publication-replacement");
                }));
            },
        )
        .unwrap();
        activation.lock().unwrap().take().unwrap().join().unwrap();

        let published = std::fs::read(&destination).unwrap();
        assert_ne!(published, b"sentinel");
        assert_eq!(observed_rx.recv().unwrap(), published);
        assert_only_destination_exists(&root, &destination);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn database_export_returns_stable_stage_errors_and_cleans_temporary_output() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-database-export-stage-errors-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let state = ProjectState::new();
        let current = install_export_database(&state, "export-errors");
        let destination = root.join("sales.csv");

        let serialization = database_command_error(
            export_database_for_project_with_before_publish(
                &state,
                &current,
                "sales",
                destination.to_string_lossy().as_ref(),
                "unsupported",
                |_| {},
                |_| {},
            )
            .unwrap_err(),
        );
        assert_eq!(serialization.code(), "database_export_unsupported");
        assert!(std::fs::read_dir(&root).unwrap().next().is_none());

        let missing_parent = root.join("missing").join("sales.csv");
        let reservation = database_command_error(
            export_database_for_project_with_before_publish(
                &state,
                &current,
                "sales",
                missing_parent.to_string_lossy().as_ref(),
                "csv",
                |_| {},
                |_| {},
            )
            .unwrap_err(),
        );
        assert_eq!(
            reservation.code(),
            "database_export_temp_reservation_failed"
        );

        let blocked_destination = root.join("blocked.csv");
        std::fs::create_dir(&blocked_destination).unwrap();
        let publication = database_command_error(
            export_database_for_project_with_before_publish(
                &state,
                &current,
                "sales",
                blocked_destination.to_string_lossy().as_ref(),
                "csv",
                |_| {},
                |_| {},
            )
            .unwrap_err(),
        );
        assert_eq!(publication.code(), "database_export_publication_failed");
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 1);

        state
            .project_store
            .write()
            .unwrap()
            .databases
            .get_mut("sales")
            .unwrap()
            .state = DatabaseState::Failed {
            error: "broken".into(),
        };
        let computation = database_command_error(
            export_database_for_project_with_before_publish(
                &state,
                &current,
                "sales",
                destination.to_string_lossy().as_ref(),
                "csv",
                |_| {},
                |_| {},
            )
            .unwrap_err(),
        );
        assert_eq!(computation.code(), "database_access_failed");

        let cleanup_target = root.join("cleanup-target");
        std::fs::create_dir(&cleanup_target).unwrap();
        std::fs::write(cleanup_target.join("child"), b"keep").unwrap();
        let cleanup =
            database_command_error(cleanup_export_temporary_file(&cleanup_target).unwrap_err());
        assert_eq!(cleanup.code(), "database_export_cleanup_failed");

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn revisioned_database_command_returns_aggregate_and_emits_exact_mutation_once() {
        let state = ProjectState::new();
        let mut data = ProjectData::new();
        data.databases.insert(
            "writer".into(),
            DatabaseDecl {
                id: DatabaseId::from_existing("writer".into()),
                engine: DatabaseEngine::InMemory {
                    name: "writer".into(),
                },
                schema_version: 1,
                required: false,
                name: "Before".into(),
            },
        );
        state.activate_project_fixture("database-command".into(), data);
        let activated = state.capture_project_session().unwrap();
        let project_instance_id = activated.instance_id.clone();
        let operation_id = OperationId::new();
        let mut events = Vec::new();

        let result = rename_database_with_emitter(
            &state,
            project_instance_id.clone(),
            "writer",
            ResourceRevision::INITIAL,
            "After",
            operation_id,
            |event| events.push(event),
        )
        .unwrap();

        assert_eq!(result.data, ());
        assert_eq!(result.mutation.operation_id, operation_id);
        assert_eq!(
            result.mutation.project_instance_id,
            activated.instance_id.as_str()
        );
        assert_eq!(result.mutation.publication_revision, 1);
        assert_eq!(events.len(), 1);
        let Event::Project(EventProject::ResourceMutationCommitted { result: emitted }) =
            &events[0]
        else {
            panic!("database command emitted a non-canonical event")
        };
        assert_eq!(emitted, &result.mutation);
        assert_eq!(
            serde_json::to_value(&result).unwrap()["data"],
            serde_json::Value::Null
        );

        let event_count = events.len();
        let stale = rename_database_with_emitter(
            &state,
            project_instance_id,
            "writer",
            ResourceRevision::INITIAL,
            "Stale",
            OperationId::new(),
            |event| events.push(event),
        );
        assert!(stale.is_err());
        assert_eq!(events.len(), event_count);
        assert_eq!(
            state.get_data().unwrap().databases["writer"].name.as_ref(),
            "After"
        );
    }

    #[test]
    fn database_command_emitters_cover_import_rename_edit_save_delete_and_rejections() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-database-command-publication-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut project = ProjectData::new();
        project.metadata.project_name = "database command publication".into();
        yss_project::fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
        let csv = root.join("writer.csv");
        std::fs::write(&csv, "value\n1\n").unwrap();
        let state = ProjectState::new();
        let session = state.activate_project_from_path(&root).unwrap();
        let project_instance_id = session.instance_id;
        let mut events = Vec::new();
        let import_operation = OperationId::new();

        let imported = load_database_with_emitter(
            &state,
            project_instance_id.clone(),
            import_operation,
            DatabaseImportSourceDTO::Csv {
                path: csv.to_string_lossy().into_owned(),
                delimiter: ',',
                has_header: true,
                infer_schema_length: None,
            },
            |event| events.push(event),
        )
        .unwrap();
        assert_eq!(imported.mutation.publication_revision, 1);
        assert_exact_event(&events, &imported, 1);
        let database_id = imported.data.id.clone();
        let imported_revision = imported.mutation.deltas[0].to_revision;

        let replay_csv = root.join("replay.csv");
        std::fs::write(&replay_csv, "value\n2\n").unwrap();
        let replay = load_database_with_emitter(
            &state,
            project_instance_id.clone(),
            import_operation,
            DatabaseImportSourceDTO::Csv {
                path: replay_csv.to_string_lossy().into_owned(),
                delimiter: ',',
                has_header: true,
                infer_schema_length: None,
            },
            |event| events.push(event),
        );
        assert_eq!(replay.unwrap_err().code(), "duplicate_operation");
        assert_eq!(events.len(), 1);
        assert_eq!(state.get_data().unwrap().databases.len(), 1);

        let renamed = rename_database_with_emitter(
            &state,
            project_instance_id.clone(),
            &database_id,
            imported_revision,
            "Renamed",
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap();
        assert_eq!(renamed.mutation.publication_revision, 2);
        assert_exact_event(&events, &renamed, 2);
        let renamed_revision = renamed.mutation.deltas[0].to_revision;

        let stale = rename_database_with_emitter(
            &state,
            project_instance_id.clone(),
            &database_id,
            imported_revision,
            "Stale",
            OperationId::new(),
            |event| events.push(event),
        );
        assert!(stale.is_err());
        assert_eq!(events.len(), 2);

        let rejected_operation = OperationId::new();
        let rejected = mutate_database_with_emitter(
            &state,
            project_instance_id.clone(),
            &database_id,
            renamed_revision,
            rejected_operation,
            DatabaseMutation::AddColumn {
                name: "rejected".into(),
                dtype: "Mystery".into(),
            },
            |event| events.push(event),
        );
        assert!(rejected.is_err());
        assert_eq!(events.len(), 2);

        let edited = mutate_database_with_emitter(
            &state,
            project_instance_id.clone(),
            &database_id,
            renamed_revision,
            rejected_operation,
            DatabaseMutation::AddColumn {
                name: "added".into(),
                dtype: "Int64".into(),
            },
            |event| events.push(event),
        )
        .unwrap();
        assert_eq!(edited.mutation.publication_revision, 3);
        assert_exact_event(&events, &edited, 3);
        let edited_revision = edited.mutation.deltas[0].to_revision;

        let saved = save_database_with_emitter(
            &state,
            project_instance_id.clone(),
            &database_id,
            edited_revision,
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap();
        assert_eq!(saved.mutation.publication_revision, 4);
        assert_exact_event(&events, &saved, 4);
        let saved_revision = saved.mutation.deltas[0].to_revision;

        let deleted = delete_database_with_emitter(
            &state,
            project_instance_id,
            &database_id,
            saved_revision,
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap();
        assert_eq!(deleted.mutation.publication_revision, 5);
        assert_exact_event(&events, &deleted, 5);
        assert!(
            !state
                .get_data()
                .unwrap()
                .databases
                .contains_key(&database_id)
        );

        let retry_operation = OperationId::new();
        let failed_import = load_database_with_emitter(
            &state,
            state.capture_project_session().unwrap().instance_id,
            retry_operation,
            DatabaseImportSourceDTO::Csv {
                path: root.join("missing.csv").to_string_lossy().into_owned(),
                delimiter: ',',
                has_header: true,
                infer_schema_length: None,
            },
            |event| events.push(event),
        );
        assert!(failed_import.is_err());
        assert_eq!(events.len(), 5);

        let retry_csv = root.join("retry.csv");
        std::fs::write(&retry_csv, "value\n3\n").unwrap();
        let retried_import = load_database_with_emitter(
            &state,
            state.capture_project_session().unwrap().instance_id,
            retry_operation,
            DatabaseImportSourceDTO::Csv {
                path: retry_csv.to_string_lossy().into_owned(),
                delimiter: ',',
                has_header: true,
                infer_schema_length: None,
            },
            |event| events.push(event),
        )
        .unwrap();
        assert_eq!(retried_import.mutation.publication_revision, 6);
        assert_eq!(
            retried_import.mutation.deltas[0].from_revision,
            ResourceRevision::INITIAL,
        );
        assert_eq!(
            retried_import.mutation.deltas[0].to_revision,
            ResourceRevision::INITIAL.next(),
        );
        assert_exact_event(&events, &retried_import, 6);

        let _ = std::fs::remove_dir_all(root);
    }
}
