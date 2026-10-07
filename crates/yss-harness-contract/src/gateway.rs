//! Capability invocation port and cooperative execution control.

use crate::{
    AgentFuture, ApplyGraphEditRequest, AutomationCapabilityRequest, AutomationCapabilityResult,
    CancellationReason, CancellationToken, CapabilityFailure, CapabilityFailureCode,
    CapabilityInvocationContext, GraphEditReceipt,
};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub type CapabilityFuture<'a> = Pin<
    Box<dyn Future<Output = Result<AutomationCapabilityResult, CapabilityFailure>> + Send + 'a>,
>;

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
            Some(CapabilityFailureCode::DeadlineElapsed)
        } else if self.cancellation.is_cancelled() || self.query_cancelled.load(Ordering::Acquire) {
            Some(CapabilityFailureCode::Cancelled)
        } else {
            None
        };
        code.map_or(Ok(()), |code| Err(CapabilityFailure::new(code)))
    }
}

pub trait CapabilityGatewayPort: Send + Sync {
    fn invoke<'a>(
        &'a self,
        context: CapabilityInvocationContext,
        request: AutomationCapabilityRequest,
        control: CapabilityControl,
    ) -> CapabilityFuture<'a>;

    /// Query an already committed batch without invoking a mutation. Read-only gateways
    /// have no graph receipts; a gateway that applies graph edits supplies its owner lookup.
    fn recover_graph_edit<'a>(
        &'a self,
        _context: CapabilityInvocationContext,
        _request: ApplyGraphEditRequest,
    ) -> AgentFuture<'a, Result<Option<GraphEditReceipt>, CapabilityFailure>> {
        Box::pin(async { Ok(None) })
    }
}
