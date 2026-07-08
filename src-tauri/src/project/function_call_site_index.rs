//! 项目级 Call Function 调用点反向索引：`function_path → [(caller_graph_path, call_node_id)]`。
//!
//! - **读**：`get_function_call_sites` / `sync_call_nodes_for_function` 只查内存表（增量维护，无全量 rescan）。
//! - **写**：项目加载时从磁盘 stub 扫描重建；图插入 / 节点增删 / patch 后增量维护。
//! - **磁盘**仍为权威；未加载 caller 的条目由 stub 扫描填充。

use crate::graph::register::value::call::CALL_FUNCTION_NODE_TYPE;
use crate::graph::{GraphInstance, NodeId};
use crate::project::normalize_graph_resource_path;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallSite {
    pub caller_graph_path: String,
    pub call_node_id: NodeId,
}

#[derive(Clone, Debug, Default)]
pub struct FunctionCallSiteIndex {
    by_function: HashMap<String, Vec<CallSite>>,
    by_caller: HashMap<String, HashMap<NodeId, String>>,
}

impl FunctionCallSiteIndex {
    pub fn clear(&mut self) {
        self.by_function.clear();
        self.by_caller.clear();
    }

    pub fn register(
        &mut self,
        caller_graph_path: String,
        call_node_id: NodeId,
        function_path: String,
    ) {
        let caller_graph_path = normalize_graph_resource_path(&caller_graph_path);
        let function_path = normalize_graph_resource_path(&function_path);
        if let Some(existing) = self
            .by_caller
            .get(&caller_graph_path)
            .and_then(|nodes| nodes.get(&call_node_id))
            .cloned()
        {
            if existing == function_path {
                return;
            }
            self.unregister_node(&caller_graph_path, call_node_id);
        }

        self.by_caller
            .entry(caller_graph_path.clone())
            .or_default()
            .insert(call_node_id, function_path.clone());
        self.by_function
            .entry(function_path)
            .or_default()
            .push(CallSite {
                caller_graph_path,
                call_node_id,
            });
    }

    pub fn unregister_node(&mut self, caller_graph_path: &str, call_node_id: NodeId) {
        let caller_graph_path = normalize_graph_resource_path(caller_graph_path);
        let Some(function_path) = self
            .by_caller
            .get_mut(&caller_graph_path)
            .and_then(|nodes| nodes.remove(&call_node_id))
        else {
            return;
        };

        if let Some(sites) = self.by_function.get_mut(&function_path) {
            sites.retain(|site| {
                !(site.caller_graph_path == caller_graph_path && site.call_node_id == call_node_id)
            });
            if sites.is_empty() {
                self.by_function.remove(&function_path);
            }
        }

        if self
            .by_caller
            .get(&caller_graph_path)
            .is_some_and(|nodes| nodes.is_empty())
        {
            self.by_caller.remove(&caller_graph_path);
        }
    }

    pub fn remove_caller(&mut self, caller_graph_path: &str) {
        let caller_graph_path = normalize_graph_resource_path(caller_graph_path);
        let Some(nodes) = self.by_caller.remove(&caller_graph_path) else {
            return;
        };
        for (call_node_id, function_path) in nodes {
            if let Some(sites) = self.by_function.get_mut(&function_path) {
                sites.retain(|site| {
                    !(site.caller_graph_path == caller_graph_path
                        && site.call_node_id == call_node_id)
                });
                if sites.is_empty() {
                    self.by_function.remove(&function_path);
                }
            }
        }
    }

    /// 删除函数图时清理反向索引；caller 图内 Call 节点仍由磁盘/打开图 reconcile 处理。
    pub fn remove_function(&mut self, function_path: &str) {
        let function_path = normalize_graph_resource_path(function_path);
        self.by_function.remove(&function_path);
        for nodes in self.by_caller.values_mut() {
            nodes.retain(|_, fp| normalize_graph_resource_path(fp) != function_path);
        }
    }

    pub fn replace_caller_from_pairs(
        &mut self,
        caller_graph_path: String,
        pairs: impl IntoIterator<Item = (NodeId, String)>,
    ) {
        self.remove_caller(&caller_graph_path);
        for (call_node_id, function_path) in pairs {
            self.register(caller_graph_path.clone(), call_node_id, function_path);
        }
    }

    pub fn sites_for_function(&self, function_path: &str) -> Vec<(String, Vec<NodeId>)> {
        let function_path = normalize_graph_resource_path(function_path);
        let Some(sites) = self.by_function.get(&function_path) else {
            return Vec::new();
        };
        let mut grouped: HashMap<String, Vec<NodeId>> = HashMap::new();
        for site in sites {
            grouped
                .entry(site.caller_graph_path.clone())
                .or_default()
                .push(site.call_node_id);
        }
        grouped.into_iter().collect()
    }
}

/// 从已加载图扫描 Call Function → 目标函数 path。
pub fn call_site_pairs_from_graph(graph: &GraphInstance) -> Vec<(NodeId, String)> {
    let data_state = graph.data_state.read().unwrap();
    data_state
        .nodes
        .iter()
        .filter_map(|(node_id, node)| {
            if node.definition.node_type != CALL_FUNCTION_NODE_TYPE {
                return None;
            }
            let function_path = node.instance_params.sub_graph_path()?;
            Some((*node_id, normalize_graph_resource_path(function_path)))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::NodeInstanceParams;
    use crate::project::GraphResourcePath;

    #[test]
    fn register_and_lookup_call_sites() {
        let caller = "events/Caller.yssbi-event".to_string();
        let function = "functions/Target.yssbi-function".to_string();
        let node = NodeId::from(uuid::Uuid::new_v4());

        let mut index = FunctionCallSiteIndex::default();
        index.register(caller.clone(), node, function.clone());

        let sites = index.sites_for_function(&function);
        assert_eq!(sites.len(), 1);
        assert_eq!(sites[0].0, caller);
        assert_eq!(sites[0].1, vec![node]);
    }

    #[test]
    fn unregister_node_removes_reverse_entry() {
        let caller = "events/Caller.yssbi-event".to_string();
        let function = "functions/Target.yssbi-function".to_string();
        let node = NodeId::from(uuid::Uuid::new_v4());

        let mut index = FunctionCallSiteIndex::default();
        index.register(caller.clone(), node, function.clone());
        index.unregister_node(&caller, node);

        assert!(index.sites_for_function(&function).is_empty());
    }

    #[test]
    fn remove_function_clears_reverse_index() {
        let caller = "events/Caller.yssbi-event".to_string();
        let function = "functions/Target.yssbi-function".to_string();
        let node = NodeId::from(uuid::Uuid::new_v4());

        let mut index = FunctionCallSiteIndex::default();
        index.register(caller.clone(), node, function.clone());
        index.remove_function(&function);

        assert!(index.sites_for_function(&function).is_empty());
        assert!(index.by_caller.get(&caller).is_none_or(|m| m.is_empty()));
    }

    #[test]
    fn call_site_pairs_from_graph_reads_subgraph_nodes() {
        let register = {
            let store = crate::project::ProjectStore::default();
            std::sync::Arc::clone(&store.node_register)
        };
        let function_path = "functions/Helper.yssbi-function".to_string();
        let graph = GraphInstance::new_with_path(
            "Main",
            crate::graph::GraphKind::Event,
            register,
            GraphResourcePath::new("events/Main.yssbi-event").unwrap(),
        );
        let node_id = graph
            .create_node_with_position(
                CALL_FUNCTION_NODE_TYPE,
                0.0,
                0.0,
                Some(NodeInstanceParams::SubGraph {
                    sub_graph_path: function_path.clone(),
                }),
            )
            .expect("create call");

        let pairs = call_site_pairs_from_graph(&graph);
        assert_eq!(pairs, vec![(node_id, function_path)]);
    }
}
