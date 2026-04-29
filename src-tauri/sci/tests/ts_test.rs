//! 时间序列模块测试

use ndarray::Array2;
use polars::prelude::*;
use yss_sci::ts;
use yss_sci::ts::unit_root::{adf_test, AdfRegression};
use yss_sci::ts::var::var_varsoc;
use yss_sci::ts::vec::{vec_estimate, VECConfig, VecTrendSpec};

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
    let (t_out, _aligned, lag_out) = ts::lag::ts_lag(&times, &values, 1, 1).unwrap();

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

#[test]
fn test_var_varsoc_shape_and_lr() {
    // T、K 足够大，两列独立非周期模式，避免任意阶 Z'Z 接近奇异
    let t = 80usize;
    let y = Array2::from_shape_fn((t, 2), |(i, j)| {
        let i = i as f64;
        if j == 0 {
            0.02 * i + (0.11 * i + 0.3).sin() * 3.0
        } else {
            -0.015 * i + (0.07 * i * i * 0.001 + 1.2).cos() * 2.5 + 5.0
        }
    });
    let r = var_varsoc(y, 3, Some(vec!["a".into(), "b".into()])).unwrap();
    assert_eq!(r.maxlag, 3);
    assert_eq!(r.num_observation, t - r.maxlag);
    // Stata 表：Lag 0 … maxlag
    assert_eq!(r.rows.len(), 4);
    assert_eq!(r.rows[0].lag, 0);
    assert!(r.rows[0].lr.is_none());
    assert_eq!(r.rows[1].lag, 1);
    assert!(r.rows[1].lr.is_some());
    assert_eq!(r.rows[1].lr_df, Some(4));
    assert!(r.rows[1].lr_p.unwrap() >= 0.0 && r.rows[1].lr_p.unwrap() <= 1.0);
}

#[test]
fn test_acf_pacf_and_breusch_godfrey_smoke() {
    let residuals = vec![1.0, -0.5, 0.25, -0.125, 0.0625, -0.03125, 0.015625, -0.0078125];
    let acf = ts::acf_pacf::acf(&residuals, 3);
    let pacf = ts::acf_pacf::pacf(&residuals, 3);
    assert_eq!(acf.len(), 4);
    assert_eq!(pacf.len(), 3);
    assert!((acf[0] - 1.0).abs() < 1e-12);
    assert!(pacf.iter().all(|v| v.is_finite()));

    let exog: Vec<Vec<f64>> = (0..residuals.len())
        .map(|i| vec![1.0, i as f64])
        .collect();
    let (bg, p) = ts::serial_correlation::breusch_godfrey(&residuals, &exog, 1, false)
        .expect("BG result");
    assert!(bg.is_finite());
    assert!((0.0..=1.0).contains(&p));
}

#[test]
fn test_adf_drift_returns_regression_stats() {
    let y: Vec<f64> = (0..100).map(|i| i as f64 + (i as f64 * 0.1).sin()).collect();
    let result = adf_test(&y, 0, true, false).unwrap();

    assert_eq!(result.lags, 0);
    assert_eq!(result.regression, AdfRegression::Drift);
    assert!(result.num_obs > 0);
    assert!(result.test_statistic.is_finite());
    assert!(result.p_value >= 0.0 && result.p_value <= 1.0);
    assert!(!result.regression_table.is_empty());
}

#[test]
fn test_vec_estimate_rejects_invalid_config() {
    let n = 80usize;
    let y = Array2::from_shape_fn((n, 2), |(i, j)| {
        let t = i as f64;
        let base = 0.05 * t + (0.1 * t).sin();
        if j == 0 {
            base
        } else {
            base + 0.2 + (0.13 * t).cos() * 0.01
        }
    });
    let config = VECConfig {
        trend_spec: VecTrendSpec::Constant,
        lags: 0,
        rank: 1,
        mlag: 2,
    };

    let err = vec_estimate(&y, &config, Some(vec!["y1".into(), "y2".into()]), None)
        .unwrap_err();
    assert!(err.contains("lags must be >= 1"));
}
