//! The typed Application capability port called by the Assistant extension.

use crate::{
    AgentFuture, ApplyGraphEditRequest, AutomationCapabilityRequest, AutomationCapabilityResult,
    CapabilityControl, CapabilityFailure, CapabilityInvocationContext, GraphEditReceipt,
};
use std::future::Future;
use std::pin::Pin;

pub type CapabilityFuture<'a> = Pin<
    Box<dyn Future<Output = Result<AutomationCapabilityResult, CapabilityFailure>> + Send + 'a>,
>;

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
