use super::*;
use crate::assistant::references::Picker;
use gpui_kit::component::input::Textarea;
use gpui_kit::{Entity, EntityInputHandler, Focusable, FontWeight, KeyDownEvent};

impl ConversationPanel {
    pub(super) fn render_starters(&self, cx: &mut Context<Self>) -> AnyElement {
        if self.running() || !self.input.read(cx).value().is_empty() {
            return gpui_kit::Empty.into_any_element();
        }
        let mut prompts = div().flex().flex_wrap().gap_2();
        for (id, label, prompt) in [
            (
                "assistant-starter-question",
                "panel.assistantStarterQuestionLabel",
                "panel.assistantStarterQuestionPrompt",
            ),
            (
                "assistant-starter-quality",
                "panel.assistantStarterQualityLabel",
                "panel.assistantStarterQualityPrompt",
            ),
            (
                "assistant-starter-method",
                "panel.assistantStarterMethodLabel",
                "panel.assistantStarterMethodPrompt",
            ),
        ] {
            prompts = prompts.child(
                Button::new(id)
                    .small()
                    .outline()
                    .label(yss_ui::text::t(label))
                    .on_click(cx.listener(move |view, _, window, cx| {
                        // A starter never replaces authored text or submits a request.
                        if view.input.read(cx).value().is_empty() {
                            view.input.update(cx, |input, cx| {
                                input.set_value(yss_ui::text::t(prompt), window, cx);
                            });
                        }
                        view.input.focus_handle(cx).focus(window, cx);
                    })),
            );
        }
        div()
            .id("assistant-start-guide")
            .flex()
            .flex_col()
            .flex_shrink_0()
            .min_w_0()
            .gap_2()
            .py_3()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(yss_ui::text::t("panel.assistantStartTitle")),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(yss_ui::text::t("panel.assistantEmptyDescription")),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(yss_ui::text::t("panel.assistantStarterHint")),
            )
            .child(prompts)
            .into_any_element()
    }

    pub(in crate::assistant) fn sync_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        let placeholder = yss_ui::text::t(if self.running() {
            "panel.assistantComposerWhileRunning"
        } else {
            "native.assistant.prompt"
        });
        if self.input.read(cx).presentation().placeholder().as_ref() != placeholder {
            self.input.update(cx, |input, cx| {
                input.set_placeholder(placeholder, window, cx)
            });
        }
    }

    pub(super) fn render_input(
        &self,
        empty: bool,
        picker: Entity<Picker>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let label = yss_ui::text::t(if self.input_expanded {
            "panel.assistantCollapseInput"
        } else {
            "panel.assistantExpandInput"
        });
        div()
            .flex()
            .flex_col()
            .min_h_0()
            .min_w_0()
            .when(empty, |input| input.flex_1())
            .capture_key_down(cx.listener(move |view, event, window, cx| {
                view.reference_key(event, &picker, window, cx)
            }))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(yss_ui::text::t("panel.assistantComposerLabel")),
                    )
                    .when(!empty, |header| {
                        header.child(
                            Button::new("assistant-expand-input")
                                .small()
                                .ghost()
                                .icon(if self.input_expanded {
                                    IconName::Minimize
                                } else {
                                    IconName::Maximize
                                })
                                .tooltip(label.clone())
                                .accessibility_label(label)
                                .on_click(cx.listener(|view, _, window, cx| {
                                    view.input_expanded = !view.input_expanded;
                                    view.input.update(cx, |input, cx| {
                                        input.set_auto_grow(
                                            if view.input_expanded { 10 } else { 3 },
                                            if view.input_expanded { 20 } else { 10 },
                                            cx,
                                        )
                                    });
                                    view.input.focus_handle(cx).focus(window, cx);
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(
                Textarea::new(&self.input)
                    .bordered(false)
                    .appearance(false)
                    .aria_label(yss_ui::text::t("panel.assistantComposerLabel"))
                    .when(empty, |input| input.h(relative(1.)).flex_1().min_h_0()),
            )
            .into_any_element()
    }

    fn reference_key(
        &self,
        event: &KeyDownEvent,
        picker: &Entity<Picker>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let key = &event.keystroke;
        if (key.key_char.as_deref() != Some("@") && key.key != "@")
            || key.modifiers.control
            || key.modifiers.alt
            || key.modifiers.platform
            || self.resource_catalog.is_none()
            || !self.input.focus_handle(cx).is_focused(window)
        {
            return;
        }
        let eligible = self.input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_none()
                && input
                    .value()
                    .get(..input.selected_range().start)
                    .is_some_and(|prefix| {
                        prefix.chars().next_back().is_none_or(char::is_whitespace)
                    })
        });
        if eligible {
            picker.update(cx, |picker, cx| picker.set_open(true, window, cx));
            cx.stop_propagation();
            cx.notify();
        }
    }

    pub(super) fn submit_input(
        &mut self,
        action: &Enter,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if action.shift || action.secondary || !self.input.focus_handle(cx).is_focused(window) {
            cx.propagate();
            return;
        }
        if !self.input.update(cx, |input, cx| {
            input.marked_text_range(window, cx).is_some()
        }) {
            self.send(window, cx);
        }
    }
}
