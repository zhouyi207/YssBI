use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tokio::sync::{RwLock, Semaphore};
use tracing::Instrument;
use yss_harness_contract::*;

use crate::conversation::agent_messages;
use crate::events::{EventWriter, PersistingAgentOutput};
use crate::tools::HarnessToolExecutor;
use crate::{HarnessPorts, ToolRegistry};

struct TaskEntry {
    task: AgentTask,
    run_id: AgentRunId,
    outcome: Option<AgentTaskOutcome>,
}

pub(crate) struct TurnOrchestrator {
    options: HarnessTurnOptions,
    pub ports: HarnessPorts,
    driver: Arc<dyn AgentDriverPort>,
    knowledge: Arc<crate::KnowledgeService>,
    pub session: HarnessSessionRecord,
    pub turn_id: HarnessTurnId,
    pub writer: EventWriter,
    pub skill: SkillPackage,
    pub cancellation: CancellationToken,
    pub access: Arc<RwLock<()>>,
    manager_run_id: AgentRunId,
    tasks: Mutex<BTreeMap<String, TaskEntry>>,
    slots: Semaphore,
}

impl TurnOrchestrator {
    fn check_task_mode(&self, scope: &AgentTaskScope) -> Result<(), CapabilityFailure> {
        if self.options.mode == HarnessMode::Ask && !scope.is_read_only() {
            return Err(rejected("ask_mode_read_only"));
        }
        Ok(())
    }
    #[allow(
        clippy::too_many_arguments,
        reason = "one user-turn authority and its injected services"
    )]
    pub(crate) fn new(
        ports: HarnessPorts,
        driver: Arc<dyn AgentDriverPort>,
        knowledge: Arc<crate::KnowledgeService>,
        session: HarnessSessionRecord,
        turn_id: HarnessTurnId,
        writer: EventWriter,
        skill: SkillPackage,
        cancellation: CancellationToken,
        access: Arc<RwLock<()>>,
        options: HarnessTurnOptions,
    ) -> Result<Arc<Self>, AgentDriverFailure> {
        let manager_run_id = AgentRunId::try_new(
            ports
                .ids
                .next_id(AutomationIdKind::AgentRun)
                .map_err(|_| driver_failure())?,
        )
        .map_err(|_| driver_failure())?;
        Ok(Arc::new(Self {
            options,
            ports,
            driver,
            knowledge,
            session,
            turn_id,
            writer,
            skill,
            cancellation,
            access,
            manager_run_id,
            tasks: Mutex::new(BTreeMap::new()),
            slots: Semaphore::new(4),
        }))
    }

    pub(crate) async fn run(
        self: &Arc<Self>,
        mut messages: Vec<AgentMessage>,
        observations: ResourceObservations,
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
        let registry = ToolRegistry::for_agent(AgentRole::Manager).with_mode(self.options.mode);
        let resources_requiring_read = observations.resources_requiring_read();
        if !resources_requiring_read.is_empty() {
            messages.insert(0, AgentMessage::System {
                content: serde_json::json!({
                    "reason": "resource_read_required",
                    "resourceIds": resources_requiring_read,
                    "nextStep": "Read the listed resources before relying on historical facts for a new or resumed task. Their current contents have not been verified; do not assume they have changed."
                }).to_string(),
            });
        }
        let request = self.request(AgentRole::Manager, messages, registry.descriptors());
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
            observations: Mutex::new(observations),
        });
        let mut result = self
            .driver
            .run_turn(request, executor.clone(), output.clone(), self.cancellation.clone())
            .instrument(tracing::info_span!("harness_agent_run", session_id = %self.session.id, turn_id = %self.turn_id, agent_run_id = %self.manager_run_id, role = "manager"))
            .await;
        if self.cancellation.is_cancelled() {
            result = Err(cancelled_driver());
        }
        let mut outcome = finish_outcome(
            self.manager_run_id.clone(),
            AgentRole::Manager,
            &result,
            &evidence,
        );
        if result.is_ok()
            && let Some(reason) = executor.completion_feedback()
        {
            outcome.state = AgentRunState::Blocked;
            output
                .emit(AgentEvent::DeliveryBlocked { reason })
                .await
                .map_err(|_| driver_failure())?;
        }
        self.append(HarnessEvent::AgentRunFinished {
            outcome: Box::new(outcome),
        })
        .await
        .map_err(|_| driver_failure())?;
        result
    }

    fn request(
        &self,
        role: AgentRole,
        mut messages: Vec<AgentMessage>,
        tools: Vec<ToolDescriptor>,
    ) -> AgentTurnRequest {
        let definition = crate::agent_definition(role);
        if self.options.mode == HarnessMode::Ask {
            messages.insert(0, AgentMessage::System { content: "Ask mode: answer and inspect existing resources only. Do not create, edit, save, export, execute graphs, or request UI actions. Delegate only read-only tasks. Explain when the user needs to switch to Write mode.".into() });
        }
        AgentTurnRequest {
            options: self.options,
            role,
            tool_concurrency: definition.tool_concurrency(),
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
            self.knowledge.clone(),
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
        self.check_task_mode(&task.scope)?;
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
        let evidence = Evidence::for_task(&task);
        self.execute_task(
            run_id,
            task,
            dependencies,
            evidence,
            WorkerContinuation::default(),
        )
        .await
    }

    async fn execute_task(
        self: &Arc<Self>,
        run_id: AgentRunId,
        task: AgentTask,
        dependencies: Vec<AgentTaskOutcome>,
        evidence: Evidence,
        continuation: WorkerContinuation,
    ) -> Result<AgentTaskOutcome, CapabilityFailure> {
        let evidence = Arc::new(Mutex::new(evidence));
        let admission = async {
            let slot = tokio::select! {
                permit = self.slots.acquire() => permit.map_err(|_| driver_failure())?,
                _ = self.cancellation.cancelled() => return Err(cancelled_driver()),
            };
            // Keep task settlement inside the same gate as its business operations.
            // A queued reader must observe committed invalidations before starting.
            let (read, write) = if task.scope.is_read_only() {
                let guard = tokio::select! {
                    guard = self.access.read() => guard,
                    _ = self.cancellation.cancelled() => return Err(cancelled_driver()),
                };
                (Some(guard), None)
            } else {
                let guard = tokio::select! {
                    guard = self.access.write() => guard,
                    _ = self.cancellation.cancelled() => return Err(cancelled_driver()),
                };
                (None, Some(guard))
            };
            Ok((slot, read, write))
        }
        .await;
        let (result, _admission) = match admission {
            Ok(admission) => (
                self.run_worker(&run_id, &task, dependencies, evidence.clone(), continuation)
                    .await,
                Some(admission),
            ),
            Err(error) => (Err(error), None),
        };
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
        continuation: WorkerContinuation,
    ) -> Result<AgentTurnResult, AgentDriverFailure> {
        let dependencies_current = {
            let tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
            dependencies.iter().all(|dependency| {
                tasks.values().any(|entry| {
                    entry.run_id == dependency.run_id
                        && entry
                            .outcome
                            .as_ref()
                            .is_some_and(|outcome| outcome.state == AgentRunState::Completed)
                })
            })
        };
        if !dependencies_current {
            evidence
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .blocked_reason = Some("dependency_not_completed".into());
            return Ok(AgentTurnResult {
                final_text: "A task dependency changed before execution. Refresh stale dependencies before delegating a new task.".into(),
            });
        }
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
        let registry = ToolRegistry::for_agent(task.worker).with_mode(self.options.mode);
        let tools = registry.descriptors();
        let executor = Arc::new(RunExecutor {
            tools: self.executor(
                registry,
                scope.clone(),
                output.clone(),
                cancellation.clone(),
            ),
            scope,
            evidence: evidence.clone(),
            manager: None,
            observations: Mutex::new(continuation.observations),
        });
        let input = serde_json::to_string(&serde_json::json!({
            "task": model::AgentTaskInput::from(task),
            "dependencies": dependencies.iter().map(model::task_outcome).collect::<Vec<_>>(),
        }))
        .map_err(|_| driver_failure())?;
        let messages = agent_messages(input, continuation.history, &[], &self.skill, task.worker);
        let request = self.request(task.worker, messages, tools);
        let work = async {
            for access in &task.scope.resources {
                if let Some(version) = &access.version {
                    let outcome = executor
                        .execute_tool(AutomationCapabilityRequest::InspectResource(
                            InspectResourceRequest {
                                resource: access.resource.clone(),
                            },
                        ))
                        .await;
                    let outcome = match outcome {
                        Ok(outcome) => outcome,
                        Err(failure) => {
                            if failure.code == CapabilityFailureCode::Cancelled {
                                return Err(cancelled_driver());
                            }
                            evidence
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .blocked_reason = Some(
                                if failure.code == CapabilityFailureCode::RevisionConflict {
                                    "input_changed"
                                } else {
                                    "input_inspection_failed"
                                }
                                .into(),
                            );
                            return Ok(AgentTurnResult {
                                final_text: serde_json::json!({
                                    "reason": "input_inspection_failed", "resource": access.resource,
                                    "failure": model::failure(&failure),
                                    "nextStep": "Inspect the current resource and correct the task before retrying."
                                }).to_string(),
                            });
                        }
                    };
                    if !matches!(outcome.result, AutomationCapabilityResult::ResourceInspection(ref value) if crate::agents::resource_version_matches(access.resource.kind, version, &value.version))
                    {
                        evidence
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .blocked_reason = Some("input_changed".into());
                        return Ok(AgentTurnResult {
                            final_text: serde_json::json!({
                                "reason": "input_changed", "resource": access.resource,
                                "nextStep": "Inspect the changed resource, reassess the task, then resume the worker."
                            }).to_string(),
                        });
                    }
                }
            }
            self.driver
                .run_turn(request, executor, output, cancellation.clone())
                .instrument(tracing::info_span!("harness_agent_run", session_id = %self.session.id, turn_id = %self.turn_id, agent_run_id = %run_id, role = task.worker.name()))
                .await
        };
        tokio::pin!(work);
        tokio::select! {
            result = &mut work => result,
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
    observations: Mutex<ResourceObservations>,
}

impl RunExecutor {
    async fn execute_tool(
        &self,
        request: AutomationCapabilityRequest,
    ) -> Result<ModelCapabilityOutcome, CapabilityFailure> {
        self.execute_tool_started(request, self.tools.now()).await
    }

    async fn execute_tool_started(
        &self,
        request: AutomationCapabilityRequest,
        started_at: UnixMillis,
    ) -> Result<ModelCapabilityOutcome, CapabilityFailure> {
        let graph_observation = if self.manager.is_none() {
            self.graph_observation(&request)
        } else {
            None
        };
        let outcome = match self
            .tools
            .execute_started(request.clone(), graph_observation, started_at)
            .await
        {
            Ok(outcome) => outcome,
            Err(failure) => {
                if self.manager.is_none()
                    && matches!(
                        failure.code,
                        CapabilityFailureCode::RevisionConflict
                            | CapabilityFailureCode::GraphDraftChanged
                    )
                {
                    self.evidence
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .blocked_reason = Some("input_changed".into());
                }
                return Err(failure);
            }
        };
        let mut evidence = self.evidence.lock().unwrap_or_else(|e| e.into_inner());
        if !evidence.invocations.contains(&outcome.invocation_id) {
            evidence.invocations.push(outcome.invocation_id.clone());
            record_receipt(
                &request,
                &outcome.result,
                &mut evidence,
                &mut self.scope.lock().unwrap_or_else(|e| e.into_inner()),
            );
        }
        Ok(outcome)
    }
}

impl ModelCapabilityExecutor for RunExecutor {
    fn begin_control<'a>(
        &'a self,
        tool: AgentControlTool,
    ) -> AgentFuture<'a, Result<ToolInvocationId, CapabilityFailure>> {
        Box::pin(self.tools.begin_control(tool))
    }

    fn finish_control<'a>(
        &'a self,
        invocation_id: ToolInvocationId,
        tool: AgentControlTool,
        failure: Option<CapabilityFailure>,
    ) -> AgentFuture<'a, Result<(), CapabilityFailure>> {
        Box::pin(self.tools.finish_control(invocation_id, tool, failure))
    }

    fn completion_feedback(&self) -> Option<String> {
        if let Some(manager) = &self.manager {
            let tasks = manager.tasks.lock().unwrap_or_else(|e| e.into_inner());
            let pending: Vec<_> = tasks.values().filter(|entry| {
                Evidence::for_task(&entry.task).delivery_pending()
                    && entry.outcome.as_ref().is_none_or(|outcome| outcome.state != AgentRunState::Completed)
            }).map(|entry| serde_json::json!({
                "runId": entry.run_id,
                "state": entry.outcome.as_ref().map(|outcome| outcome.state),
                "failureCode": entry.outcome.as_ref().and_then(|outcome| outcome.failure_code),
                "artifactCount": entry.outcome.as_ref().map_or(0, |outcome| outcome.artifacts.len()),
            })).collect();
            return (!pending.is_empty()).then(|| serde_json::json!({
                "reason": "report_delivery_incomplete", "pending": pending,
                "nextStep": "Use followup_task to continue these workers from their receipts and finish saving the requested documents. Inspect changed inputs and reassess the task before resuming. Do not claim delivery without successful saves. If an external requirement prevents completion, explain the actual blocker."
            }).to_string());
        }
        self.evidence
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .delivery_feedback()
    }
    fn execute<'a>(
        &'a self,
        request: ModelCapabilityRequest,
    ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>> {
        Box::pin(async move {
            let started_at = self.tools.now();
            let input = request.request;
            let request =
                match crate::tools::contain_tool_failure(self.bind_input(input.clone())).await {
                    Ok(request) => request,
                    Err(failure) => {
                        return self
                            .tools
                            .reject(input.capability_id(), Some(input), failure, started_at)
                            .await;
                    }
                };
            let outcome = self.execute_tool_started(request, started_at).await?;
            self.observations
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .record(&outcome.result);
            Ok(outcome)
        })
    }

    fn reject_arguments<'a>(
        &'a self,
        capability_id: CapabilityId,
        failure: CapabilityFailure,
    ) -> AgentFuture<'a, Result<ModelCapabilityOutcome, CapabilityFailure>> {
        Box::pin(async move {
            self.tools
                .reject(capability_id, None, failure, self.tools.now())
                .await
        })
    }

    fn followup<'a>(
        &'a self,
        request: model::AgentFollowupInput,
    ) -> AgentFuture<'a, Result<AgentTaskOutcome, CapabilityFailure>> {
        Box::pin(async move {
            let outcome = self
                .manager
                .as_ref()
                .ok_or_else(|| rejected("workers_cannot_delegate"))?
                .followup(request, self)
                .await?;
            self.observations
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .invalidate(&outcome.artifacts);
            Ok(outcome)
        })
    }

    fn delegate<'a>(
        &'a self,
        task: model::AgentTaskInput,
    ) -> AgentFuture<'a, Result<AgentTaskOutcome, CapabilityFailure>> {
        Box::pin(async move { self.delegate_input(task).await })
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

mod capabilities;
mod continuation;
use continuation::WorkerContinuation;
mod delegation;
mod observations;
mod receipts;
mod recovery;
pub(crate) use observations::ResourceObservations;
use receipts::{Evidence, finish_outcome, record_receipt};
pub(crate) use recovery::{pending_agent_turns, recover_runs};

#[cfg(test)]
mod tests;
