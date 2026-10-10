//! The virtual grid borrows schema metadata and holds only one bounded page with stable row IDs.
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    ActiveTheme, Icon,
    table::{Column, TableDelegate, TableSelection, TableState},
    tooltip::Tooltip,
};
use gpui_kit::{
    App, Context, Div, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, Stateful, Window,
    div, prelude::*, px,
};
use std::{sync::Arc, time::Duration};
use yss_application::database::{DatabaseMetaResult, DatabaseRowsResult};
use yss_data_contract::{TabularScalar, TabularSnapshot};

pub(super) struct DatabaseGrid {
    pub rows: TabularSnapshot,
    pub row_ids: Vec<i64>,
    pub offset: usize,
    pub has_more: bool,
    pub schema: Option<Arc<DatabaseMetaResult>>,
    pub fetch_time: Option<Duration>,
    pub selection: super::selection::PageSelection,
}
impl DatabaseGrid {
    pub fn empty() -> Self {
        Self {
            rows: TabularSnapshot::default(),
            row_ids: vec![],
            offset: 0,
            has_more: false,
            schema: None,
            fetch_time: None,
            selection: Default::default(),
        }
    }
    pub fn from_page(
        page: DatabaseRowsResult,
        offset: usize,
        schema: Arc<DatabaseMetaResult>,
        elapsed: Duration,
    ) -> Self {
        Self {
            rows: page.rows,
            row_ids: page.row_ids,
            has_more: page.has_more,
            schema: Some(schema),
            fetch_time: Some(elapsed),
            selection: Default::default(),
            offset,
        }
    }
    pub fn value(&self, row: usize, column: usize) -> Option<&TabularScalar> {
        self.rows.columns().get(column)?.values().get(row)
    }
    pub fn data_selection(&self, cursor: TableSelection) -> TableSelection {
        match cursor {
            TableSelection::Cell(row, column) if row < self.rows.row_count() => column
                .checked_sub(1)
                .filter(|column| *column < self.rows.columns().len())
                .map(|column| TableSelection::Cell(row, column))
                .unwrap_or_default(),
            TableSelection::Column(column) => column
                .checked_sub(1)
                .filter(|column| *column < self.rows.columns().len())
                .map(TableSelection::Column)
                .unwrap_or_default(),
            TableSelection::Row(row) if row < self.rows.row_count() => cursor,
            _ => TableSelection::None,
        }
    }
    fn render_row_number(
        &self,
        row: usize,
        cx: &mut Context<TableState<Self>>,
    ) -> gpui_kit::AnyElement {
        let selected = self.selection.row_selected(row);
        div()
            .id(("database-row-number", row))
            .role(gpui_kit::Role::RowHeader)
            .aria_selected(selected)
            .size_full()
            .flex()
            .items_center()
            .justify_end()
            .gap_1()
            .text_xs()
            .text_color(if selected {
                cx.theme().primary
            } else {
                cx.theme().muted_foreground
            })
            .when(selected, |view| {
                view.child(Icon::new(IconName::Check).size_3())
            })
            .child((self.offset + row + 1).to_string())
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |table, event: &MouseDownEvent, window, cx| {
                    super::selection::start(
                        table,
                        TableSelection::Row(row),
                        event.modifiers,
                        window,
                        cx,
                    );
                }),
            )
            .on_click(|_, _, cx| cx.stop_propagation())
            .into_any_element()
    }
}
impl TableDelegate for DatabaseGrid {
    fn columns_count(&self, _: &App) -> usize {
        self.rows.columns().len() + 1
    }
    fn rows_count(&self, _: &App) -> usize {
        self.rows.row_count()
    }
    fn column(&self, index: usize, _: &App) -> Column {
        if index == 0 {
            return Column::new("row-marker", "")
                .width(px(64.))
                .fixed_left()
                .resizable(false)
                .selectable(false);
        }
        let name = self.rows.columns()[index - 1].name().as_str().to_owned();
        Column::new(format!("data-{}", index - 1), name)
            .width(px(180.))
            .min_width(px(72.))
            .max_width(px(520.))
    }
    fn render_th(
        &mut self,
        index: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(column) = index.checked_sub(1) else {
            return div().into_any_element();
        };
        let name = self.rows.columns()[column].name().as_str().to_owned();
        let dtype = self
            .schema
            .as_ref()
            .and_then(|meta| meta.columns.get(column))
            .map(|column| column.display_type())
            .unwrap_or("")
            .to_owned();
        let tooltip = format!("{name} ({dtype})");
        div()
            .id(("database-column", column))
            .role(gpui_kit::Role::ColumnHeader)
            .aria_selected(self.selection.column_selected(column))
            .size_full()
            .flex()
            .items_center()
            .gap_2()
            .when(self.selection.column_selected(column), |view| {
                view.bg(cx.theme().selection.opacity(0.35))
            })
            .child(div().flex_1().min_w_0().truncate().text_xs().child(name))
            .child(
                div()
                    .max_w(px(88.))
                    .truncate()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(dtype),
            )
            .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |table, event: &MouseDownEvent, window, cx| {
                    super::selection::start(
                        table,
                        TableSelection::Column(column),
                        event.modifiers,
                        window,
                        cx,
                    );
                }),
            )
            .on_click(|_, _, cx| cx.stop_propagation())
            .into_any_element()
    }
    fn render_tr(
        &mut self,
        row: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> Stateful<Div> {
        match self.row_ids.get(row) {
            Some(id) => div().id(("database-row", *id as u64)),
            // The component also asks for blank stripe rows below a short page.
            None => div().id(("database-empty-row", row)),
        }
    }
    fn render_td(
        &mut self,
        row: usize,
        index: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(column) = index.checked_sub(1) else {
            return self.render_row_number(row, cx);
        };
        let value = self
            .value(row, column)
            .expect("visible cell in a bounded page");
        let selected = self.selection.contains(row, column);
        let content = match value {
            TabularScalar::Bool(checked) => div()
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
                        .when(*checked, |view| {
                            view.bg(cx.theme().primary)
                                .border_color(cx.theme().primary)
                                .child(
                                    Icon::new(IconName::Check)
                                        .size_3()
                                        .text_color(cx.theme().primary_foreground),
                                )
                        }),
                )
                .into_any_element(),
            _ => div()
                .size_full()
                .truncate()
                .when(
                    matches!(
                        value,
                        TabularScalar::Integer(_)
                            | TabularScalar::Unsigned(_)
                            | TabularScalar::Float64(_)
                    ),
                    |view| view.text_right(),
                )
                .when(matches!(value, TabularScalar::Null), |view| {
                    view.text_color(cx.theme().muted_foreground)
                })
                .child(display(value))
                .into_any_element(),
        };
        div()
            .id(("database-cell", row * self.rows.columns().len() + column))
            .size_full()
            .text_sm()
            .overflow_hidden()
            .when(selected, |cell| cell.bg(cx.theme().selection.opacity(0.35)))
            .child(content)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |table, event: &MouseDownEvent, window, cx| {
                    super::selection::start(
                        table,
                        TableSelection::Cell(row, column),
                        event.modifiers,
                        window,
                        cx,
                    );
                }),
            )
            .on_mouse_move(cx.listener(move |table, event: &MouseMoveEvent, _, cx| {
                if event.pressed_button != Some(MouseButton::Left) {
                    table.delegate_mut().selection.dragging = false;
                    return;
                }
                if table.delegate().selection.dragging
                    && table.selected_cell() != Some((row, index))
                {
                    table.set_selected_cell(row, index, cx);
                    cx.stop_propagation();
                }
            }))
            .on_click(|_, _, cx| cx.stop_propagation())
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
            .child(Icon::new(IconName::Database).size_8())
            .child(crate::text::translate("sidebar.noData"))
    }
}
pub(super) fn display(value: &TabularScalar) -> String {
    match value {
        TabularScalar::Null => "null".into(),
        TabularScalar::Bool(value) => value.to_string(),
        TabularScalar::Integer(value) => value.to_string(),
        TabularScalar::Unsigned(value) => value.to_string(),
        TabularScalar::Float64(value) => value.as_f64().to_string(),
        TabularScalar::String(value) => value.to_string(),
    }
}
