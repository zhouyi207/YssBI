use super::{ClearDatabaseSelection, CopyDatabaseSelection, DatabaseEditor, SelectDatabasePage};
use gpui::{Context, IntoElement, MouseButton, Render, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    menu::{DropdownMenu, PopupMenuItem},
    table::DataTable,
};
impl Render for DatabaseEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.busy();
        let rows = self.meta.as_ref().map(|meta| meta.row_count).unwrap_or(0);
        let columns = self
            .meta
            .as_ref()
            .map(|meta| meta.column_count)
            .unwrap_or(0);
        let owner = cx.entity().downgrade();
        div()
            .id("database-editor")
            .key_context("DatabaseEditor")
            .size_full()
            .flex()
            .flex_col()
            .on_action(
                cx.listener(|view, _: &CopyDatabaseSelection, _, cx| view.copy_selection(cx)),
            )
            .on_action(cx.listener(|view, _: &ClearDatabaseSelection, _, cx| {
                view.clear_selection(cx);
            }))
            .on_action(cx.listener(|view, _: &SelectDatabasePage, _, cx| {
                view.select_page(cx);
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    view.finish_selection_drag(cx);
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|view, _, _, cx| {
                    view.finish_selection_drag(cx);
                }),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .overflow_hidden()
                    .child(DataTable::new(&self.grid).small().stripe(true))
                    .when(busy || !self.ready, |view| {
                        view.child(
                            div()
                                .absolute()
                                .size_full()
                                .bg(cx.theme().background.opacity(0.7))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_sm()
                                .child(if busy {
                                    "正在读取数据…"
                                } else {
                                    "数据暂不可用"
                                }),
                        )
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .p_2()
                    .border_t_1()
                    .border_color(cx.theme().border)
                    .child(
                        Button::new("data-refresh")
                            .small()
                            .ghost()
                            .label("刷新")
                            .disabled(busy)
                            .on_click(
                                cx.listener(|view, _, window, cx| view.reload(true, window, cx)),
                            ),
                    )
                    .child(
                        Button::new("data-export")
                            .small()
                            .ghost()
                            .label("导出")
                            .disabled(busy || !self.ready)
                            .dropdown_menu(move |mut menu, _, _| {
                                for (label, format) in [("CSV", "csv"), ("Parquet", "parquet")] {
                                    let owner = owner.clone();
                                    menu = menu.item(PopupMenuItem::new(label).on_click(
                                        move |_, window, cx| {
                                            let _ = owner.update(cx, |view, cx| {
                                                view.export(format, window, cx)
                                            });
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
                            .label("复制选区")
                            .disabled(
                                busy || !self.ready
                                    || self.grid.read(cx).delegate().selection.bounds.is_none(),
                            )
                            .on_click(cx.listener(|view, _, _, cx| view.copy_selection(cx))),
                    )
                    .child(div().flex_1())
                    .when_some(
                        self.grid.read(cx).delegate().selection.bounds.clone(),
                        |view, bounds| {
                            view.child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!(
                                        "已选 {} 行 × {} 列",
                                        bounds.rows.len(),
                                        bounds.columns.len()
                                    )),
                            )
                        },
                    )
                    .child(
                        Button::new("data-previous")
                            .small()
                            .ghost()
                            .label("上一页")
                            .disabled(busy || self.offset == 0)
                            .on_click(
                                cx.listener(|view, _, window, cx| view.page(false, window, cx)),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "{} / {}",
                                self.offset / super::query::PAGE_ROWS + 1,
                                rows.div_ceil(super::query::PAGE_ROWS).max(1)
                            )),
                    )
                    .child(
                        Button::new("data-next")
                            .small()
                            .ghost()
                            .label("下一页")
                            .disabled(
                                busy || !self.ready || !self.grid.read(cx).delegate().has_more,
                            )
                            .on_click(
                                cx.listener(|view, _, window, cx| view.page(true, window, cx)),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{rows} 行 · {columns} 列")),
                    ),
            )
            .when_some(self.error.clone(), |view, error| {
                view.child(
                    div()
                        .px_3()
                        .py_2()
                        .text_xs()
                        .text_color(cx.theme().danger)
                        .child(error),
                )
            })
    }
}
