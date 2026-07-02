//! ECDF 节点：接收一个数值 DataSeries，打开 Plot 窗口绘制经验累积分布函数

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataType, DataValue};
use serde::Serialize;
use std::sync::Arc;

#[derive(Serialize)]
struct EcdfPlotData {
    data: Vec<EcdfPoint>,
    x_label: Option<String>,
    y_label: Option<String>,
}

#[derive(Serialize)]
struct EcdfPoint {
    x: f64,
    y: f64,
}

pub fn register(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("ECDF", vec!["Plot".to_string()])
        .with_ui_style("plot")
        .with_localized_description(
            "对数值 DataSeries 绘制经验累积分布函数（ECDF）",
            "Plot empirical cumulative distribution function from a numeric DataSeries",
        )
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
            PinSlot::fixed(PinDefinition::data_input(
                "Values",
                DataRole::Inputs(0),
                PinDataTypeDefinition::concrete(DataType::DataSeries(Box::new(DataType::one_of(
                    vec![
                        DataType::Float64,
                        DataType::Int64,
                        DataType::Int32,
                        DataType::Float32,
                    ],
                )))),
            )),
            PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
        ])
        .with_flow_processor(Arc::new(|ctx| {
            let value = ctx.get_input_by_role(&PinRole::Data(DataRole::Inputs(0)))?;

            let id = match &value {
                DataValue::DataSeries(v) => v.id.clone(),
                _ => return Err("ECDF: input must be a numeric DataSeries".to_string()),
            };

            let series = ctx.get_data_series(&id)?;
            let cast = series
                .cast(&polars::prelude::DataType::Float64)
                .map_err(|e| format!("ECDF: cannot cast to Float64: {}", e))?;
            let f64_chunk = cast
                .f64()
                .map_err(|e| format!("ECDF: not numeric: {}", e))?;

            let mut values: Vec<f64> = f64_chunk
                .into_iter()
                .filter_map(|v| v)
                .filter(|v| v.is_finite())
                .collect();

            if values.is_empty() {
                return Err("ECDF: no valid numeric values after filtering nulls".to_string());
            }

            values.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let n = values.len() as f64;

            let data: Vec<EcdfPoint> = values
                .into_iter()
                .enumerate()
                .map(|(i, x)| EcdfPoint {
                    x,
                    y: (i + 1) as f64 / n,
                })
                .collect();

            let x_label = series.name().to_string();

            let plot_data = EcdfPlotData {
                data,
                x_label: if x_label.is_empty() {
                    None
                } else {
                    Some(x_label)
                },
                y_label: Some("Cumulative Proportion".to_string()),
            };

            let json = serde_json::to_string(&plot_data)
                .map_err(|e| format!("ECDF: serialize failed: {}", e))?;
            ctx.open_window("ecdf".to_string(), json);

            Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
        }));
    registry.register(definition);
}
