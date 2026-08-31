use std::num::NonZeroU64;
use std::sync::Arc;

use yss_bayes_model::{BayesModelSpec, model_spec_is_valid};
use yss_bayes_result::{InferenceDiagnostics, ParameterSummary};
use yss_sci_contract::{CancelDeliveryControl, ExecutionControl, StatisticalInput};

const MAX_OPAQUE_ID_LEN: usize = 128;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BayesTaskId(Box<str>);

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BayesTaskIdValidationError {
    #[error("Bayes task ID is empty")]
    Empty,
    #[error("Bayes task ID exceeds its length limit")]
    TooLong { max: usize },
    #[error("Bayes task ID contains an invalid character")]
    InvalidCharacter { index: usize },
    #[error("Bayes task ID contains a reserved sequence")]
    ReservedSequence { index: usize },
}

impl TryFrom<&str> for BayesTaskId {
    type Error = BayesTaskIdValidationError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        validate_opaque_id(value)
            .map(Self)
            .map_err(|error| match error {
                OpaqueIdValidationError::Empty => BayesTaskIdValidationError::Empty,
                OpaqueIdValidationError::TooLong { max } => {
                    BayesTaskIdValidationError::TooLong { max }
                }
                OpaqueIdValidationError::InvalidCharacter { index } => {
                    BayesTaskIdValidationError::InvalidCharacter { index }
                }
                OpaqueIdValidationError::ReservedSequence { index } => {
                    BayesTaskIdValidationError::ReservedSequence { index }
                }
            })
    }
}

impl BayesTaskId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactId(Box<str>);

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ArtifactIdValidationError {
    #[error("Bayes artifact ID is empty")]
    Empty,
    #[error("Bayes artifact ID exceeds its length limit")]
    TooLong { max: usize },
    #[error("Bayes artifact ID contains an invalid character")]
    InvalidCharacter { index: usize },
    #[error("Bayes artifact ID contains a reserved sequence")]
    ReservedSequence { index: usize },
}

impl TryFrom<&str> for ArtifactId {
    type Error = ArtifactIdValidationError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        validate_opaque_id(value)
            .map(Self)
            .map_err(|error| match error {
                OpaqueIdValidationError::Empty => ArtifactIdValidationError::Empty,
                OpaqueIdValidationError::TooLong { max } => {
                    ArtifactIdValidationError::TooLong { max }
                }
                OpaqueIdValidationError::InvalidCharacter { index } => {
                    ArtifactIdValidationError::InvalidCharacter { index }
                }
                OpaqueIdValidationError::ReservedSequence { index } => {
                    ArtifactIdValidationError::ReservedSequence { index }
                }
            })
    }
}

impl ArtifactId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy)]
enum OpaqueIdValidationError {
    Empty,
    TooLong { max: usize },
    InvalidCharacter { index: usize },
    ReservedSequence { index: usize },
}

fn validate_opaque_id(value: &str) -> Result<Box<str>, OpaqueIdValidationError> {
    if value.is_empty() {
        return Err(OpaqueIdValidationError::Empty);
    }
    if value.len() > MAX_OPAQUE_ID_LEN {
        return Err(OpaqueIdValidationError::TooLong {
            max: MAX_OPAQUE_ID_LEN,
        });
    }
    if let Some(index) = value.as_bytes().windows(2).position(|pair| pair == b"..") {
        return Err(OpaqueIdValidationError::ReservedSequence { index });
    }
    if let Some((index, _)) = value
        .bytes()
        .enumerate()
        .find(|(_, byte)| !byte.is_ascii_alphanumeric() && !matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(OpaqueIdValidationError::InvalidCharacter { index });
    }
    Ok(value.into())
}

pub struct ValidatedBayesTask {
    task_id: BayesTaskId,
    model: BayesModelSpec,
    inputs: Arc<[StatisticalInput]>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BayesTaskValidationError {
    #[error("Bayes worker task ID is invalid")]
    InvalidTaskId,
    #[error("Bayes worker model is invalid")]
    InvalidModel,
    #[error("Bayes worker input is invalid")]
    InvalidInput { index: usize },
}

pub(crate) fn validate_bayes_task(
    task_id: &str,
    model: BayesModelSpec,
    inputs: Arc<[StatisticalInput]>,
) -> Result<ValidatedBayesTask, BayesTaskValidationError> {
    let task_id =
        BayesTaskId::try_from(task_id).map_err(|_| BayesTaskValidationError::InvalidTaskId)?;
    ValidatedBayesTask::try_new(task_id, model, inputs)
}

impl ValidatedBayesTask {
    pub fn try_new(
        task_id: BayesTaskId,
        model: BayesModelSpec,
        inputs: Arc<[StatisticalInput]>,
    ) -> Result<Self, BayesTaskValidationError> {
        if !model_spec_is_valid(&model) {
            return Err(BayesTaskValidationError::InvalidModel);
        }
        validate_inputs(&model, &inputs)
            .map_err(|index| BayesTaskValidationError::InvalidInput { index })?;
        Ok(Self {
            task_id,
            model,
            inputs,
        })
    }

    pub fn task_id(&self) -> &BayesTaskId {
        &self.task_id
    }

    pub fn model(&self) -> &BayesModelSpec {
        &self.model
    }

    pub fn inputs(&self) -> &[StatisticalInput] {
        &self.inputs
    }
}

mod validation;

use validation::validate_inputs;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BayesTaskGeneration(u64);

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BayesTaskHandle {
    task_id: BayesTaskId,
    generation: BayesTaskGeneration,
}

impl BayesTaskHandle {
    #[allow(
        dead_code,
        reason = "worker authority stays production-unreachable until the final adapter is staged"
    )]
    pub(crate) fn issue_for_worker(task_id: BayesTaskId, generation: NonZeroU64) -> Self {
        Self {
            task_id,
            generation: BayesTaskGeneration(generation.get()),
        }
    }

    pub fn task_id(&self) -> &BayesTaskId {
        &self.task_id
    }

    pub fn generation(&self) -> BayesTaskGeneration {
        self.generation
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BayesArtifactHandle {
    task: BayesTaskHandle,
    artifact: ArtifactId,
}

impl BayesArtifactHandle {
    #[allow(
        dead_code,
        reason = "worker authority stays production-unreachable until the final adapter is staged"
    )]
    pub(crate) fn mint_for_worker(task: BayesTaskHandle, artifact: ArtifactId) -> Self {
        Self { task, artifact }
    }

    pub fn task(&self) -> &BayesTaskHandle {
        &self.task
    }

    pub fn artifact_id(&self) -> &ArtifactId {
        &self.artifact
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BayesArtifactMediaType {
    Json,
    Csv,
    ArrowIpc,
}

pub struct BayesArtifact {
    handle: BayesArtifactHandle,
    media_type: BayesArtifactMediaType,
    bytes: Arc<[u8]>,
}

impl BayesArtifact {
    #[allow(
        dead_code,
        reason = "worker authority stays production-unreachable until the final adapter is staged"
    )]
    pub(crate) fn from_worker(
        handle: BayesArtifactHandle,
        media_type: BayesArtifactMediaType,
        bytes: Arc<[u8]>,
    ) -> Self {
        Self {
            handle,
            media_type,
            bytes,
        }
    }

    pub fn handle(&self) -> &BayesArtifactHandle {
        &self.handle
    }

    pub fn media_type(&self) -> BayesArtifactMediaType {
        self.media_type
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BayesWorkerTerminalCode {
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BayesWorkerPhase {
    Start,
    AwaitResult,
    CancelDelivery,
    ReadArtifact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BayesCancelTerminal {
    Cancelled,
    AlreadyTerminal { terminal: BayesWorkerTerminalCode },
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BayesWorkerError {
    #[error("Bayes worker admission is closed")]
    AdmissionClosed { task: BayesTaskId },
    #[error("Bayes worker acceptance deadline was reached")]
    AcceptanceDeadline { task: BayesTaskId },
    #[error("Bayes worker cancel delivery deadline was reached")]
    CancelDeliveryDeadline { task: BayesTaskHandle },
    #[error("Bayes worker artifact read deadline was reached")]
    ArtifactReadDeadline { artifact: BayesArtifactHandle },
    #[error("Bayes worker task handle is stale")]
    StaleTaskHandle { task: BayesTaskHandle },
    #[error("Bayes worker artifact is not owned by the task")]
    ArtifactNotOwned { artifact: BayesArtifactHandle },
    #[error("Bayes worker artifact format is unsupported")]
    ArtifactFormatUnsupported { artifact: BayesArtifactHandle },
    #[error("Bayes worker task was cancelled")]
    Cancelled { task: BayesTaskHandle },
    #[error("Bayes worker task reached a terminal state")]
    WorkerTerminal {
        task: BayesTaskHandle,
        terminal: BayesWorkerTerminalCode,
    },
    #[error("Bayes worker is unavailable")]
    WorkerUnavailable { phase: BayesWorkerPhase },
}

pub trait BayesWorkerPort: Send + Sync {
    fn start(
        &self,
        task: ValidatedBayesTask,
        control: &ExecutionControl,
    ) -> Result<BayesTaskHandle, BayesWorkerError>;

    fn await_result(
        &self,
        handle: &BayesTaskHandle,
        control: &ExecutionControl,
    ) -> Result<BayesTaskResult, BayesWorkerError>;

    fn cancel(
        &self,
        handle: &BayesTaskHandle,
        control: &CancelDeliveryControl,
    ) -> Result<BayesCancelTerminal, BayesWorkerError>;

    fn read_artifact(
        &self,
        artifact: &BayesArtifactHandle,
        control: &ExecutionControl,
    ) -> Result<BayesArtifact, BayesWorkerError>;
}

pub struct BayesTaskResult {
    inference: BayesInferenceSnapshot,
    artifacts: Arc<[BayesArtifactHandle]>,
}

pub struct BayesInferenceSnapshot {
    task: BayesTaskHandle,
    summaries: Arc<[ParameterSummary]>,
    diagnostics: InferenceDiagnostics,
}

impl BayesInferenceSnapshot {
    #[allow(
        dead_code,
        reason = "worker result authority stays unreachable until the final adapter is staged"
    )]
    pub(crate) fn from_worker(
        task: BayesTaskHandle,
        summaries: Arc<[ParameterSummary]>,
        diagnostics: InferenceDiagnostics,
    ) -> Self {
        Self {
            task,
            summaries,
            diagnostics,
        }
    }

    pub fn task(&self) -> &BayesTaskHandle {
        &self.task
    }

    pub fn summaries(&self) -> &[ParameterSummary] {
        &self.summaries
    }

    pub fn diagnostics(&self) -> &InferenceDiagnostics {
        &self.diagnostics
    }
}

impl BayesTaskResult {
    #[allow(
        dead_code,
        reason = "worker authority stays production-unreachable until the final adapter is staged"
    )]
    pub(crate) fn validated_worker_result(
        awaited: &BayesTaskHandle,
        inference: BayesInferenceSnapshot,
        artifacts: Arc<[BayesArtifactHandle]>,
    ) -> Result<Self, BayesWorkerError> {
        if inference.task() != awaited {
            return Err(BayesWorkerError::StaleTaskHandle {
                task: inference.task().clone(),
            });
        }
        if let Some(artifact) = artifacts.iter().find(|artifact| artifact.task() != awaited) {
            return Err(BayesWorkerError::ArtifactNotOwned {
                artifact: artifact.clone(),
            });
        }
        Ok(Self {
            inference,
            artifacts,
        })
    }

    pub fn task(&self) -> &BayesTaskHandle {
        self.inference.task()
    }

    pub fn inference(&self) -> &BayesInferenceSnapshot {
        &self.inference
    }

    pub fn artifacts(&self) -> &[BayesArtifactHandle] {
        &self.artifacts
    }
}

#[cfg(test)]
mod tests;
