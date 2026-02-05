//! 状态管理模块

use crate::log::LogLevel;
use crate::log_sys;
use crate::project::{ProjectData, ProjectStore};
use std::sync::{Arc, RwLock};

/// 项目状态
#[derive(Default)]
pub struct ProjectState {
    project_data: Arc<RwLock<ProjectData>>,
    project_path: Arc<RwLock<Option<String>>>,
    // 在这里可以存储数据库数据
    project_store: Arc<RwLock<ProjectStore>>,
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
        log_sys!(
            LogLevel::Info,
            format!(
                "[ProjectState.set_data] ProjectData: {}",
                project_data.info()
            )
        );

        *self.project_data.write().unwrap() = project_data;
        *self.project_store.write().unwrap() = ProjectStore::default();
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
