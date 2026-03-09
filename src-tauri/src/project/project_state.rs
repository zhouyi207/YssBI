//! 状态管理模块

use crate::database::{dataframe_to_schema, DatabaseInstance, DatabaseState};
use crate::graph::core::SchemaProvider;
use crate::log::log_sys;
use crate::project::{ProjectData, ProjectStore};
use std::sync::{Arc, RwLock};

/// 项目状态
/// 
/// 是不需要 序列化的
#[derive(Default)]
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

    /// 设置 项目数据 并清空 项目存储
    /// 方案一：从 project_data.databases 重建 project_store.databases，恢复 schema 能力
    pub fn set_data(&self, project_data: ProjectData) {
        log_sys::info!("[ProjectState.set_data] ProjectData: {}", project_data.info());

        *self.project_data.write().unwrap() = project_data.clone();

        let mut store = ProjectStore::default();
        for (id, decl) in project_data.databases.iter() {
            let instance = match decl.engine.build_lazy() {
                Ok(lazy_frame) => {
                    log_sys::info!("[ProjectState.set_data] Rebuilt database '{}' (Lazy)", id);
                    DatabaseInstance {
                        decl: decl.clone(),
                        state: DatabaseState::Lazy { lazy_frame },
                    }
                }
                Err(e) => {
                    log_sys::warn!(
                        "[ProjectState.set_data] Database '{}' build_lazy failed: {}",
                        id,
                        e
                    );
                    DatabaseInstance {
                        decl: decl.clone(),
                        state: DatabaseState::Failed {
                            error: e.to_string(),
                        },
                    }
                }
            };
            store.databases.insert(id.clone(), instance);
        }
        *self.project_store.write().unwrap() = store;

        // 恢复所有图的 registry 和 schema provider
        self.restore_graph_registries();
        self.set_graph_schema_providers();
    }

    /// 恢复所有图的 registry（在加载项目后调用）
    fn restore_graph_registries(&self) {
        let registry = {
            let store = self.project_store.read().unwrap();
            Arc::clone(&store.node_register)
        };

        let mut project_data = self.project_data.write().unwrap();
        for graph in project_data.graphs.values_mut() {
            graph.set_registry(Arc::clone(&registry));
        }
    }

    /// 构建 SchemaProvider 闭包（捕获 project_store 的引用）
    pub fn build_schema_provider(&self) -> SchemaProvider {
        let store = Arc::clone(&self.project_store);
        Arc::new(move |dataframe_id: &str| {
            let mut store = store.write().ok()?;
            let db = store.databases.get_mut(dataframe_id)?;
            let df = db.ensure_loaded().ok()?;
            Some(dataframe_to_schema(df))
        })
    }

    /// 为所有图设置 schema provider 并传播 schema（在加载项目后调用）
    fn set_graph_schema_providers(&self) {
        let provider = self.build_schema_provider();
        let mut project_data = self.project_data.write().unwrap();
        for graph in project_data.graphs.values_mut() {
            graph.set_schema_provider(provider.clone());
            graph.propagate_schemas();
            let _ = graph.resolve_all_dynamic_pins();
        }
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
}
