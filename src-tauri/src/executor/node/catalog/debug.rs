use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::{GenericInDataPin, GenericExecPin};

pub fn register(registry: &NodeRegistry) {
    let print_node = GenericNode::new_prototype("print", "Print");
    print_node.add_exec_pin(GenericExecPin::new(uuid::Uuid::nil(), "In"));
    print_node.add_exec_pin(GenericExecPin::new(uuid::Uuid::nil(), "Out"));
    print_node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Value", "string"));
    
    print_node.set_flow_processor(Box::new(|ctx, node| {
        let val = ctx.get_pin_value(&node.inputs[0].id);
        ctx.log(format!("[Print] {}", val));
        Ok("Out".into())
    }));
    
    let mut print_node = print_node;
    print_node.set_metadata(vec!["Debug".into()], "default".into(), Some("Print a value to the log".into()));
    registry.register("print".into(), Arc::new(print_node));
}
