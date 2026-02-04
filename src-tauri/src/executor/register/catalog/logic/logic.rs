//! Logic 节点

use crate::executor::infer::{TypeVarDefinition, TypeVarId};
use crate::executor::node::NodeDefinition;
use crate::executor::register::NodeRegistry;
use crate::executor::pin::{DataRole, PinDefinition, PinRole, PinTypeDesc};
use crate::executor::value::ValueType;
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_equal(registry);
}

/// Equal 节点 - 比较两个值是否相等
/// 
/// 这是一个纯数据节点：
/// - 有两个 data input（类型相同，需要类型推断）
/// - 有一个 data output（Boolean 类型）
/// - 功能：比较两个输入值是否相等
fn register_equal(registry: &NodeRegistry) {
    // 创建类型变量（两个输入共享同一类型）
    let type_var = TypeVarId::new();

    let definition = NodeDefinition::new("logic.equal", "Equal (==)")
        .with_category(vec!["Logic".to_string(), "Comparison".to_string()])
        .with_ui_style("logic")
        .with_description("Check if two values are equal")
        // 注册类型变量（可以是任意类型）
        .add_type_var(TypeVarDefinition {
            id: type_var,
            constraints: vec![],  // 无约束，可以比较任意类型
            bound: None,
        })
        // 添加两个输入（类型相同）
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
        // 添加输出（Boolean 类型）
        .add_pin(PinDefinition::data_output(
            "Result",
            DataRole::Result,
            PinTypeDesc::concrete(ValueType::Boolean),
        ))
        // 🧱 第二层：数据求值器（纯数据节点）
        .with_data_evaluator(Arc::new(|ctx| {
            // 获取两个输入值
            let value_a = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(0)))?;
            let value_b = ctx.get_input_by_role(&PinRole::Data(DataRole::Operands(1)))?;

            // 比较两个值是否相等
            let result = value_a == value_b;

            // 输出结果
            ctx.emit_output_by_role(
                &PinRole::Data(DataRole::Result),
                crate::executor::value::DataValue::Boolean(result),
            )?;

            Ok(())
        }));

    registry.register(definition);
}
