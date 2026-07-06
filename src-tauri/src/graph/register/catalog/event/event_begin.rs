//! Event Begin 节点 - 图的开始节点

use crate::execution::ExecutionEffect;
use crate::graph::node::NodeDefinition;
use crate::graph::register::catalog::docs;
use crate::graph::pin::{ExecRole, PinDefinition, PinSlot};
use crate::graph::register::NodeRegistry;
use std::sync::Arc;

pub fn register(registry: &NodeRegistry) {
    let definition = docs::event::apply_docs(
        NodeDefinition::new("Event Begin", vec!["Event".to_string()])
            .with_ui_style("event")
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
