//! Fixture gateways for tests of the Assistant extension and its Application adapters.

use crate::contract::{
    AutomationCapabilityRequest, AutomationCapabilityResult, CapabilityControl, CapabilityFailure,
    CapabilityFailureCode, CapabilityFuture, CapabilityGatewayPort, CapabilityInvocationContext,
};

#[derive(Default)]
pub struct RejectingCapabilityGateway;

impl CapabilityGatewayPort for RejectingCapabilityGateway {
    fn invoke<'a>(
        &'a self,
        _context: CapabilityInvocationContext,
        _request: AutomationCapabilityRequest,
        _control: CapabilityControl,
    ) -> CapabilityFuture<'a> {
        Box::pin(async {
            Err(CapabilityFailure::new(
                CapabilityFailureCode::INTERNAL_FAILURE,
            ))
        })
    }
}

/// Answers each request with the most recently added result that matches its capability.
pub struct StaticCapabilityGateway {
    results: Vec<AutomationCapabilityResult>,
}

impl StaticCapabilityGateway {
    pub fn new(result: AutomationCapabilityResult) -> Self {
        Self {
            results: vec![result],
        }
    }
}

impl CapabilityGatewayPort for StaticCapabilityGateway {
    fn invoke<'a>(
        &'a self,
        _context: CapabilityInvocationContext,
        request: AutomationCapabilityRequest,
        _control: CapabilityControl,
    ) -> CapabilityFuture<'a> {
        Box::pin(async move {
            self.results
                .iter()
                .rev()
                .find(|result| result.accepts_capability(request.capability_id()))
                .cloned()
                .ok_or_else(|| CapabilityFailure::new(CapabilityFailureCode::INTERNAL_FAILURE))
        })
    }
}
