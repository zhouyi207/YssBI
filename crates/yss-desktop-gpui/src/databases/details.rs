mod columns;
use super::DatabaseEditor;
use std::collections::HashSet;

use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    input::Textarea,
};
use gpui_kit::{Context, IntoElement, Window, div, prelude::*, px};

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
    pub fn render_details(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui_kit::AnyElement {
        let mut view = div()
            .p_4()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                    .child(self.name.clone()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::t("log.domains.data")),
            );
        if let Some(error) = &self.error {
            view = view
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(error.clone()),
                )
                .child(
                    Button::new("database-details-retry")
                        .small()
                        .ghost()
                        .label(crate::text::translate("common.retry"))
                        .disabled(self.busy())
                        .on_click(cx.listener(|view, _, window, cx| view.reload(true, window, cx))),
                );
        }
        let Some(meta) = self.meta.clone() else {
            return view
                .child(
                    div()
                        .text_xs()
                        .child(crate::text::t("native.databases.loadingMetadata")),
                )
                .into_any_element();
        };
        view = view.child(
            Collapsible::new()
                .open(self.details.info_open)
                .child(
                    Button::new("database-info")
                        .small()
                        .ghost()
                        .w_full()
                        .label(crate::text::translate("detail.sections.info"))
                        .icon(if self.details.info_open {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.details.info_open = !view.details.info_open;
                            view.changed(cx);
                        })),
                )
                .content(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .text_xs()
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .gap_2()
                                .child(crate::text::translate("detail.fields.columns"))
                                .child(meta.column_count.to_string()),
                        )
                        .child(
                            div()
                                .flex()
                                .justify_between()
                                .gap_2()
                                .child(crate::text::translate("detail.fields.rows"))
                                .child(meta.row_count.to_string()),
                        ),
                ),
        );
        view = view.child(self.render_columns(&meta, cx));
        if self.dirty() {
            view = view.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::t("native.databases.checkpointHint")),
            );
        }
        let table = self.grid.read(cx);
        let page = table.delegate();
        let selection = &page.selection;
        if let Some(summary) = selection.summary(page.rows.row_count(), page.rows.columns().len())
            && summary.cells > 1
        {
            view = view.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::format(
                        "native.databases.selectionHint",
                        &[
                            ("value0", summary.rows.to_string()),
                            ("value1", summary.columns.to_string()),
                            ("value2", summary.cells.to_string()),
                        ],
                    )),
            );
        }
        let primary = selection
            .primary_cell(page.data_selection(table.selection()))
            .filter(|(row, column)| page.value(*row, *column).is_some());
        let row_number = primary
            .map(|(row, _)| (page.offset + row + 1).to_string())
            .unwrap_or_else(|| "—".into());
        let column_name = primary
            .map(|(_, column)| page.rows.columns()[column].name().as_str())
            .unwrap_or("—");
        let preview_content = div()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .gap_2()
                    .text_xs()
                    .child(crate::text::translate("databaseEditor.rowNumber"))
                    .child(row_number),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .text_xs()
                    .child(crate::text::translate("databaseEditor.columnName"))
                    .child(column_name.to_owned()),
            )
            .child(
                div()
                    .text_xs()
                    .child(crate::text::translate("databaseEditor.content")),
            )
            .child(
                Textarea::new(&self.selection_preview)
                    .readonly(true)
                    .small()
                    .h(px(130.)),
            );
        view = view.child(
            Collapsible::new()
                .open(self.selection_preview_open)
                .child(
                    Button::new("database-selection-preview")
                        .small()
                        .ghost()
                        .w_full()
                        .label(crate::text::translate("databaseEditor.selected"))
                        .icon(if self.selection_preview_open {
                            IconName::ChevronDown
                        } else {
                            IconName::ChevronRight
                        })
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.selection_preview_open = !view.selection_preview_open;
                            view.changed(cx);
                        })),
                )
                .content(preview_content),
        );
        let placeholder = crate::text::translate(
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
        view.into_any_element()
    }
}
