use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::GenericExecPin;

pub fn register(registry: &NodeRegistry) {
    // 1. Call Function
    let call_f = GenericNode::new_prototype("call_function", "Call Function");
    call_f.add_exec_pin(GenericExecPin::new(uuid::Uuid::nil(), "In"));
    call_f.add_exec_pin(GenericExecPin::new(uuid::Uuid::nil(), "Out"));
    
    call_f.set_flow_processor(Box::new(|ctx, node| {
        if let Some(sub_graph_id) = &node.sub_graph_id {
            // 找到函数入口节点
            let entry_node_id = ctx.find_node_by(&|n| {
                n.node_type == "function_entry" && n.sub_graph_id.as_ref() == Some(sub_graph_id)
            });

            if let Some(entry_id) = entry_node_id {
                ctx.push_call_stack(node.id.clone());
                // 执行子流程。注意：function_entry 节点通常没有输入输出执行 Pin，
                // 或者说它就是起始点。我们这里调用 Context 执行它。
                ctx.run_flow(&entry_id, "Then")?;
                ctx.pop_call_stack();
            }
        }
        Ok("Out".into())
    }));
    
    let mut call_f = call_f;
    call_f.set_metadata(vec!["Function".into()], "default".into(), Some("Call a defined function".into()));
    registry.register("call_function".into(), Arc::new(call_f));

    // 2. Call Macro
    let call_m = GenericNode::new_prototype("call_macro", "Call Macro");
    call_m.add_exec_pin(GenericExecPin::new(uuid::Uuid::nil(), "In"));
    call_m.add_exec_pin(GenericExecPin::new(uuid::Uuid::nil(), "Out"));
    
    call_m.set_flow_processor(Box::new(|ctx, node| {
        if let Some(sub_graph_id) = &node.sub_graph_id {
            let entry_node_id = ctx.find_node_by(&|n| {
                n.node_type == "macro_inputs" && n.sub_graph_id.as_ref() == Some(sub_graph_id)
            });

            if let Some(entry_id) = entry_node_id {
                ctx.push_call_stack(node.id.clone());
                ctx.run_flow(&entry_id, "In")?;
                ctx.pop_call_stack();
            }
        }
        Ok("Out".into())
    }));
    
    let mut call_m = call_m;
    call_m.set_metadata(vec!["Macro".into()], "default".into(), Some("Call a defined macro".into()));
    registry.register("call_macro".into(), Arc::new(call_m));
}
