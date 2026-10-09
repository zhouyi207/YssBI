//! File information uses the editor's current snapshot and draft status.
use super::DocumentEditor;
use gpui::{App, IntoElement, div, prelude::*};
use gpui_component::ActiveTheme;

impl DocumentEditor {
    pub(crate) fn render_details(&self, cx: &App) -> impl IntoElement + use<> {
        div()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(self.snapshot.path.name().to_owned()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::translate("native.workbench.markdownDocument")),
            )
            .child(div().text_sm().child(self.path().to_owned()))
            .child(
                div()
                    .id("document-status")
                    .role(gpui::accesskit::Role::Status)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::translate(if self.busy() {
                        "native.workbench.loadingOrSaving"
                    } else if self.dirty() {
                        "native.workbench.unsavedChanges"
                    } else {
                        "native.workbench.saved"
                    })),
            )
            .when_some(self.error.clone(), |view, error| {
                view.child(
                    div()
                        .id("document-error")
                        .role(gpui::accesskit::Role::Alert)
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
    }
}
