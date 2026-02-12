//! Debug 节点

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::register::NodeRegistry;
use crate::graph::pin::{DataRole, ExecRole, PinDefinition, PinRole, PinDataTypeDefinition};
use crate::graph::value::DataType;
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_print(registry);
}

/// Print 节点 - 打印字符串到控制台
fn register_print(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Print")
        .with_category(vec!["Debug".to_string()])
        .with_ui_style("debug")
        .with_description("Print a string to the console")
        .with_pin_generator(Arc::new(|| {
            Ok(vec![
                PinDefinition::exec_input("In", ExecRole::ExecIn),
                PinDefinition::data_input(
                    "Message",
                    DataRole::Inputs(0),
                    PinDataTypeDefinition::concrete(DataType::String),
                ),
                PinDefinition::exec_output("Out", ExecRole::ExecOut),
            ])
        }))
        .with_flow_processor(Arc::new(|ctx| {
            let input_value = ctx
                .get_input_by_role(&PinRole::Data(DataRole::Inputs(0)))?;
            
            let message = input_value
                .as_string()
                .ok_or_else(|| "Message must be a string".to_string())?;

            ctx.log(format!("Print: {}", message));

            Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
        }));

    registry.register(definition);
}
