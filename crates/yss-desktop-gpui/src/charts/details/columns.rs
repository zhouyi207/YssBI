//! Paged, read-only columns from the chart editor's accepted metadata.
use super::ChartEditor;
use gpui::{Context, IntoElement, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    tooltip::Tooltip,
};
use gpui_kit_assets::IconName;

pub(in crate::charts) const PAGE_COLUMNS: usize = 50;

impl ChartEditor {
    pub(super) fn render_columns(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
        let mut view = div().flex().flex_col().min_w_0().gap_2().child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(crate::text::translate("chartsSidebar.columns")),
        );
        if let Some(meta) = &self.meta {
            view = view.child(
                div()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .pt_3()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(crate::text::format(
                        "native.databases.size",
                        &[
                            ("rows", meta.row_count.to_string()),
                            ("columns", meta.column_count.to_string()),
                        ],
                    )),
            );
            for (index, column) in meta
                .columns
                .iter()
                .enumerate()
                .skip(self.columns_page * PAGE_COLUMNS)
                .take(PAGE_COLUMNS)
            {
                let name = column.name().as_str().to_owned();
                view = view.child(
                    div()
                        .id(("chart-column", index))
                        .tooltip(move |window, cx| Tooltip::new(name.clone()).build(window, cx))
                        .min_w_0()
                        .flex()
                        .justify_between()
                        .gap_2()
                        .text_xs()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(column.name().as_str().to_owned()),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .font_family(cx.theme().mono_font_family.clone())
                                .text_color(cx.theme().muted_foreground)
                                .child(column.display_type().to_owned()),
                        ),
                );
            }
            let pages = meta.columns.len().div_ceil(PAGE_COLUMNS).max(1);
            if pages > 1 {
                let previous_metadata = meta.clone();
                let next_metadata = meta.clone();
                view = view.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            Button::new("chart-columns-previous")
                                .small()
                                .ghost()
                                .icon(IconName::ChevronLeft)
                                .tooltip(crate::text::translate("databaseEditor.previousPage"))
                                .disabled(self.columns_page == 0)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if !view.meta.as_ref().is_some_and(|current| {
                                        std::sync::Arc::ptr_eq(current, &previous_metadata)
                                    }) {
                                        return;
                                    }
                                    view.columns_page = view.columns_page.saturating_sub(1);
                                    view.changed(cx);
                                })),
                        )
                        .child(
                            div()
                                .text_xs()
                                .child(format!("{} / {pages}", self.columns_page + 1)),
                        )
                        .child(
                            Button::new("chart-columns-next")
                                .small()
                                .ghost()
                                .icon(IconName::ChevronRight)
                                .tooltip(crate::text::translate("databaseEditor.nextPage"))
                                .disabled(self.columns_page + 1 >= pages)
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    if !view.meta.as_ref().is_some_and(|current| {
                                        std::sync::Arc::ptr_eq(current, &next_metadata)
                                    }) {
                                        return;
                                    }
                                    view.columns_page = (view.columns_page + 1).min(pages - 1);
                                    view.changed(cx);
                                })),
                        ),
                );
            }
        }

        view.when(
            self.meta
                .as_ref()
                .is_none_or(|meta| meta.columns.is_empty()),
            |view| {
                view.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::text::translate("chartsSidebar.noColumns")),
                )
            },
        )
    }
}
