//! Arrow time-series input preparation.
use arrow::array::{Array, Date32Array, Float64Array, Int64Array};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use std::sync::Arc;
use yss_sci_runtime::data::{PreparationError, time_series as ts};

#[test]
fn test_ts_diff() {
    let values = Float64Array::from(vec![10., 20., 30., 40.]);
    assert_eq!(
        ts::diff::ts_diff(&values, 1)
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        vec![None, Some(10.), Some(10.), Some(10.)]
    );
}
#[test]
fn test_ts_pct_change() {
    let values = Float64Array::from(vec![100., 110., 121.]);
    let result = ts::pct_change::ts_pct_change(&values, 1).unwrap();
    assert!(result.is_null(0));
    assert!((result.value(1) - 0.1).abs() < 1e-10);
    assert!((result.value(2) - 0.1).abs() < 1e-10);
}
#[test]
fn test_rolling_mean() {
    let values = Float64Array::from(vec![1., 2., 3., 4., 5.]);
    assert_eq!(
        ts::rolling::rolling_mean(&values, 3)
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        vec![None, None, Some(2.), Some(3.), Some(4.)]
    );
}
#[test]
fn test_check_no_duplicate_times_rejects_duplicates() {
    let times = Int64Array::from(vec![1, 2, 2, 4]);
    assert!(matches!(
        ts::align::check_no_duplicate_times(&times),
        Err(PreparationError::DuplicateTime)
    ));
}
#[test]
fn test_check_no_duplicate_times_accepts_unique() {
    assert!(ts::align::check_no_duplicate_times(&Int64Array::from(vec![1, 2, 3, 4])).is_ok());
}
#[test]
fn test_infer_interval_int64() {
    assert_eq!(
        ts::align::infer_interval(&Int64Array::from(vec![1, 3, 5, 9])).unwrap(),
        2
    );
}
#[test]
fn test_infer_interval_single_value() {
    assert_eq!(
        ts::align::infer_interval(&Int64Array::from(vec![42])).unwrap(),
        1
    );
}
#[test]
fn test_align_batch() {
    let schema = Arc::new(Schema::new(vec![
        Field::new("t", DataType::Int64, false),
        Field::new("x", DataType::Float64, false),
        Field::new("y", DataType::Float64, false),
    ]));
    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(Int64Array::from(vec![1, 2, 4, 5])),
            Arc::new(Float64Array::from(vec![10., 20., 30., 40.])),
            Arc::new(Float64Array::from(vec![100., 200., 300., 400.])),
        ],
    )
    .unwrap();
    let aligned = ts::align::align_batch(&batch, "t", 1).unwrap();
    assert_eq!((aligned.num_rows(), aligned.num_columns()), (5, 3));
    assert_eq!(
        aligned
            .column(0)
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap()
            .values()
            .as_ref(),
        &[1, 2, 3, 4, 5]
    );
    assert_eq!(
        aligned
            .column(1)
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap()
            .iter()
            .collect::<Vec<_>>(),
        vec![Some(10.), Some(20.), None, Some(30.), Some(40.)]
    );
}
#[test]
fn test_ts_lag_numeric() {
    let times = Int64Array::from(vec![1, 2, 4, 5]);
    let values = Float64Array::from(vec![10., 20., 30., 40.]);
    let (times, _, lag) = ts::lag::ts_lag(&times, &values, 1, 1).unwrap();
    assert_eq!(
        times
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap()
            .values()
            .as_ref(),
        &[1, 2, 3, 4, 5]
    );
    assert_eq!(
        lag.iter().collect::<Vec<_>>(),
        vec![None, Some(10.), Some(20.), None, Some(30.)]
    );
}

#[test]
fn arrow_dates_nulls_and_alignment_limits_are_checked_before_allocation() {
    let dates = Date32Array::from(vec![0, 1, 3]);
    let values = Float64Array::from(vec![Some(10.), None, Some(30.)]);
    let (time, current, lag) = ts::lag::ts_lag(&dates, &values, 1, 1).unwrap();
    assert_eq!(time.data_type(), &DataType::Date32);
    assert_eq!(
        current.iter().collect::<Vec<_>>(),
        vec![Some(10.), None, None, Some(30.)]
    );
    assert_eq!(
        lag.iter().collect::<Vec<_>>(),
        vec![None, Some(10.), None, None]
    );
    assert!(
        ts::diff::ts_diff_with_time(&dates, &values, 1, 1)
            .unwrap()
            .iter()
            .all(|value| value.is_none())
    );
    assert!(matches!(
        ts::align::align_series(&dates, &values, 0),
        Err(PreparationError::Interval)
    ));
    assert!(matches!(
        ts::rolling::rolling_mean(&values, 0),
        Err(PreparationError::Interval)
    ));
    let huge = Int64Array::from(vec![i64::MIN, i64::MAX]);
    assert!(matches!(
        ts::align::align_series(&huge, &Float64Array::from(vec![1., 2.]), 1),
        Err(PreparationError::MemoryLimit)
    ));
    let missing = Int64Array::from(vec![Some(1), None]);
    assert!(matches!(
        ts::align::align_series(&missing, &Float64Array::from(vec![1., 2.]), 1),
        Err(PreparationError::NullTime)
    ));
}

#[test]
fn panel_arrow_adapter_preserves_identity_metadata_and_existing_gap_difference() {
    use yss_sci_runtime::data::panel;
    let schema = Arc::new(Schema::new(vec![
        Field::new("entity", DataType::Int64, false)
            .with_metadata([(String::from("column_id"), String::from("entity-id"))].into()),
        Field::new("date", DataType::Date32, false),
        Field::new("x", DataType::Float64, false),
    ]));
    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(Int64Array::from(vec![10, 10, 20, 20, 20])),
            Arc::new(Date32Array::from(vec![0, 2, 0, 1, 2])),
            Arc::new(Float64Array::from(vec![10., 30., 100., 150., 160.])),
        ],
    )
    .unwrap();
    let aligned = panel::align_batch(&batch, "entity", "date", Some(1)).unwrap();
    assert_eq!(aligned.num_rows(), 6);
    assert_eq!(aligned.schema().field(0), batch.schema().field(0));
    assert_eq!(aligned.column(1).data_type(), &DataType::Date32);
    assert!(aligned.column(2).is_null(1));
    let diff = panel::diff_batch(&aligned, "entity", "date").unwrap();
    assert_eq!(
        diff.column(0)
            .as_any()
            .downcast_ref::<Int64Array>()
            .unwrap()
            .values()
            .as_ref(),
        &[10, 20, 20]
    );
    assert_eq!(
        diff.column(2)
            .as_any()
            .downcast_ref::<Float64Array>()
            .unwrap()
            .values()
            .as_ref(),
        &[20., 50., 10.]
    );
}
