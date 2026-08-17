use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use polars::prelude::{Column, DataFrame};
use serde_json::json;

use super::predictor::compile_predictor;
use crate::julia::worker::{JuliaWorkerManager, JuliaWorkerProgressCallback, JuliaWorkerTask};
use crate::sci::api::bayes::{
    BayesBackend, BayesBackendError, BayesBackendRequest, BayesDataExchangeManifest,
    BayesExchangeColumn, BayesModelSpec, BayesProgressCallback, InferenceResult,
    ResultArtifactKind, TaskErrorDetails, TaskProgress,
};
use crate::tabular::dataframe_io::write_ipc_dataframe;

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
            Arc::new(move |update: crate::julia::worker::JuliaWorkerProgress| {
                callback(TaskProgress {
                    stage: update.stage,
                    completed: update.completed,
                    total: update.total,
                });
            }) as JuliaWorkerProgressCallback
        });
        let output = self
            .worker
            .run_task(
                &self.app_data_dir,
                JuliaWorkerTask {
                    task_id: Some(task_id.clone()),
                    operation: "bayes_fit".to_string(),
                    parameters: json!({ "model": spec }),
                },
                |input_path| {
                    write_exchange_files(input_path, input_table, &task_id, &exchange_spec)?;
                    report_stage(&progress_for_input, "loading_model");
                    Ok(())
                },
                worker_progress,
            )
            .map_err(map_worker_error)?;

        report_stage(&progress, "reading_result");
        let result = read_inference_result(&output.metadata_path)?;
        report_stage(&progress, "writing_artifacts");
        if !result.artifact_manifest.artifacts.iter().any(|artifact| {
            matches!(
                artifact.kind,
                ResultArtifactKind::PosteriorSamples | ResultArtifactKind::PosteriorPredictive
            )
        }) {
            if let Some(task_dir) = output.output_path.parent() {
                let _ = fs::remove_dir_all(task_dir);
            }
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
        &spec.sampler,
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

fn map_worker_error(message: String) -> BayesBackendError {
    let lower = message.to_ascii_lowercase();
    let (code, diagnostic_message) = if lower.contains("julia was not found")
        || lower.contains("failed to start julia worker")
    {
        (
            "julia_bayes_runtime_unavailable",
            "Julia is not available. Install Julia or fix the system Julia PATH, then try again.",
        )
    } else if lower.contains("unsupported capability:") {
        (
            "julia_bayes_model_unsupported",
            "This Bayesian model is not supported by the current Julia backend.",
        )
    } else if lower.contains("package")
        || lower.contains("turing")
        || lower.contains("distributions")
        || lower.contains("mcmcchains")
        || lower.contains("failed to prepare julia worker packages")
    {
        (
            "julia_bayes_package_unavailable",
            "Julia Bayesian packages are not available. Prepare the Julia worker environment, then try again.",
        )
    } else if lower.contains("column")
        || lower.contains("numeric")
        || lower.contains("finite")
        || lower.contains("observations")
        || lower.contains("invalid_parameters")
    {
        (
            "julia_bayes_invalid_data",
            "The selected data is not valid for Bayesian inference.",
        )
    } else if lower.contains("nuts")
        || lower.contains("sampling")
        || lower.contains("log density")
        || lower.contains("domainerror")
    {
        (
            "julia_bayes_sampling_failed",
            "Julia Bayesian sampling failed.",
        )
    } else {
        (
            "julia_bayes_backend_failed",
            "Julia Bayesian backend failed.",
        )
    };

    let details = TaskErrorDetails {
        column: extract_quoted_detail(&message, "column `"),
        parameter: extract_quoted_detail(&message, "parameter `"),
        ..TaskErrorDetails::default()
    };
    BayesBackendError::with_detail(code, diagnostic_message, message).with_safe_details(details)
}

fn extract_quoted_detail(message: &str, marker: &str) -> Option<String> {
    let lower = message.to_ascii_lowercase();
    let start = lower.find(marker)? + marker.len();
    let value = message.get(start..)?.split('`').next()?;
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return None;
    }
    Some(value.to_string())
}

#[cfg(test)]
mod tests {
    use super::{map_worker_error, parse_inference_result};
    use crate::sci::api::bayes::DiagnosticMetric;

    #[test]
    fn maps_worker_errors_to_stable_codes() {
        let cases = [
            (
                "Julia was not found on the system PATH.",
                "julia_bayes_runtime_unavailable",
            ),
            (
                "LoadError: ArgumentError: Package Turing not found",
                "julia_bayes_package_unavailable",
            ),
            (
                "invalid_parameters: column `x` must contain numeric values",
                "julia_bayes_invalid_data",
            ),
            (
                "UnsupportedBayesCapability: unsupported capability: model shape",
                "julia_bayes_model_unsupported",
            ),
            (
                "ArgumentError: unsupported value encountered while writing samples",
                "julia_bayes_backend_failed",
            ),
            (
                "Task failed: DomainError during NUTS sampling",
                "julia_bayes_sampling_failed",
            ),
        ];

        for (message, expected_code) in cases {
            assert_eq!(map_worker_error(message.to_string()).code, expected_code);
        }
    }

    #[test]
    fn extracts_only_safe_domain_details_from_worker_errors() {
        let column_error = map_worker_error(
            "invalid_parameters: column `predictor_x` must contain numeric values".to_string(),
        );
        assert_eq!(
            column_error.details.and_then(|details| details.column),
            Some("predictor_x".to_string())
        );

        let parameter_error =
            map_worker_error("Predictor parameter `beta` was not declared.".to_string());
        assert_eq!(
            parameter_error
                .details
                .and_then(|details| details.parameter),
            Some("beta".to_string())
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

        assert_eq!(result.artifact_manifest.task_id, "task-1");
        assert_eq!(result.artifact_manifest.artifacts.len(), 2);
        assert_eq!(result.artifact_manifest.artifacts[0].path, "summary.json");
        assert_eq!(result.diagnostics.max_treedepth_hits, None);
        assert_eq!(result.diagnostics.warnings.len(), 1);
        let warning = &result.diagnostics.warnings[0];
        assert_eq!(warning.code, "ess_too_low");
        assert_eq!(warning.metric, DiagnosticMetric::EssTail);
        assert_eq!(warning.value, 42.5);
        assert_eq!(warning.threshold, 100.0);
        assert_eq!(warning.parameter, "beta");
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

        assert_eq!(result.artifact_manifest.artifacts[0].path, "output.arrow");
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
