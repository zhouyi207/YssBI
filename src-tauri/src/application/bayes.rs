use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use std::fs;
use std::path::Path;

use polars::prelude::{DataFrame, Float64Chunked};

use crate::error::AppError;
use crate::project::ProjectState;
use crate::sci::api::bayes::{
    AutocorrelationPlotData, AutocorrelationPoint, AutocorrelationSeries, BayesBackend,
    BayesBackendError, BayesBackendRequest, BayesInferenceTask, BayesModelDraft, BayesModelSpec,
    BayesProgressCallback, DatasetSourceType, DensityPlotData, DensityPoint, DensitySeries,
    InferenceResult, PlaceholderBayesBackend, PosteriorPredictivePage, PosteriorPredictiveRow,
    PosteriorPredictiveSummary, PosteriorSamplePage, PosteriorSampleRow, ResultArtifactKind,
    TaskError, TaskProgress, TaskStatus, TracePlotData, TracePoint, TraceSeries,
    draft_to_model_spec, validate_bayes_input_table, validate_draft,
};
use crate::tabular::dataframe_io::{read_ipc_dataframe, write_csv_dataframe};

#[derive(Clone)]
pub struct BayesInferenceService {
    inner: Arc<Mutex<BayesInferenceState>>,
    backend: Arc<dyn BayesBackend>,
}

#[derive(Default)]
struct BayesInferenceState {
    tasks: HashMap<String, BayesInferenceTask>,
    results: HashMap<String, InferenceResult>,
    queue: VecDeque<BayesBackendJob>,
    runner_active: bool,
}

struct BayesBackendJob {
    task_id: String,
    spec: BayesModelSpec,
    input_table: Option<DataFrame>,
}

impl Default for BayesInferenceService {
    fn default() -> Self {
        Self::new()
    }
}

impl BayesInferenceService {
    pub fn new() -> Self {
        Self::with_backend(Arc::new(PlaceholderBayesBackend))
    }

    pub fn with_backend(backend: Arc<dyn BayesBackend>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BayesInferenceState::default())),
            backend,
        }
    }

    pub fn submit(&self, draft: BayesModelDraft) -> Result<BayesInferenceTask, AppError> {
        self.submit_with_input_table(draft, None)
    }

    pub fn submit_from_project(
        &self,
        draft: BayesModelDraft,
        project_state: &ProjectState,
    ) -> Result<BayesInferenceTask, AppError> {
        let spec = validated_spec(draft)?;
        let input_table = materialize_input_table(&spec, project_state)?;
        validate_bayes_input_table(&spec, &input_table).map_err(input_validation_app_error)?;
        self.submit_spec(spec, Some(input_table))
    }

    fn submit_with_input_table(
        &self,
        draft: BayesModelDraft,
        input_table: Option<DataFrame>,
    ) -> Result<BayesInferenceTask, AppError> {
        let spec = validated_spec(draft)?;
        if let Some(table) = &input_table {
            validate_bayes_input_table(&spec, table).map_err(input_validation_app_error)?;
        }
        self.submit_spec(spec, input_table)
    }

    fn submit_spec(
        &self,
        spec: BayesModelSpec,
        input_table: Option<DataFrame>,
    ) -> Result<BayesInferenceTask, AppError> {
        let task_id = new_task_id();
        let task = queued_task(task_id.clone());
        let should_start_runner = {
            let mut state = self.lock_state()?;
            state.tasks.insert(task_id.clone(), task.clone());
            state.queue.push_back(BayesBackendJob {
                task_id,
                spec,
                input_table,
            });
            if state.runner_active {
                false
            } else {
                state.runner_active = true;
                true
            }
        };

        if should_start_runner {
            let inner = self.inner.clone();
            let backend = self.backend.clone();
            thread::spawn(move || run_backend_queue(inner, backend));
        }

        Ok(task)
    }

    pub fn status(&self, task_id: &str) -> Result<BayesInferenceTask, AppError> {
        let state = self.lock_state()?;
        state.tasks.get(task_id).cloned().ok_or_else(|| {
            AppError::new(
                "bayes_task_not_found",
                format!("Bayesian inference task {task_id} was not found"),
            )
        })
    }

    pub fn cancel(&self, task_id: &str) -> Result<(), AppError> {
        let should_cancel_backend = {
            let mut state = self.lock_state()?;
            let task = state.tasks.get_mut(task_id).ok_or_else(|| {
                AppError::new(
                    "bayes_task_not_found",
                    format!("Bayesian inference task {task_id} was not found"),
                )
            })?;
            match task.status {
                TaskStatus::Queued => {
                    *task = cancelled_task(task_id.to_string());
                    false
                }
                TaskStatus::Running => {
                    task.status = TaskStatus::Cancelling;
                    task.progress = Some(TaskProgress {
                        stage: "cancelling".to_string(),
                        completed: None,
                        total: None,
                    });
                    true
                }
                TaskStatus::Cancelling => false,
                TaskStatus::Completed | TaskStatus::Cancelled | TaskStatus::Failed => false,
            }
        };
        if should_cancel_backend {
            self.backend.cancel(task_id).map_err(|error| {
                AppError::new(
                    "bayes_cancel_failed",
                    format!("Failed to cancel Bayesian inference task {task_id}: {error}"),
                )
            })?;
        }
        Ok(())
    }

    pub fn result(&self, task_id: &str) -> Result<InferenceResult, AppError> {
        let state = self.lock_state()?;
        result_from_state(&state, task_id)
    }

    pub fn clear_task(&self, task_id: &str) -> Result<(), AppError> {
        let result = {
            let mut state = self.lock_state()?;
            let task = state.tasks.get(task_id).ok_or_else(|| {
                AppError::new(
                    "bayes_task_not_found",
                    format!("Bayesian inference task {task_id} was not found"),
                )
            })?;
            if matches!(
                task.status,
                TaskStatus::Queued | TaskStatus::Running | TaskStatus::Cancelling
            ) {
                return Err(AppError::new(
                    "bayes_task_active",
                    format!(
                        "Bayesian inference task {task_id} is still active and cannot be cleared"
                    ),
                ));
            }
            state.tasks.remove(task_id);
            state.results.remove(task_id)
        };
        if let Some(result) = result.as_ref() {
            remove_result_artifacts(result);
        }
        Ok(())
    }

    pub fn export_artifact_csv(
        &self,
        task_id: &str,
        kind: ResultArtifactKind,
        destination: &str,
    ) -> Result<(), AppError> {
        if !matches!(
            kind,
            ResultArtifactKind::PosteriorSamples | ResultArtifactKind::PosteriorPredictive
        ) {
            return Err(AppError::new(
                "bayes_artifact_export_unsupported",
                "Only posterior samples and posterior predictive artifacts can be exported as CSV",
            ));
        }
        let result = {
            let state = self.lock_state()?;
            result_from_state(&state, task_id)?
        };
        let source = artifact_path(&result, kind).ok_or_else(|| {
            AppError::new(
                "bayes_artifact_not_found",
                format!("Bayesian artifact for task {task_id} was not found"),
            )
        })?;
        let mut dataframe = read_bayes_artifact_dataframe(&source, "Bayesian artifact")?;
        write_csv_dataframe(Path::new(destination), &mut dataframe)
            .map_err(|error| AppError::new("bayes_artifact_export_failed", error))
    }

    pub fn sample_page(
        &self,
        task_id: &str,
        offset: usize,
        limit: usize,
        parameter: Option<&str>,
    ) -> Result<PosteriorSamplePage, AppError> {
        let dataframe = self.samples_dataframe(task_id)?;
        posterior_sample_page_from_dataframe(&dataframe, offset, limit, parameter)
    }

    pub fn trace_plot_data(
        &self,
        task_id: &str,
        parameter: Option<&str>,
        max_points_per_chain: usize,
    ) -> Result<TracePlotData, AppError> {
        let dataframe = self.samples_dataframe(task_id)?;
        trace_plot_data_from_dataframe(&dataframe, parameter, max_points_per_chain.max(1))
    }

    pub fn density_plot_data(
        &self,
        task_id: &str,
        parameter: Option<&str>,
        grid_points: usize,
    ) -> Result<DensityPlotData, AppError> {
        let dataframe = self.samples_dataframe(task_id)?;
        density_plot_data_from_dataframe(&dataframe, parameter, grid_points.clamp(8, 256))
    }

    pub fn autocorrelation_plot_data(
        &self,
        task_id: &str,
        parameter: Option<&str>,
        max_lag: usize,
    ) -> Result<AutocorrelationPlotData, AppError> {
        let dataframe = self.samples_dataframe(task_id)?;
        autocorrelation_plot_data_from_dataframe(&dataframe, parameter, max_lag.clamp(1, 512))
    }

    pub fn posterior_predictive_page(
        &self,
        task_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<PosteriorPredictivePage, AppError> {
        let result = {
            let state = self.lock_state()?;
            result_from_state(&state, task_id)?
        };
        let ppc_path =
            artifact_path(&result, ResultArtifactKind::PosteriorPredictive).ok_or_else(|| {
                AppError::new(
                    "bayes_posterior_predictive_not_found",
                    format!("Bayesian posterior predictive data for task {task_id} was not found"),
                )
            })?;
        let dataframe =
            read_bayes_artifact_dataframe(&ppc_path, "Bayesian posterior predictive data")?;
        posterior_predictive_page_from_dataframe(&dataframe, offset, limit)
    }

    fn samples_dataframe(&self, task_id: &str) -> Result<DataFrame, AppError> {
        let result = {
            let state = self.lock_state()?;
            result_from_state(&state, task_id)?
        };
        let samples_path = artifact_path(&result, ResultArtifactKind::PosteriorSamples)
            .ok_or_else(|| {
                AppError::new(
                    "bayes_samples_not_found",
                    format!("Bayesian inference samples for task {task_id} were not found"),
                )
            })?;
        read_bayes_artifact_dataframe(&samples_path, "Bayesian posterior samples")
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, BayesInferenceState>, AppError> {
        self.inner.lock().map_err(|_| {
            AppError::new(
                "bayes_service_lock_poisoned",
                "Bayesian inference service state lock was poisoned",
            )
        })
    }
}

fn run_backend_queue(inner: Arc<Mutex<BayesInferenceState>>, backend: Arc<dyn BayesBackend>) {
    loop {
        let Some(job) = pop_next_backend_job(&inner) else {
            return;
        };
        let task_id = job.task_id.clone();
        let progress = task_progress_callback(inner.clone(), task_id.clone());
        let backend_result = backend.fit(BayesBackendRequest::with_progress(
            task_id.clone(),
            job.spec,
            job.input_table,
            Some(progress),
        ));
        finish_backend_task(&inner, task_id, backend_result);
    }
}

fn task_progress_callback(
    inner: Arc<Mutex<BayesInferenceState>>,
    task_id: String,
) -> BayesProgressCallback {
    Arc::new(move |progress| update_task_progress(&inner, &task_id, progress))
}

fn update_task_progress(
    inner: &Arc<Mutex<BayesInferenceState>>,
    task_id: &str,
    progress: TaskProgress,
) {
    let Ok(mut state) = inner.lock() else {
        return;
    };
    let Some(task) = state.tasks.get_mut(task_id) else {
        return;
    };
    if task.status == TaskStatus::Running {
        task.progress = Some(progress);
    }
}

fn pop_next_backend_job(inner: &Arc<Mutex<BayesInferenceState>>) -> Option<BayesBackendJob> {
    let Ok(mut state) = inner.lock() else {
        return None;
    };

    loop {
        let Some(job) = state.queue.pop_front() else {
            state.runner_active = false;
            return None;
        };
        let Some(task) = state.tasks.get_mut(&job.task_id) else {
            continue;
        };
        match task.status {
            TaskStatus::Queued => {
                task.status = TaskStatus::Running;
                task.progress = Some(TaskProgress {
                    stage: "running".to_string(),
                    completed: None,
                    total: None,
                });
                return Some(job);
            }
            TaskStatus::Cancelling | TaskStatus::Cancelled => {
                let task_id = job.task_id;
                state.tasks.insert(task_id.clone(), cancelled_task(task_id));
            }
            TaskStatus::Running | TaskStatus::Completed | TaskStatus::Failed => {}
        }
    }
}

fn finish_backend_task(
    inner: &Arc<Mutex<BayesInferenceState>>,
    task_id: String,
    backend_result: Result<InferenceResult, BayesBackendError>,
) {
    let Ok(mut state) = inner.lock() else {
        return;
    };
    let Some(current_task) = state.tasks.get(&task_id) else {
        return;
    };
    if matches!(
        current_task.status,
        TaskStatus::Cancelling | TaskStatus::Cancelled
    ) {
        state.tasks.insert(task_id.clone(), cancelled_task(task_id));
        return;
    }

    match backend_result {
        Ok(result) => {
            let completed = completed_task(task_id.clone());
            state.results.insert(task_id.clone(), result);
            state.tasks.insert(task_id, completed);
        }
        Err(error) => {
            state
                .tasks
                .insert(task_id.clone(), failed_task(task_id, error));
        }
    }
}

fn remove_result_artifacts(result: &InferenceResult) {
    let mut directories = BTreeSet::new();
    for artifact in &result.artifact_manifest.artifacts {
        let path = std::path::Path::new(&artifact.path);
        if !path.is_absolute() {
            continue;
        }
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                directories.insert(parent.to_path_buf());
            }
        }
    }
    for directory in directories {
        let _ = fs::remove_dir_all(directory);
    }
}

fn artifact_path(result: &InferenceResult, kind: ResultArtifactKind) -> Option<String> {
    result
        .artifact_manifest
        .artifacts
        .iter()
        .find(|artifact| artifact.kind == kind)
        .map(|artifact| artifact.path.clone())
}

fn result_from_state(
    state: &BayesInferenceState,
    task_id: &str,
) -> Result<InferenceResult, AppError> {
    if !state.tasks.contains_key(task_id) {
        return Err(AppError::new(
            "bayes_task_not_found",
            format!("Bayesian inference task {task_id} was not found"),
        ));
    }
    state.results.get(task_id).cloned().ok_or_else(|| {
        AppError::new(
            "bayes_result_not_found",
            format!("Bayesian inference result for task {task_id} was not found"),
        )
    })
}

fn read_bayes_artifact_dataframe(path: &str, label: &str) -> Result<DataFrame, AppError> {
    read_ipc_dataframe(Path::new(path)).map_err(|error| {
        AppError::new(
            "bayes_result_artifact_read_failed",
            format!("Failed to read {label}: {error}"),
        )
    })
}

fn posterior_sample_page_from_dataframe(
    dataframe: &DataFrame,
    offset: usize,
    limit: usize,
    parameter: Option<&str>,
) -> Result<PosteriorSamplePage, AppError> {
    let parameters = dataframe
        .column("parameter")
        .and_then(|column| column.str())
        .map_err(|error| AppError::new("bayes_samples_invalid", error.to_string()))?;
    let chains = dataframe
        .column("chain")
        .and_then(|column| column.i64())
        .map_err(|error| AppError::new("bayes_samples_invalid", error.to_string()))?;
    let draws = dataframe
        .column("draw")
        .and_then(|column| column.i64())
        .map_err(|error| AppError::new("bayes_samples_invalid", error.to_string()))?;
    let values = dataframe
        .column("value")
        .and_then(|column| column.f64())
        .map_err(|error| AppError::new("bayes_samples_invalid", error.to_string()))?;

    let mut matching_indices = Vec::new();
    for index in 0..dataframe.height() {
        let Some(row_parameter) = parameters.get(index) else {
            continue;
        };
        if parameter.is_none_or(|selected| selected == row_parameter) {
            matching_indices.push(index);
        }
    }

    let total = matching_indices.len();
    let rows = matching_indices
        .into_iter()
        .skip(offset)
        .take(limit)
        .filter_map(|index| {
            Some(PosteriorSampleRow {
                parameter: parameters.get(index)?.to_string(),
                chain: usize::try_from(chains.get(index)?).ok()?,
                draw: usize::try_from(draws.get(index)?).ok()?,
                value: values.get(index)?,
            })
        })
        .collect();

    Ok(PosteriorSamplePage {
        rows,
        offset,
        limit,
        total,
    })
}

fn posterior_predictive_page_from_dataframe(
    dataframe: &DataFrame,
    offset: usize,
    limit: usize,
) -> Result<PosteriorPredictivePage, AppError> {
    let observations = dataframe
        .column("observation")
        .and_then(|column| column.i64())
        .map_err(|error| AppError::new("bayes_posterior_predictive_invalid", error.to_string()))?;
    let transforms = dataframe
        .column("response_transform")
        .and_then(|column| column.str())
        .map_err(|error| AppError::new("bayes_posterior_predictive_invalid", error.to_string()))?;
    let response_transform = transforms.get(0).unwrap_or("identity").to_string();
    if transforms
        .into_iter()
        .flatten()
        .any(|value| value != response_transform)
    {
        return Err(AppError::new(
            "bayes_posterior_predictive_invalid",
            "Posterior predictive rows contain inconsistent response transforms",
        ));
    }
    let observed_model = predictive_f64_column(dataframe, "observed_model")?;
    let mean_model = predictive_f64_column(dataframe, "mean_model")?;
    let q025_model = predictive_f64_column(dataframe, "q025_model")?;
    let q975_model = predictive_f64_column(dataframe, "q975_model")?;
    let observed_original = predictive_f64_column(dataframe, "observed_original")?;
    let mean_original = predictive_f64_column(dataframe, "mean_original")?;
    let q025_original = predictive_f64_column(dataframe, "q025_original")?;
    let q975_original = predictive_f64_column(dataframe, "q975_original")?;

    let total = dataframe.height();
    let rows = (offset..total.min(offset.saturating_add(limit)))
        .filter_map(|index| {
            Some(PosteriorPredictiveRow {
                observation: usize::try_from(observations.get(index)?).ok()?,
                model: PosteriorPredictiveSummary {
                    observed: observed_model.get(index)?,
                    mean: mean_model.get(index)?,
                    q025: q025_model.get(index)?,
                    q975: q975_model.get(index)?,
                },
                original: PosteriorPredictiveSummary {
                    observed: observed_original.get(index)?,
                    mean: mean_original.get(index)?,
                    q025: q025_original.get(index)?,
                    q975: q975_original.get(index)?,
                },
            })
        })
        .collect();

    Ok(PosteriorPredictivePage {
        rows,
        response_transform,
        offset,
        limit,
        total,
    })
}

fn predictive_f64_column<'a>(
    dataframe: &'a DataFrame,
    name: &str,
) -> Result<&'a Float64Chunked, AppError> {
    dataframe
        .column(name)
        .and_then(|column| column.f64())
        .map_err(|error| AppError::new("bayes_posterior_predictive_invalid", error.to_string()))
}

fn trace_plot_data_from_dataframe(
    dataframe: &DataFrame,
    parameter: Option<&str>,
    max_points_per_chain: usize,
) -> Result<TracePlotData, AppError> {
    let rows = sample_rows_from_dataframe(dataframe, parameter)?;
    let mut grouped: BTreeMap<(String, usize), Vec<TracePoint>> = BTreeMap::new();
    for row in rows {
        grouped
            .entry((row.parameter, row.chain))
            .or_default()
            .push(TracePoint {
                draw: row.draw,
                value: row.value,
            });
    }

    let mut stride = 1;
    let series = grouped
        .into_iter()
        .map(|((parameter, chain), mut points)| {
            points.sort_by_key(|point| point.draw);
            let local_stride = points.len().div_ceil(max_points_per_chain).max(1);
            stride = stride.max(local_stride);
            let points = points
                .into_iter()
                .enumerate()
                .filter_map(|(index, point)| (index % local_stride == 0).then_some(point))
                .collect();
            TraceSeries {
                parameter,
                chain,
                points,
            }
        })
        .collect();

    Ok(TracePlotData {
        series,
        max_points_per_chain,
        stride,
    })
}

fn density_plot_data_from_dataframe(
    dataframe: &DataFrame,
    parameter: Option<&str>,
    grid_points: usize,
) -> Result<DensityPlotData, AppError> {
    let rows = sample_rows_from_dataframe(dataframe, parameter)?;
    let mut grouped: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for row in rows {
        grouped.entry(row.parameter).or_default().push(row.value);
    }

    let series = grouped
        .into_iter()
        .map(|(parameter, values)| DensitySeries {
            parameter,
            points: crate::sci::kde::gaussian_kde_grid(&values, grid_points)
                .into_iter()
                .map(|point| DensityPoint {
                    x: point.x,
                    density: point.density,
                })
                .collect(),
        })
        .collect();

    Ok(DensityPlotData {
        series,
        grid_points,
    })
}

fn autocorrelation_plot_data_from_dataframe(
    dataframe: &DataFrame,
    parameter: Option<&str>,
    max_lag: usize,
) -> Result<AutocorrelationPlotData, AppError> {
    let rows = sample_rows_from_dataframe(dataframe, parameter)?;
    let mut grouped: BTreeMap<(String, usize), Vec<PosteriorSampleRow>> = BTreeMap::new();
    for row in rows {
        grouped
            .entry((row.parameter.clone(), row.chain))
            .or_default()
            .push(row);
    }

    let series = grouped
        .into_iter()
        .filter_map(|((parameter, chain), mut rows)| {
            rows.sort_by_key(|row| row.draw);
            let values = rows.into_iter().map(|row| row.value).collect::<Vec<_>>();
            let points = autocorrelation_points(&values, max_lag);
            (!points.is_empty()).then_some(AutocorrelationSeries {
                parameter,
                chain,
                points,
            })
        })
        .collect();

    Ok(AutocorrelationPlotData { series, max_lag })
}

fn autocorrelation_points(values: &[f64], max_lag: usize) -> Vec<AutocorrelationPoint> {
    let values = values
        .iter()
        .copied()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if values.len() < 2 {
        return Vec::new();
    }
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    let variance = values
        .iter()
        .map(|value| {
            let centered = value - mean;
            centered * centered
        })
        .sum::<f64>();
    if variance <= f64::EPSILON {
        return Vec::new();
    }

    let max_lag = max_lag.min(values.len() - 1);
    (0..=max_lag)
        .map(|lag| {
            let covariance = values
                .iter()
                .take(values.len() - lag)
                .zip(values.iter().skip(lag))
                .map(|(left, right)| (left - mean) * (right - mean))
                .sum::<f64>();
            AutocorrelationPoint {
                lag,
                autocorrelation: covariance / variance,
            }
        })
        .collect()
}

fn sample_rows_from_dataframe(
    dataframe: &DataFrame,
    parameter: Option<&str>,
) -> Result<Vec<PosteriorSampleRow>, AppError> {
    let page = posterior_sample_page_from_dataframe(dataframe, 0, usize::MAX, parameter)?;
    Ok(page.rows)
}

fn validated_spec(draft: BayesModelDraft) -> Result<BayesModelSpec, AppError> {
    let report = validate_draft(&draft);
    if !report.ok {
        return Err(AppError::new(
            "bayes_validation_failed",
            "Bayesian model validation failed",
        ));
    }

    draft_to_model_spec(draft).map_err(|_| {
        AppError::new(
            "bayes_validation_failed",
            "Bayesian model validation failed",
        )
    })
}

fn input_validation_app_error(
    error: crate::sci::api::bayes::BayesInputValidationError,
) -> AppError {
    AppError {
        code: error.code.to_string(),
        message: error.message,
        details: Some(serde_json::json!({
            "column": error.column,
            "row": error.row,
        })),
    }
}

fn materialize_input_table(
    spec: &BayesModelSpec,
    project_state: &ProjectState,
) -> Result<DataFrame, AppError> {
    if spec.dataset.source_type != DatasetSourceType::Table {
        return Err(AppError::new(
            "bayes_dataset_source_unsupported",
            "Bayesian inference currently supports project database tables only.",
        ));
    }

    let columns = required_input_columns(spec);
    let column_refs = columns.iter().map(String::as_str).collect::<Vec<_>>();
    project_state
        .with_database_mut(&spec.dataset.source_id, |database| {
            database
                .load_columns(&column_refs)
                .map_err(|error| error.to_string())
        })
        .map_err(|message| AppError::new("bayes_dataset_load_failed", message))
}

fn required_input_columns(spec: &BayesModelSpec) -> Vec<String> {
    let mut columns = spec
        .response
        .data_variables
        .values()
        .cloned()
        .collect::<Vec<_>>();
    for column in spec.data_variables.values() {
        if !columns.iter().any(|existing| existing == column) {
            columns.push(column.clone());
        }
    }
    columns
}

fn new_task_id() -> String {
    static TASK_SEQUENCE: AtomicU64 = AtomicU64::new(1);
    let sequence = TASK_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("bayes-{}-{sequence}", chrono::Utc::now().timestamp_millis())
}

fn queued_task(task_id: String) -> BayesInferenceTask {
    BayesInferenceTask {
        task_id,
        status: TaskStatus::Queued,
        progress: Some(TaskProgress {
            stage: "queued".to_string(),
            completed: None,
            total: None,
        }),
        error: None,
    }
}

fn cancelled_task(task_id: String) -> BayesInferenceTask {
    BayesInferenceTask {
        task_id,
        status: TaskStatus::Cancelled,
        progress: None,
        error: Some(TaskError {
            code: "BAYES_TASK_CANCELLED".to_string(),
            message: "Bayesian inference task was cancelled.".to_string(),
            detail: None,
        }),
    }
}

fn failed_task(task_id: String, error: BayesBackendError) -> BayesInferenceTask {
    BayesInferenceTask {
        task_id,
        status: TaskStatus::Failed,
        progress: None,
        error: Some(TaskError {
            code: error.code,
            message: error.message,
            detail: error.detail,
        }),
    }
}

fn completed_task(task_id: String) -> BayesInferenceTask {
    BayesInferenceTask {
        task_id,
        status: TaskStatus::Completed,
        progress: None,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use polars::prelude::{Column, DataFrame};

    use crate::sci::api::bayes::{
        BayesBackend, BayesBackendError, BayesBackendRequest, BayesModelDraft, BinaryOp,
        ColumnDType, ColumnMeta, DatasetSelection, DatasetSourceType, DiagnosticWarning,
        Expression, InferenceConfig, InferenceDiagnostics, InferenceResult, LikelihoodSpec,
        ParameterConstraint, ParameterRef, ParameterSpec, PredictorSource, PredictorSourceKind,
        PriorSpec, ResponseBinding, ResultArtifactManifest, SamplerAlgorithm, SymbolDraft,
        SymbolRole,
    };

    use super::{
        BayesInferenceService, autocorrelation_plot_data_from_dataframe, completed_task,
        density_plot_data_from_dataframe, posterior_predictive_page_from_dataframe,
        posterior_sample_page_from_dataframe, required_input_columns,
        trace_plot_data_from_dataframe, validated_spec,
    };

    fn empty_result(task_id: String) -> InferenceResult {
        InferenceResult {
            summaries: Vec::new(),
            diagnostics: InferenceDiagnostics {
                chains: 0,
                draws_per_chain: 0,
                warmup: 0,
                divergences: None,
                max_treedepth_hits: None,
                warnings: Vec::new(),
            },
            artifact_manifest: ResultArtifactManifest {
                task_id,
                artifacts: Vec::new(),
            },
        }
    }

    struct CountingBackend {
        calls: Arc<Mutex<usize>>,
    }

    impl BayesBackend for CountingBackend {
        fn fit(&self, request: BayesBackendRequest) -> Result<InferenceResult, BayesBackendError> {
            assert!(request.task_id.starts_with("bayes-"));
            assert_eq!(
                request.spec.response.data_variables.get("y"),
                Some(&"response".to_string())
            );
            assert!(request.input_table.is_none());
            *self.calls.lock().expect("calls lock") += 1;
            Ok(InferenceResult {
                summaries: Vec::new(),
                diagnostics: InferenceDiagnostics {
                    chains: 1,
                    draws_per_chain: 10,
                    warmup: 5,
                    divergences: Some(0),
                    max_treedepth_hits: Some(0),
                    warnings: vec![DiagnosticWarning {
                        code: "TEST_BACKEND".to_string(),
                        message: "test".to_string(),
                        parameter: None,
                    }],
                },
                artifact_manifest: ResultArtifactManifest {
                    task_id: request.task_id,
                    artifacts: Vec::new(),
                },
            })
        }
    }

    struct SlowBackend;

    impl BayesBackend for SlowBackend {
        fn fit(&self, request: BayesBackendRequest) -> Result<InferenceResult, BayesBackendError> {
            if let Some(progress) = request.progress {
                progress(crate::sci::api::bayes::TaskProgress {
                    stage: "test_sampling".to_string(),
                    completed: Some(1),
                    total: Some(2),
                });
            }
            thread::sleep(Duration::from_millis(100));
            Ok(empty_result(request.task_id))
        }
    }

    struct CancellableSlowBackend {
        cancel_calls: Arc<AtomicUsize>,
    }

    impl BayesBackend for CancellableSlowBackend {
        fn fit(&self, request: BayesBackendRequest) -> Result<InferenceResult, BayesBackendError> {
            if let Some(progress) = request.progress {
                progress(crate::sci::api::bayes::TaskProgress {
                    stage: "test_sampling".to_string(),
                    completed: None,
                    total: None,
                });
            }
            thread::sleep(Duration::from_millis(150));
            Ok(empty_result(request.task_id))
        }

        fn cancel(&self, _task_id: &str) -> Result<(), BayesBackendError> {
            self.cancel_calls.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
    }

    struct FailingBackend;

    impl BayesBackend for FailingBackend {
        fn fit(&self, _request: BayesBackendRequest) -> Result<InferenceResult, BayesBackendError> {
            Err(BayesBackendError::new(
                "TEST_BACKEND_FAILED",
                "backend failed",
            ))
        }
    }

    fn valid_draft() -> BayesModelDraft {
        BayesModelDraft {
            formula_text: "y \\sim \\operatorname{Normal}\\left(a * x + b, \\sigma\\right)"
                .to_string(),
            raw_response: crate::sci::api::bayes::RawExpression::Symbol {
                name: "y".to_string(),
            },
            bound_response: Some(Expression::DataVariable {
                name: "y".to_string(),
            }),
            symbols: vec![
                SymbolDraft {
                    name: "y".to_string(),
                    role: SymbolRole::Dependent,
                    inferred_role: SymbolRole::Dependent,
                    user_edited: true,
                },
                SymbolDraft {
                    name: "x".to_string(),
                    role: SymbolRole::Independent,
                    inferred_role: SymbolRole::Independent,
                    user_edited: true,
                },
                SymbolDraft {
                    name: "a".to_string(),
                    role: SymbolRole::Parameter,
                    inferred_role: SymbolRole::Parameter,
                    user_edited: true,
                },
                SymbolDraft {
                    name: "b".to_string(),
                    role: SymbolRole::Parameter,
                    inferred_role: SymbolRole::Parameter,
                    user_edited: true,
                },
            ],
            dataset: Some(DatasetSelection {
                source_type: DatasetSourceType::Table,
                source_id: "demo".to_string(),
                columns: vec![
                    ColumnMeta {
                        name: "response".to_string(),
                        dtype: ColumnDType::Number,
                        nullable: false,
                    },
                    ColumnMeta {
                        name: "time".to_string(),
                        dtype: ColumnDType::Number,
                        nullable: false,
                    },
                ],
            }),
            response_binding: Some(ResponseBinding {
                symbol: "y".to_string(),
                column: "response".to_string(),
            }),
            data_bindings: BTreeMap::from([("x".to_string(), "time".to_string())]),
            bound_predictor: Some(Expression::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expression::Binary {
                    op: BinaryOp::Mul,
                    left: Box::new(Expression::Parameter {
                        name: "a".to_string(),
                    }),
                    right: Box::new(Expression::DataVariable {
                        name: "x".to_string(),
                    }),
                }),
                right: Box::new(Expression::Parameter {
                    name: "b".to_string(),
                }),
            }),
            likelihood: LikelihoodSpec::Normal {
                mean: PredictorSource {
                    source: PredictorSourceKind::Predictor,
                },
                sigma: ParameterRef {
                    parameter: "sigma".to_string(),
                },
            },
            parameters: vec![
                ParameterSpec {
                    name: "a".to_string(),
                    constraint: ParameterConstraint::Real,
                    prior: PriorSpec::Normal([0.0, 10.0]),
                },
                ParameterSpec {
                    name: "b".to_string(),
                    constraint: ParameterConstraint::Real,
                    prior: PriorSpec::Normal([0.0, 10.0]),
                },
                ParameterSpec {
                    name: "sigma".to_string(),
                    constraint: ParameterConstraint::Positive,
                    prior: PriorSpec::Exponential([1.0]),
                },
            ],
            sampler: InferenceConfig {
                algorithm: SamplerAlgorithm::Nuts,
                chains: 4,
                samples: 2_000,
                warmup: 1_000,
                seed: Some(1234),
                target_accept: Some(0.8),
                max_tree_depth: Some(10),
                save_samples: true,
            },
        }
    }

    #[test]
    fn posterior_predictive_page_paginates_rows() {
        let dataframe = DataFrame::new(
            3,
            vec![
                Column::new("observation".into(), &[1_i64, 2, 3]),
                Column::new("response_transform".into(), &["ln", "ln", "ln"]),
                Column::new("observed_model".into(), &[1.0, 2.0, 3.0]),
                Column::new("mean_model".into(), &[1.1, 2.1, 3.1]),
                Column::new("q025_model".into(), &[0.5, 1.4, 2.2]),
                Column::new("q975_model".into(), &[1.8, 2.8, 3.7]),
                Column::new("observed_original".into(), &[3.0, 5.0, 7.0]),
                Column::new("mean_original".into(), &[3.1, 5.1, 6.9]),
                Column::new("q025_original".into(), &[2.5, 4.4, 6.2]),
                Column::new("q975_original".into(), &[3.8, 5.8, 7.7]),
            ],
        )
        .expect("valid ppc dataframe");

        let page = posterior_predictive_page_from_dataframe(&dataframe, 1, 1).expect("ppc page");
        assert_eq!(page.total, 3);
        assert_eq!(page.rows.len(), 1);
        assert_eq!(page.response_transform, "ln");
        assert_eq!(page.rows[0].observation, 2);
        assert_eq!(page.rows[0].model.observed, 2.0);
        assert_eq!(page.rows[0].original.mean, 5.1);
    }

    #[test]
    fn trace_density_and_autocorrelation_plot_data_are_aggregated_from_samples() {
        let dataframe = sample_dataframe();

        let trace = trace_plot_data_from_dataframe(&dataframe, Some("a"), 1).expect("trace data");
        assert_eq!(trace.series.len(), 2);
        assert_eq!(trace.series[0].parameter, "a");
        assert_eq!(trace.series[0].points.len(), 1);

        let density =
            density_plot_data_from_dataframe(&dataframe, Some("b"), 8).expect("density data");
        assert_eq!(density.grid_points, 8);
        assert_eq!(density.series.len(), 1);
        assert_eq!(density.series[0].parameter, "b");
        assert_eq!(density.series[0].points.len(), 8);
        assert!(
            density.series[0]
                .points
                .iter()
                .all(|point| point.density.is_finite() && point.density >= 0.0)
        );

        let autocorrelation =
            autocorrelation_plot_data_from_dataframe(&dataframe, Some("a"), 2).expect("acf data");
        assert_eq!(autocorrelation.max_lag, 2);
        assert_eq!(autocorrelation.series.len(), 1);
        assert_eq!(autocorrelation.series[0].points[0].lag, 0);
        assert!((autocorrelation.series[0].points[0].autocorrelation - 1.0).abs() < 1e-12);
    }

    #[test]
    fn posterior_samples_page_filters_and_paginates() {
        let dataframe = sample_dataframe();

        let page =
            posterior_sample_page_from_dataframe(&dataframe, 1, 2, Some("a")).expect("sample page");

        assert_eq!(page.total, 3);
        assert_eq!(page.rows.len(), 2);
        assert_eq!(page.rows[0].parameter, "a");
        assert_eq!(page.rows[0].chain, 1);
        assert_eq!(page.rows[0].draw, 2);
        assert_eq!(page.rows[0].value, 1.1);
        assert_eq!(page.rows[1].chain, 2);
    }

    fn wait_for_task_stage(
        service: &BayesInferenceService,
        task_id: &str,
        expected_stage: &str,
    ) -> crate::sci::api::bayes::BayesInferenceTask {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let task = service.status(task_id).expect("task status");
            if task
                .progress
                .as_ref()
                .map(|progress| progress.stage.as_str())
                == Some(expected_stage)
            {
                return task;
            }
            assert!(
                Instant::now() < deadline,
                "Bayesian task did not reach expected stage"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn wait_for_terminal_task(
        service: &BayesInferenceService,
        task_id: &str,
    ) -> crate::sci::api::bayes::BayesInferenceTask {
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let task = service.status(task_id).expect("task status");
            if matches!(
                task.status,
                crate::sci::api::bayes::TaskStatus::Completed
                    | crate::sci::api::bayes::TaskStatus::Cancelled
                    | crate::sci::api::bayes::TaskStatus::Failed
            ) {
                return task;
            }
            assert!(
                Instant::now() < deadline,
                "Bayesian task did not finish in time"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn sample_dataframe() -> DataFrame {
        DataFrame::new(
            5,
            vec![
                Column::new("parameter".into(), &["a", "a", "b", "a", "b"]),
                Column::new("chain".into(), &[1_i64, 1, 1, 2, 2]),
                Column::new("draw".into(), &[1_i64, 2, 1, 1, 2]),
                Column::new("value".into(), &[1.0, 1.1, 2.0, 1.2, 2.1]),
            ],
        )
        .expect("valid samples dataframe")
    }

    #[test]
    fn input_materialization_uses_response_and_predictor_columns_once() {
        let spec = validated_spec(valid_draft()).expect("valid spec");
        assert_eq!(required_input_columns(&spec), vec!["response", "time"]);
    }

    #[test]
    fn completed_task_has_terminal_status() {
        let task = completed_task("task".to_string());

        assert_eq!(task.status, crate::sci::api::bayes::TaskStatus::Completed);
        assert!(task.error.is_none());
    }

    #[test]
    fn submit_starts_background_task_and_stores_result() {
        let service = BayesInferenceService::with_backend(Arc::new(SlowBackend));
        let task = service.submit(valid_draft()).expect("submitted task");
        assert_eq!(task.status, crate::sci::api::bayes::TaskStatus::Queued);
        let completed = wait_for_terminal_task(&service, &task.task_id);
        assert_eq!(
            completed.status,
            crate::sci::api::bayes::TaskStatus::Completed
        );
        assert!(service.result(&task.task_id).is_ok());
    }

    #[test]
    fn submit_rejects_invalid_draft() {
        let service = BayesInferenceService::new();
        let mut draft = valid_draft();
        draft.dataset = None;
        let error = service.submit(draft).expect_err("invalid draft rejected");
        assert_eq!(error.code, "bayes_validation_failed");
    }

    #[test]
    fn submit_uses_configured_backend() {
        let calls = Arc::new(Mutex::new(0));
        let service = BayesInferenceService::with_backend(Arc::new(CountingBackend {
            calls: calls.clone(),
        }));
        let task = service.submit(valid_draft()).expect("submitted task");
        let completed = wait_for_terminal_task(&service, &task.task_id);
        assert_eq!(
            completed.status,
            crate::sci::api::bayes::TaskStatus::Completed
        );
        let result = service.result(&task.task_id).expect("stored result");
        assert_eq!(*calls.lock().expect("calls lock"), 1);
        assert_eq!(result.diagnostics.warnings[0].code, "TEST_BACKEND");
    }

    #[test]
    fn backend_progress_updates_task_status() {
        let service = BayesInferenceService::with_backend(Arc::new(SlowBackend));
        let task = service.submit(valid_draft()).expect("submitted task");
        let running = wait_for_task_stage(&service, &task.task_id, "test_sampling");
        assert_eq!(
            running.progress.and_then(|progress| progress.completed),
            Some(1)
        );
        let completed = wait_for_terminal_task(&service, &task.task_id);
        assert_eq!(
            completed.status,
            crate::sci::api::bayes::TaskStatus::Completed
        );
    }

    #[test]
    fn clear_completed_task_removes_status_and_result() {
        let service = BayesInferenceService::with_backend(Arc::new(SlowBackend));
        let task = service.submit(valid_draft()).expect("submitted task");
        let completed = wait_for_terminal_task(&service, &task.task_id);
        assert_eq!(
            completed.status,
            crate::sci::api::bayes::TaskStatus::Completed
        );
        service.clear_task(&task.task_id).expect("clear task");
        assert!(service.status(&task.task_id).is_err());
        assert!(service.result(&task.task_id).is_err());
    }

    #[test]
    fn cancelling_queued_task_does_not_call_backend_cancel() {
        let cancel_calls = Arc::new(AtomicUsize::new(0));
        let service = BayesInferenceService::with_backend(Arc::new(CancellableSlowBackend {
            cancel_calls: cancel_calls.clone(),
        }));
        let running = service.submit(valid_draft()).expect("running task");
        wait_for_task_stage(&service, &running.task_id, "test_sampling");
        let queued = service.submit(valid_draft()).expect("queued task");

        service.cancel(&queued.task_id).expect("cancel queued task");

        assert_eq!(
            service.status(&queued.task_id).unwrap().status,
            crate::sci::api::bayes::TaskStatus::Cancelled
        );
        assert_eq!(cancel_calls.load(Ordering::SeqCst), 0);
        assert_eq!(
            wait_for_terminal_task(&service, &running.task_id).status,
            crate::sci::api::bayes::TaskStatus::Completed
        );
    }

    #[test]
    fn cancelling_running_task_calls_backend_cancel_once() {
        let cancel_calls = Arc::new(AtomicUsize::new(0));
        let service = BayesInferenceService::with_backend(Arc::new(CancellableSlowBackend {
            cancel_calls: cancel_calls.clone(),
        }));
        let task = service.submit(valid_draft()).expect("running task");
        wait_for_task_stage(&service, &task.task_id, "test_sampling");

        service.cancel(&task.task_id).expect("cancel running task");
        service
            .cancel(&task.task_id)
            .expect("repeat cancel running task");

        assert_eq!(cancel_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            wait_for_terminal_task(&service, &task.task_id).status,
            crate::sci::api::bayes::TaskStatus::Cancelled
        );
    }

    #[test]
    fn cancelling_task_prevents_result_publication() {
        let service = BayesInferenceService::with_backend(Arc::new(SlowBackend));
        let task = service.submit(valid_draft()).expect("submitted task");
        service.cancel(&task.task_id).expect("cancel task");
        let cancelled = wait_for_terminal_task(&service, &task.task_id);
        assert_eq!(
            cancelled.status,
            crate::sci::api::bayes::TaskStatus::Cancelled
        );
        assert!(service.result(&task.task_id).is_err());
    }

    #[test]
    fn backend_failure_stores_failed_task() {
        let service = BayesInferenceService::with_backend(Arc::new(FailingBackend));
        let task = service.submit(valid_draft()).expect("failed task returned");
        let failed = wait_for_terminal_task(&service, &task.task_id);
        assert_eq!(failed.status, crate::sci::api::bayes::TaskStatus::Failed);
        assert_eq!(
            failed.error.as_ref().map(|error| error.code.as_str()),
            Some("TEST_BACKEND_FAILED")
        );
        assert!(service.result(&task.task_id).is_err());
    }

    #[test]
    fn unknown_task_returns_error() {
        let service = BayesInferenceService::new();
        let error = service
            .status("missing")
            .expect_err("missing task rejected");
        assert_eq!(error.code, "bayes_task_not_found");
    }
}
