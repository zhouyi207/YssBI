//! Draft restoration and queued messages retain the choices captured with their text.
use super::{ConversationEvent, ConversationPanel, DraftMessage, projection::TurnState};
use gpui::{App, Context, Focusable, Window};

impl ConversationPanel {
    pub(super) fn capture_message(&self, cx: &App) -> Option<DraftMessage> {
        let text = self.input.read(cx).value().to_string();
        (!text.trim().is_empty()).then(|| DraftMessage {
            id: uuid::Uuid::new_v4(),
            text,
            resources: self.references.clone(),
            model: self.selection(),
            options: self.options,
        })
    }

    pub(super) fn can_submit(&self, message: &DraftMessage) -> bool {
        self.ready
            && !self.selecting
            && !self.running()
            && message
                .model
                .as_ref()
                .is_some_and(|model| self.model_available_for(model))
    }

    pub(super) fn can_send_queued(&self) -> bool {
        self.queue
            .front()
            .is_some_and(|message| self.can_submit(message))
    }

    pub(super) fn queue_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.ready || self.selecting || !self.model_available() {
            return;
        }
        let Some(message) = self.capture_message(cx) else {
            return;
        };
        if self.running() && !self.stopping {
            self.queue_paused = false;
        }
        self.queue.push_back(message);
        self.input
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.references.clear();
        cx.notify();
    }

    pub(super) fn send_queued(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.can_send_queued()
            && let Some(message) = self.queue.pop_front()
        {
            self.submit(message, window, cx);
        }
    }

    pub(super) fn advance_queue(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.queue_paused || self.queue.is_empty() || self.running() || !self.ready {
            return;
        }
        if self.error.is_some()
            || self.stream_error.is_some()
            || !self
                .transcript
                .turns
                .last()
                .is_some_and(|turn| turn.state == TurnState::Completed)
            || !self.can_send_queued()
        {
            self.queue_paused = true;
            return;
        }
        self.send_queued(window, cx);
    }

    pub(super) fn restore_unsent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.running() || !self.ready || self.selecting {
            return;
        }
        let Some(message) = self.unsent.take() else {
            return;
        };
        let current = self.input.read(cx).value();
        let text = if current.is_empty() || current.as_str() == message.text {
            message.text
        } else {
            format!("{current}\n\n{}", message.text)
        };
        self.input
            .update(cx, |input, cx| input.set_value(text, window, cx));
        for reference in message.resources {
            if !self.references.contains(&reference) {
                self.references.push(reference);
            }
        }
        self.options = message.options;
        cx.emit(ConversationEvent::OptionsChanged);
        if let Some(model) = message.model {
            if self.model_available_for(&model) {
                if self.selection().as_ref() != Some(&model) {
                    self.select_model(model, window, cx);
                } else {
                    self.reconcile_effort();
                }
            } else {
                self.error =
                    Some(crate::text::t("panel.assistantStatusProviderUnavailable").into());
            }
        }
        self.input.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    pub(super) fn interrupted(&self) -> bool {
        self.transcript
            .turns
            .last()
            .is_some_and(|turn| matches!(turn.state, TurnState::Failed | TurnState::Cancelled))
    }

    pub(super) fn continue_task(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.can_send() || !self.interrupted() {
            return;
        }
        self.submit(
            DraftMessage {
                id: uuid::Uuid::new_v4(),
                text: crate::text::t("panel.assistantContinuePrompt").into(),
                resources: vec![],
                model: self.selection(),
                options: self.options,
            },
            window,
            cx,
        );
    }
}
