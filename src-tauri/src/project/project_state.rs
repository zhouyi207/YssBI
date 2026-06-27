//! 状态管理模块

use crate::application::database::bind_duckdb_instance;
use crate::database::{DatabaseEngine, DatabaseInstance, DatabaseState};
use crate::graph::core::SchemaProvider;
use crate::graph::{DataType, GraphId, GraphInstance, PinChangeSet, PinId};
use crate::log::log_sys;
use crate::project::{
    GraphDocument, ProjectData, ProjectStore, load_project_graph_from_file,
    save_project_graph_to_file, save_project_to_file,
};
use crate::variable::VariableInstance;
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

        // Now re-insert every detached graph through the unified entry so they
        // get their runtime bindings consistently.
        for graph in detached_graphs {
            self.insert_graph(graph);
        }
    }

    /// 构建 SchemaProvider 闭包（捕获 project_store 的引用）
    pub fn build_schema_provider(&self) -> SchemaProvider {
        let store = Arc::clone(&self.project_store);
        Arc::new(move |dataframe_id: &str| {
            let mut store = store.write().ok()?;
            let db = store.databases.get_mut(dataframe_id)?;
            db.data_schema().ok()
        })
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
        graph.propagate_schemas();
        let _ = graph.infer_types();
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
        let inserted = self.insert_graph(graph);
        if !local_variables.is_empty() {
            self.project_data
                .write()
                .unwrap()
                .variables
                .extend(local_variables);
        }
        inserted
    }

    pub fn load_graph_from_current_project(
        &self,
        graph_id: &GraphId,
    ) -> Result<GraphDocument, String> {
        if let Some(existing) = self.get_graph(graph_id) {
            return Ok(GraphDocument {
                schema_version: crate::project::SCHEMA_VERSION,
                kind: (&existing.kind).into(),
                graph: existing,
                local_variables: HashMap::new(),
            });
        }
        let path = self.get_path().ok_or_else(|| "项目尚未加载".to_string())?;
        let document = load_project_graph_from_file(&path, graph_id).map_err(|e| e.to_string())?;
        self.insert_loaded_graph(document.graph.clone(), document.local_variables.clone());
        Ok(document)
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
