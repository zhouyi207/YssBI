use std::path::PathBuf;

use super::{ProjectData, ProjectError};

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
