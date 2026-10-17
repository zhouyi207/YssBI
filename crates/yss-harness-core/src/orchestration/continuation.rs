//! Resume one durable worker within its original authority, preserving committed receipts.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

impl TurnOrchestrator {
    pub(super) async fn followup(
        self: &Arc<Self>,
        input: AgentFollowupInput,
        executor: &RunExecutor,
    ) -> Result<AgentTaskOutcome, CapabilityFailure> {
        let mut request = AgentFollowup {
            run_id: input.run_id,
            instruction: input.instruction,
            bindings: Value::Null,
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
        self.check_task_mode(&task.scope)?;
        if !outcomes.contains_key(&request.run_id) {
            return Err(rejected("task_still_running"));
        }
        let hooks = self.ports.extension.start_run(RunStart {
            run_id: request.run_id.clone(),
            role: task.worker.clone(),
            task_scope: Some(task.scope.clone()),
            project: self.session.project.clone(),
        });
        let mut evidence = RunEvidence::default();
        let mut history = Vec::new();
        let mut compaction_checkpoint = None;
        let mut seen = BTreeSet::new();
        for envelope in events {
            let event = match envelope.event {
                HarnessEvent::AgentRunResumed {
                    request: previous,
                    scope,
                    ..
                } if previous.run_id == request.run_id => {
                    hooks.resume(&scope);
                    history.push(AgentMessage::User {
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
                    history.clear();
                    history.push(AgentMessage::Assistant {
                        content: format!("[Continuation checkpoint]\n{summary}"),
                    });
                }
                AgentEvent::TextDelta { delta } => {
                    crate::conversation::assistant_text(&mut history, delta)
                }
                AgentEvent::TextRetracted { characters } => {
                    crate::conversation::retract_text(&mut history, characters)
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
                    hooks.replay(&record, &self.session.project);
                    if record.state == ToolInvocationState::Running
                        || record.failure.as_ref().is_some_and(|failure| {
                            failure.code == CapabilityFailureCode::OUTCOME_UNKNOWN
                        })
                    {
                        return Err(
                            CapabilityFailure::new(CapabilityFailureCode::OUTCOME_UNKNOWN)
                                .with_detail("invocationId", invocation_id.to_string()),
                        );
                    }
                    evidence.invocations.push(invocation_id);
                    history.push(crate::conversation::tool_call(&record));
                    history.push(
                        crate::conversation::tool_result(self.ports.extension.as_ref(), record)
                            .map_err(|_| persistence_failure())?,
                    );
                }
                _ => {}
            }
        }
        let (scope, bindings) = executor.hooks.resume_task(hooks.as_ref(), executor).await?;
        task.scope = scope;
        request.bindings = bindings;
        history.push(AgentMessage::User {
            content: request.instruction.clone(),
        });
        if let Some(checkpoint) = compaction_checkpoint {
            history.push(AgentMessage::CompactionCheckpoint { checkpoint });
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
            role: task.worker.clone(),
            objective: task.objective.clone(),
        })
        .await?;
        self.execute_task(request.run_id, task, dependencies, evidence, hooks, history)
            .await
    }
}
