use crate::database::DatabaseInstance;
use crate::error::CommandError;
use crate::event::{Event, EventProject, ResourceMutationResultDto, emit_project_event};
use crate::project::{OperationId, ResourceRevision};
use crate::project::{
    ProjectFilesystemError, ProjectInstanceId, ProjectState, ResourceName, WorksheetDocument,
    WorksheetResourcePath,
};
use polars::prelude::{DataType as PDataType, Series};
use serde::Serialize;
use tauri::{AppHandle, State};

const DEFAULT_MAX_PLOT_POINTS: usize = 10_000;

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

fn series_to_plot_f64(s: &Series) -> Result<Series, CommandError> {
    let dt = s.dtype();
    let casted = if matches!(dt, PDataType::Date) {
        s.cast(&PDataType::Int32)
            .map_err(database_computation_error)?
            .cast(&PDataType::Float64)
            .map_err(database_computation_error)?
    } else if matches!(dt, PDataType::Datetime(_, _)) {
        s.cast(&PDataType::Int64)
            .map_err(database_computation_error)?
            .cast(&PDataType::Float64)
            .map_err(database_computation_error)?
    } else if matches!(dt, PDataType::Time) {
        s.cast(&PDataType::Int64)
            .map_err(database_computation_error)?
            .cast(&PDataType::Float64)
            .map_err(database_computation_error)?
    } else {
        s.cast(&PDataType::Float64)
            .map_err(database_computation_error)?
    };
    Ok(casted)
}

fn plot_format_for_series(series: &Series) -> &'static str {
    match series.dtype() {
        dt if matches!(dt, PDataType::Date) => "date",
        dt if matches!(dt, PDataType::Datetime(_, _)) => "datetime",
        _ => "number",
    }
}

fn subsample_points<T>(mut data: Vec<T>, max_points: usize) -> Vec<T> {
    if data.len() <= max_points {
        return data;
    }
    let stride = (data.len() as f64 / max_points as f64).ceil() as usize;
    if stride <= 1 {
        data.truncate(max_points);
        return data;
    }
    data.into_iter()
        .enumerate()
        .filter_map(|(index, point)| (index % stride == 0).then_some(point))
        .take(max_points)
        .collect()
}

fn compute_plot_column_pair(
    db: &mut DatabaseInstance,
    x_col: &str,
    y_col: &str,
    max_points: Option<usize>,
) -> Result<PlotColumnPairPayload, CommandError> {
    let x_series = db
        .load_column_series(x_col)
        .map_err(database_computation_error)?;
    let y_series = db
        .load_column_series(y_col)
        .map_err(database_computation_error)?;

    let x_cast = series_to_plot_f64(&x_series)?;
    let y_cast = series_to_plot_f64(&y_series)?;

    let x_f64 = x_cast.f64().map_err(database_computation_error)?;
    let y_f64 = y_cast.f64().map_err(database_computation_error)?;

    let mut data: Vec<PlotPoint> = x_f64
        .into_iter()
        .zip(y_f64.into_iter())
        .filter_map(|(ox, oy)| match (ox, oy) {
            (Some(x), Some(y)) if x.is_finite() && y.is_finite() => Some(PlotPoint { x, y }),
            _ => None,
        })
        .collect();

    if data.is_empty() {
        return Err(CommandError::expected("plot_data_empty"));
    }

    let max_points = max_points.unwrap_or(DEFAULT_MAX_PLOT_POINTS);
    data = subsample_points(data, max_points);

    let x_label = x_series.name().to_string();
    let y_label = y_series.name().to_string();

    Ok(PlotColumnPairPayload {
        data,
        x_label: if x_label.is_empty() {
            None
        } else {
            Some(x_label)
        },
        y_label: if y_label.is_empty() {
            None
        } else {
            Some(y_label)
        },
        x_format: plot_format_for_series(&x_series).to_string(),
        y_format: plot_format_for_series(&y_series).to_string(),
    })
}

fn emit_worksheet_result(
    mut emit: impl FnMut(Event),
    result: ResourceMutationResultDto,
) -> ResourceMutationResultDto {
    emit(Event::Project(EventProject::ResourceMutationCommitted {
        result: result.clone(),
    }));
    result
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

fn create_worksheet_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    name: String,
    database_id: Option<String>,
    emit: impl FnMut(Event),
) -> Result<ResourceMutationResultDto, CommandError> {
    let name = ResourceName::parse(&name)
        .map_err(ProjectFilesystemError::from)
        .map_err(CommandError::from)?;
    let requested_path = WorksheetResourcePath::from_name(&name);
    state
        .create_worksheet_resource_transaction(
            &project_instance_id,
            &name,
            database_id,
            operation_id,
        )
        .map(|result| emit_worksheet_result(emit, result))
        .map_err(|error| worksheet_command_error(&requested_path, error))
}

#[tauri::command]
pub fn create_worksheet(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    name: String,
    database_id: Option<String>,
) -> Result<ResourceMutationResultDto, CommandError> {
    create_worksheet_with_emitter(
        state.inner(),
        project_instance_id,
        operation_id,
        name,
        database_id,
        |event| emit_project_event(&app, event),
    )
}

fn duplicate_worksheet_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    worksheet_path: WorksheetResourcePath,
    expected_revision: ResourceRevision,
    emit: impl FnMut(Event),
) -> Result<ResourceMutationResultDto, CommandError> {
    state
        .duplicate_worksheet_resource_transaction(
            &project_instance_id,
            &worksheet_path,
            expected_revision,
            operation_id,
        )
        .map(|result| emit_worksheet_result(emit, result))
        .map_err(|error| worksheet_command_error(&worksheet_path, error))
}

#[tauri::command]
pub fn duplicate_worksheet(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    worksheet_path: WorksheetResourcePath,
    expected_revision: ResourceRevision,
) -> Result<ResourceMutationResultDto, CommandError> {
    duplicate_worksheet_with_emitter(
        state.inner(),
        project_instance_id,
        operation_id,
        worksheet_path,
        expected_revision,
        |event| emit_project_event(&app, event),
    )
}

#[tauri::command]
pub fn load_worksheet(
    state: State<ProjectState>,
    project_instance_id: String,
    worksheet_path: WorksheetResourcePath,
) -> Result<WorksheetDocument, CommandError> {
    let project_instance_id = crate::project::ProjectInstanceId::from_existing(project_instance_id);
    state
        .load_worksheet_document(&project_instance_id, &worksheet_path)
        .map_err(|error| worksheet_command_error(&worksheet_path, error))
}

fn save_worksheet_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    worksheet_path: WorksheetResourcePath,
    expected_revision: ResourceRevision,
    document: WorksheetDocument,
    emit: impl FnMut(Event),
) -> Result<ResourceMutationResultDto, CommandError> {
    state
        .save_worksheet_document(
            &project_instance_id,
            &worksheet_path,
            expected_revision,
            operation_id,
            document,
        )
        .map(|result| emit_worksheet_result(emit, result))
        .map_err(|error| worksheet_command_error(&worksheet_path, error))
}

#[tauri::command]
pub fn save_worksheet(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    worksheet_path: WorksheetResourcePath,
    expected_revision: ResourceRevision,
    document: WorksheetDocument,
) -> Result<ResourceMutationResultDto, CommandError> {
    save_worksheet_with_emitter(
        state.inner(),
        project_instance_id,
        operation_id,
        worksheet_path,
        expected_revision,
        document,
        |event| emit_project_event(&app, event),
    )
}

fn rename_worksheet_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    worksheet_path: WorksheetResourcePath,
    expected_revision: ResourceRevision,
    new_name: String,
    lifecycle_token: u64,
    emit: impl FnMut(Event),
) -> Result<ResourceMutationResultDto, CommandError> {
    let new_name = ResourceName::parse(&new_name)
        .map_err(ProjectFilesystemError::from)
        .map_err(CommandError::from)?;
    state
        .rename_worksheet_resource_transaction(
            &project_instance_id,
            &worksheet_path,
            expected_revision,
            &new_name,
            lifecycle_token,
            operation_id,
        )
        .map(|result| emit_worksheet_result(emit, result))
        .map_err(|error| worksheet_command_error(&worksheet_path, error))
}

#[tauri::command]
pub fn rename_worksheet_resource(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    worksheet_path: WorksheetResourcePath,
    expected_revision: ResourceRevision,
    new_name: String,
    lifecycle_token: u64,
) -> Result<ResourceMutationResultDto, CommandError> {
    rename_worksheet_with_emitter(
        state.inner(),
        project_instance_id,
        operation_id,
        worksheet_path,
        expected_revision,
        new_name,
        lifecycle_token,
        |event| emit_project_event(&app, event),
    )
}

fn remove_worksheet_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    worksheet_path: WorksheetResourcePath,
    expected_revision: ResourceRevision,
    emit: impl FnMut(Event),
) -> Result<ResourceMutationResultDto, CommandError> {
    state
        .remove_worksheet_resource_transaction(
            &project_instance_id,
            &worksheet_path,
            expected_revision,
            operation_id,
        )
        .map(|result| emit_worksheet_result(emit, result))
        .map_err(|error| worksheet_command_error(&worksheet_path, error))
}

#[tauri::command]
pub fn remove_worksheet(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    worksheet_path: WorksheetResourcePath,
    expected_revision: ResourceRevision,
) -> Result<ResourceMutationResultDto, CommandError> {
    remove_worksheet_with_emitter(
        state.inner(),
        project_instance_id,
        operation_id,
        worksheet_path,
        expected_revision,
        |event| emit_project_event(&app, event),
    )
}

fn get_plot_column_pair_for_project(
    state: &ProjectState,
    project_instance_id: &ProjectInstanceId,
    database_id: &str,
    x_col: &str,
    y_col: &str,
    max_points: Option<usize>,
) -> Result<PlotColumnPairPayload, CommandError> {
    state
        .with_database_snapshot_for_project(project_instance_id, database_id, |database| {
            compute_plot_column_pair(database, x_col, y_col, max_points)
        })
        .map_err(CommandError::from)?
}

#[tauri::command]
pub fn get_plot_column_pair(
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    database_id: String,
    x_col: String,
    y_col: String,
    max_points: Option<usize>,
) -> Result<PlotColumnPairPayload, CommandError> {
    get_plot_column_pair_for_project(
        state.inner(),
        &project_instance_id,
        &database_id,
        &x_col,
        &y_col,
        max_points,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::{DatabaseDecl, DatabaseEngine, DatabaseState, EditHistory};
    use crate::event::{Event, EventProject};
    use crate::project::ProjectData;

    fn install_plot_database(state: &ProjectState, project_name: &str) -> ProjectInstanceId {
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
        let dataframe =
            polars::df!("amount" => &[1_i64, 2_i64], "cost" => &[3_i64, 4_i64]).unwrap();
        state.project_store.write().unwrap().databases.insert(
            "sales".into(),
            DatabaseInstance {
                decl,
                state: DatabaseState::Loaded {
                    dataframe: std::sync::Arc::new(dataframe.clone()),
                    original: std::sync::Arc::new(dataframe),
                    history: EditHistory::new(),
                },
            },
        );
        state.capture_project_session().unwrap().instance_id
    }

    #[test]
    fn worksheet_plot_read_rejects_stale_project_identity() {
        let state = ProjectState::new();
        let stale = install_plot_database(&state, "plot-original");
        install_plot_database(&state, "plot-replacement");

        let error =
            get_plot_column_pair_for_project(&state, &stale, "sales", "amount", "cost", None)
                .unwrap_err();

        assert_eq!(error.code(), "stale_project_lifecycle");
    }

    #[test]
    fn worksheet_plot_computation_returns_stable_database_error() {
        let state = ProjectState::new();
        let current = install_plot_database(&state, "plot-computation");

        let error =
            get_plot_column_pair_for_project(&state, &current, "sales", "missing", "cost", None)
                .unwrap_err();

        assert_eq!(error.code(), "database_computation_failed");
    }

    #[test]
    fn worksheet_ipc_errors_serialize_the_approved_resource_contract() {
        let worksheet_path =
            WorksheetResourcePath::parse("worksheets/Sales Report.yssbi-worksheet").unwrap();
        for (source, expected_code, has_incident) in [
            (
                ProjectFilesystemError::WorksheetNotFound {
                    path: worksheet_path.clone(),
                },
                "resource_not_found",
                false,
            ),
            (
                ProjectFilesystemError::ResourceNameConflict {
                    message: "a worksheet named 'Sales Report' already exists".into(),
                },
                "resource_name_conflict",
                false,
            ),
            (
                ProjectFilesystemError::ResourceRevisionConflict {
                    message: "worksheet revision changed".into(),
                },
                "resource_revision_conflict",
                false,
            ),
            (
                ProjectFilesystemError::TransactionPrepareFailed {
                    message: "prepare fault".into(),
                },
                "filesystem_prepare_failed",
                true,
            ),
            (
                ProjectFilesystemError::TransactionCommitFailed {
                    message: "commit fault".into(),
                },
                "filesystem_commit_failed",
                true,
            ),
        ] {
            let error = worksheet_command_error(&worksheet_path, source);
            let serialized = serde_json::to_value(error).unwrap();
            assert_eq!(serialized.as_object().unwrap().len(), 3);
            assert_eq!(serialized["code"], expected_code);
            assert_eq!(
                serialized["details"],
                serde_json::json!({
                    "resourceKind": "worksheet",
                    "resourcePath": worksheet_path.as_str(),
                })
            );
            assert!(serialized.get("message").is_none());
            if has_incident {
                assert!(uuid::Uuid::parse_str(serialized["incidentId"].as_str().unwrap()).is_ok());
            } else {
                assert!(serialized["incidentId"].is_null());
            }
        }

        let recovery = worksheet_command_error(
            &worksheet_path,
            ProjectFilesystemError::TransactionRollbackFailed {
                message: "restore fault".into(),
                recovery_required: true,
            },
        );
        let serialized = serde_json::to_value(recovery).unwrap();
        assert_eq!(serialized.as_object().unwrap().len(), 3);
        assert_eq!(serialized["code"], "publication_recovery_required");
        assert_eq!(
            serialized["details"],
            serde_json::json!({
                "resourceKind": "worksheet",
                "resourcePath": worksheet_path.as_str(),
                "recoveryRequired": true,
            })
        );
        assert!(uuid::Uuid::parse_str(serialized["incidentId"].as_str().unwrap()).is_ok());
        assert!(serialized.get("message").is_none());
        assert!(!serialized.to_string().contains("restore fault"));
    }

    #[test]
    fn worksheet_commands_publish_create_duplicate_save_rename_and_remove_once() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-worksheet-command-publication-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
        state.set_projection_test_hook(std::sync::Arc::new(|| {
            panic!("worksheet mutations must not build graph projection snapshots")
        }));
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        let worksheet_path =
            WorksheetResourcePath::parse("worksheets/Canonical.yssbi-worksheet").unwrap();
        let duplicate_path =
            WorksheetResourcePath::parse("worksheets/Canonical 2.yssbi-worksheet").unwrap();
        let mut events = Vec::new();

        let created = create_worksheet_with_emitter(
            &state,
            project_instance_id.clone(),
            OperationId::new(),
            "Canonical".into(),
            None,
            |event| events.push(event),
        )
        .unwrap();
        let duplicated = duplicate_worksheet_with_emitter(
            &state,
            project_instance_id.clone(),
            OperationId::new(),
            worksheet_path.clone(),
            crate::project::ResourceRevision::INITIAL,
            |event| events.push(event),
        )
        .unwrap();
        let mut document = state.get_data().unwrap().worksheets[&worksheet_path].clone();
        document.chart_type = "line".into();
        let saved = save_worksheet_with_emitter(
            &state,
            project_instance_id.clone(),
            OperationId::new(),
            worksheet_path.clone(),
            crate::project::ResourceRevision::INITIAL,
            document,
            |event| events.push(event),
        )
        .unwrap();
        let renamed = rename_worksheet_with_emitter(
            &state,
            project_instance_id.clone(),
            OperationId::new(),
            worksheet_path,
            crate::project::ResourceRevision::new(1),
            "Renamed".into(),
            1,
            |event| events.push(event),
        )
        .unwrap();
        let removed = remove_worksheet_with_emitter(
            &state,
            project_instance_id.clone(),
            OperationId::new(),
            duplicate_path,
            crate::project::ResourceRevision::INITIAL,
            |event| events.push(event),
        )
        .unwrap();

        assert_eq!(events.len(), 5);
        for (event, result) in
            events
                .iter()
                .zip([&created, &duplicated, &saved, &renamed, &removed])
        {
            assert!(matches!(
                event,
                Event::Project(EventProject::ResourceMutationCommitted { result: emitted })
                    if emitted == result
            ));
        }
        assert!(created.publication_revision < duplicated.publication_revision);
        assert!(duplicated.publication_revision < saved.publication_revision);
        assert!(saved.publication_revision < renamed.publication_revision);
        assert!(renamed.publication_revision < removed.publication_revision);

        let stale = project_instance_id;
        state.activate_project_fixture(
            root.to_string_lossy().into_owned(),
            state.get_data().unwrap(),
        );
        let event_count = events.len();
        let error = create_worksheet_with_emitter(
            &state,
            stale,
            OperationId::new(),
            "Stale".into(),
            None,
            |event| events.push(event),
        )
        .unwrap_err();
        assert_eq!(error.code(), "stale_project_lifecycle");
        assert_eq!(events.len(), event_count);
        let _ = std::fs::remove_dir_all(root);
    }
}
