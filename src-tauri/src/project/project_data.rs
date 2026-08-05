use super::{
    GraphDocumentKind, GraphResourcePath, ProjectError, ProjectMetadata, WorksheetDocument,
};
use crate::database::DatabaseDecl;
use crate::node_system::document::{
    DocumentError, FunctionDocument, FunctionSignature, GraphDocument,
};
use crate::variable::{VariableId, VariableInstance};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

    pub fn validate(&self) -> Result<(), DocumentError> {
        self.document.validate()
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectData {
    pub variables: HashMap<VariableId, VariableInstance>,
    pub graphs: HashMap<GraphResourcePath, GraphResourceDocument>,
    #[serde(default)]
    pub worksheets: HashMap<String, WorksheetDocument>,
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
