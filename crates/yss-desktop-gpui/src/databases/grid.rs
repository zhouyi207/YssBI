//! Only the bounded page and its stable row identities are held by the native grid.
use gpui::{
    App, Context, Div, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, Stateful, Window,
    div, prelude::*, px,
};
use gpui_component::ActiveTheme;
use gpui_component::table::{Column, TableDelegate, TableState};
use yss_application::database::DatabaseRowsResult;
use yss_data_contract::{TabularScalar, TabularSnapshot};

pub(super) struct DatabaseGrid {
    pub rows: TabularSnapshot,
    pub row_ids: Vec<i64>,
    pub offset: usize,
    pub has_more: bool,
    pub selection: super::selection::PageSelection,
}
impl DatabaseGrid {
    pub fn empty() -> Self {
        Self {
            rows: TabularSnapshot::try_from_columns(Box::new([])).expect("empty table"),
            row_ids: vec![],
            offset: 0,
            has_more: false,
            selection: Default::default(),
        }
    }
    pub fn from_page(page: DatabaseRowsResult, offset: usize) -> Self {
        Self {
            rows: page.rows,
            row_ids: page.row_ids,
            has_more: page.has_more,
            selection: Default::default(),
            offset,
        }
    }
    pub fn text(&self, row: usize, column: usize) -> String {
        self.rows
            .columns()
            .get(column)
            .and_then(|column| column.values().get(row))
            .map(display)
            .unwrap_or_default()
    }
}
impl TableDelegate for DatabaseGrid {
    fn columns_count(&self, _: &App) -> usize {
        self.rows.columns().len()
    }
    fn rows_count(&self, _: &App) -> usize {
        self.rows.row_count()
    }
    fn column(&self, index: usize, _: &App) -> Column {
        let name = self.rows.columns()[index].name().as_str().to_owned();
        Column::new(name.clone(), name).width(px(180.))
    }
    fn render_tr(
        &mut self,
        row: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> Stateful<Div> {
        div().id(("database-row", self.row_ids[row] as u64))
    }
    fn render_td(
        &mut self,
        row: usize,
        column: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let selected = self
            .selection
            .bounds
            .as_ref()
            .is_some_and(|bounds| bounds.contains(row, column));
        div()
            .id(("database-cell", row * self.rows.columns().len() + column))
            .size_full()
            .text_sm()
            .overflow_hidden()
            .when(selected, |cell| cell.bg(cx.theme().selection.opacity(0.35)))
            .child(self.text(row, column))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |table, event: &MouseDownEvent, window, cx| {
                    super::selection::start_cell(
                        table,
                        row,
                        column,
                        event.modifiers.shift,
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
                    && table.selected_cell() != Some((row, column))
                {
                    table.delegate_mut().selection.extend_cell();
                    table.set_selected_cell(row, column, cx);
                    cx.stop_propagation();
                }
            }))
            .on_click(|_, _, cx| cx.stop_propagation())
    }
}
fn display(value: &TabularScalar) -> String {
    match value {
        TabularScalar::Null => "null".into(),
        TabularScalar::Bool(value) => value.to_string(),
        TabularScalar::Integer(value) => value.to_string(),
        TabularScalar::Unsigned(value) => value.to_string(),
        TabularScalar::Float64(value) => value.as_f64().to_string(),
        TabularScalar::String(value) => value.to_string(),
    }
}
