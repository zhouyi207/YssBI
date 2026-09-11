use crate::error::CommandError;
use crate::event::{Event, EventProject, emit_project_event_result};
use crate::schema::application_event::ResourceMutationCommandResultDto;
use crate::schema::{
    DatabaseImportSourceDTO, DatabaseMetaResultDto, DatabaseRowsResultDto, LoadDatabaseResultDto,
    SampleDatasetDto,
};
use tauri::{AppHandle, State};
use yss_application::database::samples::{SampleCatalog, SampleError, SampleImportError};
use yss_application::database::{
    self, DatabaseMetaResult, DatabaseMutation, DatabaseMutationResult, DatabaseRowsResult,
    DatabaseUseCaseError, LoadDatabaseResult,
};
use yss_database_edit::EditState;
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::{OperationId, ResourceRevision};

mod error;

use error::database_command_error;

async fn run_on_blocking_pool<F, R>(f: F) -> Result<R, CommandError>
where
    F: FnOnce() -> Result<R, CommandError> + Send + 'static,
    R: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(CommandError::internal)?
}

fn map_application_database_error(error: DatabaseUseCaseError) -> CommandError {
    match error {
        DatabaseUseCaseError::SessionCapture(error) => match error {
            yss_application::execution::SessionCaptureError::Inactive => {
                CommandError::expected("stale_project_lifecycle")
            }
            yss_application::execution::SessionCaptureError::Replacing => {
                CommandError::expected("project_lifecycle_admission_closed")
            }
            yss_application::execution::SessionCaptureError::Recovering => {
                CommandError::expected("project_recovery_required")
                    .with_details(serde_json::json!({ "recoveryRequired": true }))
            }
        },
        DatabaseUseCaseError::SessionChanged(error) => {
            CommandError::diagnosed("database_session_changed", error)
        }
        DatabaseUseCaseError::SessionRefresh(error) => {
            CommandError::diagnosed("database_session_refresh_failed", error)
        }
        DatabaseUseCaseError::Database(error) => database_command_error(error),
        DatabaseUseCaseError::Mutation(error) => {
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

fn serialize_application_database_value<T: serde::Serialize>(
    value: T,
) -> Result<serde_json::Value, CommandError> {
    serde_json::to_value(value)
        .map_err(|error| CommandError::diagnosed("database_serialization_failed", error))
}

fn mutate_database_from_application(
    app: &AppHandle,
    application: &yss_application::execution::ApplicationState,
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

#[tauri::command]
pub async fn load_database(
    app: AppHandle,
    application: State<'_, yss_application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    engine: DatabaseImportSourceDTO,
) -> Result<ResourceMutationCommandResultDto<LoadDatabaseResultDto>, CommandError> {
    let application = application.inner().clone();
    run_on_blocking_pool(move || {
        let result = application
            .load_database_for_application(project_instance_id, operation_id, engine.into())
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

fn map_sample_error(error: SampleError) -> CommandError {
    match error {
        SampleError::NotFound => CommandError::expected("sample_not_found"),
        SampleError::VersionMismatch => CommandError::expected("sample_version_mismatch"),
        SampleError::Integrity => CommandError::expected("sample_integrity_failed"),
        SampleError::InvalidCatalog | SampleError::InvalidPath | SampleError::CatalogDecode(_) => {
            CommandError::diagnosed("sample_catalog_invalid", error)
        }
        SampleError::Unavailable(_) | SampleError::Decode(_) => {
            CommandError::diagnosed("sample_resource_unavailable", error)
        }
    }
}

#[cfg(test)]
mod sample_tests {
    #[test]
    fn sample_rejections_use_the_common_error_wire() {
        for (error, code) in [
            (super::SampleError::NotFound, "sample_not_found"),
            (
                super::SampleError::VersionMismatch,
                "sample_version_mismatch",
            ),
            (super::SampleError::Integrity, "sample_integrity_failed"),
        ] {
            assert_eq!(
                serde_json::to_value(super::map_sample_error(error)).unwrap(),
                serde_json::json!({
                    "code": code, "details": null, "incidentId": null,
                })
            );
        }
    }
}

#[tauri::command]
pub async fn list_sample_datasets(
    catalog: State<'_, SampleCatalog>,
) -> Result<Vec<SampleDatasetDto>, CommandError> {
    let catalog = catalog.inner().clone();
    run_on_blocking_pool(move || {
        Ok(catalog
            .list()
            .map_err(map_sample_error)?
            .into_iter()
            .map(Into::into)
            .collect())
    })
    .await
}

#[tauri::command]
pub async fn import_sample_dataset(
    app: AppHandle,
    application: State<'_, yss_application::execution::ApplicationState>,
    catalog: State<'_, SampleCatalog>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    sample_id: String,
    version: u32,
) -> Result<ResourceMutationCommandResultDto<LoadDatabaseResultDto>, CommandError> {
    let application = application.inner().clone();
    let catalog = catalog.inner().clone();
    run_on_blocking_pool(move || {
        let result = application
            .import_sample_dataset_for_application(
                &catalog,
                project_instance_id,
                operation_id,
                &sample_id,
                version,
            )
            .map_err(|error| match error {
                SampleImportError::Sample(error) => map_sample_error(error),
                SampleImportError::Database(error) => map_application_database_error(error),
            })?;
        let result = load_database_result_to_transport(result);
        emit_application_database_result(&app, &result)?;
        Ok(result)
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
    application: State<yss_application::execution::ApplicationState>,
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
    application: State<'_, yss_application::execution::ApplicationState>,
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

#[tauri::command]
pub fn rename_database(
    app: AppHandle,
    application: State<yss_application::execution::ApplicationState>,
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
    application: State<yss_application::execution::ApplicationState>,
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
    application: State<'_, yss_application::execution::ApplicationState>,
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
    application: State<'_, yss_application::execution::ApplicationState>,
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
    application: State<'_, yss_application::execution::ApplicationState>,
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
    application: State<yss_application::execution::ApplicationState>,
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
    application: State<yss_application::execution::ApplicationState>,
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
    application: State<yss_application::execution::ApplicationState>,
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
    application: State<yss_application::execution::ApplicationState>,
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
    application: State<yss_application::execution::ApplicationState>,
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
    application: State<yss_application::execution::ApplicationState>,
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
    application: State<yss_application::execution::ApplicationState>,
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
    application: State<yss_application::execution::ApplicationState>,
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
    application: State<yss_application::execution::ApplicationState>,
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
    application: State<yss_application::execution::ApplicationState>,
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
/// Use `save_database_changes` to checkpoint the committed dataset and clear edit history.
#[tauri::command]
pub async fn export_database(
    application: State<'_, yss_application::execution::ApplicationState>,
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
    application: State<yss_application::execution::ApplicationState>,
    project_instance_id: ProjectInstanceId,
    id: String,
) -> Result<serde_json::Value, CommandError> {
    let result = application
        .query_database_edit_state_for_application(project_instance_id, id)
        .map_err(map_application_database_error)?;
    serialize_application_database_value(result)
}
