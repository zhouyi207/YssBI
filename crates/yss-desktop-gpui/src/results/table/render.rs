use super::*;
use gpui::{App, Context, IntoElement, Window, div, prelude::*};
use gpui_component::{
    ActiveTheme, Icon,
    table::{TableDelegate, TableState},
    tooltip::Tooltip,
};
use gpui_kit_assets::IconName;

impl TableDelegate for ResultGrid {
    fn columns_count(&self, _: &App) -> usize {
        self.columns.len() + usize::from(self.row_numbers)
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, index: usize, _: &App) -> Column {
        if self.row_numbers && index == 0 {
            Column::new("row-number", "")
                .width(px(64.))
                .fixed_left()
                .resizable(false)
                .movable(false)
                .selectable(false)
        } else {
            self.columns[index - usize::from(self.row_numbers)].clone()
        }
    }

    fn render_th(
        &mut self,
        index: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        if self.row_numbers && index == 0 {
            return div().into_any_element();
        }
        let index = index - usize::from(self.row_numbers);
        let name = self.columns[index].name.clone();
        let data_type = self.data_types.get(index).cloned();
        let tooltip = data_type.as_ref().map_or_else(
            || name.to_string(),
            |data_type| format!("{name} ({data_type})"),
        );
        div()
            .id(("result-column", index))
            .role(gpui::Role::ColumnHeader)
            .size_full()
            .flex()
            .items_center()
            .gap_2()
            .child(div().flex_1().min_w_0().truncate().child(name))
            .when_some(data_type, |header, data_type| {
                header.child(
                    div()
                        .max_w(px(80.))
                        .truncate()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(data_type),
                )
            })
            .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
            .into_any_element()
    }

    fn render_td(
        &mut self,
        row: usize,
        index: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        if self.row_numbers && index == 0 {
            return div()
                .id(("result-row-number", row))
                .role(gpui::Role::RowHeader)
                .size_full()
                .text_right()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child((self.offset + row + 1).to_string())
                .into_any_element();
        }
        let index = index - usize::from(self.row_numbers);
        let Some(cell) = self.rows[row].get(index) else {
            return div().into_any_element();
        };
        let text = cell.text.clone();
        let content = match cell.kind {
            cell::Kind::Bool(checked) => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .size_3p5()
                        .border_1()
                        .rounded_sm()
                        .border_color(cx.theme().muted_foreground)
                        .when(checked, |mark| {
                            mark.bg(cx.theme().primary)
                                .border_color(cx.theme().primary)
                                .child(
                                    Icon::new(IconName::Check)
                                        .size_3()
                                        .text_color(cx.theme().primary_foreground),
                                )
                        }),
                ),
            _ => div()
                .size_full()
                .truncate()
                .when(matches!(cell.kind, cell::Kind::Number), |cell| {
                    cell.text_right()
                })
                .when(matches!(cell.kind, cell::Kind::Null), |cell| {
                    cell.text_color(cx.theme().muted_foreground)
                })
                .child(text.clone()),
        };
        div()
            .id(("result-cell", row * self.columns.len() + index))
            .size_full()
            .text_sm()
            .overflow_hidden()
            .aria_label(text.clone())
            .child(content)
            .tooltip(move |window, cx| Tooltip::new(text.clone()).build(window, cx))
            .into_any_element()
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(Icon::new(IconName::Table).size_8())
            .child(crate::text::format(
                "detail.counts.rows",
                &[("count", "0".into())],
            ))
    }
}
