//! A virtual native grid for one bounded result page.
mod cell;
mod render;

use cell::Cell;
use gpui::{SharedString, px};
use gpui_component::table::Column;
use yss_application::graph::results::ResultPageProjection;
use yss_node_kernel::RuntimeValue;

use super::value::display;

pub struct ResultGrid {
    columns: Vec<Column>,
    rows: Vec<Vec<Cell>>,
    data_types: Vec<SharedString>,
    row_numbers: bool,
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
                .map(|(index, name)| {
                    Column::new(index.to_string(), name.clone())
                        .width(px(150.))
                        .movable(false)
                })
                .collect(),
            total_count: Some(rows.len()),
            rows: rows
                .into_iter()
                .map(|row| row.into_iter().map(Cell::text).collect())
                .collect(),
            data_types: vec![],
            row_numbers: false,
            offset: 0,
            has_more: false,
        }
    }

    pub fn from_page(page: ResultPageProjection) -> Self {
        Self::with_format(page, display, true)
    }
    pub fn from_report(page: ResultPageProjection) -> Self {
        Self::with_format(page, super::report::scalar, false)
    }

    /// A bounded array page can use the same grid when every cell is scalar.
    pub fn from_structured(page: &ResultPageProjection) -> Option<Self> {
        use yss_application::graph::results::report::structured::table_reference;
        const MAX_COLUMNS: usize = 64;
        let scalar = |value: &RuntimeValue| matches!(value.unannotated(), RuntimeValue::Scalar(_));
        let values = &page.values;
        let (names, rows): (Vec<String>, Vec<Vec<Cell>>) = if values.iter().all(scalar) {
            let rows = values
                .iter()
                .map(|value| vec![Cell::new(value, super::report::scalar)])
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
                                .map(|value| Cell::new(value, super::report::scalar))
                                .unwrap_or_else(|| Cell::text(String::new()))
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
                .map(|(index, name)| {
                    Column::new(index.to_string(), name.clone())
                        .width(px(180.))
                        .movable(false)
                })
                .collect(),
            rows,
            data_types: vec![],
            row_numbers: false,
            offset: page.offset,
            total_count: page.total_count,
            has_more: page.has_more,
        })
    }
    fn with_format(
        page: ResultPageProjection,
        display: fn(&RuntimeValue) -> String,
        row_numbers: bool,
    ) -> Self {
        let names: Vec<String> = page
            .columns
            .iter()
            .map(|column| column.name.to_string())
            .collect();
        let columns = if names.is_empty() {
            vec![Column::new("value", "value").width(px(400.)).movable(false)]
        } else {
            names
                .iter()
                .enumerate()
                .map(|(index, name)| {
                    Column::new(index.to_string(), name.clone())
                        .width(px(180.))
                        .movable(false)
                })
                .collect()
        };
        let rows = page
            .values
            .iter()
            .map(|value| match value.unannotated() {
                RuntimeValue::List(values) => values
                    .iter()
                    .map(|value| Cell::new(value, display))
                    .collect(),
                RuntimeValue::Record(record) => names
                    .iter()
                    .map(|name| {
                        record
                            .get(name.as_str())
                            .map(|value| Cell::new(value, display))
                            .unwrap_or_else(|| {
                                Cell::new(&yss_data_contract::TabularScalar::Null.into(), display)
                            })
                    })
                    .collect(),
                value => vec![Cell::new(value, display)],
            })
            .collect();
        Self {
            columns,
            rows,
            data_types: if row_numbers {
                page.columns
                    .iter()
                    .map(|column| column.data_type.to_string().into())
                    .collect()
            } else {
                vec![]
            },
            row_numbers,
            offset: page.offset,
            total_count: page.total_count,
            has_more: page.has_more,
        }
    }
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }
}
