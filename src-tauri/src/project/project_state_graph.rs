use super::ProjectState;
use super::unique_name;
use crate::graph::{FunctionSignaturePin, GraphId, GraphInstance, GraphKind};
use crate::project::{GraphDocumentKind, read_project_index};
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

        self.with_graph_mut(function_id, |mut ctx| {
            if ctx.graph_ref().kind != GraphKind::Function {
                return Err(format!("Graph '{}' is not a Function", function_id));
            }

            if let Some(inputs) = inputs {
                ctx.graph().function_inputs = inputs;
            }
            if let Some(outputs) = outputs {
                ctx.graph().function_outputs = outputs;
            }

            Ok(ctx.graph_ref().clone())
        })
    }

    /// Names already used by other graphs of the same kind (in memory + on disk).
    pub fn collect_peer_graph_names(
        &self,
        graph_id: &GraphId,
        graph_kind: &GraphKind,
    ) -> Result<Vec<String>, String> {
        let mut existing: Vec<String> = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .values()
            .filter(|item| item.kind == *graph_kind && item.id != *graph_id)
            .map(|item| item.name.clone())
            .collect();
        if let Some(path) = self.get_path() {
            let expected_kind = GraphDocumentKind::from(graph_kind);
            existing.extend(
                read_project_index(&path)
                    .map_err(|e| e.to_string())?
                    .graphs
                    .into_iter()
                    .filter(|item| item.graph_type == expected_kind && item.id != *graph_id)
                    .map(|item| item.name),
            );
        }
        existing.sort();
        existing.dedup();
        Ok(existing)
    }

    /// Rename a graph: unique name within kind, persist document, return final name + kind.
    pub fn rename_graph(
        &self,
        graph_id: &GraphId,
        new_name: &str,
    ) -> Result<(String, GraphKind), String> {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err("Graph name cannot be empty".to_string());
        }

        if self.get_graph(graph_id).is_none() {
            self.load_graph_from_current_project(graph_id)?;
        }

        let graph_kind = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .get(graph_id)
            .map(|graph| graph.kind.clone())
            .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;

        let existing = self.collect_peer_graph_names(graph_id, &graph_kind)?;
        let final_name = unique_name::unique_name(trimmed, existing);

        self.with_graph_mut(graph_id, |mut ctx| {
            ctx.graph().name = final_name.clone();
            Ok(())
        })?;

        if self.get_path().is_some() {
            self.persist_loaded_graph(graph_id)?;
        }

        Ok((final_name, graph_kind))
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

    #[test]
    fn rename_graph_deduplicates_against_loaded_peer() {
        let state = ProjectState::new();
        let first = state.add_event("Event A");
        let second = state.add_event("Event B");

        let (final_name, _) = state
            .rename_graph(&second.id, "Event A")
            .expect("rename should succeed");

        assert_eq!(final_name, "Event A 1");
        assert_eq!(
            state
                .get_graph(&second.id)
                .expect("graph should exist")
                .name,
            "Event A 1"
        );
        assert_eq!(
            state
                .get_graph(&first.id)
                .expect("peer should exist")
                .name,
            "Event A"
        );
    }
}
