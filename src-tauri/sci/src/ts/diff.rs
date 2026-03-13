//! 差分
//!
//! ts_diff: 按位置做 diff（y_t - y_{t-lag}）
//! ts_diff_with_time: 与 Stata D. 一致，仅当 time[i] - time[i-lag] == interval*lag 时输出 diff，不跨 gap

use polars::prelude::*;

/// 从时间 Series 提取 i64 序列（用于 gap 判断）
fn time_series_to_i64(time_series: &Series) -> PolarsResult<Vec<i64>> {
    let dtype = time_series.dtype();
    match dtype {
        DataType::Int64 => {
            let ca = time_series.i64()?;
            Ok(ca
                .into_iter()
                .map(|v| v.unwrap_or(0))
                .collect())
        }
        DataType::Date => {
            let ca = time_series.date()?;
            let physical = ca.physical();
            Ok(physical
                .into_iter()
                .map(|v| v.unwrap_or(0) as i64)
                .collect())
        }
        _ => Err(PolarsError::SchemaMismatch(
            format!("ts_diff_with_time: time must be Int64 or Date, got {:?}", dtype).into(),
        )),
    }
}

/// 对 Polars Series 做差分（按位置，不依赖时间）
///
/// * `value_series` - 数值列（Float64）
/// * `lag` - 差分阶数
///
/// 返回新 Series，前 lag 个为 null；若当前或 lag 前有 null 则输出 null
pub fn ts_diff(value_series: &Series, lag: usize) -> PolarsResult<Series> {
    let values: Vec<Option<f64>> = value_series.f64()?.into_iter().map(|v| v).collect();
    let n = values.len();
    let mut out = Vec::with_capacity(n);

    for i in 0..n {
        if i < lag {
            out.push(None);
        } else {
            match (values[i], values[i - lag]) {
                (Some(a), Some(b)) => out.push(Some(a - b)),
                _ => out.push(None),
            }
        }
    }

    Ok(Series::from_iter(out.into_iter())
        .with_name(format!("{}_diff{}", value_series.name(), lag).into()))
}

/// 时间感知的差分：仅当 time[i] - time[i-lag] == interval * lag 时输出 diff
///
/// 与 Stata D. 一致，不跨 gap。当时间间隔不等于 interval*lag 时输出 null。
///
/// * `time_series` - 时间列（Int64 或 Date）
/// * `value_series` - 数值列（Float64）
/// * `lag` - 差分阶数
/// * `interval` - 期望的时间步长（delta），默认 1
pub fn ts_diff_with_time(
    time_series: &Series,
    value_series: &Series,
    lag: usize,
    interval: i64,
) -> PolarsResult<Series> {
    let times = time_series_to_i64(time_series)?;
    let values: Vec<Option<f64>> = value_series.f64()?.into_iter().map(|v| v).collect();
    let n = times.len();
    if values.len() != n {
        return Err(PolarsError::ComputeError(
            "ts_diff_with_time: time and value must have same length".into(),
        ));
    }
    let expected_gap = interval * (lag as i64);
    let mut out = Vec::with_capacity(n);

    for i in 0..n {
        if i < lag {
            out.push(None);
        } else {
            let time_gap = times[i] - times[i - lag];
            if time_gap == expected_gap {
                match (values[i], values[i - lag]) {
                    (Some(a), Some(b)) => out.push(Some(a - b)),
                    _ => out.push(None),
                }
            } else {
                out.push(None);
            }
        }
    }

    Ok(Series::from_iter(out.into_iter())
        .with_name(format!("{}_diff{}", value_series.name(), lag).into()))
}
