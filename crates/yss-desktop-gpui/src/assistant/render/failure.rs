//! A localized failure stays in its message; the original code is available on demand.
use super::super::ConversationPanel;
use gpui_kit::component::ActiveTheme;
use gpui_kit::{AnyElement, Context, IntoElement, SharedString, div, prelude::*};

impl ConversationPanel {
    pub(super) fn failure_card(&self, id: u64, code: &str, cx: &mut Context<Self>) -> AnyElement {
        let key = format!("failure-{id}");
        let open = self.is_expanded(&key, false);
        let label = super::super::commands::failure_text(code);
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_1()
            .p_3()
            .rounded_md()
            .border_1()
            .border_color(cx.theme().danger.opacity(0.2))
            .bg(cx.theme().danger.opacity(0.05))
            .text_xs()
            .child(if label == code {
                crate::text::t("panel.assistantReplyInterrupted").into_owned()
            } else {
                label
            })
            .child(self.disclosure(
                key,
                crate::text::t("panel.assistantTechnicalDetails").into(),
                open,
                cx,
            ))
            .when(open, |card| {
                card.child(
                    div()
                        .font_family(cx.theme().mono_font_family.clone())
                        .child(super::plain::PlainText::new(
                            SharedString::from(format!("failure-code-{id}")),
                            code.to_owned(),
                            "panel.assistantTechnicalDetails",
                        )),
                )
            })
            .into_any_element()
    }
}
