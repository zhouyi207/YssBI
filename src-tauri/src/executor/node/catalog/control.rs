use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::{GenericInDataPin, GenericOutExecPin, GenericInExecPin};

pub fn register(registry: &NodeRegistry) {
    // 1. IfElse Node
    let if_else = GenericNode::new_prototype("if_else", "If Else");
    if_else.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    if_else.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "Condition", "bool"));
    if_else.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "True"));
    if_else.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "False"));
    
    if_else.set_flow_processor(Box::new(|ctx, node| {
        let cond = ctx.get_pin_value(&node.inputs[0].id).as_bool().unwrap_or(false);
        if cond {
            Ok("True".into())
        } else {
            Ok("False".into())
        }
    }));
    
    let mut if_else = if_else;
    if_else.set_metadata(vec!["Control".into()], "default".into(), Some("Branch flow based on condition".into()));
    registry.register("if_else".into(), Arc::new(if_else));

    // 2. Sequence Node
    let seq = GenericNode::new_prototype("sequence", "Sequence");
    seq.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    seq.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 0"));
    seq.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then 1"));
    
    seq.set_flow_processor(Box::new(|ctx, node| {
        // 执行第一个分支，然后由 ExecutionContext 自动处理后续？
        // 其实 Sequence 需要特殊的执行逻辑，或者它实际上只是触发多个输出。
        // 目前的 ExecutionContext 一次只能返回一个输出针脚。
        // 如果要支持 Sequence，ExecutionContext 的 run_flow 应该能返回多个针脚或者由节点自己触发。
        
        // 我们这里尝试触发 Then 0，然后 Then 1。
        // 但由于 run_flow_internal 是递归的，我们需要 ctx 支持 run_flow。
        
        ctx.run_flow(&node.id, "Then 0")?;
        Ok("Then 1".into())
    }));
    
    let mut seq = seq;
    seq.set_metadata(vec!["Control".into()], "default".into(), Some("Execute outputs in order".into()));
    registry.register("sequence".into(), Arc::new(seq));
}
