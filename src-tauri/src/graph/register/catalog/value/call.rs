//! 函数/宏调用节点

use crate::graph::node::NodeDefinition;
use crate::graph::pin::{ExecRole, PinDefinition};
use crate::graph::register::NodeRegistry;
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    register_call_function(registry);
    register_call_macro(registry);
}

/// Call Function 节点 - 调用子图函数
/// 需要 sub_graph_id 绑定具体函数
fn register_call_function(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Call Function", vec!["Functions".to_string()])
        .with_node_type("call_function")
        .with_ui_style("function")
        .with_description("Call a function subgraph")
        .with_pin_generator(Arc::new(|| {
            Ok(vec![
                PinDefinition::exec_input("In", ExecRole::ExecIn),
                PinDefinition::exec_output("Out", ExecRole::ExecOut),
            ])
        }));

    registry.register(definition);
}

/// Call Macro 节点 - 调用宏子图
/// 需要 sub_graph_id 绑定具体宏
fn register_call_macro(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Call Macro", vec!["Macros".to_string()])
        .with_node_type("call_macro")
        .with_ui_style("macro")
        .with_description("Call a macro subgraph")
        .with_pin_generator(Arc::new(|| {
            Ok(vec![
                PinDefinition::exec_input("In", ExecRole::ExecIn),
                PinDefinition::exec_output("Out", ExecRole::ExecOut),
            ])
        }));

    registry.register(definition);
}
