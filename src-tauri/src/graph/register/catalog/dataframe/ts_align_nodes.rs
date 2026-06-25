//! 时间序列节点
//!
//! - TS Align: 对齐时间序列
//! - TS Diff: 差分
//! - TS Pct Change: 百分比变化
//! - TS Rolling Mean: 滚动均值
//! - TS Lag: 严格时间对齐的滞后

use crate::database::polars_dtype_to_data_type;
use crate::graph::node::{passthrough_input_schema_resolver, NodeDefinition};
use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataSeriesValue, DataType, DataValue};
use polars::prelude::Series;
use std::sync::Arc;
use yss_sci::ts::align::{align_dataframe, align_series, check_no_duplicate_times, infer_interval};
use yss_sci::ts::{diff, pct_change, rolling};

pub fn register(registry: &NodeRegistry) {
    register_ts_align(registry);
    register_ts_diff(registry);
    register_ts_pct_change(registry);
    register_ts_rolling_mean(registry);
    register_ts_lag(registry);
}

fn register_ts_align(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "TS Align",
        vec!["Data".to_string(), "Time Series".to_string()],
    )
    .with_ui_style("dataframe")
    .with_localized_description(
        "对齐时间序列：补齐缺失时间点，拒绝重复时间。时间列需为 Int64 或 Date。",
        "Align time series: fill missing timestamps, reject duplicates. Time column must be Int64 or Date.",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "DataFrame",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataFrame),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Time Series Name",
            DataRole::Custom("time_series_name".to_string()),
            PinDataTypeDefinition::concrete(DataType::String),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Interval",
            DataRole::Custom("interval".to_string()),
            PinDataTypeDefinition::concrete(DataType::Int64),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Aligned",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataFrame),
        )),
    ])
    .with_output_schema_resolver(passthrough_input_schema_resolver(PinRole::Data(
        DataRole::Input,
    )))
    .with_data_evaluator(Arc::new(|ctx| {
        let df_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
        let df_id = match &df_value {
            DataValue::DataFrame(id) => id.clone(),
            DataValue::Null => {
                return Err("TS Align: 请连接 DataFrame 输入".to_string());
            }
            _ => return Err("TS Align: 输入必须是 DataFrame".to_string()),
        };

        let name_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom(
            "time_series_name".to_string(),
        )))?;
        let time_series_name = match &name_value {
            DataValue::String(s) if !s.is_empty() => s.clone(),
            DataValue::String(_) => return Err("TS Align: 时间列名不能为空".to_string()),
            DataValue::Null => {
                return Err("TS Align: 请提供时间列名（或连接 String 常量）".to_string())
            }
            _ => return Err("TS Align: 时间列名必须是 String".to_string()),
        };

        let df = ctx.get_dataframe(&df_id)?;
        let time_col = df
            .column(&time_series_name)
            .map_err(|e| format!("TS Align: 列 '{}' 不存在: {}", time_series_name, e))?;
        let time_series = time_col.clone().take_materialized_series();

        check_no_duplicate_times(&time_series).map_err(|e| e.to_string())?;

        let interval =
            match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("interval".to_string()))) {
                Ok(DataValue::Int64(i)) => {
                    if i > 0 {
                        Some(i)
                    } else {
                        return Err("TS Align: Interval 必须为正整数".to_string());
                    }
                }
                _ => None,
            };

        let interval = interval.unwrap_or_else(|| infer_interval(&time_series).unwrap_or(1));

        let aligned = align_dataframe(&df, &time_series_name, interval)
            .map_err(|e| format!("TS Align: {}", e))?;

        let result_id = ctx.put_dataframe(aligned)?;
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Output),
            DataValue::DataFrame(result_id),
        )?;

        Ok(())
    }));
    registry.register(definition);
}

fn register_ts_diff(registry: &NodeRegistry) {
    let time_series_type = DataType::DataSeries(Box::new(DataType::one_of(vec![
        DataType::Int64,
        DataType::Date,
    ])));
    let definition = NodeDefinition::new("TS Diff", vec!["Data".to_string(), "Time Series".to_string()])
        .with_ui_style("dataframe")
        .with_localized_description(
            "对 DataSeries 做差分：y_t - y_{t-lag}。连接 Time Series 时与 Stata D. 一致，仅对相邻时间点（interval）差分，不跨 gap。",
            "Difference on DataSeries: y_t - y_{t-lag}. With Time Series, matches Stata D. on adjacent intervals only.",
        )
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "Value Series",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
            )),
            PinSlot::fixed(
                PinDefinition::data_input(
                    "Time Series",
                    DataRole::Custom("time_series".to_string()),
                    PinDataTypeDefinition::concrete(time_series_type),
                )
                .with_optional(true),
            ),
            PinSlot::fixed(PinDefinition::data_input(
                "Lag",
                DataRole::Custom("lag".to_string()),
                PinDataTypeDefinition::concrete(DataType::Int64),
            )),
            PinSlot::fixed(
                PinDefinition::data_input(
                    "Interval",
                    DataRole::Custom("interval".to_string()),
                    PinDataTypeDefinition::concrete(DataType::Int64),
                )
                .with_optional(true),
            ),
            PinSlot::fixed(PinDefinition::data_output(
                "Diff",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let series_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
            let series_id = match &series_value {
                DataValue::DataSeries(v) => v.id.clone(),
                DataValue::Null => return Err("TS Diff: 请连接 Value Series".to_string()),
                _ => return Err("TS Diff: 输入必须是 DataSeries".to_string()),
            };
            let lag = match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("lag".to_string()))) {
                Ok(DataValue::Int64(i)) if i >= 0 => i as usize,
                Ok(DataValue::Int64(_)) => return Err("TS Diff: Lag 必须为非负整数".to_string()),
                _ => 1,
            };
            let series = ctx.get_series(&series_id)?;

            let time_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("time_series".to_string())));
            let result = match time_value {
                Ok(DataValue::DataSeries(v)) if !v.id.is_empty() => {
                    let time_series = ctx.get_series(&v.id)?;
                    let interval = match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("interval".to_string()))) {
                        Ok(DataValue::Int64(i)) if i > 0 => i,
                        Ok(DataValue::Int64(_)) => return Err("TS Diff: Interval 必须为正整数".to_string()),
                        _ => 1,
                    };
                    diff::ts_diff_with_time(&time_series, &series, lag, interval)
                }
                _ => diff::ts_diff(&series, lag),
            };
            let result = result.map_err(|e| format!("TS Diff: {}", e))?;
            let result_id = ctx.put_series(result)?;
            ctx.emit_output_by_role(
                &PinRole::Data(DataRole::Output),
                DataValue::DataSeries(DataSeriesValue::with_element_type(result_id, DataType::Float64)),
            )?;
            Ok(())
        }));
    registry.register(definition);
}

fn register_ts_pct_change(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "TS Pct Change",
        vec!["Data".to_string(), "Time Series".to_string()],
    )
    .with_ui_style("dataframe")
    .with_localized_description(
        "百分比变化：(y_t - y_{t-lag}) / y_{t-lag}，前 lag 个为 null",
        "Percent change: (y_t - y_{t-lag}) / y_{t-lag}; first lag values are null",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Lag",
            DataRole::Custom("lag".to_string()),
            PinDataTypeDefinition::concrete(DataType::Int64),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Pct Change",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let series_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
        let series_id = match &series_value {
            DataValue::DataSeries(v) => v.id.clone(),
            DataValue::Null => return Err("TS Pct Change: 请连接 DataSeries".to_string()),
            _ => return Err("TS Pct Change: 输入必须是 DataSeries".to_string()),
        };
        let lag = match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("lag".to_string()))) {
            Ok(DataValue::Int64(i)) if i >= 0 => i as usize,
            Ok(DataValue::Int64(_)) => return Err("TS Pct Change: Lag 必须为非负整数".to_string()),
            _ => 1,
        };
        let series = ctx.get_series(&series_id)?;
        let result =
            pct_change::ts_pct_change(&series, lag).map_err(|e| format!("TS Pct Change: {}", e))?;
        let result_id = ctx.put_series(result)?;
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Output),
            DataValue::DataSeries(DataSeriesValue::with_element_type(
                result_id,
                DataType::Float64,
            )),
        )?;
        Ok(())
    }));
    registry.register(definition);
}

fn register_ts_rolling_mean(registry: &NodeRegistry) {
    let definition = NodeDefinition::new(
        "TS Rolling Mean",
        vec!["Data".to_string(), "Time Series".to_string()],
    )
    .with_ui_style("dataframe")
    .with_localized_description(
        "滚动均值：前 (window-1) 个为 null",
        "Rolling mean; first (window-1) values are null",
    )
    .with_pin_slots(vec![
        PinSlot::fixed(PinDefinition::data_input(
            "Series",
            DataRole::Input,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
        PinSlot::fixed(PinDefinition::data_input(
            "Window",
            DataRole::Custom("window".to_string()),
            PinDataTypeDefinition::concrete(DataType::Int64),
        )),
        PinSlot::fixed(PinDefinition::data_output(
            "Rolling Mean",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
        )),
    ])
    .with_data_evaluator(Arc::new(|ctx| {
        let series_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
        let series_id = match &series_value {
            DataValue::DataSeries(v) => v.id.clone(),
            DataValue::Null => return Err("TS Rolling Mean: 请连接 DataSeries".to_string()),
            _ => return Err("TS Rolling Mean: 输入必须是 DataSeries".to_string()),
        };
        let window =
            match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("window".to_string()))) {
                Ok(DataValue::Int64(i)) if i > 0 => i as usize,
                Ok(DataValue::Int64(_)) => {
                    return Err("TS Rolling Mean: Window 必须为正整数".to_string())
                }
                _ => return Err("TS Rolling Mean: 请提供 Window（正整数）".to_string()),
            };
        let series = ctx.get_series(&series_id)?;
        let result = rolling::rolling_mean(&series, window)
            .map_err(|e| format!("TS Rolling Mean: {}", e))?;
        let result_id = ctx.put_series(result)?;
        ctx.emit_output_by_role(
            &PinRole::Data(DataRole::Output),
            DataValue::DataSeries(DataSeriesValue::with_element_type(
                result_id,
                DataType::Float64,
            )),
        )?;
        Ok(())
    }));
    registry.register(definition);
}

fn register_ts_lag(registry: &NodeRegistry) {
    use crate::graph::value::TimeSeriesState;

    let time_series_type = DataType::DataSeries(Box::new(DataType::one_of(vec![
        DataType::Int64,
        DataType::Date,
    ])));
    let definition = NodeDefinition::new("TS Lag", vec!["Data".to_string(), "Time Series".to_string()])
        .with_ui_style("dataframe")
        .with_localized_description(
            "严格时间对齐的滞后（Stata L. 语义）。Time 为 Aligned 时跳过对齐。时间列支持 Int64 或 Date。",
            "Time-aligned lag (Stata L.). Skips realign when Time is already aligned. Int64 or Date time column.",
        )
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "Time Series",
                DataRole::Input,
                PinDataTypeDefinition::concrete(time_series_type.clone()),
            )),
            PinSlot::fixed(PinDefinition::data_input(
                "Value Series",
                DataRole::Custom("value".to_string()),
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
            )),
            PinSlot::fixed(PinDefinition::data_input(
                "Lag",
                DataRole::Custom("lag".to_string()),
                PinDataTypeDefinition::concrete(DataType::Int64),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "Time",
                DataRole::Custom("time_out".to_string()),
                PinDataTypeDefinition::concrete(time_series_type),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "Lagged",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Float64))),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let time_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
            let time_id = match &time_value {
                DataValue::DataSeries(v) => v.id.clone(),
                DataValue::Null => return Err("TS Lag: 请连接 Time Series".to_string()),
                _ => return Err("TS Lag: Time Series 必须是 DataSeries (Int64 或 Date)".to_string()),
            };
            let value_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("value".to_string())))?;
            let value_id = match &value_value {
                DataValue::DataSeries(v) => v.id.clone(),
                DataValue::Null => return Err("TS Lag: 请连接 Value Series".to_string()),
                _ => return Err("TS Lag: Value Series 必须是 DataSeries (Float64)".to_string()),
            };
            let lag = match ctx.get_input_by_role(&PinRole::Data(DataRole::Custom("lag".to_string()))) {
                Ok(DataValue::Int64(i)) if i >= 0 => i as usize,
                Ok(DataValue::Int64(_)) => return Err("TS Lag: Lag 必须为非负整数".to_string()),
                _ => 1,
            };
            let time_series = ctx.get_series(&time_id)?;
            let value_series = ctx.get_series(&value_id)?;
            let is_aligned = matches!(time_value, DataValue::DataSeries(v) if v.time_series_state.as_ref() == Some(&TimeSeriesState::Aligned));
            let (full_times, lagged_values) = if is_aligned && time_series.len() == value_series.len() {
                let aligned_vec: Vec<Option<f64>> = value_series.f64().map_err(|e| format!("TS Lag: {}", e))?.into_iter().map(|v| v).collect();
                let n = aligned_vec.len();
                let mut lagged = Vec::with_capacity(n);
                for i in 0..n {
                    lagged.push(if i < lag { None } else { aligned_vec[i - lag] });
                }
                let lagged_s = Series::from_iter(lagged)
                    .with_name(format!("{}_lag{}", value_series.name(), lag).into());
                (time_series.clone(), lagged_s)
            } else {
                check_no_duplicate_times(&time_series).map_err(|e| e.to_string())?;
                let interval = infer_interval(&time_series).unwrap_or(1);
                let (full_times, aligned_values) = align_series(&time_series, &value_series, interval)
                    .map_err(|e| format!("TS Lag: {}", e))?;
                let aligned_vec: Vec<Option<f64>> = aligned_values.f64().map_err(|e| format!("TS Lag: {}", e))?.into_iter().map(|v| v).collect();
                let n = aligned_vec.len();
                let mut lagged = Vec::with_capacity(n);
                for i in 0..n {
                    lagged.push(if i < lag { None } else { aligned_vec[i - lag] });
                }
                let lagged_s = Series::from_iter(lagged)
                    .with_name(format!("{}_lag{}", value_series.name(), lag).into());
                (full_times, lagged_s)
            };
            let time_out_id = ctx.put_series(full_times.clone())?;
            let lagged_id = ctx.put_series(lagged_values)?;
            let time_element_type = polars_dtype_to_data_type(full_times.dtype());
            ctx.emit_output_by_role(
                &PinRole::Data(DataRole::Custom("time_out".to_string())),
                DataValue::DataSeries(
                    DataSeriesValue::with_element_type(time_out_id, time_element_type)
                        .with_time_series_state(TimeSeriesState::Aligned),
                ),
            )?;
            ctx.emit_output_by_role(
                &PinRole::Data(DataRole::Output),
                DataValue::DataSeries(DataSeriesValue::with_element_type(lagged_id, DataType::Float64)),
            )?;
            Ok(())
        }));
    registry.register(definition);
}
