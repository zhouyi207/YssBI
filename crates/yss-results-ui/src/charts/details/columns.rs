//! Paged, read-only columns from the chart editor's accepted metadata.
use super::ChartEditor;
use gpui_kit::component::{ActiveTheme, tooltip::Tooltip};
use gpui_kit::{Context, div, prelude::*};
use yss_ui::inspector::{self, InspectorSection, Pager};
use yss_ui::text::translate;

pub(in crate::charts) const PAGE_COLUMNS: usize = 50;

impl ChartEditor {
    pub(super) fn render_columns(&self, cx: &mut Context<Self>) -> InspectorSection {
        let section = InspectorSection::new("chart-columns", translate("chartsSidebar.columns"));
        let Some(meta) = self.meta.clone().filter(|meta| !meta.columns.is_empty()) else {
            return section.child(inspector::hint(translate("chartsSidebar.noColumns"), cx));
        };
        let pages = meta.columns.len().div_ceil(PAGE_COLUMNS).max(1);
        let rows = meta
            .columns
            .iter()
            .enumerate()
            .skip(self.columns_page * PAGE_COLUMNS)
            .take(PAGE_COLUMNS)
            .map(|(index, column)| {
                let name = column.name().as_str().to_owned();
                let tooltip = name.clone();
                div()
                    .id(("chart-column", index))
                    .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
                    .h_6()
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .child(div().flex_1().min_w_0().truncate().child(name))
                    .child(
                        div()
                            .flex_shrink_0()
                            .text_color(cx.theme().muted_foreground)
                            .child(column.display_type().to_owned()),
                    )
            });
        section
            .description(yss_ui::text::format(
                "native.databases.size",
                &[
                    ("rows", meta.row_count.to_string()),
                    ("columns", meta.column_count.to_string()),
                ],
            ))
            .child(
                div()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().min_w_0().flex().flex_col().children(rows))
                    .when(pages > 1, |body| {
                        body.child(div().flex().justify_end().child(
                            Pager::new("chart-columns-pager", self.columns_page, pages).on_change(
                                cx.listener(move |view, page: &usize, _, cx| {
                                    // A page chosen from replaced metadata no longer
                                    // describes the visible columns.
                                    if !view.meta.as_ref().is_some_and(|current| {
                                        std::sync::Arc::ptr_eq(current, &meta)
                                    }) {
                                        return;
                                    }
                                    view.columns_page = *page;
                                    view.changed(cx);
                                }),
                            ),
                        ))
                    }),
            )
    }
}
