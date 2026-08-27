use crate::application::database::{self, DatabaseApplicationError, DatabaseMutation};
#[cfg(test)]
use crate::application::database::{
    cleanup_export_temporary_file, export_database_for_project_with_before_publish,
};
use crate::error::CommandError;
use crate::event::{Event, EventProject, ResourceMutationCommandResultDto, emit_project_event};
use crate::node_system::document::{OperationId, ResourceRevision};
use crate::project::{ProjectInstanceId, ProjectState};
use crate::schema::DatabaseImportSourceDTO;
use tauri::{AppHandle, State};

mod error;
mod types;

use error::database_command_error;
use types::dataframe_to_row_matrix;

fn emit_database_result<T>(
    result: &ResourceMutationCommandResultDto<T>,
    mut emit: impl FnMut(Event),
) {
    emit(Event::Project(EventProject::ResourceMutationCommitted {
        result: result.mutation.clone(),
    }));
}

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

fn mutate_database_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    id: &str,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    mutation: DatabaseMutation,
    emit: impl FnMut(Event),
) -> Result<ResourceMutationCommandResultDto<crate::database::EditState>, CommandError> {
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

fn save_database_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    id: &str,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    emit: impl FnMut(Event),
) -> Result<ResourceMutationCommandResultDto<crate::database::EditState>, CommandError> {
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

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseRowsPayload {
    rows: Vec<Vec<serde_json::Value>>,
    row_ids: Vec<i64>,
}

fn serialize_database_value<T: serde::Serialize>(
    value: T,
) -> Result<serde_json::Value, CommandError> {
    serde_json::to_value(value)
        .map_err(|error| CommandError::diagnosed("database_serialization_failed", error))
}

fn get_database_meta_for_project(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
) -> Result<serde_json::Value, CommandError> {
    let result = database::read_database_meta(state, project_instance_id, id)
        .map_err(database_command_error)?;
    serialize_database_value(result)
}

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

fn get_column_stats_for_project(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
) -> Result<serde_json::Value, CommandError> {
    let stats = database::read_column_statistics(state, project_instance_id, id)
        .map_err(database_command_error)?;
    serialize_database_value(stats)
}

fn get_column_distribution_for_project(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
) -> Result<serde_json::Value, CommandError> {
    let distributions = database::read_column_distributions(state, project_instance_id, id)
        .map_err(database_command_error)?;
    serialize_database_value(distributions)
}

fn get_dataset_overview_for_project(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    id: &str,
) -> Result<serde_json::Value, CommandError> {
    let overview = database::read_dataset_overview(state, project_instance_id, id)
        .map_err(database_command_error)?;
    serialize_database_value(overview)
}

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
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    engine: DatabaseImportSourceDTO,
) -> Result<
    ResourceMutationCommandResultDto<crate::application::database::LoadDatabaseResult>,
    CommandError,
> {
    let state = state.inner().clone();
    run_on_blocking_pool(move || {
        load_database_with_emitter(&state, project_instance_id, operation_id, engine, |event| {
            emit_project_event(&app, event)
        })
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
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
) -> Result<serde_json::Value, CommandError> {
    get_database_meta_for_project(state.inner(), &project_instance_id, &id)
}

#[tauri::command]
pub async fn delete_database(
    app: AppHandle,
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ResourceMutationCommandResultDto<()>, CommandError> {
    let state = state.inner().clone();
    run_on_blocking_pool(move || {
        delete_database_with_emitter(
            &state,
            project_instance_id,
            &id,
            expected_revision,
            operation_id,
            |event| emit_project_event(&app, event),
        )
    })
    .await
}

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
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    name: String,
    operation_id: OperationId,
) -> Result<ResourceMutationCommandResultDto<()>, CommandError> {
    rename_database_with_emitter(
        state.inner(),
        project_instance_id,
        &id,
        expected_revision,
        &name,
        operation_id,
        |event| emit_project_event(&app, event),
    )
}

#[tauri::command]
pub fn get_database_rows(
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    offset: usize,
    limit: usize,
) -> Result<serde_json::Value, CommandError> {
    get_database_rows_for_project(state.inner(), &project_instance_id, &id, offset, limit)
}

#[tauri::command]
pub async fn get_column_stats(
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
) -> Result<serde_json::Value, CommandError> {
    let state = state.inner().clone();
    run_on_blocking_pool(move || get_column_stats_for_project(&state, &project_instance_id, &id))
        .await
}

#[tauri::command]
pub async fn get_column_distribution(
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
) -> Result<serde_json::Value, CommandError> {
    let state = state.inner().clone();
    run_on_blocking_pool(move || {
        get_column_distribution_for_project(&state, &project_instance_id, &id)
    })
    .await
}

#[tauri::command]
pub async fn get_dataset_overview(
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
) -> Result<serde_json::Value, CommandError> {
    let state = state.inner().clone();
    run_on_blocking_pool(move || {
        get_dataset_overview_for_project(&state, &project_instance_id, &id)
    })
    .await
}

// ==================== Edit Commands ====================

#[tauri::command]
pub fn edit_cell(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    row: usize,
    col_name: String,
    value: serde_json::Value,
    row_id: Option<i64>,
) -> Result<ResourceMutationCommandResultDto<crate::database::EditState>, CommandError> {
    mutate_database_with_emitter(
        state.inner(),
        project_instance_id,
        &id,
        expected_revision,
        operation_id,
        DatabaseMutation::EditCell {
            row,
            column: col_name,
            value,
            row_id,
        },
        |event| emit_project_event(&app, event),
    )
}

#[tauri::command]
pub fn add_row(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    index: Option<usize>,
) -> Result<ResourceMutationCommandResultDto<crate::database::EditState>, CommandError> {
    mutate_database_with_emitter(
        state.inner(),
        project_instance_id,
        &id,
        expected_revision,
        operation_id,
        DatabaseMutation::AddRow { index },
        |event| emit_project_event(&app, event),
    )
}

#[tauri::command]
pub fn delete_rows(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    indices: Vec<usize>,
    row_ids: Option<Vec<i64>>,
) -> Result<ResourceMutationCommandResultDto<crate::database::EditState>, CommandError> {
    mutate_database_with_emitter(
        state.inner(),
        project_instance_id,
        &id,
        expected_revision,
        operation_id,
        DatabaseMutation::DeleteRows { indices, row_ids },
        |event| emit_project_event(&app, event),
    )
}

#[tauri::command]
pub fn add_column(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    name: String,
    dtype: String,
) -> Result<ResourceMutationCommandResultDto<crate::database::EditState>, CommandError> {
    mutate_database_with_emitter(
        state.inner(),
        project_instance_id,
        &id,
        expected_revision,
        operation_id,
        DatabaseMutation::AddColumn { name, dtype },
        |event| emit_project_event(&app, event),
    )
}

#[tauri::command]
pub fn delete_column(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    name: String,
) -> Result<ResourceMutationCommandResultDto<crate::database::EditState>, CommandError> {
    mutate_database_with_emitter(
        state.inner(),
        project_instance_id,
        &id,
        expected_revision,
        operation_id,
        DatabaseMutation::DeleteColumn { name },
        |event| emit_project_event(&app, event),
    )
}

#[tauri::command]
pub fn cast_column(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    col_name: String,
    new_dtype: String,
    force: Option<bool>,
) -> Result<ResourceMutationCommandResultDto<crate::database::EditState>, CommandError> {
    mutate_database_with_emitter(
        state.inner(),
        project_instance_id,
        &id,
        expected_revision,
        operation_id,
        DatabaseMutation::CastColumn {
            column: col_name,
            dtype: new_dtype,
            force: force.unwrap_or(false),
        },
        |event| emit_project_event(&app, event),
    )
}

#[tauri::command]
pub fn rename_column(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
    old_name: String,
    new_name: String,
) -> Result<ResourceMutationCommandResultDto<crate::database::EditState>, CommandError> {
    mutate_database_with_emitter(
        state.inner(),
        project_instance_id,
        &id,
        expected_revision,
        operation_id,
        DatabaseMutation::RenameColumn { old_name, new_name },
        |event| emit_project_event(&app, event),
    )
}

#[tauri::command]
pub fn undo_edit(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ResourceMutationCommandResultDto<crate::database::EditState>, CommandError> {
    mutate_database_with_emitter(
        state.inner(),
        project_instance_id,
        &id,
        expected_revision,
        operation_id,
        DatabaseMutation::Undo,
        |event| emit_project_event(&app, event),
    )
}

#[tauri::command]
pub fn redo_edit(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ResourceMutationCommandResultDto<crate::database::EditState>, CommandError> {
    mutate_database_with_emitter(
        state.inner(),
        project_instance_id,
        &id,
        expected_revision,
        operation_id,
        DatabaseMutation::Redo,
        |event| emit_project_event(&app, event),
    )
}

#[tauri::command]
pub fn save_database_changes(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    expected_revision: ResourceRevision,
    operation_id: OperationId,
) -> Result<ResourceMutationCommandResultDto<crate::database::EditState>, CommandError> {
    save_database_with_emitter(
        state.inner(),
        project_instance_id,
        &id,
        expected_revision,
        operation_id,
        |event| emit_project_event(&app, event),
    )
}

/// Export the current dataset view (including unsaved in-memory edits) to an external file.
/// Use `save_database_changes` to persist edits into `project.duckdb`.
#[tauri::command]
pub async fn export_database(
    state: State<'_, ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
    path: String,
    format: String,
) -> Result<(), CommandError> {
    let state = state.inner().clone();
    run_on_blocking_pool(move || {
        database::export_database_for_project(&state, &project_instance_id, &id, &path, &format)
            .map_err(database_command_error)
    })
    .await
}

#[tauri::command]
pub fn get_edit_state(
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    id: String,
) -> Result<serde_json::Value, CommandError> {
    get_edit_state_for_project(state.inner(), &project_instance_id, &id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::DatabaseState;
    use crate::database_contract::{DatabaseDecl, DatabaseEngine};
    use crate::event::{Event, EventProject};
    use crate::node_system::document::{OperationId, ResourceRevision};
    use crate::project::ProjectData;

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
            id: "sales".into(),
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
            crate::database::DatabaseInstance {
                decl,
                state: crate::database::DatabaseState::Loaded {
                    dataframe: std::sync::Arc::new(dataframe.clone()),
                    original: std::sync::Arc::new(dataframe),
                    history: crate::database::EditHistory::new(),
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
                id: "writer".into(),
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
        assert_eq!(state.get_data().unwrap().databases["writer"].name, "After");
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
        crate::project::fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
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
