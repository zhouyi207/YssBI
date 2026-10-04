//! Per-run cancellation and deadline.
use crate::error::{ExecutePreparedError, RunPhase};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

#[derive(Clone)]
pub struct RunExecutionControl {
    pub(super) cancellation: Arc<AtomicBool>,
    pub(super) deadline: Instant,
}

impl RunExecutionControl {
    #[cfg(test)]
    pub(super) fn new(deadline: Instant) -> Self {
        Self {
            cancellation: Arc::new(AtomicBool::new(false)),
            deadline,
        }
    }

    pub fn with_cancellation(cancellation: Arc<AtomicBool>, deadline: Instant) -> Self {
        Self {
            cancellation,
            deadline,
        }
    }

    pub(super) fn check(&self, phase: RunPhase) -> Result<(), ExecutePreparedError> {
        if self.cancellation.load(Ordering::Acquire) {
            return Err(ExecutePreparedError::Cancelled { phase });
        }
        if Instant::now() >= self.deadline {
            return Err(ExecutePreparedError::DeadlineExceeded { phase });
        }
        Ok(())
    }
}
