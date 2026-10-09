use super::*;
use crate::assistant::ConversationEvent;
use gpui::{Empty, SharedString, px};

impl ConversationPanel {
    pub(super) fn render_drafts(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.unsent.is_none()
            && self.queue.is_empty()
            && !self.interrupted()
            && self.error.is_none()
            && self.stream_error.is_none()
        {
            return Empty.into_any_element();
        }
        let mut drafts = div().flex().flex_col().gap_2().flex_shrink_0();
        if !self.running()
            && let Some(message) = &self.unsent
        {
            drafts = drafts.child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(cx.theme().muted)
                    .text_xs()
                    .child(crate::text::t("panel.assistantUnsentMessage"))
                    .child(
                        div()
                            .max_h(px(60.))
                            .overflow_hidden()
                            .child(message.text.clone()),
                    )
                    .child(self.reference_chips(
                        "unsent-references",
                        message.resources.iter().map(|r| (r, None)),
                        false,
                        cx,
                    ))
                    .child(
                        Button::new("assistant-restore")
                            .small()
                            .ghost()
                            .label(crate::text::t("panel.assistantRestoreDraft"))
                            .disabled(!self.ready || self.selecting)
                            .on_click(
                                cx.listener(|view, _, window, cx| view.restore_unsent(window, cx)),
                            ),
                    ),
            )
        }
        if !self.queue.is_empty() {
            drafts = drafts.child(self.render_queue(cx));
        }
        if !self.running()
            && (self.error.is_some() || self.stream_error.is_some() || self.interrupted())
        {
            drafts = drafts.child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .when(self.stream_error.is_some(), |actions| {
                        actions.child(
                            Button::new("assistant-reconnect")
                                .small()
                                .label(crate::text::t("panel.assistantReloadConversations"))
                                .disabled(self.refreshing)
                                .on_click(
                                    cx.listener(|view, _, window, cx| view.refresh(window, cx)),
                                ),
                        )
                    })
                    .when(self.interrupted(), |actions| {
                        actions.child(
                            Button::new("assistant-continue")
                                .small()
                                .label(crate::text::t("panel.assistantContinueTask"))
                                .disabled(!self.can_send())
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.continue_task(window, cx)
                                })),
                        )
                    })
                    .child(
                        Button::new("assistant-recovery-settings")
                            .small()
                            .ghost()
                            .label(crate::text::t("panel.assistantOpenSettings"))
                            .on_click(
                                cx.listener(|_, _, _, cx| cx.emit(ConversationEvent::Settings)),
                            ),
                    ),
            );
        }
        drafts.into_any_element()
    }

    fn render_queue(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut rows = div()
            .id("assistant-queued-messages")
            .max_h(px(112.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1();
        for message in &self.queue {
            let id = message.id;
            rows = rows.child(
                div()
                    .flex()
                    .gap_2()
                    .items_start()
                    .child(
                        div()
                            .text_xs()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .max_h(px(40.))
                                    .overflow_hidden()
                                    .child(message.text.clone()),
                            )
                            .child(self.reference_chips(
                                format!("queued-references-{id}"),
                                message.resources.iter().map(|r| (r, None)),
                                false,
                                cx,
                            )),
                    )
                    .child(
                        Button::new(SharedString::from(format!("queued-{id}")))
                            .xsmall()
                            .ghost()
                            .icon(IconName::X)
                            .tooltip(crate::text::t("panel.assistantRemoveQueued"))
                            .accessibility_label(crate::text::t("panel.assistantRemoveQueued"))
                            .on_click(cx.listener(move |view, _, _, cx| {
                                view.queue.retain(|message| message.id != id);
                                cx.notify();
                            })),
                    ),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .rounded_md()
            .bg(cx.theme().muted)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_xs().child(crate::text::format(
                        "panel.assistantQueuedMessages",
                        &[("count", self.queue.len().to_string())],
                    )))
                    .when(!self.running(), |header| {
                        header.child(
                            Button::new("assistant-send-queued")
                                .xsmall()
                                .ghost()
                                .label(crate::text::t("panel.assistantSendQueued"))
                                .disabled(!self.can_send_queued())
                                .on_click(
                                    cx.listener(|view, _, window, cx| view.send_queued(window, cx)),
                                ),
                        )
                    }),
            )
            .child(rows)
            .into_any_element()
    }
}
