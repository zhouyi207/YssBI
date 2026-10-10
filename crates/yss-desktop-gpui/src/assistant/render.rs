use super::{
    CancelResponse, ConversationPanel,
    projection::{Turn, TurnState},
};
use gpui_kit::component::{ActiveTheme, Sizable, clipboard::Clipboard};
use gpui_kit::{
    AnyElement, Context, Empty, IntoElement, Render, SharedString, Window, div, prelude::*,
};
mod failure;
mod output;
mod plain;
mod task;

impl Render for ConversationPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.refresh_timing(window, cx);
        self.sync_input(window, cx);
        div()
            .id("assistant-conversation")
            .key_context("AssistantConversation")
            .track_focus(&self.focus)
            .size_full()
            .min_w_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(cx.theme().background)
            .on_action(cx.listener(|view, _: &CancelResponse, _, cx| view.cancel(cx)))
            .child(self.render_thread(cx))
            .child(self.status(cx))
            .child(self.render_composer(window, cx))
    }
}
impl ConversationPanel {
    fn status(&self, cx: &mut Context<Self>) -> AnyElement {
        let latest = self.transcript.turns.last();
        let error = self.stream_error.as_ref().or(self.error.as_ref());
        let text = if let Some(error) = error {
            error.clone()
        } else if self.refreshing {
            crate::text::t("native.assistant.restoring").into()
        } else if !self.model_available() {
            crate::text::t("native.assistant.configureModelHint").into()
        } else if self.stopping {
            crate::text::t("native.assistant.waitingForStop").into()
        } else if self.running() {
            latest
                .and_then(|turn| turn.output.activity.as_ref())
                .map(super::activity::Activity::label)
                .unwrap_or_else(|| crate::text::t("native.assistant.processing").into())
        } else {
            return Empty.into_any_element();
        };
        div()
            .px_3()
            .py_1()
            .text_xs()
            .flex_shrink_0()
            .text_color(if error.is_some() {
                cx.theme().danger
            } else {
                cx.theme().muted_foreground
            })
            .child(text)
            .into_any_element()
    }
    pub(super) fn render_turn(&self, turn: &Turn, cx: &mut Context<Self>) -> AnyElement {
        let text = turn.user.clone();
        let id = &turn.id;
        let mut item = div().flex().flex_col().gap_3().min_w_0().child(
            div()
                .p_3()
                .rounded_md()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().tab_active)
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .child(
                            div()
                                .flex_1()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child(crate::text::t("panel.assistantYou")),
                        )
                        .when(!text.is_empty(), |row| {
                            row.child(self.copy_action(turn, true, cx))
                        }),
                )
                .child(plain::PlainText::new(
                    SharedString::from(format!("user-{id}")),
                    text,
                    "panel.assistantYou",
                ))
                .child(
                    self.reference_chips(
                        format!("history-references-{}", turn.id),
                        turn.resources
                            .iter()
                            .map(|reference| (&reference.resource, Some(reference.name.as_str()))),
                        false,
                        cx,
                    ),
                ),
        );
        item = item.child(self.execution_header(turn, cx));
        item = item.child(self.render_output(
            &turn.output,
            &turn.tasks,
            turn.state == TurnState::Running,
            cx,
        ));
        item = item.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .justify_end()
                .when(turn.state != TurnState::Completed, |footer| {
                    footer.child(match turn.state {
                        TurnState::Running => crate::text::t("native.assistant.generating"),
                        TurnState::Completed => "".into(),
                        TurnState::Failed if turn.output.failure().is_none() => {
                            crate::text::t("panel.assistantReplyInterrupted")
                        }
                        TurnState::Failed => "".into(),
                        TurnState::Cancelled => crate::text::t("panel.assistantReplyStopped"),
                    })
                })
                .when(
                    turn.state != TurnState::Running && turn.output.has_text(),
                    |footer| footer.child(self.copy_action(turn, false, cx)),
                ),
        );
        item.into_any_element()
    }

    fn copy_action(&self, turn: &Turn, user: bool, cx: &Context<Self>) -> Clipboard {
        let owner = cx.entity().downgrade();
        let id = turn.id.clone();
        let label = crate::text::t(if user {
            "native.assistant.copyMessage"
        } else {
            "panel.assistantCopy"
        });
        Clipboard::new(SharedString::from(format!("copy-{user}-{id}")))
            .xsmall()
            .tooltip(label.clone())
            .accessibility_label(label)
            .value_fn(move |_, cx| {
                owner
                    .upgrade()
                    .and_then(|owner| {
                        owner
                            .read(cx)
                            .transcript
                            .turns
                            .iter()
                            .find(|turn| turn.id == id)
                            .map(|turn| {
                                if user {
                                    turn.user.clone()
                                } else {
                                    turn.output.text()
                                }
                            })
                    })
                    .unwrap_or_default()
                    .into()
            })
    }
}
