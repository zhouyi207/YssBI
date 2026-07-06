//! 控制流节点

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
    register_if_else(registry);
    register_sequence(registry);
}

fn register_if_else(registry: &NodeRegistry) {
    let definition = docs::control::apply_docs(
        NodeDefinition::new("Branch", vec!["Control Flow".to_string()])
            .with_ui_style("control")
            .with_pin_slots(vec![
                PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
                PinSlot::fixed(
                    PinDefinition::data_input(
                        "Condition",
                        DataRole::Condition,
                        PinDataTypeDefinition::concrete(DataType::Boolean),
                    )
                    .with_optional(true),
                ),
                PinSlot::fixed(PinDefinition::exec_output("True", ExecRole::ExecTrue)),
                PinSlot::fixed(PinDefinition::exec_output("False", ExecRole::ExecFalse)),
            ])
            .with_flow_processor(Arc::new(|ctx| {
                let condition = ctx
                    .get_input_by_role(&PinRole::Data(DataRole::Condition))?
                    .as_bool()
                    .ok_or_else(|| "Condition must be a boolean value".to_string())?;
                if condition {
                    Ok(ExecutionEffect::trigger(ExecRole::ExecTrue))
                } else {
                    Ok(ExecutionEffect::trigger(ExecRole::ExecFalse))
                }
            })),
        "Branch",
    );
    registry.register(definition);
}

fn register_sequence(registry: &NodeRegistry) {
    let definition = docs::control::apply_docs(
        NodeDefinition::new("Sequence", vec!["Control Flow".to_string()])
            .with_ui_style("control")
            .with_pin_slots(vec![
                PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
                PinSlot::repeatable(
                    PinDefinition::exec_output("", ExecRole::Steps(0)),
                    "Then",
                    3,
                    None,
                ),
            ])
            .with_flow_processor(Arc::new(|ctx| {
                let roles = ctx.get_exec_step_outputs();
                ctx.log(format!("Sequence: scheduling {} steps", roles.len()));
                Ok(ExecutionEffect::sequence(roles))
            })),
        "Sequence",
    );
    registry.register(definition);
}
