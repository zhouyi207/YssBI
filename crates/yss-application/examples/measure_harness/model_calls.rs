//! Observe model-facing business calls, excluding controls and internal baseline reads.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use serde_json::Value;
use yss_assistant_contract::PROPOSE_STATISTICAL_PLAN_TOOL;
use yss_harness_contract::*;

pub type CallLog = Arc<Mutex<Vec<CallMeasurement>>>;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallMeasurement {
    pub role: AgentRoleId,
    pub capability_id: String,
    pub effect: Option<ToolEffect>,
    pub invocation_id: Option<ToolInvocationId>,
    pub argument_hash: Option<String>,
    pub result_hash: Option<String>,
    pub result_bytes: Option<usize>,
    pub result_characters: Option<usize>,
    pub elapsed_ms: u128,
    pub failure_code: Option<CapabilityFailureCode>,
    pub fatal: Option<AgentDriverFailureCode>,
}

impl CallMeasurement {
    pub fn record_projection(&mut self, projection: &Value) {
        let encoded = projection.to_string();
        self.result_hash = fingerprint(projection);
        self.result_bytes = Some(encoded.len());
        self.result_characters = Some(encoded.chars().count());
    }
}

pub struct ObservedExecutor {
    pub inner: Arc<dyn AgentToolExecutor>,
    pub role: AgentRoleId,
    pub effects: BTreeMap<String, ToolEffect>,
    pub calls: CallLog,
}

struct CallTrace {
    tool: String,
    invocation_id: Option<ToolInvocationId>,
}

tokio::task_local! {
    // The event sink runs in the invoking future. Scope, rather than wall-clock
    // matching, keeps concurrent identical calls and worker baselines distinct.
    static CURRENT_CALL: RefCell<CallTrace>;
}

pub fn is_control(tool: &str) -> bool {
    matches!(
        tool,
        DELEGATE_TASK_TOOL | FOLLOWUP_TASK_TOOL | PROPOSE_STATISTICAL_PLAN_TOOL
    )
}

pub fn observe_event(event: &HarnessEvent) {
    let (HarnessEvent::Agent(event) | HarnessEvent::AgentRunOutput { event, .. }) = event else {
        return;
    };
    let Ok(()) = CURRENT_CALL.try_with(|trace| {
        let mut trace = trace.borrow_mut();
        if let AgentEvent::ToolInvocationStarted {
            invocation_id,
            tool,
        } = event
            && tool == &trace.tool
        {
            // Binding may perform baseline reads first. The actual model call
            // starts last and owns the identity even when its outcome is fatal.
            trace.invocation_id = Some(invocation_id.clone());
        }
    }) else {
        // Internal invocations outside a model business call have no observation scope.
        return;
    };
}

impl AgentToolExecutor for ObservedExecutor {
    fn call<'a>(&'a self, call: ToolCall) -> AgentFuture<'a, ToolCallOutcome> {
        Box::pin(async move {
            if is_control(&call.name) {
                return self.inner.call(call).await;
            }
            let started = Instant::now();
            let trace = RefCell::new(CallTrace {
                tool: call.name.clone(),
                invocation_id: None,
            });
            CURRENT_CALL
                .scope(trace, async move {
                    let name = call.name.clone();
                    let outcome = self.inner.call(call).await;
                    let invocation_id =
                        CURRENT_CALL.with(|trace| trace.borrow().invocation_id.clone());
                    let mut measured = CallMeasurement {
                        role: self.role.clone(),
                        effect: self.effects.get(&name).copied(),
                        capability_id: name,
                        invocation_id: outcome.invocation_id.clone().or(invocation_id),
                        // Filled from the authoritative ledger, never raw provider JSON
                        // or an extension-bound request containing internal revisions.
                        argument_hash: None,
                        result_hash: None,
                        result_bytes: None,
                        result_characters: None,
                        elapsed_ms: started.elapsed().as_millis(),
                        failure_code: None,
                        fatal: outcome.fatal,
                    };
                    if measured.invocation_id.is_none() && outcome.fatal.is_none() {
                        measured.record_projection(&outcome.output);
                    }
                    self.calls
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .push(measured);
                    outcome
                })
                .await
        })
    }

    fn completion_feedback(&self) -> Option<String> {
        self.inner.completion_feedback()
    }

    fn summary_projection(&self, tool: &str, output: &Value) -> Option<Value> {
        self.inner.summary_projection(tool, output)
    }
}

pub fn fingerprint(value: &impl Serialize) -> Option<String> {
    yss_canonical_hash::hash_canonical("yssbi.harness.measurement", value)
        .ok()
        .map(|bytes| bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}
