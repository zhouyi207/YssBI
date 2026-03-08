//! 严格时间对齐的 lag
//!
//! 按时间轴对齐后做 lag，符合 Stata L. 语义。

use polars::prelude::*;

use crate::ts::align;

/// 对 Polars Series 做严格时间对齐的 lag
///
/// * `time_series` - 时间列（Int64 或 Date）
/// * `value_series` - 数值列（Float64）
/// * `lag` - lag 阶数
/// * `interval` - 时间步长（数字时间为步数，日期为天数）
///
/// 返回 (完整时间轴, 对齐后的当前值, lag 后的值，前 lag 个为 null)
pub fn ts_lag(
    time_series: &Series,
    value_series: &Series,
    lag: usize,
    interval: i64,
) -> PolarsResult<(Series, Series, Series)> {
    let (full_times, aligned) = align::align_series(time_series, value_series, interval)?;

    let n = aligned.len();
    let mut out = Vec::with_capacity(n);

    let aligned_vec: Vec<Option<f64>> = aligned.f64()?.into_iter().map(|v| v).collect();

    for i in 0..n {
        if i < lag {
            out.push(None);
        } else {
            out.push(aligned_vec[i - lag]);
        }
    }

    let out_series = Series::from_iter(out.into_iter())
        .with_name(format!("{}_lag{}", value_series.name(), lag).into());

    Ok((full_times, aligned, out_series))
}
