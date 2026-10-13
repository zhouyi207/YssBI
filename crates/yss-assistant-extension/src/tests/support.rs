//! Typed access to Assistant calls made through the generic Harness executor.
//!
//! The executor returns model projections. Tests read the authoritative typed receipt, outcome
//! and failure back from the ledger and event store, as the Assistant projections do.

use std::sync::{Arc, OnceLock};

use serde_json::Value;
use yss_harness_core::test_support::InMemoryHarnessStore;

use crate::contract::*;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Receipt {
    pub invocation_id: ToolInvocationId,
    pub result: AutomationCapabilityResult,
}

/// The Harness outcome with its Assistant facts expanded.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TaskOutcome {
    pub run_id: AgentRunId,
    pub role: AgentRole,
    pub state: AgentRunState,
    pub report: Option<WorkerReport>,
    pub failure_code: Option<AgentDriverFailureCode>,
    pub evidence: Vec<ToolInvocationId>,
    pub artifacts: Vec<ResourceChange>,
    pub results: Vec<GraphResultReference>,
    pub plan: Option<StatisticalPlan>,
    pub invalidated_runs: Vec<AgentRunId>,
}

impl From<AgentTaskOutcome> for TaskOutcome {
    fn from(outcome: AgentTaskOutcome) -> Self {
        let facts = AgentTaskFacts::from_value(&outcome.facts).unwrap();
        Self {
            run_id: outcome.run_id,
            role: AgentRole::from_id(&outcome.role).unwrap(),
            state: outcome.state,
            report: outcome.report,
            failure_code: outcome.failure_code,
            evidence: outcome.evidence,
            artifacts: facts.artifacts,
            results: facts.results,
            plan: facts.plan,
            invalidated_runs: outcome.invalidated_runs,
        }
    }
}

/// Shared by a test and the driver it scripts, set once the store exists.
#[derive(Default)]
pub(crate) struct StoreSlot(OnceLock<Arc<InMemoryHarnessStore>>);

impl StoreSlot {
    pub fn set(&self, store: Arc<InMemoryHarnessStore>) {
        assert!(self.0.set(store).is_ok(), "store is set once");
    }

    pub fn get(&self) -> &InMemoryHarnessStore {
        self.0.get().expect("test store is set before the first turn")
    }
}

pub(crate) struct Tools<'a> {
    pub executor: &'a dyn AgentToolExecutor,
    pub store: &'a InMemoryHarnessStore,
}

fn sequence(id: &ToolInvocationId) -> u64 {
    id.as_str()
        .rsplit('-')
        .next()
        .and_then(|number| number.parse().ok())
        .unwrap_or_default()
}

impl Tools<'_> {
    pub async fn execute(
        &self,
        input: impl Into<model::CapabilityInput>,
    ) -> Result<Receipt, CapabilityFailure> {
        let input = input.into();
        let mut value = serde_json::to_value(&input).unwrap();
        let name = value["type"].as_str().unwrap().to_owned();
        let outcome = self
            .executor
            .call(ToolCall {
                name: name.clone(),
                arguments: value["payload"].take(),
            })
            .await;
        let records = self.store.tool_invocations();
        if let Some(id) = outcome.invocation_id {
            let record = records.into_iter().find(|record| record.id == id).unwrap();
            return Ok(Receipt {
                invocation_id: id,
                result: decode_result(record.result.as_ref().unwrap()).unwrap(),
            });
        }
        let record = records
            .into_iter()
            .filter(|record| record.tool == name && record.failure.is_some())
            .max_by_key(|record| sequence(&record.id))
            .unwrap();
        Err(record.failure.unwrap())
    }

    async fn control(&self, name: &str, arguments: Value) -> Result<TaskOutcome, CapabilityFailure> {
        let outcome = self
            .executor
            .call(ToolCall {
                name: name.into(),
                arguments,
            })
            .await;
        let events = self.store.published_events();
        if outcome.output.get("failure").is_some() || outcome.fatal.is_some() {
            return Err(events
                .into_iter()
                .rev()
                .find_map(|event| match event.event {
                    HarnessEvent::Agent(AgentEvent::ControlToolFinished {
                        tool,
                        failure_code: Some(code),
                        failure_details,
                        ..
                    }) if tool == name => Some(CapabilityFailure {
                        code,
                        details: failure_details.unwrap_or_default(),
                    }),
                    _ => None,
                })
                .unwrap());
        }
        let run_id = outcome.output["runId"].as_str().unwrap().to_owned();
        Ok(events
            .into_iter()
            .rev()
            .find_map(|event| match event.event {
                HarnessEvent::AgentRunFinished { outcome } if outcome.run_id.as_str() == run_id => {
                    Some(TaskOutcome::from(*outcome))
                }
                _ => None,
            })
            .unwrap())
    }

    pub async fn delegate(
        &self,
        input: model::AgentTaskInput,
    ) -> Result<TaskOutcome, CapabilityFailure> {
        self.control(DELEGATE_TASK_TOOL, serde_json::to_value(input).unwrap())
            .await
    }

    pub async fn followup(
        &self,
        input: AgentFollowupInput,
    ) -> Result<TaskOutcome, CapabilityFailure> {
        self.control(FOLLOWUP_TASK_TOOL, serde_json::to_value(input).unwrap())
            .await
    }

    pub fn completion_feedback(&self) -> Option<String> {
        self.executor.completion_feedback()
    }
}

/// Ports with the Assistant extension over `gateway`, backed by one in-memory store.
pub(crate) fn ports(
    store: Arc<InMemoryHarnessStore>,
    models: Arc<dyn LanguageModelResolverPort>,
    gateway: Arc<dyn CapabilityGatewayPort>,
) -> yss_harness_core::HarnessPorts {
    yss_harness_core::HarnessPorts {
        resources: Arc::new(yss_harness_core::test_support::FixtureResourceResolver),
        models,
        extension: Arc::new(crate::AssistantExtension::new(gateway).unwrap()),
        sessions: store.clone(),
        events: store.clone(),
        event_sink: store.clone(),
        workflows: store.clone(),
        tool_ledger: store.clone(),
        knowledge: store.clone(),
        knowledge_index: Arc::new(yss_harness_tantivy::TantivyKnowledgeIndex),
        approvals: store,
        clock: Arc::new(yss_harness_core::test_support::FixedClock::new(1000)),
        ids: Arc::new(yss_harness_core::test_support::SequentialIds::default()),
    }
}

pub(crate) fn test_project() -> ProjectSessionBinding {
    ProjectSessionBinding::new(
        yss_project_identity::ProjectInstanceId::from_existing("project".into()),
        yss_project_identity::ProjectSessionId::new("project-session"),
    )
}

/// One Assistant run over the real Harness ledger executor, without turn orchestration.
pub(crate) struct TestRun {
    pub hooks: Arc<dyn AgentRunHooks>,
    pub executor: Arc<dyn AgentToolExecutor>,
    pub store: Arc<InMemoryHarnessStore>,
}

impl TestRun {
    pub fn new(
        gateway: Arc<dyn CapabilityGatewayPort>,
        store: Arc<InMemoryHarnessStore>,
        ids: Arc<yss_harness_core::test_support::SequentialIds>,
        scope: AgentInvocationScope,
    ) -> Self {
        let extension = Arc::new(crate::AssistantExtension::new(gateway).unwrap());
        let (hooks, executor) = yss_harness_core::test_support::run_executor(
            extension,
            store.clone(),
            ids,
            RunStart {
                run_id: scope.run_id,
                role: scope.role.id(),
                task_scope: scope.task.map(|task| serde_json::to_value(task).unwrap()),
                project: test_project(),
            },
        );
        Self {
            hooks,
            executor,
            store,
        }
    }

    pub fn state(&self) -> &crate::run::AssistantRun {
        self.hooks.as_any().downcast_ref().unwrap()
    }

    pub async fn execute(
        &self,
        input: impl Into<model::CapabilityInput>,
    ) -> Result<Receipt, CapabilityFailure> {
        Tools {
            executor: self.executor.as_ref(),
            store: &self.store,
        }
        .execute(input)
        .await
    }
}
