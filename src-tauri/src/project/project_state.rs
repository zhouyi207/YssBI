//! 状态管理模块

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
    pub fn set_data(&self, project_data: ProjectData) {
        log_sys::info!("[ProjectState.set_data] ProjectData: {}", project_data.info());

        *self.project_data.write().unwrap() = project_data;
        *self.project_store.write().unwrap() = ProjectStore::default();
        
        // 恢复所有图的 registry
        self.restore_graph_registries();
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
