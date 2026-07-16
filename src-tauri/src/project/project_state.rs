//! 状态管理模块

use super::function_call_site_index::FunctionCallSiteIndex;
use super::function_signature_table::FunctionSignatureTable;
use crate::application::database::bind_duckdb_instance;
use crate::database::{DatabaseDecl, DatabaseEngine, DatabaseInstance, DatabaseState};
use crate::graph::core::SchemaProvider;
use crate::graph::value::DataType;
use crate::graph::{GraphInstance, GraphKind, GraphRecompileScope, PinChangeSet, PinId};
use crate::log::log_sys;
use crate::project::{
    GraphDocument, GraphResourcePath, ProjectData, ProjectStore,
    cascade_graph_path_references_on_disk, load_project_graph_from_file, project_root_from_path,
    save_project_graph_to_file, save_project_to_file,
};
use crate::tabular::is_variable_handle;
use crate::variable::{VariableId, VariableInstance, VariableScope};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// 项目状态
///
/// 是不需要 序列化的
#[derive(Default, Clone)]
pub struct ProjectState {
    pub project_data: Arc<RwLock<ProjectData>>,
    pub project_path: Arc<RwLock<Option<String>>>,
    // 在这里可以存储 数据库 数据
    pub project_store: Arc<RwLock<ProjectStore>>,
    /// Bumped when project-wide graph runtime inputs change (e.g. database catalog).
    /// Compared against `GraphInstance::runtime_prepared_epoch` to skip redundant prepare.
    graph_runtime_epoch: Arc<RwLock<u64>>,
    /// 函数签名表（项目索引层缓存，与 `read_project_index` / 已加载函数图对齐）。
    function_signatures: Arc<RwLock<FunctionSignatureTable>>,
    /// Call Function 调用点反向索引（与磁盘 stub 扫描 / 已加载图对齐）。
    function_call_sites: Arc<RwLock<FunctionCallSiteIndex>>,
}

impl ProjectState {
    pub fn new() -> Self {
        Self {
            project_data: Arc::new(RwLock::new(ProjectData::new())),
            project_path: Arc::new(RwLock::new(None)),
            project_store: Arc::new(RwLock::new(ProjectStore::default())),
            graph_runtime_epoch: Arc::new(RwLock::new(1)),
            function_signatures: Arc::new(RwLock::new(FunctionSignatureTable::default())),
            function_call_sites: Arc::new(RwLock::new(FunctionCallSiteIndex::default())),
        }
    }

    pub(crate) fn function_signatures(&self) -> &Arc<RwLock<FunctionSignatureTable>> {
        &self.function_signatures
    }

    pub(crate) fn function_call_sites(&self) -> &Arc<RwLock<FunctionCallSiteIndex>> {
        &self.function_call_sites
    }

    fn graph_runtime_epoch(&self) -> u64 {
        *self.graph_runtime_epoch.read().unwrap()
    }

    fn mark_graph_runtime_prepared(&self, graph: &mut GraphInstance) {
        graph.runtime_prepared_epoch = self.graph_runtime_epoch();
    }

    fn graph_runtime_is_current(&self, graph: &GraphInstance) -> bool {
        graph.runtime_prepared_epoch == self.graph_runtime_epoch()
    }

    /// Invalidate cached graph runtime preparation (variable/dataframe symbol tables, etc.).
    pub fn invalidate_graph_runtime(&self) {
        let mut epoch = self.graph_runtime_epoch.write().unwrap();
        *epoch = epoch.saturating_add(1).max(1);
    }

    fn graph_document_from_instance(graph: GraphInstance) -> GraphDocument {
        GraphDocument {
            schema_version: crate::project::SCHEMA_VERSION,
            kind: (&graph.kind).into(),
            graph,
            local_variables: HashMap::new(),
        }
    }

    /// 获取 项目数据 克隆
    pub fn get_data(&self) -> ProjectData {
        self.project_data.read().unwrap().clone()
    }

    /// Replace the in-memory project with a freshly loaded `ProjectData`.
    ///
    /// Responsibilities:
    /// 1. Rebuild `project_store.databases` from the new declarations.
    /// 2. Re-bind every graph's runtime context (registry, schema provider,
    ///    schema propagation, dynamic pin resolution) via the single
    ///    `insert_graph` entry point — guaranteeing the runtime invariants
    ///    are maintained no matter how `ProjectData` got constructed.
    pub fn set_data(&self, project_data: ProjectData) {
        log_sys::info!(
            "[ProjectState.set_data] ProjectData: {}",
            project_data.info()
        );

        // Take graphs out so we can re-insert them through the single
        // `insert_graph` entry point. The fields written here (variables,
        // metadata, databases) need no extra preparation.
        let ProjectData {
            variables,
            mut graphs,
            databases,
            metadata,
        } = project_data;

        let detached_graphs: Vec<GraphInstance> = graphs.drain().map(|(_, g)| g).collect();

        // Reset the project_data first (without graphs) so subsequent
        // `insert_graph` calls see a clean state.
        {
            let mut data = self.project_data.write().unwrap();
            *data = ProjectData {
                variables,
                graphs: HashMap::new(),
                databases: databases.clone(),
                metadata,
            };
        }

        // Rebuild project_store.databases from DuckDB table declarations only.
        let project_root = self
            .get_path()
            .as_ref()
            .map(|path| crate::project::project_root_from_path(path));
        let mut store = ProjectStore::default();
        for (id, decl) in databases.iter() {
            let instance = if matches!(decl.engine, DatabaseEngine::DuckDb { .. }) {
                log_sys::info!("[ProjectState.set_data] Database '{}' bound (DuckDb)", id);
                bind_duckdb_instance(decl, project_root.as_deref())
            } else {
                log_sys::warn!(
                    "[ProjectState.set_data] Database '{}' has unsupported engine; expected DuckDb",
                    id
                );
                DatabaseInstance {
                    decl: decl.clone(),
                    state: DatabaseState::Failed {
                        error: "Only DuckDb datasets are supported; re-import the data".into(),
                    },
                }
            };
            store.databases.insert(id.clone(), instance);
        }
        *self.project_store.write().unwrap() = store;
        self.sync_all_variable_tabular();

        // Now re-insert every detached graph through the unified entry so they
        // get their runtime bindings consistently.
        for graph in detached_graphs {
            self.insert_graph(graph);
        }
    }

    /// 统一 tabular schema 查询：`var:{id}` 走变量缓存，其它 id 走数据集。
    pub fn build_schema_provider(&self) -> SchemaProvider {
        let store = Arc::clone(&self.project_store);
        Arc::new(move |tabular_id: &str| {
            if is_variable_handle(tabular_id) {
                let store = store.read().ok()?;
                return store
                    .variable_tabular
                    .get(tabular_id)
                    .map(|entry| entry.schema.clone());
            }
            let mut store = store.write().ok()?;
            let db = store.databases.get_mut(tabular_id)?;
            db.data_schema().ok()
        })
    }

    /// 变量变更后，重编译引用该变量的图（schema 传播 + schema 派生 pin 物化）。
    pub fn recompile_graphs_for_variable(&self, variable_id: &VariableId) {
        let var_id_str = variable_id.to_string();
        let seed_nodes: Vec<(GraphResourcePath, Vec<crate::graph::NodeId>)> = {
            let data = self.project_data.read().unwrap();
            data.graphs
                .iter()
                .filter_map(|(graph_path, graph)| {
                    let seeds: Vec<_> = graph
                        .data_state
                        .read()
                        .unwrap()
                        .nodes
                        .values()
                        .filter(|node| {
                            node.instance_params.variable_id() == Some(var_id_str.as_str())
                        })
                        .map(|node| node.id)
                        .collect();
                    if seeds.is_empty() {
                        None
                    } else {
                        Some((graph_path.clone(), seeds))
                    }
                })
                .collect()
        };

        let mut data = self.project_data.write().unwrap();
        for (graph_path, seeds) in seed_nodes {
            let graph_kind = match data.graphs.get(&graph_path) {
                Some(graph) => graph.kind.clone(),
                None => continue,
            };
            let variable_symbols =
                Self::variable_symbols_from_variables(&data.variables, &graph_path, &graph_kind);
            let dataframe_symbols = Self::dataframe_symbols_from_databases(&data.databases);
            let Some(graph) = data.graphs.get_mut(&graph_path) else {
                continue;
            };
            Self::bind_graph_runtime(graph, self);
            Self::apply_runtime_symbols(graph, &variable_symbols, &dataframe_symbols);
            graph.recompile(GraphRecompileScope::FromSeeds(seeds));
        }
    }

    /// Bind the project's runtime context onto a graph: registry, schema
    /// provider and schema propagation. Idempotent.
    ///
    /// Schema-derived pin materialization is deferred until the graph tab is
    /// opened (`resolve_graph_dynamic_pins` command). See DESIGN_RULE §3.7.
    /// Always called by `insert_graph` — do not call from elsewhere.
    fn prepare_graph_runtime(&self, graph: &mut GraphInstance) {
        Self::bind_graph_runtime(graph, self);
        let graph_path = graph.resource_path.clone();
        let graph_kind = graph.kind.clone();
        let data = self.project_data.read().unwrap();
        let variable_symbols =
            Self::variable_symbols_from_variables(&data.variables, &graph_path, &graph_kind);
        let dataframe_symbols = Self::dataframe_symbols_from_databases(&data.databases);
        drop(data);
        Self::apply_runtime_symbols(graph, &variable_symbols, &dataframe_symbols);
        graph.recompile(GraphRecompileScope::RuntimePrepare);
    }

    pub(crate) fn variable_symbols_from_variables(
        variables: &HashMap<VariableId, VariableInstance>,
        graph_path: &GraphResourcePath,
        graph_kind: &GraphKind,
    ) -> HashMap<String, (String, DataType)> {
        let graph_path = graph_path.as_str();
        variables
            .values()
            .filter(|variable| match (&variable.scope, graph_kind) {
                (VariableScope::Global, _) => true,
                (VariableScope::Event { event_path }, GraphKind::Event) => event_path == graph_path,
                (VariableScope::Function { function_path }, GraphKind::Function) => {
                    function_path == graph_path
                }
                _ => false,
            })
            .map(|variable| {
                (
                    variable.id.to_string(),
                    (variable.name.clone(), variable.data_type.clone()),
                )
            })
            .collect()
    }

    pub(crate) fn dataframe_symbols_from_databases(
        databases: &HashMap<String, DatabaseDecl>,
    ) -> HashMap<String, String> {
        databases
            .iter()
            .map(|(id, decl)| (id.clone(), decl.name.clone().unwrap_or_else(|| id.clone())))
            .collect()
    }

    /// Materialize schema-derived pins for a loaded graph (tab open path).
    ///
    /// 同时把本图内所有 Call Function 节点的 pin 按各自目标函数的当前签名重建，
    /// 消除「本图 unload 期间目标函数改过签名」导致的 Call pin 陈旧。
    pub fn resolve_graph_dynamic_pins(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Result<
        (
            GraphInstance,
            Vec<PinChangeSet>,
            Vec<(PinId, DataType)>,
            Vec<crate::graph::GraphValidationWarning>,
        ),
        String,
    > {
        let mut shell_sets = self.sync_function_shell_pins_in_graph(graph_path);
        let mut call_sets = self.sync_all_call_nodes_in_graph(graph_path);

        let graph = self
            .get_graph(graph_path)
            .ok_or_else(|| format!("Graph '{}' not found", graph_path))?;
        let result = graph.recompile(crate::graph::GraphRecompileScope::Materialize);
        let mut change_sets = result.change_sets;
        change_sets.append(&mut shell_sets);
        change_sets.append(&mut call_sets);
        Ok((
            graph,
            change_sets,
            result.inferred,
            result.inference_warnings,
        ))
    }

    /// Single entry point for placing a graph into `project_data.graphs`.
    ///
    /// Every code path that wants the project's authoritative copy of a graph
    /// (newly created, loaded from disk, duplicated, restored from a snapshot,
    /// imported, etc.) MUST go through this method. It enforces the runtime
    /// invariants (registry, schema provider, schema propagation) before the
    /// graph becomes visible to readers. Schema-derived pin materialization is
    /// deferred to `resolve_graph_dynamic_pins` when the graph tab is opened.
    pub fn insert_graph(&self, mut graph: GraphInstance) -> GraphInstance {
        self.prepare_graph_runtime(&mut graph);
        self.mark_graph_runtime_prepared(&mut graph);
        let graph_path = graph.resource_path.clone();
        self.project_data
            .write()
            .unwrap()
            .graphs
            .insert(graph_path.clone(), graph.clone());
        if graph.kind == GraphKind::Function {
            self.function_signatures()
                .write()
                .unwrap()
                .upsert_from_graph(&graph);
        }
        self.refresh_call_sites_for_caller(&graph_path);
        graph
    }

    /// Insert a graph that was loaded from disk along with its scoped
    /// (local) variables. Routes through `insert_graph` to keep runtime
    /// preparation centralized.
    pub fn insert_loaded_graph(
        &self,
        graph: GraphInstance,
        local_variables: HashMap<crate::variable::VariableId, VariableInstance>,
    ) -> GraphInstance {
        if !local_variables.is_empty() {
            self.project_data
                .write()
                .unwrap()
                .variables
                .extend(local_variables);
        }
        self.insert_graph(graph)
    }

    pub fn load_graph_from_current_project(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Result<GraphDocument, String> {
        if let Some(existing) = self.get_graph(graph_path) {
            if self.graph_runtime_is_current(&existing) {
                return Ok(Self::graph_document_from_instance(existing));
            }
            let graph = self.insert_graph(existing);
            return Ok(Self::graph_document_from_instance(graph));
        }
        let path = self.get_path().ok_or_else(|| "项目尚未加载".to_string())?;
        let document =
            load_project_graph_from_file(&path, graph_path).map_err(|e| e.to_string())?;
        let GraphDocument {
            schema_version,
            kind,
            graph,
            local_variables,
        } = document;
        let graph = self.insert_loaded_graph(graph, local_variables.clone());
        Ok(GraphDocument {
            schema_version,
            kind,
            graph,
            local_variables,
        })
    }

    /// 获取当前路径
    pub fn get_path(&self) -> Option<String> {
        self.project_path.read().unwrap().clone()
    }

    /// 设置当前路径
    pub fn set_path(&self, path: Option<String>) {
        *self.project_path.write().unwrap() = path;
    }

    /// 清空项目
    pub fn clear(&self) {
        *self.project_data.write().unwrap() = ProjectData::default();
        *self.project_path.write().unwrap() = None;
        *self.project_store.write().unwrap() = ProjectStore::default();
        *self.graph_runtime_epoch.write().unwrap() = 1;
        self.function_signatures().write().unwrap().clear();
        self.function_call_sites().write().unwrap().clear();
    }

    /// Reset in-memory project after disk load or save-as. Clears execution caches too.
    pub fn activate_loaded_snapshot(
        &self,
        source_store: &crate::execution::ResultSourceStore,
        path: String,
        project_data: ProjectData,
    ) {
        self.clear();
        source_store.clear_all();
        self.set_path(Some(path));
        self.set_data(project_data);
        if let Err(e) = self.rebuild_function_signature_table() {
            log_sys::warn!("function signature table rebuild failed: {}", e);
        }
        if let Err(e) = self.rebuild_function_call_site_index() {
            log_sys::warn!("function call site index rebuild failed: {}", e);
        }
    }

    pub fn persist_current_project(&self) -> Result<(), String> {
        let Some(path) = self.get_path() else {
            return Ok(());
        };
        let snapshot = {
            let mut data = self.project_data.write().unwrap();
            data.update_metadata();
            data.clone()
        };
        save_project_to_file(&snapshot, &path).map_err(|e| e.to_string())
    }

    pub fn persist_loaded_graph(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Result<Option<GraphResourcePath>, String> {
        let Some(path) = self.get_path() else {
            return Err("项目尚未加载".to_string());
        };
        let snapshot = {
            let mut data = self.project_data.write().unwrap();
            data.update_metadata();
            data.clone()
        };
        let saved_path =
            save_project_graph_to_file(&snapshot, &path, graph_path).map_err(|e| e.to_string())?;
        let new_path = GraphResourcePath::new(saved_path).map_err(|e| e.to_string())?;
        if new_path.as_str() == graph_path.as_str() {
            return Ok(None);
        }

        let root = project_root_from_path(&path);
        cascade_graph_path_references_on_disk(
            root.as_path(),
            graph_path.as_str(),
            new_path.as_str(),
            Some(root.join(new_path.as_str()).as_path()),
        )
        .map_err(|e| e.to_string())?;

        self.move_graph_resource_path(graph_path, &new_path)?;
        let _ = self.persist_loaded_graph(&new_path)?;
        Ok(Some(new_path))
    }
}

#[cfg(test)]
mod runtime_load_tests {
    use super::*;

    #[test]
    fn load_graph_skips_prepare_when_runtime_is_current() {
        let state = ProjectState::new();
        let inserted = state.add_event("Event A");
        let graph_path = inserted.resource_path.clone();
        let epoch_after_insert = inserted.runtime_prepared_epoch;

        let first = state
            .load_graph_from_current_project(&graph_path)
            .expect("first load should succeed");
        assert_eq!(first.graph.runtime_prepared_epoch, epoch_after_insert);

        let second = state
            .load_graph_from_current_project(&graph_path)
            .expect("second load should succeed");
        assert_eq!(second.graph.runtime_prepared_epoch, epoch_after_insert);
    }

    #[test]
    fn load_graph_reprepares_after_runtime_invalidation() {
        let state = ProjectState::new();
        let inserted = state.add_event("Event B");
        let graph_path = inserted.resource_path.clone();
        let epoch_after_insert = inserted.runtime_prepared_epoch;

        state
            .load_graph_from_current_project(&graph_path)
            .expect("warm load");

        state.invalidate_graph_runtime();
        let reloaded = state
            .load_graph_from_current_project(&graph_path)
            .expect("reload after invalidation");
        assert_ne!(reloaded.graph.runtime_prepared_epoch, epoch_after_insert);
        assert_eq!(
            reloaded.graph.runtime_prepared_epoch,
            state.graph_runtime_epoch()
        );
    }
}
