//! Subscribe before replay, merge only continuous sequences, and reject stale query delivery.
use super::{ConversationEvent, ConversationPanel, Submission, projection::Transcript};
use gpui_kit::{Context, Window};
use std::sync::Arc;
use tokio::sync::broadcast::error::RecvError;
use yss_assistant_contract::AssistantEvent;
use yss_desktop_services::DesktopEvent;
use yss_harness_contract::{HarnessEvent, HarnessEventEnvelope, HarnessSessionId};
use yss_ui::lifecycle::LogReleased;

fn accept_replay_event(
    transcript: &mut Transcript,
    session_id: &HarnessSessionId,
    event: &HarnessEventEnvelope,
) -> Result<(), ()> {
    if &event.session_id != session_id {
        return Err(());
    }
    transcript.accept(AssistantEvent::try_from(event).map_err(|_| ())?)
}

fn replay_history(
    session_id: &HarnessSessionId,
    events: &[HarnessEventEnvelope],
    mut pending: Option<Submission>,
) -> Result<(Transcript, Option<uuid::Uuid>), ()> {
    let mut transcript = Transcript::default();
    for event in events {
        accept_replay_event(&mut transcript, session_id, event)?;
        if let Some(pending) = &mut pending {
            pending.accept(event);
        }
    }
    let accepted = pending
        .filter(|pending| pending.accepted)
        .map(|pending| pending.message.id);
    Ok((transcript, accepted))
}

impl ConversationPanel {
    pub(super) fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut receiver = self.services.tasks.events().subscribe();
        self.event_task = Some(cx.spawn_in(window, async move |view, cx| {
            loop {
                let alive = match receiver.recv().await {
                    Ok(DesktopEvent::Harness(event)) => view
                        .update_in(cx, |view, window, cx| view.accept_live(event, window, cx))
                        .is_ok(),
                    Ok(DesktopEvent::ModelsChanged) => view
                        .update_in(cx, |view, window, cx| view.reload_models(window, cx))
                        .is_ok(),
                    Err(RecvError::Lagged(_)) => view
                        .update_in(cx, |view, window, cx| view.recover(window, cx))
                        .is_ok(),
                    Err(RecvError::Closed) => false,
                    _ => true,
                };
                if !alive {
                    break;
                }
            }
        }));
    }
    fn accept_live(
        &mut self,
        event: HarnessEventEnvelope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.session_id != self.session.id {
            return;
        }
        if self.refreshing {
            if self.buffered.len() < 512 {
                self.buffered.push(event);
            } else {
                self.overflow = true;
            }
            return;
        }
        if !self.ready {
            return;
        }
        self.accept_submission(&event);
        let directory_changed = matches!(event.event, HarnessEvent::TurnStarted { .. });
        if AssistantEvent::try_from(&event)
            .map_err(|_| ())
            .and_then(|event| self.transcript.accept(event))
            .is_err()
        {
            self.recover(window, cx);
            return;
        }
        if directory_changed {
            cx.emit(ConversationEvent::DirectoryChanged);
        }
        if !self.transcript.running() {
            self.stopping = false;
        }
        cx.notify();
    }
    fn recover(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.refreshing {
            self.overflow = true;
            return;
        }
        if self.recovering {
            self.ready = false;
            self.set_stream_error(
                yss_ui::text::t("native.assistant.eventsDiscontinuous").into(),
                cx,
            );
            cx.notify();
        } else {
            self.recovering = true;
            self.reload(true, window, cx);
        }
    }
    pub(crate) fn reload(&mut self, recovery: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.generation = self.generation.wrapping_add(1);
        self.selection_generation = self.selection_generation.wrapping_add(1);
        self.selecting = false;
        self.ready = false;
        self.refreshing = true;
        self.buffered.clear();
        self.overflow = false;
        if !recovery {
            self.recovering = false;
        }
        let generation = self.generation;
        let services = self.services.clone();
        let principal = self.principal.clone();
        let id = self.session.id.clone();
        let pending = self.pending.as_ref().map(|pending| Submission {
            message: pending.message.clone(),
            after_sequence: pending.after_sequence,
            accepted: pending.accepted,
        });
        let job = self.services.tasks.executor().spawn(async move {
            let session = services
                .tasks
                .application()
                .open_harness_session(&services.host, &principal, &id)
                .await
                .map_err(super::commands::session_failure)?;
            let events = services
                .host
                .events_after(&id, 0)
                .await
                .map_err(super::commands::harness_failure)?;
            let catalog =
                services.models.catalog().await.map_err(|_| {
                    yss_ui::text::t("native.assistant.catalogUnavailable").into_owned()
                })?;
            let replay_session = session.id.clone();
            let replay = services
                .tasks
                .executor()
                .spawn_blocking(move || replay_history(&replay_session, &events, pending))
                .await
                .map_err(|_| yss_ui::text::t("native.assistant.readFailed").into_owned())?;
            services
                .tasks
                .application()
                .validate_harness_session(&services.host, &principal, &id)
                .await
                .map_err(super::commands::session_failure)?;
            Ok::<_, String>((session, replay, catalog))
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job
                .await
                .unwrap_or_else(|_| Err(yss_ui::text::t("native.assistant.readFailed").into()));
            view.update_in(cx, |view, window, cx| {
                if view.generation != generation {
                    return;
                }
                view.refreshing = false;
                match result {
                    Ok((session, replay, catalog)) => {
                        if let Ok((mut candidate, accepted)) = replay
                            && !view.overflow
                            && view.buffered.iter().all(|event| {
                                accept_replay_event(&mut candidate, &session.id, event).is_ok()
                            })
                        {
                            if let Some(pending) = &mut view.pending
                                && accepted == Some(pending.message.id)
                            {
                                pending.accepted = true;
                            }
                            let buffered = std::mem::take(&mut view.buffered);
                            for event in &buffered {
                                view.accept_submission(event);
                            }
                            view.transcript = candidate;
                            view.viewport.reset(
                                view.transcript.turns.len(),
                                !view.transcript.session_output.parts.is_empty(),
                            );
                            view.catalog = Some(Arc::new(catalog));
                            view.update_session(session);
                            view.ready = true;
                            view.stream_error = None;
                            view.recovering = false;
                            view.stopping = false;
                            cx.emit(ConversationEvent::DirectoryChanged);
                        } else {
                            view.set_stream_error(
                                yss_ui::text::t("native.assistant.historyGap").into(),
                                cx,
                            );
                        }
                    }
                    Err(error) => {
                        view.set_stream_error(error, cx);
                    }
                }
                view.buffered.clear();
                view.advance_queue(window, cx);
                cx.notify();
            })
            .log_released();
        })
        .detach();
        cx.notify();
    }
    pub(crate) fn reload_models(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.model_generation = self.model_generation.wrapping_add(1);
        let generation = self.model_generation;
        let session_generation = self.generation;
        let service = self.services.models.clone();
        let job = self
            .services
            .tasks
            .executor()
            .spawn(async move { service.catalog().await });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            view.update_in(cx, |view, _, cx| {
                if view.model_generation != generation || view.generation != session_generation {
                    return;
                }
                if let Some(catalog) = result {
                    view.catalog = Some(Arc::new(catalog));
                } else {
                    view.catalog = None;
                    view.set_error(yss_ui::text::t("native.assistant.catalogFailed").into(), cx);
                }
                view.reconcile_effort();
                cx.notify();
            })
            .log_released();
        })
        .detach();
    }
    fn accept_submission(&mut self, event: &HarnessEventEnvelope) {
        if let Some(pending) = &mut self.pending {
            pending.accept(event);
        }
    }
}

impl Submission {
    fn accept(&mut self, event: &HarnessEventEnvelope) {
        if event.sequence <= self.after_sequence {
            return;
        }
        if let HarnessEvent::TurnStarted {
            user_message,
            resources,
            ..
        } = &event.event
            && user_message == &self.message.text
            && resources
                .iter()
                .map(|reference| &reference.resource)
                .eq(self.message.resources.iter())
        {
            self.accepted = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::{DraftMessage, projection::TurnState};
    use yss_harness_contract::{
        AgentEvent, HarnessMode, HarnessResourceReference, HarnessTurnId, HarnessTurnOptions,
        LanguageModelIdentity, LanguageModelSelection, ProjectResourceKind, ProjectResourceRef,
        UnixMillis,
    };

    fn session_id() -> HarnessSessionId {
        HarnessSessionId::try_new("replay-session").expect("valid session")
    }

    fn event(sequence: u64, event: HarnessEvent) -> HarnessEventEnvelope {
        HarnessEventEnvelope {
            sequence,
            session_id: session_id(),
            turn_id: (sequence > 1)
                .then(|| HarnessTurnId::try_new("replay-turn").expect("valid turn")),
            occurred_at: UnixMillis::from_existing(sequence * 100),
            event,
        }
    }

    fn history() -> Vec<HarnessEventEnvelope> {
        vec![
            event(1, HarnessEvent::SessionCreated),
            event(
                2,
                HarnessEvent::TurnStarted {
                    user_message: "Explain the result".into(),
                    model: LanguageModelIdentity {
                        selection: LanguageModelSelection {
                            provider_id: "offline".into(),
                            model_id: "fixture".into(),
                        },
                        provider_name: "Offline fixture".into(),
                        model_name: "Replay".into(),
                    },
                    resources: vec![],
                },
            ),
            event(
                3,
                HarnessEvent::Agent(AgentEvent::TextDelta {
                    delta: "First ".into(),
                }),
            ),
        ]
    }

    fn pending(after_sequence: u64) -> Submission {
        Submission {
            message: DraftMessage {
                id: uuid::Uuid::new_v4(),
                text: "Explain the result".into(),
                resources: vec![],
                model: None,
                options: HarnessTurnOptions {
                    mode: HarnessMode::Ask,
                    reasoning_effort: None,
                },
            },
            after_sequence,
            accepted: false,
        }
    }

    #[test]
    fn persisted_replay_merges_overlapping_live_output_without_duplicates() {
        let history = history();
        let (mut transcript, _) =
            replay_history(&session_id(), &history, None).expect("continuous history");
        let buffered = [
            history[1].clone(),
            history[2].clone(),
            event(
                4,
                HarnessEvent::Agent(AgentEvent::TextDelta {
                    delta: "answer".into(),
                }),
            ),
            event(
                5,
                HarnessEvent::TurnCompleted {
                    final_text: "First answer".into(),
                },
            ),
        ];
        for event in &buffered {
            accept_replay_event(&mut transcript, &session_id(), event).expect("continuous overlap");
        }
        assert_eq!(transcript.sequence, 5);
        assert_eq!(transcript.turns.len(), 1);
        assert_eq!(transcript.turns[0].user, "Explain the result");
        assert_eq!(transcript.turns[0].output.text(), "First answer");
        assert!(transcript.turns[0].state == TurnState::Completed);
        assert!(!transcript.running());
    }

    #[test]
    fn invalid_history_and_live_gaps_cannot_be_installed_as_complete_transcripts() {
        let mut events = history();
        events.remove(1);
        assert!(replay_history(&session_id(), &events, None).is_err());

        let mut events = history();
        events[1].session_id = HarnessSessionId::try_new("other-session").expect("valid session");
        assert!(replay_history(&session_id(), &events, None).is_err());

        let mut events = history();
        events[1].turn_id = None;
        assert!(replay_history(&session_id(), &events, None).is_err());

        let mut events = history();
        events.push(event(
            4,
            HarnessEvent::Agent(AgentEvent::Custom {
                invocation_id: None,
                kind: "unknown-notice".into(),
                payload: serde_json::Value::Null,
            }),
        ));
        assert!(replay_history(&session_id(), &events, None).is_err());

        let (mut transcript, _) =
            replay_history(&session_id(), &history(), None).expect("continuous history");
        assert!(
            accept_replay_event(
                &mut transcript,
                &session_id(),
                &event(5, HarnessEvent::TurnFailed),
            )
            .is_err()
        );
        assert_eq!(transcript.sequence, 3);
        assert!(transcript.running());
    }

    #[test]
    fn replay_retains_visible_failure_and_partial_response() {
        let mut events = history();
        events.push(event(
            4,
            HarnessEvent::Agent(AgentEvent::DeliveryBlocked {
                reason: "report_resource_not_saved".into(),
            }),
        ));
        events.push(event(5, HarnessEvent::TurnFailed));
        let (transcript, _) =
            replay_history(&session_id(), &events, None).expect("continuous failure history");
        assert_eq!(transcript.turns[0].output.text(), "First ");
        assert_eq!(
            transcript.turns[0].output.failure(),
            Some("report_resource_not_saved")
        );
        assert!(transcript.turns[0].state == TurnState::Failed);
        assert_eq!(transcript.turns[0].timing.finished_at, Some(500));
        assert!(!transcript.running());
    }

    #[test]
    fn replay_acknowledges_only_matching_submissions_after_the_captured_sequence() {
        let submission = pending(1);
        let id = submission.message.id;
        let (_, accepted) = replay_history(&session_id(), &history(), Some(submission))
            .expect("continuous history");
        assert_eq!(accepted, Some(id));

        let (_, accepted) = replay_history(&session_id(), &history(), Some(pending(2)))
            .expect("continuous history");
        assert_eq!(accepted, None);

        let mut submission = pending(1);
        submission.message.text = "Different message".into();
        let (_, accepted) = replay_history(&session_id(), &history(), Some(submission))
            .expect("continuous history");
        assert_eq!(accepted, None);

        let mut events = history();
        let HarnessEvent::TurnStarted { resources, .. } = &mut events[1].event else {
            panic!("turn start fixture");
        };
        resources.push(HarnessResourceReference {
            resource: ProjectResourceRef {
                kind: ProjectResourceKind::EventGraph,
                id: "selected-graph".into(),
            },
            name: "Graph".into(),
        });
        let (_, accepted) =
            replay_history(&session_id(), &events, Some(pending(1))).expect("continuous history");
        assert_eq!(accepted, None);
    }

    #[test]
    fn buffered_turn_start_can_acknowledge_a_submission_absent_from_persisted_history() {
        let history = history();
        let submission = pending(1);
        let (mut transcript, accepted) =
            replay_history(&session_id(), &history[..1], Some(submission))
                .expect("continuous history");
        assert_eq!(accepted, None);
        let mut current = pending(1);
        accept_replay_event(&mut transcript, &session_id(), &history[1])
            .expect("continuous buffered turn");
        current.accept(&history[1]);
        assert!(current.accepted);
        assert_eq!(transcript.turns[0].user, current.message.text);
    }
}
