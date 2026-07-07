//! Event Begin 节点 - 图的开始节点

use crate::execution::ExecutionEffect;
use crate::graph::node::{NodeDefinition, NodeGraphScope, ShellRole};
use crate::graph::pin::{ExecRole, PinDefinition, PinSlot};
use crate::graph::register::NodeRegistry;
use crate::graph::register::catalog::docs;
use std::sync::Arc;

/// 事件图入口壳节点的规范 node_type。
pub const EVENT_BEGIN_NODE_TYPE: &str = "Event:Event Begin";

pub fn register(registry: &NodeRegistry) {
    let definition = docs::event::apply_docs(
        NodeDefinition::new("Event Begin", vec!["Event".to_string()])
            .with_ui_style("event")
            .with_graph_scope(NodeGraphScope::Event)
            .as_shell(ShellRole::EventBegin)
            .with_pin_slots(vec![PinSlot::fixed(PinDefinition::exec_output(
                "Out",
                ExecRole::ExecOut,
            ))])
            .with_flow_processor(Arc::new(|ctx| {
                ctx.log("Event Begin: starting execution".to_string());
                Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
            })),
        "Event Begin",
    );
    registry.register(definition);
}
