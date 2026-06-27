//! Event Begin 节点 - 图的开始节点

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::pin::{ExecRole, PinDefinition, PinSlot};
use crate::graph::register::NodeRegistry;
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Event Begin", vec!["Event".to_string()])
        .with_ui_style("event")
        .with_localized_description(
            "图的入口节点，执行从此处开始",
            "Entry point of the graph. Execution starts from this node.",
        )
        .with_pin_slots(vec![PinSlot::fixed(PinDefinition::exec_output(
            "Out",
            ExecRole::ExecOut,
        ))])
        .with_flow_processor(Arc::new(|ctx| {
            ctx.log("Event Begin: starting execution".to_string());
            Ok(ExecutionEffect::trigger(ExecRole::ExecOut))
        }));
    registry.register(definition);
}
