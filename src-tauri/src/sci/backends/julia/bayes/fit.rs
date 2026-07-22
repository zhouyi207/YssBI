use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use polars::prelude::{Column, DataFrame};
use serde_json::json;

use crate::julia::worker::{JuliaWorkerManager, JuliaWorkerProgressCallback, JuliaWorkerTask};
use crate::sci::api::bayes::{
    BayesBackend, BayesBackendError, BayesBackendRequest, BayesDataExchangeManifest,
    BayesExchangeColumn, BayesModelSpec, BayesProgressCallback, InferenceResult,
    ResultArtifactKind, TaskProgress,
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
    let exchange_manifest_path = task_dir.join("exchange_manifest.json");
    let output_path = task_dir.join("output.arrow");
    let metadata_path = task_dir.join("metadata.json");

    write_json_file(&model_spec_path, spec, "Bayesian model spec")?;
    write_json_file(
        &inference_config_path,
        &spec.sampler,
        "Bayesian inference config",
    )?;
    let manifest = BayesDataExchangeManifest::new(
        task_id,
        input_path.to_string_lossy().into_owned(),
        model_spec_path.to_string_lossy().into_owned(),
        inference_config_path.to_string_lossy().into_owned(),
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
            "JULIA_BAYES_RESULT_MISSING",
            "Julia Bayesian backend did not write a result.",
            error.to_string(),
        )
    })?;
    parse_inference_result(&contents)
}

fn parse_inference_result(contents: &str) -> Result<InferenceResult, BayesBackendError> {
    serde_json::from_str(contents).map_err(|error| {
        BayesBackendError::with_detail(
            "JULIA_BAYES_RESULT_INVALID",
            "Julia Bayesian backend returned an invalid result.",
            error.to_string(),
        )
    })
}

fn map_worker_error(message: String) -> BayesBackendError {
    let lower = message.to_ascii_lowercase();
    let (code, user_message) = if lower.contains("julia was not found")
        || lower.contains("failed to start julia worker")
    {
        (
            "JULIA_BAYES_RUNTIME_UNAVAILABLE",
            "Julia is not available. Install Julia or fix the system Julia PATH, then try again.",
        )
    } else if lower.contains("unsupported capability:") {
        (
            "JULIA_BAYES_MODEL_UNSUPPORTED",
            "This Bayesian model is not supported by the current Julia backend.",
        )
    } else if lower.contains("package")
        || lower.contains("turing")
        || lower.contains("distributions")
        || lower.contains("mcmcchains")
        || lower.contains("failed to prepare julia worker packages")
    {
        (
            "JULIA_BAYES_PACKAGE_UNAVAILABLE",
            "Julia Bayesian packages are not available. Prepare the Julia worker environment, then try again.",
        )
    } else if lower.contains("column")
        || lower.contains("numeric")
        || lower.contains("finite")
        || lower.contains("observations")
        || lower.contains("invalid_parameters")
    {
        (
            "JULIA_BAYES_INVALID_DATA",
            "The selected data is not valid for Bayesian inference.",
        )
    } else if lower.contains("nuts")
        || lower.contains("sampling")
        || lower.contains("log density")
        || lower.contains("domainerror")
    {
        (
            "JULIA_BAYES_SAMPLING_FAILED",
            "Julia Bayesian sampling failed.",
        )
    } else {
        (
            "JULIA_BAYES_BACKEND_FAILED",
            "Julia Bayesian backend failed.",
        )
    };

    BayesBackendError::with_detail(code, user_message, message)
}

#[cfg(test)]
mod tests {
    use super::{map_worker_error, parse_inference_result};

    #[test]
    fn maps_worker_errors_to_stable_codes() {
        let cases = [
            (
                "Julia was not found on the system PATH.",
                "JULIA_BAYES_RUNTIME_UNAVAILABLE",
            ),
            (
                "LoadError: ArgumentError: Package Turing not found",
                "JULIA_BAYES_PACKAGE_UNAVAILABLE",
            ),
            (
                "invalid_parameters: column `x` must contain numeric values",
                "JULIA_BAYES_INVALID_DATA",
            ),
            (
                "UnsupportedBayesCapability: unsupported capability: model shape",
                "JULIA_BAYES_MODEL_UNSUPPORTED",
            ),
            (
                "ArgumentError: unsupported value encountered while writing samples",
                "JULIA_BAYES_BACKEND_FAILED",
            ),
            (
                "Task failed: DomainError during NUTS sampling",
                "JULIA_BAYES_SAMPLING_FAILED",
            ),
        ];

        for (message, expected_code) in cases {
            assert_eq!(map_worker_error(message.to_string()).code, expected_code);
        }
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
                    "warnings": []
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

        assert_eq!(result.code, "JULIA_BAYES_RESULT_INVALID");
    }
}
