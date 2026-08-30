//! Stable dataset-profile DTOs and in-memory Polars profiling.

mod column_distribution;
mod column_stats;
mod dataset_overview;

pub use column_distribution::{
    CategoryCount, ColumnDistribution, HistogramBin, NumericDistribution, StringDistribution,
    compute_all_column_distributions, compute_column_distribution,
};
pub use column_stats::{
    ColumnStats, NumericColumnStats, StringColumnStats, compute_all_column_stats,
    compute_column_stats,
};
pub use dataset_overview::{
    DataCompleteness, DatasetOverview, SchemaOverview, SizeShape, compute_dataset_overview,
};

use std::collections::BTreeMap;

use polars::prelude::{DataType, StringChunked};

pub const DEFAULT_HISTOGRAM_BIN_COUNT: usize = 20;
pub const DEFAULT_TOP_CATEGORY_COUNT: usize = 15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProfileColumnKind {
    Numeric,
    Categorical,
    String,
    Temporal,
    Boolean,
}

pub fn profile_column_kind(data_type: &DataType) -> ProfileColumnKind {
    match data_type {
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
        | DataType::Decimal(_, _) => ProfileColumnKind::Numeric,
        DataType::Categorical(_, _) | DataType::Enum(_, _) => ProfileColumnKind::Categorical,
        DataType::String => ProfileColumnKind::String,
        DataType::Date | DataType::Time | DataType::Datetime(_, _) | DataType::Duration(_) => {
            ProfileColumnKind::Temporal
        }
        DataType::Boolean => ProfileColumnKind::Boolean,
        _ => ProfileColumnKind::String,
    }
}

pub fn profile_column_kind_from_name(data_type: &str) -> ProfileColumnKind {
    match data_type {
        "Int8" | "Int16" | "Int32" | "Int64" | "UInt8" | "UInt16" | "UInt32" | "UInt64"
        | "Float32" | "Float64" => ProfileColumnKind::Numeric,
        "Boolean" => ProfileColumnKind::Boolean,
        "Date" | "Time" => ProfileColumnKind::Temporal,
        value if value.starts_with("Decimal(") => ProfileColumnKind::Numeric,
        value if value.starts_with("Categorical") || value.starts_with("Enum") => {
            ProfileColumnKind::Categorical
        }
        value if value.starts_with("Datetime") || value.starts_with("Duration") => {
            ProfileColumnKind::Temporal
        }
        _ => ProfileColumnKind::String,
    }
}

pub fn format_histogram_bin_label(
    lower_bound: f64,
    upper_bound: f64,
    precision: usize,
    is_final_bin: bool,
) -> String {
    let closing_delimiter = if is_final_bin { ']' } else { ')' };
    format!("[{lower_bound:.precision$}, {upper_bound:.precision$}{closing_delimiter}")
}

fn is_numeric_dtype(data_type: &DataType) -> bool {
    profile_column_kind(data_type) == ProfileColumnKind::Numeric
}

fn string_frequencies(column: &StringChunked) -> BTreeMap<String, usize> {
    let mut frequencies = BTreeMap::new();
    for value in column.into_iter().flatten() {
        *frequencies.entry(value.to_owned()).or_insert(0) += 1;
    }
    frequencies
}

#[cfg(test)]
mod tests;
