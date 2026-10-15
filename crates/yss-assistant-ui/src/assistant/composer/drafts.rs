use super::*;
use crate::assistant::ConversationEvent;
use gpui_kit::{Empty, FontWeight, SharedString, rems};

impl ConversationPanel {
    pub(super) fn render_drafts(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.unsent.is_none() && self.queue.is_empty() && !self.interrupted() {
            return Empty.into_any_element();
        }
        let mut drafts = div().flex().flex_col().gap_2().flex_shrink_0().min_w_0();
        if !self.running()
            && let Some(message) = &self.unsent
        {
            drafts = drafts.child(
                div()
                    .flex()
                    .flex_col()
                    .min_w_0()
                    .gap_2()
                    .p_2()
                    .rounded_md()
                    .bg(cx.theme().muted)
                    .text_sm()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(yss_ui::text::t("panel.assistantUnsentMessage")),
                    )
                    .child(
                        div()
                            .id("assistant-unsent-text")
                            .max_h(rems(6.))
                            .overflow_y_scroll()
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
                            .label(yss_ui::text::t("panel.assistantRestoreDraft"))
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
                    .gap_2()
                    .when(self.stream_error.is_some(), |actions| {
                        actions.child(
                            Button::new("assistant-reconnect")
                                .xsmall()
                                .label(yss_ui::text::t("panel.assistantReloadConversations"))
                                .disabled(self.refreshing)
                                .on_click(
                                    cx.listener(|view, _, window, cx| view.refresh(window, cx)),
                                ),
                        )
                    })
                    .when(self.interrupted(), |actions| {
                        actions.child(
                            Button::new("assistant-continue")
                                .xsmall()
                                .label(yss_ui::text::t("panel.assistantContinueTask"))
                                .disabled(!self.can_send())
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.continue_task(window, cx)
                                })),
                        )
                    })
                    .child(
                        Button::new("assistant-recovery-settings")
                            .xsmall()
                            .ghost()
                            .label(yss_ui::text::t("panel.assistantOpenSettings"))
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
            .max_h(rems(9.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2();
        for message in &self.queue {
            let id = message.id;
            rows = rows.child(
                div()
                    .id(SharedString::from(format!("queued-message-{id}")))
                    .flex()
                    .gap_2()
                    .items_start()
                    .child(
                        div()
                            .text_sm()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().child(message.text.clone()))
                            .child(self.reference_chips(
                                format!("queued-references-{id}"),
                                message.resources.iter().map(|r| (r, None)),
                                false,
                                cx,
                            )),
                    )
                    .child(
                        Button::new(SharedString::from(format!("queued-{id}")))
                            .small()
                            .ghost()
                            .icon(IconName::X)
                            .tooltip(yss_ui::text::t("panel.assistantRemoveQueued"))
                            .accessibility_label(yss_ui::text::t("panel.assistantRemoveQueued"))
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
            .gap_2()
            .p_2()
            .rounded_md()
            .bg(cx.theme().muted)
            .child(
                div()
                    .flex()
                    .items_center()
                    .flex_wrap()
                    .gap_2()
                    .justify_between()
                    .child(div().text_xs().child(yss_ui::text::format(
                        "panel.assistantQueuedMessages",
                        &[("count", self.queue.len().to_string())],
                    )))
                    .when(!self.running(), |header| {
                        header.child(
                            Button::new("assistant-send-queued")
                                .small()
                                .ghost()
                                .label(yss_ui::text::t("panel.assistantSendQueued"))
                                .disabled(!self.can_send_queued())
                                .on_click(
                                    cx.listener(|view, _, window, cx| view.send_queued(window, cx)),
                                ),
                        )
                    }),
            )
            .when(self.queue_paused, |queue| {
                queue.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(yss_ui::text::t("panel.assistantQueuePaused")),
                )
            })
            .child(rows)
            .into_any_element()
    }
}
