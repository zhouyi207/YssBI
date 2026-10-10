use super::*;
use crate::assistant::references::Picker;
use gpui_kit::component::input::Textarea;
use gpui_kit::{Entity, EntityInputHandler, Focusable, KeyDownEvent};

impl ConversationPanel {
    pub(in crate::assistant) fn sync_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        let placeholder = crate::text::t(if self.running() {
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
        let label = crate::text::t(if self.input_expanded {
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
            .when(!empty, |input| {
                input.child(
                    div().flex().justify_end().child(
                        Button::new("assistant-expand-input")
                            .xsmall()
                            .ghost()
                            .icon(if self.input_expanded {
                                IconName::Minimize
                            } else {
                                IconName::Maximize
                            })
                            .tooltip(label)
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
                    ),
                )
            })
            .child(
                Textarea::new(&self.input)
                    .bordered(false)
                    .appearance(false)
                    .aria_label(crate::text::t("panel.assistantComposerLabel"))
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
