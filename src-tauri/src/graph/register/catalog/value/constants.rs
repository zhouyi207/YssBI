use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, PinDataTypeDefinition, PinDefinition, PinRole, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::register::catalog::docs;
use crate::graph::value::DataType;
use std::sync::Arc;

fn constant_evaluator(
    ctx: &mut dyn crate::execution::NodeExecutionContextTrait,
) -> Result<(), String> {
    let value = ctx.get_resolved_value_by_role(&PinRole::Data(DataRole::Result))?;
    ctx.emit_output_by_role(&PinRole::Data(DataRole::Result), value)?;
    Ok(())
}

fn register_constant(registry: &NodeRegistry, name: &str, data_type: DataType) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new(name, vec!["Value".to_string(), "Constants".to_string()])
            .with_ui_style("value")
            .with_pin_slots(vec![PinSlot::fixed(PinDefinition::data_output(
                "Value",
                DataRole::Result,
                PinDataTypeDefinition::concrete(data_type),
            ))])
            .with_data_evaluator(Arc::new(constant_evaluator)),
        name,
    );
    registry.register(definition);
}

pub fn register(registry: &NodeRegistry) {
    register_constant(registry, "Boolean", DataType::Boolean);
    register_constant(registry, "Int64", DataType::Int64);
    register_constant(registry, "Float64", DataType::Float64);
    register_constant(registry, "String", DataType::String);
}
