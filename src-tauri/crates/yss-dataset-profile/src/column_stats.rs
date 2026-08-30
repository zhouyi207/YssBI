use polars::prelude::*;
use serde::Serialize;

use crate::{is_numeric_dtype, string_frequencies};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NumericColumnStats {
    pub column_name: String,
    pub column_type: String,
    pub kind: &'static str,
    pub count: usize,
    pub null_count: usize,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub mean: Option<f64>,
    pub median: Option<f64>,
    pub std: Option<f64>,
    pub variance: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StringColumnStats {
    pub column_name: String,
    pub column_type: String,
    pub kind: &'static str,
    pub count: usize,
    pub null_count: usize,
    pub empty_count: usize,
    pub valid_ratio: f64,
    pub unique: usize,
    pub mode: Option<String>,
    pub mode_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ColumnStats {
    Numeric(NumericColumnStats),
    String(StringColumnStats),
}

pub fn compute_column_stats(column: &Column) -> ColumnStats {
    let name = column.name().to_string();
    let data_type = column.dtype().clone();
    let data_type_name = format!("{data_type:?}");
    let count = column.len();
    let null_count = column.null_count();

    if is_numeric_dtype(&data_type) {
        compute_numeric_stats(column, name, data_type_name, count, null_count)
    } else {
        compute_string_stats(column, name, data_type_name, count, null_count)
    }
}

fn compute_numeric_stats(
    column: &Column,
    name: String,
    data_type_name: String,
    count: usize,
    null_count: usize,
) -> ColumnStats {
    let values = column
        .cast(&DataType::Float64)
        .ok()
        .and_then(|column| column.f64().ok().cloned());
    let finite_values = values.as_ref().map(|values| {
        Float64Chunked::from_iter_values(
            column.name().clone(),
            values.into_no_null_iter().filter(|value| value.is_finite()),
        )
    });

    let (min, max, mean, median, standard_deviation) = match &finite_values {
        Some(values) => (
            values.min(),
            values.max(),
            values.mean(),
            values.median(),
            values.std(1),
        ),
        None => (None, None, None, None, None),
    };
    let variance = standard_deviation.map(|value| value * value);

    ColumnStats::Numeric(NumericColumnStats {
        column_name: name,
        column_type: data_type_name,
        kind: "numeric",
        count,
        null_count,
        min,
        max,
        mean,
        median,
        std: standard_deviation,
        variance,
    })
}

fn compute_string_stats(
    column: &Column,
    name: String,
    data_type_name: String,
    count: usize,
    null_count: usize,
) -> ColumnStats {
    let string_column = column.cast(&DataType::String).ok();
    let frequencies = string_column
        .as_ref()
        .and_then(|column| column.str().ok())
        .map(string_frequencies)
        .unwrap_or_default();
    let empty_count = frequencies.get("").copied().unwrap_or_default();
    let valid_count = count.saturating_sub(null_count).saturating_sub(empty_count);
    let valid_ratio = if count == 0 {
        0.0
    } else {
        valid_count as f64 / count as f64
    };
    let mode = frequencies
        .iter()
        .filter(|(value, _)| !value.is_empty())
        .max_by(|(left_value, left_count), (right_value, right_count)| {
            left_count
                .cmp(right_count)
                .then_with(|| right_value.cmp(left_value))
        })
        .map(|(value, count)| (Some(value.clone()), *count))
        .unwrap_or((None, 0));

    ColumnStats::String(StringColumnStats {
        column_name: name,
        column_type: data_type_name,
        kind: "string",
        count,
        null_count,
        empty_count,
        valid_ratio,
        unique: frequencies.len(),
        mode: mode.0,
        mode_count: mode.1,
    })
}

pub fn compute_all_column_stats(dataframe: &DataFrame) -> Vec<ColumnStats> {
    dataframe
        .columns()
        .iter()
        .map(compute_column_stats)
        .collect()
}
