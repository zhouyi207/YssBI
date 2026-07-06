//! 函数调用节点

use crate::graph::node::NodeDefinition;
use crate::graph::register::catalog::docs;
use crate::graph::pin::{ExecRole, PinDefinition, PinSlot};
use crate::graph::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    register_call_function(registry);
}

fn register_call_function(registry: &NodeRegistry) {
    let definition = docs::value::apply_docs(
        NodeDefinition::new("Call Function", vec!["Functions".to_string()])
            .with_ui_style("function")
            .with_pin_slots(vec![
                PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
                PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
            ]),
        "Call Function",
    );
    registry.register(definition);
}
