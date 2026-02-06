//! 项目管理模块

pub mod project_data;
pub mod project_error;
pub mod project_metadata;
pub mod project_state;
pub mod project_store;

pub use project_data::*;
pub use project_error::*;
pub use project_metadata::*;
pub use project_state::*;
pub use project_store::*;
use std::path::PathBuf;

// ==================== 文件操作 ====================

/// 保存项目到文件
pub fn save_project_to_file(project_data: &ProjectData, path: &str) -> Result<(), ProjectError> {
    let json = project_data.to_json()?;
    std::fs::write(path, json)?;
    Ok(())
}

/// 从文件加载项目
pub fn load_project_from_file(path: &str) -> Result<ProjectData, ProjectError> {
    let path = PathBuf::from(path);

    if !path.exists() {
        return Err(ProjectError::FileNotFound(path));
    }

    let content = std::fs::read_to_string(&path)?;
    ProjectData::from_json(&content)
}
