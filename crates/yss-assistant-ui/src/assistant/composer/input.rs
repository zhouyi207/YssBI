use super::*;
use crate::assistant::references::Picker;
use gpui_kit::component::{Icon, input::Textarea};
use gpui_kit::{Entity, EntityInputHandler, Focusable, FontWeight, KeyDownEvent};

fn reference_boundary(text: &Rope, offset: usize) -> bool {
    offset <= text.len()
        && text.is_char_boundary(offset)
        && text.chars_at(offset).prev().is_none_or(char::is_whitespace)
}

impl ConversationPanel {
    pub(in crate::assistant) fn render_welcome(&self, cx: &mut Context<Self>) -> AnyElement {
        let show_prompts = !self.running() && self.input.read(cx).text().len() == 0;
        let mut prompts = div().flex().flex_col().items_stretch().gap_1().w_full();
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
                    .outline()
                    .w_full()
                    .justify_start()
                    .icon(IconName::ArrowRight)
                    .label(yss_ui::text::t(label))
                    .on_click(cx.listener(move |view, _, window, cx| {
                        // A starter never replaces authored text or submits a request.
                        if view.input.read(cx).text().len() == 0 {
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
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .px_6()
            .py_6()
            .child(
                div()
                    .w_full()
                    .max_w(rems(26.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_3()
                    .text_center()
                    .child(
                        Icon::new(IconName::Sparkles)
                            .size_8()
                            .text_color(cx.theme().muted_foreground),
                    )
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
                    .when(show_prompts, |guide| guide.child(div().pt_2().w_full().child(prompts))),
            )
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

    pub(super) fn render_input(&self, picker: Entity<Picker>, cx: &mut Context<Self>) -> AnyElement {
        div()
            .min_h_0()
            .min_w_0()
            .px_2()
            .pt_1()
            .capture_key_down(cx.listener(move |view, event, window, cx| {
                view.reference_key(event, &picker, window, cx)
            }))
            .child(
                Textarea::new(&self.input)
                    .bordered(false)
                    .appearance(false)
                    .p_0()
                    .aria_label(yss_ui::text::t("panel.assistantComposerLabel")),
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
                && reference_boundary(input.text(), input.selected_range().start)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_drafts_distinguish_unicode_whitespace_from_authored_text() {
        let empty = Rope::from("");
        assert!(draft_is_blank(&empty));
        let whitespace = Rope::from(" \t\n\u{a0}\u{3000}");
        assert!(draft_is_blank(&whitespace));
        assert!(!draft_is_blank(&Rope::from(" \n问")));
        assert!(!draft_is_blank(&Rope::from("\u{200b}")));
    }

    #[test]
    fn reference_picker_requires_a_whitespace_boundary_at_the_selection_start() {
        let text = Rope::from("问\u{3000}题 @");
        assert!(reference_boundary(&text, 0));
        assert!(!reference_boundary(&text, "问".len()));
        assert!(reference_boundary(&text, "问\u{3000}".len()));
        assert!(!reference_boundary(&text, "问\u{3000}题".len()));
        assert!(reference_boundary(&text, "问\u{3000}题 ".len()));
        assert!(!reference_boundary(&text, text.len()));
        assert!(!reference_boundary(&text, 1));
        assert!(!reference_boundary(&text, text.len() + 1));
    }

    #[test]
    fn reference_boundary_and_blank_checks_cross_rope_chunks() {
        let mut text = Rope::from(" \u{3000}\n".repeat(4096));
        assert!(draft_is_blank(&text));
        assert!(reference_boundary(&text, text.len()));
        text.insert(text.len(), "问");
        assert!(!draft_is_blank(&text));
        assert!(!reference_boundary(&text, text.len()));
    }
}
