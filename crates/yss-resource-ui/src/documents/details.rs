//! File information uses the editor's current snapshot and draft status.
use super::DocumentEditor;
use gpui_kit::component::ActiveTheme;
use gpui_kit::{App, IntoElement, div, prelude::*};
use std::borrow::Cow;

/// Borrowed inspector fields; retain an owned copy only after the display changes.
#[derive(PartialEq)]
pub struct DetailsDisplay<'a> {
    path: Cow<'a, str>,
    busy: bool,
    dirty: bool,
    error: Option<Cow<'a, str>>,
}

impl DetailsDisplay<'_> {
    pub fn into_owned(self) -> DetailsDisplay<'static> {
        DetailsDisplay {
            path: Cow::Owned(self.path.into_owned()),
            busy: self.busy,
            dirty: self.dirty,
            error: self.error.map(|error| Cow::Owned(error.into_owned())),
        }
    }
}

impl DocumentEditor {
    /// Read-only inspector dependencies; document text is deliberately excluded.
    pub fn details_state(&self) -> DetailsDisplay<'_> {
        DetailsDisplay {
            path: Cow::Borrowed(self.path()),
            busy: self.busy(),
            dirty: self.dirty(),
            error: self.error.as_deref().map(Cow::Borrowed),
        }
    }

    pub fn render_details(&self, cx: &App) -> impl IntoElement + use<> {
        div()
            .px_4()
            .pb_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .h(yss_ui::appearance::HEADER_HEIGHT)
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .min_w_0()
                    .truncate()
                    .text_sm()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child(self.snapshot.path.name().to_owned()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(yss_ui::text::translate("native.workbench.markdownDocument")),
            )
            .child(div().text_sm().child(self.path().to_owned()))
            .child(
                div()
                    .id("document-status")
                    .role(gpui_kit::accesskit::Role::Status)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(yss_ui::text::translate(if self.busy() {
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
                        .role(gpui_kit::accesskit::Role::Alert)
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
    }
}
