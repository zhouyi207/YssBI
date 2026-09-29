use std::collections::BTreeMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use tokio::sync::{RwLock, Semaphore};
use yss_harness_contract::*;

use crate::conversation::agent_messages;
use crate::events::{EventWriter, PersistingAgentOutput};
use crate::tools::HarnessToolExecutor;
use crate::{HarnessPorts, ToolRegistry};

const MAX_TASKS: usize = 24;
const MAX_TOOL_CALLS: usize = 256;

struct TaskEntry {
    task: AgentTask,
    run_id: AgentRunId,
    outcome: Option<AgentTaskOutcome>,
}

pub(crate) struct TurnOrchestrator {
    pub ports: HarnessPorts,
    pub session: HarnessSessionRecord,
    pub turn_id: HarnessTurnId,
    pub writer: EventWriter,
    pub skill: SkillPackage,
    pub cancellation: CancellationToken,
    pub access: Arc<RwLock<()>>,
    manager_run_id: AgentRunId,
    tasks: Mutex<BTreeMap<String, TaskEntry>>,
    slots: Semaphore,
    tool_calls: Arc<AtomicUsize>,
}

impl TurnOrchestrator {
    #[allow(
        clippy::too_many_arguments,
        reason = "one user-turn authority and its injected services"
    )]
    pub(crate) fn new(
        ports: HarnessPorts,
        session: HarnessSessionRecord,
        turn_id: HarnessTurnId,
        writer: EventWriter,
        skill: SkillPackage,
        cancellation: CancellationToken,
        access: Arc<RwLock<()>>,
    ) -> Result<Arc<Self>, AgentDriverFailure> {
        let manager_run_id = AgentRunId::try_new(
            ports
                .ids
                .next_id(AutomationIdKind::AgentRun)
                .map_err(|_| driver_failure())?,
        )
        .map_err(|_| driver_failure())?;
        Ok(Arc::new(Self {
            ports,
            session,
            turn_id,
            writer,
            skill,
            cancellation,
            access,
            manager_run_id,
            tasks: Mutex::new(BTreeMap::new()),
            slots: Semaphore::new(4),
            tool_calls: Arc::new(AtomicUsize::new(0)),
        }))
    }

    pub(crate) async fn run(
        self: &Arc<Self>,
        messages: Vec<AgentMessage>,
    ) -> Result<AgentTurnResult, AgentDriverFailure> {
        self.append(HarnessEvent::AgentRunStarted {
            run_id: self.manager_run_id.clone(),
            parent_run_id: None,
            role: AgentRole::Manager,
            task: None,
        })
        .await
        .map_err(|_| driver_failure())?;
        let scope = Arc::new(Mutex::new(AgentInvocationScope {
            run_id: self.manager_run_id.clone(),
            role: AgentRole::Manager,
            task: None,
        }));
        let evidence = Arc::new(Mutex::new(Evidence::default()));
        let output = Arc::new(PersistingAgentOutput {
            writer: self.writer.clone(),
            session_id: self.session.id.clone(),
            turn_id: self.turn_id.clone(),
            run_id: None,
        });
        let registry = ToolRegistry::for_agent(AgentRole::Manager);
        let request = Self::request(AgentRole::Manager, messages, registry.descriptors());
        let executor = Arc::new(RunExecutor {
            tools: self.executor(
                registry,
                scope.clone(),
                output.clone(),
                self.cancellation.clone(),
            ),
            scope,
            evidence: evidence.clone(),
            manager: Some(self.clone()),
            tool_calls: self.tool_calls.clone(),
        });
        let mut result = self
            .ports
            .agent_driver
            .run_turn(request, executor, output, self.cancellation.clone())
            .await;
        if self.cancellation.is_cancelled() {
            result = Err(cancelled_driver());
        }
        let outcome = finish_outcome(
            self.manager_run_id.clone(),
            AgentRole::Manager,
            &result,
            &evidence,
        );
        self.append(HarnessEvent::AgentRunFinished {
            outcome: Box::new(outcome),
        })
        .await
        .map_err(|_| driver_failure())?;
        result
    }

    fn request(
        role: AgentRole,
        messages: Vec<AgentMessage>,
        tools: Vec<ToolDescriptor>,
    ) -> AgentTurnRequest {
        let definition = crate::agent_definition(role);
        AgentTurnRequest {
            role,
            limits: definition.limits(),
            control_tools: definition.control_tools(),
            output_mode: definition.output_mode(),
            messages,
            tools,
        }
    }

    fn executor(
        &self,
        registry: ToolRegistry,
        scope: Arc<Mutex<AgentInvocationScope>>,
        output: Arc<dyn AgentEventOutput>,
        cancellation: CancellationToken,
    ) -> HarnessToolExecutor {
        HarnessToolExecutor::new(
            registry,
            self.ports.capability_gateway.clone(),
            self.ports.tool_ledger.clone(),
            self.ports.clock.clone(),
            self.ports.ids.clone(),
            self.session.principal_id.clone(),
            self.session.id.clone(),
            self.turn_id.clone(),
            self.session.project.clone(),
            cancellation,
        )
        .with_agent(scope)
        .with_output(output)
    }

    async fn append(&self, event: HarnessEvent) -> Result<(), CapabilityFailure> {
        self.writer
            .append(&self.session.id, Some(&self.turn_id), event)
            .await
            .map_err(|_| persistence_failure())
    }

    async fn delegate(
        self: &Arc<Self>,
        task: AgentTask,
    ) -> Result<AgentTaskOutcome, CapabilityFailure> {
        task.validate()
            .map_err(|error| rejected(&error.to_string()))?;
        if self.cancellation.is_cancelled() {
            return Err(cancelled());
        }
        let (run_id, dependencies) = {
            let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(existing) = tasks.get(&task.key) {
                if existing.task != task {
                    return Err(rejected("task_key_conflict"));
                }
                return existing
                    .outcome
                    .clone()
                    .ok_or_else(|| rejected("task_still_running"));
            }
            if tasks.len() >= MAX_TASKS {
                return Err(rejected("task_budget_exhausted"));
            }
            let dependencies = task
                .depends_on
                .iter()
                .map(|id| {
                    tasks
                        .values()
                        .find(|entry| &entry.run_id == id)
                        .and_then(|entry| entry.outcome.as_ref())
                        .filter(|outcome| outcome.state == AgentRunState::Completed)
                        .cloned()
                        .ok_or_else(|| rejected("dependency_not_completed"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let run_id = AgentRunId::try_new(
                self.ports
                    .ids
                    .next_id(AutomationIdKind::AgentRun)
                    .map_err(|_| persistence_failure())?,
            )
            .map_err(|_| persistence_failure())?;
            tasks.insert(
                task.key.clone(),
                TaskEntry {
                    task: task.clone(),
                    run_id: run_id.clone(),
                    outcome: None,
                },
            );
            (run_id, dependencies)
        };
        self.append(HarnessEvent::AgentRunStarted {
            run_id: run_id.clone(),
            parent_run_id: Some(self.manager_run_id.clone()),
            role: task.worker,
            task: Some(Box::new(task.clone())),
        })
        .await?;
        let evidence = Arc::new(Mutex::new(Evidence::for_task(&task)));
        let result = self
            .run_worker(&run_id, &task, dependencies, evidence.clone())
            .await;
        let mut outcome = finish_outcome(run_id, task.worker, &result, &evidence);
        outcome.invalidated_runs = self.invalidate_dependents(&outcome);
        for run_id in &outcome.invalidated_runs {
            self.append(HarnessEvent::AgentRunInvalidated {
                run_id: run_id.clone(),
            })
            .await?;
        }
        self.append(HarnessEvent::AgentRunFinished {
            outcome: Box::new(outcome.clone()),
        })
        .await?;
        self.tasks
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_mut(&task.key)
            .ok_or_else(persistence_failure)?
            .outcome = Some(outcome.clone());
        Ok(outcome)
    }

    fn invalidate_dependents(&self, outcome: &AgentTaskOutcome) -> Vec<AgentRunId> {
        let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
        let mut invalidated = Vec::new();
        loop {
            let before = invalidated.len();
            for entry in tasks.values_mut() {
                let Some(previous) = &mut entry.outcome else {
                    continue;
                };
                if previous.state != AgentRunState::Completed {
                    continue;
                }
                let changed = entry.task.scope.resources.iter().any(|input| {
                    outcome.artifacts.iter().any(|change| {
                        change.resource == input.resource
                            && (change.deleted
                                || change.revision_kind != ResourceRevisionKind::Resource
                                || input
                                    .version
                                    .as_ref()
                                    .is_none_or(|version| version.revision != change.revision))
                    })
                });
                if changed
                    || entry
                        .task
                        .depends_on
                        .iter()
                        .any(|id| invalidated.contains(id))
                {
                    previous.state = AgentRunState::Stale;
                    invalidated.push(entry.run_id.clone());
                }
            }
            if before == invalidated.len() {
                break;
            }
        }
        invalidated
    }

    async fn run_worker(
        self: &Arc<Self>,
        run_id: &AgentRunId,
        task: &AgentTask,
        dependencies: Vec<AgentTaskOutcome>,
        evidence: Arc<Mutex<Evidence>>,
    ) -> Result<AgentTurnResult, AgentDriverFailure> {
        let deadline = tokio::time::Instant::now()
            + std::time::Duration::from_millis(
                crate::agent_definition(task.worker)
                    .limits()
                    .maximum_duration_ms,
            );
        let _slot = tokio::select! {
            permit = self.slots.acquire() => permit.map_err(|_| driver_failure())?,
            _ = self.cancellation.cancelled() => return Err(cancelled_driver()),
            _ = tokio::time::sleep_until(deadline) => return Err(AgentDriverFailure::new(AgentDriverFailureCode::DeadlineElapsed)),
        };
        // A conservative shared gate permits concurrent readers and serializes all writers.
        // Business owners still validate revisions against concurrent GUI operations.
        let (_read, _write) = if task.scope.is_read_only() {
            let guard = tokio::select! {
                guard = self.access.read() => guard,
                _ = self.cancellation.cancelled() => return Err(cancelled_driver()),
                _ = tokio::time::sleep_until(deadline) => return Err(AgentDriverFailure::new(AgentDriverFailureCode::DeadlineElapsed)),
            };
            (Some(guard), None)
        } else {
            let guard = tokio::select! {
                guard = self.access.write() => guard,
                _ = self.cancellation.cancelled() => return Err(cancelled_driver()),
                _ = tokio::time::sleep_until(deadline) => return Err(AgentDriverFailure::new(AgentDriverFailureCode::DeadlineElapsed)),
            };
            (None, Some(guard))
        };
        let cancellation = CancellationToken::default();
        let scope = Arc::new(Mutex::new(AgentInvocationScope {
            run_id: run_id.clone(),
            role: task.worker,
            task: Some(task.scope.clone()),
        }));
        let output = Arc::new(WorkerOutput {
            inner: PersistingAgentOutput {
                writer: self.writer.clone(),
                session_id: self.session.id.clone(),
                turn_id: self.turn_id.clone(),
                run_id: Some(run_id.clone()),
            },
            role: task.worker,
            evidence: evidence.clone(),
        });
        let registry = ToolRegistry::for_agent(task.worker);
        let tools = registry.descriptors();
        let executor = Arc::new(RunExecutor {
            tools: self.executor(
                registry,
                scope.clone(),
                output.clone(),
                cancellation.clone(),
            ),
            scope,
            evidence,
            manager: None,
            tool_calls: self.tool_calls.clone(),
        });
        let input = serde_json::to_string(
            &serde_json::json!({ "task": task, "dependencies": dependencies }),
        )
        .map_err(|_| driver_failure())?;
        let messages = agent_messages(input, &[], Vec::new(), None, &self.skill, task.worker);
        let request = Self::request(task.worker, messages, tools);
        let work = async {
            for access in &task.scope.resources {
                if let Some(version) = &access.version {
                    let outcome = executor
                        .execute(ModelCapabilityRequest {
                            request: AutomationCapabilityRequest::InspectResource(
                                InspectResourceRequest {
                                    resource: access.resource.clone(),
                                    offset: 0,
                                    limit: 1,
                                },
                            ),
                        })
                        .await
                        .map_err(|_| {
                            AgentDriverFailure::new(AgentDriverFailureCode::InvalidProviderResponse)
                        })?;
                    if !matches!(outcome.result, AutomationCapabilityResult::ResourceInspection(ref value) if &value.version == version)
                    {
                        return Ok(AgentTurnResult { final_text: serde_json::to_string(&WorkerReport {
                            summary: "Input resource version changed before execution.".into(),
                            warnings: Vec::new(), blocked_reason: Some("input_revision_changed".into()),
                            next_steps: vec!["Inspect the current resource and delegate a task using its current version.".into()],
                        }).map_err(|_| driver_failure())? });
                    }
                }
            }
            self.ports
                .agent_driver
                .run_turn(request, executor, output, cancellation.clone())
                .await
        };
        tokio::pin!(work);
        tokio::select! {
            result = &mut work => result,
            _ = tokio::time::sleep_until(deadline) => {
                cancellation.cancel(CancellationReason::DeadlineElapsed);
                let _ = work.await;
                Err(AgentDriverFailure::new(AgentDriverFailureCode::DeadlineElapsed))
            }
            _ = self.cancellation.cancelled() => {
                cancellation.cancel(self.cancellation.reason().unwrap_or(CancellationReason::User));
                // Await accepted business operations so real commit receipts survive cancellation.
                let _ = work.await;
                Err(cancelled_driver())
            }
        }
    }
}

struct WorkerOutput {
    inner: PersistingAgentOutput,
    role: AgentRole,
    evidence: Arc<Mutex<Evidence>>,
}

impl AgentEventOutput for WorkerOutput {
    fn emit<'a>(&'a self, event: AgentEvent) -> AgentFuture<'a, Result<(), AgentOutputFailure>> {
        Box::pin(async move {
            if matches!(&event, AgentEvent::PlanProposed { .. }) && self.role != AgentRole::Stats {
                return Err(AgentOutputFailure::Closed);
            }
            self.inner.emit(event.clone()).await?;
            let mut evidence = self.evidence.lock().unwrap_or_else(|e| e.into_inner());
            match event {
                AgentEvent::PlanProposed { plan } => evidence.plan = Some(plan),
                AgentEvent::ToolInvocationFailed { invocation_id, .. }
                    if !evidence.invocations.contains(&invocation_id) =>
                {
                    evidence.invocations.push(invocation_id);
                }
                _ => {}
            }
            Ok(())
        })
    }
}

struct RunExecutor {
    tools: HarnessToolExecutor,
    scope: Arc<Mutex<AgentInvocationScope>>,
    evidence: Arc<Mutex<Evidence>>,
    manager: Option<Arc<TurnOrchestrator>>,
    tool_calls: Arc<AtomicUsize>,
}

impl ModelCapabilityExecutor for RunExecutor {
    fn execute<'a>(
        &'a self,
        request: ModelCapabilityRequest,
    ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>> {
        Box::pin(async move {
            if self.tool_calls.fetch_add(1, Ordering::AcqRel) >= MAX_TOOL_CALLS {
                return Err(rejected("tool_budget_exhausted"));
            }
            let outcome = self.tools.execute(request.clone()).await?;
            let mut evidence = self.evidence.lock().unwrap_or_else(|e| e.into_inner());
            if !evidence.invocations.contains(&outcome.invocation_id) {
                evidence.invocations.push(outcome.invocation_id.clone());
                record_receipt(
                    &request.request,
                    &outcome.result,
                    &mut evidence,
                    &mut self.scope.lock().unwrap_or_else(|e| e.into_inner()),
                );
            }
            Ok(outcome)
        })
    }

    fn delegate<'a>(
        &'a self,
        task: AgentTask,
    ) -> AgentFuture<'a, Result<AgentTaskOutcome, CapabilityFailure>> {
        Box::pin(async move {
            self.manager
                .as_ref()
                .ok_or_else(|| rejected("workers_cannot_delegate"))?
                .delegate(task)
                .await
        })
    }
}

fn rejected(reason: &str) -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::InvalidRequest).with_detail("reason", reason)
}
fn persistence_failure() -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::PersistenceUnavailable)
}
fn cancelled() -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::Cancelled)
}
fn driver_failure() -> AgentDriverFailure {
    AgentDriverFailure::new(AgentDriverFailureCode::InternalFailure)
}
fn cancelled_driver() -> AgentDriverFailure {
    AgentDriverFailure::new(AgentDriverFailureCode::Cancelled)
}

mod receipts;
mod recovery;
use receipts::{Evidence, finish_outcome, record_receipt};
pub(crate) use recovery::{pending_agent_turns, recover_runs};

#[cfg(test)]
mod tests;
