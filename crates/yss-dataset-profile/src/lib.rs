//! Stable dataset-profile DTOs, column categories, and display formatting.

mod column_distribution;
mod column_stats;
mod dataset_overview;

pub use column_distribution::{
    CategoryCount, ColumnDistribution, HistogramBin, NumericDistribution, StringDistribution,
};
pub use column_stats::{ColumnStats, NumericColumnStats, StringColumnStats};
pub use dataset_overview::{DataCompleteness, DatasetOverview, SchemaOverview, SizeShape};

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

pub fn format_histogram_bin_label(
    lower_bound: f64,
    upper_bound: f64,
    precision: usize,
    is_final_bin: bool,
) -> String {
    let closing_delimiter = if is_final_bin { ']' } else { ')' };
    format!("[{lower_bound:.precision$}, {upper_bound:.precision$}{closing_delimiter}")
}
