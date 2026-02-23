//! 函数/宏调用节点

use crate::graph::node::NodeDefinition;
use crate::graph::pin::{ExecRole, PinDefinition, PinSlot};
use crate::graph::register::NodeRegistry;

pub fn register(registry: &NodeRegistry) {
    register_call_function(registry);
    register_call_macro(registry);
}

fn register_call_function(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Call Function", vec!["Functions".to_string()])
        .with_ui_style("function")
        .with_description("Call a function subgraph")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
            PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
        ]);
    registry.register(definition);
}

fn register_call_macro(registry: &NodeRegistry) {
    let definition = NodeDefinition::new("Call Macro", vec!["Macros".to_string()])
        .with_ui_style("macro")
        .with_description("Call a macro subgraph")
        .with_pin_slots(vec![
            PinSlot::fixed(PinDefinition::exec_input("In", ExecRole::ExecIn)),
            PinSlot::fixed(PinDefinition::exec_output("Out", ExecRole::ExecOut)),
        ]);
    registry.register(definition);
}
