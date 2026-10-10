use super::{ClearDatabaseSelection, CopyDatabaseSelection, DatabaseEditor, SelectDatabasePage};
use gpui_kit::component::{ActiveTheme, Sizable, table::DataTable};
use gpui_kit::{Context, IntoElement, MouseButton, Render, Window, div, prelude::*};
impl Render for DatabaseEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let busy = self.busy();
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
            .on_action(cx.listener(|view, _: &SelectDatabasePage, window, cx| {
                view.select_page(window, cx);
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
                                    crate::text::t("native.databases.loading")
                                } else {
                                    crate::text::t("native.databases.unavailableTitle")
                                }),
                        )
                    }),
            )
            .child(self.render_toolbar(cx))
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
