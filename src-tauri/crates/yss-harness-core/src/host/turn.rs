use crate::conversation::agent_messages;
use crate::{HarnessError, HarnessHost};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use yss_harness_contract::{
    AgentDriverFailureCode, AgentEvent, AgentTurnResult, AutomationIdKind, CancellationReason,
    CancellationToken, CapabilityFailureCode, HarnessEvent, HarnessSessionId, HarnessSessionState,
    HarnessTurnId, HarnessTurnRecord, HarnessTurnState, ProjectSessionBinding,
};

const MAX_USER_MESSAGE_BYTES: usize = 64 * 1024;

impl HarnessHost {
    pub async fn submit_turn(
        &self,
        session_id: &HarnessSessionId,
        expected_project: &ProjectSessionBinding,
        user_message: String,
        resources: Vec<yss_harness_contract::ProjectResourceRef>,
        model: Option<yss_harness_contract::LanguageModelSelection>,
        options: yss_harness_contract::HarnessTurnOptions,
    ) -> Result<AgentTurnResult, HarnessError> {
        validate_user_message(&user_message)?;
        let mut selected = std::collections::BTreeSet::new();
        if resources.iter().any(|entry| !selected.insert(entry)) {
            return Err(HarnessError::InvalidMessage);
        }
        let (cancellation, _admission) = self.admit_turn(session_id)?;
        let access = self.session_access().await;
        let session = self
            .ports
            .sessions
            .load_session(session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        if session.state != HarnessSessionState::Active || &session.project != expected_project {
            return Err(HarnessError::SessionNotActive);
        }
        let requested_model = model.or_else(|| {
            session
                .conversation
                .as_ref()
                .and_then(|value| value.model.clone())
        });
        // Reference lookup and credential access may perform I/O. They must be
        // cancellable and must not block model selection or project replacement.
        drop(access);
        let preparation = async {
            let references = if resources.is_empty() {
                Vec::new()
            } else {
                self.ports
                    .resources
                    .resolve(expected_project, &resources, cancellation.clone())
                    .await?
            };
            if references.len() != resources.len()
                || references.iter().zip(&resources).any(|(entry, requested)| {
                    &entry.resource != requested
                        || entry.name.trim().is_empty()
                        || entry.validate().is_err()
                })
            {
                return Err(HarnessError::Capability(
                    yss_harness_contract::CapabilityFailure::new(
                        CapabilityFailureCode::InternalFailure,
                    ),
                ));
            }
            let resolved = self.ports.models.resolve(requested_model.as_ref()).await?;
            Ok::<_, HarnessError>((references, resolved))
        };
        let (resources, resolved) = tokio::select! {
            biased;
            _ = cancellation.cancelled() => return Err(HarnessError::Cancelled),
            result = preparation => result?,
        };
        let access = self.session_access().await;
        let mut session = self
            .ports
            .sessions
            .load_session(session_id)
            .await?
            .ok_or(HarnessError::SessionNotFound)?;
        if cancellation.is_cancelled() {
            return Err(HarnessError::Cancelled);
        }
        if session.state != HarnessSessionState::Active || &session.project != expected_project {
            return Err(HarnessError::SessionNotActive);
        }
        if let Some(conversation) = &mut session.conversation
            && conversation.model.is_none()
        {
            // A queued turn carries its own model; it must not replace the user's
            // newer selection for subsequent turns.
            conversation.model = Some(resolved.identity.selection.clone());
        }
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
        }
        session.updated_at = self.ports.clock.now();
        self.ports.sessions.update_session(&session).await?;
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
                    model: resolved.identity,
                    resources: resources.clone(),
                },
            )
            .await
        {
            turn.state = HarnessTurnState::Failed;
            turn.finished_at = Some(self.ports.clock.now());
            let _ = self.ports.sessions.update_turn(&turn).await;
            return Err(error);
        }

        if let Err(error) = self
            .event_writer()
            .append(
                session_id,
                Some(&turn_id),
                HarnessEvent::TurnConfigured { options },
            )
            .await
        {
            return self.fail_turn(&mut turn, error).await;
        }
        let previous = match crate::conversation::history(
            self.ports.events.as_ref(),
            self.ports.tool_ledger.as_ref(),
            session_id,
            &turn_id,
            &session.project,
        )
        .await
        {
            Ok(messages) => messages,
            Err(error) => return self.fail_turn(&mut turn, error.into()).await,
        };
        let orchestrator = match crate::orchestration::TurnOrchestrator::new(
            self.ports.clone(),
            resolved.driver,
            self.knowledge.clone(),
            session.clone(),
            turn_id.clone(),
            self.event_writer(),
            self.report_writing_skill.clone(),
            cancellation.clone(),
            self.agent_access.clone(),
            options,
        ) {
            Ok(orchestrator) => orchestrator,
            Err(error) => return self.fail_turn(&mut turn, error.into()).await,
        };
        let result = orchestrator
            .run(
                agent_messages(
                    user_message,
                    previous.messages,
                    &resources,
                    &self.report_writing_skill,
                    yss_harness_contract::AgentRole::Manager,
                ),
                previous.observations,
            )
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
                && record.request.bound().is_some()
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
                failure_details: None,
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
