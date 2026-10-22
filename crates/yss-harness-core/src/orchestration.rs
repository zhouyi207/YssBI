use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use tokio::sync::{RwLock, Semaphore};
use tracing::Instrument;
use yss_harness_contract::*;

use crate::HarnessPorts;
use crate::events::{EventWriter, PersistingAgentOutput};
use crate::tools::{
    HarnessToolExecutor, InvocationScope, ToolRegistry, ToolServices, contain_tool_failure,
};

struct TaskEntry {
    task: AgentTask,
    run_id: AgentRunId,
    outcome: Option<AgentTaskOutcome>,
}

/// Harness-owned facts of one run. Extension facts live in its hooks.
#[derive(Default)]
pub(crate) struct RunEvidence {
    invocations: Vec<ToolInvocationId>,
    blocked_reason: Option<String>,
}

pub(crate) struct TurnOrchestrator {
    options: HarnessTurnOptions,
    pub ports: HarnessPorts,
    driver: Arc<dyn AgentDriverPort>,
    knowledge: Arc<crate::KnowledgeService>,
    approvals: Arc<crate::approvals::Approvals>,
    pub session: HarnessSessionRecord,
    pub turn_id: HarnessTurnId,
    pub writer: EventWriter,
    pub cancellation: CancellationToken,
    pub access: Arc<RwLock<()>>,
    root_role: AgentRoleId,
    manager_run_id: AgentRunId,
    tasks: Mutex<BTreeMap<String, TaskEntry>>,
    slots: Semaphore,
}

impl TurnOrchestrator {
    fn check_task_mode(&self, scope: &Value) -> Result<(), CapabilityFailure> {
        if self.options.mode == HarnessMode::Ask && !self.ports.extension.scope_is_read_only(scope)
        {
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
        approvals: Arc<crate::approvals::Approvals>,
        session: HarnessSessionRecord,
        turn_id: HarnessTurnId,
        writer: EventWriter,
        cancellation: CancellationToken,
        access: Arc<RwLock<()>>,
        options: HarnessTurnOptions,
    ) -> Result<Arc<Self>, AgentDriverFailure> {
        let manager_run_id = AgentRunId::try_new(ports.ids.next_id(AutomationIdKind::AgentRun))
            .map_err(|_| driver_failure())?;
        let root_role = ports.extension.root_role();
        Ok(Arc::new(Self {
            options,
            ports,
            driver,
            knowledge,
            approvals,
            session,
            turn_id,
            writer,
            cancellation,
            access,
            root_role,
            manager_run_id,
            tasks: Mutex::new(BTreeMap::new()),
            slots: Semaphore::new(4),
        }))
    }

    pub(crate) fn root_role(&self) -> &AgentRoleId {
        &self.root_role
    }

    /// The root run's hooks. The Host replays the session history into them before the run.
    pub(crate) fn root_hooks(&self) -> Arc<dyn AgentRunHooks> {
        self.ports.extension.start_run(RunStart {
            run_id: self.manager_run_id.clone(),
            role: self.root_role.clone(),
            task_scope: None,
            project: self.session.project.clone(),
        })
    }

    pub(crate) async fn run(
        self: &Arc<Self>,
        mut messages: Vec<AgentMessage>,
        hooks: Arc<dyn AgentRunHooks>,
    ) -> Result<AgentTurnResult, AgentDriverFailure> {
        self.append(HarnessEvent::AgentRunStarted {
            run_id: self.manager_run_id.clone(),
            parent_run_id: None,
            role: self.root_role.clone(),
            task: None,
        })
        .await
        .map_err(|_| driver_failure())?;
        let profile = self.profile(&self.root_role)?;
        let evidence = Arc::new(Mutex::new(RunEvidence::default()));
        let output = Arc::new(PersistingAgentOutput {
            writer: self.writer.clone(),
            session_id: self.session.id.clone(),
            turn_id: self.turn_id.clone(),
            run_id: None,
        });
        for content in hooks.context_messages().into_iter().rev() {
            messages.insert(0, AgentMessage::System { content });
        }
        let registry = ToolRegistry::new(profile.tools.clone()).with_mode(self.options.mode);
        let executor = Arc::new(RunExecutor {
            tools: self.executor(
                registry,
                &self.manager_run_id,
                hooks.clone(),
                output.clone(),
                self.cancellation.clone(),
            ),
            hooks: hooks.clone(),
            extension: self.ports.extension.clone(),
            evidence: evidence.clone(),
            manager: Some(self.clone()),
        });
        let request = self.request(&profile, messages, executor.tools.registry());
        let mut result = self
            .driver
            .run_turn(request, executor.clone(), output.clone(), self.cancellation.clone())
            .instrument(tracing::info_span!("harness_agent_run", session_id = %self.session.id, turn_id = %self.turn_id, agent_run_id = %self.manager_run_id, role = %self.root_role))
            .await;
        if self.cancellation.is_cancelled() {
            result = Err(cancelled_driver());
        }
        let mut outcome = finish_outcome(
            self.manager_run_id.clone(),
            self.root_role.clone(),
            false,
            &result,
            &evidence,
            hooks.as_ref(),
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

    fn profile(&self, role: &AgentRoleId) -> Result<AgentProfile, AgentDriverFailure> {
        self.ports
            .extension
            .profile(role)
            .ok_or_else(driver_failure)
    }

    /// System context, prior history and the new user content for one role.
    pub(crate) fn agent_messages(
        &self,
        role: &AgentRoleId,
        user_message: String,
        previous: Vec<AgentMessage>,
        resources: &[HarnessResourceReference],
    ) -> Result<Vec<AgentMessage>, AgentDriverFailure> {
        let profile = self.profile(role)?;
        let mut messages: Vec<_> = profile
            .instructions
            .into_iter()
            .map(|content| AgentMessage::System { content })
            .collect();
        if role != &self.root_role {
            messages.push(AgentMessage::System {
                content: "Return a concise final message describing the work completed, evidence, artifacts and any blockers or remaining work. Plain text and Markdown are accepted; no JSON wrapper is required. Tool receipts establish actual effects. The Manager must assess whether your task objective was achieved before relying on the result.".into(),
            });
        }
        messages.extend(previous);
        messages.push(AgentMessage::User {
            content: crate::conversation::user_input(user_message, resources),
        });
        Ok(messages)
    }

    fn request(
        &self,
        profile: &AgentProfile,
        mut messages: Vec<AgentMessage>,
        registry: &ToolRegistry,
    ) -> AgentTurnRequest {
        if let Some(content) = self.ports.extension.mode_instructions(self.options.mode) {
            messages.insert(0, AgentMessage::System { content });
        }
        let mut tools = registry.specs();
        if profile.delegates {
            tools.extend(self.control_specs());
        }
        AgentTurnRequest {
            options: self.options,
            role: profile.role.clone(),
            tool_concurrency: profile.tool_concurrency,
            output_mode: profile.output_mode,
            messages,
            tools,
        }
    }

    fn control_specs(&self) -> [ToolSpec; 2] {
        [
            ToolSpec::new(
                DELEGATE_TASK_TOOL,
                "Delegate a task with precise resources, permissions and completion criteria. Always provide worker, objective, constraints (a string), completionCriteria, dependsOn and scope as top-level fields. The host captures read observations and deduplicates identical task specifications within this turn. Use agent_continue_task to continue a previous worker.",
                self.ports.extension.delegate_task_schema(),
                ToolEffect::Inspect,
            ),
            ToolSpec::new(
                FOLLOWUP_TASK_TOOL,
                "Continue an existing worker by runId, keeping its history, checkpoints and committed receipts. Inspect changed inputs before resuming; the host binds observed facts within the original grant. Use after partial completion or interruption instead of creating a fresh worker.",
                followup_task_schema(),
                ToolEffect::Inspect,
            ),
        ]
    }

    fn executor(
        &self,
        registry: ToolRegistry,
        run_id: &AgentRunId,
        hooks: Arc<dyn AgentRunHooks>,
        output: Arc<dyn AgentEventOutput>,
        cancellation: CancellationToken,
    ) -> HarnessToolExecutor {
        HarnessToolExecutor::new(
            registry,
            ToolServices {
                extension: self.ports.extension.clone(),
                knowledge: self.knowledge.clone(),
                web: self.ports.web.clone(),
                python: self.ports.python.clone(),
                approvals: self.approvals.clone(),
                ledger: self.ports.tool_ledger.clone(),
                clock: self.ports.clock.clone(),
                ids: self.ports.ids.clone(),
            },
            InvocationScope {
                principal_id: self.session.principal_id.clone(),
                session_id: self.session.id.clone(),
                turn_id: self.turn_id.clone(),
                project: self.session.project.clone(),
            },
            cancellation,
        )
        .with_run(run_id.clone(), hooks)
        .with_output(output)
    }

    async fn append(&self, event: HarnessEvent) -> Result<(), CapabilityFailure> {
        self.writer
            .append(&self.session.id, Some(&self.turn_id), event)
            .await
            .map_err(|_| persistence_failure())
    }

    fn validate_task(&self, task: &AgentTask) -> Result<(), CapabilityFailure> {
        if task.worker == self.root_role
            || !task.has_valid_text()
            || self.ports.extension.profile(&task.worker).is_none()
        {
            return Err(rejected("invalid field 'task'"));
        }
        self.ports.extension.validate_task(task)
    }

    async fn delegate(
        self: &Arc<Self>,
        task: AgentTask,
    ) -> Result<AgentTaskOutcome, CapabilityFailure> {
        self.check_task_mode(&task.scope)?;
        self.validate_task(&task)?;
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
            let run_id = AgentRunId::try_new(self.ports.ids.next_id(AutomationIdKind::AgentRun))
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
            role: task.worker.clone(),
            task: Some(Box::new(task.clone())),
        })
        .await?;
        let hooks = self.ports.extension.start_run(RunStart {
            run_id: run_id.clone(),
            role: task.worker.clone(),
            task_scope: Some(task.scope.clone()),
            project: self.session.project.clone(),
        });
        self.execute_task(
            run_id,
            task,
            dependencies,
            RunEvidence::default(),
            hooks,
            Vec::new(),
        )
        .await
    }

    async fn execute_task(
        self: &Arc<Self>,
        run_id: AgentRunId,
        task: AgentTask,
        dependencies: Vec<AgentTaskOutcome>,
        evidence: RunEvidence,
        hooks: Arc<dyn AgentRunHooks>,
        history: Vec<AgentMessage>,
    ) -> Result<AgentTaskOutcome, CapabilityFailure> {
        let evidence = Arc::new(Mutex::new(evidence));
        let read_only = self.ports.extension.scope_is_read_only(&task.scope);
        let admission = async {
            let slot = tokio::select! {
                permit = self.slots.acquire() => permit.map_err(|_| driver_failure())?,
                _ = self.cancellation.cancelled() => return Err(cancelled_driver()),
            };
            // Keep task settlement inside the same gate as its business operations.
            // A queued reader must observe committed invalidations before starting.
            let (read, write) = if read_only {
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
                self.run_worker(
                    &run_id,
                    &task,
                    dependencies,
                    evidence.clone(),
                    hooks.clone(),
                    history,
                )
                .await,
                Some(admission),
            ),
            Err(error) => (Err(error), None),
        };
        let mut outcome = finish_outcome(
            run_id,
            task.worker.clone(),
            true,
            &result,
            &evidence,
            hooks.as_ref(),
        );
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
                if self
                    .ports
                    .extension
                    .task_invalidated(&entry.task.scope, &outcome.facts)
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
        evidence: Arc<Mutex<RunEvidence>>,
        hooks: Arc<dyn AgentRunHooks>,
        history: Vec<AgentMessage>,
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
        let profile = self.profile(&task.worker)?;
        let cancellation = CancellationToken::default();
        let output = Arc::new(WorkerOutput {
            inner: PersistingAgentOutput {
                writer: self.writer.clone(),
                session_id: self.session.id.clone(),
                turn_id: self.turn_id.clone(),
                run_id: Some(run_id.clone()),
            },
            evidence: evidence.clone(),
        });
        let registry = ToolRegistry::new(profile.tools.clone()).with_mode(self.options.mode);
        let executor = Arc::new(RunExecutor {
            tools: self.executor(
                registry,
                run_id,
                hooks.clone(),
                output.clone(),
                cancellation.clone(),
            ),
            hooks: hooks.clone(),
            extension: self.ports.extension.clone(),
            evidence,
            manager: None,
        });
        let input = serde_json::to_string(&serde_json::json!({
            "task": task_input(self.ports.extension.as_ref(), task),
            "dependencies": dependencies
                .iter()
                .map(|outcome| task_outcome(self.ports.extension.as_ref(), outcome))
                .collect::<Vec<_>>(),
        }))
        .map_err(|_| driver_failure())?;
        let messages = self.agent_messages(&task.worker, input, history, &[])?;
        let request = self.request(&profile, messages, executor.tools.registry());
        let work = async {
            if let Some(final_text) = hooks.before_run(executor.as_ref()).await? {
                return Ok(AgentTurnResult { final_text });
            }
            self.driver
                .run_turn(request, executor, output, cancellation.clone())
                .instrument(tracing::info_span!("harness_agent_run", session_id = %self.session.id, turn_id = %self.turn_id, agent_run_id = %run_id, role = %task.worker))
                .await
        };
        tokio::pin!(work);
        tokio::select! {
            result = &mut work => result,
            _ = self.cancellation.cancelled() => {
                cancellation.cancel(self.cancellation.reason().unwrap_or(CancellationReason::User));
                // Await accepted business operations so real commit receipts survive cancellation.
                if let Err(error) = work.await {
                    tracing::debug!(domain = "Harness", event = "harness_cancelled_worker_settled", code = %error.code);
                }
                Err(cancelled_driver())
            }
        }
    }
}

struct WorkerOutput {
    inner: PersistingAgentOutput,
    evidence: Arc<Mutex<RunEvidence>>,
}

impl AgentEventOutput for WorkerOutput {
    fn emit<'a>(&'a self, event: AgentEvent) -> AgentFuture<'a, Result<(), AgentOutputFailure>> {
        Box::pin(async move {
            self.inner.emit(event.clone()).await?;
            if let AgentEvent::ToolInvocationFailed { invocation_id, .. } = event {
                let mut evidence = self.evidence.lock().unwrap_or_else(|e| e.into_inner());
                if !evidence.invocations.contains(&invocation_id) {
                    evidence.invocations.push(invocation_id);
                }
            }
            Ok(())
        })
    }
}

pub(crate) struct RunExecutor {
    tools: HarnessToolExecutor,
    hooks: Arc<dyn AgentRunHooks>,
    extension: Arc<dyn HarnessExtension>,
    evidence: Arc<Mutex<RunEvidence>>,
    manager: Option<Arc<TurnOrchestrator>>,
}

impl RunExecutor {
    /// A run without a root orchestrator, for exercising one extension run in isolation.
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn standalone(
        tools: HarnessToolExecutor,
        hooks: Arc<dyn AgentRunHooks>,
        extension: Arc<dyn HarnessExtension>,
    ) -> Self {
        Self {
            tools,
            hooks,
            extension,
            evidence: Arc::new(Mutex::new(RunEvidence::default())),
            manager: None,
        }
    }
}

impl RunExecutor {
    async fn run_bound(
        &self,
        call: PreparedToolCall,
        started_at: UnixMillis,
    ) -> Result<ToolInvocationOutcome, CapabilityFailure> {
        let outcome = self.tools.execute(call.clone(), started_at).await;
        if let Ok(outcome) = &outcome {
            let mut evidence = self.evidence.lock().unwrap_or_else(|e| e.into_inner());
            if !evidence.invocations.contains(&outcome.invocation_id) {
                evidence.invocations.push(outcome.invocation_id.clone());
            }
        }
        self.hooks.observe(&call, &outcome);
        outcome
    }

    async fn prepare(&self, call: ToolCall) -> Result<PreparedToolCall, RejectedToolCall> {
        if self.tools.registry().get(&call.name).is_none() {
            return Err(
                CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST)
                    .with_detail("capabilityId", &call.name)
                    .into(),
            );
        }
        if is_harness_tool(&call.name) {
            return prepare_harness_tool(call, &self.tools);
        }
        contain_tool_failure(async {
            Ok::<_, CapabilityFailure>(self.hooks.prepare(call, self).await)
        })
        .await
        .map_err(RejectedToolCall::from)?
    }

    async fn call_tool(&self, call: ToolCall) -> ToolCallOutcome {
        let started_at = self.tools.now();
        let tool = call.name.clone();
        let outcome = match self.prepare(call).await {
            Ok(call) => self.run_bound(call, started_at).await,
            Err(rejected) => {
                self.tools
                    .reject(
                        tool.clone(),
                        rejected.arguments,
                        rejected.failure,
                        started_at,
                    )
                    .await
            }
        };
        match outcome {
            Ok(outcome) => match self.extension.project_result(&tool, &outcome.result) {
                Ok(output) => ToolCallOutcome {
                    invocation_id: Some(outcome.invocation_id),
                    output,
                    fatal: None,
                },
                Err(_) => fatal(AgentDriverFailureCode::InternalFailure),
            },
            Err(failure) => self.failed(&failure),
        }
    }

    fn failed(&self, failure: &CapabilityFailure) -> ToolCallOutcome {
        if let Some(code) = failure.code.fatal() {
            return fatal(code);
        }
        ToolCallOutcome {
            invocation_id: None,
            output: failed_output(self.extension.as_ref(), failure),
            fatal: None,
        }
    }

    async fn call_control(&self, call: ToolCall) -> ToolCallOutcome {
        let invocation_id = match self.tools.begin_control(&call.name).await {
            Ok(invocation_id) => invocation_id,
            Err(failure) => return self.failed(&failure),
        };
        let tool = call.name.clone();
        let outcome = contain_tool_failure(self.control(call)).await;
        if let Err(failure) = self
            .tools
            .finish_control(
                invocation_id.clone(),
                &tool,
                outcome.as_ref().err().cloned(),
            )
            .await
        {
            return self.failed(&failure);
        }
        match outcome {
            Ok(output) => ToolCallOutcome {
                invocation_id: Some(invocation_id),
                output,
                fatal: None,
            },
            Err(failure) => self.failed(&failure),
        }
    }

    async fn control(&self, call: ToolCall) -> Result<Value, CapabilityFailure> {
        let manager = self
            .manager
            .as_ref()
            .ok_or_else(|| rejected("workers_cannot_delegate"))?;
        let outcome = match call.name.as_str() {
            DELEGATE_TASK_TOOL => {
                let input = self.extension.decode_task_input(call.arguments)?;
                self.delegate_input(manager, input).await?
            }
            FOLLOWUP_TASK_TOOL => {
                let input: AgentFollowupInput =
                    decode_arguments(call.arguments, &followup_task_schema())?;
                manager.followup(input, self).await?
            }
            _ => return Err(rejected("unknown_control_tool")),
        };
        self.hooks.invalidate(&outcome.facts);
        Ok(task_outcome(self.extension.as_ref(), &outcome))
    }
}

impl ToolRunner for RunExecutor {
    fn run<'a>(
        &'a self,
        call: PreparedToolCall,
    ) -> AgentFuture<'a, Result<ToolInvocationOutcome, CapabilityFailure>> {
        Box::pin(async move { self.run_bound(call, self.tools.now()).await })
    }

    fn new_call_key(&self) -> Result<String, CapabilityFailure> {
        self.tools.new_call_key()
    }
}

impl AgentToolExecutor for RunExecutor {
    fn call<'a>(&'a self, call: ToolCall) -> AgentFuture<'a, ToolCallOutcome> {
        Box::pin(async move {
            if matches!(call.name.as_str(), DELEGATE_TASK_TOOL | FOLLOWUP_TASK_TOOL) {
                self.call_control(call).await
            } else {
                self.call_tool(call).await
            }
        })
    }

    fn summary_projection(&self, tool: &str, output: &Value) -> Option<Value> {
        self.extension.summary_projection(tool, output)
    }

    fn completion_feedback(&self) -> Option<String> {
        let Some(manager) = &self.manager else {
            return self.hooks.completion_feedback(&[]);
        };
        let tasks = manager.tasks.lock().unwrap_or_else(|e| e.into_inner());
        let delegated: Vec<_> = tasks
            .values()
            .map(|entry| DelegatedTask {
                run_id: &entry.run_id,
                task: &entry.task,
                outcome: entry.outcome.as_ref(),
            })
            .collect();
        self.hooks.completion_feedback(&delegated)
    }
}

fn prepare_harness_tool(
    call: ToolCall,
    tools: &HarnessToolExecutor,
) -> Result<PreparedToolCall, RejectedToolCall> {
    let Some(spec) = tools.registry().get(&call.name) else {
        return Err(CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST).into());
    };
    let schema = &spec.input_schema;
    let request = match call.name.as_str() {
        SEARCH_KNOWLEDGE_TOOL => serde_json::to_value(decode_arguments::<SearchKnowledgeRequest>(
            call.arguments,
            schema,
        )?),
        WEB_SEARCH_TOOL => serde_json::to_value(decode_arguments::<WebSearchRequest>(
            call.arguments,
            schema,
        )?),
        WEB_FETCH_TOOL => {
            serde_json::to_value(decode_arguments::<WebFetchRequest>(call.arguments, schema)?)
        }
        PYTHON_INSPECT_ENVIRONMENT_TOOL => serde_json::to_value(decode_arguments::<
            PythonInspectEnvironmentRequest,
        >(call.arguments, schema)?),
        PYTHON_READ_SCRIPT_TOOL => serde_json::to_value(
            decode_arguments::<PythonReadScriptRequest>(call.arguments, schema)?,
        ),
        PYTHON_WRITE_SCRIPT_TOOL => serde_json::to_value(decode_arguments::<
            PythonWriteScriptRequest,
        >(call.arguments, schema)?),
        PYTHON_INSTALL_PACKAGES_TOOL => serde_json::to_value(decode_arguments::<
            PythonInstallPackagesRequest,
        >(call.arguments, schema)?),
        PYTHON_RUN_SCRIPT_TOOL => serde_json::to_value(decode_arguments::<PythonRunScriptRequest>(
            call.arguments,
            schema,
        )?),
        _ => serde_json::to_value(decode_arguments::<ReadKnowledgeRequest>(
            call.arguments,
            schema,
        )?),
    }
    .map_err(|_| CapabilityFailure::new(CapabilityFailureCode::INTERNAL_FAILURE))?;
    Ok(PreparedToolCall {
        tool: call.name,
        arguments: request.clone(),
        request,
        idempotency_token: None,
        internal: false,
    })
}

fn fatal(code: AgentDriverFailureCode) -> ToolCallOutcome {
    ToolCallOutcome {
        invocation_id: None,
        output: Value::Null,
        fatal: Some(code),
    }
}

pub(crate) fn failed_output(
    extension: &dyn HarnessExtension,
    failure: &CapabilityFailure,
) -> Value {
    serde_json::json!({"state": "failed", "failure": extension.project_failure(failure)})
}

/// The model-facing delegation arguments of a bound task.
pub(crate) fn task_input(extension: &dyn HarnessExtension, task: &AgentTask) -> AgentTaskInput {
    AgentTaskInput {
        worker: task.worker.clone(),
        objective: task.objective.clone(),
        constraints: task.constraints.clone(),
        completion_criteria: task.completion_criteria.clone(),
        depends_on: task.depends_on.clone(),
        scope: extension.task_scope_input(&task.scope),
    }
}

/// The model-facing delegation result. Extension facts are projected by their owner.
pub(crate) fn task_outcome(extension: &dyn HarnessExtension, outcome: &AgentTaskOutcome) -> Value {
    let mut value = serde_json::json!({
        "runId": outcome.run_id,
        "role": outcome.role,
        "state": outcome.state,
        "report": outcome.report,
        "failureCode": outcome.failure_code,
        "evidence": outcome.evidence,
        "invalidatedRuns": outcome.invalidated_runs,
    });
    if let (Value::Object(target), Value::Object(facts)) =
        (&mut value, extension.project_task_facts(&outcome.facts))
    {
        target.extend(facts);
    }
    value
}

pub(crate) fn finish_outcome(
    run_id: AgentRunId,
    role: AgentRoleId,
    worker: bool,
    result: &Result<AgentTurnResult, AgentDriverFailure>,
    evidence: &Mutex<RunEvidence>,
    hooks: &dyn AgentRunHooks,
) -> AgentTaskOutcome {
    let finish = hooks.finish();
    let evidence = evidence.lock().unwrap_or_else(|e| e.into_inner());
    // The public reply is already owned by the turn transcript; only Workers produce reports.
    let report = result.as_ref().ok().filter(|_| worker).map(|result| {
        let (blocked_reason, next_steps) = match &evidence.blocked_reason {
            Some(reason) => (Some(reason.clone()), Vec::new()),
            None => (finish.blocked_reason.clone(), finish.next_steps.clone()),
        };
        WorkerReport {
            summary: result.final_text.clone(),
            warnings: vec![],
            blocked_reason,
            next_steps,
        }
    });
    let failure_code = result.as_ref().err().map(|error| error.code);
    let state = match failure_code {
        Some(AgentDriverFailureCode::Cancelled) => AgentRunState::Cancelled,
        Some(_) => AgentRunState::Failed,
        None if report
            .as_ref()
            .is_some_and(|report| report.blocked_reason.is_some()) =>
        {
            AgentRunState::Blocked
        }
        None => AgentRunState::Completed,
    };
    AgentTaskOutcome {
        run_id,
        role,
        state,
        report,
        failure_code,
        evidence: evidence.invocations.clone(),
        facts: finish.facts,
        invalidated_runs: vec![],
    }
}

fn rejected(reason: &str) -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST).with_detail("reason", reason)
}
fn persistence_failure() -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::PERSISTENCE_UNAVAILABLE)
}
fn cancelled() -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::CANCELLED)
}
fn driver_failure() -> AgentDriverFailure {
    AgentDriverFailure::new(AgentDriverFailureCode::InternalFailure)
}
fn cancelled_driver() -> AgentDriverFailure {
    AgentDriverFailure::new(AgentDriverFailureCode::Cancelled)
}

mod continuation;
mod delegation;
mod recovery;
pub(crate) use recovery::{pending_agent_turns, recover_runs};

#[cfg(test)]
mod tests;
