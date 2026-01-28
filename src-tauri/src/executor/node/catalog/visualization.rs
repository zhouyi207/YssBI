use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::GenericExecPin;

pub fn register(registry: &NodeRegistry) {
    let plot = GenericNode::new_prototype("plot", "Plot");
    plot.add_exec_pin(GenericExecPin::new(uuid::Uuid::nil(), "In"));
    plot.add_exec_pin(GenericExecPin::new(uuid::Uuid::nil(), "Out"));
    
    plot.set_flow_processor(Box::new(|ctx, _node| {
        ctx.open_window(
            "plot_window".into(),
            "Data Plot".into(),
            "/plot".into(),
        )?;
        Ok("Out".into())
    }));
    
    let mut plot = plot;
    plot.set_metadata(vec!["Visualization".into()], "default".into(), Some("Open a new plot window for data visualization".into()));
    registry.register("plot".into(), Arc::new(plot));
}
