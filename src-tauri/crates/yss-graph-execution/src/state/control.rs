//! Per-run cancellation, deadline and kernel admission budget.
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
    pub(super) max_input_bytes: usize,
}

impl RunExecutionControl {
    #[cfg(test)]
    pub(super) fn new(deadline: Instant) -> Self {
        Self {
            cancellation: Arc::new(AtomicBool::new(false)),
            deadline,
            max_input_bytes: yss_node_kernel::DEFAULT_MAX_INPUT_BYTES,
        }
    }

    pub fn with_cancellation(cancellation: Arc<AtomicBool>, deadline: Instant) -> Self {
        Self {
            cancellation,
            deadline,
            max_input_bytes: yss_node_kernel::DEFAULT_MAX_INPUT_BYTES,
        }
    }

    /// Per-node input, workspace and result admission budget for this execution.
    pub fn with_memory_budget(mut self, bytes: usize) -> Self {
        self.max_input_bytes = bytes;
        self
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
