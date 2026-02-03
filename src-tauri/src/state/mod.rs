//! 状态管理模块

use crate::project::ProjectDto;
use std::sync::RwLock;

/// 项目状态
pub struct ProjectState {
    current_project: RwLock<Option<ProjectDto>>,
    project_path: RwLock<Option<String>>,
}

impl ProjectState {
    pub fn new() -> Self {
        Self {
            current_project: RwLock::new(None),
            project_path: RwLock::new(None),
        }
    }

    pub fn set_project(&self, project: ProjectDto, path: Option<String>) {
        *self.current_project.write().unwrap() = Some(project);
        *self.project_path.write().unwrap() = path;
    }

    pub fn get_project(&self) -> Option<ProjectDto> {
        self.current_project.read().unwrap().clone()
    }

    pub fn get_path(&self) -> Option<String> {
        self.project_path.read().unwrap().clone()
    }

    pub fn clear(&self) {
        *self.current_project.write().unwrap() = None;
        *self.project_path.write().unwrap() = None;
    }
}

impl Default for ProjectState {
    fn default() -> Self {
        Self::new()
    }
}
