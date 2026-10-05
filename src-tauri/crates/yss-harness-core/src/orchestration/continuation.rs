//! Resume one durable worker within its original authority, preserving committed receipts.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub(super) struct WorkerContinuation {
    pub history: Vec<AgentMessage>,
    pub observations: ResourceObservations,
}

impl TurnOrchestrator {
    pub(super) async fn followup(
        self: &Arc<Self>,
        input: model::AgentFollowupInput,
        executor: &RunExecutor,
    ) -> Result<AgentTaskOutcome, CapabilityFailure> {
        let mut request = AgentFollowup {
            run_id: input.run_id,
            instruction: input.instruction,
            resource_versions: Vec::new(),
        };
        if request.instruction.trim().is_empty() {
            return Err(rejected("instruction_required"));
        }
        if self.cancellation.is_cancelled() {
            return Err(cancelled());
        }
        let events = self
            .ports
            .events
            .load_events_after(&self.session.id, 0)
            .await
            .map_err(|_| persistence_failure())?;
        let mut definitions = BTreeMap::new();
        let mut outcomes = BTreeMap::new();
        for event in &events {
            match &event.event {
                HarnessEvent::AgentRunStarted {
                    run_id,
                    task: Some(task),
                    ..
                } => {
                    definitions.insert(run_id.clone(), (**task).clone());
                }
                HarnessEvent::AgentRunFinished { outcome } => {
                    outcomes.insert(outcome.run_id.clone(), (**outcome).clone());
                }
                HarnessEvent::AgentRunResumed { request, .. } => {
                    outcomes.remove(&request.run_id);
                }
                HarnessEvent::AgentRunInvalidated { run_id } => {
                    if let Some(outcome) = outcomes.get_mut(run_id) {
                        outcome.state = AgentRunState::Stale;
                    }
                }
                _ => {}
            }
        }
        let mut task = definitions
            .get(&request.run_id)
            .cloned()
            .ok_or_else(|| rejected("worker_not_found"))?;
        if !outcomes.contains_key(&request.run_id) {
            return Err(rejected("task_still_running"));
        }
        let mut evidence = Evidence::for_task(&task);
        let mut scope = AgentInvocationScope {
            run_id: request.run_id.clone(),
            role: task.worker,
            task: Some(task.scope.clone()),
        };
        let mut continuation = WorkerContinuation::default();
        let mut compaction_checkpoint = None;
        let mut seen = BTreeSet::new();
        let mut previous_project_session = false;
        for envelope in events {
            let event = match envelope.event {
                HarnessEvent::AgentRunResumed {
                    request: previous,
                    scope: previous_scope,
                    ..
                } if previous.run_id == request.run_id => {
                    scope.task = Some(previous_scope);
                    continuation.history.push(AgentMessage::User {
                        content: previous.instruction,
                    });
                    continue;
                }
                HarnessEvent::AgentRunOutput { run_id, event } if run_id == request.run_id => event,
                _ => continue,
            };
            match event {
                AgentEvent::ContextCompactionProgress {
                    checkpoint: Some(checkpoint),
                    ..
                } => compaction_checkpoint = Some(checkpoint),
                AgentEvent::ContextCompacted { summary } => {
                    compaction_checkpoint = None;
                    continuation.history.clear();
                    continuation.history.push(AgentMessage::Assistant {
                        content: format!("[Continuation checkpoint]\n{summary}"),
                    });
                }
                AgentEvent::TextDelta { delta } => {
                    crate::conversation::assistant_text(&mut continuation.history, delta)
                }
                AgentEvent::TextRetracted { characters } => {
                    crate::conversation::retract_text(&mut continuation.history, characters)
                }
                AgentEvent::PlanProposed { plan } => {
                    evidence.plan = Some(plan.clone());
                    continuation.history.push(AgentMessage::Plan { plan });
                }
                AgentEvent::ToolInvocationStarted { invocation_id, .. }
                | AgentEvent::ToolInvocationCompleted { invocation_id, .. }
                | AgentEvent::ToolInvocationFailed { invocation_id, .. }
                    if seen.insert(invocation_id.clone()) =>
                {
                    let record = self
                        .ports
                        .tool_ledger
                        .load_invocation(&self.session.id, &invocation_id)
                        .await
                        .map_err(|_| persistence_failure())?
                        .ok_or_else(persistence_failure)?;
                    if record.agent_run_id.as_ref() != Some(&request.run_id) {
                        return Err(persistence_failure());
                    }
                    previous_project_session |= record.project != self.session.project;
                    continuation
                        .observations
                        .replay(&record, &self.session.project);
                    if record.state == ToolInvocationState::Running
                        || record.failure.as_ref().is_some_and(|failure| {
                            failure.code == CapabilityFailureCode::OutcomeUnknown
                        })
                    {
                        return Err(
                            CapabilityFailure::new(CapabilityFailureCode::OutcomeUnknown)
                                .with_detail("invocationId", invocation_id.to_string()),
                        );
                    }
                    evidence.invocations.push(invocation_id);
                    if let Some(result) = &record.result {
                        record_receipt(&record.request, result, &mut evidence, &mut scope);
                    }
                    continuation
                        .history
                        .push(crate::conversation::tool_call(&record));
                    continuation.history.push(
                        crate::conversation::tool_result(record)
                            .map_err(|_| persistence_failure())?,
                    );
                }
                _ => {}
            }
        }
        task.scope = scope.task.ok_or_else(persistence_failure)?;
        for access in &mut task.scope.resources {
            if let Some(version) = executor
                .task_resource_version(&access.resource, false)
                .await?
            {
                continuation.observations.refresh_from(
                    &executor
                        .observations
                        .lock()
                        .unwrap_or_else(|e| e.into_inner()),
                    &access.resource,
                );
                access.version = Some(version.clone());
                request.resource_versions.push(AgentResourceVersion {
                    resource: access.resource.clone(),
                    version,
                });
            } else if previous_project_session {
                return Err(CapabilityFailure::new(CapabilityFailureCode::RevisionConflict)
                    .with_detail("reason", "resource_requires_current_read")
                    .with_detail("resourceId", &access.resource.id)
                    .with_detail("nextStep", "Inspect the resource in the current project session before resuming this worker."));
            }
        }
        continuation.history.push(AgentMessage::User {
            content: request.instruction.clone(),
        });
        if let Some(checkpoint) = compaction_checkpoint {
            continuation
                .history
                .push(AgentMessage::CompactionCheckpoint { checkpoint });
        }
        let dependencies = task
            .depends_on
            .iter()
            .map(|id| {
                outcomes
                    .get(id)
                    .filter(|outcome| outcome.state == AgentRunState::Completed)
                    .cloned()
                    .ok_or_else(|| rejected("dependency_not_completed"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Keys belong to a user turn; resumed runs retain their durable run ID across turns.
        task.key = format!("resume:{}", request.run_id);
        {
            let mut tasks = self.tasks.lock().unwrap_or_else(|e| e.into_inner());
            if tasks
                .values()
                .any(|entry| entry.run_id == request.run_id && entry.outcome.is_none())
            {
                return Err(rejected("task_still_running"));
            }
            for dependency in &dependencies {
                if !tasks
                    .values()
                    .any(|entry| entry.run_id == dependency.run_id)
                {
                    let definition = definitions
                        .get(&dependency.run_id)
                        .ok_or_else(persistence_failure)?
                        .clone();
                    tasks.insert(
                        format!("history:{}", dependency.run_id),
                        TaskEntry {
                            task: definition,
                            run_id: dependency.run_id.clone(),
                            outcome: Some(dependency.clone()),
                        },
                    );
                }
            }
            tasks.retain(|_, entry| entry.run_id != request.run_id);
            tasks.insert(
                task.key.clone(),
                TaskEntry {
                    task: task.clone(),
                    run_id: request.run_id.clone(),
                    outcome: None,
                },
            );
        }
        self.append(HarnessEvent::AgentRunResumed {
            request: request.clone(),
            scope: task.scope.clone(),
            role: task.worker,
            objective: task.objective.clone(),
        })
        .await?;
        self.execute_task(request.run_id, task, dependencies, evidence, continuation)
            .await
    }
}
