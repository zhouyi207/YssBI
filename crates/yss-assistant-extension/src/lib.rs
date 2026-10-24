//! The YssBI Assistant extension for the domain-neutral Harness: Manager–Worker roles and prompts,
//! typed Application tools, task authority, read observations, receipt evidence, delivery checks,
//! statistical planning and built-in knowledge.

#![forbid(unsafe_code)]

mod agents;
mod knowledge;
mod planner;
mod run;
mod skills;
mod summary;
mod tools;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

#[cfg(test)]
mod tests;

/// Generic Harness and YssBI business contracts under one path for this extension.
mod contract {
    pub use yss_assistant_contract::*;
    pub use yss_harness_contract::*;
}

use std::sync::Arc;

use serde_json::Value;

use contract::*;

pub use agents::{
    AGENT_DEFINITIONS, AgentDefinition, agent_definition, authorize_agent_capability,
    authorize_agent_resource,
};
pub use knowledge::{KnowledgeError, install_builtin_statistical_knowledge};
pub use planner::{MethodRegistry, StatisticalPlanner, StatisticalPlannerError};
pub use skills::{SkillError, SkillRegistry};

#[derive(Debug, thiserror::Error)]
pub enum AssistantExtensionError {
    #[error("built-in Assistant skill is unavailable")]
    Skill(#[from] SkillError),
    #[error("built-in statistical methods are unavailable")]
    Methods(#[from] StatisticalPlannerError),
}

/// State shared by the extension and every run it starts.
pub(crate) struct Shared {
    gateway: Arc<dyn CapabilityGatewayPort>,
    report_writing_skill: SkillPackage,
    methods: MethodRegistry,
}

impl Shared {
    /// Invoke an owner request and verify that its receipt answers the request.
    pub(crate) async fn execute_request(
        &self,
        context: CapabilityInvocationContext,
        request: &AutomationCapabilityRequest,
        control: CapabilityControl,
    ) -> Result<AutomationCapabilityResult, CapabilityFailure> {
        let capability = request.capability_id();
        let mut outcome = self
            .gateway
            .invoke(context.clone(), request.clone(), control)
            .await;
        if outcome.as_ref().is_err_and(|failure| {
            failure.code == CapabilityFailureCode::INTERNAL_FAILURE
                || failure.code == CapabilityFailureCode::OUTCOME_UNKNOWN
        }) && let Some(edit) = request.graph_edit()
            && let Ok(Some(receipt)) = self.gateway.recover_graph_edit(context, edit).await
        {
            outcome = Ok(AutomationCapabilityResult::GraphEditReceipt(receipt));
        }
        let result = outcome?;
        if !result.accepts_capability(capability) {
            return Err(CapabilityFailure::new(
                CapabilityFailureCode::INTERNAL_FAILURE,
            ));
        }
        // Owners bound mutation receipts before committing. Never turn an already
        // committed write into a retryable size failure or drop its created IDs.
        if capability.descriptor().effect == ToolEffect::Inspect {
            result.validate_budget(MAX_CAPABILITY_RESULT_BYTES)?;
        }
        Ok(result)
    }

    pub(crate) fn propose_plan(&self, request: &Value) -> Result<ToolExecution, CapabilityFailure> {
        let plan: StatisticalPlan = serde_json::from_value(request.clone()).map_err(|_| {
            CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST)
                .with_detail("capabilityId", PROPOSE_STATISTICAL_PLAN_TOOL)
        })?;
        if let Err(error) = StatisticalPlanner::validate(&plan, &self.methods) {
            let methods =
                serde_json::to_string(&self.methods.cards()).map_err(|_| internal_failure())?;
            return Err(
                CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST)
                    .with_detail("reason", error.to_string())
                    .with_detail("availableMethods", methods),
            );
        }
        let payload = serde_json::to_value(&plan).map_err(|_| internal_failure())?;
        Ok(ToolExecution {
            result: serde_json::json!({ "accepted": true, "plan": payload }),
            notices: vec![AgentNotice {
                kind: PLAN_PROPOSED_NOTICE.into(),
                payload,
            }],
        })
    }
}

/// Encode an owner receipt and publish its domain execution status.
pub(crate) fn execution(
    result: AutomationCapabilityResult,
) -> Result<ToolExecution, CapabilityFailure> {
    let mut notices = Vec::new();
    if let AutomationCapabilityResult::GraphExecution(execution) = &result {
        notices.push(AgentNotice {
            kind: GRAPH_EXECUTION_NOTICE.into(),
            payload: serde_json::to_value(GraphExecutionNotice {
                status: execution.status.clone(),
                failure_code: execution.failure_code.clone(),
            })
            .map_err(|_| internal_failure())?,
        });
    }
    Ok(ToolExecution {
        result: encode_result(&result).map_err(|_| internal_failure())?,
        notices,
    })
}

fn internal_failure() -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::INTERNAL_FAILURE)
}

fn rejected(reason: &str) -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST).with_detail("reason", reason)
}

/// The YssBI Assistant: six statistical roles over typed Application capabilities.
pub struct AssistantExtension {
    shared: Arc<Shared>,
}

impl AssistantExtension {
    pub fn new(gateway: Arc<dyn CapabilityGatewayPort>) -> Result<Self, AssistantExtensionError> {
        Ok(Self {
            shared: Arc::new(Shared {
                gateway,
                report_writing_skill: skills::report_writing_skill()?,
                methods: MethodRegistry::builtins()?,
            }),
        })
    }

    fn decode_scope(scope: &Value) -> Result<AgentTaskScope, CapabilityFailure> {
        serde_json::from_value(scope.clone()).map_err(|_| rejected("invalid field 'scope'"))
    }
}

impl AssistantExtension {
    fn validate_request(&self, tool: &str, request: &Value) -> Result<(), CapabilityFailure> {
        let request = decode_request(request).map_err(|_| {
            CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST)
                .with_detail("capabilityId", tool)
        })?;
        let capability = request.capability_id();
        if capability.as_str() != tool {
            return Err(
                CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST)
                    .with_detail("capabilityId", tool),
            );
        }
        request
            .validate()
            .map_err(|error| contract_failure(error, capability))
    }
}

impl HarnessExtension for AssistantExtension {
    fn root_role(&self) -> AgentRoleId {
        AgentRole::Manager.id()
    }

    fn profile(&self, role: &AgentRoleId) -> Option<AgentProfile> {
        let role = AgentRole::from_id(role)?;
        let definition = agent_definition(role);
        let mut instructions = vec![
            include_str!("agents/prompts/tools.md").to_owned(),
            definition.instructions.to_owned(),
        ];
        if matches!(role, AgentRole::Manager | AgentRole::Report) {
            // Preload the scoped writing method so follow-up report edits retain its rules.
            let skill = &self.shared.report_writing_skill;
            instructions.push(format!(
                "Built-in skill {}@{}:\n{}",
                skill.manifest.id, skill.manifest.version, skill.instructions,
            ));
        }
        Some(AgentProfile {
            role: role.id(),
            instructions,
            tools: tools::role_specs(definition, &self.shared.methods),
            tool_concurrency: definition.tool_concurrency(),
            delegates: definition.delegates(),
            output_mode: definition.output_mode(),
        })
    }

    fn mode_instructions(&self, mode: HarnessMode) -> Option<String> {
        (mode == HarnessMode::Ask).then(|| "Ask mode: answer and inspect existing resources only. Do not create, edit, save, export, execute graphs, or request UI actions. Delegate only read-only tasks. Explain when the user needs to switch to Write mode.".into())
    }

    fn delegate_task_schema(&self) -> Value {
        Value::from(schemars::schema_for!(model::AgentTaskInput))
    }

    fn decode_task_input(&self, arguments: Value) -> Result<AgentTaskInput, CapabilityFailure> {
        let input: model::AgentTaskInput =
            decode_arguments(arguments, &self.delegate_task_schema())?;
        Ok(AgentTaskInput {
            worker: input.worker.id(),
            objective: input.objective,
            constraints: input.constraints,
            completion_criteria: input.completion_criteria,
            depends_on: input.depends_on,
            scope: serde_json::to_value(input.scope).map_err(|_| internal_failure())?,
        })
    }

    fn validate_task(&self, task: &AgentTask) -> Result<(), CapabilityFailure> {
        let role =
            AgentRole::from_id(&task.worker).ok_or_else(|| rejected("invalid field 'task'"))?;
        Self::decode_scope(&task.scope)?
            .validate(role)
            .map_err(|error| rejected(&error.to_string()))
    }

    fn scope_is_read_only(&self, scope: &Value) -> bool {
        // An unreadable scope is treated as a writer so it cannot bypass the write gate or Ask mode.
        Self::decode_scope(scope).is_ok_and(|scope| scope.is_read_only())
    }

    fn task_scope_input(&self, scope: &Value) -> Value {
        Self::decode_scope(scope)
            .and_then(|scope| {
                serde_json::to_value(model::AgentTaskScopeInput::from(&scope))
                    .map_err(|_| internal_failure())
            })
            .unwrap_or_else(|failure| {
                tracing::warn!(
                    domain = "Assistant",
                    event = "assistant_task_scope_unreadable",
                    code = %failure.code,
                    "Assistant could not project a recorded task scope"
                );
                Value::Null
            })
    }

    fn task_invalidated(&self, scope: &Value, facts: &Value) -> bool {
        let (Ok(scope), Ok(facts)) = (Self::decode_scope(scope), AgentTaskFacts::from_value(facts))
        else {
            // Unreadable facts cannot prove that a completed task stayed current.
            return true;
        };
        scope.resources.iter().any(|input| {
            facts.artifacts.iter().any(|change| {
                change.resource == input.resource
                    && (change.deleted
                        || change.revision_kind != ResourceRevisionKind::Resource
                        || input
                            .version
                            .as_ref()
                            .is_none_or(|version| version.revision != change.revision))
            })
        })
    }

    fn project_result(&self, tool: &str, result: &Value) -> Result<Value, CapabilityFailure> {
        project_tool_result(tool, result).map_err(|_| internal_failure())
    }

    fn project_failure(&self, failure: &CapabilityFailure) -> Value {
        let mut projected = model::failure(failure);
        if let Some(methods) = failure.details.get("availableMethods")
            && let Ok(methods) = serde_json::from_str::<Value>(methods)
        {
            projected["details"]["availableMethods"] = methods;
        }
        projected
    }

    fn project_task_facts(&self, facts: &Value) -> Value {
        match AgentTaskFacts::from_value(facts) {
            Ok(facts) => model::task_facts(&facts),
            Err(error) => {
                tracing::warn!(
                    domain = "Assistant",
                    event = "assistant_task_facts_unreadable",
                    %error,
                    "Assistant could not project task facts"
                );
                Value::Object(Default::default())
            }
        }
    }

    fn summary_projection(&self, tool: &str, output: &Value) -> Option<Value> {
        summary::projection(tool, output)
    }

    fn execute<'a>(
        &'a self,
        context: ToolInvocationContext,
        tool: &'a str,
        request: &'a Value,
        control: CapabilityControl,
    ) -> AgentFuture<'a, Result<ToolExecution, CapabilityFailure>> {
        Box::pin(async move {
            if tool == PROPOSE_STATISTICAL_PLAN_TOOL {
                return self.shared.propose_plan(request);
            }
            self.validate_request(tool, request)?;
            let request = decode_request(request).map_err(|_| internal_failure())?;
            let result = self
                .shared
                .execute_request(context.into(), &request, control)
                .await
                .map_err(|failure| run::with_resource(failure, &request))?;
            execution(result)
        })
    }

    fn recover<'a>(
        &'a self,
        context: ToolInvocationContext,
        _tool: &'a str,
        request: &'a Value,
    ) -> AgentFuture<'a, Result<ToolRecovery, CapabilityFailure>> {
        Box::pin(async move {
            let request = decode_request(request).map_err(|_| internal_failure())?;
            let Some(edit) = request.graph_edit() else {
                return Ok(ToolRecovery::Unsupported);
            };
            match self
                .shared
                .gateway
                .recover_graph_edit(context.into(), edit)
                .await?
            {
                Some(receipt) => Ok(ToolRecovery::Committed(
                    encode_result(&AutomationCapabilityResult::GraphEditReceipt(receipt))
                        .map_err(|_| internal_failure())?,
                )),
                None => Ok(ToolRecovery::Unknown),
            }
        })
    }

    fn start_run(&self, run: RunStart) -> Arc<dyn AgentRunHooks> {
        let role = AgentRole::from_id(&run.role);
        let task = run
            .task_scope
            .as_ref()
            .map(Self::decode_scope)
            .transpose()
            .unwrap_or_else(|failure| {
                tracing::warn!(
                    domain = "Assistant",
                    event = "assistant_task_scope_unreadable",
                    code = %failure.code,
                    "Assistant started a run without its unreadable scope"
                );
                None
            });
        // An unknown role receives an empty worker scope and therefore no authority.
        let (role, task) = match role {
            Some(role) => (role, task),
            None => (AgentRole::Review, Some(AgentTaskScope::default())),
        };
        Arc::new(run::AssistantRun::new(
            Arc::clone(&self.shared),
            run.run_id,
            role,
            task,
        ))
    }
}
