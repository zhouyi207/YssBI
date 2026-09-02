use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use yss_automation_contract::{
    AgentDriverFailure, AgentDriverFailureCode, AgentDriverPort, AgentEvent, AgentEventOutput,
    AgentMessage, AgentMessageRole, AgentOutputFailure, AgentTurnRequest, AgentTurnResult,
    ApprovalGrantId, ApprovalGrantRecord, ApprovalStorePort, AutomationCapabilityRequest,
    AutomationCapabilityResult, AutomationIdKind, AutomationIdentityError, CancellationReason,
    CancellationToken, CapabilityFailure, CapabilityFailureCode, ClockPort, HarnessEvent,
    HarnessEventEnvelope, HarnessEventSinkPort, HarnessEventStorePort, HarnessSessionId,
    HarnessSessionRecord, HarnessSessionState, HarnessSessionStorePort, HarnessTurnId,
    HarnessTurnRecord, HarnessTurnState, IdGenerationFailure, IdGeneratorPort, KnowledgeSearchHit,
    KnowledgeSourceStorePort, MemoryAuthor, MemoryConfidence, MemoryProposal, MemoryRecord,
    MemoryRecordId, MemoryScope, MemorySourceRef, MemoryStorePort, ModelCapabilityExecutor,
    ModelCapabilityRequest, PersistenceFailure, PrincipalId, ProjectSessionBinding,
    RetentionPolicy, SensitivityClass, StructuredMemoryValue, ToolInvocationLedgerPort,
    WorkflowRunId, WorkflowRunRecord, WorkflowRunState, WorkflowStepKind, WorkflowStorePort,
};

use crate::tools::HarnessToolExecutor;
use crate::{
    ApprovalError, ApprovalService, CompiledWorkflow, KnowledgeError, KnowledgeQuery,
    KnowledgeService, MemoryError, MemoryService, MethodRegistry, StatisticalPlanner, ToolRegistry,
    WorkflowCompileError, WorkflowRuntime, WorkflowRuntimeError,
};

const MAX_USER_MESSAGE_BYTES: usize = 64 * 1024;
const MAX_AGENT_TEXT_BYTES: usize = 1024 * 1024;

#[derive(Clone)]
pub struct HarnessPorts {
    pub agent_driver: Arc<dyn AgentDriverPort>,
    pub capability_gateway: Arc<dyn yss_automation_contract::CapabilityGatewayPort>,
    pub sessions: Arc<dyn HarnessSessionStorePort>,
    pub events: Arc<dyn HarnessEventStorePort>,
    pub event_sink: Arc<dyn HarnessEventSinkPort>,
    pub workflows: Arc<dyn WorkflowStorePort>,
    pub tool_ledger: Arc<dyn ToolInvocationLedgerPort>,
    pub knowledge: Arc<dyn KnowledgeSourceStorePort>,
    pub memory: Arc<dyn MemoryStorePort>,
    pub approvals: Arc<dyn ApprovalStorePort>,
    pub clock: Arc<dyn ClockPort>,
    pub ids: Arc<dyn IdGeneratorPort>,
}

pub struct HarnessHost {
    ports: HarnessPorts,
    tools: ToolRegistry,
    active_turns: Arc<Mutex<BTreeMap<HarnessSessionId, CancellationToken>>>,
    event_sequences: Arc<Mutex<BTreeMap<HarnessSessionId, u64>>>,
}

impl HarnessHost {
    pub fn new(ports: HarnessPorts) -> Result<Self, HarnessError> {
        Ok(Self {
            ports,
            tools: ToolRegistry::read_only_foundation()?,
            active_turns: Arc::new(Mutex::new(BTreeMap::new())),
            event_sequences: Arc::new(Mutex::new(BTreeMap::new())),
        })
    }

    pub async fn create_session(
        &self,
        principal_id: PrincipalId,
        project: ProjectSessionBinding,
    ) -> Result<HarnessSessionRecord, HarnessError> {
        let id =
            HarnessSessionId::try_new(self.ports.ids.next_id(AutomationIdKind::HarnessSession)?)?;
        let now = self.ports.clock.now();
        let record = HarnessSessionRecord {
            id: id.clone(),
            principal_id,
            project,
            state: HarnessSessionState::Active,
            created_at: now,
            updated_at: now,
        };
        self.ports.sessions.create_session(&record).await?;
        self.event_writer()
            .append(&id, None, HarnessEvent::SessionCreated)
            .await?;
        Ok(record)
    }

    pub async fn submit_turn(
        &self,
        session_id: &HarnessSessionId,
        user_message: String,
    ) -> Result<AgentTurnResult, HarnessError> {
        validate_user_message(&user_message)?;
        let session = self
            .ports
            .sessions
            .load_session(session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        if session.state != HarnessSessionState::Active {
            return Err(HarnessError::SessionNotActive);
        }
        let (cancellation, _admission) = self.admit_turn(session_id)?;
        let turn_id =
            HarnessTurnId::try_new(self.ports.ids.next_id(AutomationIdKind::HarnessTurn)?)?;
        let started_at = self.ports.clock.now();
        let mut turn = HarnessTurnRecord {
            id: turn_id.clone(),
            session_id: session_id.clone(),
            state: HarnessTurnState::Running,
            user_message: user_message.clone(),
            final_text: None,
            started_at,
            finished_at: None,
        };
        self.ports.sessions.create_turn(&turn).await?;
        if let Err(error) = self
            .event_writer()
            .append(
                session_id,
                Some(&turn_id),
                HarnessEvent::TurnStarted {
                    user_message: user_message.clone(),
                },
            )
            .await
        {
            turn.state = HarnessTurnState::Failed;
            turn.finished_at = Some(self.ports.clock.now());
            let _ = self.ports.sessions.update_turn(&turn).await;
            return Err(error);
        }

        let preparation = async {
            let memory_service = MemoryService::new(
                Arc::clone(&self.ports.memory),
                Arc::clone(&self.ports.clock),
                Arc::clone(&self.ports.ids),
            );
            match memory_service
                .propose(MemoryProposal {
                    session_id: session.id.clone(),
                    scope: MemoryScope::Session,
                    value: StructuredMemoryValue::ResearchQuestion {
                        question: user_message.clone(),
                    },
                    source_refs: vec![MemorySourceRef {
                        source_id: turn_id.to_string(),
                        source_revision: "1".to_owned(),
                    }],
                    confidence: MemoryConfidence::High,
                    project: Some(session.project.clone()),
                    sensitivity: SensitivityClass::Internal,
                    created_by: MemoryAuthor::User,
                    supersedes: None,
                    retention: RetentionPolicy::Session,
                })
                .await
            {
                Ok(record) => {
                    self.event_writer()
                        .append(
                            session_id,
                            Some(&turn_id),
                            HarnessEvent::MemoryRecorded { record },
                        )
                        .await?;
                }
                Err(MemoryError::PolicyRejected) => {}
                Err(error) => return Err(HarnessError::from(error)),
            }

            let knowledge = KnowledgeService::new(Arc::clone(&self.ports.knowledge))
                .search(KnowledgeQuery {
                    text: bounded_query(&user_message, 256),
                    scopes: Vec::new(),
                    project: Some(session.project.clone()),
                    limit: 5,
                })
                .await?;
            for hit in &knowledge {
                self.event_writer()
                    .append(
                        session_id,
                        Some(&turn_id),
                        HarnessEvent::KnowledgeCited {
                            citation: hit.citation.clone(),
                        },
                    )
                    .await?;
            }
            Ok::<_, HarnessError>(knowledge)
        }
        .await;
        let knowledge = match preparation {
            Ok(knowledge) => knowledge,
            Err(error) => return self.fail_turn(&mut turn, error).await,
        };

        let capability_executor = Arc::new(HarnessToolExecutor::new(
            self.tools.clone(),
            Arc::clone(&self.ports.capability_gateway),
            Arc::clone(&self.ports.tool_ledger),
            Arc::clone(&self.ports.clock),
            Arc::clone(&self.ports.ids),
            session.principal_id.clone(),
            session.id.clone(),
            turn_id.clone(),
            session.project.clone(),
            cancellation.clone(),
        ));
        let output = Arc::new(PersistingAgentOutput {
            writer: self.event_writer(),
            session_id: session.id.clone(),
            turn_id: turn_id.clone(),
        });
        let request = AgentTurnRequest {
            session_id: session.id.clone(),
            turn_id: turn_id.clone(),
            principal_id: session.principal_id.clone(),
            project: session.project.clone(),
            messages: agent_messages(user_message, &knowledge),
            tools: self.tools.descriptors(),
        };
        let result = self
            .ports
            .agent_driver
            .run_turn(request, capability_executor, output, cancellation.clone())
            .await;

        if cancellation.is_cancelled() {
            turn.state = HarnessTurnState::Cancelled;
            turn.finished_at = Some(self.ports.clock.now());
            self.ports.sessions.update_turn(&turn).await?;
            self.event_writer()
                .append(session_id, Some(&turn_id), HarnessEvent::TurnCancelled)
                .await?;
            self.finish_closing_session(session_id).await?;
            return Err(HarnessError::Cancelled);
        }
        match result {
            Ok(result) => {
                if result.final_text.len() > MAX_AGENT_TEXT_BYTES {
                    return self
                        .fail_turn(
                            &mut turn,
                            HarnessError::Agent(AgentDriverFailureCode::InvalidProviderResponse),
                        )
                        .await;
                }
                turn.state = HarnessTurnState::Completed;
                turn.final_text = Some(result.final_text.clone());
                turn.finished_at = Some(self.ports.clock.now());
                self.ports.sessions.update_turn(&turn).await?;
                self.event_writer()
                    .append(
                        session_id,
                        Some(&turn_id),
                        HarnessEvent::TurnCompleted {
                            final_text: result.final_text.clone(),
                        },
                    )
                    .await?;
                self.finish_closing_session(session_id).await?;
                Ok(result)
            }
            Err(error) => self.fail_turn(&mut turn, error.into()).await,
        }
    }

    pub fn cancel_turn(&self, session_id: &HarnessSessionId) -> bool {
        self.active_turns
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(session_id)
            .is_some_and(|token| token.cancel(CancellationReason::User))
    }

    pub async fn close_session(&self, session_id: &HarnessSessionId) -> Result<(), HarnessError> {
        let mut session = self
            .ports
            .sessions
            .load_session(session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        if session.state == HarnessSessionState::Closed {
            return Ok(());
        }
        session.state = HarnessSessionState::Closing;
        session.updated_at = self.ports.clock.now();
        self.ports.sessions.update_session(&session).await?;
        if self.cancel_active_turn(session_id, CancellationReason::SessionClosing) {
            return Err(HarnessError::TurnStillRunning);
        }
        session.state = HarnessSessionState::Closed;
        session.updated_at = self.ports.clock.now();
        MemoryService::new(
            Arc::clone(&self.ports.memory),
            Arc::clone(&self.ports.clock),
            Arc::clone(&self.ports.ids),
        )
        .expire_session(session_id)
        .await?;
        self.ports.sessions.update_session(&session).await?;
        self.event_writer()
            .append(session_id, None, HarnessEvent::SessionClosed)
            .await?;
        Ok(())
    }

    pub async fn mark_project_replaced(
        &self,
        session_id: &HarnessSessionId,
    ) -> Result<(), HarnessError> {
        let mut session = self
            .ports
            .sessions
            .load_session(session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        session.state = HarnessSessionState::Stale;
        session.updated_at = self.ports.clock.now();
        self.ports.sessions.update_session(&session).await?;
        self.cancel_active_turn(session_id, CancellationReason::ProjectReplaced);
        MemoryService::new(
            Arc::clone(&self.ports.memory),
            Arc::clone(&self.ports.clock),
            Arc::clone(&self.ports.ids),
        )
        .expire_session(session_id)
        .await?;
        Ok(())
    }

    pub async fn reconcile_project_session(
        &self,
        current: &ProjectSessionBinding,
    ) -> Result<usize, HarnessError> {
        let mut stale_count = 0usize;
        for mut session in self.ports.sessions.load_open_sessions().await? {
            if &session.project == current {
                continue;
            }
            session.state = HarnessSessionState::Stale;
            session.updated_at = self.ports.clock.now();
            self.ports.sessions.update_session(&session).await?;
            self.cancel_active_turn(&session.id, CancellationReason::ProjectReplaced);
            MemoryService::new(
                Arc::clone(&self.ports.memory),
                Arc::clone(&self.ports.clock),
                Arc::clone(&self.ports.ids),
            )
            .expire_session(&session.id)
            .await?;
            stale_count += 1;
        }
        Ok(stale_count)
    }

    pub async fn plan_workflow(
        &self,
        session_id: &HarnessSessionId,
        turn_id: Option<&HarnessTurnId>,
        compiled: &CompiledWorkflow,
    ) -> Result<WorkflowRunRecord, HarnessError> {
        let session = self
            .ports
            .sessions
            .load_session(session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        if session.state != HarnessSessionState::Active {
            return Err(HarnessError::SessionNotActive);
        }
        if let Some(turn_id) = turn_id {
            let turn = self
                .ports
                .sessions
                .load_turn(turn_id)
                .await?
                .ok_or(HarnessError::WorkflowTurnNotFound)?;
            if turn.session_id != session.id {
                return Err(HarnessError::WorkflowTurnMismatch);
            }
        }
        self.ports
            .workflows
            .save_definition(compiled.definition())
            .await?;
        let run_id =
            WorkflowRunId::try_new(self.ports.ids.next_id(AutomationIdKind::WorkflowRun)?)?;
        let run = WorkflowRuntime::plan(
            compiled,
            run_id.clone(),
            session.id.clone(),
            turn_id.cloned(),
            session.project,
            self.ports.clock.now(),
        );
        self.ports.workflows.save_run(&run).await?;
        self.event_writer()
            .append(
                session_id,
                turn_id,
                HarnessEvent::WorkflowPlanned { run_id },
            )
            .await?;
        Ok(run)
    }

    pub async fn advance_workflow(
        &self,
        run_id: &WorkflowRunId,
    ) -> Result<WorkflowRunRecord, HarnessError> {
        let mut run = self
            .ports
            .workflows
            .load_run(run_id)
            .await?
            .ok_or(HarnessError::WorkflowNotFound)?;
        if matches!(
            run.state,
            WorkflowRunState::Completed | WorkflowRunState::Failed | WorkflowRunState::Cancelled
        ) {
            return Ok(run);
        }
        if matches!(
            run.state,
            WorkflowRunState::WaitingForApproval | WorkflowRunState::WaitingForExternalInput
        ) {
            return Err(HarnessError::WorkflowWaiting);
        }
        let session = self
            .ports
            .sessions
            .load_session(&run.session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        if session.state != HarnessSessionState::Active || session.project != run.project {
            run.state = WorkflowRunState::Paused;
            run.updated_at = self.ports.clock.now();
            self.ports.workflows.save_run(&run).await?;
            return Err(HarnessError::WorkflowProjectChanged);
        }
        let definition = self
            .ports
            .workflows
            .load_definition(&run.definition_id, &run.definition_version)
            .await?
            .ok_or(HarnessError::WorkflowDefinitionNotFound)?;
        let compiled = CompiledWorkflow::compile(definition)?;
        let was_planned = run.state == WorkflowRunState::Planned;
        if matches!(
            run.state,
            WorkflowRunState::Planned | WorkflowRunState::Ready | WorkflowRunState::Paused
        ) {
            WorkflowRuntime::start(&mut run, self.ports.clock.now())?;
            self.ports.workflows.save_run(&run).await?;
            if was_planned {
                self.event_writer()
                    .append(
                        &run.session_id,
                        run.turn_id.as_ref(),
                        HarnessEvent::WorkflowStarted {
                            run_id: run.id.clone(),
                        },
                    )
                    .await?;
            }
        }
        let Some(step_id) = WorkflowRuntime::ready_steps(&compiled, &run)?
            .into_iter()
            .next()
        else {
            return Ok(run);
        };
        let step = compiled
            .definition()
            .steps
            .iter()
            .find(|step| step.id == step_id)
            .ok_or(HarnessError::WorkflowDefinitionNotFound)?;
        match &step.kind {
            WorkflowStepKind::Approval { .. } => {
                WorkflowRuntime::wait_for_approval(
                    &compiled,
                    &mut run,
                    &step_id,
                    self.ports.clock.now(),
                )?;
                self.ports.workflows.save_run(&run).await?;
            }
            WorkflowStepKind::Decision { .. } => {
                WorkflowRuntime::wait_for_external_input(
                    &compiled,
                    &mut run,
                    &step_id,
                    self.ports.clock.now(),
                )?;
                self.ports.workflows.save_run(&run).await?;
            }
            WorkflowStepKind::Capability(request) => {
                let turn_id = run
                    .turn_id
                    .clone()
                    .ok_or(HarnessError::WorkflowTurnNotFound)?;
                let turn = self
                    .ports
                    .sessions
                    .load_turn(&turn_id)
                    .await?
                    .ok_or(HarnessError::WorkflowTurnNotFound)?;
                if turn.session_id != run.session_id {
                    return Err(HarnessError::WorkflowTurnMismatch);
                }
                WorkflowRuntime::start_step(&compiled, &mut run, &step_id, self.ports.clock.now())?;
                self.ports.workflows.save_run(&run).await?;
                self.event_writer()
                    .append(
                        &run.session_id,
                        Some(&turn_id),
                        HarnessEvent::WorkflowStepStarted {
                            run_id: run.id.clone(),
                            step_id: step_id.clone(),
                        },
                    )
                    .await?;
                let executor = HarnessToolExecutor::new_for_workflow(
                    self.tools.clone(),
                    Arc::clone(&self.ports.capability_gateway),
                    Arc::clone(&self.ports.tool_ledger),
                    Arc::clone(&self.ports.clock),
                    Arc::clone(&self.ports.ids),
                    session.principal_id,
                    run.session_id.clone(),
                    turn_id.clone(),
                    run.project.clone(),
                    CancellationToken::default(),
                    run.id.clone(),
                    step_id.clone(),
                );
                match executor
                    .execute(ModelCapabilityRequest {
                        request: request.clone(),
                    })
                    .await
                {
                    Ok(_) => {
                        WorkflowRuntime::succeed_step(
                            &compiled,
                            &mut run,
                            &step_id,
                            self.ports.clock.now(),
                        )?;
                        self.ports.workflows.save_run(&run).await?;
                        self.event_writer()
                            .append(
                                &run.session_id,
                                Some(&turn_id),
                                HarnessEvent::WorkflowStepCompleted {
                                    run_id: run.id.clone(),
                                    step_id,
                                },
                            )
                            .await?;
                        if run.state == WorkflowRunState::Completed {
                            self.event_writer()
                                .append(
                                    &run.session_id,
                                    Some(&turn_id),
                                    HarnessEvent::WorkflowCompleted {
                                        run_id: run.id.clone(),
                                    },
                                )
                                .await?;
                        }
                    }
                    Err(failure) => {
                        let retriable = workflow_failure_is_retriable(failure.code);
                        WorkflowRuntime::fail_step(
                            &mut run,
                            &step_id,
                            retriable,
                            self.ports.clock.now(),
                        )?;
                        self.ports.workflows.save_run(&run).await?;
                        self.event_writer()
                            .append(
                                &run.session_id,
                                Some(&turn_id),
                                HarnessEvent::WorkflowStepFailed {
                                    run_id: run.id.clone(),
                                    step_id,
                                    retriable,
                                },
                            )
                            .await?;
                    }
                }
            }
        }
        Ok(run)
    }

    pub async fn pause_workflow(
        &self,
        run_id: &WorkflowRunId,
    ) -> Result<WorkflowRunRecord, HarnessError> {
        let mut run = self
            .ports
            .workflows
            .load_run(run_id)
            .await?
            .ok_or(HarnessError::WorkflowNotFound)?;
        WorkflowRuntime::pause(&mut run, self.ports.clock.now())?;
        self.ports.workflows.save_run(&run).await?;
        self.event_writer()
            .append(
                &run.session_id,
                run.turn_id.as_ref(),
                HarnessEvent::WorkflowPaused {
                    run_id: run.id.clone(),
                },
            )
            .await?;
        Ok(run)
    }

    pub async fn resume_workflow(
        &self,
        run_id: &WorkflowRunId,
    ) -> Result<WorkflowRunRecord, HarnessError> {
        let mut run = self
            .ports
            .workflows
            .load_run(run_id)
            .await?
            .ok_or(HarnessError::WorkflowNotFound)?;
        WorkflowRuntime::resume(&mut run, self.ports.clock.now())?;
        self.ports.workflows.save_run(&run).await?;
        self.event_writer()
            .append(
                &run.session_id,
                run.turn_id.as_ref(),
                HarnessEvent::WorkflowResumed {
                    run_id: run.id.clone(),
                },
            )
            .await?;
        Ok(run)
    }

    pub async fn cancel_workflow(
        &self,
        run_id: &WorkflowRunId,
    ) -> Result<WorkflowRunRecord, HarnessError> {
        let mut run = self
            .ports
            .workflows
            .load_run(run_id)
            .await?
            .ok_or(HarnessError::WorkflowNotFound)?;
        WorkflowRuntime::cancel(&mut run, self.ports.clock.now())?;
        self.ports.workflows.save_run(&run).await?;
        self.event_writer()
            .append(
                &run.session_id,
                run.turn_id.as_ref(),
                HarnessEvent::WorkflowCancelled {
                    run_id: run.id.clone(),
                },
            )
            .await?;
        Ok(run)
    }

    pub async fn recover_workflows(&self) -> Result<usize, HarnessError> {
        let mut recovered = 0usize;
        for mut run in self.ports.workflows.load_recoverable_runs().await? {
            if run.state != WorkflowRunState::Running {
                continue;
            }
            let definition = self
                .ports
                .workflows
                .load_definition(&run.definition_id, &run.definition_version)
                .await?
                .ok_or(HarnessError::WorkflowDefinitionNotFound)?;
            let compiled = CompiledWorkflow::compile(definition)?;
            WorkflowRuntime::recover_interrupted(&compiled, &mut run, self.ports.clock.now())?;
            let current = self.ports.sessions.load_session(&run.session_id).await?;
            if current.as_ref().is_none_or(|session| {
                session.state != HarnessSessionState::Active || session.project != run.project
            }) {
                run.state = WorkflowRunState::Paused;
            }
            self.ports.workflows.save_run(&run).await?;
            recovered += 1;
        }
        Ok(recovered)
    }

    pub async fn events_after(
        &self,
        session_id: &HarnessSessionId,
        sequence: u64,
    ) -> Result<Vec<HarnessEventEnvelope>, HarnessError> {
        Ok(self
            .ports
            .events
            .load_events_after(session_id, sequence)
            .await?)
    }

    pub async fn session_memory(
        &self,
        session_id: &HarnessSessionId,
    ) -> Result<Vec<MemoryRecord>, HarnessError> {
        self.ports
            .sessions
            .load_session(session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        Ok(MemoryService::new(
            Arc::clone(&self.ports.memory),
            Arc::clone(&self.ports.clock),
            Arc::clone(&self.ports.ids),
        )
        .records_for_session(session_id)
        .await?)
    }

    pub async fn issue_capability_approval(
        &self,
        session_id: &HarnessSessionId,
        turn_id: &HarnessTurnId,
        request: &AutomationCapabilityRequest,
        ttl_ms: u64,
    ) -> Result<ApprovalGrantRecord, HarnessError> {
        let session = self
            .ports
            .sessions
            .load_session(session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        if session.state != HarnessSessionState::Active {
            return Err(HarnessError::SessionNotActive);
        }
        let turn = self
            .ports
            .sessions
            .load_turn(turn_id)
            .await?
            .ok_or(HarnessError::WorkflowTurnNotFound)?;
        if turn.session_id != session.id {
            return Err(HarnessError::WorkflowTurnMismatch);
        }
        Ok(ApprovalService::new(
            Arc::clone(&self.ports.approvals),
            Arc::clone(&self.ports.clock),
            Arc::clone(&self.ports.ids),
        )
        .issue(
            session.principal_id,
            session.id,
            session.project,
            request,
            ttl_ms,
        )
        .await?)
    }

    pub async fn execute_approved_capability(
        &self,
        session_id: &HarnessSessionId,
        turn_id: &HarnessTurnId,
        approval_grant_id: &ApprovalGrantId,
        request: AutomationCapabilityRequest,
    ) -> Result<AutomationCapabilityResult, HarnessError> {
        let session = self
            .ports
            .sessions
            .load_session(session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        if session.state != HarnessSessionState::Active {
            return Err(HarnessError::SessionNotActive);
        }
        let turn = self
            .ports
            .sessions
            .load_turn(turn_id)
            .await?
            .ok_or(HarnessError::WorkflowTurnNotFound)?;
        if turn.session_id != session.id {
            return Err(HarnessError::WorkflowTurnMismatch);
        }
        ApprovalService::new(
            Arc::clone(&self.ports.approvals),
            Arc::clone(&self.ports.clock),
            Arc::clone(&self.ports.ids),
        )
        .consume(
            approval_grant_id,
            &session.principal_id,
            &session.id,
            &session.project,
            &request,
        )
        .await?;
        let capability_id = request.capability_id();
        self.event_writer()
            .append(
                session_id,
                Some(turn_id),
                HarnessEvent::Agent(AgentEvent::ToolInvocationRequested { capability_id }),
            )
            .await?;
        let registry = self.tools.clone().with_approved_capability(capability_id)?;
        let outcome = HarnessToolExecutor::new_approved(
            registry,
            Arc::clone(&self.ports.capability_gateway),
            Arc::clone(&self.ports.tool_ledger),
            Arc::clone(&self.ports.clock),
            Arc::clone(&self.ports.ids),
            session.principal_id,
            session.id,
            turn_id.clone(),
            session.project,
            approval_grant_id.clone(),
        )
        .execute(ModelCapabilityRequest { request })
        .await?;
        self.event_writer()
            .append(
                session_id,
                Some(turn_id),
                HarnessEvent::Agent(AgentEvent::ToolInvocationCompleted {
                    invocation_id: outcome.invocation_id,
                    capability_id,
                }),
            )
            .await?;
        Ok(outcome.result)
    }

    pub async fn delete_session_memory(
        &self,
        session_id: &HarnessSessionId,
        record_id: &MemoryRecordId,
    ) -> Result<(), HarnessError> {
        if !self
            .session_memory(session_id)
            .await?
            .iter()
            .any(|record| &record.id == record_id)
        {
            return Err(HarnessError::MemoryNotFound);
        }
        MemoryService::new(
            Arc::clone(&self.ports.memory),
            Arc::clone(&self.ports.clock),
            Arc::clone(&self.ports.ids),
        )
        .delete(record_id)
        .await?;
        self.event_writer()
            .append(
                session_id,
                None,
                HarnessEvent::MemoryDeleted {
                    record_id: record_id.clone(),
                },
            )
            .await?;
        Ok(())
    }

    async fn fail_turn<T>(
        &self,
        turn: &mut HarnessTurnRecord,
        error: HarnessError,
    ) -> Result<T, HarnessError> {
        turn.state = HarnessTurnState::Failed;
        turn.finished_at = Some(self.ports.clock.now());
        self.ports.sessions.update_turn(turn).await?;
        self.event_writer()
            .append(&turn.session_id, Some(&turn.id), HarnessEvent::TurnFailed)
            .await?;
        self.finish_closing_session(&turn.session_id).await?;
        Err(error)
    }

    async fn finish_closing_session(
        &self,
        session_id: &HarnessSessionId,
    ) -> Result<(), HarnessError> {
        let Some(mut session) = self.ports.sessions.load_session(session_id).await? else {
            return Ok(());
        };
        if session.state != HarnessSessionState::Closing {
            return Ok(());
        }
        MemoryService::new(
            Arc::clone(&self.ports.memory),
            Arc::clone(&self.ports.clock),
            Arc::clone(&self.ports.ids),
        )
        .expire_session(session_id)
        .await?;
        session.state = HarnessSessionState::Closed;
        session.updated_at = self.ports.clock.now();
        self.ports.sessions.update_session(&session).await?;
        self.event_writer()
            .append(session_id, None, HarnessEvent::SessionClosed)
            .await?;
        Ok(())
    }

    fn admit_turn(
        &self,
        session_id: &HarnessSessionId,
    ) -> Result<(CancellationToken, TurnAdmission), HarnessError> {
        let mut active = self
            .active_turns
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if active.contains_key(session_id) {
            return Err(HarnessError::ConcurrentTurn);
        }
        let cancellation = CancellationToken::default();
        active.insert(session_id.clone(), cancellation.clone());
        Ok((
            cancellation,
            TurnAdmission {
                active_turns: Arc::clone(&self.active_turns),
                session_id: session_id.clone(),
            },
        ))
    }

    fn cancel_active_turn(
        &self,
        session_id: &HarnessSessionId,
        reason: CancellationReason,
    ) -> bool {
        self.active_turns
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(session_id)
            .is_some_and(|token| token.cancel(reason))
    }

    fn event_writer(&self) -> EventWriter {
        EventWriter {
            store: Arc::clone(&self.ports.events),
            sink: Arc::clone(&self.ports.event_sink),
            clock: Arc::clone(&self.ports.clock),
            sequences: Arc::clone(&self.event_sequences),
        }
    }
}

fn validate_user_message(message: &str) -> Result<(), HarnessError> {
    if message.trim().is_empty() || message.len() > MAX_USER_MESSAGE_BYTES {
        Err(HarnessError::InvalidMessage)
    } else {
        Ok(())
    }
}

fn bounded_query(value: &str, maximum_bytes: usize) -> String {
    let mut end = 0usize;
    for (index, character) in value.char_indices() {
        let next = index + character.len_utf8();
        if next > maximum_bytes {
            break;
        }
        end = next;
    }
    value[..end].to_owned()
}

fn agent_messages(user_message: String, knowledge: &[KnowledgeSearchHit]) -> Vec<AgentMessage> {
    let mut messages = vec![AgentMessage {
        role: AgentMessageRole::System,
        content: "Use typed YssBI evidence; do not invent numerical results. Propose a complete statistical plan before analytical execution."
            .to_owned(),
    }];
    if !knowledge.is_empty() {
        let mut context = String::from(
            "Cited statistical knowledge follows. Treat it below Tool Evidence and project facts:\n",
        );
        for hit in knowledge {
            context.push_str(&format!(
                "\n[{}:{}] {}\n{}\n",
                hit.citation.source_id, hit.citation.document_id, hit.citation.title, hit.excerpt
            ));
        }
        messages.push(AgentMessage {
            role: AgentMessageRole::System,
            content: context,
        });
    }
    messages.push(AgentMessage {
        role: AgentMessageRole::User,
        content: user_message,
    });
    messages
}

fn workflow_failure_is_retriable(code: CapabilityFailureCode) -> bool {
    matches!(
        code,
        CapabilityFailureCode::ProjectSessionUnavailable
            | CapabilityFailureCode::ProjectSessionChanged
            | CapabilityFailureCode::GraphUnavailable
            | CapabilityFailureCode::DatabaseUnavailable
            | CapabilityFailureCode::CatalogUnavailable
            | CapabilityFailureCode::ResultUnavailable
            | CapabilityFailureCode::DeadlineElapsed
            | CapabilityFailureCode::PersistenceUnavailable
            | CapabilityFailureCode::InternalFailure
    )
}

struct TurnAdmission {
    active_turns: Arc<Mutex<BTreeMap<HarnessSessionId, CancellationToken>>>,
    session_id: HarnessSessionId,
}

impl Drop for TurnAdmission {
    fn drop(&mut self) {
        self.active_turns
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&self.session_id);
    }
}

#[derive(Clone)]
struct EventWriter {
    store: Arc<dyn HarnessEventStorePort>,
    sink: Arc<dyn HarnessEventSinkPort>,
    clock: Arc<dyn ClockPort>,
    sequences: Arc<Mutex<BTreeMap<HarnessSessionId, u64>>>,
}

impl EventWriter {
    async fn append(
        &self,
        session_id: &HarnessSessionId,
        turn_id: Option<&HarnessTurnId>,
        event: HarnessEvent,
    ) -> Result<(), HarnessError> {
        let observed = self.store.latest_sequence(session_id).await?;
        let sequence = {
            let mut sequences = self
                .sequences
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let current = sequences.entry(session_id.clone()).or_insert(observed);
            if *current < observed {
                *current = observed;
            }
            *current = current
                .checked_add(1)
                .ok_or(HarnessError::SequenceExhausted)?;
            *current
        };
        let envelope = HarnessEventEnvelope {
            sequence,
            session_id: session_id.clone(),
            turn_id: turn_id.cloned(),
            occurred_at: self.clock.now(),
            event,
        };
        if let Err(error) = self.store.append_event(&envelope).await {
            let mut sequences = self
                .sequences
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if sequences.get(session_id) == Some(&sequence) {
                sequences.insert(session_id.clone(), sequence.saturating_sub(1));
            }
            return Err(error.into());
        }
        self.sink.publish(&envelope).await?;
        Ok(())
    }
}

struct PersistingAgentOutput {
    writer: EventWriter,
    session_id: HarnessSessionId,
    turn_id: HarnessTurnId,
}

impl AgentEventOutput for PersistingAgentOutput {
    fn emit<'a>(
        &'a self,
        event: AgentEvent,
    ) -> yss_automation_contract::AgentFuture<'a, Result<(), AgentOutputFailure>> {
        Box::pin(async move {
            if let AgentEvent::PlanProposed { plan } = &event {
                let methods =
                    MethodRegistry::builtins().map_err(|_| AgentOutputFailure::PolicyRejected)?;
                StatisticalPlanner::validate(plan, &methods)
                    .map_err(|_| AgentOutputFailure::PolicyRejected)?;
            }
            self.writer
                .append(
                    &self.session_id,
                    Some(&self.turn_id),
                    HarnessEvent::Agent(event),
                )
                .await
                .map_err(|_| AgentOutputFailure::PersistenceFailed)
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum HarnessError {
    #[error("automation identity is invalid")]
    Identity(#[from] AutomationIdentityError),
    #[error("automation id generation failed")]
    IdGeneration(#[from] IdGenerationFailure),
    #[error("harness persistence failed")]
    Persistence(#[from] PersistenceFailure),
    #[error("harness knowledge retrieval failed")]
    Knowledge(#[from] KnowledgeError),
    #[error("harness memory operation failed")]
    Memory(#[from] MemoryError),
    #[error("harness memory record was not found in the session")]
    MemoryNotFound,
    #[error("harness approval operation failed")]
    Approval(#[from] ApprovalError),
    #[error("harness capability execution failed")]
    Capability(#[from] CapabilityFailure),
    #[error("harness session was not found")]
    SessionNotFound,
    #[error("harness session is not active")]
    SessionNotActive,
    #[error("harness session already has an active turn")]
    ConcurrentTurn,
    #[error("harness user message is invalid")]
    InvalidMessage,
    #[error("agent turn failed")]
    Agent(AgentDriverFailureCode),
    #[error("harness turn was cancelled")]
    Cancelled,
    #[error("harness turn is still draining")]
    TurnStillRunning,
    #[error("harness event sequence is exhausted")]
    SequenceExhausted,
    #[error("workflow compilation failed")]
    WorkflowCompile(#[from] WorkflowCompileError),
    #[error("workflow state transition failed")]
    WorkflowRuntime(#[from] WorkflowRuntimeError),
    #[error("workflow run was not found")]
    WorkflowNotFound,
    #[error("workflow immutable definition was not found")]
    WorkflowDefinitionNotFound,
    #[error("workflow originating turn was not found")]
    WorkflowTurnNotFound,
    #[error("workflow originating turn belongs to another session")]
    WorkflowTurnMismatch,
    #[error("workflow project binding changed")]
    WorkflowProjectChanged,
    #[error("workflow is waiting for approval or external input")]
    WorkflowWaiting,
}

impl From<AgentDriverFailure> for HarnessError {
    fn from(error: AgentDriverFailure) -> Self {
        if error.code == AgentDriverFailureCode::Cancelled {
            Self::Cancelled
        } else {
            Self::Agent(error.code)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::dataset_quality_review_workflow;
    use crate::test_support::{
        FixedClock, InMemoryHarnessStore, MockAgentDriver, RejectingCapabilityGateway,
        SequentialIds, StaticCapabilityGateway,
    };
    use yss_automation_contract::{
        ApplyGraphEditRequest, AutomationCapabilityRequest, AutomationCapabilityResult,
        CapabilityFuture, CapabilityGatewayPort, CapabilityInvocationContext,
        DatasetProfileInspection, DatasetSchemaInspection, GraphEditOperation, GraphEditPosition,
        GraphEditReceipt, WorkflowRunState,
    };
    use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

    #[tokio::test]
    async fn session_turn_persists_one_gap_free_ordered_event_stream() {
        let store = Arc::new(InMemoryHarnessStore::default());
        let host = HarnessHost::new(HarnessPorts {
            agent_driver: Arc::new(MockAgentDriver::new("Evidence is ready.")),
            capability_gateway: Arc::new(RejectingCapabilityGateway),
            sessions: store.clone(),
            events: store.clone(),
            event_sink: store.clone(),
            workflows: store.clone(),
            tool_ledger: store.clone(),
            knowledge: store.clone(),
            memory: store.clone(),
            approvals: store.clone(),
            clock: Arc::new(FixedClock::new(1_000)),
            ids: Arc::new(SequentialIds::default()),
        })
        .unwrap();
        let session = host
            .create_session(
                PrincipalId::try_new("user-1").unwrap(),
                ProjectSessionBinding::new(
                    ProjectInstanceId::from_existing("project-1".into()),
                    ProjectSessionId::new("project-session-1"),
                ),
            )
            .await
            .unwrap();

        let result = host
            .submit_turn(&session.id, "Review the dataset.".to_owned())
            .await
            .unwrap();
        let events = host.events_after(&session.id, 0).await.unwrap();

        assert_eq!(result.final_text, "Evidence is ready.");
        assert_eq!(
            events
                .iter()
                .map(|event| event.sequence)
                .collect::<Vec<_>>(),
            [1, 2, 3, 4, 5]
        );
        assert!(matches!(events[0].event, HarnessEvent::SessionCreated));
        assert!(matches!(
            events[4].event,
            HarnessEvent::TurnCompleted { .. }
        ));
        assert_eq!(store.published_events(), events);
    }

    #[tokio::test]
    async fn dataset_quality_workflow_persists_and_completes_its_typed_tool_step() {
        let store = Arc::new(InMemoryHarnessStore::default());
        let host = HarnessHost::new(HarnessPorts {
            agent_driver: Arc::new(MockAgentDriver::new("Plan ready.")),
            capability_gateway: Arc::new(
                StaticCapabilityGateway::new(AutomationCapabilityResult::DatasetSchemaInspection(
                    DatasetSchemaInspection {
                        database_id: "database-1".to_owned(),
                        runtime_revision: 1,
                        schema_revision: 1,
                        columns: Vec::new(),
                    },
                ))
                .with_result(
                    AutomationCapabilityResult::DatasetProfileInspection(
                        DatasetProfileInspection {
                            database_id: "database-1".to_owned(),
                            runtime_revision: 1,
                            schema_revision: 1,
                            row_count: 0,
                            column_count: 0,
                            estimated_memory_bytes: Some(0),
                            duplicated_rows: Some(0),
                            numeric_columns: 0,
                            categorical_columns: 0,
                            string_columns: 0,
                            temporal_columns: 0,
                            boolean_columns: 0,
                            total_nulls: 0,
                            null_ratio: 0.0,
                            columns_with_nulls: 0,
                            rows_with_nulls: 0,
                        },
                    ),
                ),
            ),
            sessions: store.clone(),
            events: store.clone(),
            event_sink: store.clone(),
            workflows: store.clone(),
            tool_ledger: store.clone(),
            knowledge: store.clone(),
            memory: store.clone(),
            approvals: store.clone(),
            clock: Arc::new(FixedClock::new(2_000)),
            ids: Arc::new(SequentialIds::default()),
        })
        .unwrap();
        let session = host
            .create_session(
                PrincipalId::try_new("user-1").unwrap(),
                ProjectSessionBinding::new(
                    ProjectInstanceId::from_existing("project-1".into()),
                    ProjectSessionId::new("project-session-1"),
                ),
            )
            .await
            .unwrap();
        host.submit_turn(&session.id, "Review quality.".to_owned())
            .await
            .unwrap();
        let turn_id = host.events_after(&session.id, 0).await.unwrap()[1]
            .turn_id
            .clone()
            .unwrap();
        let workflow = dataset_quality_review_workflow("database-1").unwrap();
        let planned = host
            .plan_workflow(&session.id, Some(&turn_id), &workflow)
            .await
            .unwrap();

        let running = host.advance_workflow(&planned.id).await.unwrap();
        assert_eq!(running.state, WorkflowRunState::Running);
        let completed = host.advance_workflow(&planned.id).await.unwrap();

        assert_eq!(completed.state, WorkflowRunState::Completed);
        assert!(
            host.events_after(&session.id, 0)
                .await
                .unwrap()
                .iter()
                .any(|event| matches!(event.event, HarnessEvent::WorkflowCompleted { .. }))
        );
    }

    #[tokio::test]
    async fn project_session_reconciliation_stales_old_open_sessions() {
        let store = Arc::new(InMemoryHarnessStore::default());
        let host = HarnessHost::new(HarnessPorts {
            agent_driver: Arc::new(MockAgentDriver::new("unused")),
            capability_gateway: Arc::new(RejectingCapabilityGateway),
            sessions: store.clone(),
            events: store.clone(),
            event_sink: store.clone(),
            workflows: store.clone(),
            tool_ledger: store.clone(),
            knowledge: store.clone(),
            memory: store.clone(),
            approvals: store.clone(),
            clock: Arc::new(FixedClock::new(3_000)),
            ids: Arc::new(SequentialIds::default()),
        })
        .unwrap();
        let old = host
            .create_session(
                PrincipalId::try_new("user-1").unwrap(),
                ProjectSessionBinding::new(
                    ProjectInstanceId::from_existing("project-1".into()),
                    ProjectSessionId::new("project-session-old"),
                ),
            )
            .await
            .unwrap();
        let current = ProjectSessionBinding::new(
            ProjectInstanceId::from_existing("project-1".into()),
            ProjectSessionId::new("project-session-current"),
        );

        assert_eq!(host.reconcile_project_session(&current).await.unwrap(), 1);
        assert_eq!(
            store.load_session(&old.id).await.unwrap().unwrap().state,
            HarnessSessionState::Stale
        );
    }

    struct ApprovedGateway;

    impl CapabilityGatewayPort for ApprovedGateway {
        fn invoke<'a>(
            &'a self,
            context: CapabilityInvocationContext,
            request: AutomationCapabilityRequest,
        ) -> CapabilityFuture<'a> {
            Box::pin(async move {
                assert!(context.approval_grant_id().is_some());
                assert!(matches!(
                    request,
                    AutomationCapabilityRequest::ApplyGraphEdit(_)
                ));
                Ok(AutomationCapabilityResult::GraphEditReceipt(
                    GraphEditReceipt {
                        graph_path: "events/Main.yssbi-event".to_owned(),
                        from_revision: 1,
                        to_revision: 2,
                        operation_id: "operation-1".to_owned(),
                        client_key: "assistant-edit-1".to_owned(),
                        can_undo: true,
                    },
                ))
            })
        }
    }

    #[tokio::test]
    async fn approved_capability_is_ledgered_and_cannot_reuse_its_grant() {
        let store = Arc::new(InMemoryHarnessStore::default());
        let host = HarnessHost::new(HarnessPorts {
            agent_driver: Arc::new(MockAgentDriver::new("Ready for approval.")),
            capability_gateway: Arc::new(ApprovedGateway),
            sessions: store.clone(),
            events: store.clone(),
            event_sink: store.clone(),
            workflows: store.clone(),
            tool_ledger: store.clone(),
            knowledge: store.clone(),
            memory: store.clone(),
            approvals: store.clone(),
            clock: Arc::new(FixedClock::new(4_000)),
            ids: Arc::new(SequentialIds::default()),
        })
        .unwrap();
        let session = host
            .create_session(
                PrincipalId::try_new("user-1").unwrap(),
                ProjectSessionBinding::new(
                    ProjectInstanceId::from_existing("project-1".into()),
                    ProjectSessionId::new("project-session-1"),
                ),
            )
            .await
            .unwrap();
        host.submit_turn(&session.id, "Move the node.".to_owned())
            .await
            .unwrap();
        let turn_id = host.events_after(&session.id, 0).await.unwrap()[1]
            .turn_id
            .clone()
            .unwrap();
        let request = AutomationCapabilityRequest::ApplyGraphEdit(ApplyGraphEditRequest {
            graph_path: "events/Main.yssbi-event".to_owned(),
            base_revision: 1,
            client_key: "assistant-edit-1".to_owned(),
            locale: "en-US".to_owned(),
            operations: vec![GraphEditOperation::MoveNodes {
                positions: vec![GraphEditPosition {
                    node_id: "00000000-0000-0000-0000-000000000001".to_owned(),
                    x: 10.0,
                    y: 20.0,
                }],
            }],
        });
        let grant = host
            .issue_capability_approval(&session.id, &turn_id, &request, 1_000)
            .await
            .unwrap();

        assert!(matches!(
            host.execute_approved_capability(&session.id, &turn_id, &grant.id, request.clone())
                .await
                .unwrap(),
            AutomationCapabilityResult::GraphEditReceipt(_)
        ));
        assert!(matches!(
            host.execute_approved_capability(&session.id, &turn_id, &grant.id, request)
                .await,
            Err(HarnessError::Approval(ApprovalError::AlreadyConsumed))
        ));
    }
}
