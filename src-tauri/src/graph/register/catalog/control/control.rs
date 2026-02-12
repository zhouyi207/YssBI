//! 控制流节点

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, ExecRole, PinDefinition, PinRole, PinDataTypeDefinition};
use crate::graph::register::NodeRegistry;
use crate::graph::value::DataType;
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_if_else(registry);
    register_sequence(registry);
}

/// If-Else 节点（混合节点：既有数据输入，又有控制流输出）
fn register_if_else(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Branch")
        .with_category(vec!["Control Flow".to_string()])
        .with_ui_style("control")
        .with_description("Branch execution based on condition")
        .with_pin_generator(Arc::new(|| {
            Ok(vec![
                PinDefinition::exec_input("In", ExecRole::ExecIn),
                PinDefinition::data_input(
                    "Condition",
                    DataRole::Condition,
                    PinDataTypeDefinition::concrete(DataType::Boolean),
                ),
                PinDefinition::exec_output("True", ExecRole::ExecTrue),
                PinDefinition::exec_output("False", ExecRole::ExecFalse),
            ])
        }))
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
fn register_sequence(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Sequence")
        .with_category(vec!["Control Flow".to_string()])
        .with_ui_style("control")
        .with_description("Execute steps in sequence")
        .with_pin_generator(Arc::new(|| {
            let mut pins = vec![PinDefinition::exec_input("In", ExecRole::ExecIn)];
            
            // 添加默认的 3 个步骤
            for i in 0..3 {
                pins.push(PinDefinition::exec_output(
                    format!("Then {}", i + 1),
                    ExecRole::Steps(i),
                ));
            }
            
            Ok(pins)
        }))
        .with_flow_processor(Arc::new(|ctx| {
            ctx.log("Sequence: scheduling all steps".to_string());

            Ok(ExecutionEffect::sequence(vec![
                ExecRole::Steps(0),
                ExecRole::Steps(1),
                ExecRole::Steps(2),
            ]))
        }));

    registry.register(definition);
}
