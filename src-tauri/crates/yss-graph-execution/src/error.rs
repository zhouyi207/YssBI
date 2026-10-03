use crate::identity::RuntimeGeneration;
use crate::resource_preparation::ResourcePreparationError;
use crate::run_registry::RunRegistryError;
use std::time::SystemTimeError;
use thiserror::Error;
use yss_node_kernel::KernelError;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RunPhase {
    Admission,
    PlanValidation,
    ResourcePreparation,
    Execution,
    Finalization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunFailureCode {
    KernelFailed,
    KernelNotFound,
    InvalidNumericInput,
    ShapeMismatch,
    InvalidParameter,
    UnalignedSeries,
    BudgetExceeded,
    InputLayoutMismatch,
    OutputContractMismatch,
    ScientificFailure,
    DivisionByZero,
    NonFiniteResult,
    DeadlineExceeded,
    ResourceUnavailable,
    FinalizationFailed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunFailure {
    pub code: RunFailureCode,
    pub phase: RunPhase,
    pub source: Option<crate::plan::PlanSourceIdentity>,
}

#[derive(Debug, Error)]
pub enum ExecutePreparedError {
    #[error("prepared execution belongs to another kernel capability version")]
    KernelCapabilitiesChanged,
    #[error("prepared execution belongs to another runtime generation")]
    RuntimeGenerationMismatch {
        expected: RuntimeGeneration,
        actual: RuntimeGeneration,
    },
    #[error("execution admission failed")]
    Admission(#[source] ExecutionAdmissionError),
    #[error("execution resource preparation failed")]
    ResourcePreparation(#[source] ResourcePreparationError),
    #[error("execution run lifecycle failed")]
    RunRegistry(#[source] RunRegistryError),
    #[error("execution was cancelled")]
    Cancelled { phase: RunPhase },
    #[error("execution deadline was exceeded")]
    DeadlineExceeded { phase: RunPhase },
    #[error("prepared execution kernel failed")]
    Kernel(#[source] OperationExecutionError),
    #[error("execution result identity space is exhausted")]
    ResultIdentityExhausted,
    #[error("execution result timestamp is unavailable")]
    ResultTimestamp(#[source] SystemTimeError),
}

#[derive(Debug, Error)]
pub enum OperationExecutionError {
    #[error(transparent)]
    Kernel(#[from] KernelError),
    #[error("prepared graph execution contains inconsistent values")]
    Failed,
    #[error("requested graph output is unavailable in the prepared plan")]
    DemandOutputUnavailable,
    #[error("node execution failed")]
    AtNode {
        source: crate::plan::PlanSourceIdentity,
        #[source]
        error: Box<OperationExecutionError>,
    },
}

impl OperationExecutionError {
    pub(crate) fn at_node(self, source: &crate::plan::PlanSourceIdentity) -> Self {
        match self {
            Self::Kernel(KernelError::Cancelled | KernelError::DeadlineExceeded)
            | Self::AtNode { .. } => self,
            error => Self::AtNode {
                source: source.clone(),
                error: Box::new(error),
            },
        }
    }

    fn failure(&self) -> RunFailure {
        let code = match self {
            Self::AtNode { source, error } => {
                return RunFailure {
                    source: Some(source.clone()),
                    ..error.failure()
                };
            }
            Self::Kernel(KernelError::ShapeMismatch) => RunFailureCode::ShapeMismatch,
            Self::Kernel(KernelError::InvalidParameter) => RunFailureCode::InvalidParameter,
            Self::Kernel(KernelError::UnalignedSeries) => RunFailureCode::UnalignedSeries,
            Self::Kernel(KernelError::BudgetExceeded) => RunFailureCode::BudgetExceeded,
            Self::Kernel(KernelError::InputLayoutMismatch) => RunFailureCode::InputLayoutMismatch,
            Self::Kernel(KernelError::OutputContractMismatch) => {
                RunFailureCode::OutputContractMismatch
            }
            Self::Kernel(KernelError::ScientificFailure) => RunFailureCode::ScientificFailure,
            Self::Kernel(KernelError::DivisionByZero) => RunFailureCode::DivisionByZero,
            Self::Kernel(KernelError::NonFiniteResult) => RunFailureCode::NonFiniteResult,
            Self::Kernel(KernelError::InvalidNumericInput) => RunFailureCode::InvalidNumericInput,
            Self::Kernel(KernelError::KernelNotFound) => RunFailureCode::KernelNotFound,
            Self::Kernel(KernelError::DeadlineExceeded) => RunFailureCode::DeadlineExceeded,
            _ => RunFailureCode::KernelFailed,
        };
        RunFailure {
            code,
            phase: RunPhase::Execution,
            source: None,
        }
    }
}

impl ExecutePreparedError {
    pub fn failure(&self) -> RunFailure {
        let (code, phase) = match self {
            Self::Kernel(error) => return error.failure(),
            Self::DeadlineExceeded { phase } => (RunFailureCode::DeadlineExceeded, *phase),
            Self::ResourcePreparation(_) => (
                RunFailureCode::ResourceUnavailable,
                RunPhase::ResourcePreparation,
            ),
            Self::ResultIdentityExhausted | Self::ResultTimestamp(_) => {
                (RunFailureCode::FinalizationFailed, RunPhase::Finalization)
            }
            _ => (RunFailureCode::KernelFailed, RunPhase::Execution),
        };
        RunFailure {
            code,
            phase,
            source: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Error)]
pub enum ExecutionAdmissionError {
    #[error("execution session admission is closed")]
    Closed,
}
