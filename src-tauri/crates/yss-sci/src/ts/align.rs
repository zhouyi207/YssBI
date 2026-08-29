//! 时间序列对齐
//!
//! 将不规则时间序列补齐到规则时间轴，缺失位置为 null。
//! 使用 Series min/max + range 生成完整时间轴，再通过 Polars join 对齐，避免逐行转换。

use polars::prelude::*;

use crate::ts::types::TimeValue;

/// Polars Date  epoch: 1970-01-01
const EPOCH_DAYS_CE: i32 = 719163;

/// 从 Polars Series 提取 TimeValue 序列
///
/// 支持 Int64（数字时间）和 Date 类型
pub fn series_to_time_values(series: &Series) -> PolarsResult<Vec<TimeValue>> {
    let dtype = series.dtype();
    match dtype {
        DataType::Int64 => {
            let ca = series.i64()?;
            Ok(ca
                .into_iter()
                .filter_map(|v| v.map(TimeValue::Num))
                .collect())
        }
        DataType::Date => {
            let ca = series.date()?;
            let physical = ca.physical();
            let epoch =
                chrono::NaiveDate::from_num_days_from_ce_opt(EPOCH_DAYS_CE).unwrap_or_default();
            Ok(physical
                .into_iter()
                .filter_map(|v: Option<i32>| {
                    v.map(|d| TimeValue::Date(epoch + chrono::Duration::days(d as i64)))
                })
                .collect())
        }
        _ => Err(PolarsError::SchemaMismatch(
            format!("align: time column must be Int64 or Date, got {:?}", dtype).into(),
        )),
    }
}

/// 将 TimeValue 序列写回 Polars Series
pub fn time_values_to_series(name: &str, times: &[TimeValue]) -> PolarsResult<Series> {
    if times.is_empty() {
        return Ok(Series::new(name.into(), [] as [i64; 0]));
    }
    match &times[0] {
        TimeValue::Num(_) => {
            let vals: Vec<i64> = times
                .iter()
                .filter_map(|t| {
                    if let TimeValue::Num(v) = t {
                        Some(*v)
                    } else {
                        None
                    }
                })
                .collect();
            Ok(Series::new(name.into(), vals))
        }
        TimeValue::Date(_) => {
            let epoch =
                chrono::NaiveDate::from_num_days_from_ce_opt(EPOCH_DAYS_CE).unwrap_or_default();
            let vals: Vec<i32> = times
                .iter()
                .filter_map(|t| {
                    if let TimeValue::Date(d) = t {
                        Some((*d - epoch).num_days() as i32)
                    } else {
                        None
                    }
                })
                .collect();
            Ok(Int32Chunked::from_vec(name.into(), vals)
                .into_series()
                .cast(&DataType::Date)?)
        }
    }
}

/// 对齐时间序列：补齐时间轴，缺失处为 null
///
/// 复用 align_dataframe 的 Series min/max + join 逻辑，避免 iter/min/max 和 HashMap。
///
/// * `time_series` - 时间列（Int64 或 Date）
/// * `value_series` - 数值列（Float64）
/// * `interval` - 时间步长（数字时间为步数，日期为天数）
pub fn align_series(
    time_series: &Series,
    value_series: &Series,
    interval: i64,
) -> PolarsResult<(Series, Series)> {
    let time_name = time_series.name().as_str();
    let df = DataFrame::new(
        time_series.len(),
        vec![
            Column::from(time_series.clone()),
            Column::from(value_series.clone()),
        ],
    )?;
    let aligned = align_dataframe(&df, time_name, interval)?;
    let out_times = aligned
        .column(time_name)?
        .clone()
        .take_materialized_series();
    let out_values = aligned
        .column(value_series.name().as_str())?
        .clone()
        .take_materialized_series();
    Ok((out_times, out_values))
}

/// 生成完整时间轴 Series（min..=max，步长 interval）
fn full_time_range_series(
    time_series: &Series,
    time_series_name: &str,
    interval: i64,
) -> PolarsResult<Series> {
    let dtype = time_series.dtype();
    match dtype {
        DataType::Int64 => {
            let ca = time_series.i64()?;
            let min_val = ca.min().ok_or_else(|| {
                PolarsError::ComputeError("align_dataframe: empty time series".into())
            })?;
            let max_val = ca.max().ok_or_else(|| {
                PolarsError::ComputeError("align_dataframe: empty time series".into())
            })?;
            let full: Vec<i64> = (0..)
                .map(|i| min_val + i * interval)
                .take_while(|&x| x <= max_val)
                .collect();
            Ok(Series::new(time_series_name.into(), full))
        }
        DataType::Date => {
            let ca = time_series.date()?;
            let physical = ca.physical();
            let min_val = physical.min().ok_or_else(|| {
                PolarsError::ComputeError("align_dataframe: empty time series".into())
            })?;
            let max_val = physical.max().ok_or_else(|| {
                PolarsError::ComputeError("align_dataframe: empty time series".into())
            })?;
            let full: Vec<i32> = (0..)
                .map(|i| min_val + (i as i32) * (interval as i32))
                .take_while(|&x| x <= max_val)
                .collect();
            let s = Int32Chunked::from_vec(time_series_name.into(), full)
                .into_series()
                .cast(&DataType::Date)?;
            Ok(s.with_name(time_series_name.into()))
        }
        _ => Err(PolarsError::SchemaMismatch(
            format!(
                "align_dataframe: time column must be Int64 or Date, got {:?}",
                dtype
            )
            .into(),
        )),
    }
}

/// 检查时间列是否存在重复值，若有则返回错误
pub fn check_no_duplicate_times(series: &Series) -> PolarsResult<()> {
    let n = series.len();
    let n_unique = series.n_unique().map_err(|e| {
        PolarsError::ComputeError(format!("check_no_duplicate_times: {}", e).into())
    })?;
    if n != n_unique {
        return Err(PolarsError::ComputeError(
            format!(
                "TS Align: 时间列存在重复值 ({} 行中有 {} 个唯一值)，拒绝处理",
                n, n_unique
            )
            .into(),
        ));
    }
    Ok(())
}

/// 从时间列推断最小间隔（排序去重后相邻时间差的最小值）
///
/// 若数据为空或仅一个点，返回 1。
pub fn infer_interval(series: &Series) -> PolarsResult<i64> {
    let dtype = series.dtype();
    match dtype {
        DataType::Int64 => {
            let ca = series.i64()?;
            let mut sorted: Vec<i64> = ca.into_no_null_iter().collect();
            sorted.sort_unstable();
            sorted.dedup();
            if sorted.len() < 2 {
                return Ok(1);
            }
            let min_gap = sorted.windows(2).map(|w| w[1] - w[0]).min().unwrap_or(1);
            Ok(min_gap.max(1))
        }
        DataType::Date => {
            let ca = series.date()?;
            let physical = ca.physical();
            let mut sorted: Vec<i32> = physical.into_no_null_iter().collect();
            sorted.sort_unstable();
            sorted.dedup();
            if sorted.len() < 2 {
                return Ok(1);
            }
            let min_gap = sorted
                .windows(2)
                .map(|w| (w[1] - w[0]) as i64)
                .min()
                .unwrap_or(1);
            Ok(min_gap.max(1))
        }
        _ => Err(PolarsError::SchemaMismatch(
            format!(
                "infer_interval: time column must be Int64 or Date, got {:?}",
                dtype
            )
            .into(),
        )),
    }
}

/// 对齐 DataFrame：以指定时间列为轴补齐，所有列在缺失时间点为 null
///
/// 使用 Series min/max 获取范围，生成完整时间轴，再通过 Polars left join 对齐，避免逐行转换。
///
/// * `df` - 输入 DataFrame
/// * `time_series_name` - 时间列名（Int64 或 Date）
/// * `interval` - 时间步长（数字时间为步数，日期为天数）
pub fn align_dataframe(
    df: &DataFrame,
    time_series_name: &str,
    interval: i64,
) -> PolarsResult<DataFrame> {
    let time_col = df.column(time_series_name).map_err(|_| {
        PolarsError::ColumnNotFound(
            format!("align_dataframe: column '{}' not found", time_series_name).into(),
        )
    })?;
    let time_series = time_col.clone().take_materialized_series();

    let full_times = full_time_range_series(&time_series, time_series_name, interval)?;
    let full_df = DataFrame::new(full_times.len(), vec![Column::from(full_times)])?;

    full_df.join(
        df,
        [time_series_name],
        [time_series_name],
        JoinArgs::new(JoinType::Left),
        None,
    )
}
