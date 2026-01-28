use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::GenericExecPin;

pub fn register(registry: &NodeRegistry) {
    // 1. On Run Event
    let on_run = GenericNode::new_prototype("event_on_run", "On Run");
    on_run.add_exec_pin(GenericExecPin::new(uuid::Uuid::nil(), "Out"));
    
    let mut on_run = on_run;
    on_run.set_metadata(vec!["Internal".into(), "Events".into()], "event".into(), Some("Start execution".into()));
    registry.register("event_on_run".into(), Arc::new(on_run));

    // 2. Function Entry
    let f_entry = GenericNode::new_prototype("function_entry", "Function Entry");
    f_entry.add_exec_pin(GenericExecPin::new(uuid::Uuid::nil(), "Then"));
    
    let mut f_entry = f_entry;
    f_entry.set_metadata(vec!["Internal".into()], "default".into(), Some("Function start point".into()));
    registry.register("function_entry".into(), Arc::new(f_entry));

    // 3. Macro Inputs
    let m_in = GenericNode::new_prototype("macro_inputs", "Macro Inputs");
    m_in.add_exec_pin(GenericExecPin::new(uuid::Uuid::nil(), "In"));
    
    let mut m_in = m_in;
    m_in.set_metadata(vec!["Internal".into()], "default".into(), Some("Macro entry point".into()));
    registry.register("macro_inputs".into(), Arc::new(m_in));

    // 4. Macro Outputs
    let m_out = GenericNode::new_prototype("macro_outputs", "Macro Outputs");
    m_out.add_exec_pin(GenericExecPin::new(uuid::Uuid::nil(), "Out"));
    
    let mut m_out = m_out;
    m_out.set_metadata(vec!["Internal".into()], "default".into(), Some("Macro exit point".into()));
    registry.register("macro_outputs".into(), Arc::new(m_out));
}
