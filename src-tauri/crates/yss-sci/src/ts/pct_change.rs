//! 百分比变化
//!
//! (y_t - y_{t-lag}) / y_{t-lag}

use polars::prelude::*;

/// 对 Polars Series 做百分比变化
///
/// * `value_series` - 数值列（Float64）
/// * `lag` - 滞后阶数
///
/// 当 y_{t-lag} == 0 或存在 null 时返回 null
pub fn ts_pct_change(value_series: &Series, lag: usize) -> PolarsResult<Series> {
    let values: Vec<Option<f64>> = value_series.f64()?.into_iter().map(|v| v).collect();
    let n = values.len();
    let mut out = Vec::with_capacity(n);

    for i in 0..n {
        if i < lag {
            out.push(None);
            continue;
        }

        match (values[i], values[i - lag]) {
            (Some(curr), Some(prev)) => {
                if prev == 0.0 {
                    out.push(None);
                } else {
                    out.push(Some((curr - prev) / prev));
                }
            }
            _ => out.push(None),
        }
    }

    Ok(Series::from_iter(out.into_iter())
        .with_name(format!("{}_pct_change{}", value_series.name(), lag).into()))
}
