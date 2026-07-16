//! 函数调用节点
//!
//! Call Function 的 pin 全部由目标函数签名投影而来（见 `graph_instance/function_shell.rs`
//! 的 `sync_call_function_pins_from_signature`），因此定义本身不含静态 pin。
//! - 签名含 exec 入参：Call 带 exec pin，执行走 `flow_processor`（同步运行子图后触发 exec 输出）。
//! - 签名无 exec 入参：Call 无 exec pin，作为数据节点被下游拉取，执行走 `data_evaluator`。

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::register::NodeRegistry;
use crate::graph::register::catalog::docs;
use std::sync::Arc;

/// Call Function 节点的规范 node_type。
pub const CALL_FUNCTION_NODE_TYPE: &str = "Functions:Call Function";

pub fn register(registry: &NodeRegistry) {
    register_call_function(registry);
}

fn register_call_function(registry: &NodeRegistry) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new("Call Function", vec!["Functions".to_string()])
            .with_ui_style("function")
            .with_flow_processor(Arc::new(|ctx| {
                ctx.call_subgraph()?;
                let roles = ctx.get_exec_output_roles();
                if roles.is_empty() {
                    Ok(ExecutionEffect::done())
                } else {
                    Ok(ExecutionEffect::sequence(roles))
                }
            }))
            .with_data_evaluator(Arc::new(|ctx| ctx.call_subgraph())),
        "Call Function",
    );
    registry.register(definition);
}
