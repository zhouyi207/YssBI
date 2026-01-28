use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::{GenericOutExecPin, GenericInExecPin};

pub fn register(registry: &NodeRegistry) {
    let plot = GenericNode::new_prototype("plot", "Plot");
    plot.add_in_exec_pin(GenericInExecPin::new(uuid::Uuid::nil(), "In"));
    plot.add_out_exec_pin(GenericOutExecPin::new(uuid::Uuid::nil(), "Out"));
    
    plot.set_flow_processor(Box::new(|ctx, _node| {
        ctx.open_window(
            "plot_window".into(),
            "Data Plot".into(),
            "#/plot".into(),
        )?;
        Ok("Out".into())
    }));
    
    let mut plot = plot;
    plot.set_metadata(vec!["Visualization".into()], "default".into(), Some("Open a new plot window for data visualization".into()));
    registry.register("plot".into(), Arc::new(plot));
}
