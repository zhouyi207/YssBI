//! 滚动窗口
//!
//! rolling mean，复杂度 O(n × window)

use polars::prelude::*;

/// 对 Polars Series 做滚动均值
///
/// * `value_series` - 数值列（Float64）
/// * `window` - 窗口大小
///
/// 前 (window - 1) 个为 null；窗口内若有 null 则输出 null
pub fn rolling_mean(value_series: &Series, window: usize) -> PolarsResult<Series> {
    let values: Vec<Option<f64>> = value_series.f64()?.into_iter().map(|v| v).collect();
    let n = values.len();
    let mut out = Vec::with_capacity(n);

    for i in 0..n {
        if i + 1 < window {
            out.push(None);
            continue;
        }

        let start = i + 1 - window;
        let mut sum = 0.0;
        let mut has_null = false;
        for j in start..=i {
            match values[j] {
                Some(v) => sum += v,
                None => has_null = true,
            }
        }
        out.push(if has_null {
            None
        } else {
            Some(sum / window as f64)
        });
    }

    Ok(Series::from_iter(out.into_iter())
        .with_name(format!("{}_rolling_mean{}", value_series.name(), window).into()))
}
