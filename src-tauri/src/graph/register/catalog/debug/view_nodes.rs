//! View 节点：查看各种类型数据的具体内容
//!
//! 通过 ResultSourceStore 注册 source；DataFrame/DataSeries 通过 typed page API 分页拉取。

use crate::execution::{
    ExecutionEffect, build_dataframe_source, build_json_source_from_data_value,
    build_series_source, build_struct_source,
};
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataType, DataValue};
use std::sync::Arc;

fn window_source_id() -> String {
    format!("window_{}", uuid::Uuid::new_v4().simple())
}

pub fn register(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("View", vec!["Debug".to_string(), "Data".to_string()])
        .with_ui_style("debug")
        .with_localized_description("在窗口中查看数据（DataFrame、DataSeries 或标量），执行后数据仍保留", "View data in a window (DataFrame, DataSeries, or scalar). Data persists after execution.")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
            PinSlot::fixed(
                PinDefinition::data_input(
                    "Data",
                    DataRole::Input,
                    PinDataTypeDefinition::concrete(DataType::Any),
                )
                .with_optional(true),
            ),
            PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
        ])
        .with_flow_processor(Arc::new(|ctx| {
            let input_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;

            if let Some(source_id) = ctx.get_input_source_id_by_role(&PinRole::Data(DataRole::Input)) {
                ctx.open_existing_source_window("data_view".to_string(), source_id);
                ctx.log("View: opened existing runtime source".to_string());
                return Ok(ExecutionEffect::trigger(ExecRole::ExecOut));
            }

            match &input_value {
                DataValue::Null => {
                    let record = build_json_source_from_data_value(
                        window_source_id(),
                        "View: (null)",
                        &DataValue::Null,
                        None,
                    );
                    ctx.open_result_source_window("data_view".to_string(), record);
                }
                DataValue::DataFrame(id) => {
                    let df = ctx.get_dataframe(id)?;
                    let record =
                        build_dataframe_source(window_source_id(), "View: DataFrame", df, None);
                    ctx.open_result_source_window(
                        "data_view".to_string(),
                        record,
                    );
                }
                DataValue::DataSeries(v) => {
                    let series = ctx.get_series(&v.id)?;
                    let title = {
                        let name = series.name().to_string();
                        format!("View: {}", if name.is_empty() { "Series" } else { &name })
                    };
                    let record = build_series_source(window_source_id(), title, series, None);
                    ctx.open_result_source_window("data_view".to_string(), record);
                }
                DataValue::Struct { type_key, handle_id } => {
                    let handle = ctx.get_handle(handle_id).ok();
                    let record =
                        build_struct_source(window_source_id(), type_key, handle_id, handle, None)?;
                    ctx.open_result_source_window("data_view".to_string(), record);
                }
                scalar => {
                    let record = build_json_source_from_data_value(
                        window_source_id(),
                        "View: Scalar",
                        scalar,
                        None,
                    );
                    ctx.open_result_source_window("data_view".to_string(), record);
                }
            }

            ctx.log("View: opened data view window".to_string());
            Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
        }));
    registry.register(definition);
}
