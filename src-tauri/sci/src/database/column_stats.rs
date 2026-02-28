use polars::prelude::*;
use serde::Serialize;
use std::collections::HashMap;

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

pub fn compute_column_stats(col: &Column) -> ColumnStats {
    let name = col.name().to_string();
    let dtype = col.dtype().clone();
    let dtype_str = format!("{:?}", dtype);
    let len = col.len();
    let null_count = col.null_count();

    if is_numeric_dtype(&dtype) {
        compute_numeric_stats(col, name, dtype_str, len, null_count)
    } else {
        compute_string_stats(col, name, dtype_str, len, null_count)
    }
}

fn compute_numeric_stats(
    col: &Column,
    name: String,
    dtype_str: String,
    count: usize,
    null_count: usize,
) -> ColumnStats {
    let ca = col.cast(&DataType::Float64).ok().and_then(|c| {
        c.f64().ok().cloned()
    });

    let (min, max, mean, median_val, std_val) = match &ca {
        Some(f64_ca) => {
            let min = f64_ca.min();
            let max = f64_ca.max();
            let mean = f64_ca.mean();
            let median = f64_ca.median();
            let std = f64_ca.std(1);
            (min, max, mean, median, std)
        }
        None => (None, None, None, None, None),
    };

    let variance = std_val.map(|s| s * s);

    ColumnStats::Numeric(NumericColumnStats {
        column_name: name,
        column_type: dtype_str,
        kind: "numeric",
        count,
        null_count,
        min,
        max,
        mean,
        median: median_val,
        std: std_val,
        variance,
    })
}

fn compute_string_stats(
    col: &Column,
    name: String,
    dtype_str: String,
    count: usize,
    null_count: usize,
) -> ColumnStats {
    let str_result = col.str();

    let empty_count = str_result
        .as_ref()
        .map(|ca| {
            ca.into_iter()
                .filter(|opt| opt.map(|s| s.is_empty()).unwrap_or(false))
                .count()
        })
        .unwrap_or(0);

    let valid_count = count.saturating_sub(null_count).saturating_sub(empty_count);
    let valid_ratio = if count > 0 {
        valid_count as f64 / count as f64
    } else {
        0.0
    };

    let unique = col.n_unique().unwrap_or(0);

    let (mode, mode_count) = str_result
        .as_ref()
        .map(|ca| find_mode_string(ca))
        .unwrap_or((None, 0));

    ColumnStats::String(StringColumnStats {
        column_name: name,
        column_type: dtype_str,
        kind: "string",
        count,
        null_count,
        empty_count,
        valid_ratio,
        unique,
        mode,
        mode_count,
    })
}

fn find_mode_string(ca: &StringChunked) -> (Option<String>, usize) {
    let mut freq: HashMap<&str, usize> = HashMap::new();
    for opt in ca.into_iter() {
        if let Some(s) = opt {
            *freq.entry(s).or_insert(0) += 1;
        }
    }
    freq.into_iter()
        .max_by_key(|(_, count)| *count)
        .map(|(val, count)| (Some(val.to_string()), count))
        .unwrap_or((None, 0))
}

pub fn compute_all_column_stats(df: &DataFrame) -> Vec<ColumnStats> {
    df.get_columns()
        .iter()
        .map(|col| compute_column_stats(col))
        .collect()
}
