//! Cooperative cancellation and deadline control for one tool invocation.

use crate::{CancellationReason, CancellationToken, CapabilityFailure, CapabilityFailureCode};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// One invocation's cooperative execution budget. It is never persisted or sent over IPC.
#[derive(Clone, Debug)]
pub struct CapabilityControl {
    cancellation: CancellationToken,
    query_cancelled: Arc<AtomicBool>,
    deadline: Instant,
}

impl CapabilityControl {
    pub fn new(cancellation: CancellationToken, timeout: Duration) -> Self {
        Self {
            cancellation,
            query_cancelled: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + timeout,
        }
    }

    pub fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }
    pub fn cancellation_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.query_cancelled)
    }
    pub fn deadline(&self) -> Instant {
        self.deadline
    }

    pub fn cancel_query(&self) {
        self.query_cancelled.store(true, Ordering::Release);
    }

    pub fn check(&self) -> Result<(), CapabilityFailure> {
        let code = if self.cancellation.reason() == Some(CancellationReason::DeadlineElapsed)
            || Instant::now() >= self.deadline
        {
            Some(CapabilityFailureCode::DEADLINE_ELAPSED)
        } else if self.cancellation.is_cancelled() || self.query_cancelled.load(Ordering::Acquire) {
            Some(CapabilityFailureCode::CANCELLED)
        } else {
            None
        };
        code.map_or(Ok(()), |code| Err(CapabilityFailure::new(code)))
    }
}
