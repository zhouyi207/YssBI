use super::ProjectState;
use super::unique_name;
use crate::graph::{
    FunctionSignaturePin, GraphId, GraphInstance, GraphKind, GraphRecompileScope, NodeInstanceParams,
    PinChangeSet,
};
use crate::graph::register::value::call::CALL_FUNCTION_NODE_TYPE;
use crate::project::{
    FunctionSignatureEntry, FunctionSignatureTable, GraphDocumentKind,
    call_site_pairs_from_graph, read_function_signature_header_from_project,
    read_graph_call_sites_from_project, read_project_index,
};
use crate::variable::VariableScope;
use std::collections::HashMap;
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
        let removed = self.project_data.write().unwrap().graphs.remove(graph_id);
        if removed.as_ref().is_some_and(|g| g.kind == GraphKind::Function) {
            self.function_signatures().write().unwrap().remove(graph_id);
        }
        self.function_call_sites().write().unwrap().remove_caller(graph_id);
        removed
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
        drop(data);
        self.refresh_call_sites_for_caller(graph_id);
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
    ) -> Result<(GraphInstance, Vec<PinChangeSet>), String> {
        if self.get_graph(function_id).is_none() {
            self.load_graph_from_current_project(function_id)?;
        }

        let (graph, change_sets) = self.with_graph_mut(function_id, |mut ctx| {
            if ctx.graph_ref().kind != GraphKind::Function {
                return Err(format!("Graph '{}' is not a Function", function_id));
            }

            if let Some(inputs) = inputs {
                ctx.graph().function_inputs = inputs;
            }
            if let Some(outputs) = outputs {
                ctx.graph().function_outputs = outputs;
            }

            // 将新签名投影到 Entry / Return 壳节点的 pin 上。
            let change_sets = ctx.graph_ref().sync_function_shell_pins();
            ctx.recompile(GraphRecompileScope::InferOnly);

            Ok((ctx.graph_ref().clone(), change_sets))
        })?;
        self.function_signatures()
            .write()
            .unwrap()
            .upsert_from_graph(&graph);
        Ok((graph, change_sets))
    }

    /// Call Function 节点：从 node_type + params 解析目标函数 id。
    pub fn call_function_target_id(
        node_type: &str,
        params: Option<&NodeInstanceParams>,
    ) -> Option<GraphId> {
        if node_type != CALL_FUNCTION_NODE_TYPE {
            return None;
        }
        params
            .and_then(|p| p.sub_graph_id())
            .and_then(|s| uuid::Uuid::parse_str(s).ok())
            .map(GraphId::from)
    }

    /// 从项目索引重建函数签名表，并用已加载函数图覆盖（内存为最新）。
    pub fn rebuild_function_signature_table(&self) -> Result<(), String> {
        let Some(path) = self.get_path() else {
            self.function_signatures().write().unwrap().clear();
            return Ok(());
        };
        let index = read_project_index(&path).map_err(|e| e.to_string())?;
        let mut table = FunctionSignatureTable::default();
        for entry in index.graphs {
            if entry.graph_type != GraphDocumentKind::Function {
                continue;
            }
            table.upsert(entry.id, entry.function_inputs, entry.function_outputs);
        }
        let data = self.project_data.read().unwrap();
        for graph in data.graphs.values() {
            if graph.kind == GraphKind::Function {
                table.upsert_from_graph(graph);
            }
        }
        *self.function_signatures().write().unwrap() = table;
        Ok(())
    }

    /// 从项目磁盘 stub 扫描重建 Call 索引，并用已加载图覆盖（内存为最新）。
    pub fn rebuild_function_call_site_index(&self) -> Result<(), String> {
        let Some(path) = self.get_path() else {
            self.function_call_sites().write().unwrap().clear();
            return Ok(());
        };
        let index = read_project_index(&path).map_err(|e| e.to_string())?;
        let mut call_index = crate::project::FunctionCallSiteIndex::default();
        for entry in index.graphs {
            let stubs = read_graph_call_sites_from_project(&path, &entry.id)
                .map_err(|e| e.to_string())?;
            let pairs: Vec<_> = stubs
                .into_iter()
                .filter_map(|stub| {
                    stub.target_function_id
                        .map(|function_id| (stub.node_id, function_id))
                })
                .collect();
            call_index.replace_caller_from_pairs(entry.id, pairs);
        }
        let data = self.project_data.read().unwrap();
        for graph in data.graphs.values() {
            call_index.replace_caller_from_pairs(graph.id, call_site_pairs_from_graph(graph));
        }
        *self.function_call_sites().write().unwrap() = call_index;
        Ok(())
    }

    /// 刷新某 caller 图在索引中的 Call 条目：已加载图优先，否则读磁盘 stub。
    pub fn refresh_call_sites_for_caller(&self, caller_graph_id: &GraphId) {
        if let Some(graph) = self.get_graph(caller_graph_id) {
            let pairs = call_site_pairs_from_graph(&graph);
            self.function_call_sites()
                .write()
                .unwrap()
                .replace_caller_from_pairs(*caller_graph_id, pairs);
            return;
        }
        if let Some(path) = self.get_path() {
            if let Ok(stubs) = read_graph_call_sites_from_project(&path, caller_graph_id) {
                let pairs: Vec<_> = stubs
                    .into_iter()
                    .filter_map(|stub| {
                        stub.target_function_id
                            .map(|function_id| (stub.node_id, function_id))
                    })
                    .collect();
                self.function_call_sites()
                    .write()
                    .unwrap()
                    .replace_caller_from_pairs(*caller_graph_id, pairs);
                return;
            }
        }
        self.function_call_sites()
            .write()
            .unwrap()
            .remove_caller(caller_graph_id);
    }

    /// 节点创建后登记 Call 调用点（非 Call 节点为 no-op）。
    pub fn register_call_site_for_node(
        &self,
        caller_graph_id: &GraphId,
        call_node_id: crate::graph::NodeId,
        node_type: &str,
        params: Option<&NodeInstanceParams>,
    ) {
        let Some(function_id) = Self::call_function_target_id(node_type, params) else {
            return;
        };
        self.function_call_sites()
            .write()
            .unwrap()
            .register(*caller_graph_id, call_node_id, function_id);
    }

    /// 节点删除前移除 Call 调用点（非 Call 节点为 no-op）。
    pub fn unregister_call_site_for_node(
        &self,
        caller_graph_id: &GraphId,
        call_node_id: crate::graph::NodeId,
        node_type: &str,
    ) {
        if node_type != CALL_FUNCTION_NODE_TYPE {
            return;
        }
        self.function_call_sites()
            .write()
            .unwrap()
            .unregister_node(*caller_graph_id, call_node_id);
    }

    fn cache_function_signature(&self, function_id: &GraphId, entry: &FunctionSignatureEntry) {
        self.function_signatures()
            .write()
            .unwrap()
            .upsert(
                *function_id,
                entry.inputs.clone(),
                entry.outputs.clone(),
            );
    }

    /// 解析函数签名：已加载图 > 签名表 > 图文件头（不加载整图）。
    pub fn get_function_signature(
        &self,
        function_id: &GraphId,
    ) -> Result<FunctionSignatureEntry, String> {
        if let Some(graph) = self.get_graph(function_id) {
            if graph.kind == GraphKind::Function {
                let entry = FunctionSignatureEntry {
                    inputs: graph.function_inputs.clone(),
                    outputs: graph.function_outputs.clone(),
                };
                self.cache_function_signature(function_id, &entry);
                return Ok(entry);
            }
        }
        if let Some(entry) = self.function_signatures().read().unwrap().get_cloned(function_id) {
            return Ok(entry);
        }
        let Some(path) = self.get_path() else {
            return Err(format!("Function signature for '{}' not found", function_id));
        };
        let entry = read_function_signature_header_from_project(&path, function_id)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Function signature for '{}' not found", function_id))?;
        self.cache_function_signature(function_id, &entry);
        Ok(entry)
    }

    /// Call Function：锁外解析目标签名（不加载整图）。
    pub fn resolve_call_projection_signature(
        &self,
        node_type: &str,
        params: Option<&NodeInstanceParams>,
    ) -> Result<Option<FunctionSignatureEntry>, String> {
        let Some(target_id) = Self::call_function_target_id(node_type, params) else {
            return Ok(None);
        };
        Ok(Some(self.get_function_signature(&target_id)?))
    }

    /// 节点创建后：若为 Call Function，将目标函数签名投影到该节点 pin。
    pub fn project_call_node_pins(
        &self,
        caller_graph_id: &GraphId,
        node_id: crate::graph::NodeId,
        node_type: &str,
        params: Option<&NodeInstanceParams>,
    ) -> Result<Option<(GraphInstance, PinChangeSet)>, String> {
        let Some(target_id) = Self::call_function_target_id(node_type, params) else {
            return Ok(None);
        };
        let out = self.sync_call_node(caller_graph_id, node_id, &target_id)?;
        Ok(Some(out))
    }

    /// 依据目标函数签名，同步某个调用方图内单个 Call Function 节点的 pin（不加载目标整图）。
    pub fn sync_call_node(
        &self,
        caller_graph_id: &GraphId,
        call_node_id: crate::graph::NodeId,
        target_function_id: &GraphId,
    ) -> Result<(GraphInstance, PinChangeSet), String> {
        let signature = self.get_function_signature(target_function_id)?;

        self.with_graph_mut(caller_graph_id, |ctx| {
            let change_set = ctx.graph_ref().sync_call_function_pins_from_signature(
                call_node_id,
                &signature.inputs,
                &signature.outputs,
            );
            ctx.recompile(GraphRecompileScope::InferOnly);
            Ok((ctx.graph_ref().clone(), change_set))
        })
    }

    /// 收集项目中所有引用 `function_id` 的 Call Function 节点（读内存索引）。
    pub fn get_function_call_sites(
        &self,
        function_id: &GraphId,
    ) -> Vec<(GraphId, Vec<crate::graph::NodeId>)> {
        self.sync_call_site_index_from_loaded_graphs();
        self.function_call_sites()
            .read()
            .unwrap()
            .sites_for_function(function_id)
    }

    /// 已加载图是权威态：把内存中的 Call 站点写回索引（不读盘）。
    fn sync_call_site_index_from_loaded_graphs(&self) {
        let graphs: Vec<GraphInstance> = {
            let data = self.project_data.read().unwrap();
            data.graphs.values().cloned().collect()
        };
        let mut index = self.function_call_sites().write().unwrap();
        for graph in graphs {
            index.replace_caller_from_pairs(graph.id, call_site_pairs_from_graph(&graph));
        }
    }

    fn collect_function_call_sites(&self, function_id: &GraphId) -> Vec<(GraphId, Vec<crate::graph::NodeId>)> {
        self.get_function_call_sites(function_id)
    }

    /// 目标函数签名变化后，同步所有引用该函数的 Call 节点 pin（含未加载的调用方图）。
    /// 返回每个受影响图的 `(graph_id, graph, change_sets)` 供命令层发事件。
    pub fn sync_call_nodes_for_function(
        &self,
        function_id: &GraphId,
    ) -> Vec<(GraphId, GraphInstance, Vec<PinChangeSet>)> {
        let Ok(signature) = self.get_function_signature(function_id) else {
            return Vec::new();
        };

        let callers = self.collect_function_call_sites(function_id);
        if callers.is_empty() {
            return Vec::new();
        }

        let mut out = Vec::new();
        let mut persist_unloaded = Vec::new();
        for (gid, node_ids) in callers {
            let was_loaded = self.get_graph(&gid).is_some();
            if !was_loaded && self.load_graph_from_current_project(&gid).is_err() {
                continue;
            }

            let result = self.with_graph_mut(&gid, |ctx| {
                let mut sets = Vec::new();
                for nid in &node_ids {
                    sets.push(ctx.graph_ref().sync_call_function_pins_from_signature(
                        *nid,
                        &signature.inputs,
                        &signature.outputs,
                    ));
                }
                ctx.recompile(GraphRecompileScope::InferOnly);
                Ok((gid, ctx.graph_ref().clone(), sets))
            });
            if let Ok(entry) = result {
                if !was_loaded {
                    persist_unloaded.push(entry.0);
                }
                out.push(entry);
            }
        }

        for gid in persist_unloaded {
            let _ = self.persist_loaded_graph(&gid);
        }
        out
    }

    /// 函数体是否含「副作用 / 控制流」节点：即非壳节点且带 exec pin。
    ///
    /// 签名无 exec 入参时函数按数据拉取求值，这类节点不会被执行。用于签名保存时提示用户。
    pub fn function_has_side_effect_nodes(&self, function_id: &GraphId) -> bool {
        let Some(graph) = self.get_graph(function_id) else {
            return false;
        };
        let ds = graph.data_state.read().unwrap();
        ds.nodes.iter().any(|(_, node)| {
            if node.definition.metadata.shell_role.is_some() {
                return false;
            }
            node.pin_ids.iter().any(|pid| {
                ds.pins
                    .get(pid)
                    .map(|p| p.is_exec())
                    .unwrap_or(false)
            })
        })
    }

    /// 重建某个图内所有 Call Function 节点的 pin（按各自目标函数的当前签名）。
    ///
    /// Call pin 是目标函数签名的**派生**投影：持久化仅作缓存。图加载 / tab 打开时调用本方法，
    /// 保证即使目标函数在本图 unload 期间改过签名，Call pin 也不会陈旧。
    /// 返回变更集供命令层发 `NodePinsUpdated` 事件。
    pub fn sync_all_call_nodes_in_graph(&self, graph_id: &GraphId) -> Vec<PinChangeSet> {
        let calls: Vec<(crate::graph::NodeId, GraphId)> = {
            let Some(graph) = self.get_graph(graph_id) else {
                return Vec::new();
            };
            call_site_pairs_from_graph(&graph)
        };

        if calls.is_empty() {
            return Vec::new();
        }

        let mut signatures: HashMap<GraphId, FunctionSignatureEntry> = HashMap::new();
        for (_, target_id) in &calls {
            if signatures.contains_key(target_id) {
                continue;
            }
            if let Ok(signature) = self.get_function_signature(target_id) {
                signatures.insert(*target_id, signature);
            }
        }

        let result = self.with_graph_mut(graph_id, |ctx| {
            let mut sets = Vec::new();
            for (call_node_id, target_id) in calls {
                let Some(signature) = signatures.get(&target_id) else {
                    continue;
                };
                sets.push(ctx.graph_ref().sync_call_function_pins_from_signature(
                    call_node_id,
                    &signature.inputs,
                    &signature.outputs,
                ));
            }
            ctx.recompile(GraphRecompileScope::InferOnly);
            Ok(sets)
        });

        result.unwrap_or_default()
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

    /// 执行前预加载本次可能触达的函数图（含嵌套 Call），避免 Call Function 运行时
    /// `project_data.graphs` 中缺少目标函数。
    pub fn preload_execution_dependencies(
        &self,
        target_graph_id: Option<GraphId>,
    ) -> Result<(), String> {
        use std::collections::{HashSet, VecDeque};

        let entry_ids: Vec<GraphId> = {
            let data = self.project_data.read().unwrap();
            data.graphs
                .iter()
                .filter(|(graph_id, graph)| {
                    graph.kind == GraphKind::Event
                        && target_graph_id.is_none_or(|target| **graph_id == target)
                })
                .map(|(graph_id, _)| *graph_id)
                .collect()
        };

        let mut seen = HashSet::new();
        let mut queue: VecDeque<GraphId> = entry_ids.into_iter().collect();

        while let Some(graph_id) = queue.pop_front() {
            if !seen.insert(graph_id) {
                continue;
            }

            if self.get_graph(&graph_id).is_none() {
                self.load_graph_from_current_project(&graph_id)?;
            }

            let Some(graph) = self.get_graph(&graph_id) else {
                continue;
            };

            for (_, target_id) in call_site_pairs_from_graph(&graph) {
                queue.push_back(target_id);
            }
        }

        Ok(())
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
    use crate::graph::core::{default_function_exec_inputs, default_function_exec_outputs};

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

        let (updated, _change_sets) = state
            .update_function_signature(&target.id, Some(vec![signature_pin("input")]), None)
            .expect("function signature should update");

        assert_eq!(updated.function_inputs, vec![signature_pin("input")]);
        assert_eq!(updated.function_outputs, default_function_exec_outputs());

        let other_graph = state
            .get_graph(&other.id)
            .expect("other function should exist");
        assert_eq!(other_graph.function_inputs, default_function_exec_inputs());
        assert_eq!(other_graph.function_outputs, default_function_exec_outputs());
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
            state.get_graph(&first.id).expect("peer should exist").name,
            "Event A"
        );
    }
}
