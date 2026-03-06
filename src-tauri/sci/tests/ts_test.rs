//! 时间序列模块测试

use polars::prelude::*;
use yss_sci::ts;

#[test]
fn test_ts_diff() {
    let values = Series::new("x".into(), &[10.0, 20.0, 30.0, 40.0]);
    let result = ts::diff::ts_diff(&values, 1).unwrap();
    let expected: Vec<Option<f64>> = vec![None, Some(10.0), Some(10.0), Some(10.0)];
    let got: Vec<Option<f64>> = result.f64().unwrap().into_iter().map(|v| v).collect();
    assert_eq!(got, expected);
}

#[test]
fn test_ts_pct_change() {
    let values = Series::new("x".into(), &[100.0, 110.0, 121.0]);
    let result = ts::pct_change::ts_pct_change(&values, 1).unwrap();
    let got: Vec<Option<f64>> = result.f64().unwrap().into_iter().map(|v| v).collect();
    assert_eq!(got[0], None);
    assert!((got[1].unwrap() - 0.1).abs() < 1e-10);
    assert!((got[2].unwrap() - 0.1).abs() < 1e-10);
}

#[test]
fn test_rolling_mean() {
    let values = Series::new("x".into(), &[1.0, 2.0, 3.0, 4.0, 5.0]);
    let result = ts::rolling::rolling_mean(&values, 3).unwrap();
    let got: Vec<Option<f64>> = result.f64().unwrap().into_iter().map(|v| v).collect();
    assert_eq!(got[0], None);
    assert_eq!(got[1], None);
    assert_eq!(got[2], Some(2.0)); // (1+2+3)/3
    assert_eq!(got[3], Some(3.0)); // (2+3+4)/3
    assert_eq!(got[4], Some(4.0)); // (3+4+5)/3
}

#[test]
fn test_check_no_duplicate_times_rejects_duplicates() {
    let times = Series::new("t".into(), &[1_i64, 2, 2, 4]);
    let err = yss_sci::ts::align::check_no_duplicate_times(&times).unwrap_err();
    assert!(err.to_string().contains("重复"));
}

#[test]
fn test_check_no_duplicate_times_accepts_unique() {
    let times = Series::new("t".into(), &[1_i64, 2, 3, 4]);
    assert!(yss_sci::ts::align::check_no_duplicate_times(&times).is_ok());
}

#[test]
fn test_infer_interval_int64() {
    let times = Series::new("t".into(), &[1_i64, 3, 5, 9]);
    let interval = yss_sci::ts::align::infer_interval(&times).unwrap();
    assert_eq!(interval, 2); // min gap between 1,3,5,9
}

#[test]
fn test_infer_interval_single_value() {
    let times = Series::new("t".into(), &[42_i64]);
    let interval = yss_sci::ts::align::infer_interval(&times).unwrap();
    assert_eq!(interval, 1);
}

#[test]
fn test_align_dataframe() {
    let times = Series::new("t".into(), &[1_i64, 2, 4, 5]);
    let x = Series::new("x".into(), &[10.0, 20.0, 30.0, 40.0]);
    let y = Series::new("y".into(), &[100.0, 200.0, 300.0, 400.0]);
    let df = DataFrame::new(
        4,
        vec![
            polars::prelude::Column::from(times),
            polars::prelude::Column::from(x),
            polars::prelude::Column::from(y),
        ],
    )
    .unwrap();

    let aligned = yss_sci::ts::align::align_dataframe(&df, "t", 1).unwrap();

    assert_eq!(aligned.height(), 5); // t=1,2,3,4,5
    assert_eq!(aligned.width(), 3);  // t, x, y

    let t_col = aligned.column("t").unwrap().clone().take_materialized_series();
    let t_vals: Vec<i64> = t_col.i64().unwrap().into_iter().filter_map(|v| v).collect();
    assert_eq!(t_vals, vec![1, 2, 3, 4, 5]);

    let x_col = aligned.column("x").unwrap().clone().take_materialized_series();
    let x_vals: Vec<Option<f64>> = x_col.f64().unwrap().into_iter().map(|v| v).collect();
    assert_eq!(x_vals[0], Some(10.0));
    assert_eq!(x_vals[1], Some(20.0));
    assert_eq!(x_vals[2], None); // t=3 缺失
    assert_eq!(x_vals[3], Some(30.0));
    assert_eq!(x_vals[4], Some(40.0));
}

#[test]
fn test_ts_lag_numeric() {
    let times = Series::new("t".into(), &[1_i64, 2, 4, 5]);
    let values = Series::new("x".into(), &[10.0, 20.0, 30.0, 40.0]);
    let (t_out, lag_out) = ts::lag::ts_lag(&times, &values, 1, 1).unwrap();

    // 对齐后: t=1,2,3,4,5; values=10,20,NA,30,40
    // lag1: NA,1,2,3,4 -> NA,10,20,NA,30
    let t_vals: Vec<i64> = t_out.i64().unwrap().into_iter().filter_map(|v| v).collect();
    let lag_vals: Vec<Option<f64>> = lag_out.f64().unwrap().into_iter().map(|v| v).collect();

    assert_eq!(t_vals, vec![1, 2, 3, 4, 5]);
    assert_eq!(lag_vals[0], None);
    assert_eq!(lag_vals[1], Some(10.0));
    assert_eq!(lag_vals[2], Some(20.0));
    assert_eq!(lag_vals[3], None);
    assert_eq!(lag_vals[4], Some(30.0));
}
