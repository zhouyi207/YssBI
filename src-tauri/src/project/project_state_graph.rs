use super::ProjectState;
use super::unique_name;
use crate::graph::{FunctionSignaturePin, GraphId, GraphInstance, GraphKind};
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

    pub fn add_function(&self, graph_name: &str) -> GraphInstance {
        self.add_graph(graph_name, GraphKind::Function)
    }

    pub fn update_function_signature(
        &self,
        function_id: &GraphId,
        inputs: Option<Vec<FunctionSignaturePin>>,
        outputs: Option<Vec<FunctionSignaturePin>>,
    ) -> Result<GraphInstance, String> {
        if self.get_graph(function_id).is_none() {
            self.load_graph_from_current_project(function_id)?;
        }

        let mut project_data = self.project_data.write().unwrap();
        let graph = project_data
            .graphs
            .get_mut(function_id)
            .ok_or_else(|| format!("Function graph '{}' not found", function_id))?;

        if graph.kind != GraphKind::Function {
            return Err(format!("Graph '{}' is not a Function", function_id));
        }

        if let Some(inputs) = inputs {
            graph.function_inputs = inputs;
        }
        if let Some(outputs) = outputs {
            graph.function_outputs = outputs;
        }

        Ok(graph.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signature_pin(id: &str) -> FunctionSignaturePin {
        FunctionSignaturePin {
            id: id.to_string(),
            name: id.to_string(),
            pin_type: "int".to_string(),
            container_type: None,
        }
    }

    #[test]
    fn update_function_signature_only_updates_target_function() {
        let state = ProjectState::new();
        let target = state.add_function("Target");
        let other = state.add_function("Other");

        let updated = state
            .update_function_signature(&target.id, Some(vec![signature_pin("input")]), None)
            .expect("function signature should update");

        assert_eq!(updated.function_inputs, vec![signature_pin("input")]);
        assert!(updated.function_outputs.is_empty());

        let other_graph = state
            .get_graph(&other.id)
            .expect("other function should exist");
        assert!(other_graph.function_inputs.is_empty());
        assert!(other_graph.function_outputs.is_empty());
    }

    #[test]
    fn update_function_signature_rejects_event_graphs() {
        let state = ProjectState::new();
        let event = state.add_event("Event");

        let result =
            state.update_function_signature(&event.id, Some(vec![signature_pin("input")]), None);

        assert!(result.is_err());
        let event_graph = state.get_graph(&event.id).expect("event should exist");
        assert!(event_graph.function_inputs.is_empty());
    }
}
