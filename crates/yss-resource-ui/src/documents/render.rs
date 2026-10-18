use super::{DocumentEditor, SaveDocument, ToggleDocumentPreview};
use crate::file_session::FileEditor;
use gpui_kit::component::{
    ActiveTheme, ElementExt, Sizable, Size,
    button::{Button, ButtonVariants},
    input::Editor,
};
use gpui_kit::{Context, IntoElement, Render, Window, div, prelude::*};
use yss_ui::lifecycle::LogReleased;

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
                    let owner = cx.entity().downgrade();
                    div()
                        .size_full()
                        .overflow_hidden()
                        .on_prepaint(move |bounds, _, cx| {
                            owner
                                .update(cx, |view, cx| {
                                    view.observe_preview_width(bounds.size.width, cx)
                                })
                                .log_released();
                        })
                        .child(
                            div()
                                .h_full()
                                .map(|preview| match self.preview_width {
                                    Some(width) => preview.w(width),
                                    None => preview.w_full(),
                                })
                                .child(self.preview_view.clone().px_6().py_4()),
                        )
                        .into_any_element()
                } else {
                    // The editor frame keeps a fixed, unscrolled inset above line one and offers no
                    // way to remove it; lift the frame by that inset so line one meets the header.
                    div()
                        .size_full()
                        .relative()
                        .overflow_hidden()
                        .child(
                            div()
                                .absolute()
                                .top(-Size::default().input_py())
                                .bottom_0()
                                .left_0()
                                .right_0()
                                .child(
                                    Editor::new(&self.input)
                                        .size_full()
                                        .bordered(false)
                                        .appearance(false)
                                        .readonly(
                                            (self.file.committing() && !self.autosave_in_flight)
                                                || self.file.reading()
                                                || self.file.removed(),
                                        ),
                                ),
                        )
                        .into_any_element()
                },
            ))
    }
}
