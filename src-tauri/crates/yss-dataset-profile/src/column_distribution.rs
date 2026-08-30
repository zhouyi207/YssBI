use polars::prelude::{Column, DataFrame, DataType, Float64Chunked};
use serde::Serialize;

use crate::{
    DEFAULT_HISTOGRAM_BIN_COUNT, DEFAULT_TOP_CATEGORY_COUNT, format_histogram_bin_label,
    is_numeric_dtype, string_frequencies,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistogramBin {
    pub label: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryCount {
    pub label: String,
    pub value: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NumericDistribution {
    pub column_name: String,
    pub kind: &'static str,
    pub bins: Vec<HistogramBin>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StringDistribution {
    pub column_name: String,
    pub kind: &'static str,
    pub categories: Vec<CategoryCount>,
    pub other_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum ColumnDistribution {
    Numeric(NumericDistribution),
    String(StringDistribution),
}

pub fn compute_column_distribution(column: &Column) -> ColumnDistribution {
    let name = column.name().to_string();
    if is_numeric_dtype(column.dtype()) {
        compute_numeric_distribution(column, name)
    } else {
        compute_string_distribution(column, name)
    }
}

fn compute_numeric_distribution(column: &Column, name: String) -> ColumnDistribution {
    let values = column
        .cast(&DataType::Float64)
        .ok()
        .and_then(|column| column.f64().ok().cloned());
    let bins = values
        .as_ref()
        .map(|values| build_histogram(values, DEFAULT_HISTOGRAM_BIN_COUNT))
        .unwrap_or_default();

    ColumnDistribution::Numeric(NumericDistribution {
        column_name: name,
        kind: "numeric",
        bins,
    })
}

fn build_histogram(values: &Float64Chunked, number_of_bins: usize) -> Vec<HistogramBin> {
    let values = values
        .into_no_null_iter()
        .filter(|value| value.is_finite())
        .collect::<Vec<_>>();
    if values.is_empty() || number_of_bins == 0 {
        return Vec::new();
    }

    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if (max - min).abs() < f64::EPSILON {
        return vec![HistogramBin {
            label: format!("{min:.2}"),
            count: values.len(),
        }];
    }

    let bin_width = (max - min) / number_of_bins as f64;
    let mut counts = vec![0; number_of_bins];
    for value in values {
        let index = (((value - min) / bin_width) as usize).min(number_of_bins - 1);
        counts[index] += 1;
    }

    let precision = usize::from(bin_width < 1.0) + 1;
    counts
        .into_iter()
        .enumerate()
        .map(|(index, count)| {
            let lower_bound = min + index as f64 * bin_width;
            let upper_bound = lower_bound + bin_width;
            HistogramBin {
                label: format_histogram_bin_label(
                    lower_bound,
                    upper_bound,
                    precision,
                    index + 1 == number_of_bins,
                ),
                count,
            }
        })
        .collect()
}

fn compute_string_distribution(column: &Column, name: String) -> ColumnDistribution {
    let string_column = column.cast(&DataType::String).ok();
    let frequencies = string_column
        .as_ref()
        .and_then(|column| column.str().ok())
        .map(string_frequencies)
        .unwrap_or_default();
    let mut entries = frequencies
        .into_iter()
        .filter(|(value, _)| !value.is_empty())
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    let total = entries.iter().map(|(_, count)| *count).sum::<usize>();
    let top_count = entries.len().min(DEFAULT_TOP_CATEGORY_COUNT);
    let top_sum = entries
        .iter()
        .take(top_count)
        .map(|(_, count)| *count)
        .sum::<usize>();
    let categories = entries
        .into_iter()
        .take(top_count)
        .map(|(label, value)| CategoryCount { label, value })
        .collect();

    ColumnDistribution::String(StringDistribution {
        column_name: name,
        kind: "string",
        categories,
        other_count: total.saturating_sub(top_sum),
    })
}

pub fn compute_all_column_distributions(dataframe: &DataFrame) -> Vec<ColumnDistribution> {
    dataframe
        .columns()
        .iter()
        .map(compute_column_distribution)
        .collect()
}
