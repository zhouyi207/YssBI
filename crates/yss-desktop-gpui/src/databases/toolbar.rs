//! Controls and counters are derived from the accepted page, not a second pagination model.
use super::DatabaseEditor;
use crate::text::translate as t;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
};
use gpui_kit::{Context, IntoElement, div, prelude::*};

impl DatabaseEditor {
    pub(super) fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.busy();
        let rows = self.meta.as_ref().map_or(0, |meta| meta.row_count);
        let columns = self.meta.as_ref().map_or(0, |meta| meta.column_count);
        let grid = self.grid.read(cx).delegate();
        let page_start = if grid.rows.row_count() == 0 {
            0
        } else {
            grid.offset + 1
        };
        let page_end = (grid.offset + grid.rows.row_count()).min(rows);
        let page = grid.offset / self.page_size + 1;
        let page_count = rows.div_ceil(self.page_size).max(1);
        let fetch_time = grid
            .fetch_time
            .map(|time| {
                if time.as_secs() > 0 {
                    format!("{:.2}s", time.as_secs_f64())
                } else {
                    format!("{}ms", time.as_millis())
                }
            })
            .unwrap_or_else(|| "—".into());
        let selection = grid
            .selection
            .summary(grid.rows.row_count(), grid.rows.columns().len());
        let can_copy = !grid.selection.is_empty() && grid.rows.row_count() > 0;
        let has_more = grid.has_more;
        let owner = cx.entity().downgrade();
        let actions = div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                Button::new("data-refresh")
                    .small()
                    .ghost()
                    .icon(IconName::RefreshCw)
                    .tooltip(t("common.refresh"))
                    .disabled(busy)
                    .on_click(cx.listener(|view, _, window, cx| view.reload(true, window, cx))),
            )
            .child(
                Button::new("data-export")
                    .small()
                    .ghost()
                    .icon(IconName::Download)
                    .tooltip(t("common.export"))
                    .disabled(busy || !self.ready)
                    .dropdown_menu(move |mut menu, _, _| {
                        for (label, format) in [
                            (t("importModal.types.csv.label"), "csv"),
                            ("Parquet".into(), "parquet"),
                        ] {
                            let owner = owner.clone();
                            menu = menu.item(PopupMenuItem::new(label).on_click(
                                move |_, window, cx| {
                                    let _ = owner
                                        .update(cx, |view, cx| view.export(format, window, cx));
                                },
                            ));
                        }
                        menu
                    }),
            )
            .child(
                Button::new("data-copy")
                    .small()
                    .ghost()
                    .icon(IconName::Copy)
                    .tooltip(t("native.databases.copySelection"))
                    .disabled(busy || !self.ready || !can_copy)
                    .on_click(cx.listener(|view, _, _, cx| view.copy_selection(cx))),
            );
        let pagination = div()
            .flex()
            .items_center()
            .gap_1()
            .child(
                Button::new("data-previous")
                    .small()
                    .ghost()
                    .icon(IconName::ChevronLeft)
                    .tooltip(t("databaseEditor.previousPage"))
                    .disabled(busy || !self.ready || grid.offset == 0)
                    .on_click(cx.listener(|view, _, window, cx| view.page(false, window, cx))),
            )
            .child(
                div()
                    .text_xs()
                    .child(format!("{page_start}–{page_end} / {rows}")),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{page} / {page_count}")),
            )
            .child(
                Button::new("data-next")
                    .small()
                    .ghost()
                    .icon(IconName::ChevronRight)
                    .tooltip(t("databaseEditor.nextPage"))
                    .disabled(busy || !self.ready || !has_more)
                    .on_click(cx.listener(|view, _, window, cx| view.page(true, window, cx))),
            );
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_2()
            .p_2()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(actions)
            .child(pagination)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .when_some(selection, |view, selection| {
                        view.child(crate::text::format(
                            "native.databases.selectionSize",
                            &[("count", selection.cells.to_string())],
                        ))
                    })
                    .child(crate::text::format(
                        "native.databases.size",
                        &[("rows", rows.to_string()), ("columns", columns.to_string())],
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(t("databaseEditor.fetchTime"))
                            .child(fetch_time),
                    ),
            )
    }
}
