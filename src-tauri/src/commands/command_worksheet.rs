use crate::database::DatabaseInstance;
use crate::error::AppError;
use crate::event::{Event, EventProject, emit_project_event};
use crate::node_system::document::OperationId;
use crate::project::project_writers::WorksheetMutationResultDto;
use crate::project::{ProjectInstanceId, ProjectState, WorksheetDocument};
use polars::prelude::{DataType as PDataType, Series};
use serde::Serialize;
use tauri::{AppHandle, State};

const DEFAULT_MAX_PLOT_POINTS: usize = 10_000;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlotPoint {
    x: f64,
    y: f64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlotColumnPairPayload {
    data: Vec<PlotPoint>,
    x_label: Option<String>,
    y_label: Option<String>,
    x_format: String,
    y_format: String,
}

fn series_to_plot_f64(s: &Series) -> Result<Series, AppError> {
    let dt = s.dtype();
    let casted = if matches!(dt, PDataType::Date) {
        s.cast(&PDataType::Int32)
            .map_err(AppError::internal)?
            .cast(&PDataType::Float64)
            .map_err(AppError::internal)?
    } else if matches!(dt, PDataType::Datetime(_, _)) {
        s.cast(&PDataType::Int64)
            .map_err(AppError::internal)?
            .cast(&PDataType::Float64)
            .map_err(AppError::internal)?
    } else if matches!(dt, PDataType::Time) {
        s.cast(&PDataType::Int64)
            .map_err(AppError::internal)?
            .cast(&PDataType::Float64)
            .map_err(AppError::internal)?
    } else {
        s.cast(&PDataType::Float64).map_err(AppError::internal)?
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
) -> Result<PlotColumnPairPayload, AppError> {
    let x_series = db.load_column_series(x_col).map_err(AppError::internal)?;
    let y_series = db.load_column_series(y_col).map_err(AppError::internal)?;

    let x_cast = series_to_plot_f64(&x_series)?;
    let y_cast = series_to_plot_f64(&y_series)?;

    let x_f64 = x_cast.f64().map_err(AppError::internal)?;
    let y_f64 = y_cast.f64().map_err(AppError::internal)?;

    let mut data: Vec<PlotPoint> = x_f64
        .into_iter()
        .zip(y_f64.into_iter())
        .filter_map(|(ox, oy)| match (ox, oy) {
            (Some(x), Some(y)) if x.is_finite() && y.is_finite() => Some(PlotPoint { x, y }),
            _ => None,
        })
        .collect();

    if data.is_empty() {
        return Err(AppError::new(
            "plot_data_empty",
            "No valid (x, y) pairs after filtering nulls and non-finite values",
        ));
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
    result: WorksheetMutationResultDto,
) -> WorksheetMutationResultDto {
    emit(Event::Project(EventProject::ResourceMutationCommitted {
        result: result.result.clone(),
    }));
    result
}

fn create_worksheet_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    name: Option<String>,
    database_id: Option<String>,
    emit: impl FnMut(Event),
) -> Result<WorksheetMutationResultDto, AppError> {
    state
        .create_worksheet_document(&project_instance_id, name, database_id, operation_id)
        .map(|result| emit_worksheet_result(emit, result))
        .map_err(AppError::from)
}

#[tauri::command]
pub fn create_worksheet(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    name: Option<String>,
    database_id: Option<String>,
) -> Result<WorksheetMutationResultDto, AppError> {
    create_worksheet_with_emitter(
        state.inner(),
        project_instance_id,
        operation_id,
        name,
        database_id,
        |event| emit_project_event(&app, event),
    )
}

#[tauri::command]
pub fn load_worksheet(
    state: State<ProjectState>,
    project_instance_id: String,
    worksheet_id: String,
) -> Result<WorksheetDocument, AppError> {
    let project_instance_id = crate::project::ProjectInstanceId::from_existing(project_instance_id);
    state
        .load_worksheet_document(&project_instance_id, &worksheet_id)
        .map_err(AppError::from)
}

fn save_worksheet_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    document: WorksheetDocument,
    emit: impl FnMut(Event),
) -> Result<WorksheetMutationResultDto, AppError> {
    state
        .save_worksheet_document(&project_instance_id, document, operation_id)
        .map(|result| emit_worksheet_result(emit, result))
        .map_err(AppError::from)
}

#[tauri::command]
pub fn save_worksheet(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    document: WorksheetDocument,
) -> Result<WorksheetMutationResultDto, AppError> {
    save_worksheet_with_emitter(
        state.inner(),
        project_instance_id,
        operation_id,
        document,
        |event| emit_project_event(&app, event),
    )
}

fn delete_worksheet_with_emitter(
    state: &ProjectState,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    worksheet_id: &str,
    emit: impl FnMut(Event),
) -> Result<WorksheetMutationResultDto, AppError> {
    state
        .delete_worksheet_document(&project_instance_id, worksheet_id, operation_id)
        .map(|result| emit_worksheet_result(emit, result))
        .map_err(AppError::from)
}

#[tauri::command]
pub fn delete_worksheet(
    app: AppHandle,
    state: State<ProjectState>,
    project_instance_id: ProjectInstanceId,
    operation_id: OperationId,
    worksheet_id: String,
) -> Result<WorksheetMutationResultDto, AppError> {
    delete_worksheet_with_emitter(
        state.inner(),
        project_instance_id,
        operation_id,
        &worksheet_id,
        |event| emit_project_event(&app, event),
    )
}

#[tauri::command]
pub fn get_plot_column_pair(
    state: State<ProjectState>,
    database_id: String,
    x_col: String,
    y_col: String,
    max_points: Option<usize>,
) -> Result<PlotColumnPairPayload, AppError> {
    state
        .with_database_snapshot(&database_id, |db| {
            compute_plot_column_pair(db, &x_col, &y_col, max_points).map_err(|e| e.message)
        })
        .map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Event, EventProject};
    use crate::project::ProjectData;

    #[test]
    fn worksheet_commands_preserve_identity_operation_emit_once_and_reject_stale() {
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
        let (names, default_database_id) = state.worksheet_creation_snapshot().unwrap();
        assert!(names.is_empty());
        assert_eq!(default_database_id, None);
        let project_instance_id = state.capture_project_session().unwrap().instance_id;
        let create_operation_id = crate::node_system::document::OperationId::new();
        let save_operation_id = crate::node_system::document::OperationId::new();
        let delete_operation_id = crate::node_system::document::OperationId::new();
        let mut events = Vec::new();

        let created = create_worksheet_with_emitter(
            &state,
            project_instance_id.clone(),
            create_operation_id,
            Some("Canonical".into()),
            None,
            |event| events.push(event),
        )
        .unwrap();
        let worksheet = created.document.clone();
        let saved = save_worksheet_with_emitter(
            &state,
            project_instance_id.clone(),
            save_operation_id,
            worksheet.clone(),
            |event| events.push(event),
        )
        .unwrap();
        let deleted = delete_worksheet_with_emitter(
            &state,
            project_instance_id.clone(),
            delete_operation_id,
            &worksheet.id,
            |event| events.push(event),
        )
        .unwrap();

        assert_eq!(events.len(), 3);
        assert_eq!(created.document.name, "Canonical");
        assert_eq!(created.document.revision.get(), 0);
        assert_eq!(saved.document.id, worksheet.id);
        assert_eq!(saved.document.revision.get(), 1);
        assert_eq!(deleted.document, saved.document);
        for (event, result) in events
            .iter()
            .zip([&created.result, &saved.result, &deleted.result])
        {
            assert!(matches!(
                event,
                Event::Project(EventProject::ResourceMutationCommitted { result: emitted })
                    if emitted == result
            ));
        }
        assert_eq!(
            created.result.project_instance_id,
            project_instance_id.as_str()
        );
        assert_eq!(created.operation_id, create_operation_id);
        assert_eq!(saved.operation_id, save_operation_id);
        assert_eq!(deleted.operation_id, delete_operation_id);
        assert!(created.result.publication_revision < saved.result.publication_revision);
        assert!(saved.result.publication_revision < deleted.result.publication_revision);

        let stale = project_instance_id;
        state.activate_project_fixture(
            root.to_string_lossy().into_owned(),
            state.get_data().unwrap(),
        );
        let event_count = events.len();
        let error = create_worksheet_with_emitter(
            &state,
            stale,
            crate::node_system::document::OperationId::new(),
            Some("Stale".into()),
            None,
            |event| events.push(event),
        )
        .unwrap_err();
        assert_eq!(error.code, "stale_project_lifecycle");
        assert_eq!(events.len(), event_count);
        let _ = std::fs::remove_dir_all(root);
    }
}
