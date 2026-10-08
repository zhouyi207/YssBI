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
    pub fn formatted(names: Vec<String>, rows: Vec<Vec<String>>) -> Self {
        Self {
            columns: names
                .iter()
                .enumerate()
                .map(|(index, name)| Column::new(index.to_string(), name.clone()).width(px(150.)))
                .collect(),
            total_count: Some(rows.len()),
            rows,
            offset: 0,
            has_more: false,
        }
    }

    pub fn from_page(page: ResultPageProjection) -> Self {
        Self::with_format(page, display)
    }
    pub fn from_report(page: ResultPageProjection) -> Self {
        Self::with_format(page, super::report::scalar)
    }

    /// A bounded array page can use the same grid when every cell is scalar.
    pub fn from_structured(page: &ResultPageProjection) -> Option<Self> {
        use yss_application::graph::results::report::structured::table_reference;
        const MAX_COLUMNS: usize = 64;
        let scalar = |value: &RuntimeValue| matches!(value.unannotated(), RuntimeValue::Scalar(_));
        let values = &page.values;
        let (names, rows): (Vec<String>, Vec<Vec<String>>) = if values.iter().all(scalar) {
            let rows = values
                .iter()
                .map(|value| vec![super::report::scalar(value)])
                .collect();
            (vec!["value".into()], rows)
        } else {
            let records = values
                .iter()
                .map(|value| {
                    let RuntimeValue::Record(fields) = value.unannotated() else {
                        return None;
                    };
                    (fields.len() <= MAX_COLUMNS
                        && table_reference(value).is_none()
                        && fields.values().all(scalar))
                    .then_some(fields)
                })
                .collect::<Option<Vec<_>>>()?;
            // Disjoint record keys must not turn a bounded page into a huge sparse matrix.
            let mut names = std::collections::BTreeSet::new();
            for fields in &records {
                for key in fields.keys() {
                    names.insert(key.to_string());
                    if names.len() > MAX_COLUMNS {
                        return None;
                    }
                }
            }
            let names = names.into_iter().collect::<Vec<_>>();
            let rows = records
                .into_iter()
                .map(|fields| {
                    names
                        .iter()
                        .map(|key| {
                            fields
                                .get(key.as_str())
                                .map(super::report::scalar)
                                .unwrap_or_default()
                        })
                        .collect()
                })
                .collect();
            (names, rows)
        };
        // An empty record is a value, not a table with no visible columns.
        if names.is_empty() {
            return None;
        }
        Some(Self {
            columns: names
                .iter()
                .enumerate()
                .map(|(index, name)| Column::new(index.to_string(), name.clone()).width(px(180.)))
                .collect(),
            rows,
            offset: page.offset,
            total_count: page.total_count,
            has_more: page.has_more,
        })
    }
    fn with_format(page: ResultPageProjection, display: fn(&RuntimeValue) -> String) -> Self {
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
                .enumerate()
                .map(|(index, name)| Column::new(index.to_string(), name.clone()).width(px(180.)))
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
