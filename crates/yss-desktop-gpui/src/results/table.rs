//! A virtual native grid for one bounded result page.
use gpui::{App, Context, IntoElement, Window, div, prelude::*, px};
use gpui_component::table::{Column, TableDelegate, TableState};
use yss_application::graph::results::ResultPageProjection;
use yss_node_kernel::RuntimeValue;

use super::value::display;

pub struct ResultGrid {
    columns: Vec<Column>,
    rows: Vec<Vec<String>>,
    pub offset: usize,
    pub total_count: Option<usize>,
    pub has_more: bool,
}

impl ResultGrid {
    pub fn from_page(page: ResultPageProjection) -> Self {
        let names: Vec<String> = page
            .columns
            .iter()
            .map(|column| column.name.to_string())
            .collect();
        let columns = if names.is_empty() {
            vec![Column::new("value", "值").width(px(400.))]
        } else {
            names
                .iter()
                .map(|name| Column::new(name.clone(), name.clone()).width(px(180.)))
                .collect()
        };
        let rows = page
            .values
            .iter()
            .map(|value| match value.unannotated() {
                RuntimeValue::List(values) => values.iter().map(display).collect(),
                RuntimeValue::Record(record) => names
                    .iter()
                    .map(|name| {
                        record
                            .get(name.as_str())
                            .map(display)
                            .unwrap_or_else(|| "null".into())
                    })
                    .collect(),
                value => vec![display(value)],
            })
            .collect();
        Self {
            columns,
            rows,
            offset: page.offset,
            total_count: page.total_count,
            has_more: page.has_more,
        }
    }
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
}

impl TableDelegate for ResultGrid {
    fn columns_count(&self, _: &App) -> usize {
        self.columns.len()
    }
    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }
    fn column(&self, index: usize, _: &App) -> Column {
        self.columns[index].clone()
    }
    fn render_td(
        &mut self,
        row: usize,
        column: usize,
        _: &mut Window,
        _: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        div()
            .text_sm()
            .overflow_hidden()
            .child(self.rows[row].get(column).cloned().unwrap_or_default())
    }
}
