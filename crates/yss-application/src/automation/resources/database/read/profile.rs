//! Explicit profile projection; runtime DTOs never enter the tool ledger directly.
use yss_dataset_profile::{ColumnDistribution, ColumnStats};
use yss_harness_contract::model::*;

pub(super) fn project(
    snapshot: yss_database_runtime::session_api::DatabaseProfileSnapshot,
) -> DatabaseProfileMetricSelection {
    DatabaseProfileMetricSelection {
        completeness: snapshot.completeness.map(|v| DatabaseCompleteness {
            row_count: v.size_shape.n_rows,
            column_count: v.size_shape.n_columns,
            total_nulls: v.data_completeness.total_nulls,
            null_ratio: v.data_completeness.null_ratio,
            columns_with_nulls: v.data_completeness.cols_with_nulls,
            rows_with_nulls: v.data_completeness.rows_with_nulls,
        }),
        statistics: snapshot.statistics.map(|values| {
            values
                .into_iter()
                .map(|value| match value {
                    ColumnStats::Numeric(v) => DatabaseColumnStatistics::Numeric {
                        column: v.column_name,
                        count: v.count,
                        null_count: v.null_count,
                        min: v.min,
                        max: v.max,
                        mean: v.mean,
                        median: v.median,
                        std: v.std,
                        variance: v.variance,
                    },
                    ColumnStats::String(v) => DatabaseColumnStatistics::Categorical {
                        column: v.column_name,
                        count: v.count,
                        null_count: v.null_count,
                        empty_count: v.empty_count,
                        valid_ratio: v.valid_ratio,
                        unique: v.unique,
                        mode: v.mode,
                        mode_count: v.mode_count,
                    },
                })
                .collect()
        }),
        distributions: snapshot.distributions.map(|values| {
            values
                .into_iter()
                .map(|value| match value {
                    ColumnDistribution::Numeric(v) => DatabaseColumnDistribution::Numeric {
                        column: v.column_name,
                        bins: v
                            .bins
                            .into_iter()
                            .map(|v| DatabaseDistributionCount {
                                label: v.label,
                                count: v.count,
                            })
                            .collect(),
                    },
                    ColumnDistribution::String(v) => DatabaseColumnDistribution::Categorical {
                        column: v.column_name,
                        other_count: v.other_count,
                        categories: v
                            .categories
                            .into_iter()
                            .map(|v| DatabaseDistributionCount {
                                label: v.label,
                                count: v.value,
                            })
                            .collect(),
                    },
                })
                .collect()
        }),
    }
}
