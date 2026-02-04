//! 控制流节点

use crate::executor::execution::ExecutionEffect;
use crate::executor::node::NodeDefinition;
use crate::executor::register::NodeRegistry;
use crate::executor::pin::{DataRole, ExecRole, PinDefinition, PinRole, PinTypeDesc};
use crate::executor::value::{DataValue, ValueType};
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_if_else(registry);
    register_sequence(registry);
}

/// If-Else 节点（混合节点：既有数据输入，又有控制流输出）
fn register_if_else(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("flow.branch", "Branch")
        .with_category(vec!["Control Flow".to_string()])
        .with_ui_style("control")
        .with_description("Branch execution based on condition")
        .add_pin(PinDefinition::exec_input("In", ExecRole::ExecIn))
        .add_pin(
            PinDefinition::data_input(
                "Condition",
                DataRole::Condition,
                PinTypeDesc::concrete(ValueType::Boolean),
            )
            .with_default(Some(DataValue::Boolean(false))),
        )
        .add_pin(PinDefinition::exec_output("True", ExecRole::ExecTrue))
        .add_pin(PinDefinition::exec_output("False", ExecRole::ExecFalse))
        // 🧱 第一层：控制流处理器
        // 返回 ExecutionEffect 而不是 PinRole
        .with_flow_processor(Arc::new(|ctx| {
            let condition = ctx
                .get_input_by_role(&PinRole::Data(DataRole::Condition))?
                .as_bool()
                .ok_or_else(|| "Condition must be a boolean value".to_string())?;

            if condition {
                Ok(ExecutionEffect::trigger(ExecRole::ExecTrue))
            } else {
                Ok(ExecutionEffect::trigger(ExecRole::ExecFalse))
            }
        }));

    registry.register(definition);
}

/// Sequence 节点 - 按顺序执行多个步骤（纯控制流节点）
/// 
/// 实现说明：
/// - 使用 ExecutionEffect::sequence() 返回所有步骤
/// - 执行器会自动按顺序执行每个步骤
/// - 每个步骤的子流程完成后，自动继续下一个步骤
/// 
/// 这是 continuation-based 执行的核心示例：
/// 1. 节点不决定"谁是下一个"
/// 2. 节点只声明"我想按顺序执行这些输出"
/// 3. 执行器负责解释这个声明并管理执行栈
fn register_sequence(registry: &NodeRegistry) {
    let mut definition = NodeDefinition::new("flow.sequence", "Sequence")
        .with_category(vec!["Control Flow".to_string()])
        .with_ui_style("control")
        .with_description("Execute steps in sequence")
        .add_pin(PinDefinition::exec_input("In", ExecRole::ExecIn));

    // 添加默认的 3 个步骤
    for i in 0..3 {
        definition = definition.add_pin(
            PinDefinition::exec_output(format!("Then {}", i + 1), ExecRole::Steps(i))
        );
    }

    // 🧱 第一层：控制流处理器
    // 返回 ExecutionEffect::sequence() 声明要按顺序执行所有步骤
    definition = definition.with_flow_processor(Arc::new(|ctx| {
        ctx.log("Sequence: scheduling all steps".to_string());
        
        // 声明要按顺序执行 3 个步骤
        // 执行器会：
        // 1. 触发 Steps(0)
        // 2. 等待 Steps(0) 的子流程完成
        // 3. 触发 Steps(1)
        // 4. 等待 Steps(1) 的子流程完成
        // 5. 触发 Steps(2)
        // 6. 等待 Steps(2) 的子流程完成
        Ok(ExecutionEffect::sequence(vec![
            ExecRole::Steps(0),
            ExecRole::Steps(1),
            ExecRole::Steps(2),
        ]))
    }));

    // 标记为支持动态 Pin（可以添加更多步骤）
    definition = definition.dynamic();

    registry.register(definition);
}