use super::{DocumentEditor, SaveDocument, ToggleDocumentPreview};
use crate::appearance;
use gpui::{Context, IntoElement, Render, Window, div, prelude::*, rgb};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    input::Editor,
    text::TextView,
};

impl Render for DocumentEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("markdown-document")
            .key_context("DocumentEditor")
            .size_full()
            .relative()
            .overflow_hidden()
            .bg(rgb(appearance::CANVAS))
            .on_action(cx.listener(|view, _: &SaveDocument, window, cx| view.save(window, cx)))
            .on_action(cx.listener(|view, _: &ToggleDocumentPreview, window, cx| {
                view.toggle_preview(window, cx)
            }))
            .child(if self.preview_visible {
                div()
                    .size_full()
                    .px_8()
                    .py_6()
                    .child(
                        TextView::new(&self.preview).plugin(crate::markdown::Markdown::default()),
                    )
                    .into_any_element()
            } else {
                Editor::new(&self.input)
                    .size_full()
                    .bordered(false)
                    .appearance(false)
                    .readonly(self.busy || self.refreshing)
                    .into_any_element()
            })
            .child(
                div().absolute().top_2().right_3().child(
                    Button::new("document-preview")
                        .small()
                        .ghost()
                        .label(if self.preview_visible {
                            crate::text::t("detail.constantValue.edit")
                        } else {
                            crate::text::t("documents.preview")
                        })
                        .tooltip(crate::text::t("native.documents.togglePreview"))
                        .on_click(
                            cx.listener(|view, _, window, cx| view.toggle_preview(window, cx)),
                        ),
                ),
            )
            .when_some(self.error.as_ref(), |view, error| {
                view.child(
                    div()
                        .absolute()
                        .bottom_2()
                        .left_3()
                        .right_3()
                        .p_2()
                        .rounded_md()
                        .bg(cx.theme().muted)
                        .text_color(cx.theme().danger)
                        .text_xs()
                        .child(error.clone()),
                )
            })
    }
}
