use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::GenericOutExecPin;

pub fn register(registry: &NodeRegistry) {
    // 1. On Run Event
    let on_run = GenericNode::new_prototype("event_on_run", "On Run");
    on_run.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Exec"));
    on_run.set_flow_processor(Box::new(|_ctx, _node| {
        Ok("Exec".into())
    }));
    
    let mut on_run = on_run;
    on_run.set_metadata(vec!["Internal".into(), "Events".into()], "event".into(), Some("Start execution".into()));
    registry.register("event_on_run".into(), Arc::new(on_run));

    // 2. Function Entry
    let f_entry = GenericNode::new_prototype("function_entry", "Function Entry");
    f_entry.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Then"));
    f_entry.set_flow_processor(Box::new(|_ctx, _node| {
        Ok("Then".into())
    }));
    
    let mut f_entry = f_entry;
    f_entry.set_metadata(vec!["Internal".into()], "default".into(), Some("Function start point".into()));
    registry.register("function_entry".into(), Arc::new(f_entry));

    // 3. Macro Inputs
    let m_in = GenericNode::new_prototype("macro_inputs", "Macro Inputs");
    m_in.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "In"));
    m_in.set_flow_processor(Box::new(|_ctx, _node| {
        Ok("In".into())
    }));
    
    let mut m_in = m_in;
    m_in.set_metadata(vec!["Internal".into()], "default".into(), Some("Macro entry point".into()));
    registry.register("macro_inputs".into(), Arc::new(m_in));

    // 4. Macro Outputs
    let m_out = GenericNode::new_prototype("macro_outputs", "Macro Outputs");
    m_out.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Out"));
    // Macro Outputs usually ends the macro flow, but it might have a continuation in the caller.
    // However, the caller handles continuation. 
    // If it's a GenericNode, we can return empty to stop flow within the macro.
    
    let mut m_out = m_out;
    m_out.set_metadata(vec!["Internal".into()], "default".into(), Some("Macro exit point".into()));
    registry.register("macro_outputs".into(), Arc::new(m_out));
}
