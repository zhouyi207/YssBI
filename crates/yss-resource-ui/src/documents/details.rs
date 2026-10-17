//! File information uses the editor's current snapshot and draft status.
use super::DocumentEditor;
use crate::file_session::{FileEditor, file_status};
use gpui_kit::assets::IconName;
use gpui_kit::{App, IntoElement, div, prelude::*};
use std::borrow::Cow;
use yss_ui::inspector::{self, InspectorHeader, InspectorSection};
use yss_ui::text::translate;

/// Borrowed inspector fields; retain an owned copy only after the display changes.
#[derive(PartialEq)]
pub struct DetailsDisplay<'a> {
    path: Cow<'a, str>,
    status: &'static str,
    error: Option<Cow<'a, str>>,
}

impl DetailsDisplay<'_> {
    pub fn into_owned(self) -> DetailsDisplay<'static> {
        DetailsDisplay {
            path: Cow::Owned(self.path.into_owned()),
            status: self.status,
            error: self.error.map(|error| Cow::Owned(error.into_owned())),
        }
    }
}

impl DocumentEditor {
    /// Read-only inspector dependencies; document text is deliberately excluded.
    pub fn details_state(&self) -> DetailsDisplay<'_> {
        DetailsDisplay {
            path: Cow::Borrowed(self.path()),
            status: file_status(self),
            error: self.file.error().map(|error| Cow::Borrowed(error.as_ref())),
        }
    }

    pub fn render_details(&self, cx: &App) -> impl IntoElement + use<> {
        let status = translate(file_status(self));
        div()
            .min_w_0()
            .flex()
            .flex_col()
            .child(
                InspectorHeader::new(IconName::FileText, self.snapshot().path.name().to_owned())
                    .detail(translate("native.workbench.markdownDocument")),
            )
            .child(
                InspectorSection::new("document-info", translate("detail.sections.info")).child(
                    inspector::property_list([
                        inspector::property_row(
                            translate("detail.fields.path"),
                            self.path().to_owned(),
                            cx,
                        )
                        .into_any_element(),
                        inspector::property_row(translate("detail.fields.status"), status, cx)
                            .id("document-status")
                            .role(gpui_kit::accesskit::Role::Status)
                            .into_any_element(),
                    ]),
                ),
            )
    }
}
