//! Presentation of the Application's existing statistical display contract.
use gpui_kit::component::{ActiveTheme, StyledExt};
use gpui_kit::{App, Div, IntoElement, div, prelude::*};
use yss_application::graph::results::report::presentation::{
    DisplayData, DisplayFormat, DisplayValue,
};

use crate::results::table::ResultGrid;

pub(crate) fn number(value: f64, decimals: usize) -> String {
    if value.is_nan() {
        "—".into()
    } else if value.is_infinite() {
        if value.is_sign_positive() {
            "∞"
        } else {
            "−∞"
        }
        .into()
    } else if value != 0. && (value.abs() < 10_f64.powi(-(decimals as i32)) || value.abs() >= 1e6) {
        format!("{value:.3e}")
    } else {
        format!("{value:.decimals$}")
    }
}

fn value(value: &DisplayValue, format: DisplayFormat) -> String {
    match value {
        DisplayValue::Text(value) => value.clone(),
        DisplayValue::Integer(value) => value.to_string(),
        DisplayValue::Number(value) => number(
            *value,
            if matches!(format, DisplayFormat::PValue) {
                3
            } else {
                4
            },
        ),
    }
}

pub(super) fn section(key: &str, content: impl IntoElement, cx: &App) -> Div {
    div()
        .w_full()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .border_1()
        .border_color(cx.theme().border)
        .rounded_lg()
        .child(
            div()
                .text_sm()
                .font_semibold()
                .child(crate::text::translate(key)),
        )
        .child(content)
}

pub(super) fn metrics(data: &DisplayData, cx: &App) -> Div {
    let items = match data {
        DisplayData::KeyValue { items } => items.as_slice(),
        DisplayData::StatCard { stat } => std::slice::from_ref(stat),
        DisplayData::Table { .. } => &[],
    };
    div()
        .flex()
        .flex_wrap()
        .gap_3()
        .children(items.iter().map(|metric| {
            div()
                .min_w(gpui_kit::px(150.))
                .flex_1()
                .p_2()
                .rounded_md()
                .bg(cx.theme().muted)
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(crate::text::translate(&format!(
                            "native.reports.metrics.{}",
                            metric.id
                        ))),
                )
                .child(
                    div()
                        .text_sm()
                        .font_family(cx.theme().mono_font_family.clone())
                        .child(value(&metric.value, metric.format)),
                )
        }))
}

pub(super) fn table(data: &DisplayData) -> Option<ResultGrid> {
    let DisplayData::Table { columns, rows } = data else {
        return None;
    };
    let names = columns
        .iter()
        .map(|column| crate::text::translate(&format!("native.reports.columns.{}", column.id)))
        .collect();
    let rows = rows
        .iter()
        .map(|row| {
            row.iter()
                .zip(columns)
                .map(|(cell, column)| {
                    if column.id == "source"
                        && let DisplayValue::Text(label) = cell
                    {
                        return crate::text::translate(&format!("native.reports.sources.{label}"));
                    }
                    value(cell, column.format)
                })
                .collect()
        })
        .collect();
    Some(ResultGrid::formatted(names, rows))
}
