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
use gpui_kit::{AnyElement, Context, IntoElement, Window, div, prelude::*, relative};

impl ConversationPanel {
    pub(super) fn render_composer(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let empty =
            self.transcript.turns.is_empty() && self.transcript.session_output.parts.is_empty();
        let (reference_button, picker) = self.reference_picker(window, cx);
        let editor = div()
            .id("assistant-composer-body")
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_3()
            .when(empty, |editor| {
                editor.flex_1().child(self.render_starters(cx))
            })
            .child(self.render_drafts(cx))
            .child(
                div()
                    .min_w_0()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_2()
                    .border_1()
                    .border_color(cx.theme().input)
                    .rounded_md()
                    .bg(cx.theme().background)
                    .when(empty, |input| input.flex_1())
                    .when(!self.references.is_empty(), |input| {
                        input.child(self.reference_chips(
                            "draft-references",
                            self.references.iter().map(|r| (r, None)),
                            true,
                            cx,
                        ))
                    })
                    .child(self.render_input(empty, picker, cx)),
            );
        div()
            .id("assistant-composer")
            .on_action(cx.listener(Self::submit_input))
            .flex()
            .flex_col()
            .min_w_0()
            .min_h_0()
            .gap_2()
            .p_3()
            .bg(cx.theme().background)
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
            .when(
                self.refreshing || self.stopping || !self.ready || !self.model_available(),
                |composer| {
                    composer.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_center()
                            .gap_2()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(yss_ui::text::t(if self.refreshing {
                                "native.assistant.restoring"
                            } else if self.stopping {
                                "native.assistant.waitingForStop"
                            } else if !self.ready {
                                "panel.assistantStatusInitializing"
                            } else {
                                "panel.assistantStatusProviderUnavailable"
                            }))
                            .when(self.ready && !self.model_available(), |status| {
                                status.child(
                                    Button::new("assistant-configure-model")
                                        .small()
                                        .ghost()
                                        .label(yss_ui::text::t("panel.assistantOpenSettings"))
                                        .on_click(cx.listener(|_, _, _, cx| {
                                            cx.emit(super::ConversationEvent::Settings);
                                        })),
                                )
                            }),
                    )
                },
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(yss_ui::text::t(if self.running() {
                        "panel.assistantComposerQueueKeys"
                    } else {
                        "panel.assistantComposerSendKeys"
                    })),
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
            .gap_2()
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
                    .gap_2()
                    .child(self.usage_indicator(cx))
                    .child(self.mode_picker(cx))
                    .child(self.model_picker(window, cx))
                    .when(running, |controls| {
                        controls.child(
                            Button::new("assistant-queue")
                                .small()
                                .outline()
                                .icon(IconName::ListPlus)
                                .label(yss_ui::text::t("panel.assistantQueueMessage"))
                                .tooltip(yss_ui::text::t("panel.assistantQueueHint"))
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
                    .child(if running {
                        Button::new("assistant-stop")
                            .small()
                            .outline()
                            .icon(IconName::Square)
                            .label(yss_ui::text::t("panel.assistantComposerStop"))
                            .accessibility_label(yss_ui::text::t("panel.assistantCancel"))
                            .disabled(self.stopping)
                            .loading(self.stopping)
                            .when(self.stopping, |button| {
                                button.tooltip(yss_ui::text::t("native.assistant.waitingForStop"))
                            })
                            .on_click(cx.listener(|view, _, _, cx| view.cancel(cx)))
                    } else {
                        Button::new("assistant-send")
                            .small()
                            .primary()
                            .icon(IconName::ArrowUp)
                            .label(yss_ui::text::t("panel.assistantSend"))
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
                            .loading(self.refreshing)
                            .when(self.refreshing, |button| {
                                button.tooltip(yss_ui::text::t("native.assistant.restoring"))
                            })
                            .on_click(cx.listener(|view, _, window, cx| view.send(window, cx)))
                    }),
            )
            .into_any_element()
    }
}
