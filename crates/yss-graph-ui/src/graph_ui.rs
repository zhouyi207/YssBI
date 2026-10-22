//! Graph editing presentation over Application-owned graph state.
pub mod canvas;
pub mod catalog_rows;
pub mod details;
pub mod drag;
pub mod feedback;
mod graph_properties;
mod node_creation;
mod opened_graph;
pub mod overview;
mod parameters;
pub mod problems;
pub mod results;

pub use opened_graph::OpenedGraph;

use std::sync::Arc;
use yss_application::{events::CommittedResourceMutation, session::ApplicationState};
use yss_desktop_services::ApplicationTasks;
use yss_ui::layout_store::LayoutStore;

/// The graph feature's async application capability and viewport persistence.
pub struct GraphServices {
    tasks: Arc<ApplicationTasks>,
    layouts: Arc<LayoutStore>,
}

impl GraphServices {
    pub fn new(tasks: Arc<ApplicationTasks>, layouts: Arc<LayoutStore>) -> Self {
        Self { tasks, layouts }
    }

    fn run<T: Send + 'static>(
        self: &Arc<Self>,
        operation: impl FnOnce(&ApplicationState) -> anyhow::Result<T> + Send + 'static,
    ) -> tokio::task::JoinHandle<anyhow::Result<T>> {
        self.tasks.run(operation)
    }

    fn publish_resource(&self, mutation: CommittedResourceMutation) {
        self.tasks.publish_resource(mutation);
    }
}
