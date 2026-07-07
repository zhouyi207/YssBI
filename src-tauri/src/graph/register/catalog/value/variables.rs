use crate::execution::ExecutionEffect;
use crate::graph::node::{NodeDefinition, OutputSchemaContext};
use crate::graph::pin::{
    DataRole, ExecRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot,
};
use crate::graph::register::NodeRegistry;
use crate::graph::register::catalog::docs;
use crate::graph::value::DataType;
use crate::tabular::variable_handle_str;
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_get_variable(registry);
    register_set_variable(registry);
}

fn get_variable_dataframe_schema(
    ctx: &OutputSchemaContext,
) -> Option<crate::graph::node::DataSchema> {
    let variable_id = ctx.instance_params.variable_id()?;
    let provider = ctx.schema_provider.as_ref()?;
    provider(&variable_handle_str(variable_id))
}

fn register_get_variable(registry: &NodeRegistry) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new("Get Variable", vec!["Variables".to_string()])
            .with_ui_style("variable")
            .with_pin_slots(vec![PinSlot::fixed(PinDefinition::data_output(
                "Value",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::Any),
            ))])
            .with_output_schema_resolver(Arc::new(get_variable_dataframe_schema))
            .with_data_evaluator(Arc::new(|ctx| {
                let params = ctx.get_instance_params();
                let variable_id = params
                    .variable_id()
                    .ok_or("Get Variable: variable_id not set")?;
                let value = ctx.get_variable_value(variable_id)?;
                ctx.emit_output_by_role(&PinRole::Data(DataRole::Output), value)?;
                Ok(())
            })),
        "Get Variable",
    );
    registry.register(definition);
}

fn register_set_variable(registry: &NodeRegistry) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new("Set Variable", vec!["Variables".to_string()])
            .with_ui_style("variable")
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
            })),
        "Set Variable",
    );
    registry.register(definition);
}
