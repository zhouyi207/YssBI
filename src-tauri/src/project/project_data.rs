use super::ProjectMetadata;
use crate::graph::{GraphData, GraphId};
use crate::variable::VariableDefinition;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProjectData {
    pub variables: HashMap<String, VariableDefinition>,
    pub graphs: HashMap<GraphId, GraphData>,
    pub metadata: ProjectMetadata,
}

impl ProjectData {
    /// 创建新的空项目数据
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            graphs: HashMap::new(),
            metadata: ProjectMetadata::default(),
        }
    }

    /// 获取项目信息摘要
    pub fn info(&self) -> String {
        format!(
            "variables={}, graphs={}",
            self.variables.len(),
            self.graphs.len()
        )
    }
}
