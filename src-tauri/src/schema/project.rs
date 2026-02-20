use super::{GraphInstanceDTO, VariableInstanceDTO, DatabaseDeclDTO};
use crate::graph::GraphId;
use crate::project::{ProjectMetadata, ProjectData};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMetadataDTO {
    pub export_time: String,
    pub app_version: String,
}

impl From<&ProjectMetadata> for ProjectMetadataDTO {
    fn from(value: &ProjectMetadata) -> Self {
        Self {
            export_time: value.export_time.clone(),
            app_version: value.app_version.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDataDTO {
    pub variables: HashMap<String, VariableInstanceDTO>,
    pub graphs: HashMap<GraphId, GraphInstanceDTO>,
    pub databases: HashMap<String, DatabaseDeclDTO>,
    pub metadata: ProjectMetadataDTO,
}

impl From<&ProjectData> for ProjectDataDTO {
    fn from(value: &ProjectData) -> Self {
        Self {
            variables: value
                .variables
                .iter()
                .map(|(k, v)| (k.to_string(), VariableInstanceDTO::from(v)))
                .collect(),
            graphs: value
                .graphs
                .iter()
                .map(|(k, v)| (*k, GraphInstanceDTO::from(v)))
                .collect(),
            databases: value
                .databases
                .iter()
                .map(|(k, v)| (k.clone(), DatabaseDeclDTO::from(v)))
                .collect(),
            metadata: ProjectMetadataDTO::from(&value.metadata),
        }
    }
}

/// 分阶段加载：databases + variables（第一步）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabasesVariablesDTO {
    pub databases: HashMap<String, DatabaseDeclDTO>,
    pub variables: HashMap<String, VariableInstanceDTO>,
}

/// 分阶段加载：graphs + 引用校验结果（第二步）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphsWithValidationDTO {
    pub graphs: HashMap<GraphId, GraphInstanceDTO>,
    /// 每个 graph 的无效引用：nodeId -> { variableId?, dataframeId?, subGraphId? }
    pub invalid_references: HashMap<GraphId, Vec<InvalidReferenceDTO>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidReferenceDTO {
    pub node_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variable_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dataframe_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub_graph_id: Option<String>,
}
