//! 控制流节点

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
        .with_flow_processor(Arc::new(|ctx| {
            let condition = ctx
                .get_input_by_role(&PinRole::Data(DataRole::Condition))?
                .as_bool()?;

            if condition {
                Ok(PinRole::Exec(ExecRole::ExecTrue))
            } else {
                Ok(PinRole::Exec(ExecRole::ExecFalse))
            }
        }));

    registry.register(definition);
}

/// Sequence 节点 - 按顺序执行多个步骤（纯控制流节点）
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
    // Sequence 节点按顺序触发所有步骤
    // 这里简化实现，只触发第一个步骤
    definition = definition.with_flow_processor(Arc::new(|_ctx| {
        Ok(PinRole::Exec(ExecRole::Steps(0)))
    }));

    // 标记为支持动态 Pin（可以添加更多步骤）
    definition = definition.dynamic();

    registry.register(definition);
}

