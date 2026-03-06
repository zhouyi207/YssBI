//! 差分
//!
//! 不依赖时间，按位置做 diff：y_t - y_{t-lag}

use polars::prelude::*;

/// 对 Polars Series 做差分
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
