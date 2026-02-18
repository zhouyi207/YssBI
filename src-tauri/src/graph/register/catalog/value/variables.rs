use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{DataRole, ExecRole, PinDefinition, PinDataTypeDefinition, PinRole};
use crate::graph::register::NodeRegistry;
use crate::graph::value::DataType;
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_get_variable(registry);
    register_set_variable(registry);
}

/// Get Variable 节点 - 读取变量值（纯数据节点，无副作用）
/// 输出类型为 Any，运行时根据 variable_id 绑定的变量类型确定
fn register_get_variable(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Get Variable", vec!["Variables".to_string()])
        .with_node_type("get_variable")
        .with_ui_style("variable")
        .with_description("Read the value of a variable")
        .with_pin_generator(Arc::new(|| {
            Ok(vec![PinDefinition::data_output(
                "Value",
                DataRole::Output,
                PinDataTypeDefinition::concrete(DataType::Any),
            )])
        }));

    registry.register(definition);
}

/// Set Variable 节点 - 写入变量值（impure 节点，有副作用，需要执行流）
///
/// Pins:
/// - ExecIn / ExecOut（控制流）
/// - Data Input "Value"（要写入的值）
/// - Data Output "Value"（pass-through，用于链式调用）
fn register_set_variable(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Set Variable", vec!["Variables".to_string()])
        .with_node_type("set_variable")
        .with_ui_style("variable")
        .with_description("Write a value to a variable")
        .with_pin_generator(Arc::new(|| {
            Ok(vec![
                PinDefinition::exec_input("In", ExecRole::ExecIn),
                PinDefinition::exec_output("Out", ExecRole::ExecOut),
                PinDefinition::data_input(
                    "Value",
                    DataRole::Input,
                    PinDataTypeDefinition::concrete(DataType::Any),
                ),
                PinDefinition::data_output(
                    "Value",
                    DataRole::Output,
                    PinDataTypeDefinition::concrete(DataType::Any),
                ),
            ])
        }))
        .with_flow_processor(Arc::new(|_ctx| {
            Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
        }))
        .with_data_evaluator(Arc::new(|ctx| {
            // pass-through：将输入值直接传递给输出
            let value = ctx.get_input_by_role(&PinRole::Data(DataRole::Input))?;
            ctx.emit_output_by_role(&PinRole::Data(DataRole::Output), value)?;
            Ok(())
        }));

    registry.register(definition);
}
