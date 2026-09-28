//! Shared cancellation, deadlines and synchronous computation failures.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

#[derive(Clone, Default)]
pub struct ScientificCancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl ScientificCancellationToken {
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

/// Cooperative control for a synchronous scientific computation.
///
/// Callers own scheduling and budgets. ACF/PACF samples this control during input
/// scans and numerical loops, and before returning. Linear regression checks at
/// stage boundaries; a running matrix decomposition is not interruptible.
#[derive(Clone)]
pub struct ScientificExecutionControl {
    pub cancellation: ScientificCancellationToken,
    pub deadline: Instant,
}

impl ScientificExecutionControl {
    pub fn check(&self) -> Result<(), ScientificComputationError> {
        if self.cancellation.is_cancelled() {
            return Err(ScientificComputationError::Cancelled);
        }
        if self.deadline <= Instant::now() {
            return Err(ScientificComputationError::DeadlineExceeded);
        }
        Ok(())
    }

    pub fn from_shared(cancellation: Arc<AtomicBool>, deadline: Instant) -> Self {
        Self {
            cancellation: ScientificCancellationToken::from_shared(cancellation),
            deadline,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScientificInputViolation {
    EmptyInput,
    NonFiniteInput,
    ShapeMismatch,
    ParameterOutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ScientificComputationError {
    #[error("scientific input is invalid")]
    InvalidInput { violation: ScientificInputViolation },
    #[error("scientific execution was cancelled")]
    Cancelled,
    #[error("scientific execution deadline was exceeded")]
    DeadlineExceeded,
    #[error("scientific computation failed")]
    ComputationFailed,
}

#[cfg(test)]
mod tests;
