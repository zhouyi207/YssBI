//! 函数图壳节点 - Function Entry / Function Return
//!
//! 这两个节点是系统托管的「壳节点」：随函数图自动创建、不可删除 / 复制 / 从 palette 添加。
//! 它们的数据与 exec pin 是函数签名（`function_inputs` / `function_outputs`）的投影，
//! 由 `GraphInstance::sync_function_shell_pins` 在签名变更时重建（见 `graph_instance/function_shell.rs`）。

use crate::execution::ExecutionEffect;
use crate::graph::node::{NodeDefinition, NodeGraphScope, ShellRole};
use crate::graph::register::NodeRegistry;
use crate::graph::register::catalog::docs;
use std::sync::Arc;

/// 函数入口壳节点的规范 node_type。
pub const FUNCTION_ENTRY_NODE_TYPE: &str = "Functions:Function Entry";
/// 函数返回壳节点的规范 node_type。
pub const FUNCTION_RETURN_NODE_TYPE: &str = "Functions:Function Return";

pub fn register(registry: &NodeRegistry) {
    // Entry：签名 inputs 投影为「输出」pin。入参数据值在调用时由 Call 预置；
    // 有 exec 入参时经 flow_processor 触发签名中的 exec 输出 pin 驱动函数体控制流。
    registry.register(docs::function::apply_docs(
        NodeDefinition::new("Function Entry", vec!["Functions".to_string()])
            .with_ui_style("event")
            .with_graph_scope(NodeGraphScope::Function)
            .as_shell(ShellRole::FunctionEntry)
            .with_flow_processor(Arc::new(|ctx| {
                let roles = ctx.get_exec_output_roles();
                if roles.is_empty() {
                    Ok(ExecutionEffect::done())
                } else {
                    Ok(ExecutionEffect::sequence(roles))
                }
            }))
            .with_description("Function inputs (auto-managed)"),
        "Function Entry",
    ));

    // Return：签名 outputs 投影为「输入」pin。无处理器：到达即结束，
    // 返回值由调用方在子图运行后按签名读取。
    registry.register(docs::function::apply_docs(
        NodeDefinition::new("Function Return", vec!["Functions".to_string()])
            .with_ui_style("event")
            .with_graph_scope(NodeGraphScope::Function)
            .as_shell(ShellRole::FunctionReturn)
            .with_description("Function outputs (auto-managed)"),
        "Function Return",
    ));
}
