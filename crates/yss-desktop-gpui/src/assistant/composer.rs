//! Input layout composes existing draft commands, reference and model controls.
mod drafts;
mod input;

use super::ConversationPanel;
use gpui::{AnyElement, Context, IntoElement, Window, div, prelude::*, relative};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Enter,
};
use gpui_kit_assets::IconName;

impl ConversationPanel {
    pub(super) fn render_composer(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let empty = self.transcript.turns.is_empty();
        let (reference_button, picker) = self.reference_picker(window, cx);
        let editor = div()
            .id("assistant-composer-body")
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2()
            .when(empty, |editor| editor.flex_1())
            .child(self.render_drafts(cx))
            .child(self.reference_chips(
                "draft-references",
                self.references.iter().map(|r| (r, None)),
                true,
                cx,
            ))
            .child(self.render_input(empty, picker, cx));
        div()
            .id("assistant-composer")
            .on_action(cx.listener(Self::submit_input))
            .flex()
            .flex_col()
            .min_w_0()
            .min_h_0()
            .gap_2()
            .p_2()
            .bg(cx.theme().tab_active)
            .when(empty, |composer| composer.flex_1())
            .when(!empty, |composer| {
                composer
                    .flex_shrink_0()
                    .max_h(relative(0.75))
                    .border_t_1()
                    .border_color(cx.theme().border)
            })
            .child(editor)
            .child(self.render_controls(reference_button, window, cx))
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
                    .child(self.mode_picker(cx))
                    .child(self.model_picker(window, cx))
                    .when(running && !empty, |controls| {
                        controls.child(
                            Button::new("assistant-queue")
                                .xsmall()
                                .ghost()
                                .icon(IconName::ListPlus)
                                .tooltip(crate::text::t("panel.assistantQueueHint"))
                                .accessibility_label(crate::text::t("panel.assistantQueueMessage"))
                                .disabled(!self.ready || self.selecting || !self.model_available())
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.queue_message(window, cx)
                                })),
                        )
                    })
                    .child(if running {
                        Button::new("assistant-stop")
                            .xsmall()
                            .danger()
                            .icon(IconName::Square)
                            .tooltip(crate::text::t("panel.assistantCancel"))
                            .accessibility_label(crate::text::t("panel.assistantCancel"))
                            .disabled(self.stopping)
                            .on_click(cx.listener(|view, _, _, cx| view.cancel(cx)))
                    } else {
                        Button::new("assistant-send")
                            .xsmall()
                            .primary()
                            .icon(IconName::ArrowUp)
                            .tooltip_with_action(
                                crate::text::t("panel.assistantSend"),
                                &Enter {
                                    secondary: false,
                                    shift: false,
                                },
                                Some("Input"),
                            )
                            .accessibility_label(crate::text::t("panel.assistantSend"))
                            .disabled(empty || !self.can_send())
                            .on_click(cx.listener(|view, _, window, cx| view.send(window, cx)))
                    }),
            )
            .into_any_element()
    }
}
