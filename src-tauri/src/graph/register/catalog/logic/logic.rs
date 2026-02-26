//! Logic 节点

use crate::graph::node::NodeDefinition;
use crate::graph::register::NodeRegistry;
use crate::graph::pin::{DataRole, PinDefinition, PinRole, PinDataTypeDefinition, PinSlot};
use crate::graph::value::{DataType, DataValue};
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_equal(registry);
    register_not_equal(registry);
    register_and(registry);
    register_or(registry);
    register_not(registry);
}

fn register_equal(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Equal (==)", vec!["Logic".to_string(), "Comparison".to_string()])
        .with_ui_style("logic")
        .with_description("Check if two values are equal")
        .with_pin_slots(vec![
            PinSlot::fixed(
                PinDefinition::data_input("A", DataRole::Operands(0), PinDataTypeDefinition::concrete(DataType::Float64))
                    .with_optional(true),
            ),
            PinSlot::fixed(
                PinDefinition::data_input("B", DataRole::Operands(1), PinDataTypeDefinition::concrete(DataType::Float64))
                    .with_optional(true),
            ),
            PinSlot::fixed(PinDefinition::data_output("Result", DataRole::Result, PinDataTypeDefinition::concrete(DataType::Boolean))),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let value_a = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(0)))?;
            let value_b = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(1)))?;
            let result = value_a == value_b;
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Result), DataValue::Boolean(result))?;
            Ok(())
        }));
    registry.register(definition);
}

fn register_not_equal(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Not Equal (!=)", vec!["Logic".to_string(), "Comparison".to_string()])
        .with_ui_style("logic")
        .with_description("Check if two values are not equal")
        .with_pin_slots(vec![
            PinSlot::fixed(
                PinDefinition::data_input("A", DataRole::Operands(0), PinDataTypeDefinition::concrete(DataType::Float64))
                    .with_optional(true),
            ),
            PinSlot::fixed(
                PinDefinition::data_input("B", DataRole::Operands(1), PinDataTypeDefinition::concrete(DataType::Float64))
                    .with_optional(true),
            ),
            PinSlot::fixed(PinDefinition::data_output("Result", DataRole::Result, PinDataTypeDefinition::concrete(DataType::Boolean))),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let value_a = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(0)))?;
            let value_b = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(1)))?;
            let result = value_a != value_b;
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Result), DataValue::Boolean(result))?;
            Ok(())
        }));
    registry.register(definition);
}

fn register_and(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("And (&&)", vec!["Logic".to_string(), "Boolean".to_string()])
        .with_ui_style("logic")
        .with_description("Logical AND operation")
        .with_pin_slots(vec![
            PinSlot::fixed(
                PinDefinition::data_input("A", DataRole::Operands(0), PinDataTypeDefinition::concrete(DataType::Boolean))
                    .with_optional(true),
            ),
            PinSlot::fixed(
                PinDefinition::data_input("B", DataRole::Operands(1), PinDataTypeDefinition::concrete(DataType::Boolean))
                    .with_optional(true),
            ),
            PinSlot::fixed(PinDefinition::data_output("Result", DataRole::Result, PinDataTypeDefinition::concrete(DataType::Boolean))),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let a = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(0)))?.as_bool().ok_or("A must be a boolean")?;
            let b = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(1)))?.as_bool().ok_or("B must be a boolean")?;
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Result), DataValue::Boolean(a && b))?;
            Ok(())
        }));
    registry.register(definition);
}

fn register_or(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Or (||)", vec!["Logic".to_string(), "Boolean".to_string()])
        .with_ui_style("logic")
        .with_description("Logical OR operation")
        .with_pin_slots(vec![
            PinSlot::fixed(
                PinDefinition::data_input("A", DataRole::Operands(0), PinDataTypeDefinition::concrete(DataType::Boolean))
                    .with_optional(true),
            ),
            PinSlot::fixed(
                PinDefinition::data_input("B", DataRole::Operands(1), PinDataTypeDefinition::concrete(DataType::Boolean))
                    .with_optional(true),
            ),
            PinSlot::fixed(PinDefinition::data_output("Result", DataRole::Result, PinDataTypeDefinition::concrete(DataType::Boolean))),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let a = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(0)))?.as_bool().ok_or("A must be a boolean")?;
            let b = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(1)))?.as_bool().ok_or("B must be a boolean")?;
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Result), DataValue::Boolean(a || b))?;
            Ok(())
        }));
    registry.register(definition);
}

fn register_not(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Not (!)", vec!["Logic".to_string(), "Boolean".to_string()])
        .with_ui_style("logic")
        .with_description("Logical NOT operation")
        .with_pin_slots(vec![
            PinSlot::fixed(
                PinDefinition::data_input("A", DataRole::Operands(0), PinDataTypeDefinition::concrete(DataType::Boolean))
                    .with_optional(true),
            ),
            PinSlot::fixed(PinDefinition::data_output("Result", DataRole::Result, PinDataTypeDefinition::concrete(DataType::Boolean))),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let a = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(0)))?.as_bool().ok_or("A must be a boolean")?;
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Result), DataValue::Boolean(!a))?;
            Ok(())
        }));
    registry.register(definition);
}
