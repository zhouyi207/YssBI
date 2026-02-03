//! 控制流节点

use crate::executor::node::{NodeDefinition, NodeProcessor, NodeRegistry};
use crate::executor::pin::{PinDefinition, PinGroup, PinRole};
use crate::executor::value::{DataValue, PinTypeDesc, ValueType};

pub fn register(registry: &NodeRegistry) {
    register_if_else(registry);
    register_sequence(registry);
}

/// If-Else 节点
fn register_if_else(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("if_else", "If-Else")
        .with_category(vec!["Control Flow".to_string()])
        .with_ui_style("control")
        .with_description("Branch execution based on condition")
        .add_pin(PinDefinition::exec_input("In", PinRole::ExecIn))
        .add_pin(
            PinDefinition::data_input(
                "Condition",
                PinRole::Condition,
                PinTypeDesc::concrete(ValueType::Boolean),
            )
            .with_default(DataValue::Boolean(false)),
        )
        .add_pin(PinDefinition::exec_output("True", PinRole::TrueBranch))
        .add_pin(PinDefinition::exec_output("False", PinRole::FalseBranch))
        .with_processor(NodeProcessor::Flow(Box::new(|ctx| {
            let condition = ctx
                .get_input_by_role(&PinRole::Condition)
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if condition {
                Ok(PinRole::TrueBranch)
            } else {
                Ok(PinRole::FalseBranch)
            }
        })));

    registry.register(definition);
}

/// Sequence 节点 - 按顺序执行多个步骤
fn register_sequence(registry: &NodeRegistry) {
    let mut definition = NodeDefinition::new("sequence", "Sequence")
        .with_category(vec!["Control Flow".to_string()])
        .with_ui_style("control")
        .with_description("Execute steps in sequence")
        .add_pin(PinDefinition::exec_input("In", PinRole::ExecIn));

    // 添加默认的 3 个步骤
    for i in 0..3 {
        definition = definition.add_pin(
            PinDefinition::exec_output(format!("Step {}", i + 1), PinRole::Step(i))
                .with_group(PinGroup::steps()),
        );
    }

    // Sequence 节点按顺序触发所有步骤
    // 这里简化实现，只触发第一个步骤
    definition = definition.with_processor(NodeProcessor::Flow(Box::new(|_ctx| {
        Ok(PinRole::Step(0))
    })));

    registry.register(definition);
}
