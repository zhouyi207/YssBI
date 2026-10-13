mod columns;
use super::DatabaseEditor;
use std::borrow::Cow;
use std::collections::HashSet;
use yss_project_identity::ResourceRevision;

/// The inspector's display dependencies, borrowed for allocation-free comparison.
#[derive(PartialEq)]
pub struct DetailsDisplay<'a> {
    name: Cow<'a, str>,
    revision: ResourceRevision,
    metadata: Option<(u64, usize, usize)>,
    busy: bool,
    dirty: bool,
    error: Option<Cow<'a, str>>,
    disclosure: Cow<'a, DetailsState>,
    preview_open: bool,
    summary: Option<(usize, usize, usize)>,
    primary: Option<(usize, usize)>,
    offset: usize,
}

impl DetailsDisplay<'_> {
    pub fn into_owned(self) -> DetailsDisplay<'static> {
        DetailsDisplay {
            name: Cow::Owned(self.name.into_owned()),
            revision: self.revision,
            metadata: self.metadata,
            busy: self.busy,
            dirty: self.dirty,
            error: self.error.map(|error| Cow::Owned(error.into_owned())),
            disclosure: Cow::Owned(self.disclosure.into_owned()),
            preview_open: self.preview_open,
            summary: self.summary,
            primary: self.primary,
            offset: self.offset,
        }
    }
}

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Disableable, Sizable,
    button::{Button, ButtonVariants},
    input::Textarea,
};
use gpui_kit::{Context, IntoElement, Window, div, prelude::*};
use yss_application::database::DatabaseMetaResult;
use yss_ui::inspector::{self, InlineMessage, InspectorHeader, InspectorSection, Severity};
use yss_ui::text::translate;

#[derive(Clone, PartialEq)]
pub(super) struct DetailsState {
    info_open: bool,
    columns_open: bool,
    columns_page: usize,
    expanded: HashSet<String>,
}
impl Default for DetailsState {
    fn default() -> Self {
        Self {
            info_open: true,
            columns_open: true,
            columns_page: 0,
            expanded: HashSet::new(),
        }
    }
}

impl DatabaseEditor {
    /// Schema revisions and selection describe the inspector without copying table data.
    pub fn details_state(&self, cx: &gpui_kit::App) -> DetailsDisplay<'_> {
        let table = self.grid.read(cx);
        let page = table.delegate();
        let summary = page
            .selection
            .summary(page.rows.row_count(), page.rows.columns().len())
            .filter(|summary| summary.cells > 1)
            .map(|summary| (summary.rows, summary.columns, summary.cells));
        let primary = page
            .selection
            .primary_cell(page.data_selection(table.selection()))
            .filter(|(row, column)| page.value(*row, *column).is_some());
        DetailsDisplay {
            name: Cow::Borrowed(&self.name),
            revision: self.revision,
            metadata: self
                .meta
                .as_ref()
                .map(|meta| (meta.data_revision, meta.row_count, meta.column_count)),
            busy: self.busy(),
            dirty: self.dirty(),
            error: self.error.as_deref().map(Cow::Borrowed),
            disclosure: Cow::Borrowed(&self.details),
            preview_open: self.selection_preview_open,
            summary,
            primary,
            offset: page.offset,
        }
    }

    pub fn render_details(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let busy = self.busy();
        let view =
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .child(
                    InspectorHeader::new(IconName::Database, self.name.clone())
                        .detail(translate("detail.itemTypes.data")),
                )
                .when_some(self.error.clone(), |view, error| {
                    view.child(inspector::status_band(
                        InlineMessage::error(error)
                            .id("database-details-error")
                            .action(
                                Button::new("database-details-retry")
                                    .small()
                                    .ghost()
                                    .label(translate("common.retry"))
                                    .disabled(busy)
                                    .on_click(cx.listener(|view, _, window, cx| {
                                        view.reload(true, window, cx)
                                    })),
                            ),
                        cx,
                    ))
                });
        let Some(meta) = self.meta.clone() else {
            return view
                .when(self.error.is_none(), |view| {
                    view.child(inspector::status_band(
                        inspector::hint(translate("native.databases.loadingMetadata"), cx),
                        cx,
                    ))
                })
                .into_any_element();
        };
        view.when(self.dirty(), |view| {
            view.child(inspector::status_band(
                InlineMessage::new(
                    Severity::Information,
                    translate("native.databases.checkpointHint"),
                ),
                cx,
            ))
        })
        .child(self.render_info(&meta, cx))
        .child(self.render_columns(&meta, cx))
        .child(self.render_selection(window, cx))
        .into_any_element()
    }

    fn render_info(&self, meta: &DatabaseMetaResult, cx: &mut Context<Self>) -> InspectorSection {
        InspectorSection::new("database-info", translate("detail.sections.info"))
            .collapsible(
                self.details.info_open,
                cx.listener(|view, open: &bool, _, cx| {
                    view.details.info_open = *open;
                    view.changed(cx);
                }),
            )
            .child(inspector::property_list([
                inspector::property_row(
                    translate("detail.fields.columns"),
                    meta.column_count.to_string(),
                    cx,
                ),
                inspector::property_row(
                    translate("detail.fields.rows"),
                    meta.row_count.to_string(),
                    cx,
                ),
            ]))
    }

    /// The preview follows the grid's primary cell; a multi-cell selection adds its extent.
    fn render_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> InspectorSection {
        let table = self.grid.read(cx);
        let page = table.delegate();
        let selection = &page.selection;
        let summary = selection
            .summary(page.rows.row_count(), page.rows.columns().len())
            .filter(|summary| summary.cells > 1);
        let primary = selection
            .primary_cell(page.data_selection(table.selection()))
            .filter(|(row, column)| page.value(*row, *column).is_some());
        let row_number = primary
            .map(|(row, _)| (page.offset + row + 1).to_string())
            .unwrap_or_else(|| "—".into());
        let column_name = primary
            .and_then(|(_, column)| page.rows.columns().get(column))
            .map_or_else(
                || "—".to_owned(),
                |column| column.name().as_str().to_owned(),
            );
        let placeholder = translate(
            match primary.and_then(|(row, column)| page.value(row, column)) {
                None => "databaseEditor.cellPreviewPlaceholder",
                Some(yss_data_contract::TabularScalar::Null) => {
                    "databaseEditor.nullCellPlaceholder"
                }
                _ => "databaseEditor.emptyStringPlaceholder",
            },
        );
        if self
            .selection_preview
            .read(cx)
            .presentation()
            .placeholder()
            .as_ref()
            != placeholder.as_str()
        {
            self.selection_preview.update(cx, |input, cx| {
                input.set_placeholder(placeholder, window, cx)
            });
        }
        InspectorSection::new("database-selection", translate("databaseEditor.selected"))
            .collapsible(
                self.selection_preview_open,
                cx.listener(|view, open: &bool, _, cx| {
                    view.selection_preview_open = *open;
                    view.changed(cx);
                }),
            )
            .when_some(summary, |section, summary| {
                section.child(inspector::hint(
                    yss_ui::text::format(
                        "native.databases.selectionHint",
                        &[
                            ("value0", summary.rows.to_string()),
                            ("value1", summary.columns.to_string()),
                            ("value2", summary.cells.to_string()),
                        ],
                    ),
                    cx,
                ))
            })
            .child(inspector::property_list([
                inspector::property_row(translate("databaseEditor.rowNumber"), row_number, cx),
                inspector::property_row(translate("databaseEditor.columnName"), column_name, cx),
            ]))
            .child(inspector::sub_field(
                translate("databaseEditor.content"),
                Textarea::new(&self.selection_preview)
                    .readonly(true)
                    .small()
                    .h_32(),
                cx,
            ))
    }
}
