use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, PinDefinition, PinDataTypeDefinition, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::value::DataType;

pub fn register(registry: &NodeRegistry) {
    register_boolean_constant(registry);
    register_int32_constant(registry);
    register_int64_constant(registry);
    register_float32_constant(registry);
    register_float64_constant(registry);
    register_string_constant(registry);
}

fn register_boolean_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Boolean", vec!["Value".to_string(), "Constants".to_string()])
        .with_ui_style("value")
        .with_description("Boolean constant value")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_output("Value", DataRole::Result, PinDataTypeDefinition::concrete(DataType::Boolean))),
        ]);
    registry.register(definition);
}

fn register_int32_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Int32", vec!["Value".to_string(), "Constants".to_string()])
        .with_ui_style("value")
        .with_description("32-bit integer constant value")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_output("Value", DataRole::Result, PinDataTypeDefinition::concrete(DataType::Int32))),
        ]);
    registry.register(definition);
}

fn register_int64_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Int64", vec!["Value".to_string(), "Constants".to_string()])
        .with_ui_style("value")
        .with_description("64-bit integer constant value")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_output("Value", DataRole::Result, PinDataTypeDefinition::concrete(DataType::Int64))),
        ]);
    registry.register(definition);
}

fn register_float32_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Float32", vec!["Value".to_string(), "Constants".to_string()])
        .with_ui_style("value")
        .with_description("32-bit floating point constant value")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_output("Value", DataRole::Result, PinDataTypeDefinition::concrete(DataType::Float32))),
        ]);
    registry.register(definition);
}

fn register_float64_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Float64", vec!["Value".to_string(), "Constants".to_string()])
        .with_ui_style("value")
        .with_description("64-bit floating point constant value")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_output("Value", DataRole::Result, PinDataTypeDefinition::concrete(DataType::Float64))),
        ]);
    registry.register(definition);
}

fn register_string_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("String", vec!["Value".to_string(), "Constants".to_string()])
        .with_ui_style("value")
        .with_description("String constant value")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_output("Value", DataRole::Result, PinDataTypeDefinition::concrete(DataType::String))),
        ]);
    registry.register(definition);
}
