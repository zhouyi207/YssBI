//! Observe model-facing business calls, excluding Core's internal baseline reads.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use yss_harness_contract::*;

pub type CallLog = Arc<Mutex<Vec<CallMeasurement>>>;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallMeasurement {
    pub role: AgentRole,
    pub capability_id: CapabilityId,
    pub invocation_id: Option<ToolInvocationId>,
    pub argument_hash: Option<String>,
    pub result_hash: Option<String>,
    pub result_bytes: Option<usize>,
    pub result_characters: Option<usize>,
    pub elapsed_ms: u128,
    pub failure_code: Option<CapabilityFailureCode>,
}

pub struct ObservedExecutor {
    pub inner: Arc<dyn ModelCapabilityExecutor>,
    pub role: AgentRole,
    pub calls: CallLog,
}

impl ObservedExecutor {
    fn record(
        &self,
        capability_id: CapabilityId,
        argument_hash: Option<String>,
        started: Instant,
        outcome: &Result<ModelCapabilityOutcome, CapabilityFailure>,
    ) {
        let projection = match outcome {
            Ok(value) => model::capability_result(&value.result),
            Err(failure) => Ok(model::failure(failure)),
        };
        let encoded = projection
            .as_ref()
            .ok()
            .and_then(|value| serde_json::to_string(value).ok());
        self.calls
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .push(CallMeasurement {
                role: self.role,
                capability_id,
                invocation_id: outcome
                    .as_ref()
                    .ok()
                    .map(|value| value.invocation_id.clone()),
                argument_hash,
                result_hash: projection.as_ref().ok().and_then(fingerprint),
                result_bytes: encoded.as_ref().map(String::len),
                result_characters: encoded.as_ref().map(|value| value.chars().count()),
                elapsed_ms: started.elapsed().as_millis(),
                failure_code: outcome.as_ref().err().map(|failure| failure.code),
            });
    }
}

impl ModelCapabilityExecutor for ObservedExecutor {
    fn begin_control<'a>(
        &'a self,
        tool: AgentControlTool,
    ) -> AgentFuture<'a, Result<ToolInvocationId, CapabilityFailure>> {
        self.inner.begin_control(tool)
    }

    fn finish_control<'a>(
        &'a self,
        invocation_id: ToolInvocationId,
        tool: AgentControlTool,
        failure: Option<CapabilityFailure>,
    ) -> AgentFuture<'a, Result<(), CapabilityFailure>> {
        self.inner.finish_control(invocation_id, tool, failure)
    }

    fn execute<'a>(
        &'a self,
        request: ModelCapabilityRequest,
    ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>> {
        Box::pin(async move {
            let started = Instant::now();
            let capability = request.request.capability_id();
            let arguments = fingerprint(&request.request);
            let outcome = self.inner.execute(request).await;
            self.record(capability, arguments, started, &outcome);
            outcome
        })
    }

    fn reject_arguments<'a>(
        &'a self,
        capability_id: CapabilityId,
        failure: CapabilityFailure,
    ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>> {
        Box::pin(async move {
            let started = Instant::now();
            let outcome = self.inner.reject_arguments(capability_id, failure).await;
            self.record(capability_id, None, started, &outcome);
            outcome
        })
    }

    fn delegate<'a>(
        &'a self,
        task: model::AgentTaskInput,
    ) -> AgentFuture<'a, Result<AgentTaskOutcome, CapabilityFailure>> {
        self.inner.delegate(task)
    }

    fn followup<'a>(
        &'a self,
        request: model::AgentFollowupInput,
    ) -> AgentFuture<'a, Result<AgentTaskOutcome, CapabilityFailure>> {
        self.inner.followup(request)
    }

    fn completion_feedback(&self) -> Option<String> {
        self.inner.completion_feedback()
    }
}

fn fingerprint(value: &impl Serialize) -> Option<String> {
    yss_canonical_hash::hash_canonical("yssbi.harness.measurement", value)
        .ok()
        .map(|bytes| bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
