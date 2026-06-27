use super::ProjectState;
use super::unique_name;
use crate::graph::{GraphId, GraphInstance, GraphKind};
use crate::variable::VariableScope;
use std::sync::Arc;

impl ProjectState {
    pub fn add_graph_with_existing_names(
        &self,
        graph_name: &str,
        graph_kind: GraphKind,
        existing_names: Vec<String>,
    ) -> GraphInstance {
        let unique_graph_name = {
            let project_data = self.project_data.read().unwrap();
            let mut existing: Vec<String> = project_data
                .graphs
                .values()
                .filter(|g| g.kind == graph_kind)
                .map(|g| g.name.clone())
                .collect();
            existing.extend(existing_names);
            unique_name::unique_name(graph_name, existing)
        };

        let graph_register = {
            let store = self.project_store.read().unwrap();
            Arc::clone(&store.node_register)
        };
        let graph_data = GraphInstance::new(&unique_graph_name, graph_kind, graph_register);
        // Funnel through the single `insert_graph` entry point so registry +
        // schema provider + schema propagation are bound consistently with the
        // load / duplicate / import paths.
        self.insert_graph(graph_data)
    }

    pub fn add_graph(&self, graph_name: &str, graph_kind: GraphKind) -> GraphInstance {
        self.add_graph_with_existing_names(graph_name, graph_kind, Vec::new())
    }

    pub fn remove_graph(&self, graph_id: &GraphId) -> Option<GraphInstance> {
        self.project_data.write().unwrap().graphs.remove(graph_id)
    }

    pub fn unload_graph(&self, graph_id: &GraphId) {
        let graph_id_string = graph_id.to_string();
        let mut data = self.project_data.write().unwrap();
        data.graphs.remove(graph_id);
        data.variables.retain(|_, variable| match &variable.scope {
            VariableScope::Global => true,
            VariableScope::Event { event_id } => event_id != &graph_id_string,
            VariableScope::Function { function_id } => function_id != &graph_id_string,
        });
    }

    pub fn get_graph(&self, graph_id: &GraphId) -> Option<GraphInstance> {
        self.project_data
            .read()
            .unwrap()
            .graphs
            .get(graph_id)
            .cloned()
    }

    pub fn add_event(&self, graph_name: &str) -> GraphInstance {
        self.add_graph(graph_name, GraphKind::Event)
    }

    /// 可能会拓展
    pub fn update_event(&self) {}

    pub fn add_function(&self, graph_name: &str) -> GraphInstance {
        self.add_graph(graph_name, GraphKind::Function)
    }

    /// 可能会拓展
    pub fn update_function(&self) {}
}
