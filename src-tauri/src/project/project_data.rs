use super::{
    GraphDocumentKind, GraphResourcePath, ProjectError, ProjectMetadata, WorksheetDocument,
    WorksheetResourcePath,
};
use crate::project::{FunctionDocument, FunctionSignature};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use yss_computation_settings::ProjectComputationSettings;
use yss_database_contract::DatabaseDecl;
use yss_graph_document::GraphDocument;
use yss_variable_contract::{VariableId, VariableInstance};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphResourceDocument {
    pub name: String,
    pub kind: GraphDocumentKind,
    pub document: GraphDocument,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<FunctionDocument>,
}

impl GraphResourceDocument {
    pub fn new(name: impl Into<String>, kind: GraphDocumentKind) -> Self {
        Self {
            name: name.into(),
            kind,
            document: GraphDocument::default(),
            function: matches!(kind, GraphDocumentKind::Function)
                .then(|| FunctionDocument::new(FunctionSignature::default())),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectData {
    #[serde(default)]
    pub computation_settings: ProjectComputationSettings,
    pub variables: HashMap<VariableId, VariableInstance>,
    pub graphs: HashMap<GraphResourcePath, GraphResourceDocument>,
    #[serde(default)]
    pub worksheets: HashMap<WorksheetResourcePath, WorksheetDocument>,
    pub databases: HashMap<String, DatabaseDecl>,
    pub metadata: ProjectMetadata,
}

impl ProjectData {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn info(&self) -> String {
        format!(
            "variables={}, databases={}, graphs={}, worksheets={}",
            self.variables.len(),
            self.databases.len(),
            self.graphs.len(),
            self.worksheets.len()
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
