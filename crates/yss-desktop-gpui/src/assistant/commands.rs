//! User submission and model selection call the existing session and turn owners.
use super::{ConversationEvent, ConversationPanel, DraftMessage, Submission};
use gpui::{Context, Window};
use yss_application::harness::HarnessSessionError;
use yss_harness_contract::LanguageModelSelection;
use yss_harness_core::HarnessError;

impl ConversationPanel {
    pub(super) fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.running() {
            self.queue_message(window, cx);
            return;
        }
        if !self.can_send() {
            return;
        }
        let Some(message) = self.capture_message(cx) else {
            return;
        };
        self.input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.references.clear();
        self.submit(message, window, cx);
    }
    pub(super) fn submit(
        &mut self,
        message: DraftMessage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_submit(&message) {
            self.unsent = Some(message);
            cx.notify();
            return;
        }
        let id = message.id;
        let services = self.services.clone();
        let session = self.session.clone();
        let principal = self.principal.clone();
        let after_sequence = self.transcript.sequence;
        self.pending = Some(Submission {
            message: message.clone(),
            after_sequence: self.transcript.sequence,
            accepted: false,
        });
        self.error = None;
        self.queue_paused = false;
        let job = self.services.executor.spawn(async move {
            let requested = message.clone();
            let result = async {
                services
                .application
                .application
                .validate_harness_session(
                    &services.application.harness.host,
                    &principal,
                    &session.id,
                )
                .await
                .map_err(session_failure)?;
            services
                .application
                .harness
                .host
                .submit_turn(
                    &session.id,
                    &session.project,
                    message.text,
                    message.resources,
                    message.model,
                    message.options,
                )
                .await
                .map_err(harness_failure)
            }.await;
            let accepted = if result.is_ok() { true } else if services.application.application
                .validate_harness_session(&services.application.harness.host, &principal, &session.id).await.is_ok() {
                services.application.harness.host.events_after(&session.id, after_sequence).await
                    .is_ok_and(|events| events.iter().any(|event| matches!(&event.event,
                        yss_harness_contract::HarnessEvent::TurnStarted { user_message, resources, .. }
                        if user_message == &requested.text && resources.iter().map(|reference| &reference.resource).eq(requested.resources.iter()))))
            } else { false };
            (result, accepted)
        });
        cx.spawn_in(window, async move |view, cx| {
            let (result, accepted) = job.await.unwrap_or_else(|_| {
                (
                    Err(crate::text::t("native.assistant.submitUnconfirmed").into()),
                    false,
                )
            });
            let _ = view.update_in(cx, |view, window, cx| {
                let Some(pending) = view.pending.take() else {
                    return;
                };
                if pending.message.id != id {
                    view.pending = Some(pending);
                    return;
                }
                if let Err(error) = result {
                    view.queue_paused = true;
                    view.error = Some(error);
                    if !pending.accepted && !accepted {
                        view.unsent = Some(pending.message);
                    }
                }
                view.stopping = false;
                view.reload(false, window, cx);
                cx.emit(ConversationEvent::DirectoryChanged);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
    pub(super) fn cancel(&mut self, cx: &mut Context<Self>) {
        if !self.running() || self.stopping {
            return;
        }
        self.queue_paused = true;
        self.stopping = self
            .services
            .application
            .harness
            .host
            .cancel_turn(&self.session.id);
        cx.notify();
    }
    pub(super) fn select_model(
        &mut self,
        model: LanguageModelSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.ready || self.selecting || !self.model_available_for(&model) {
            cx.notify();
            return;
        }
        self.selecting = true;
        self.selection_generation = self.selection_generation.wrapping_add(1);
        let generation = self.selection_generation;
        let services = self.services.clone();
        let principal = self.principal.clone();
        let id = self.session.id.clone();
        let job = self.services.executor.spawn(async move {
            services
                .application
                .application
                .select_harness_model(&services.application.harness.host, &principal, &id, model)
                .await
                .map_err(session_failure)
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.unwrap_or_else(|_| {
                Err(crate::text::t("native.assistant.modelUnconfirmed").into())
            });
            let _ = view.update_in(cx, |view, _, cx| {
                if view.selection_generation != generation {
                    return;
                }
                view.selecting = false;
                match result {
                    Ok(session) => {
                        view.update_session(session);
                        view.error = None;
                    }
                    Err(error) => view.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}
pub(super) fn session_failure(error: HarnessSessionError) -> String {
    match error {
        HarnessSessionError::Host(error) => harness_failure(error),
        _ => crate::text::t("native.assistant.sessionChanged").into(),
    }
}
pub(super) fn harness_failure(error: HarnessError) -> String {
    let code = match error {
        HarnessError::Agent(code) => code.to_string(),
        HarnessError::ConcurrentTurn => {
            return crate::text::t("native.assistant.alreadyRunning").into();
        }
        HarnessError::InvalidMessage => {
            return crate::text::t("native.assistant.invalidMessage").into();
        }
        HarnessError::SessionNotActive | HarnessError::SessionNotFound => {
            return crate::text::t("native.assistant.conversationExpired").into();
        }
        HarnessError::Cancelled => "cancelled".into(),
        _ => return crate::text::t("native.assistant.operationFailed").into(),
    };
    failure_text(&code)
}
pub(super) fn failure_text(code: &str) -> String {
    let public_code = match code {
        "provider_authentication_failed" => "assistant_authentication_failed".to_owned(),
        "provider_rate_limited" => "assistant_rate_limited".to_owned(),
        "provider_transport_failed" => "assistant_provider_connection_failed".to_owned(),
        "internal_failure" | "output_unavailable" => "assistant_turn_failed".to_owned(),
        _ => format!("assistant_{code}"),
    };
    for key in [
        format!("panel.assistantErrors.{code}"),
        format!("panel.assistantErrors.{public_code}"),
        format!("panel.assistantBlockedReasons.{code}"),
    ] {
        let localized = crate::text::translate(&key);
        if localized != key {
            return localized;
        }
    }
    code.to_owned()
}
