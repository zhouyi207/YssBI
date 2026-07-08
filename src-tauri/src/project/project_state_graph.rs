use super::ProjectState;
use super::unique_name;
use crate::graph::{
    FunctionSignaturePin, GraphInstance, GraphKind, GraphRecompileScope, NodeInstanceParams,
    PinChangeSet,
};
use crate::graph::register::value::call::CALL_FUNCTION_NODE_TYPE;
use crate::project::{
    FunctionSignatureEntry, FunctionSignatureTable, GraphDocumentKind, GraphResourcePath,
    call_site_pairs_from_graph, read_function_signature_header_from_project,
    read_graph_call_sites_from_project, read_project_index,
};
use crate::variable::VariableScope;
use std::collections::HashMap;
use std::sync::Arc;

impl ProjectState {
    fn graph_resource_dir(kind: &GraphKind) -> &'static str {
        match kind {
            GraphKind::Event => crate::project::EVENTS_DIR,
            GraphKind::Function => crate::project::FUNCTIONS_DIR,
        }
    }

    fn graph_resource_extension(kind: &GraphKind) -> &'static str {
        match kind {
            GraphKind::Event => crate::project::EVENT_EXTENSION,
            GraphKind::Function => crate::project::FUNCTION_EXTENSION,
        }
    }

    fn sanitize_graph_file_stem(name: &str) -> String {
        let sanitized: String = name
            .trim()
            .chars()
            .map(|ch| {
                if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
                {
                    '_'
                } else {
                    ch
                }
            })
            .collect();
        let sanitized = sanitized.trim_matches([' ', '.']).trim();
        if sanitized.is_empty() {
            "Untitled".to_string()
        } else {
            sanitized.to_string()
        }
    }

    fn allocate_new_graph_path(
        &self,
        graph_kind: &GraphKind,
        graph_name: &str,
    ) -> Result<GraphResourcePath, String> {
        let dir = Self::graph_resource_dir(graph_kind);
        let extension = Self::graph_resource_extension(graph_kind);
        let stem = Self::sanitize_graph_file_stem(graph_name);
        let mut used_paths: std::collections::HashSet<String> = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .keys()
            .map(|path| path.as_str().to_string())
            .collect();
        if let Some(path) = self.get_path() {
            if let Ok(index) = read_project_index(&path) {
                used_paths.extend(index.graphs.into_iter().map(|entry| entry.path));
            }
        }
        for index in 0.. {
            let file_name = if index == 0 {
                format!("{stem}.{extension}")
            } else {
                format!("{stem} {index}.{extension}")
            };
            let candidate = format!("{dir}/{file_name}");
            if !used_paths.contains(&candidate) {
                return GraphResourcePath::new(candidate).map_err(|e| e.to_string());
            }
        }
        unreachable!("allocate_new_graph_path loop should return")
    }

    fn allocate_untitled_graph_path(
        &self,
        graph_kind: &GraphKind,
    ) -> Result<GraphResourcePath, String> {
        let kind_str = match graph_kind {
            GraphKind::Event => "event",
            GraphKind::Function => "function",
        };
        let prefix = format!("untitled:{kind_str}:");
        let used_labels: Vec<String> = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .keys()
            .filter_map(|path| {
                let s = path.as_str();
                if s.starts_with(&prefix) {
                    Some(s[prefix.len()..].to_string())
                } else {
                    None
                }
            })
            .collect();
        let label = unique_name::unique_untitled_label(used_labels);
        GraphResourcePath::new(format!("{prefix}{label}")).map_err(|e| e.to_string())
    }

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
        let resource_path = self
            .allocate_new_graph_path(&graph_kind, &unique_graph_name)
            .expect("failed to allocate graph resource path");
        let graph_data = GraphInstance::new_with_path(
            &unique_graph_name,
            graph_kind,
            graph_register,
            resource_path,
        );
        // Funnel through the single `insert_graph` entry point so registry +
        // schema provider + schema propagation are bound consistently with the
        // load / duplicate / import paths.
        self.insert_graph(graph_data)
    }

    /// Create an in-memory draft graph (`untitled:{kind}:{label}`) without writing to disk.
    pub fn add_draft_graph_with_existing_names(
        &self,
        graph_name: &str,
        graph_kind: GraphKind,
        existing_names: Vec<String>,
    ) -> GraphInstance {
        let graph_register = {
            let store = self.project_store.read().unwrap();
            Arc::clone(&store.node_register)
        };
        let resource_path = self
            .allocate_untitled_graph_path(&graph_kind)
            .expect("failed to allocate untitled graph resource path");
        let base_name = if graph_name.trim().is_empty() {
            resource_path.display_name().to_string()
        } else {
            graph_name.to_string()
        };
        let unique_graph_name = {
            let project_data = self.project_data.read().unwrap();
            let mut existing: Vec<String> = project_data
                .graphs
                .values()
                .filter(|g| g.kind == graph_kind)
                .map(|g| g.name.clone())
                .collect();
            existing.extend(existing_names);
            unique_name::unique_name(&base_name, existing)
        };

        let graph_data = GraphInstance::new_with_path(
            &unique_graph_name,
            graph_kind,
            graph_register,
            resource_path,
        );
        self.insert_graph(graph_data)
    }

    pub fn add_graph(&self, graph_name: &str, graph_kind: GraphKind) -> GraphInstance {
        self.add_graph_with_existing_names(graph_name, graph_kind, Vec::new())
    }

    pub fn remove_graph(&self, graph_path: &GraphResourcePath) -> Option<GraphInstance> {
        let removed = self.project_data.write().unwrap().graphs.remove(graph_path);
        if removed.as_ref().is_some_and(|g| g.kind == GraphKind::Function) {
            self.function_signatures().write().unwrap().remove(graph_path);
            self.function_call_sites()
                .write()
                .unwrap()
                .remove_function(graph_path.as_str());
        }
        self.function_call_sites()
            .write()
            .unwrap()
            .remove_caller(graph_path.as_str());
        removed
    }

    pub fn unload_graph(&self, graph_path: &GraphResourcePath) {
        let graph_path_string = graph_path.as_str().to_string();
        let mut data = self.project_data.write().unwrap();
        data.graphs.remove(graph_path);
        data.variables.retain(|_, variable| match &variable.scope {
            VariableScope::Global => true,
            VariableScope::Event { event_path } => event_path != &graph_path_string,
            VariableScope::Function { function_path } => function_path != &graph_path_string,
        });
        drop(data);
        self.refresh_call_sites_for_caller(graph_path);
    }

    pub fn get_graph(&self, graph_path: &GraphResourcePath) -> Option<GraphInstance> {
        self.project_data
            .read()
            .unwrap()
            .graphs
            .get(graph_path)
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
        function_path: &GraphResourcePath,
        inputs: Option<Vec<FunctionSignaturePin>>,
        outputs: Option<Vec<FunctionSignaturePin>>,
    ) -> Result<(GraphInstance, Vec<PinChangeSet>), String> {
        if self.get_graph(function_path).is_none() {
            self.load_graph_from_current_project(function_path)?;
        }

        let (graph, change_sets) = self.with_graph_mut(function_path, |mut ctx| {
            if ctx.graph_ref().kind != GraphKind::Function {
                return Err(format!("Graph '{}' is not a Function", function_path));
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
    pub fn call_function_target_path(
        node_type: &str,
        params: Option<&NodeInstanceParams>,
    ) -> Option<GraphResourcePath> {
        if node_type != CALL_FUNCTION_NODE_TYPE {
            return None;
        }
        params.and_then(|p| p.sub_graph_path()).and_then(|s| GraphResourcePath::new(s).ok())
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
            if let Ok(path) = GraphResourcePath::new(entry.path) {
                table.upsert(path, entry.function_inputs, entry.function_outputs);
            }
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
            let Ok(graph_path) = GraphResourcePath::new(entry.path.clone()) else {
                continue;
            };
            let stubs = read_graph_call_sites_from_project(&path, &graph_path)
                .map_err(|e| e.to_string())?;
            let pairs: Vec<_> = stubs
                .into_iter()
                .filter_map(|stub| {
                    stub.target_function_path
                        .map(|function_path| (stub.node_id, function_path))
                })
                .collect();
            call_index.replace_caller_from_pairs(graph_path.as_str().to_string(), pairs);
        }
        let data = self.project_data.read().unwrap();
        for graph in data.graphs.values() {
            call_index.replace_caller_from_pairs(
                graph.resource_path.as_str().to_string(),
                call_site_pairs_from_graph(graph),
            );
        }
        *self.function_call_sites().write().unwrap() = call_index;
        Ok(())
    }

    /// 刷新某 caller 图在索引中的 Call 条目：已加载图优先，否则读磁盘 stub。
    pub fn refresh_call_sites_for_caller(&self, caller_graph_path: &GraphResourcePath) {
        if let Some(graph) = self.get_graph(caller_graph_path) {
            let pairs = call_site_pairs_from_graph(&graph);
            self.function_call_sites()
                .write()
                .unwrap()
                .replace_caller_from_pairs(caller_graph_path.as_str().to_string(), pairs);
            return;
        }
        if let Some(path) = self.get_path() {
            if let Ok(stubs) = read_graph_call_sites_from_project(&path, caller_graph_path) {
                let pairs: Vec<_> = stubs
                    .into_iter()
                    .filter_map(|stub| {
                        stub.target_function_path
                            .map(|function_path| (stub.node_id, function_path))
                    })
                    .collect();
                self.function_call_sites()
                    .write()
                    .unwrap()
                    .replace_caller_from_pairs(caller_graph_path.as_str().to_string(), pairs);
                return;
            }
        }
        self.function_call_sites()
            .write()
            .unwrap()
            .remove_caller(caller_graph_path.as_str());
    }

    /// 节点创建后登记 Call 调用点（非 Call 节点为 no-op）。
    pub fn register_call_site_for_node(
        &self,
        caller_graph_path: &GraphResourcePath,
        call_node_id: crate::graph::NodeId,
        node_type: &str,
        params: Option<&NodeInstanceParams>,
    ) {
        let Some(function_path) = Self::call_function_target_path(node_type, params) else {
            return;
        };
        self.function_call_sites()
            .write()
            .unwrap()
            .register(
                caller_graph_path.as_str().to_string(),
                call_node_id,
                function_path.as_str().to_string(),
            );
    }

    /// 节点删除前移除 Call 调用点（非 Call 节点为 no-op）。
    pub fn unregister_call_site_for_node(
        &self,
        caller_graph_path: &GraphResourcePath,
        call_node_id: crate::graph::NodeId,
        node_type: &str,
    ) {
        if node_type != CALL_FUNCTION_NODE_TYPE {
            return;
        }
        self.function_call_sites()
            .write()
            .unwrap()
            .unregister_node(caller_graph_path.as_str(), call_node_id);
    }

    fn cache_function_signature(
        &self,
        function_path: &GraphResourcePath,
        entry: &FunctionSignatureEntry,
    ) {
        self.function_signatures()
            .write()
            .unwrap()
            .upsert(
                function_path.clone(),
                entry.inputs.clone(),
                entry.outputs.clone(),
            );
    }

    /// 解析函数签名：已加载图 > 签名表 > 图文件头（不加载整图）。
    pub fn get_function_signature(
        &self,
        function_path: &GraphResourcePath,
    ) -> Result<FunctionSignatureEntry, String> {
        if let Some(graph) = self.get_graph(function_path) {
            if graph.kind == GraphKind::Function {
                let entry = FunctionSignatureEntry {
                    inputs: graph.function_inputs.clone(),
                    outputs: graph.function_outputs.clone(),
                };
                self.cache_function_signature(function_path, &entry);
                return Ok(entry);
            }
        }
        if let Some(entry) = self
            .function_signatures()
            .read()
            .unwrap()
            .get_cloned(function_path)
        {
            return Ok(entry);
        }
        let Some(path) = self.get_path() else {
            return Err(format!("Function signature for '{}' not found", function_path));
        };
        let entry = read_function_signature_header_from_project(&path, function_path)
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Function signature for '{}' not found", function_path))?;
        self.cache_function_signature(function_path, &entry);
        Ok(entry)
    }

    /// Call Function：锁外解析目标签名（不加载整图）。
    pub fn resolve_call_projection_signature(
        &self,
        node_type: &str,
        params: Option<&NodeInstanceParams>,
    ) -> Result<Option<FunctionSignatureEntry>, String> {
        let Some(target_path) = Self::call_function_target_path(node_type, params) else {
            return Ok(None);
        };
        Ok(Some(self.get_function_signature(&target_path)?))
    }

    /// 节点创建后：若为 Call Function，将目标函数签名投影到该节点 pin。
    pub fn project_call_node_pins(
        &self,
        caller_graph_path: &GraphResourcePath,
        node_id: crate::graph::NodeId,
        node_type: &str,
        params: Option<&NodeInstanceParams>,
    ) -> Result<Option<(GraphInstance, PinChangeSet)>, String> {
        let Some(target_path) = Self::call_function_target_path(node_type, params) else {
            return Ok(None);
        };
        let out = self.sync_call_node(caller_graph_path, node_id, &target_path)?;
        Ok(Some(out))
    }

    /// 依据目标函数签名，同步某个调用方图内单个 Call Function 节点的 pin（不加载目标整图）。
    pub fn sync_call_node(
        &self,
        caller_graph_path: &GraphResourcePath,
        call_node_id: crate::graph::NodeId,
        target_function_path: &GraphResourcePath,
    ) -> Result<(GraphInstance, PinChangeSet), String> {
        let signature = self.get_function_signature(target_function_path)?;

        self.with_graph_mut(caller_graph_path, |ctx| {
            let change_set = ctx.graph_ref().sync_call_function_pins_from_signature(
                call_node_id,
                &signature.inputs,
                &signature.outputs,
                None,
            );
            ctx.recompile(GraphRecompileScope::InferOnly);
            Ok((ctx.graph_ref().clone(), change_set))
        })
    }

    /// 收集项目中所有引用 `function_path` 的 Call Function 节点（读内存索引）。
    pub fn get_function_call_sites(
        &self,
        function_path: &GraphResourcePath,
    ) -> Vec<(GraphResourcePath, Vec<crate::graph::NodeId>)> {
        self.function_call_sites()
            .read()
            .unwrap()
            .sites_for_function(function_path.as_str())
            .into_iter()
            .filter_map(|(path, node_ids)| GraphResourcePath::new(path).ok().map(|p| (p, node_ids)))
            .collect()
    }

    /// 删除函数前：移除所有 caller 图中的 Call Function 节点并持久化。
    pub fn purge_call_nodes_for_function(
        &self,
        function_path: &GraphResourcePath,
    ) -> Result<Vec<(GraphResourcePath, GraphInstance)>, String> {
        let sites = self.get_function_call_sites(function_path);
        if sites.is_empty() {
            return Ok(Vec::new());
        }

        let mut updated = Vec::new();
        for (caller_path, node_ids) in sites {
            if self.get_graph(&caller_path).is_none() {
                self.load_graph_from_current_project(&caller_path)?;
            }

            let graph = self.with_graph_mut(&caller_path, |mut ctx| {
                for node_id in &node_ids {
                    ctx.graph().remove_node_raw(*node_id)?;
                }
                ctx.recompile(GraphRecompileScope::InferOnly);
                Ok(ctx.graph_ref().clone())
            })?;
            self.refresh_call_sites_for_caller(&caller_path);
            let _ = self.persist_loaded_graph(&caller_path);
            updated.push((caller_path, graph));
        }
        Ok(updated)
    }

    /// 目标函数签名变化后，同步所有引用该函数的 Call 节点 pin（含未加载的调用方图）。
    /// 返回每个受影响图的 `(graph_path, graph, change_sets)` 供 command 层发事件。
    pub fn sync_call_nodes_for_function(
        &self,
        function_path: &GraphResourcePath,
    ) -> Vec<(GraphResourcePath, GraphInstance, Vec<PinChangeSet>)> {
        let Ok(signature) = self.get_function_signature(function_path) else {
            return Vec::new();
        };

        let callers = self.get_function_call_sites(function_path);
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
                        None,
                    ));
                }
                ctx.recompile(GraphRecompileScope::InferOnly);
                Ok((gid.clone(), ctx.graph_ref().clone(), sets))
            });
            if let Ok(entry) = result {
                if !was_loaded {
                    persist_unloaded.push(entry.0.clone());
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
    pub fn function_has_side_effect_nodes(&self, function_path: &GraphResourcePath) -> bool {
        let Some(graph) = self.get_graph(function_path) else {
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

    /// 打开 Function 图 Tab 时：将签名表投影到 Entry / Return 壳节点 pin。
    pub fn sync_function_shell_pins_in_graph(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Vec<PinChangeSet> {
        let is_function = self
            .get_graph(graph_path)
            .is_some_and(|g| g.kind == GraphKind::Function);
        if !is_function {
            return Vec::new();
        }
        self.with_graph_mut(graph_path, |ctx| {
            let sets = ctx.graph_ref().sync_function_shell_pins();
            ctx.recompile(GraphRecompileScope::InferOnly);
            Ok(sets)
        })
        .unwrap_or_default()
    }

    /// 重建某个图内所有 Call Function 节点的 pin（按各自目标函数的当前签名）。
    ///
    /// Call pin 是目标函数签名的**派生**投影：持久化仅作缓存。图加载 / tab 打开时调用本方法，
    /// 保证即使目标函数在本图 unload 期间改过签名，Call pin 也不会陈旧。
    /// 返回变更集供命令层发 `NodePinsUpdated` 事件。
    pub fn sync_all_call_nodes_in_graph(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Vec<PinChangeSet> {
        let calls: Vec<(crate::graph::NodeId, String)> = {
            let Some(graph) = self.get_graph(graph_path) else {
                return Vec::new();
            };
            call_site_pairs_from_graph(&graph)
        };

        if calls.is_empty() {
            return Vec::new();
        }

        let mut signatures: HashMap<String, FunctionSignatureEntry> = HashMap::new();
        for (_, target_id) in &calls {
            if signatures.contains_key(target_id) {
                continue;
            }
            let Ok(target_path) = GraphResourcePath::new(target_id.clone()) else {
                continue;
            };
            if let Ok(signature) = self.get_function_signature(&target_path) {
                signatures.insert(target_id.clone(), signature);
            }
        }

        let result = self.with_graph_mut(graph_path, |ctx| {
            let mut sets = Vec::new();
            for (call_node_id, target_id) in calls {
                let Some(signature) = signatures.get(&target_id) else {
                    continue;
                };
                sets.push(ctx.graph_ref().sync_call_function_pins_from_signature(
                    call_node_id,
                    &signature.inputs,
                    &signature.outputs,
                    None,
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
        graph_path: &GraphResourcePath,
        graph_kind: &GraphKind,
    ) -> Result<Vec<String>, String> {
        let mut existing: Vec<String> = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .values()
            .filter(|item| item.kind == *graph_kind && item.resource_path != *graph_path)
            .map(|item| item.name.clone())
            .collect();
        if let Some(path) = self.get_path() {
            let expected_kind = GraphDocumentKind::from(graph_kind);
            existing.extend(
                read_project_index(&path)
                    .map_err(|e| e.to_string())?
                    .graphs
                    .into_iter()
                    .filter(|item| item.graph_type == expected_kind && item.path != graph_path.as_str())
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
        target_graph_path: Option<GraphResourcePath>,
    ) -> Result<(), String> {
        use std::collections::{HashSet, VecDeque};

        let entry_paths: Vec<GraphResourcePath> = {
            let data = self.project_data.read().unwrap();
            data.graphs
                .iter()
                .filter(|(graph_path, graph)| {
                    graph.kind == GraphKind::Event
                        && target_graph_path
                            .as_ref()
                            .is_none_or(|target| *graph_path == target)
                })
                .map(|(graph_path, _)| graph_path.clone())
                .collect()
        };

        let mut seen = HashSet::new();
        let mut queue: VecDeque<GraphResourcePath> = entry_paths.into_iter().collect();

        while let Some(graph_path) = queue.pop_front() {
            if !seen.insert(graph_path.clone()) {
                continue;
            }

            self.ensure_graph_loaded_for_execution(&graph_path)?;

            let Some(graph) = self.get_graph(&graph_path) else {
                continue;
            };

            for (_, target_path) in call_site_pairs_from_graph(&graph) {
                if let Ok(path) = GraphResourcePath::new(target_path) {
                    queue.push_back(path);
                }
            }
        }

        Ok(())
    }

    fn ensure_graph_loaded_for_execution(&self, graph_path: &GraphResourcePath) -> Result<(), String> {
        if self.get_graph(graph_path).is_some() {
            return Ok(());
        }
        if self.load_graph_from_current_project(graph_path).is_ok() {
            return Ok(());
        }
        Err(format!(
            "invalid project format: graph '{}' not found in project graph files",
            graph_path
        ))
    }

    /// Re-key a loaded graph resource and cascade in-memory references.
    pub fn move_graph_resource_path(
        &self,
        from: &GraphResourcePath,
        to: &GraphResourcePath,
    ) -> Result<(), String> {
        if from.as_str() == to.as_str() {
            return Ok(());
        }
        let from_norm = crate::project::normalize_graph_resource_path(from.as_str());
        let to_norm = crate::project::normalize_graph_resource_path(to.as_str());

        let is_function = {
            let mut data = self.project_data.write().unwrap();
            let Some(mut graph) = data.graphs.remove(from) else {
                return Err(format!("Graph '{}' not loaded", from));
            };
            let is_function = graph.kind == GraphKind::Function;
            graph.resource_path = to.clone();
            data.graphs.insert(to.clone(), graph);

            for variable in data.variables.values_mut() {
                match &mut variable.scope {
                    VariableScope::Event { event_path }
                        if crate::project::normalize_graph_resource_path(event_path) == from_norm =>
                    {
                        *event_path = to_norm.clone();
                    }
                    VariableScope::Function { function_path }
                        if crate::project::normalize_graph_resource_path(function_path) == from_norm =>
                    {
                        *function_path = to_norm.clone();
                    }
                    _ => {}
                }
            }

            for graph in data.graphs.values_mut() {
                let mut data_state = graph.data_state.write().unwrap();
                for node in data_state.nodes.values_mut() {
                    if let NodeInstanceParams::SubGraph { sub_graph_path } = &mut node.instance_params
                    {
                        if crate::project::normalize_graph_resource_path(sub_graph_path) == from_norm
                        {
                            *sub_graph_path = to_norm.clone();
                        }
                    }
                }
            }
            is_function
        };

        if is_function {
            let mut signatures = self.function_signatures().write().unwrap();
            if let Some(entry) = signatures.get_cloned(from) {
                signatures.remove(from);
                signatures.upsert(to.clone(), entry.inputs, entry.outputs);
            }
        }

        self.rebuild_function_call_site_index()?;
        Ok(())
    }

    /// Rename a graph: unique name within kind, persist document, return final name + kind.
    pub fn rename_graph(
        &self,
        graph_path: &GraphResourcePath,
        new_name: &str,
    ) -> Result<(String, GraphKind, Option<GraphResourcePath>), String> {
        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return Err("Graph name cannot be empty".to_string());
        }

        if self.get_graph(graph_path).is_none() {
            self.load_graph_from_current_project(graph_path)?;
        }

        let graph_kind = self
            .project_data
            .read()
            .unwrap()
            .graphs
            .get(graph_path)
            .map(|graph| graph.kind.clone())
            .ok_or_else(|| format!("Graph '{}' not found", graph_path))?;

        let existing = self.collect_peer_graph_names(graph_path, &graph_kind)?;
        let final_name = unique_name::unique_name(trimmed, existing);

        self.with_graph_mut(graph_path, |mut ctx| {
            ctx.graph().name = final_name.clone();
            Ok(())
        })?;

        if self.get_path().is_some() {
            let moved_to = self.persist_loaded_graph(graph_path)?;
            return Ok((final_name, graph_kind, moved_to));
        }

        Ok((final_name, graph_kind, None))
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
            .update_function_signature(
                &target.resource_path,
                Some(vec![signature_pin("input")]),
                None,
            )
            .expect("function signature should update");

        assert_eq!(updated.function_inputs, vec![signature_pin("input")]);
        assert_eq!(updated.function_outputs, default_function_exec_outputs());

        let other_graph = state
            .get_graph(&other.resource_path)
            .expect("other function should exist");
        assert_eq!(other_graph.function_inputs, default_function_exec_inputs());
        assert_eq!(other_graph.function_outputs, default_function_exec_outputs());
    }

    #[test]
    fn update_function_signature_rejects_event_graphs() {
        let state = ProjectState::new();
        let event = state.add_event("Event");

        let result = state.update_function_signature(
            &event.resource_path,
            Some(vec![signature_pin("input")]),
            None,
        );

        assert!(result.is_err());
        let event_graph = state
            .get_graph(&event.resource_path)
            .expect("event should exist");
        assert!(event_graph.function_inputs.is_empty());
    }

    #[test]
    fn rename_graph_cascades_sub_graph_path_references() {
        use crate::graph::register::value::call::CALL_FUNCTION_NODE_TYPE;
        use crate::graph::NodeInstanceParams;

        let root = std::env::temp_dir().join(format!(
            "yssbi-rename-cascade-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let state = ProjectState::new();
        state.set_path(Some(root.to_string_lossy().to_string()));
        let function = state.add_function("Helper");
        let caller = state.add_event("Caller");
        let function_path = function.resource_path.clone();

        state
            .with_graph_mut(&caller.resource_path, |mut ctx| {
                ctx.graph().create_node_with_position(
                    CALL_FUNCTION_NODE_TYPE,
                    0.0,
                    0.0,
                    Some(NodeInstanceParams::SubGraph {
                        sub_graph_path: function_path.as_str().to_string(),
                    }),
                )
            })
            .expect("create call node");

        let (_, _, moved_to) = state
            .rename_graph(&function_path, "Renamed Helper")
            .expect("rename should succeed");
        let new_path = moved_to.expect("rename should move resource path");

        let caller_graph = state
            .get_graph(&caller.resource_path)
            .expect("caller should remain loaded");
        let updated = caller_graph
            .get_all_nodes()
            .into_iter()
            .find_map(|node| node.instance_params.sub_graph_path().map(|path| path.to_string()))
            .expect("call node should exist");
        assert_eq!(updated, new_path.as_str());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rename_graph_deduplicates_against_loaded_peer() {
        let state = ProjectState::new();
        let first = state.add_event("Event A");
        let second = state.add_event("Event B");

        let (final_name, _, _) = state
            .rename_graph(&second.resource_path, "Event A")
            .expect("rename should succeed");

        assert_eq!(final_name, "Event A 1");
        assert_eq!(
            state
                .get_graph(&second.resource_path)
                .expect("graph should exist")
                .name,
            "Event A 1"
        );
        assert_eq!(
            state
                .get_graph(&first.resource_path)
                .expect("peer should exist")
                .name,
            "Event A"
        );
    }

    #[test]
    fn draft_graph_first_save_migrates_to_disk_path() {
        use crate::project::EVENTS_DIR;

        let root = std::env::temp_dir().join(format!(
            "yssbi-draft-save-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let state = ProjectState::new();
        state.set_path(Some(root.to_string_lossy().to_string()));

        let draft = state.add_draft_graph_with_existing_names("New Event", GraphKind::Event, vec![]);
        let draft_path = draft.resource_path.clone();
        assert!(draft_path.as_str().starts_with("untitled:event:"));

        let moved_to = state
            .persist_loaded_graph(&draft_path)
            .expect("first save should persist draft")
            .expect("draft save should migrate path");
        assert!(!moved_to.as_str().starts_with("untitled:"));
        assert!(moved_to.as_str().starts_with(&format!("{EVENTS_DIR}/")));

        assert!(state.get_graph(&draft_path).is_none());
        assert!(state.get_graph(&moved_to).is_some());

        let disk_file = root.join(moved_to.as_str());
        assert!(disk_file.is_file());

        let index = read_project_index(root.to_string_lossy().as_ref()).unwrap();
        assert!(index.graphs.iter().any(|g| g.path == moved_to.as_str()));

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn save_project_to_directory_skips_untitled_drafts() {
        use crate::project::save_project_to_file;

        let root = std::env::temp_dir().join(format!(
            "yssbi-skip-untitled-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let state = ProjectState::new();
        let _draft = state.add_draft_graph_with_existing_names("Draft", GraphKind::Event, vec![]);
        let _persisted = state.add_event("Saved");

        save_project_to_file(&state.get_data(), root.to_string_lossy().as_ref()).unwrap();

        let index = read_project_index(root.to_string_lossy().as_ref()).unwrap();
        assert_eq!(index.graphs.len(), 1);
        assert_eq!(index.graphs[0].name, "Saved");

        let _ = std::fs::remove_dir_all(root);
    }
}
