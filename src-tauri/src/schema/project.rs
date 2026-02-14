use super::{DatabaseDeclDTO, GraphInstanceDTO, VariableDefinitionDTO};
use crate::graph::GraphId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMetadataDTO {
    #[serde(rename = "exportTime")]
    pub export_time: String,
    #[serde(rename = "appVersion")]
    pub app_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDataDTO {
    pub variables: HashMap<String, VariableDefinitionDTO>,
    pub graphs: HashMap<GraphId, GraphInstanceDTO>,
    pub databases: HashMap<String, DatabaseDeclDTO>,
    pub metadata: ProjectMetadataDTO,
}

