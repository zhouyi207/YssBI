use polars::prelude::{Column, DataFrame, DataType, NamedFrom, Series};

use crate::{
    ColumnDistribution, ColumnStats, ProfileColumnKind, compute_column_distribution,
    compute_column_stats, compute_dataset_overview, profile_column_kind,
};

fn dataframe(row_count: usize, series: Vec<Series>) -> DataFrame {
    DataFrame::new(row_count, series.into_iter().map(Column::from).collect())
        .expect("test dataframe should be valid")
}

#[test]
fn string_profiles_are_stable_and_exclude_empty_values_from_the_mode() {
    let labels = Column::from(Series::new(
        "labels".into(),
        &[
            Some("beta"),
            Some("alpha"),
            Some("beta"),
            Some("alpha"),
            Some(""),
            None,
        ],
    ));

    let ColumnStats::String(stats) = compute_column_stats(&labels) else {
        panic!("string input must produce string stats");
    };
    assert_eq!(stats.empty_count, 1);
    assert_eq!(stats.unique, 3);
    assert_eq!(stats.mode.as_deref(), Some("alpha"));
    assert_eq!(stats.mode_count, 2);

    let ColumnDistribution::String(distribution) = compute_column_distribution(&labels) else {
        panic!("string input must produce a string distribution");
    };
    assert_eq!(
        distribution
            .categories
            .iter()
            .map(|category| (category.label.as_str(), category.value))
            .collect::<Vec<_>>(),
        vec![("alpha", 2), ("beta", 2)]
    );
}

#[test]
fn non_string_columns_use_the_same_string_projection_as_physical_profiles() {
    let flags = Column::from(Series::new("flags".into(), &[true, false, true]));

    let ColumnStats::String(stats) = compute_column_stats(&flags) else {
        panic!("boolean input must produce projected string stats");
    };
    assert_eq!(stats.mode.as_deref(), Some("true"));
    assert_eq!(stats.mode_count, 2);

    let ColumnDistribution::String(distribution) = compute_column_distribution(&flags) else {
        panic!("boolean input must produce a projected string distribution");
    };
    assert_eq!(distribution.categories[0].label, "true");
    assert_eq!(distribution.categories[0].value, 2);
}

#[test]
fn numeric_histograms_ignore_non_finite_values_and_close_the_final_bin() {
    let values = Column::from(Series::new(
        "values".into(),
        &[1.0, 2.0, f64::NAN, f64::INFINITY],
    ));
    let ColumnDistribution::Numeric(distribution) = compute_column_distribution(&values) else {
        panic!("numeric input must produce a numeric distribution");
    };
    let ColumnStats::Numeric(stats) = compute_column_stats(&values) else {
        panic!("numeric input must produce numeric stats");
    };

    assert_eq!(stats.min, Some(1.0));
    assert_eq!(stats.max, Some(2.0));
    assert_eq!(stats.mean, Some(1.5));
    assert_eq!(
        distribution.bins.iter().map(|bin| bin.count).sum::<usize>(),
        2
    );
    assert!(
        distribution
            .bins
            .last()
            .expect("finite values produce bins")
            .label
            .ends_with(']')
    );
}

#[test]
fn decimal_columns_follow_the_numeric_profile_path() {
    let decimal = Series::new("decimal".into(), &[100_i64, 250_i64])
        .cast(&DataType::Decimal(12, 2))
        .expect("integer values should cast to Decimal");
    let decimal = Column::from(decimal);

    let ColumnStats::Numeric(stats) = compute_column_stats(&decimal) else {
        panic!("Decimal input must produce numeric stats");
    };
    assert!(stats.min.is_some());
    assert!(stats.max.is_some());

    let ColumnDistribution::Numeric(distribution) = compute_column_distribution(&decimal) else {
        panic!("Decimal input must produce a numeric distribution");
    };
    assert!(!distribution.bins.is_empty());
}

#[test]
fn overview_uses_one_dtype_classifier_and_checked_cell_arithmetic() {
    assert_eq!(
        profile_column_kind(&DataType::Decimal(12, 2)),
        ProfileColumnKind::Numeric
    );
    assert_eq!(
        profile_column_kind(&DataType::Binary),
        ProfileColumnKind::String
    );

    let dataframe = dataframe(
        3,
        vec![
            Series::new("ids".into(), &[Some(1_i64), None, Some(1)]),
            Series::new("labels".into(), &["a", "b", "a"]),
            Series::new("flags".into(), &[true, false, true]),
        ],
    );
    let overview = compute_dataset_overview(&dataframe);

    assert_eq!(overview.size_shape.n_rows, 3);
    assert_eq!(overview.size_shape.n_columns, 3);
    assert_eq!(overview.schema_overview.numeric_cols, 1);
    assert_eq!(overview.schema_overview.string_cols, 1);
    assert_eq!(overview.schema_overview.bool_cols, 1);
    assert_eq!(overview.data_completeness.total_nulls, 1);
    assert_eq!(overview.data_completeness.rows_with_nulls, 1);
    assert_eq!(overview.data_completeness.cols_with_nulls, 1);
}
