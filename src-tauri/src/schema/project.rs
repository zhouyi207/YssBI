use super::{GraphInstanceDTO, VariableDefinitionDTO, DatabaseDeclDTO};
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
    pub variables: HashMap<String, VariableDefinitionDTO>,
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
                .map(|(k, v)| (k.clone(), VariableDefinitionDTO::from(v)))
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
