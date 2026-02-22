//! 数学运算节点

use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, PinDefinition, PinDataTypeDefinition, PinRole, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::value::DataType;
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_add(registry);
    register_subtract(registry);
    register_multiply(registry);
    register_divide(registry);
}

fn register_add(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Add (+)", vec!["Math".to_string(), "Operators".to_string()])
        .with_ui_style("math")
        .with_description("Add two numbers together")
        .with_pin_slots(vec![
            PinSlot::repeatable(
                PinDefinition::data_input("", DataRole::Operands(0), PinDataTypeDefinition::concrete(DataType::Float64)),
                "",
                2,
                None,
            ),
            PinSlot::fixed(PinDefinition::data_output(
                "Result",
                DataRole::Result,
                PinDataTypeDefinition::concrete(DataType::Float64),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let a = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(0)))?;
            let b = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(1)))?;
            let result = (a + b)?;
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Result), result)?;
            Ok(())
        }));

    registry.register(definition);
}

fn register_subtract(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Subtract (-)", vec!["Math".to_string(), "Operators".to_string()])
        .with_ui_style("math")
        .with_description("Subtract B from A")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "A", DataRole::Operands(0), PinDataTypeDefinition::concrete(DataType::Float64),
            )),
            PinSlot::fixed(PinDefinition::data_input(
                "B", DataRole::Operands(1), PinDataTypeDefinition::concrete(DataType::Float64),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "Result", DataRole::Result, PinDataTypeDefinition::concrete(DataType::Float64),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let a = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(0)))?;
            let b = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(1)))?;
            let result = (a - b)?;
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Result), result)?;
            Ok(())
        }));

    registry.register(definition);
}

fn register_multiply(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Multiply (*)", vec!["Math".to_string(), "Operators".to_string()])
        .with_ui_style("math")
        .with_description("Multiply two numbers")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "A", DataRole::Operands(0), PinDataTypeDefinition::concrete(DataType::Float64),
            )),
            PinSlot::fixed(PinDefinition::data_input(
                "B", DataRole::Operands(1), PinDataTypeDefinition::concrete(DataType::Float64),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "Result", DataRole::Result, PinDataTypeDefinition::concrete(DataType::Float64),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let a = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(0)))?;
            let b = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(1)))?;
            let result = (a * b)?;
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Result), result)?;
            Ok(())
        }));

    registry.register(definition);
}

fn register_divide(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Divide (/)", vec!["Math".to_string(), "Operators".to_string()])
        .with_ui_style("math")
        .with_description("Divide A by B")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::data_input(
                "A", DataRole::Operands(0), PinDataTypeDefinition::concrete(DataType::Float64),
            )),
            PinSlot::fixed(PinDefinition::data_input(
                "B", DataRole::Operands(1), PinDataTypeDefinition::concrete(DataType::Float64),
            )),
            PinSlot::fixed(PinDefinition::data_output(
                "Result", DataRole::Result, PinDataTypeDefinition::concrete(DataType::Float64),
            )),
        ])
        .with_data_evaluator(Arc::new(|ctx| {
            let a = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(0)))?;
            let b = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(1)))?;
            let result = (a / b)?;
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Result), result)?;
            Ok(())
        }));

    registry.register(definition);
}
