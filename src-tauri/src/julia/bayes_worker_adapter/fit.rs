use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use polars::prelude::{Column, DataFrame, IpcWriter, SerWriter};
use serde::{Deserialize, Serialize};

use super::JuliaTaskCompletion;
use super::predictor::{JuliaGeneratedModel, JuliaModelGenerationError, generate_julia_model};
use crate::sci::api::bayes::contract::{InferenceDiagnostics, ParameterSummary};
use crate::sci::api::bayes::worker::{
    ArtifactId, BayesArtifactHandle, BayesArtifactMediaType, BayesInferenceSnapshot,
    BayesTaskHandle, BayesTaskResult, BayesWorkerError, ValidatedBayesTask,
};
use crate::sci::api::bayes::{Expression, InferenceConfig, LikelihoodSpec, ParameterSpec};
use crate::sci::api::computation::{CategoricalRole, StatisticalInput, StatisticalScalar};
use yss_julia_worker::{
    JuliaWorkerError, JuliaWorkerErrorCode, JuliaWorkerManager, JuliaWorkerTask,
    JuliaWorkerTaskDirectory,
};

pub(super) struct PreparedJuliaTask {
    model: serde_json::Value,
    sampler: serde_json::Value,
    generated: JuliaGeneratedModel,
    inputs: Box<[StatisticalInput]>,
}

impl PreparedJuliaTask {
    pub(super) fn try_from_task(task: &ValidatedBayesTask) -> Result<Self, JuliaWorkerError> {
        let generated = generate_julia_model(task.model()).map_err(model_generation_error)?;
        let response = response_input(task.inputs(), task.model().data_variables())?;
        let payload = JuliaModelPayload {
            response: JuliaResponsePayload {
                expression: JuliaResponseExpression {
                    kind: "data_variable",
                    name: "response",
                },
                data_variables: BTreeMap::from([("response", response)]),
            },
            predictor: task.model().predictor(),
            data_variables: task.model().data_variables(),
            likelihood: task.model().likelihood(),
            parameters: task.model().parameters(),
            sampler: task.model().sampler(),
        };
        let model = serde_json::to_value(payload).map_err(|_| task_generation_error())?;
        let sampler =
            serde_json::to_value(task.model().sampler()).map_err(|_| task_generation_error())?;
        Ok(Self {
            model,
            sampler,
            generated,
            inputs: task.inputs().to_vec().into_boxed_slice(),
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JuliaModelPayload<'a> {
    response: JuliaResponsePayload<'a>,
    predictor: &'a Expression,
    data_variables: &'a BTreeMap<String, String>,
    likelihood: &'a LikelihoodSpec,
    parameters: &'a [ParameterSpec],
    sampler: &'a InferenceConfig,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JuliaResponsePayload<'a> {
    expression: JuliaResponseExpression,
    data_variables: BTreeMap<&'static str, &'a str>,
}

#[derive(Serialize)]
struct JuliaResponseExpression {
    #[serde(rename = "type")]
    kind: &'static str,
    name: &'static str,
}

fn response_input<'a>(
    inputs: &'a [StatisticalInput],
    predictor_bindings: &BTreeMap<String, String>,
) -> Result<&'a str, JuliaWorkerError> {
    let mut candidates = inputs
        .iter()
        .filter(|input| {
            !predictor_bindings
                .values()
                .any(|column| column == input.name())
        })
        .map(StatisticalInput::name);
    let Some(response) = candidates.next() else {
        return Err(task_generation_error());
    };
    if candidates.next().is_some() {
        return Err(task_generation_error());
    }
    Ok(response)
}

fn model_generation_error(error: JuliaModelGenerationError) -> JuliaWorkerError {
    tracing::warn!(
        target: "yssbi::julia::bayes_worker_adapter",
        diagnostic_domain = "execution",
        error = ?error,
        "Julia model generation rejected a validated task"
    );
    JuliaWorkerError::new(
        JuliaWorkerErrorCode::ModelGenerationFailed,
        "Julia model generation failed.",
    )
}

fn task_generation_error() -> JuliaWorkerError {
    JuliaWorkerError::new(
        JuliaWorkerErrorCode::TaskGenerationFailed,
        "Julia task generation failed.",
    )
}

pub(super) fn run_manager_task(
    worker: &JuliaWorkerManager,
    app_data_dir: &Path,
    worker_task_id: &str,
    task: &PreparedJuliaTask,
) -> Result<JuliaTaskCompletion, JuliaWorkerError> {
    let mut output = worker.run_task_with_typed_input(
        app_data_dir,
        JuliaWorkerTask {
            task_id: Some(worker_task_id.to_owned()),
            operation: "bayes_fit".to_owned(),
            parameters: serde_json::Value::Object(serde_json::Map::new()),
        },
        |input_path| write_task_files(input_path, worker_task_id, task),
        None,
    )?;
    let task_directory = output.take_task_directory().ok_or_else(|| {
        JuliaWorkerError::new(
            JuliaWorkerErrorCode::TaskDirectoryInvalid,
            "Julia task directory ownership is unavailable.",
        )
    })?;
    Ok(JuliaTaskCompletion {
        worker_task_id: output.task_id.into(),
        metadata_path: output.metadata_path,
        task_directory,
    })
}

fn write_task_files(
    input_path: &Path,
    worker_task_id: &str,
    task: &PreparedJuliaTask,
) -> Result<(), JuliaWorkerError> {
    write_input_table(input_path, &task.inputs)?;
    let task_dir = input_path.parent().ok_or_else(task_generation_error)?;
    let model_spec_path = task_dir.join("model_spec.json");
    let inference_config_path = task_dir.join("inference_config.json");
    let predictor_path = task_dir.join("predictor_kernel.jl");
    let likelihood_path = task_dir.join("likelihood_kernel.jl");
    let manifest_path = task_dir.join("exchange_manifest.json");
    let output_path = task_dir.join("output.arrow");
    let metadata_path = task_dir.join("metadata.json");

    write_json(&model_spec_path, &task.model)?;
    write_json(&inference_config_path, &task.sampler)?;
    write_bytes(&predictor_path, task.generated.predictor.as_bytes())?;
    write_bytes(&likelihood_path, task.generated.likelihood.as_bytes())?;
    let manifest = JuliaExchangeManifest {
        task_id: worker_task_id,
        input_table_path: input_path,
        model_spec_path: &model_spec_path,
        inference_config_path: &inference_config_path,
        predictor_kernel_path: &predictor_path,
        likelihood_kernel_path: &likelihood_path,
        predictor_columns: &task.generated.columns,
        output_path: &output_path,
        metadata_path: &metadata_path,
        row_count: task.inputs.first().map_or(0, |input| input.values().len()),
        columns: task
            .inputs
            .iter()
            .map(|input| JuliaExchangeColumn {
                name: input.name(),
                categorical_role: match input.categorical_role() {
                    Some(CategoricalRole::General) => Some(JuliaCategoricalRole::General),
                    Some(CategoricalRole::Individual) => Some(JuliaCategoricalRole::Individual),
                    Some(CategoricalRole::Time) => Some(JuliaCategoricalRole::Time),
                    None => None,
                },
            })
            .collect(),
    };
    write_json(&manifest_path, &manifest)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JuliaExchangeManifest<'a> {
    task_id: &'a str,
    input_table_path: &'a Path,
    model_spec_path: &'a Path,
    inference_config_path: &'a Path,
    predictor_kernel_path: &'a Path,
    likelihood_kernel_path: &'a Path,
    predictor_columns: &'a [String],
    output_path: &'a Path,
    metadata_path: &'a Path,
    row_count: usize,
    columns: Vec<JuliaExchangeColumn<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JuliaExchangeColumn<'a> {
    name: &'a str,
    categorical_role: Option<JuliaCategoricalRole>,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum JuliaCategoricalRole {
    General,
    Individual,
    Time,
}

fn write_input_table(
    input_path: &Path,
    inputs: &[StatisticalInput],
) -> Result<(), JuliaWorkerError> {
    let height = inputs.first().map_or(0, |input| input.values().len());
    let columns = inputs
        .iter()
        .map(input_column)
        .collect::<Result<Vec<_>, _>>()?;
    let mut dataframe = DataFrame::new(height, columns).map_err(|_| task_generation_error())?;
    let mut file = File::create(input_path).map_err(|_| task_generation_error())?;
    IpcWriter::new(&mut file)
        .finish(&mut dataframe)
        .map_err(|_| task_generation_error())?;
    Ok(())
}

fn input_column(input: &StatisticalInput) -> Result<Column, JuliaWorkerError> {
    let numeric = input
        .values()
        .iter()
        .all(|value| matches!(value, None | Some(StatisticalScalar::Numeric(_))));
    let categorical = input
        .values()
        .iter()
        .all(|value| matches!(value, None | Some(StatisticalScalar::Category(_))));
    if numeric {
        let values = input
            .values()
            .iter()
            .map(|value| match value {
                Some(StatisticalScalar::Numeric(value)) => Ok(Some(*value)),
                None => Ok(None),
                Some(StatisticalScalar::Category(_)) => Err(task_generation_error()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(Column::new(input.name().into(), values));
    }
    if categorical {
        let values = input
            .values()
            .iter()
            .map(|value| match value {
                Some(StatisticalScalar::Category(value)) => Ok(Some(value.as_ref())),
                None => Ok(None),
                Some(StatisticalScalar::Numeric(_)) => Err(task_generation_error()),
            })
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(Column::new(input.name().into(), values));
    }
    Err(task_generation_error())
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<(), JuliaWorkerError> {
    let file = File::create(path).map_err(|_| task_generation_error())?;
    serde_json::to_writer_pretty(file, value).map_err(|_| task_generation_error())
}

fn write_bytes(path: &Path, value: &[u8]) -> Result<(), JuliaWorkerError> {
    fs::write(path, value).map_err(|_| task_generation_error())
}

pub(super) struct CompletedJuliaTask {
    pub(super) result: BayesTaskResult,
    pub(super) artifacts: BTreeMap<BayesArtifactHandle, OwnedArtifact>,
    pub(super) task_directory: JuliaWorkerTaskDirectory,
}

pub(super) struct OwnedArtifact {
    pub(super) path: PathBuf,
    pub(super) media_type: BayesArtifactMediaType,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JuliaInferenceMetadata {
    summaries: Vec<ParameterSummary>,
    diagnostics: InferenceDiagnostics,
    artifact_manifest: JuliaArtifactManifest,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct JuliaArtifactManifest {
    task_id: String,
    artifacts: Vec<JuliaArtifactRecord>,
}

#[derive(Deserialize)]
struct JuliaArtifactRecord {
    path: PathBuf,
}

pub(super) fn finish_task(
    handle: &BayesTaskHandle,
    expected_worker_task_id: &str,
    completion: JuliaTaskCompletion,
) -> Result<CompletedJuliaTask, BayesWorkerError> {
    if completion.worker_task_id.as_ref() != expected_worker_task_id {
        return Err(worker_terminal(handle));
    }
    let bytes = fs::read(&completion.metadata_path).map_err(|_| worker_terminal(handle))?;
    let metadata: JuliaInferenceMetadata =
        serde_json::from_slice(&bytes).map_err(|_| worker_terminal(handle))?;
    if metadata.artifact_manifest.task_id != completion.worker_task_id.as_ref() {
        return Err(worker_terminal(handle));
    }

    let mut artifacts = BTreeMap::new();
    let mut handles = Vec::new();
    for record in metadata.artifact_manifest.artifacts {
        let name = record
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| worker_terminal(handle))?;
        let artifact_id = ArtifactId::try_from(name).map_err(|_| worker_terminal(handle))?;
        let artifact = BayesArtifactHandle::mint_for_worker(handle.clone(), artifact_id);
        let path = completion
            .task_directory
            .claim_artifact(&record.path)
            .map_err(|_| BayesWorkerError::ArtifactNotOwned {
                artifact: artifact.clone(),
            })?;
        let media_type = artifact_media_type(&artifact)?;
        if artifacts
            .insert(artifact.clone(), OwnedArtifact { path, media_type })
            .is_some()
        {
            return Err(BayesWorkerError::ArtifactNotOwned { artifact });
        }
        handles.push(artifact);
    }
    let inference = BayesInferenceSnapshot::from_worker(
        handle.clone(),
        Arc::from(metadata.summaries),
        metadata.diagnostics,
    );
    let result = BayesTaskResult::validated_worker_result(handle, inference, Arc::from(handles))?;
    Ok(CompletedJuliaTask {
        result,
        artifacts,
        task_directory: completion.task_directory,
    })
}

fn artifact_media_type(
    artifact: &BayesArtifactHandle,
) -> Result<BayesArtifactMediaType, BayesWorkerError> {
    let extension = Path::new(artifact.artifact_id().as_str())
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("json") => Ok(BayesArtifactMediaType::Json),
        Some("csv") => Ok(BayesArtifactMediaType::Csv),
        Some("png") => Ok(BayesArtifactMediaType::Png),
        Some("arrow" | "ipc" | "bin") => Ok(BayesArtifactMediaType::Binary),
        None | Some(_) => Err(BayesWorkerError::ArtifactFormatUnsupported {
            artifact: artifact.clone(),
        }),
    }
}

fn worker_terminal(handle: &BayesTaskHandle) -> BayesWorkerError {
    BayesWorkerError::WorkerTerminal {
        task: handle.clone(),
        terminal: crate::sci::api::bayes::worker::BayesWorkerTerminalCode::Failed,
    }
}

pub(super) fn map_worker_error(
    handle: &BayesTaskHandle,
    error: JuliaWorkerError,
) -> BayesWorkerError {
    if error.code() == JuliaWorkerErrorCode::Cancelled {
        BayesWorkerError::Cancelled {
            task: handle.clone(),
        }
    } else {
        tracing::warn!(
            target: "yssbi::julia::bayes_worker_adapter",
            diagnostic_domain = "execution",
            error_code = error.code().as_str(),
            "Julia Bayes worker task failed"
        );
        worker_terminal(handle)
    }
}
