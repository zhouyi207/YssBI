use crate::conversation::{agent_messages, bounded_query};
use crate::{
    HarnessError, HarnessHost, KnowledgeQuery, KnowledgeService, MemoryError, MemoryService,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use yss_harness_contract::{
    AgentDriverFailureCode, AgentEvent, AgentTurnResult, AutomationIdKind, CancellationReason,
    CancellationToken, CapabilityFailureCode, HarnessEvent, HarnessSessionId, HarnessSessionState,
    HarnessTurnId, HarnessTurnRecord, HarnessTurnState, MemoryAuthor, MemoryConfidence,
    MemoryProposal, MemoryScope, MemorySourceRef, ProjectSessionBinding, RetentionPolicy,
    SensitivityClass, StructuredMemoryValue,
};

const MAX_USER_MESSAGE_BYTES: usize = 64 * 1024;
const MAX_AGENT_TEXT_BYTES: usize = 1024 * 1024;

impl HarnessHost {
    pub async fn submit_turn(
        &self,
        session_id: &HarnessSessionId,
        expected_project: &ProjectSessionBinding,
        user_message: String,
        active_graph_path: Option<String>,
    ) -> Result<AgentTurnResult, HarnessError> {
        validate_user_message(&user_message)?;
        let access = self.session_access().await;
        let mut session = self
            .ports
            .sessions
            .load_session(session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        if session.state != HarnessSessionState::Active || &session.project != expected_project {
            return Err(HarnessError::SessionNotActive);
        }
        let (cancellation, _admission) = self.admit_turn(session_id)?;
        if let Some(conversation) = &mut session.conversation
            && conversation.title.is_empty()
        {
            conversation.title = user_message
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(60)
                .collect();
            session.updated_at = self.ports.clock.now();
            self.ports.sessions.update_session(&session).await?;
        }
        // Session writes share the selection gate; executing a turn must not hold it.
        drop(access);
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

        let previous = match crate::conversation::history(
            self.ports.events.as_ref(),
            self.ports.tool_ledger.as_ref(),
            session_id,
            &turn_id,
        )
        .await
        {
            Ok(messages) => messages,
            Err(error) => return self.fail_turn(&mut turn, error.into()).await,
        };
        let orchestrator = match crate::orchestration::TurnOrchestrator::new(
            self.ports.clone(),
            session.clone(),
            turn_id.clone(),
            self.event_writer(),
            self.report_writing_skill.clone(),
            cancellation.clone(),
            self.agent_access.clone(),
        ) {
            Ok(orchestrator) => orchestrator,
            Err(error) => return self.fail_turn(&mut turn, error.into()).await,
        };
        let result = orchestrator
            .run(agent_messages(
                user_message,
                &knowledge,
                previous,
                active_graph_path.as_deref(),
                &self.report_writing_skill,
                yss_harness_contract::AgentRole::Manager,
            ))
            .await;

        if cancellation.reason() == Some(CancellationReason::DeadlineElapsed) {
            return self
                .fail_turn(
                    &mut turn,
                    HarnessError::Agent(AgentDriverFailureCode::DeadlineElapsed),
                )
                .await;
        }
        if cancellation.is_cancelled() {
            turn.state = HarnessTurnState::Cancelled;
            turn.finished_at = Some(self.ports.clock.now());
            self.ports.sessions.update_turn(&turn).await?;
            self.event_writer()
                .append(session_id, Some(&turn_id), HarnessEvent::TurnCancelled)
                .await?;
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
                Ok(result)
            }
            Err(error) => self.fail_turn(&mut turn, error.into()).await,
        }
    }

    pub fn cancel_turn(&self, session_id: &HarnessSessionId) -> bool {
        self.cancel_active_turn(session_id, CancellationReason::User)
    }

    /// Startup-only recovery. Read-only invocations from the previous process cannot still run.
    pub async fn recover_interrupted_turns(&self) -> Result<usize, HarnessError> {
        if !self
            .active_turns
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .is_empty()
        {
            return Err(HarnessError::ConcurrentTurn);
        }
        for mut record in self.ports.tool_ledger.load_running_invocations().await? {
            // An interrupted mutation has an unknown effect; do not imply that it rolled back.
            let failure_code = if record.capability_id.descriptor().effect
                != yss_harness_contract::ToolEffect::Inspect
            {
                CapabilityFailureCode::OutcomeUnknown
            } else {
                CapabilityFailureCode::InternalFailure
            };
            record.state = yss_harness_contract::ToolInvocationState::Failed;
            record.finished_at = Some(self.ports.clock.now());
            record.failure = Some(yss_harness_contract::CapabilityFailure::new(failure_code));
            self.ports.tool_ledger.finish(&record).await?;
            let failed = AgentEvent::ToolInvocationFailed {
                invocation_id: record.id,
                capability_id: record.capability_id,
                failure_code,
            };
            let event = if let Some(run_id) = record.agent_run_id {
                let manager = self.ports.events.load_events_after(&record.session_id, 0).await?
                    .iter().any(|entry| matches!(&entry.event,
                        HarnessEvent::AgentRunStarted { run_id: id, role: yss_harness_contract::AgentRole::Manager, .. } if id == &run_id));
                if manager {
                    HarnessEvent::Agent(failed)
                } else {
                    HarnessEvent::AgentRunOutput {
                        run_id,
                        event: failed,
                    }
                }
            } else {
                HarnessEvent::Agent(failed)
            };
            self.event_writer()
                .append(&record.session_id, Some(&record.turn_id), event)
                .await?;
        }
        let mut turns = self.ports.sessions.load_running_turns().await?;
        for turn in crate::orchestration::pending_agent_turns(&self.ports).await? {
            if !turns.iter().any(|existing| existing.id == turn.id) {
                turns.push(turn);
            }
        }
        let recovered = turns.len();
        for mut turn in turns {
            crate::orchestration::recover_runs(&self.ports, &self.event_writer(), &turn).await?;
            if turn.state != HarnessTurnState::Running {
                continue;
            }
            turn.state = HarnessTurnState::Failed;
            turn.finished_at = Some(self.ports.clock.now());
            self.ports.sessions.update_turn(&turn).await?;
            self.event_writer()
                .append(&turn.session_id, Some(&turn.id), HarnessEvent::TurnFailed)
                .await?;
        }
        Ok(recovered)
    }

    pub(super) async fn fail_turn<T>(
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
        Err(error)
    }

    pub(super) fn admit_turn(
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

    pub(super) fn cancel_active_turn(
        &self,
        session_id: &HarnessSessionId,
        reason: CancellationReason,
    ) -> bool {
        let token = self
            .active_turns
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(session_id)
            .cloned();
        // Waking a waiter can synchronously reenter the Host.
        token.is_some_and(|token| token.cancel(reason))
    }
}

fn validate_user_message(message: &str) -> Result<(), HarnessError> {
    if message.trim().is_empty() || message.len() > MAX_USER_MESSAGE_BYTES {
        Err(HarnessError::InvalidMessage)
    } else {
        Ok(())
    }
}

pub(super) struct TurnAdmission {
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
