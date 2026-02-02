use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::{GenericInDataPin, GenericOutExecPin, GenericInExecPin};
use crate::executor::value::{PinTypeDesc, ValueType};

pub fn register(registry: &NodeRegistry) {
    let print_node = GenericNode::new_prototype("print", "Print");
    print_node.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    print_node.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Out"));
    
    // 🔑 只接受 String 类型，其他类型需要先转换
    print_node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "Value",
        PinTypeDesc::concrete(ValueType::String)
    ));
    
    print_node.set_flow_processor(Box::new(|ctx, node| {
        // Safely get input value
        if !node.inputs.is_empty() {
            let val = ctx.get_pin_value(&node.inputs[0].id);
            ctx.log(format!("[Print] {}", val));
        } else {
            ctx.log("[Print] No input value".to_string());
        }
        Ok("Out".into())
    }));
    
    let mut print_node = print_node;
    print_node.set_metadata(vec!["Debug".into()], "default".into(), Some("Print a value to the log".into()));
    registry.register("print".into(), Arc::new(print_node));
}
