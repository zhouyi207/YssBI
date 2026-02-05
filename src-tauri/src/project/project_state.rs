//! 状态管理模块

use crate::log::LogLevel;
use crate::log_sys;
use crate::project::{ProjectData, ProjectStore};
use std::sync::{Arc, RwLock};

/// 项目状态
pub struct ProjectState {
    project_data: Arc<RwLock<ProjectData>>,
    project_path: Arc<RwLock<Option<String>>>,
    // 在这里可以存储数据库数据
    project_store: Arc<RwLock<ProjectStore>>,
}

impl Default for ProjectState {
    fn default() -> Self {
        Self::new()
    }
}

impl ProjectState {
    pub fn new() -> Self {
        Self {
            project_data: Arc::new(RwLock::new(ProjectData::new())),
            project_path: Arc::new(RwLock::new(None)),
            project_store: Arc::new(RwLock::new(ProjectStore::new())),
        }
    }

    /// 获取 项目数据 克隆
    pub fn get_data(&self) -> ProjectData {
        self.project_data.read().unwrap().clone()
    }

    /// 设置 项目数据 并
    pub fn set_data(&self, project_data: ProjectData) {
        log_sys!(
            LogLevel::Info,
            format!("[ProjectState] Setting data: {}", project_data.info())
        );

        *self.project_data.write().unwrap() = project_data;
        self.project_store.write().unwrap().clear();
    }

    pub fn set_project(&self, project_data: ProjectData, path: Option<String>) {
        *self.project_data.write().unwrap() = project_data;
        *self.project_path.write().unwrap() = path;
    }

    pub fn get_project(&self) -> ProjectData {
        self.project_data.read().unwrap().clone()
    }

    pub fn get_path(&self) -> Option<String> {
        self.project_path.read().unwrap().clone()
    }

    pub fn clear(&self) {
        *self.project_data.write().unwrap() = ProjectData::new();
        *self.project_path.write().unwrap() = None;
    }
}
