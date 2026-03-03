use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, PinDefinition, PinDataTypeDefinition, PinRole, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::value::DataType;
use std::sync::Arc;

/// 常数节点的 data_evaluator：将输出 pin 的 user_value 或默认值写入运行时，并触发 NodeStart/NodeComplete 以更新前端执行状态
fn constant_evaluator(ctx: &mut dyn crate::execution::NodeExecutionContextTrait) -> Result<(), String> {
    let value = ctx.get_resolved_value_by_role(&PinRole::Data(DataRole::Result))?;
    ctx.emit_output_by_role(&PinRole::Data(DataRole::Result), value)?;
    Ok(())
}

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
        ])
        .with_data_evaluator(Arc::new(constant_evaluator));
    registry.register(definition);
}

fn register_int32_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Int32", vec!["Value".to_string(), "Constants".to_string()])
        .with_ui_style("value")
        .with_description("32-bit integer constant value")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_output("Value", DataRole::Result, PinDataTypeDefinition::concrete(DataType::Int32))),
        ])
        .with_data_evaluator(Arc::new(constant_evaluator));
    registry.register(definition);
}

fn register_int64_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Int64", vec!["Value".to_string(), "Constants".to_string()])
        .with_ui_style("value")
        .with_description("64-bit integer constant value")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_output("Value", DataRole::Result, PinDataTypeDefinition::concrete(DataType::Int64))),
        ])
        .with_data_evaluator(Arc::new(constant_evaluator));
    registry.register(definition);
}

fn register_float32_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Float32", vec!["Value".to_string(), "Constants".to_string()])
        .with_ui_style("value")
        .with_description("32-bit floating point constant value")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_output("Value", DataRole::Result, PinDataTypeDefinition::concrete(DataType::Float32))),
        ])
        .with_data_evaluator(Arc::new(constant_evaluator));
    registry.register(definition);
}

fn register_float64_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Float64", vec!["Value".to_string(), "Constants".to_string()])
        .with_ui_style("value")
        .with_description("64-bit floating point constant value")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_output("Value", DataRole::Result, PinDataTypeDefinition::concrete(DataType::Float64))),
        ])
        .with_data_evaluator(Arc::new(constant_evaluator));
    registry.register(definition);
}

fn register_string_constant(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("String", vec!["Value".to_string(), "Constants".to_string()])
        .with_ui_style("value")
        .with_description("String constant value")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_output("Value", DataRole::Result, PinDataTypeDefinition::concrete(DataType::String))),
        ])
        .with_data_evaluator(Arc::new(constant_evaluator));
    registry.register(definition);
}
