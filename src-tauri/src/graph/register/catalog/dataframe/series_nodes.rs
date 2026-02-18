//! DataSeries 节点定义

use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition, PinRole};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataType, DataValue};
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_get_dataseries(registry);
    register_series_length(registry);
    register_series_sum(registry);
    register_series_mean(registry);
}

/// Get DataSeries 节点 - 从 DataFrame 按列名提取一列作为 DataSeries
/// 通过 instance_params.dataframe_id + column_name 绑定
fn register_get_dataseries(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Get DataSeries", vec!["Data".to_string()])
        .with_node_type("get_dataseries")
        .with_ui_style("dataframe")
        .with_description("Get a DataSeries from a DataFrame by column name")
        .with_pin_generator(Arc::new(|| {
            Ok(vec![
                PinDefinition::data_input(
                    "DataFrame",
                    DataRole::Input,
                    PinDataTypeDefinition::concrete(DataType::DataFrame),
                ),
                PinDefinition::data_output(
                    "Series",
                    DataRole::Output,
                    PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Any))),
                ),
            ])
        }))
        .with_data_evaluator(Arc::new(|ctx| {
            let params = ctx.get_instance_params();
            let column_name = params
                .column_name
                .as_deref()
                .ok_or("Get DataSeries: column_name not set")?;

            let df_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
            let df_id = match &df_value {
                DataValue::DataFrame(id) => id.clone(),
                _ => return Err("Get DataSeries: input is not a DataFrame reference".to_string()),
            };

            let df = ctx.get_dataframe(&df_id)?;
            let series = df
                .column(column_name)
                .map_err(|e| format!("Get DataSeries: {}", e))?
                .clone()
                .take_materialized_series();

            let series_id = ctx.put_series(series)?;
            ctx.emit_output_by_role(
                &PinRole::Data(DataRole::Output),
                DataValue::DataSeries(series_id),
            )?;
            Ok(())
        }));

    registry.register(definition);
}

/// Series Length 节点 - 获取 DataSeries 长度
fn register_series_length(registry: &NodeRegistry) {
    let definition =
        NodeDefinition::new("Series Length", vec!["Data".to_string(), "Series".to_string()])
            .with_node_type("series_length")
            .with_ui_style("dataframe")
            .with_description("Get the number of elements in a DataSeries")
            .with_pin_generator(Arc::new(|| {
                Ok(vec![
                    PinDefinition::data_input(
                        "Series",
                        DataRole::Input,
                        PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Any))),
                    ),
                    PinDefinition::data_output(
                        "Length",
                        DataRole::Output,
                        PinDataTypeDefinition::concrete(DataType::Int64),
                    ),
                ])
            }))
            .with_data_evaluator(Arc::new(|ctx| {
                let series_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
                let series_id = match &series_value {
                    DataValue::DataSeries(id) => id.clone(),
                    _ => return Err("Series Length: input is not a DataSeries".to_string()),
                };

                let series = ctx.get_series(&series_id)?;
                let len = series.len() as i64;

                ctx.emit_output_by_role(
                    &PinRole::Data(DataRole::Output),
                    DataValue::Int64(len),
                )?;
                Ok(())
            }));

    registry.register(definition);
}

/// Series Sum 节点 - 对数值 DataSeries 求和
fn register_series_sum(registry: &NodeRegistry) {
    let definition =
        NodeDefinition::new("Series Sum", vec!["Data".to_string(), "Series".to_string()])
            .with_node_type("series_sum")
            .with_ui_style("dataframe")
            .with_description("Calculate the sum of a numeric DataSeries")
            .with_pin_generator(Arc::new(|| {
                Ok(vec![
                    PinDefinition::data_input(
                        "Series",
                        DataRole::Input,
                        PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Any))),
                    ),
                    PinDefinition::data_output(
                        "Sum",
                        DataRole::Output,
                        PinDataTypeDefinition::concrete(DataType::Float64),
                    ),
                ])
            }))
            .with_data_evaluator(Arc::new(|ctx| {
                let series_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
                let series_id = match &series_value {
                    DataValue::DataSeries(id) => id.clone(),
                    _ => return Err("Series Sum: input is not a DataSeries".to_string()),
                };

                let series = ctx.get_series(&series_id)?;
                let sum = series
                    .sum_reduce()
                    .map_err(|e| format!("Series Sum: {}", e))?;

                let result = sum
                    .value()
                    .try_extract::<f64>()
                    .map_err(|e| format!("Series Sum: cannot extract as f64: {}", e))?;

                ctx.emit_output_by_role(
                    &PinRole::Data(DataRole::Output),
                    DataValue::Float64(result),
                )?;
                Ok(())
            }));

    registry.register(definition);
}

/// Series Mean 节点 - 对数值 DataSeries 求平均值
fn register_series_mean(registry: &NodeRegistry) {
    let definition =
        NodeDefinition::new("Series Mean", vec!["Data".to_string(), "Series".to_string()])
            .with_node_type("series_mean")
            .with_ui_style("dataframe")
            .with_description("Calculate the mean of a numeric DataSeries")
            .with_pin_generator(Arc::new(|| {
                Ok(vec![
                    PinDefinition::data_input(
                        "Series",
                        DataRole::Input,
                        PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::Any))),
                    ),
                    PinDefinition::data_output(
                        "Mean",
                        DataRole::Output,
                        PinDataTypeDefinition::concrete(DataType::Float64),
                    ),
                ])
            }))
            .with_data_evaluator(Arc::new(|ctx| {
                let series_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
                let series_id = match &series_value {
                    DataValue::DataSeries(id) => id.clone(),
                    _ => return Err("Series Mean: input is not a DataSeries".to_string()),
                };

                let series = ctx.get_series(&series_id)?;
                let mean = series.mean().ok_or("Series Mean: cannot compute mean")?;

                ctx.emit_output_by_role(
                    &PinRole::Data(DataRole::Output),
                    DataValue::Float64(mean),
                )?;
                Ok(())
            }));

    registry.register(definition);
}
