use num_traits::{Float, One};
use polars::prelude::*;
use serde::Serialize;
use std::collections::HashMap;

const DEFAULT_BINS: usize = 20;
const DEFAULT_TOP_N: usize = 15;

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

fn is_numeric_dtype(dt: &DataType) -> bool {
    matches!(
        dt,
        DataType::Int8
            | DataType::Int16
            | DataType::Int32
            | DataType::Int64
            | DataType::UInt8
            | DataType::UInt16
            | DataType::UInt32
            | DataType::UInt64
            | DataType::Float32
            | DataType::Float64
    )
}

pub fn compute_column_distribution(col: &Column) -> ColumnDistribution {
    let name = col.name().to_string();
    if is_numeric_dtype(col.dtype()) {
        compute_numeric_distribution(col, name)
    } else {
        compute_string_distribution(col, name)
    }
}

fn compute_numeric_distribution(col: &Column, name: String) -> ColumnDistribution {
    let ca = col
        .cast(&DataType::Float64)
        .ok()
        .and_then(|c| c.f64().ok().cloned());

    let bins = match ca {
        Some(ref f64_ca) => build_histogram(f64_ca, DEFAULT_BINS),
        None => vec![],
    };

    ColumnDistribution::Numeric(NumericDistribution {
        column_name: name,
        kind: "numeric",
        bins,
    })
}

fn build_histogram(ca: &Float64Chunked, num_bins: usize) -> Vec<HistogramBin> {
    let values: Vec<f64> = ca.into_no_null_iter().collect();
    if values.is_empty() {
        return vec![];
    }

    let min = values
        .iter()
        .cloned()
        .fold(f64::infinity(), f64::min);
    let max = values
        .iter()
        .cloned()
        .fold(f64::neg_infinity(), f64::max);

    if (max - min).abs() < f64::epsilon() {
        return vec![HistogramBin {
            label: format!("{:.2}", min),
            count: values.len(),
        }];
    }

    let bin_width = (max - min) / num_bins as f64;
    let mut counts = vec![0usize; num_bins];

    for &v in &values {
        let idx = ((v - min) / bin_width) as usize;
        let idx = idx.min(num_bins - 1);
        counts[idx] += 1;
    }

    let precision = if bin_width >= f64::one() { 1 } else { 2 };

    counts
        .into_iter()
        .enumerate()
        .map(|(i, count)| {
            let lo = min + i as f64 * bin_width;
            let hi = lo + bin_width;
            HistogramBin {
                label: format!("[{:.p$}, {:.p$})", lo, hi, p = precision),
                count,
            }
        })
        .collect()
}

fn compute_string_distribution(col: &Column, name: String) -> ColumnDistribution {
    let str_result = col.str();

    let mut freq: HashMap<String, usize> = HashMap::new();
    if let Ok(ca) = str_result {
        for opt in ca.into_iter() {
            if let Some(s) = opt {
                if !s.is_empty() {
                    *freq.entry(s.to_string()).or_insert(0) += 1;
                }
            }
        }
    }

    let mut entries: Vec<(String, usize)> = freq.into_iter().collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1));

    let total: usize = entries.iter().map(|(_, c)| *c).sum();
    let top_n = entries.len().min(DEFAULT_TOP_N);
    let top_sum: usize = entries.iter().take(top_n).map(|(_, c)| *c).sum();

    let categories: Vec<CategoryCount> = entries
        .into_iter()
        .take(top_n)
        .map(|(label, value)| CategoryCount { label, value })
        .collect();

    ColumnDistribution::String(StringDistribution {
        column_name: name,
        kind: "string",
        categories,
        other_count: total.saturating_sub(top_sum),
    })
}

pub fn compute_all_column_distributions(df: &DataFrame) -> Vec<ColumnDistribution> {
    df.columns()
        .iter()
        .map(|col| compute_column_distribution(col))
        .collect()
}
