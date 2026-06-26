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
    use crate::graph::pin::{DataRole, PinRole};
    use crate::graph::register::NodeRegistry;
    use crate::graph::{GraphInstance, GraphKind, NodeId};
    use std::sync::Arc;

    fn math_registry() -> Arc<NodeRegistry> {
        let registry = Arc::new(NodeRegistry::new());
        crate::graph::register::catalog::math::register(&registry);
        registry
    }

    /// best-effort 全图推断：含一条不兼容连接（String→数值）时，`infer_graph`
    /// 不再整图失败，而是跳过脏边并继续推断其余正常连接（Float64→数值）。
    #[test]
    fn infer_all_skips_incompatible_edge_and_infers_the_rest() {
        let registry = Arc::new(NodeRegistry::new());
        crate::graph::register::catalog::math::register(&registry);
        crate::graph::register::catalog::value::register(&registry);
        let graph = GraphInstance::new("Test", GraphKind::Event, registry);

        let string_node = graph
            .create_node("Value:Constants:String")
            .expect("string const");
        let f64_node = graph
            .create_node("Value:Constants:Float64")
            .expect("float64 const");
        let sqrt_bad = graph.create_node("Math:Functions:Sqrt").expect("sqrt bad");
        let sqrt_good = graph.create_node("Math:Functions:Sqrt").expect("sqrt good");

        let result_pin = |nid: NodeId| {
            graph
                .get_pin_instances_by_node_id(nid)
                .into_iter()
                .find(|p| p.definition.role == PinRole::Data(DataRole::Result))
                .expect("result pin")
                .id
        };
        let input_pin = |nid: NodeId| {
            graph
                .get_pin_instances_by_node_id(nid)
                .into_iter()
                .find(|p| p.definition.role == PinRole::Data(DataRole::Input))
                .expect("input pin")
                .id
        };

        let string_out = result_pin(string_node);
        let f64_out = result_pin(f64_node);
        let sqrt_bad_in = input_pin(sqrt_bad);
        let sqrt_good_in = input_pin(sqrt_good);

        // 直接注入连接，绕过拓扑层的类型校验：模拟历史脏边 + 正常边并存。
        {
            let ds = graph.data_state.read().unwrap();
            ds.connections.connect(string_out, sqrt_bad_in);
            ds.connections.connect(f64_out, sqrt_good_in);
        }

        let resolved =
            infer_graph(&graph).expect("infer must be Ok despite one incompatible edge");

        // 正常边对应的输入 pin 仍被细化为 Float64，未被脏边毒化。
        let good_type = resolved
            .iter()
            .find(|(pid, _)| *pid == sqrt_good_in)
            .map(|(_, dt)| dt.clone());
        assert_eq!(
            good_type,
            Some(DataType::Float64),
            "valid edge should still resolve to Float64"
        );
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

        let value = serde_json::to_value(&graph).expect("serialize graph");

        // 运行期缓存绝不落盘
        assert!(
            value.get("typeVarBindings").is_none(),
            "typeVarBindings must not be written to project files"
        );
        assert!(
            value.get("pinTypes").is_none(),
            "pinTypes must not be written to project files"
        );

        // 扁平、与快照对齐的磁盘格式：无 dataState 包裹，nodes/connections 为数组
        assert!(
            value.get("dataState").is_none(),
            "graph should serialize flat (no dataState wrapper)"
        );
        assert!(
            value.get("nodes").and_then(|n| n.as_array()).is_some(),
            "nodes should serialize as a flat array"
        );
        assert!(
            value.get("connections").and_then(|c| c.as_array()).is_some(),
            "connections should serialize as a flat array"
        );
    }
}
