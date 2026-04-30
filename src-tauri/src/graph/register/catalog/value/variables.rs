use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::value::DataType;
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_get_variable(registry);
    register_set_variable(registry);
}

fn register_get_variable(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Get Variable", vec!["Variables".to_string()])
        .with_ui_style("variable")
        .with_description("Read the value of a variable")
        .with_pin_slots(vec![PinSlot::fixed(PinDefinition::data_output(
            "Value",
            DataRole::Output,
            PinDataTypeDefinition::concrete(DataType::Any),
        ))])
        .with_data_evaluator(Arc::new(|ctx| {
            let params = ctx.get_instance_params();
            let variable_id = params
                .variable_id()
                .ok_or("Get Variable: variable_id not set")?;
            let value = ctx.get_variable_value(variable_id)?;
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Output), value)?;
            Ok(())
        }));
    registry.register(definition);
}

fn register_set_variable(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Set Variable", vec!["Variables".to_string()])
        .with_ui_style("variable")
        .with_description("Write a value to a variable")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
            PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
            PinSlot::fixed(PinDefinition::data_input(
                "Value",
                DataRole::Input,
                PinDataTypeDefinition::concrete(DataType::Any),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "Value",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::Any),
            )),
        ])
        .with_flow_processor(Arc::new(|_ctx| {
            Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
        }))
        .with_data_evaluator(Arc::new(|ctx| {
            let params = ctx.get_instance_params();
            let variable_id = params
                .variable_id()
                .ok_or("Set Variable: variable_id not set")?;
            let value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
            ctx.set_variable_value(variable_id, value.clone())?;
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Output), value)?;
            Ok(())
        }));
    registry.register(definition);
}
