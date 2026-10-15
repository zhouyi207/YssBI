//! Input layout composes existing draft commands, reference and model controls.
mod drafts;
mod input;

use super::ConversationPanel;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Enter,
};
use gpui_kit::{AnyElement, Context, IntoElement, Window, div, prelude::*, relative, rems};

impl ConversationPanel {
    pub(super) fn render_composer(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let (reference_button, picker) = self.reference_picker(window, cx);
        let card = div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .p_1()
            .border_1()
            .border_color(cx.theme().input)
            .rounded_lg()
            .bg(cx.theme().background)
            .shadow_xs()
            .when(!self.references.is_empty(), |card| {
                card.child(div().px_2().pt_1().child(self.reference_chips(
                    "draft-references",
                    self.references.iter().map(|r| (r, None)),
                    true,
                    cx,
                )))
            })
            .child(self.render_input(picker, cx))
            .child(self.render_controls(reference_button, window, cx));
        div()
            .id("assistant-composer")
            .on_action(cx.listener(Self::submit_input))
            .flex_shrink_0()
            .max_h(relative(0.75))
            .flex()
            .justify_center()
            .px_4()
            .pt_2()
            .pb_3()
            .bg(cx.theme().background)
            .child(
                div()
                    .id("assistant-composer-body")
                    .w_full()
                    .max_w(rems(48.))
                    .min_w_0()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(self.render_drafts(cx))
                    .child(card),
            )
            .into_any_element()
    }

    fn render_controls(
        &self,
        reference_button: AnyElement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let running = self.running();
        let empty = self.input.read(cx).value().trim().is_empty();
        div()
            .flex()
            .flex_wrap()
            .flex_shrink_0()
            .items_center()
            .gap_1()
            .child(reference_button)
            .child(self.mode_picker(cx))
            .child(self.effort_picker(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .justify_end()
                    .gap_1()
                    .child(self.usage_indicator(cx))
                    .child(self.model_picker(window, cx))
                    .when(running, |controls| {
                        controls.child(
                            Button::new("assistant-queue")
                                .small()
                                .ghost()
                                .icon(IconName::ListPlus)
                                .tooltip(yss_ui::text::t("panel.assistantQueueHint"))
                                .accessibility_label(yss_ui::text::t("panel.assistantQueueMessage"))
                                .disabled(
                                    empty
                                        || !self.ready
                                        || self.selecting
                                        || !self.model_available(),
                                )
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.queue_message(window, cx)
                                })),
                        )
                    })
                    .child(self.render_action_button(empty, cx)),
            )
            .into_any_element()
    }

    fn render_action_button(&self, empty: bool, cx: &mut Context<Self>) -> Button {
        let running = self.running();
        let button = if running {
            Button::new("assistant-stop")
                .small()
                .outline()
                .icon(IconName::Square)
                .tooltip(yss_ui::text::t("panel.assistantComposerStop"))
                .accessibility_label(yss_ui::text::t("panel.assistantCancel"))
                .on_click(cx.listener(|view, _, _, cx| view.cancel(cx)))
        } else {
            Button::new("assistant-send")
                .small()
                .primary()
                .icon(IconName::ArrowUp)
                .tooltip_with_action(
                    yss_ui::text::t("panel.assistantSend"),
                    &Enter {
                        secondary: false,
                        shift: false,
                    },
                    Some("Input"),
                )
                .accessibility_label(yss_ui::text::t("panel.assistantSend"))
                .disabled(empty || !self.can_send())
                .on_click(cx.listener(|view, _, window, cx| view.send(window, cx)))
        };
        if self.stopping {
            let label = yss_ui::text::t("native.assistant.waitingForStop");
            button
                .warning()
                .outline()
                .disabled(true)
                .loading(true)
                .tooltip(label.clone())
                .accessibility_label(label)
        } else if self.refreshing || (!self.ready && !running) {
            let failed = !self.refreshing && self.stream_error.is_some();
            let label = yss_ui::text::t(if failed {
                "panel.assistantStatusError"
            } else if self.catalog.is_none() {
                "panel.assistantStatusInitializing"
            } else {
                "native.assistant.restoring"
            });
            button
                .info()
                .outline()
                .when(failed, |button| {
                    button.warning().icon(IconName::TriangleAlert)
                })
                .disabled(true)
                .loading(!failed)
                .tooltip(label.clone())
                .accessibility_label(label)
        } else {
            button
        }
    }
}
