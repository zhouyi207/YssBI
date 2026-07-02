//! 状态管理模块

use crate::application::database::bind_duckdb_instance;
use crate::database::{DatabaseDecl, DatabaseEngine, DatabaseInstance, DatabaseState};
use crate::graph::core::SchemaProvider;
use crate::graph::value::DataType;
use crate::graph::{GraphId, GraphInstance, GraphKind, PinChangeSet, PinId};
use crate::tabular::is_variable_handle;
use crate::log::log_sys;
use crate::project::{
    GraphDocument, ProjectData, ProjectStore, load_project_graph_from_file,
    save_project_graph_to_file, save_project_to_file,
};
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
}

impl ProjectState {
    pub fn new() -> Self {
        Self {
            project_data: Arc::new(RwLock::new(ProjectData::new())),
            project_path: Arc::new(RwLock::new(None)),
            project_store: Arc::new(RwLock::new(ProjectStore::default())),
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
        let schema_provider = self.build_schema_provider();
        let seed_nodes: Vec<(GraphId, Vec<crate::graph::NodeId>)> = {
            let data = self.project_data.read().unwrap();
            data.graphs
                .iter()
                .filter_map(|(graph_id, graph)| {
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
                        Some((*graph_id, seeds))
                    }
                })
                .collect()
        };

        let mut data = self.project_data.write().unwrap();
        for (graph_id, seeds) in seed_nodes {
            let Some(graph) = data.graphs.get_mut(&graph_id) else {
                continue;
            };
            graph.set_schema_provider(schema_provider.clone());
            graph.compile_graph_from_seeds(&seeds);
        }
    }

    /// Bind the project's runtime context onto a graph: registry, schema
    /// provider and schema propagation. Idempotent.
    ///
    /// Schema-derived pin materialization is deferred until the graph tab is
    /// opened (`resolve_graph_dynamic_pins` command). See DESIGN_RULE §3.7.
    /// Always called by `insert_graph` — do not call from elsewhere.
    fn prepare_graph_runtime(&self, graph: &mut GraphInstance) {
        let registry = {
            let store = self.project_store.read().unwrap();
            Arc::clone(&store.node_register)
        };
        graph.set_registry(registry);
        graph.set_schema_provider(self.build_schema_provider());
        let variable_symbols = self.variable_symbols_for_graph(&graph.id, &graph.kind);
        graph.resolve_variable_nodes(&variable_symbols);
        let dataframe_symbols = self.dataframe_symbols();
        graph.resolve_dataframe_nodes(&dataframe_symbols);
        graph.propagate_schemas();
        let _ = graph.infer_types();
    }

    pub(crate) fn variable_symbols_for_graph(
        &self,
        graph_id: &GraphId,
        graph_kind: &GraphKind,
    ) -> HashMap<String, (String, DataType)> {
        let data = self.project_data.read().unwrap();
        Self::variable_symbols_from_variables(&data.variables, graph_id, graph_kind)
    }

    pub(crate) fn variable_symbols_from_variables(
        variables: &HashMap<VariableId, VariableInstance>,
        graph_id: &GraphId,
        graph_kind: &GraphKind,
    ) -> HashMap<String, (String, DataType)> {
        let graph_id = graph_id.to_string();
        variables
            .values()
            .filter(|variable| match (&variable.scope, graph_kind) {
                (VariableScope::Global, _) => true,
                (VariableScope::Event { event_id }, GraphKind::Event) => event_id == &graph_id,
                (VariableScope::Function { function_id }, GraphKind::Function) => {
                    function_id == &graph_id
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

    pub(crate) fn dataframe_symbols(&self) -> HashMap<String, String> {
        let data = self.project_data.read().unwrap();
        Self::dataframe_symbols_from_databases(&data.databases)
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
    pub fn resolve_graph_dynamic_pins(
        &self,
        graph_id: &GraphId,
    ) -> Result<(GraphInstance, Vec<PinChangeSet>, Vec<(PinId, DataType)>), String> {
        let graph = self
            .get_graph(graph_id)
            .ok_or_else(|| format!("Graph '{}' not found", graph_id))?;
        let (change_sets, inferred) = graph.materialize_dynamic_pins();
        Ok((graph, change_sets, inferred))
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
        let graph_id = graph.id;
        self.project_data
            .write()
            .unwrap()
            .graphs
            .insert(graph_id, graph.clone());
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
        graph_id: &GraphId,
    ) -> Result<GraphDocument, String> {
        if let Some(existing) = self.get_graph(graph_id) {
            let graph = self.insert_graph(existing);
            return Ok(GraphDocument {
                schema_version: crate::project::SCHEMA_VERSION,
                kind: (&graph.kind).into(),
                graph,
                local_variables: HashMap::new(),
            });
        }
        let path = self.get_path().ok_or_else(|| "项目尚未加载".to_string())?;
        let document = load_project_graph_from_file(&path, graph_id).map_err(|e| e.to_string())?;
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

    pub fn persist_loaded_graph(&self, graph_id: &GraphId) -> Result<(), String> {
        let Some(path) = self.get_path() else {
            return Err("项目尚未加载".to_string());
        };
        let snapshot = {
            let mut data = self.project_data.write().unwrap();
            data.update_metadata();
            data.clone()
        };
        save_project_graph_to_file(&snapshot, &path, graph_id).map_err(|e| e.to_string())
    }
}
