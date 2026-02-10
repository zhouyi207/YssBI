use super::ProjectState;
use crate::graph::{GraphData, GraphId, GraphKind};
use std::sync::Arc;

impl ProjectState {
    fn add_graph(&self, graph_name: &str, graph_kind: GraphKind) -> GraphData {
        let graph_id = GraphId::new();
        let graph_register = {
            let store = self.project_store.read().unwrap();
            Arc::clone(&store.node_register)
        };
        let graph_data =
            GraphData::new(graph_id, graph_name.to_string(), graph_kind, graph_register);
        self.project_data
            .write()
            .unwrap()
            .graphs
            .insert(graph_id, graph_data.clone());
        graph_data
    }

    pub fn remove_graph(&self, graph_id: &GraphId) -> Option<GraphData> {
        self.project_data.write().unwrap().graphs.remove(graph_id)
    }

    pub fn get_graph(&self, graph_id: &GraphId) -> Option<GraphData> {
        self.project_data.read().unwrap().graphs.get(graph_id).cloned()
    }

    pub fn add_event(&self, graph_name: &str) -> GraphData {
        self.add_graph(graph_name, GraphKind::Event)
    }

    /// 可能会拓展
    pub fn update_event(&self) {}

    pub fn add_function(&self, graph_name: &str) -> GraphData {
        self.add_graph(graph_name, GraphKind::Function)
    }

    /// 可能会拓展
    pub fn update_function(&self) {}

    pub fn add_macro(&self, graph_name: &str) -> GraphData {
        self.add_graph(graph_name, GraphKind::Macro)
    }

    /// 可能会拓展
    pub fn update_macro() {}
}
