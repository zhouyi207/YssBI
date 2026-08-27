use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fmt;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;

use polars::prelude::{DataFrame, Float64Chunked};

use crate::error::new_diagnostic_incident_id;
use crate::julia::worker::JuliaWorkerTaskDirectory;
use crate::project::{ProjectDatabaseError, ProjectState};
use crate::sci::api::bayes::{
    AutocorrelationPlotData, AutocorrelationPoint, AutocorrelationSeries, BayesBackend,
    BayesBackendError, BayesBackendRequest, BayesInferenceTask, BayesInputValidationError,
    BayesModelDraft, BayesModelSpec, BayesProgressCallback, DatasetSourceType, DensityPlotData,
    DensityPoint, DensitySeries, InferenceResult, PlaceholderBayesBackend, PosteriorPredictivePage,
    PosteriorPredictiveRow, PosteriorPredictiveSummary, PosteriorSamplePage, PosteriorSampleRow,
    ResultArtifactKind, TaskError, TaskProgress, TaskStatus, TracePlotData, TracePoint,
    TraceSeries, draft_to_model_spec, validate_bayes_input_table,
};
use crate::tabular::dataframe_io::{read_ipc_dataframe, write_csv_dataframe};

#[derive(Debug)]
pub enum BayesApplicationError {
    ValidationFailed,
    InputValidation(BayesInputValidationError),
    DatasetSourceUnsupported,
    TaskNotFound,
    TaskActive,
    ResultNotFound,
    ArtifactExportUnsupported,
    ArtifactNotFound,
    SamplesNotFound,
    PosteriorPredictiveNotFound,
    PagingInvalid {
        offset: usize,
        limit: usize,
    },
    CancelFailed {
        task_id: String,
        source: BayesBackendError,
    },
    ServiceLockPoisoned,
    DatasetLoadFailed {
        source: ProjectDatabaseError,
    },
    ArtifactReadFailed {
        context: &'static str,
        source: String,
    },
    ArtifactWriteFailed {
        destination: String,
        source: String,
    },
    SamplesInvalid {
        source: String,
    },
    PosteriorPredictiveInvalid {
        source: String,
    },
    BackendStateInvalid {
        task_id: String,
        status: TaskStatus,
        result_present: bool,
    },
}

impl fmt::Display for BayesApplicationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ValidationFailed => formatter.write_str("Bayesian model validation failed"),
            Self::InputValidation(source) => write!(formatter, "{source}"),
            Self::DatasetSourceUnsupported => {
                formatter.write_str("Bayesian dataset source is unsupported")
            }
            Self::TaskNotFound => formatter.write_str("Bayesian inference task was not found"),
            Self::TaskActive => formatter.write_str("Bayesian inference task is still active"),
            Self::ResultNotFound => formatter.write_str("Bayesian inference result is unavailable"),
            Self::ArtifactExportUnsupported => {
                formatter.write_str("Bayesian artifact cannot be exported as CSV")
            }
            Self::ArtifactNotFound => formatter.write_str("Bayesian artifact was not found"),
            Self::SamplesNotFound => formatter.write_str("Bayesian samples were not found"),
            Self::PosteriorPredictiveNotFound => {
                formatter.write_str("Bayesian posterior predictive data was not found")
            }
            Self::PagingInvalid { offset, limit } => {
                write!(
                    formatter,
                    "invalid Bayesian paging offset={offset} limit={limit}"
                )
            }
            Self::CancelFailed { task_id, source } => {
                write!(
                    formatter,
                    "failed to cancel Bayesian task {task_id}: {source}"
                )
            }
            Self::ServiceLockPoisoned => {
                formatter.write_str("Bayesian inference service state lock was poisoned")
            }
            Self::DatasetLoadFailed { source } => {
                write!(formatter, "failed to load Bayesian dataset: {source}")
            }
            Self::ArtifactReadFailed { context, source } => {
                write!(formatter, "failed to read Bayesian {context}: {source}")
            }
            Self::ArtifactWriteFailed {
                destination,
                source,
            } => write!(
                formatter,
                "failed to write Bayesian artifact to {destination}: {source}"
            ),
            Self::SamplesInvalid { source } => {
                write!(
                    formatter,
                    "Bayesian posterior samples are invalid: {source}"
                )
            }
            Self::PosteriorPredictiveInvalid { source } => write!(
                formatter,
                "Bayesian posterior predictive data is invalid: {source}"
            ),
            Self::BackendStateInvalid {
                task_id,
                status,
                result_present,
            } => write!(
                formatter,
                "Bayesian backend state is inconsistent for task {task_id}: status={status:?}, result_present={result_present}"
            ),
        }
    }
}

impl std::error::Error for BayesApplicationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InputValidation(source) => Some(source),
            Self::CancelFailed { source, .. } => Some(source),
            Self::DatasetLoadFailed { source } => Some(source),
            _ => None,
        }
    }
}

#[derive(Clone)]
pub struct BayesInferenceService {
    inner: Arc<Mutex<BayesInferenceState>>,
    backend: Arc<dyn BayesBackend>,
}

#[derive(Default)]
struct BayesInferenceState {
    tasks: HashMap<String, BayesInferenceTask>,
    results: HashMap<String, StoredInferenceResult>,
    queue: VecDeque<BayesBackendJob>,
    runner_active: bool,
}

struct StoredInferenceResult {
    result: InferenceResult,
    artifact_owner: Option<JuliaWorkerTaskDirectory>,
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

    pub fn submit(
        &self,
        draft: BayesModelDraft,
    ) -> Result<BayesInferenceTask, BayesApplicationError> {
        self.submit_with_input_table(draft, None)
    }

    pub fn submit_from_project(
        &self,
        draft: BayesModelDraft,
        project_state: &ProjectState,
    ) -> Result<BayesInferenceTask, BayesApplicationError> {
        let spec = validated_spec(draft)?;
        let input_table = materialize_input_table(&spec, project_state)?;
        validate_bayes_input_table(&spec, &input_table)
            .map_err(BayesApplicationError::InputValidation)?;
        self.submit_spec(spec, Some(input_table))
    }

    fn submit_with_input_table(
        &self,
        draft: BayesModelDraft,
        input_table: Option<DataFrame>,
    ) -> Result<BayesInferenceTask, BayesApplicationError> {
        let spec = validated_spec(draft)?;
        if let Some(table) = &input_table {
            validate_bayes_input_table(&spec, table)
                .map_err(BayesApplicationError::InputValidation)?;
        }
        self.submit_spec(spec, input_table)
    }

    fn submit_spec(
        &self,
        spec: BayesModelSpec,
        input_table: Option<DataFrame>,
    ) -> Result<BayesInferenceTask, BayesApplicationError> {
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

    pub fn status(&self, task_id: &str) -> Result<BayesInferenceTask, BayesApplicationError> {
        let state = self.lock_state()?;
        state
            .tasks
            .get(task_id)
            .cloned()
            .ok_or(BayesApplicationError::TaskNotFound)
    }

    pub fn cancel(&self, task_id: &str) -> Result<(), BayesApplicationError> {
        let should_cancel_backend = {
            let mut state = self.lock_state()?;
            let task = state
                .tasks
                .get_mut(task_id)
                .ok_or(BayesApplicationError::TaskNotFound)?;
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
            self.backend
                .cancel(task_id)
                .map_err(|source| BayesApplicationError::CancelFailed {
                    task_id: task_id.to_string(),
                    source,
                })?;
        }
        Ok(())
    }

    pub fn result(&self, task_id: &str) -> Result<InferenceResult, BayesApplicationError> {
        let state = self.lock_state()?;
        result_from_state(&state, task_id)
    }

    pub fn clear_task(&self, task_id: &str) -> Result<(), BayesApplicationError> {
        let result = {
            let mut state = self.lock_state()?;
            let task = state
                .tasks
                .get(task_id)
                .ok_or(BayesApplicationError::TaskNotFound)?;
            if matches!(
                task.status,
                TaskStatus::Queued | TaskStatus::Running | TaskStatus::Cancelling
            ) {
                return Err(BayesApplicationError::TaskActive);
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
    ) -> Result<(), BayesApplicationError> {
        if !matches!(
            kind,
            ResultArtifactKind::PosteriorSamples | ResultArtifactKind::PosteriorPredictive
        ) {
            return Err(BayesApplicationError::ArtifactExportUnsupported);
        }
        let result = {
            let state = self.lock_state()?;
            result_from_state(&state, task_id)?
        };
        let source = artifact_path(&result, kind).ok_or(BayesApplicationError::ArtifactNotFound)?;
        let mut dataframe = read_bayes_artifact_dataframe(&source, "artifact")?;
        write_csv_dataframe(Path::new(destination), &mut dataframe).map_err(|source| {
            BayesApplicationError::ArtifactWriteFailed {
                destination: destination.to_string(),
                source,
            }
        })
    }

    pub fn sample_page(
        &self,
        task_id: &str,
        offset: usize,
        limit: usize,
        parameter: Option<&str>,
    ) -> Result<PosteriorSamplePage, BayesApplicationError> {
        validate_paging(offset, limit)?;
        let dataframe = self.samples_dataframe(task_id)?;
        posterior_sample_page_from_dataframe(&dataframe, offset, limit, parameter)
    }

    pub fn trace_plot_data(
        &self,
        task_id: &str,
        parameter: Option<&str>,
        max_points_per_chain: usize,
    ) -> Result<TracePlotData, BayesApplicationError> {
        let dataframe = self.samples_dataframe(task_id)?;
        trace_plot_data_from_dataframe(&dataframe, parameter, max_points_per_chain.max(1))
    }

    pub fn density_plot_data(
        &self,
        task_id: &str,
        parameter: Option<&str>,
        grid_points: usize,
    ) -> Result<DensityPlotData, BayesApplicationError> {
        let dataframe = self.samples_dataframe(task_id)?;
        density_plot_data_from_dataframe(&dataframe, parameter, grid_points.clamp(8, 256))
    }

    pub fn autocorrelation_plot_data(
        &self,
        task_id: &str,
        parameter: Option<&str>,
        max_lag: usize,
    ) -> Result<AutocorrelationPlotData, BayesApplicationError> {
        let dataframe = self.samples_dataframe(task_id)?;
        autocorrelation_plot_data_from_dataframe(&dataframe, parameter, max_lag.clamp(1, 512))
    }

    pub fn posterior_predictive_page(
        &self,
        task_id: &str,
        offset: usize,
        limit: usize,
    ) -> Result<PosteriorPredictivePage, BayesApplicationError> {
        validate_paging(offset, limit)?;
        let result = {
            let state = self.lock_state()?;
            result_from_state(&state, task_id)?
        };
        let ppc_path = artifact_path(&result, ResultArtifactKind::PosteriorPredictive)
            .ok_or(BayesApplicationError::PosteriorPredictiveNotFound)?;
        let dataframe = read_bayes_artifact_dataframe(&ppc_path, "posterior predictive data")?;
        posterior_predictive_page_from_dataframe(&dataframe, offset, limit)
    }

    fn samples_dataframe(&self, task_id: &str) -> Result<DataFrame, BayesApplicationError> {
        let result = {
            let state = self.lock_state()?;
            result_from_state(&state, task_id)?
        };
        let samples_path = artifact_path(&result, ResultArtifactKind::PosteriorSamples)
            .ok_or(BayesApplicationError::SamplesNotFound)?;
        read_bayes_artifact_dataframe(&samples_path, "posterior samples")
    }

    fn lock_state(
        &self,
    ) -> Result<std::sync::MutexGuard<'_, BayesInferenceState>, BayesApplicationError> {
        self.inner
            .lock()
            .map_err(|_| BayesApplicationError::ServiceLockPoisoned)
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
        Ok(mut result) => {
            let completed = completed_task(task_id.clone());
            let artifact_owner = result.take_artifact_owner();
            state.results.insert(
                task_id.clone(),
                StoredInferenceResult {
                    result,
                    artifact_owner,
                },
            );
            state.tasks.insert(task_id, completed);
        }
        Err(error) => {
            state
                .tasks
                .insert(task_id.clone(), failed_task(task_id, error));
        }
    }
}

fn remove_result_artifacts(result: &StoredInferenceResult) {
    let Some(owner) = result.artifact_owner.as_ref() else {
        return;
    };
    if let Err(error) = owner.cleanup() {
        tracing::warn!(
            target: "yssbi::bayes",
            diagnostic_domain = "application",
            error_code = error.code().as_str(),
            diagnostic = error.diagnostic(),
            "Failed to clean owned Bayesian result artifacts"
        );
    }
}

fn artifact_path(result: &InferenceResult, kind: ResultArtifactKind) -> Option<String> {
    result
        .artifact_manifest()
        .artifacts()
        .iter()
        .find(|artifact| artifact.kind() == kind)
        .map(|artifact| artifact.path().to_owned())
}

fn result_from_state(
    state: &BayesInferenceState,
    task_id: &str,
) -> Result<InferenceResult, BayesApplicationError> {
    let task = state
        .tasks
        .get(task_id)
        .ok_or(BayesApplicationError::TaskNotFound)?;
    match state.results.get(task_id) {
        Some(stored) if task.status == TaskStatus::Completed => Ok(stored.result.clone()),
        Some(_) => Err(BayesApplicationError::BackendStateInvalid {
            task_id: task_id.to_string(),
            status: task.status.clone(),
            result_present: true,
        }),
        None if task.status == TaskStatus::Completed => {
            Err(BayesApplicationError::BackendStateInvalid {
                task_id: task_id.to_string(),
                status: task.status.clone(),
                result_present: false,
            })
        }
        None => Err(BayesApplicationError::ResultNotFound),
    }
}

fn read_bayes_artifact_dataframe(
    path: &str,
    context: &'static str,
) -> Result<DataFrame, BayesApplicationError> {
    read_ipc_dataframe(Path::new(path))
        .map_err(|source| BayesApplicationError::ArtifactReadFailed { context, source })
}

fn validate_paging(offset: usize, limit: usize) -> Result<(), BayesApplicationError> {
    if limit == 0 || offset.checked_add(limit).is_none() {
        return Err(BayesApplicationError::PagingInvalid { offset, limit });
    }
    Ok(())
}

fn samples_invalid(source: impl fmt::Display) -> BayesApplicationError {
    BayesApplicationError::SamplesInvalid {
        source: source.to_string(),
    }
}

fn posterior_predictive_invalid(source: impl fmt::Display) -> BayesApplicationError {
    BayesApplicationError::PosteriorPredictiveInvalid {
        source: source.to_string(),
    }
}

fn posterior_sample_page_from_dataframe(
    dataframe: &DataFrame,
    offset: usize,
    limit: usize,
    parameter: Option<&str>,
) -> Result<PosteriorSamplePage, BayesApplicationError> {
    let parameters = dataframe
        .column("parameter")
        .and_then(|column| column.str())
        .map_err(samples_invalid)?;
    let chains = dataframe
        .column("chain")
        .and_then(|column| column.i64())
        .map_err(samples_invalid)?;
    let draws = dataframe
        .column("draw")
        .and_then(|column| column.i64())
        .map_err(samples_invalid)?;
    let values = dataframe
        .column("value")
        .and_then(|column| column.f64())
        .map_err(samples_invalid)?;

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
) -> Result<PosteriorPredictivePage, BayesApplicationError> {
    let observations = dataframe
        .column("observation")
        .and_then(|column| column.i64())
        .map_err(posterior_predictive_invalid)?;
    let transforms = dataframe
        .column("response_transform")
        .and_then(|column| column.str())
        .map_err(posterior_predictive_invalid)?;
    let response_transform = transforms.get(0).unwrap_or("identity").to_string();
    if transforms
        .into_iter()
        .flatten()
        .any(|value| value != response_transform)
    {
        return Err(posterior_predictive_invalid(
            "posterior predictive rows contain inconsistent response transforms",
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
) -> Result<&'a Float64Chunked, BayesApplicationError> {
    dataframe
        .column(name)
        .and_then(|column| column.f64())
        .map_err(posterior_predictive_invalid)
}

fn trace_plot_data_from_dataframe(
    dataframe: &DataFrame,
    parameter: Option<&str>,
    max_points_per_chain: usize,
) -> Result<TracePlotData, BayesApplicationError> {
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
) -> Result<DensityPlotData, BayesApplicationError> {
    let rows = sample_rows_from_dataframe(dataframe, parameter)?;
    let mut grouped: BTreeMap<String, BTreeMap<usize, Vec<f64>>> = BTreeMap::new();
    for row in rows {
        grouped
            .entry(row.parameter)
            .or_default()
            .entry(row.chain)
            .or_default()
            .push(row.value);
    }

    let mut series = Vec::new();
    for (parameter, chains) in grouped {
        let pooled = chains.values().flatten().copied().collect::<Vec<_>>();
        series.push(density_series(&parameter, None, &pooled, grid_points));
        series.extend(
            chains.into_iter().map(|(chain, values)| {
                density_series(&parameter, Some(chain), &values, grid_points)
            }),
        );
    }

    Ok(DensityPlotData {
        series,
        grid_points,
    })
}

fn density_series(
    parameter: &str,
    chain: Option<usize>,
    values: &[f64],
    grid_points: usize,
) -> DensitySeries {
    DensitySeries {
        parameter: parameter.to_string(),
        chain,
        points: crate::sci::api::density::compute_kernel_density(
            crate::sci::api::density::KernelDensityInput {
                values,
                grid_points,
                min_x: None,
            },
        )
        .points
        .into_iter()
        .map(|point| DensityPoint {
            x: point.x,
            density: point.density,
        })
        .collect(),
    }
}

fn autocorrelation_plot_data_from_dataframe(
    dataframe: &DataFrame,
    parameter: Option<&str>,
    max_lag: usize,
) -> Result<AutocorrelationPlotData, BayesApplicationError> {
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
) -> Result<Vec<PosteriorSampleRow>, BayesApplicationError> {
    let page = posterior_sample_page_from_dataframe(dataframe, 0, usize::MAX, parameter)?;
    Ok(page.rows)
}

fn validated_spec(draft: BayesModelDraft) -> Result<BayesModelSpec, BayesApplicationError> {
    draft_to_model_spec(draft).map_err(|_| BayesApplicationError::ValidationFailed)
}

fn materialize_input_table(
    spec: &BayesModelSpec,
    project_state: &ProjectState,
) -> Result<DataFrame, BayesApplicationError> {
    if spec.dataset().source_type != DatasetSourceType::Table {
        return Err(BayesApplicationError::DatasetSourceUnsupported);
    }

    let columns = required_input_columns(spec);
    let column_refs = columns.iter().map(String::as_str).collect::<Vec<_>>();
    project_state
        .with_database_snapshot(&spec.dataset().source_id, |database| {
            database
                .load_columns(&column_refs)
                .map_err(|error| error.to_string())
        })
        .map_err(|source| BayesApplicationError::DatasetLoadFailed { source })
}

fn required_input_columns(spec: &BayesModelSpec) -> Vec<String> {
    let mut columns = spec
        .response()
        .data_variables
        .values()
        .cloned()
        .collect::<Vec<_>>();
    for column in spec.data_variables().values() {
        if !columns.iter().any(|existing| existing == column) {
            columns.push(column.clone());
        }
    }
    columns
}

fn new_task_id() -> String {
    format!("bayes-{}", uuid::Uuid::new_v4())
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
        error: None,
    }
}

fn failed_task(task_id: String, error: BayesBackendError) -> BayesInferenceTask {
    let incident_id = new_diagnostic_incident_id();
    tracing::error!(
        target: "yssbi::bayes",
        diagnostic_domain = "application",
        diagnostic_event = "bayesInferenceFailed",
        task_id = task_id.as_str(),
        error_code = error.code.as_str(),
        incident_id = incident_id.as_str(),
        backend_message = error.message.as_str(),
        backend_detail = error.detail.as_deref().unwrap_or_default(),
        "Bayesian inference task failed"
    );

    let task_error = TaskError {
        code: error.code,
        details: error.details,
        incident_id: Some(incident_id),
    };
    BayesInferenceTask {
        task_id,
        status: TaskStatus::Failed,
        progress: None,
        error: Some(task_error),
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
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    use polars::prelude::{Column, DataFrame};
    use tracing_subscriber::layer::SubscriberExt;
    use uuid::Uuid;

    use crate::diagnostics::dispatcher::DiagnosticsHub;
    use crate::diagnostics::recent_layer::RecentDiagnosticsLayer;
    use crate::julia::worker::JuliaWorkerTaskDirectory;

    use crate::sci::api::bayes::{
        BayesBackend, BayesBackendError, BayesBackendRequest, BayesModelDraft, BinaryOp,
        ColumnDType, ColumnMeta, DatasetSelection, DatasetSourceType, Expression, InferenceConfig,
        InferenceResult, LikelihoodSpec, ParameterConstraint, ParameterRef, ParameterSpec,
        PredictorSource, PredictorSourceKind, PriorSpec, ResponseBinding, SamplerAlgorithm,
        SymbolDraft, SymbolRole, TaskErrorDetails,
    };

    use super::{
        BayesApplicationError, BayesInferenceService, autocorrelation_plot_data_from_dataframe,
        cancelled_task, completed_task, density_plot_data_from_dataframe, failed_task,
        posterior_predictive_page_from_dataframe, posterior_sample_page_from_dataframe,
        required_input_columns, trace_plot_data_from_dataframe, validated_spec,
    };

    struct TemporaryAppRoot {
        path: PathBuf,
    }

    impl TemporaryAppRoot {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!("yssbi-{label}-{}", Uuid::new_v4()));
            fs::create_dir_all(&path).expect("create temporary test root");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TemporaryAppRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    struct CountingBackend {
        calls: Arc<Mutex<usize>>,
    }

    impl BayesBackend for CountingBackend {
        fn fit(&self, request: BayesBackendRequest) -> Result<InferenceResult, BayesBackendError> {
            assert!(request.task_id.starts_with("bayes-"));
            assert_eq!(
                request.spec.response().data_variables.get("y"),
                Some(&"response".to_string())
            );
            assert!(request.input_table.is_none());
            *self.calls.lock().expect("calls lock") += 1;
            serde_json::from_value(serde_json::json!({
                "summaries": [],
                "diagnostics": {
                    "chains": 1,
                    "drawsPerChain": 10,
                    "warmup": 5,
                    "divergences": 0,
                    "maxTreedepthHits": 0,
                    "warnings": [{
                        "code": "test_backend",
                        "metric": "rhat",
                        "value": 1.02,
                        "threshold": 1.01,
                        "parameter": "a"
                    }]
                },
                "artifactManifest": {
                    "taskId": request.task_id,
                    "artifacts": []
                }
            }))
            .map_err(|error| BayesBackendError::new("test_result_invalid", error.to_string()))
        }
    }

    struct OwnedArtifactBackend {
        app_root: PathBuf,
        external_artifact: PathBuf,
        owned_directory: Arc<Mutex<Option<PathBuf>>>,
    }

    impl BayesBackend for OwnedArtifactBackend {
        fn fit(&self, request: BayesBackendRequest) -> Result<InferenceResult, BayesBackendError> {
            let owner = JuliaWorkerTaskDirectory::create(&self.app_root, &request.task_id)
                .expect("create owned Julia worker task directory");
            let owned_directory = owner.path().to_path_buf();
            fs::write(owned_directory.join("output.arrow"), b"owned artifact")
                .expect("write owned artifact");
            *self.owned_directory.lock().expect("owned directory lock") = Some(owned_directory);

            let mut result: InferenceResult = serde_json::from_value(serde_json::json!({
                "summaries": [],
                "diagnostics": {
                    "chains": 1,
                    "drawsPerChain": 1,
                    "warmup": 0,
                    "divergences": 0,
                    "maxTreedepthHits": 0,
                    "warnings": []
                },
                "artifactManifest": {
                    "taskId": request.task_id,
                    "artifacts": [{
                        "kind": "posterior_samples",
                        "format": "arrow_ipc",
                        "path": self.external_artifact.to_string_lossy(),
                        "rows": 1
                    }]
                }
            }))
            .map_err(|error| BayesBackendError::new("test_result_invalid", error.to_string()))?;
            result.set_artifact_owner(owner);
            Ok(result)
        }
    }

    struct FailingBackend;

    impl BayesBackend for FailingBackend {
        fn fit(&self, _request: BayesBackendRequest) -> Result<InferenceResult, BayesBackendError> {
            Err(BayesBackendError::with_detail(
                "test_backend_failed",
                "private asynchronous backend message",
                "private asynchronous backend detail",
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
        assert_eq!(density.series.len(), 3);
        assert_eq!(density.series[0].parameter, "b");
        assert_eq!(density.series[0].chain, None);
        assert_eq!(density.series[1].chain, Some(1));
        assert_eq!(density.series[2].chain, Some(2));
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
    fn cancelled_task_has_no_error_payload() {
        let task = cancelled_task("task".to_string());

        assert_eq!(task.status, crate::sci::api::bayes::TaskStatus::Cancelled);
        assert!(task.error.is_none());
    }

    #[test]
    fn failed_task_logs_internal_diagnostics_and_returns_safe_error() {
        let (hub, _guard) = DiagnosticsHub::start();
        let subscriber =
            tracing_subscriber::registry().with(RecentDiagnosticsLayer::new(hub.clone()));
        let task = tracing::subscriber::with_default(subscriber, || {
            failed_task(
                "task-1".to_string(),
                BayesBackendError::with_detail(
                    "test_backend_failed",
                    "private backend message",
                    "private backend detail",
                )
                .with_safe_details(TaskErrorDetails {
                    column: Some("predictor_x".to_string()),
                    row: Some(7),
                    parameter: Some("beta".to_string()),
                    path: Some("parameters.beta".to_string()),
                }),
            )
        });

        let error = task.error.as_ref().expect("failed task error");
        assert_eq!(error.code, "test_backend_failed");
        assert_eq!(
            error
                .details
                .as_ref()
                .and_then(|details| details.column.as_deref()),
            Some("predictor_x")
        );
        assert_eq!(
            error.details.as_ref().and_then(|details| details.row),
            Some(7)
        );
        assert_eq!(
            error
                .details
                .as_ref()
                .and_then(|details| details.parameter.as_deref()),
            Some("beta")
        );
        let incident_id = error.incident_id.as_deref().expect("failure incident ID");
        assert!(Uuid::parse_str(incident_id).is_ok());

        let wire = serde_json::to_value(&task).expect("serialize failed task");
        assert_eq!(wire["error"]["incidentId"], incident_id);
        assert_eq!(wire["error"]["details"]["column"], "predictor_x");
        assert_eq!(wire["error"]["details"]["row"], 7);
        assert_eq!(
            wire["error"].as_object().expect("task error object").len(),
            3
        );
        assert!(wire["error"].get("message").is_none());
        assert!(wire["error"].get("detail").is_none());
        assert!(!wire.to_string().contains("private backend message"));
        assert!(!wire.to_string().contains("private backend detail"));

        let subscription = hub.subscribe(|_| true).expect("diagnostic subscription");
        let record = subscription
            .entries
            .iter()
            .find(|entry| entry.event.as_deref() == Some("bayesInferenceFailed"))
            .expect("Bayes failure diagnostic");
        assert_eq!(record.fields["error_code"], "test_backend_failed");
        assert_eq!(record.fields["incident_id"], incident_id);
        assert_eq!(record.fields["backend_message"], "private backend message");
        assert_eq!(record.fields["backend_detail"], "private backend detail");
        hub.unsubscribe(subscription.subscription_id)
            .expect("unsubscribe diagnostics");
    }

    #[test]
    fn submit_rejects_invalid_draft() {
        let service = BayesInferenceService::new();
        let mut draft = valid_draft();
        draft.dataset = None;
        let error = service.submit(draft).expect_err("invalid draft rejected");
        assert!(matches!(error, BayesApplicationError::ValidationFailed));
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
        assert_eq!(result.diagnostics().warnings()[0].code(), "test_backend");
    }

    #[test]
    fn backend_failure_stores_failed_task() {
        let service = BayesInferenceService::with_backend(Arc::new(FailingBackend));
        let task = service.submit(valid_draft()).expect("failed task returned");
        let failed = wait_for_terminal_task(&service, &task.task_id);
        assert_eq!(failed.status, crate::sci::api::bayes::TaskStatus::Failed);
        let error = failed.error.as_ref().expect("failed task error");
        assert_eq!(error.code, "test_backend_failed");
        assert!(error.details.is_none());
        assert!(
            Uuid::parse_str(error.incident_id.as_deref().expect("failure incident ID")).is_ok()
        );
        let wire = serde_json::to_string(&failed).expect("serialize asynchronous failure");
        assert!(!wire.contains("private asynchronous backend message"));
        assert!(!wire.contains("private asynchronous backend detail"));
        assert!(service.result(&task.task_id).is_err());
    }

    #[test]
    fn clear_task_deletes_only_owned_worker_task_directory() {
        let test_root = TemporaryAppRoot::new("bayes-artifact-ownership");
        let app_root = test_root.path().join("app");
        fs::create_dir_all(&app_root).expect("create temporary app root");
        let external_directory = test_root.path().join("external");
        fs::create_dir_all(&external_directory).expect("create external artifact directory");
        let external_artifact = external_directory.join("samples.arrow");
        fs::write(&external_artifact, b"external artifact").expect("write external artifact");
        let owned_directory = Arc::new(Mutex::new(None));
        let service = BayesInferenceService::with_backend(Arc::new(OwnedArtifactBackend {
            app_root,
            external_artifact: external_artifact.clone(),
            owned_directory: owned_directory.clone(),
        }));
        let task = service.submit(valid_draft()).expect("submitted task");
        let completed = wait_for_terminal_task(&service, &task.task_id);
        assert_eq!(
            completed.status,
            crate::sci::api::bayes::TaskStatus::Completed
        );
        let owned_directory = owned_directory
            .lock()
            .expect("owned directory lock")
            .clone()
            .expect("owned task directory");
        assert!(owned_directory.exists());

        service.clear_task(&task.task_id).expect("clear task");

        assert!(!owned_directory.exists());
        assert!(external_artifact.exists());
    }

    #[test]
    fn unknown_task_returns_error() {
        let service = BayesInferenceService::new();
        let error = service
            .status("missing")
            .expect_err("missing task rejected");
        assert!(matches!(error, BayesApplicationError::TaskNotFound));
    }
}
