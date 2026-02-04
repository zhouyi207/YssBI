//! 数学运算节点

use crate::executor::register::NodeRegistry;
use crate::executor::infer::{TypeConstraint, TypeVarDefinition, TypeVarId};
use crate::executor::node::NodeDefinition;
use crate::executor::pin::{DataRole, PinDefinition, PinRole, PinTypeDesc};
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_add(registry);
}

/// Add 节点 - 支持任意数量的操作数
fn register_add(registry: &NodeRegistry) {
    // 创建类型变量（所有操作数和结果共享同一类型）
    let type_var = TypeVarId::new();

    let definition = NodeDefinition::new("math.add", "Add (+)")
        .with_category(vec!["Math".to_string(), "Operators".to_string()])
        .with_ui_style("math")
        .with_description("Add numbers together")
        // 注册类型变量及其约束
        .add_type_var(TypeVarDefinition {
            id: type_var,
            constraints: vec![TypeConstraint::Numeric],
            bound: None,
        })
        // 添加两个默认操作数（不设置默认值，因为类型未知）
        .add_pin(PinDefinition::data_input(
            "A",
            DataRole::Operands(0),
            PinTypeDesc::type_var(type_var),
        ))
        .add_pin(PinDefinition::data_input(
            "B",
            DataRole::Operands(1),
            PinTypeDesc::type_var(type_var),
        ))
        .add_pin(PinDefinition::data_output(
            "Result",
            DataRole::Result,
            PinTypeDesc::type_var(type_var),
        ))
        // 🧱 第二层：数据求值器（纯数据节点）
        .with_data_evaluator(Arc::new(|ctx| {
            // 获取所有 Operands 家族的输入值
            let operands = ctx.get_inputs_by_family(&PinRole::Data(DataRole::Operands(0)))?;

            if operands.is_empty() {
                return Err("No operands found".to_string());
            }

            // 对所有操作数进行累加
            let mut result = operands[0].clone();
            for operand in operands.iter().skip(1) {
                result = (result + operand.clone())?;
            }

            // 输出结果
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Result), result)?;

            Ok(())
        }));

    registry.register(definition);
}

