use super::ProjectError;
use super::ProjectMetadata;
use crate::database::DatabaseDecl;
use crate::graph::{GraphId, GraphInstance};
use crate::variable::{VariableId, VariableInstance};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectData {
    pub variables: HashMap<VariableId, VariableInstance>,
    pub graphs: HashMap<GraphId, GraphInstance>,
    pub databases: HashMap<String, DatabaseDecl>,
    pub metadata: ProjectMetadata,
}

impl ProjectData {
    /// 创建新的空项目数据
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            graphs: HashMap::new(),
            databases: HashMap::new(),
            metadata: ProjectMetadata::default(),
        }
    }

    /// 获取项目信息摘要
    pub fn info(&self) -> String {
        format!(
            "variables={}, databases={}, graphs={}",
            self.variables.len(),
            self.databases.len(),
            self.graphs.len()
        )
    }

    pub fn to_json(&self) -> Result<String, ProjectError> {
        serde_json::to_string_pretty(self).map_err(ProjectError::Serialize)
    }

    pub fn from_json(json: &str) -> Result<Self, ProjectError> {
        serde_json::from_str(json).map_err(ProjectError::Deserialize)
    }

    pub fn update_metadata(&mut self) {
        self.metadata.export_time = chrono::Utc::now().to_rfc3339();
    }
}
