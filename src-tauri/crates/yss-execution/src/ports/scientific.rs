use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

#[derive(Clone, Default)]
pub struct BackendCancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl BackendCancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub(crate) fn from_shared(cancelled: Arc<AtomicBool>) -> Self {
        Self { cancelled }
    }
}

/// Admission control for a synchronous scientific backend call.
///
/// Implementations sample cancellation and the deadline before dispatch. This
/// contract does not claim cooperative interruption after computation starts.
pub struct BackendExecutionControl {
    pub cancellation: BackendCancellationToken,
    pub deadline: Instant,
}

impl BackendExecutionControl {
    pub fn from_shared(cancellation: Arc<AtomicBool>, deadline: Instant) -> Self {
        Self {
            cancellation: BackendCancellationToken::from_shared(cancellation),
            deadline,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AcfPacfRequest {
    pub values: Vec<f64>,
    pub max_lag: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AcfPacfResult {
    pub acf: Vec<f64>,
    pub pacf: Vec<f64>,
    pub n: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScientificInputViolation {
    EmptyInput,
    NonFiniteInput,
    ShapeMismatch,
    ParameterOutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ScientificBackendError {
    #[error("scientific input is invalid")]
    InvalidInput { violation: ScientificInputViolation },
    #[error("scientific execution was cancelled")]
    Cancelled,
    #[error("scientific execution deadline was exceeded")]
    DeadlineExceeded,
    #[error("scientific backend is unavailable")]
    Unavailable,
    #[error("scientific computation failed")]
    ComputationFailed,
}

/// Execution's scientific backend boundary.
///
/// Every method applies [`BackendExecutionControl`] at admission. A concrete
/// backend may add real cooperative checkpoints, but callers cannot infer them
/// from this synchronous port.
pub trait ScientificBackend: Send + Sync {
    fn acf_pacf(
        &self,
        request: AcfPacfRequest,
        control: &BackendExecutionControl,
    ) -> Result<AcfPacfResult, ScientificBackendError>;
}

#[cfg(test)]
mod tests;
