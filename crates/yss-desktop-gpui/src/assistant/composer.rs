//! Composer controls use the original model catalog, options and project resource identities.
use super::ConversationPanel;
use gpui::{AnyElement, Context, IntoElement, Window, div, prelude::*, px};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Textarea,
};

impl ConversationPanel {
    pub(super) fn render_composer(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut composer = div()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .flex_shrink_0()
            .border_t_1()
            .border_color(cx.theme().border);
        if let Some(message) = &self.unsent {
            composer = composer.child(
                div()
                    .p_2()
                    .rounded_md()
                    .bg(cx.theme().muted)
                    .text_xs()
                    .child("未确认的原文已保留，请检查历史后再决定是否重发。")
                    .child(
                        div()
                            .max_h(px(65.))
                            .overflow_hidden()
                            .child(message.text.clone()),
                    )
                    .child(self.reference_chips(
                        "unsent-references",
                        message.resources.iter().map(|resource| (resource, None)),
                        false,
                        cx,
                    ))
                    .child(
                        Button::new("assistant-restore")
                            .small()
                            .ghost()
                            .label("恢复到输入区")
                            .on_click(
                                cx.listener(|view, _, window, cx| view.restore_unsent(window, cx)),
                            ),
                    ),
            );
        }
        if !self.queue.is_empty() {
            let mut queued = div()
                .flex()
                .flex_col()
                .gap_1()
                .p_2()
                .rounded_md()
                .bg(cx.theme().muted);
            for message in &self.queue {
                let id = message.id;
                queued = queued.child(
                    div()
                        .flex()
                        .gap_2()
                        .items_center()
                        .child(
                            div()
                                .text_xs()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .child(div().truncate().child(message.text.clone()))
                                .child(self.reference_chips(
                                    format!("queued-references-{id}"),
                                    message.resources.iter().map(|resource| (resource, None)),
                                    false,
                                    cx,
                                )),
                        )
                        .child(
                            Button::new(gpui::SharedString::from(format!("queued-{id}")))
                                .small()
                                .ghost()
                                .label("移除")
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    view.queue.retain(|message| message.id != id);
                                    cx.notify();
                                })),
                        ),
                );
            }
            composer = composer.child(
                queued.child(
                    Button::new("assistant-send-queued")
                        .small()
                        .ghost()
                        .label("发送下一条")
                        .disabled(!self.can_send())
                        .on_click(cx.listener(|view, _, window, cx| view.send_queued(window, cx))),
                ),
            );
        }
        composer = composer.child(self.reference_chips(
            "draft-references",
            self.references.iter().map(|resource| (resource, None)),
            true,
            cx,
        ));
        composer
            .child(Textarea::new(&self.input).bordered(false).appearance(false))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_1()
                    .child(self.usage_indicator(cx))
                    .child(self.model_picker(window, cx))
                    .child(self.mode_picker(cx))
                    .child(self.effort_picker(cx))
                    .child(self.reference_picker(window, cx))
                    .child(div().flex_1())
                    .child(
                        Button::new("assistant-queue")
                            .small()
                            .ghost()
                            .label("加入队列")
                            .disabled(
                                !self.ready
                                    || self.selecting
                                    || self.input.read(cx).value().trim().is_empty(),
                            )
                            .on_click(
                                cx.listener(|view, _, window, cx| view.queue_message(window, cx)),
                            ),
                    )
                    .child(if self.running() {
                        Button::new("assistant-stop")
                            .small()
                            .danger()
                            .label(if self.stopping {
                                "正在停止"
                            } else {
                                "停止"
                            })
                            .disabled(self.stopping)
                            .on_click(cx.listener(|view, _, _, cx| view.cancel(cx)))
                    } else {
                        Button::new("assistant-send")
                            .small()
                            .primary()
                            .label("发送")
                            .disabled(
                                !self.can_send() || self.input.read(cx).value().trim().is_empty(),
                            )
                            .on_click(cx.listener(|view, _, window, cx| view.send(window, cx)))
                    }),
            )
            .into_any_element()
    }
}
