//! 数学运算节点

use crate::executor::node::{NodeDefinition, NodeProcessor, NodeRegistry};
use crate::executor::pin::{PinDefinition, PinGroup, PinRole};
use crate::executor::value::{DataValue, PinTypeDesc, TypeConstraint, TypeVarId};

pub fn register(registry: &NodeRegistry) {
    register_add(registry);
    register_subtract(registry);
    register_multiply(registry);
    register_divide(registry);
}

/// Add 节点 - 支持任意数量的操作数
fn register_add(registry: &NodeRegistry) {
    // 创建类型变量（所有操作数和结果共享同一类型）
    let type_var = TypeVarId::new();

    let mut definition = NodeDefinition::new("add", "Add (+)")
        .with_category(vec!["Math".to_string(), "Operators".to_string()])
        .with_ui_style("math")
        .with_description("Add numbers together");

    // 添加两个默认操作数
    definition = definition
        .add_pin(
            PinDefinition::data_input(
                "A",
                PinRole::Operand,
                PinTypeDesc::type_var_with_constraints(type_var, vec![TypeConstraint::Numeric]),
            )
            .with_group(PinGroup::operands())
            .with_default(DataValue::Float64(0.0)),
        )
        .add_pin(
            PinDefinition::data_input(
                "B",
                PinRole::Operand,
                PinTypeDesc::type_var_with_constraints(type_var, vec![TypeConstraint::Numeric]),
            )
            .with_group(PinGroup::operands())
            .with_default(DataValue::Float64(0.0)),
        )
        .add_pin(
            PinDefinition::data_output("Result", PinRole::Result, PinTypeDesc::type_var(type_var)),
        );

    // 设置处理器（通过 Role 访问输入）
    definition = definition.with_processor(NodeProcessor::Data(Box::new(|ctx| {
        // 获取所有操作数（按 Role）
        let operands = ctx.get_inputs_by_role(&PinRole::Operand);

        // 计算总和
        let sum = operands
            .iter()
            .filter_map(|v| v.as_f64())
            .sum::<f64>();

        // 输出结果（按 Role）
        ctx.emit_output_by_role(PinRole::Result, DataValue::Float64(sum));

        Ok(())
    })));

    registry.register(definition);
}

/// Subtract 节点
fn register_subtract(registry: &NodeRegistry) {
    let type_var = TypeVarId::new();

    let definition = NodeDefinition::new("subtract", "Subtract (-)")
        .with_category(vec!["Math".to_string(), "Operators".to_string()])
        .with_ui_style("math")
        .add_pin(
            PinDefinition::data_input(
                "A",
                PinRole::PrimaryInput,
                PinTypeDesc::type_var_with_constraints(type_var, vec![TypeConstraint::Numeric]),
            )
            .with_default(DataValue::Float64(0.0)),
        )
        .add_pin(
            PinDefinition::data_input(
                "B",
                PinRole::Custom("subtrahend".to_string()),
                PinTypeDesc::type_var_with_constraints(type_var, vec![TypeConstraint::Numeric]),
            )
            .with_default(DataValue::Float64(0.0)),
        )
        .add_pin(
            PinDefinition::data_output("Result", PinRole::Result, PinTypeDesc::type_var(type_var)),
        )
        .with_processor(NodeProcessor::Data(Box::new(|ctx| {
            let a = ctx
                .get_input_by_role(&PinRole::PrimaryInput)
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let b = ctx
                .get_input_by_role(&PinRole::Custom("subtrahend".to_string()))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            ctx.emit_output_by_role(PinRole::Result, DataValue::Float64(a - b));
            Ok(())
        })));

    registry.register(definition);
}

/// Multiply 节点
fn register_multiply(registry: &NodeRegistry) {
    let type_var = TypeVarId::new();

    let definition = NodeDefinition::new("multiply", "Multiply (*)")
        .with_category(vec!["Math".to_string(), "Operators".to_string()])
        .with_ui_style("math")
        .add_pin(
            PinDefinition::data_input(
                "A",
                PinRole::Operand,
                PinTypeDesc::type_var_with_constraints(type_var, vec![TypeConstraint::Numeric]),
            )
            .with_group(PinGroup::operands())
            .with_default(DataValue::Float64(1.0)),
        )
        .add_pin(
            PinDefinition::data_input(
                "B",
                PinRole::Operand,
                PinTypeDesc::type_var_with_constraints(type_var, vec![TypeConstraint::Numeric]),
            )
            .with_group(PinGroup::operands())
            .with_default(DataValue::Float64(1.0)),
        )
        .add_pin(
            PinDefinition::data_output("Result", PinRole::Result, PinTypeDesc::type_var(type_var)),
        )
        .with_processor(NodeProcessor::Data(Box::new(|ctx| {
            let operands = ctx.get_inputs_by_role(&PinRole::Operand);

            let product = operands
                .iter()
                .filter_map(|v| v.as_f64())
                .product::<f64>();

            ctx.emit_output_by_role(PinRole::Result, DataValue::Float64(product));
            Ok(())
        })));

    registry.register(definition);
}

/// Divide 节点
fn register_divide(registry: &NodeRegistry) {
    let type_var = TypeVarId::new();

    let definition = NodeDefinition::new("divide", "Divide (/)")
        .with_category(vec!["Math".to_string(), "Operators".to_string()])
        .with_ui_style("math")
        .add_pin(
            PinDefinition::data_input(
                "A",
                PinRole::Custom("dividend".to_string()),
                PinTypeDesc::type_var_with_constraints(type_var, vec![TypeConstraint::Numeric]),
            )
            .with_default(DataValue::Float64(0.0)),
        )
        .add_pin(
            PinDefinition::data_input(
                "B",
                PinRole::Custom("divisor".to_string()),
                PinTypeDesc::type_var_with_constraints(type_var, vec![TypeConstraint::Numeric]),
            )
            .with_default(DataValue::Float64(1.0)),
        )
        .add_pin(
            PinDefinition::data_output("Result", PinRole::Result, PinTypeDesc::type_var(type_var)),
        )
        .with_processor(NodeProcessor::Data(Box::new(|ctx| {
            let a = ctx
                .get_input_by_role(&PinRole::Custom("dividend".to_string()))
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);

            let b = ctx
                .get_input_by_role(&PinRole::Custom("divisor".to_string()))
                .and_then(|v| v.as_f64())
                .unwrap_or(1.0);

            if b == 0.0 {
                return Err("Division by zero".to_string());
            }

            ctx.emit_output_by_role(PinRole::Result, DataValue::Float64(a / b));
            Ok(())
        })));

    registry.register(definition);
}
