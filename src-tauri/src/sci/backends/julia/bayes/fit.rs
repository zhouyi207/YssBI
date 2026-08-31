use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use polars::prelude::{Column, DataFrame};
use serde_json::json;

use super::predictor::compile_predictor;
use crate::sci::api::bayes::{
    BayesBackend, BayesBackendError, BayesBackendRequest, BayesDataExchangeManifest,
    BayesExchangeColumn, BayesProgressCallback, InferenceResult, ResultArtifactKind,
    ResultArtifactOwner, TaskErrorDetails, TaskProgress,
};
use yss_bayes_model::BayesModelSpec;
use yss_julia_worker::{
    JuliaWorkerError, JuliaWorkerErrorCode, JuliaWorkerManager, JuliaWorkerProgress,
    JuliaWorkerProgressCallback, JuliaWorkerTask, JuliaWorkerTaskDirectory,
};
use yss_tabular_io::write_ipc_dataframe;

#[derive(Debug)]
struct JuliaResultArtifactOwner(JuliaWorkerTaskDirectory);

impl ResultArtifactOwner for JuliaResultArtifactOwner {
    fn cleanup(&self) -> Result<(), Box<str>> {
        self.0
            .cleanup()
            .map_err(|error| error.to_string().into_boxed_str())
    }
}

#[derive(Clone)]
pub struct JuliaBayesBackend {
    app_data_dir: PathBuf,
    worker: JuliaWorkerManager,
}

impl JuliaBayesBackend {
    pub fn new(app_data_dir: impl Into<PathBuf>, worker: JuliaWorkerManager) -> Self {
        Self {
            app_data_dir: app_data_dir.into(),
            worker,
        }
    }
}

impl BayesBackend for JuliaBayesBackend {
    fn fit(&self, request: BayesBackendRequest) -> Result<InferenceResult, BayesBackendError> {
        let task_id = request.task_id.clone();
        let input_table = request.input_table;
        let spec = request.spec;
        let exchange_spec = spec.clone();
        let progress = request.progress.clone();
        report_stage(&progress, "materializing_data");
        let progress_for_input = progress.clone();
        let worker_progress = progress.clone().map(|callback| {
            Arc::new(move |update: JuliaWorkerProgress| {
                callback(TaskProgress {
                    stage: update.stage,
                    completed: update.completed,
                    total: update.total,
                });
            }) as JuliaWorkerProgressCallback
        });
        let mut output = self
            .worker
            .run_task_with_typed_input(
                &self.app_data_dir,
                JuliaWorkerTask {
                    task_id: Some(task_id.clone()),
                    operation: "bayes_fit".to_string(),
                    parameters: json!({ "model": spec }),
                },
                |input_path| {
                    write_exchange_files(input_path, input_table, &task_id, &exchange_spec)
                        .map_err(|diagnostic| {
                            JuliaWorkerError::new(
                                JuliaWorkerErrorCode::InputWriteFailed,
                                diagnostic,
                            )
                        })?;
                    report_stage(&progress_for_input, "loading_model");
                    Ok(())
                },
                worker_progress,
            )
            .map_err(map_worker_error)?;

        report_stage(&progress, "reading_result");
        let mut result = read_inference_result(&output.metadata_path)?;
        if result.artifact_manifest().task_id() != output.task_id {
            return Err(BayesBackendError::with_detail(
                "julia_bayes_result_invalid",
                "Julia Bayesian backend returned a mismatched result.",
                format!(
                    "worker task {} returned artifact manifest for task {}",
                    output.task_id,
                    result.artifact_manifest().task_id()
                ),
            ));
        }

        report_stage(&progress, "writing_artifacts");
        let retains_artifacts = result
            .artifact_manifest()
            .artifacts()
            .iter()
            .any(|artifact| {
                matches!(
                    artifact.kind(),
                    ResultArtifactKind::PosteriorSamples | ResultArtifactKind::PosteriorPredictive
                )
            });
        if retains_artifacts {
            let owner = output.take_task_directory().ok_or_else(|| {
                BayesBackendError::new(
                    "julia_bayes_result_invalid",
                    "Julia Bayesian result has no artifact owner.",
                )
            })?;
            result.set_artifact_owner(JuliaResultArtifactOwner(owner));
        }
        Ok(result)
    }

    fn cancel(&self, task_id: &str) -> Result<(), BayesBackendError> {
        if self.worker.cancel(task_id).map_err(map_worker_error)? {
            self.worker
                .restart_task(task_id)
                .map_err(map_worker_error)?;
        }
        Ok(())
    }
}

fn report_stage(progress: &Option<BayesProgressCallback>, stage: impl Into<String>) {
    if let Some(progress) = progress {
        progress(TaskProgress {
            stage: stage.into(),
            completed: None,
            total: None,
        });
    }
}

fn write_exchange_files(
    input_path: &Path,
    table: Option<DataFrame>,
    task_id: &str,
    spec: &BayesModelSpec,
) -> Result<(), String> {
    let values: [i32; 0] = [];
    let mut dataframe = table.unwrap_or_else(|| {
        DataFrame::new(0, vec![Column::new("__unused".into(), &values)])
            .expect("empty input table is valid")
    });
    write_ipc_dataframe(input_path, &mut dataframe)
        .map_err(|error| format!("Failed to write Julia Bayesian input table: {error}"))?;

    let task_dir = input_path
        .parent()
        .ok_or_else(|| "Julia Bayesian input path has no parent directory.".to_string())?;
    let model_spec_path = task_dir.join("model_spec.json");
    let inference_config_path = task_dir.join("inference_config.json");
    let predictor_kernel_path = task_dir.join("predictor_kernel.jl");
    let likelihood_kernel_path = task_dir.join("likelihood_kernel.jl");
    let exchange_manifest_path = task_dir.join("exchange_manifest.json");
    let output_path = task_dir.join("output.arrow");
    let metadata_path = task_dir.join("metadata.json");

    write_json_file(&model_spec_path, spec, "Bayesian model spec")?;
    write_json_file(
        &inference_config_path,
        spec.sampler(),
        "Bayesian inference config",
    )?;
    let predictor = compile_predictor(spec)?;
    fs::write(&predictor_kernel_path, &predictor.predictor_source)
        .map_err(|error| format!("Failed to write Julia predictor kernel: {error}"))?;
    fs::write(&likelihood_kernel_path, &predictor.likelihood_source)
        .map_err(|error| format!("Failed to write Julia likelihood kernel: {error}"))?;
    let manifest = BayesDataExchangeManifest::new(
        task_id,
        input_path.to_string_lossy().into_owned(),
        model_spec_path.to_string_lossy().into_owned(),
        inference_config_path.to_string_lossy().into_owned(),
        predictor_kernel_path.to_string_lossy().into_owned(),
        likelihood_kernel_path.to_string_lossy().into_owned(),
        predictor.columns,
        output_path.to_string_lossy().into_owned(),
        metadata_path.to_string_lossy().into_owned(),
        dataframe.height(),
        dataframe
            .get_column_names()
            .into_iter()
            .map(|name| BayesExchangeColumn {
                name: name.to_string(),
            })
            .collect(),
    );
    write_json_file(
        &exchange_manifest_path,
        &manifest,
        "Bayesian exchange manifest",
    )
}

fn write_json_file<T: serde::Serialize>(path: &Path, value: &T, label: &str) -> Result<(), String> {
    let file = File::create(path).map_err(|error| format!("Failed to create {label}: {error}"))?;
    serde_json::to_writer_pretty(file, value)
        .map_err(|error| format!("Failed to write {label}: {error}"))
}

fn read_inference_result(path: &Path) -> Result<InferenceResult, BayesBackendError> {
    let contents = fs::read_to_string(path).map_err(|error| {
        BayesBackendError::with_detail(
            "julia_bayes_result_missing",
            "Julia Bayesian backend did not write a result.",
            error.to_string(),
        )
    })?;
    parse_inference_result(&contents)
}

fn parse_inference_result(contents: &str) -> Result<InferenceResult, BayesBackendError> {
    serde_json::from_str(contents).map_err(|error| {
        BayesBackendError::with_detail(
            "julia_bayes_result_invalid",
            "Julia Bayesian backend returned an invalid result.",
            error.to_string(),
        )
    })
}

fn map_worker_error(error: JuliaWorkerError) -> BayesBackendError {
    let (code, diagnostic_message) = match error.code() {
        JuliaWorkerErrorCode::RuntimeUnavailable | JuliaWorkerErrorCode::StartFailed => (
            "julia_bayes_runtime_unavailable",
            "Julia runtime is unavailable for Bayesian inference.",
        ),
        JuliaWorkerErrorCode::EnvironmentUnavailable | JuliaWorkerErrorCode::PackageUnavailable => {
            (
                "julia_bayes_package_unavailable",
                "Julia Bayesian packages are unavailable.",
            )
        }
        JuliaWorkerErrorCode::InvalidParameters => (
            "julia_bayes_invalid_data",
            "Julia Bayesian input validation failed.",
        ),
        JuliaWorkerErrorCode::UnsupportedCapability => (
            "julia_bayes_model_unsupported",
            "The Julia Bayesian backend does not support this model.",
        ),
        JuliaWorkerErrorCode::SamplingFailed => (
            "julia_bayes_sampling_failed",
            "Julia Bayesian sampling failed.",
        ),
        _ => (
            "julia_bayes_backend_failed",
            "Julia Bayesian backend failed.",
        ),
    };
    let details = error
        .details()
        .map(|details| TaskErrorDetails {
            column: details.column.clone(),
            row: details.row,
            parameter: details.parameter.clone(),
            path: details.path.clone(),
        })
        .unwrap_or_default();
    BayesBackendError::with_detail(code, diagnostic_message, error.diagnostic())
        .with_safe_details(details)
}

#[cfg(test)]
mod tests {
    use super::{map_worker_error, parse_inference_result};
    use crate::sci::api::bayes::contract::DiagnosticMetric;
    use yss_julia_worker::JuliaWorkerError;

    #[test]
    fn maps_stable_worker_codes_without_classifying_diagnostic_prose() {
        let internal = JuliaWorkerError::from_json_rpc_error(&serde_json::json!({
            "code": "internal_error",
            "message": "column Turing DomainError unsupported capability"
        }));
        assert_eq!(
            map_worker_error(internal).code,
            "julia_bayes_backend_failed"
        );

        let invalid_data = JuliaWorkerError::from_json_rpc_error(&serde_json::json!({
            "code": "invalid_parameters",
            "message": "opaque diagnostic",
            "data": { "column": "predictor_x" }
        }));
        let mapped = map_worker_error(invalid_data);
        assert_eq!(mapped.code, "julia_bayes_invalid_data");
        assert_eq!(
            mapped.details.and_then(|details| details.column),
            Some("predictor_x".to_string())
        );
    }

    #[test]
    fn parses_julia_bayes_result_with_artifact_manifest() {
        let result = parse_inference_result(
            r#"{
                "summaries": [],
                "diagnostics": {
                    "chains": 1,
                    "drawsPerChain": 20,
                    "warmup": 10,
                    "divergences": 0,
                    "maxTreedepthHits": null,
                    "warnings": [{
                        "code": "ess_too_low",
                        "metric": "ess_tail",
                        "value": 42.5,
                        "threshold": 100.0,
                        "parameter": "beta"
                    }]
                },
                "artifactManifest": {
                    "taskId": "task-1",
                    "artifacts": [
                        {"kind": "summary", "format": "json", "path": "summary.json", "rows": 0},
                        {"kind": "metadata", "format": "json", "path": "metadata.json", "rows": null}
                    ]
                }
            }"#,
        )
        .expect("valid manifest result");

        assert_eq!(result.artifact_manifest().task_id(), "task-1");
        assert_eq!(result.artifact_manifest().artifacts().len(), 2);
        assert_eq!(
            result.artifact_manifest().artifacts()[0].path(),
            "summary.json"
        );
        assert_eq!(result.diagnostics().max_treedepth_hits(), None);
        assert_eq!(result.diagnostics().warnings().len(), 1);
        let warning = &result.diagnostics().warnings()[0];
        assert_eq!(warning.code(), "ess_too_low");
        assert_eq!(warning.metric(), DiagnosticMetric::EssTail);
        assert_eq!(warning.value(), 42.5);
        assert_eq!(warning.threshold(), 100.0);
        assert_eq!(warning.parameter(), "beta");
    }

    #[test]
    fn parses_julia_bayes_result_with_samples_artifact() {
        let result = parse_inference_result(
            r#"{
                "summaries": [],
                "diagnostics": {
                    "chains": 1,
                    "drawsPerChain": 20,
                    "warmup": 10,
                    "divergences": 0,
                    "maxTreedepthHits": 0,
                    "warnings": []
                },
                "artifactManifest": {
                    "taskId": "task-1",
                    "artifacts": [
                        {"kind": "posterior_samples", "format": "arrow_ipc", "path": "output.arrow", "rows": null}
                    ]
                }
            }"#,
        )
        .expect("valid sampled result");

        assert_eq!(
            result.artifact_manifest().artifacts()[0].path(),
            "output.arrow"
        );
    }

    #[test]
    fn rejects_result_without_artifact_manifest() {
        let result = parse_inference_result(
            r#"{
                "summaries": [],
                "diagnostics": {
                    "chains": 4,
                    "drawsPerChain": 2000,
                    "warmup": 1000,
                    "divergences": 0,
                    "maxTreedepthHits": 0,
                    "warnings": []
                }
            }"#,
        )
        .expect_err("artifact manifest is required");

        assert_eq!(result.code, "julia_bayes_result_invalid");
    }
}
