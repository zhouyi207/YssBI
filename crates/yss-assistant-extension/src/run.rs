//! Per-run YssBI authority: task scope, read observations and receipt-derived evidence.

use std::any::Any;
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::Shared;
use crate::contract::*;

mod binding;
mod delegation;
pub(crate) mod observations;
mod receipts;

#[cfg(test)]
mod tests;

use observations::ResourceObservations;
use receipts::{Evidence, record_receipt};

pub(crate) struct AssistantRun {
    shared: Arc<Shared>,
    worker: bool,
    pub(crate) scope: Mutex<AgentInvocationScope>,
    pub(crate) evidence: Mutex<Evidence>,
    pub(crate) observations: Mutex<ResourceObservations>,
}

impl AssistantRun {
    pub(crate) fn new(
        shared: Arc<Shared>,
        run_id: AgentRunId,
        role: AgentRole,
        task: Option<AgentTaskScope>,
    ) -> Self {
        let evidence = task
            .as_ref()
            .map(|scope| Evidence::for_task(role, scope))
            .unwrap_or_default();
        Self {
            shared,
            worker: role != AgentRole::Manager,
            scope: Mutex::new(AgentInvocationScope {
                run_id,
                role,
                task,
            }),
            evidence: Mutex::new(evidence),
            observations: Mutex::new(ResourceObservations::default()),
        }
    }

    /// Execute an owner request issued by this run's own binding logic.
    async fn execute_internal(
        &self,
        runner: &dyn ToolRunner,
        request: AutomationCapabilityRequest,
    ) -> Result<(ToolInvocationId, AutomationCapabilityResult), CapabilityFailure> {
        let call = internal_call(&request)?;
        let outcome = runner.run(call).await?;
        let result = decode_result(&outcome.result).map_err(|_| internal_failure())?;
        Ok((outcome.invocation_id, result))
    }

    fn record_plan(&self, result: &Value) {
        match serde_json::from_value::<StatisticalPlan>(result["plan"].clone()) {
            Ok(plan) => {
                self.evidence
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .plan = Some(plan)
            }
            Err(error) => tracing::warn!(
                domain = "Assistant",
                event = "assistant_plan_receipt_unreadable",
                %error,
                "Assistant could not read a recorded statistical plan"
            ),
        }
    }

    fn record(
        &self,
        id: &ToolInvocationId,
        request: &AutomationCapabilityRequest,
        result: &AutomationCapabilityResult,
    ) {
        let mut evidence = self.evidence.lock().unwrap_or_else(|e| e.into_inner());
        if !evidence.invocations.contains(id) {
            evidence.invocations.push(id.clone());
            record_receipt(
                request,
                result,
                &mut evidence,
                &mut self.scope.lock().unwrap_or_else(|e| e.into_inner()),
            );
        }
    }

    async fn execute_call(
        &self,
        context: ToolInvocationContext,
        call: &PreparedToolCall,
        control: CapabilityControl,
    ) -> Result<ToolExecution, CapabilityFailure> {
        if call.tool == PROPOSE_STATISTICAL_PLAN_TOOL {
            return self.shared.propose_plan(&call.request);
        }
        let request = decode_request(&call.request).map_err(|_| internal_failure())?;
        let capability = request.capability_id();
        let scope = self.scope.lock().unwrap_or_else(|e| e.into_inner()).clone();
        crate::agents::authorize_agent_capability(&scope, &request)?;
        request
            .validate()
            .map_err(|error| contract_failure(error, capability))?;
        let graph_observation = if self.worker {
            self.graph_observation(&request)
        } else {
            None
        };
        let context = CapabilityInvocationContext::from(context)
            .with_agent(scope.clone())
            .with_graph_observation(graph_observation.clone());
        let result = self
            .shared
            .execute_request(context, &request, control)
            .await
            .and_then(|result| {
                crate::agents::validate_agent_read(&scope, &result)?;
                if let Some(expected) = graph_observation.as_deref()
                    && graph_read_inputs(&result).is_some_and(|actual| actual != expected)
                {
                    return Err(
                        CapabilityFailure::new(CapabilityFailureCode::REVISION_CONFLICT)
                            .with_detail("reason", "graph_inputs_changed"),
                    );
                }
                Ok(result)
            })
            .map_err(|failure| with_resource(failure, &request))?;
        crate::execution(result)
    }
}

impl AgentRunHooks for AssistantRun {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn replay(&self, record: &ToolInvocationRecord, project: &ProjectSessionBinding) {
        if crate::tools::is_harness_tool(&record.tool) {
            return;
        }
        let Some(result) = &record.result else {
            return;
        };
        if record.tool == PROPOSE_STATISTICAL_PLAN_TOOL {
            self.record_plan(result);
            return;
        }
        let decoded = record
            .request
            .as_ref()
            .map(decode_request)
            .transpose()
            .and_then(|request| Ok((request, decode_result(result)?)));
        let (request, result) = match decoded {
            Ok(decoded) => decoded,
            Err(error) => {
                tracing::warn!(
                    domain = "Assistant",
                    event = "assistant_receipt_unreadable",
                    tool = %record.tool,
                    %error,
                    "Assistant skipped an unreadable recorded receipt"
                );
                return;
            }
        };
        self.observations
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .record(&result, &record.project == project);
        if let Some(request) = request {
            self.record(&record.id, &request, &result);
        }
    }

    fn invalidate(&self, facts: &Value) {
        match AgentTaskFacts::from_value(facts) {
            Ok(facts) => self
                .observations
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .invalidate(&facts.artifacts),
            Err(error) => tracing::warn!(
                domain = "Assistant",
                event = "assistant_task_facts_unreadable",
                %error,
                "Assistant could not read delegated task facts"
            ),
        }
    }

    fn resume(&self, scope: &Value) {
        match serde_json::from_value::<AgentTaskScope>(scope.clone()) {
            Ok(scope) => {
                self.scope
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .task = Some(scope)
            }
            Err(error) => tracing::warn!(
                domain = "Assistant",
                event = "assistant_resumed_scope_unreadable",
                %error,
                "Assistant could not restore a resumed task scope"
            ),
        }
    }

    fn context_messages(&self) -> Vec<String> {
        let observations = self.observations.lock().unwrap_or_else(|e| e.into_inner());
        let resources_requiring_read = observations.resources_requiring_read();
        if resources_requiring_read.is_empty() {
            return Vec::new();
        }
        vec![serde_json::json!({
            "reason": "resource_read_required",
            "resourceIds": resources_requiring_read,
            "nextStep": "Read the listed resources before relying on historical facts for a new or resumed task. Their current contents have not been verified; do not assume they have changed."
        })
        .to_string()]
    }

    fn prepare<'a>(
        &'a self,
        call: ToolCall,
        runner: &'a dyn ToolRunner,
    ) -> AgentFuture<'a, Result<PreparedToolCall, RejectedToolCall>> {
        Box::pin(async move {
            if call.name == PROPOSE_STATISTICAL_PLAN_TOOL {
                let plan: StatisticalPlan = decode_arguments(
                    call.arguments,
                    &Value::from(statistical_plan_schema()),
                )?;
                let plan = serde_json::to_value(plan).map_err(|_| internal_failure())?;
                return Ok(PreparedToolCall {
                    tool: call.name,
                    arguments: plan.clone(),
                    request: plan,
                    idempotency_token: None,
                    internal: false,
                });
            }
            let capability = crate::tools::capability(&call.name).ok_or_else(|| {
                CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST)
                    .with_detail("capabilityId", &call.name)
            })?;
            let schema = Value::from(capability_input_schema(capability));
            let input = crate::tools::decode_input(capability, call.arguments, &schema)?;
            let arguments = model_arguments(&input)?;
            let rejected = |failure| RejectedToolCall {
                failure,
                arguments: Some(arguments.clone()),
            };
            let request = self.bind_input(input, runner).await.map_err(rejected)?;
            Ok(PreparedToolCall {
                tool: call.name,
                arguments: arguments.clone(),
                request: encode_request(&request).map_err(|_| rejected(internal_failure()))?,
                idempotency_token: request.graph_edit().map(|edit| edit.client_key),
                internal: false,
            })
        })
    }

    fn execute<'a>(
        &'a self,
        context: ToolInvocationContext,
        call: &'a PreparedToolCall,
        control: CapabilityControl,
    ) -> AgentFuture<'a, Result<ToolExecution, CapabilityFailure>> {
        Box::pin(self.execute_call(context, call, control))
    }

    fn observe(
        &self,
        call: &PreparedToolCall,
        outcome: &Result<ToolInvocationOutcome, CapabilityFailure>,
    ) {
        if crate::tools::is_harness_tool(&call.tool) {
            return;
        }
        if call.tool == PROPOSE_STATISTICAL_PLAN_TOOL {
            if let Ok(outcome) = outcome {
                self.record_plan(&outcome.result);
            }
            return;
        }
        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(failure) => {
                if self.worker
                    && (failure.code == CapabilityFailureCode::REVISION_CONFLICT
                        || failure.code == failure_code::GRAPH_DRAFT_CHANGED)
                {
                    self.evidence
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .blocked_reason = Some("input_changed".into());
                }
                return;
            }
        };
        let decoded = decode_request(&call.request)
            .and_then(|request| Ok((request, decode_result(&outcome.result)?)));
        let (request, result) = match decoded {
            Ok(decoded) => decoded,
            Err(error) => {
                tracing::warn!(
                    domain = "Assistant",
                    event = "assistant_receipt_unreadable",
                    tool = %call.tool,
                    %error,
                    "Assistant skipped an unreadable receipt"
                );
                return;
            }
        };
        self.record(&outcome.invocation_id, &request, &result);
        if !call.internal {
            self.observations
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .record(&result, true);
        }
    }

    fn bind_task<'a>(
        &'a self,
        input: AgentTaskInput,
        runner: &'a dyn ToolRunner,
    ) -> AgentFuture<'a, Result<Value, CapabilityFailure>> {
        Box::pin(self.bind_task_scope(input, runner))
    }

    fn resume_task<'a>(
        &'a self,
        worker: &'a dyn AgentRunHooks,
        runner: &'a dyn ToolRunner,
    ) -> AgentFuture<'a, Result<(Value, Value), CapabilityFailure>> {
        Box::pin(self.rebind_worker(worker, runner))
    }

    fn before_run<'a>(
        &'a self,
        runner: &'a dyn ToolRunner,
    ) -> AgentFuture<'a, Result<Option<String>, AgentDriverFailure>> {
        Box::pin(self.check_inputs(runner))
    }

    fn completion_feedback(&self, delegated: &[DelegatedTask<'_>]) -> Option<String> {
        if self.worker {
            return self
                .evidence
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .delivery_feedback();
        }
        let pending: Vec<_> = delegated
            .iter()
            .filter(|entry| {
                requires_delivery(entry.task)
                    && entry
                        .outcome
                        .is_none_or(|outcome| outcome.state != AgentRunState::Completed)
            })
            .map(|entry| {
                let artifacts = entry
                    .outcome
                    .and_then(|outcome| AgentTaskFacts::from_value(&outcome.facts).ok())
                    .map_or(0, |facts| facts.artifacts.len());
                serde_json::json!({
                    "runId": entry.run_id,
                    "state": entry.outcome.map(|outcome| outcome.state),
                    "failureCode": entry.outcome.and_then(|outcome| outcome.failure_code),
                    "artifactCount": artifacts,
                })
            })
            .collect();
        (!pending.is_empty()).then(|| serde_json::json!({
            "reason": "report_delivery_incomplete", "pending": pending,
            "nextStep": "Use followup_task to continue these workers from their receipts and finish saving the requested documents. Inspect changed inputs and reassess the task before resuming. Do not claim delivery without successful saves. If an external requirement prevents completion, explain the actual blocker."
        }).to_string())
    }

    fn finish(&self) -> RunFinish {
        let evidence = self.evidence.lock().unwrap_or_else(|e| e.into_inner());
        let mut finish = RunFinish {
            blocked_reason: evidence.blocked_reason.clone(),
            next_steps: Vec::new(),
            facts: Value::Null,
        };
        if evidence.delivery_pending() && finish.blocked_reason.is_none() {
            finish.blocked_reason = Some("report_resource_not_saved".into());
            finish.next_steps.push("Use followup_task to continue this worker from committed receipts. Use the authorized document or topic editing tools and finish with save_resource. A Mind-only task does not require a Doc.".into());
        }
        let facts = AgentTaskFacts {
            artifacts: evidence.artifacts.clone(),
            results: evidence.results.clone(),
            plan: evidence.plan.clone(),
        };
        match serde_json::to_value(facts) {
            Ok(facts) => finish.facts = facts,
            Err(error) => tracing::warn!(
                domain = "Assistant",
                event = "assistant_task_facts_unencodable",
                %error,
                "Assistant could not encode task facts"
            ),
        }
        finish
    }
}

fn requires_delivery(task: &AgentTask) -> bool {
    let Some(role) = AgentRole::from_id(&task.worker) else {
        return false;
    };
    match serde_json::from_value::<AgentTaskScope>(task.scope.clone()) {
        Ok(scope) => Evidence::for_task(role, &scope).delivery_pending(),
        Err(error) => {
            tracing::warn!(
                domain = "Assistant",
                event = "assistant_task_scope_unreadable",
                %error,
                "Assistant could not read a delegated task scope"
            );
            false
        }
    }
}

fn internal_failure() -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::INTERNAL_FAILURE)
}

fn persistence_failure() -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::PERSISTENCE_UNAVAILABLE)
}

fn rejected(reason: &str) -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST).with_detail("reason", reason)
}

/// The normalized model-facing arguments of a typed input.
pub(crate) fn model_arguments(input: &model::CapabilityInput) -> Result<Value, CapabilityFailure> {
    let mut value = serde_json::to_value(input).map_err(|_| internal_failure())?;
    Ok(value["payload"].take())
}

pub(crate) fn internal_call(
    request: &AutomationCapabilityRequest,
) -> Result<PreparedToolCall, CapabilityFailure> {
    Ok(PreparedToolCall {
        tool: request.capability_id().as_str().to_owned(),
        arguments: model_arguments(&request.into())?,
        request: encode_request(request).map_err(|_| internal_failure())?,
        idempotency_token: request.graph_edit().map(|edit| edit.client_key),
        internal: true,
    })
}

fn graph_read_inputs(result: &AutomationCapabilityResult) -> Option<&str> {
    match result {
        AutomationCapabilityResult::GraphInspection(value) => Some(&value.semantic_input_hash),
        AutomationCapabilityResult::GraphInspectionPage(value) => Some(&value.semantic_input_hash),
        _ => None,
    }
}

/// Owners report revision conflicts without the model-facing resource identity.
pub(crate) fn with_resource(
    mut failure: CapabilityFailure,
    request: &AutomationCapabilityRequest,
) -> CapabilityFailure {
    if (failure.code == CapabilityFailureCode::REVISION_CONFLICT
        || failure.code == failure_code::GRAPH_DRAFT_CHANGED)
        && !failure.details.contains_key("resourceId")
        && let Some(resource) = request_resource_id(request)
    {
        failure = failure.with_detail("resourceId", resource);
    }
    failure
}

fn request_resource_id(request: &AutomationCapabilityRequest) -> Option<&str> {
    use AutomationCapabilityRequest::*;
    use ManageResourceRequest as Manage;
    Some(match request {
        InspectChart(value) => &value.chart.id,
        InspectResource(value) => &value.resource.id,
        ManageResource(
            Manage::Rename { resource, .. }
            | Manage::Duplicate { resource, .. }
            | Manage::Delete { resource, .. }
            | Manage::Save { resource, .. },
        ) => &resource.id,
        EditResource(value) => &value.resource.id,
        ReadDatabase(value) => &value.input.database().id,
        ReadMind(value) => &value.input.mind().id,
        ReadDocument(value) => &value.input.document().id,
        ExportDatabase(value) => &value.resource.id,
        InspectDatasetSchema(value) => &value.database_id,
        InspectDatasetProfile(value) => &value.database_id,
        InspectGraph(value) => &value.graph.id,
        FindNodes(value) => &value.graph.id,
        FindConstants(value) => &value.graph.id,
        InspectConstants(value) => &value.graph.id,
        InspectNodes(value) => &value.graph.id,
        FindConnections(value) => &value.graph.id,
        ApplyGraphEdit(value) => &value.graph_path,
        GraphMutation(value) => &value.input.graph().id,
        ValidateGraph(value) => &value.graph.id,
        ExecuteGraph(value) => &value.graph.id,
        SaveGraph(value) => &value.graph_path,
        ListGraphResults(value) => &value.graph.id,
        _ => return None,
    })
}
