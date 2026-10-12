use super::{DocumentEditor, SaveDocument, ToggleDocumentPreview};
use gpui_kit::component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    input::Editor,
};
use gpui_kit::{Context, IntoElement, Render, Window, div, prelude::*};

impl Render for DocumentEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("markdown-document")
            .key_context("DocumentEditor")
            .size_full()
            .flex()
            .flex_col()
            .min_w_0()
            .min_h_0()
            .bg(cx.theme().background)
            .on_action(cx.listener(|view, _: &SaveDocument, window, cx| view.save(window, cx)))
            .on_action(cx.listener(|view, _: &ToggleDocumentPreview, window, cx| {
                view.toggle_preview(window, cx)
            }))
            .child(
                div()
                    .h(yss_ui::appearance::HEADER_HEIGHT)
                    .flex()
                    .items_center()
                    .justify_between()
                    .flex_shrink_0()
                    .gap_3()
                    .px_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child("Markdown"),
                    )
                    .child(
                        Button::new("document-preview")
                            .small()
                            .ghost()
                            .label(if self.preview_visible {
                                yss_ui::text::t("detail.constantValue.edit")
                            } else {
                                yss_ui::text::t("documents.preview")
                            })
                            .tooltip(yss_ui::text::t("native.documents.togglePreview"))
                            .on_click(
                                cx.listener(|view, _, window, cx| view.toggle_preview(window, cx)),
                            ),
                    ),
            )
            .child(div().flex_1().min_w_0().min_h_0().overflow_hidden().child(
                if self.preview_visible {
                    self.preview_view.clone().px_6().py_4().into_any_element()
                } else {
                    Editor::new(&self.input)
                        .size_full()
                        .bordered(false)
                        .appearance(false)
                        .readonly((self.busy && !self.autosave_in_flight) || self.refreshing)
                        .into_any_element()
                },
            ))
            .when_some(self.error.as_ref(), |view, error| {
                view.child(
                    div()
                        .id("document-save-error")
                        .role(gpui_kit::accesskit::Role::Alert)
                        .flex_shrink_0()
                        .px_3()
                        .py_2()
                        .border_t_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().muted)
                        .text_color(cx.theme().danger)
                        .text_xs()
                        .child(error.clone()),
                )
            })
    }
}
