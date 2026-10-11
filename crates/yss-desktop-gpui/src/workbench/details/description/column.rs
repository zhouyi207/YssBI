//! Column-local disclosure and a bounded frequency table reuse the result grid.
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable,
    button::{Button, ButtonVariants},
    collapsible::Collapsible,
    table::TableState,
};
use gpui_kit::{AppContext, Context, Entity, IntoElement, Render, Window, div, prelude::*};
use yss_application::graph::results::description::{
    DescriptionColumn, DescriptionStatistics, NUMERIC_FIELDS,
};
use yss_data_contract::TabularScalar;

use super::render::heading;
use crate::{
    results::{ResultGrid, format_number},
    text::translate,
};

const PAGE_ROWS: usize = 100;

pub(super) struct ColumnSummary {
    column: DescriptionColumn,
    open: bool,
    categories_open: bool,
    page: usize,
    table: Option<Entity<TableState<ResultGrid>>>,
    locale: &'static str,
}

impl ColumnSummary {
    pub(super) fn new(column: DescriptionColumn) -> Self {
        Self {
            column,
            open: false,
            categories_open: false,
            page: 0,
            table: None,
            locale: crate::text::locale(),
        }
    }

    fn prepare_table(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.locale != crate::text::locale() {
            self.locale = crate::text::locale();
            self.table = None;
        }
        if self.table.is_some() || !self.open || !self.categories_open {
            return;
        }
        let DescriptionStatistics::Categorical { categories, .. } = &self.column.statistics else {
            return;
        };
        if categories.is_empty() {
            return;
        }
        // Label presence belongs to the entire distribution, so page changes retain the columns.
        let labels = categories.iter().any(|category| category.label.is_some());
        let mut names = vec![translate("detail.description.fields.value")];
        if labels {
            names.push(translate("detail.description.fields.label"));
        }
        names.extend([
            translate("detail.description.fields.frequency"),
            translate("detail.description.fields.proportion"),
        ]);
        let rows = categories
            .iter()
            .skip(self.page * PAGE_ROWS)
            .take(PAGE_ROWS)
            .map(|category| {
                let mut cells = vec![code(&category.value)];
                if labels {
                    cells.push(
                        category
                            .label
                            .as_ref()
                            .map(|label| {
                                if label.is_empty() {
                                    "\"\"".into()
                                } else {
                                    label.clone()
                                }
                            })
                            .unwrap_or_else(|| "—".into()),
                    );
                }
                cells.extend([
                    category.frequency.to_string(),
                    format_number(category.proportion, 4),
                ]);
                cells
            })
            .collect();
        self.table = Some(cx.new(|cx| {
            TableState::new(ResultGrid::formatted(names, rows), window, cx).sortable(false)
        }));
    }

    fn summary(&self, cx: &gpui_kit::App) -> gpui_kit::Div {
        let semantic = match &self.column.statistics {
            DescriptionStatistics::Numeric { .. } => "Numeric",
            DescriptionStatistics::Categorical { semantic, .. } => semantic,
        };
        let mut fields = vec![
            ("semantic", semantic.to_string()),
            ("count", self.column.count.to_string()),
            ("missing", self.column.missing.to_string()),
        ];
        match &self.column.statistics {
            DescriptionStatistics::Numeric { metrics } => {
                fields.extend(NUMERIC_FIELDS.into_iter().zip(metrics.iter().map(metric)))
            }
            DescriptionStatistics::Categorical { unique, .. } => {
                fields.push(("unique", unique.to_string()))
            }
        }
        div().children(fields.into_iter().map(|(key, value)| {
            div()
                .min_w_0()
                .flex()
                .gap_2()
                .px_3()
                .py_1()
                .child(
                    div()
                        .flex_1()
                        .child(translate(&format!("detail.description.fields.{key}"))),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_right()
                        .text_color(cx.theme().muted_foreground)
                        .child(value),
                )
        }))
    }

    fn frequencies(&self, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let DescriptionStatistics::Categorical { categories, .. } = &self.column.statistics else {
            return div().into_any_element();
        };
        Collapsible::new()
            .w_full()
            .min_w_0()
            .open(self.categories_open)
            .child(
                heading(
                    "description-categories",
                    translate("detail.description.categories"),
                    self.categories_open,
                    cx,
                )
                .on_click(cx.listener(|view, _, _, cx| {
                    view.categories_open = !view.categories_open;
                    cx.notify();
                })),
            )
            .when(self.categories_open, |section| {
                let start = self.page * PAGE_ROWS;
                let end = (start + PAGE_ROWS).min(categories.len());
                section.content(
                    div()
                        .min_w_0()
                        .when(categories.is_empty(), |body| {
                            body.child(
                                div()
                                    .p_2()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(translate("detail.description.noCategories")),
                            )
                        })
                        .when_some(self.table.as_ref(), |body, table| {
                            body.child(
                                div()
                                    .w_full()
                                    .h(crate::results::table::height((end - start).min(8), cx))
                                    .child(crate::results::table::present(table, cx)),
                            )
                        })
                        .when(categories.len() > PAGE_ROWS, |body| {
                            body.child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .justify_end()
                                    .items_center()
                                    .gap_1()
                                    .child(format!("{}–{} / {}", start + 1, end, categories.len()))
                                    .child(
                                        Button::new("category-prev")
                                            .small()
                                            .ghost()
                                            .label(translate("sourceInspector.previous"))
                                            .disabled(self.page == 0)
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                view.page = view.page.saturating_sub(1);
                                                view.table = None;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        Button::new("category-next")
                                            .small()
                                            .ghost()
                                            .label(translate("sourceInspector.next"))
                                            .disabled(end == categories.len())
                                            .on_click(cx.listener(|view, _, _, cx| {
                                                if let DescriptionStatistics::Categorical {
                                                    categories,
                                                    ..
                                                } = &view.column.statistics
                                                    && (view.page + 1) * PAGE_ROWS
                                                        < categories.len()
                                                {
                                                    view.page += 1;
                                                    view.table = None;
                                                    cx.notify();
                                                }
                                            })),
                                    ),
                            )
                        }),
                )
            })
            .into_any_element()
    }
}

impl Render for ColumnSummary {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.prepare_table(window, cx);
        Collapsible::new()
            .w_full()
            .min_w_0()
            .open(self.open)
            .child(
                heading(
                    "description-column",
                    self.column.name.clone(),
                    self.open,
                    cx,
                )
                .on_click(cx.listener(|view, _, _, cx| {
                    view.open = !view.open;
                    cx.notify();
                })),
            )
            .when(self.open, |section| {
                section.content(
                    div()
                        .min_w_0()
                        .py_1()
                        .pl_2()
                        .text_xs()
                        .child(self.summary(cx))
                        .child(self.frequencies(cx)),
                )
            })
    }
}

fn code(value: &TabularScalar) -> String {
    match value {
        TabularScalar::Null => "—".into(),
        TabularScalar::String(value) if value.is_empty() => "\"\"".into(),
        TabularScalar::String(value) => value.to_string(),
        TabularScalar::Integer(value) => value.to_string(),
        TabularScalar::Unsigned(value) => value.to_string(),
        TabularScalar::Bool(value) => value.to_string(),
        TabularScalar::Float64(value) => value.as_f64().to_string(),
    }
}

fn metric(value: &TabularScalar) -> String {
    if let TabularScalar::Float64(value) = value
        && value.as_f64().fract() != 0.
    {
        format_number(value.as_f64(), 4)
    } else {
        code(value)
    }
}
