//! 项目级 Call Function 调用点反向索引：`function_id → [(caller_graph_id, call_node_id)]`。
//!
//! - **读**：`collect_function_call_sites` / `sync_call_nodes_for_function` 只查内存表。
//! - **写**：项目加载时从磁盘 stub 扫描重建；图插入 / 节点增删 / patch 后增量维护。
//! - **磁盘**仍为权威；未加载 caller 的条目由 stub 扫描填充。

use crate::graph::register::value::call::CALL_FUNCTION_NODE_TYPE;
use crate::graph::{GraphId, GraphInstance, NodeId};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallSite {
    pub caller_graph_id: GraphId,
    pub call_node_id: NodeId,
}

#[derive(Clone, Debug, Default)]
pub struct FunctionCallSiteIndex {
    by_function: HashMap<GraphId, Vec<CallSite>>,
    by_caller: HashMap<GraphId, HashMap<NodeId, GraphId>>,
}

impl FunctionCallSiteIndex {
    pub fn clear(&mut self) {
        self.by_function.clear();
        self.by_caller.clear();
    }

    pub fn register(&mut self, caller_graph_id: GraphId, call_node_id: NodeId, function_id: GraphId) {
        if let Some(existing) = self
            .by_caller
            .get(&caller_graph_id)
            .and_then(|nodes| nodes.get(&call_node_id).copied())
        {
            if existing == function_id {
                return;
            }
            self.unregister_node(caller_graph_id, call_node_id);
        }

        self.by_caller
            .entry(caller_graph_id)
            .or_default()
            .insert(call_node_id, function_id);
        self.by_function
            .entry(function_id)
            .or_default()
            .push(CallSite {
                caller_graph_id,
                call_node_id,
            });
    }

    pub fn unregister_node(&mut self, caller_graph_id: GraphId, call_node_id: NodeId) {
        let Some(function_id) = self
            .by_caller
            .get_mut(&caller_graph_id)
            .and_then(|nodes| nodes.remove(&call_node_id))
        else {
            return;
        };

        if let Some(sites) = self.by_function.get_mut(&function_id) {
            sites.retain(|site| {
                !(site.caller_graph_id == caller_graph_id && site.call_node_id == call_node_id)
            });
            if sites.is_empty() {
                self.by_function.remove(&function_id);
            }
        }

        if self
            .by_caller
            .get(&caller_graph_id)
            .is_some_and(|nodes| nodes.is_empty())
        {
            self.by_caller.remove(&caller_graph_id);
        }
    }

    pub fn remove_caller(&mut self, caller_graph_id: &GraphId) {
        let Some(nodes) = self.by_caller.remove(caller_graph_id) else {
            return;
        };
        for (call_node_id, function_id) in nodes {
            if let Some(sites) = self.by_function.get_mut(&function_id) {
                sites.retain(|site| {
                    !(site.caller_graph_id == *caller_graph_id
                        && site.call_node_id == call_node_id)
                });
                if sites.is_empty() {
                    self.by_function.remove(&function_id);
                }
            }
        }
    }

    pub fn replace_caller_from_pairs(
        &mut self,
        caller_graph_id: GraphId,
        pairs: impl IntoIterator<Item = (NodeId, GraphId)>,
    ) {
        self.remove_caller(&caller_graph_id);
        for (call_node_id, function_id) in pairs {
            self.register(caller_graph_id, call_node_id, function_id);
        }
    }

    pub fn sites_for_function(&self, function_id: &GraphId) -> Vec<(GraphId, Vec<NodeId>)> {
        let Some(sites) = self.by_function.get(function_id) else {
            return Vec::new();
        };
        let mut grouped: HashMap<GraphId, Vec<NodeId>> = HashMap::new();
        for site in sites {
            grouped
                .entry(site.caller_graph_id)
                .or_default()
                .push(site.call_node_id);
        }
        grouped.into_iter().collect()
    }
}

/// 从已加载图扫描 Call Function → 目标函数 id。
pub fn call_site_pairs_from_graph(graph: &GraphInstance) -> Vec<(NodeId, GraphId)> {
    let data_state = graph.data_state.read().unwrap();
    data_state
        .nodes
        .iter()
        .filter_map(|(node_id, node)| {
            if node.definition.node_type != CALL_FUNCTION_NODE_TYPE {
                return None;
            }
            let target = node.instance_params.sub_graph_id()?;
            let function_id = uuid::Uuid::parse_str(target).ok().map(GraphId::from)?;
            Some((*node_id, function_id))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::NodeInstanceParams;

    #[test]
    fn register_and_lookup_call_sites() {
        let caller = GraphId::from(uuid::Uuid::new_v4());
        let function = GraphId::from(uuid::Uuid::new_v4());
        let node = NodeId::from(uuid::Uuid::new_v4());

        let mut index = FunctionCallSiteIndex::default();
        index.register(caller, node, function);

        let sites = index.sites_for_function(&function);
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].0, caller);
        assert_eq!(sites[0].1, vec![node]);
    }

    #[test]
    fn unregister_node_removes_reverse_entry() {
        let caller = GraphId::from(uuid::Uuid::new_v4());
        let function = GraphId::from(uuid::Uuid::new_v4());
        let node = NodeId::from(uuid::Uuid::new_v4());

        let mut index = FunctionCallSiteIndex::default();
        index.register(caller, node, function);
        index.unregister_node(caller, node);

        assert!(index.sites_for_function(&function).is_empty());
    }

    #[test]
    fn call_site_pairs_from_graph_reads_subgraph_nodes() {
        let register = {
            let store = crate::project::ProjectStore::default();
            std::sync::Arc::clone(&store.node_register)
        };
        let function_id = GraphId::from(uuid::Uuid::new_v4());
        let mut graph = GraphInstance::new("Main", crate::graph::GraphKind::Event, register);
        let node_id = graph
            .create_node_with_position(
                CALL_FUNCTION_NODE_TYPE,
                0.0,
                0.0,
                Some(NodeInstanceParams::SubGraph {
                    sub_graph_id: function_id.to_string(),
                }),
            )
            .expect("create call");

        let pairs = call_site_pairs_from_graph(&graph);
        assert_eq!(pairs, vec![(node_id, function_id)]);
    }
}
