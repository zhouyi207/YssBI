pub mod type_constraint;
pub mod type_inference_context;
pub mod type_inference_session;
pub mod type_var_definition;
pub mod type_var_id;
pub mod type_var_inference;
pub mod type_var_key;

pub use type_constraint::*;
pub use type_inference_context::*;
pub use type_inference_session::*;
pub use type_var_definition::*;
pub use type_var_id::*;
pub use type_var_inference::*;
pub use type_var_key::*;

use crate::graph::{DataType, GraphInstance, PinId};

/// 执行全量类型推断，返回所有被解析的 (PinId, DataType)
pub fn infer_graph(graph_instance: &GraphInstance) -> Result<Vec<(PinId, DataType)>, String> {
    let mut session = TypeInferenceSession::new(graph_instance);
    session.register_all();
    session.infer_all()?;
    let resolved = session.commit_to_graph()?;
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::register::NodeRegistry;
    use crate::graph::{GraphInstance, GraphKind};
    use std::sync::Arc;

    fn math_registry() -> Arc<NodeRegistry> {
        let registry = Arc::new(NodeRegistry::new());
        crate::graph::register::catalog::math::register(&registry);
        registry
    }

    #[test]
    fn type_var_bindings_do_not_accumulate_on_repeated_infer() {
        let graph = GraphInstance::new("Test", GraphKind::Event, math_registry());
        graph
            .create_node("Math:Operators:Add (+)")
            .expect("create add node");

        let live_type_var_count = graph
            .data_state
            .read()
            .unwrap()
            .nodes
            .values()
            .map(|node| node.type_var_map.len())
            .sum::<usize>();

        for _ in 0..20 {
            infer_graph(&graph).expect("infer");
        }

        let binding_count = graph.data_state.read().unwrap().type_var_bindings.len();
        assert!(
            binding_count <= live_type_var_count,
            "expected at most {live_type_var_count} bindings, got {binding_count}"
        );
    }

    #[test]
    fn type_var_bindings_are_not_serialized() {
        let graph = GraphInstance::new("Test", GraphKind::Event, math_registry());
        graph
            .create_node("Math:Operators:Add (+)")
            .expect("create add node");
        infer_graph(&graph).expect("infer");

        let data_state = graph.data_state.read().unwrap();
        let value = serde_json::to_value(&*data_state).expect("serialize data state");
        assert!(
            value.get("typeVarBindings").is_none(),
            "typeVarBindings must not be written to project files"
        );
        assert!(
            value.get("pinTypes").is_none(),
            "pinTypes must not be written to project files"
        );
        let connections = value.get("connections").expect("connections");
        assert!(
            connections.get("links").is_some(),
            "connections should serialize as {{ links: [...] }}"
        );
        assert!(
            connections.get("reverseConnections").is_none(),
            "reverseConnections must not be written to project files"
        );
    }
}
