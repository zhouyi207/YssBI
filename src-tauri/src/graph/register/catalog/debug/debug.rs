//! Debug 节点

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::register::catalog::docs;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::DataType;
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_print(registry);
}

fn register_print(registry: &NodeRegistry) {
    let definition = docs::debug::apply_docs(
        NodeDefinition::new("Print", vec!["Debug".to_string()])
            .with_ui_style("debug")
            .with_pin_slots(vec![
                PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
                PinSlot::fixed(
                    PinDefinition::data_input(
                        "Message",
                        DataRole::Inputs(0),
                        PinDataTypeDefinition::concrete(DataType::String),
                    )
                    .with_optional(true),
                ),
                PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
            ])
            .with_flow_processor(Arc::new(|ctx| {
                let input_value = ctx.get_input_by_role(&PinRole::Data(DataRole::Inputs(0)))?;
                let message = input_value
                    .as_string()
                    .ok_or_else(|| "Message must be a string".to_string())?;
                ctx.log(format!("Print: {}", message));
                Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
            })),
        "Print",
    );
    registry.register(definition);
}
