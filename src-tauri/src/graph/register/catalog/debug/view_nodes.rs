//! View 节点：查看各种类型数据的具体内容
//!
//! 通过 ResultSourceStore 注册 source；DataFrame/DataSeries 通过 typed page API 分页拉取。

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::DataType;
use std::sync::Arc;

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
            ctx.ensure_view_source_for_input(&PinRole::Data(DataRole::Input))?;
            ctx.log("View: opened source inspector window".to_string());
            Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
        }));
    registry.register(definition);
}
