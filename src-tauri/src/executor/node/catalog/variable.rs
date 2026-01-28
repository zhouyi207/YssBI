use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::{GenericOutDataPin, GenericInDataPin, GenericOutExecPin, GenericInExecPin};
use serde_json::Value;

pub fn register(registry: &NodeRegistry) {
    // 1. Get Variable
    let get_var = GenericNode::new_prototype("get_variable", "Get Variable");
    get_var.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Value", "object"));
    get_var.set_data_processor(Box::new(|ctx, node, _pin_id| {
        if let Some(var_id) = &node.variable_id {
            ctx.get_variable(var_id).cloned().unwrap_or(Value::Null)
        } else {
            Value::Null
        }
    }));
    
    let mut get_var = get_var; 
    get_var.set_metadata(vec!["Variable".into()], "default".into(), Some("Get variable value".into()));
    registry.register("get_variable".into(), Arc::new(get_var));

    // 2. Set Variable
    let set_var = GenericNode::new_prototype("set_variable", "Set Variable");
    set_var.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    set_var.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Out"));
    set_var.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Value", "object"));
    set_var.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Value", "object"));
    
    set_var.set_flow_processor(Box::new(|ctx, node| {
        let val = ctx.get_pin_value(&node.inputs[0].id);
        if let Some(var_id) = &node.variable_id {
            ctx.set_variable(var_id, val);
        }
        Ok("Out".into())
    }));
    
    set_var.set_data_processor(Box::new(|ctx, node, _pin_id| {
        if let Some(var_id) = &node.variable_id {
            ctx.get_variable(var_id).cloned().unwrap_or(Value::Null)
        } else {
            Value::Null
        }
    }));
    
    let mut set_var = set_var;
    set_var.set_metadata(vec!["Variable".into()], "default".into(), Some("Set variable value".into()));
    registry.register("set_variable".into(), Arc::new(set_var));
}
