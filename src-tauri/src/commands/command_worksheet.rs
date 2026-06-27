use crate::database::DatabaseInstance;
use crate::project::ProjectState;
use crate::project::{
    WorksheetDocument, delete_worksheet_from_file, ensure_worksheets_dir, existing_worksheet_names,
    load_worksheet_from_file, project_root_from_path, save_worksheet_to_file,
};
use polars::prelude::{DataType as PDataType, Series};
use serde::Serialize;
use tauri::State;

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

fn series_to_plot_f64(s: &Series) -> Result<Series, String> {
    let dt = s.dtype();
    let casted = if matches!(dt, PDataType::Date) {
        s.cast(&PDataType::Int32)
            .map_err(|e| format!("Date->Int32: {}", e))?
            .cast(&PDataType::Float64)
            .map_err(|e| format!("Int32->Float64: {}", e))?
    } else if matches!(dt, PDataType::Datetime(_, _)) {
        s.cast(&PDataType::Int64)
            .map_err(|e| format!("Datetime->Int64: {}", e))?
            .cast(&PDataType::Float64)
            .map_err(|e| format!("Int64->Float64: {}", e))?
    } else if matches!(dt, PDataType::Time) {
        s.cast(&PDataType::Int64)
            .map_err(|e| format!("Time->Int64: {}", e))?
            .cast(&PDataType::Float64)
            .map_err(|e| format!("Int64->Float64: {}", e))?
    } else {
        s.cast(&PDataType::Float64)
            .map_err(|e| format!("cast to Float64: {}", e))?
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
) -> Result<PlotColumnPairPayload, String> {
    let x_series = db
        .load_column_series(x_col)
        .map_err(|e| format!("Failed to load column '{x_col}': {e}"))?;
    let y_series = db
        .load_column_series(y_col)
        .map_err(|e| format!("Failed to load column '{y_col}': {e}"))?;

    let x_cast = series_to_plot_f64(&x_series).map_err(|e| format!("X column: {e}"))?;
    let y_cast = series_to_plot_f64(&y_series).map_err(|e| format!("Y column: {e}"))?;

    let x_f64 = x_cast
        .f64()
        .map_err(|e| format!("X is not plottable: {e}"))?;
    let y_f64 = y_cast
        .f64()
        .map_err(|e| format!("Y is not plottable: {e}"))?;

    let mut data: Vec<PlotPoint> = x_f64
        .into_iter()
        .zip(y_f64.into_iter())
        .filter_map(|(ox, oy)| match (ox, oy) {
            (Some(x), Some(y)) if x.is_finite() && y.is_finite() => Some(PlotPoint { x, y }),
            _ => None,
        })
        .collect();

    if data.is_empty() {
        return Err(
            "No valid (x, y) pairs after filtering nulls and non-finite values".to_string(),
        );
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

fn unique_worksheet_name(existing: &[String], requested: &str) -> String {
    let base = requested.trim();
    let base = if base.is_empty() {
        "New Worksheet"
    } else {
        base
    };
    if !existing.iter().any(|name| name == base) {
        return base.to_string();
    }
    for index in 2.. {
        let candidate = format!("{base} {index}");
        if !existing.iter().any(|name| name == &candidate) {
            return candidate;
        }
    }
    unreachable!("unique worksheet name loop should always return")
}

#[tauri::command]
pub fn create_worksheet(
    state: State<ProjectState>,
    name: Option<String>,
    database_id: Option<String>,
) -> Result<WorksheetDocument, String> {
    let path = state
        .get_path()
        .ok_or_else(|| "No project is open".to_string())?;
    let root = project_root_from_path(&path);
    ensure_worksheets_dir(root.as_path()).map_err(|e| e.to_string())?;

    let existing = existing_worksheet_names(root.as_path(), None).map_err(|e| e.to_string())?;
    let requested = name.unwrap_or_else(|| "New Worksheet".to_string());
    let unique_name = unique_worksheet_name(&existing, &requested);

    let default_db = database_id.or_else(|| {
        state
            .project_store
            .read()
            .ok()
            .and_then(|store| store.databases.keys().next().cloned())
    });

    let document = WorksheetDocument::new(unique_name, default_db.unwrap_or_default());
    save_worksheet_to_file(root.as_path(), &document).map_err(|e| e.to_string())?;
    Ok(document)
}

#[tauri::command]
pub fn load_worksheet(
    state: State<ProjectState>,
    worksheet_id: String,
) -> Result<WorksheetDocument, String> {
    let path = state
        .get_path()
        .ok_or_else(|| "No project is open".to_string())?;
    let root = project_root_from_path(&path);
    load_worksheet_from_file(root.as_path(), &worksheet_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_worksheet(
    state: State<ProjectState>,
    document: WorksheetDocument,
) -> Result<(), String> {
    let path = state
        .get_path()
        .ok_or_else(|| "No project is open".to_string())?;
    let root = project_root_from_path(&path);
    save_worksheet_to_file(root.as_path(), &document).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_worksheet(state: State<ProjectState>, worksheet_id: String) -> Result<(), String> {
    let path = state
        .get_path()
        .ok_or_else(|| "No project is open".to_string())?;
    let root = project_root_from_path(&path);
    delete_worksheet_from_file(root.as_path(), &worksheet_id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_plot_column_pair(
    state: State<ProjectState>,
    database_id: String,
    x_col: String,
    y_col: String,
    max_points: Option<usize>,
) -> Result<PlotColumnPairPayload, String> {
    state.with_database_mut(&database_id, |db| {
        compute_plot_column_pair(db, &x_col, &y_col, max_points)
    })
}
