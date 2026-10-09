//! Literal message text uses the existing read-only selection and clipboard engine.
use gpui::{App, ElementId, IntoElement, RenderOnce, SharedString, Window, prelude::*};
use gpui_component::input::{Textarea, TextareaState};

#[derive(IntoElement)]
pub(super) struct PlainText {
    id: ElementId,
    text: SharedString,
    label: &'static str,
}

impl PlainText {
    pub fn new(
        id: impl Into<ElementId>,
        text: impl Into<SharedString>,
        label: &'static str,
    ) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            label,
        }
    }
}

impl RenderOnce for PlainText {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state(self.id, cx, |window, cx| {
            TextareaState::new(window, cx)
                .auto_grow(1, usize::MAX)
                .default_value(self.text.clone())
        });
        if !state.read(cx).text().chars().eq(self.text.chars()) {
            state.update(cx, |state, cx| {
                let selection = state.selected_range();
                let scroll = state.scroll_offset();
                state.set_value(self.text, window, cx);
                state.set_selected_range(selection, cx);
                state.set_scroll_offset(scroll, cx);
            });
        }
        Textarea::new(&state)
            .readonly(true)
            .appearance(false)
            .bordered(false)
            .p_0()
            .text_color(window.text_style().color)
            .aria_label(crate::text::t(self.label))
    }
}
