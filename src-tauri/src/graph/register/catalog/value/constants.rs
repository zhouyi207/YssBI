use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, PinDefinition, PinTypeDesc};
use crate::graph::register::NodeRegistry;
use crate::graph::value::{DataValue, DataType};

pub fn register(registry: &NodeRegistry) {
    register_boolean_constant(registry);
    register_int32_constant(registry);
    register_int64_constant(registry);
    register_float32_constant(registry);
    register_float64_constant(registry);
    register_string_constant(registry);
}

/// Boolean 常量节点
fn register_boolean_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("value.const.boolean", "Boolean")
        .with_category(vec!["Value".to_string(), "Constants".to_string()])
        .with_ui_style("value")
        .with_description("Boolean constant value")
        .add_pin(
            PinDefinition::data_output(
                "Value",
                DataRole::Result,
                PinTypeDesc::concrete(DataType::Boolean),
            )
            .with_default(Some(DataValue::Boolean(false))),
        );

    registry.register(definition);
}

/// Int32 常量节点
fn register_int32_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("value.const.int32", "Int32")
        .with_category(vec!["Value".to_string(), "Constants".to_string()])
        .with_ui_style("value")
        .with_description("32-bit integer constant value")
        .add_pin(
            PinDefinition::data_output(
                "Value",
                DataRole::Result,
                PinTypeDesc::concrete(DataType::Int32),
            )
            .with_default(Some(DataValue::Int32(0))),
        );

    registry.register(definition);
}

/// Int64 常量节点
fn register_int64_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("value.const.int64", "Int64")
        .with_category(vec!["Value".to_string(), "Constants".to_string()])
        .with_ui_style("value")
        .with_description("64-bit integer constant value")
        .add_pin(
            PinDefinition::data_output(
                "Value",
                DataRole::Result,
                PinTypeDesc::concrete(DataType::Int64),
            )
            .with_default(Some(DataValue::Int64(0))),
        );

    registry.register(definition);
}

/// Float32 常量节点
fn register_float32_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("value.const.float32", "Float32")
        .with_category(vec!["Value".to_string(), "Constants".to_string()])
        .with_ui_style("value")
        .with_description("32-bit floating point constant value")
        .add_pin(
            PinDefinition::data_output(
                "Value",
                DataRole::Result,
                PinTypeDesc::concrete(DataType::Float32),
            )
            .with_default(Some(DataValue::Float32(0.0))),
        );

    registry.register(definition);
}

/// Float64 常量节点
fn register_float64_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("value.const.float64", "Float64")
        .with_category(vec!["Value".to_string(), "Constants".to_string()])
        .with_ui_style("value")
        .with_description("64-bit floating point constant value")
        .add_pin(
            PinDefinition::data_output(
                "Value",
                DataRole::Result,
                PinTypeDesc::concrete(DataType::Float64),
            )
            .with_default(Some(DataValue::Float64(0.0))),
        );

    registry.register(definition);
}

/// String 常量节点
fn register_string_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("value.const.string", "String")
        .with_category(vec!["Value".to_string(), "Constants".to_string()])
        .with_ui_style("value")
        .with_description("String constant value")
        .add_pin(
            PinDefinition::data_output(
                "Value",
                DataRole::Result,
                PinTypeDesc::concrete(DataType::String),
            )
            .with_default(Some(DataValue::String(String::new()))),
        );

    registry.register(definition);
}
