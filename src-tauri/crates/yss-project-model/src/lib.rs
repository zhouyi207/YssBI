//! Canonical in-memory project aggregate.
//!
//! Persisted manifest, graph, chart, database, and variable wire formats
//! remain owned by their dedicated crates and Project I/O adapters. This model
//! deliberately does not read the clock or expose monolithic JSON persistence.

mod patch;

pub use patch::ProjectDataPatch;

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use yss_chart_document::{ChartDocument, ChartResourcePath};
use yss_database_contract::DatabaseDecl;
use yss_graph_document::{GraphDocument, GraphResourceKind, GraphResourcePath};
use yss_project_history::{FunctionDocument, FunctionSignature};
use yss_variable_contract::{VariableId, VariableInstance};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphResourceDocument {
    pub name: String,
    pub kind: GraphResourceKind,
    pub document: GraphDocument,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub function: Option<FunctionDocument>,
}

impl GraphResourceDocument {
    pub fn new(name: impl Into<String>, kind: GraphResourceKind) -> Self {
        Self {
            name: name.into(),
            kind,
            document: GraphDocument::default(),
            function: matches!(kind, GraphResourceKind::Function)
                .then(|| FunctionDocument::new(FunctionSignature::default())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectMetadata {
    pub project_name: String,
    pub export_time: String,
}

impl Default for ProjectMetadata {
    fn default() -> Self {
        Self {
            project_name: "未命名项目".to_owned(),
            export_time: String::new(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ProjectData {
    pub variables: HashMap<VariableId, VariableInstance>,
    pub graphs: HashMap<GraphResourcePath, GraphResourceDocument>,
    pub charts: HashMap<ChartResourcePath, ChartDocument>,
    pub databases: HashMap<String, DatabaseDecl>,
    pub metadata: ProjectMetadata,
}

impl ProjectData {
    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graph_constructor_matches_function_shape_to_kind() {
        let event = GraphResourceDocument::new("Event", GraphResourceKind::Event);
        let function = GraphResourceDocument::new("Function", GraphResourceKind::Function);

        assert!(event.function.is_none());
        assert!(function.function.is_some());
    }

    #[test]
    fn graph_history_payload_wire_keeps_kind_and_optional_function_stable() {
        let event = GraphResourceDocument::new("Event", GraphResourceKind::Event);
        let value = serde_json::to_value(&event).unwrap();

        assert_eq!(value["kind"], serde_json::json!("event"));
        assert!(value.get("function").is_none());
        assert_eq!(
            serde_json::from_value::<GraphResourceDocument>(value).unwrap(),
            event
        );
    }

    #[test]
    fn empty_project_is_deterministic_and_does_not_invent_export_time() {
        let project = ProjectData::new();

        assert_eq!(project.metadata.project_name, "未命名项目");
        assert!(project.metadata.export_time.is_empty());
        assert!(project.variables.is_empty());
        assert!(project.graphs.is_empty());
        assert!(project.charts.is_empty());
        assert!(project.databases.is_empty());
    }

    #[test]
    fn project_data_patch_keeps_typed_graph_identity_and_revision() {
        let path = GraphResourcePath::new("events/Main.yssbi-event").unwrap();
        let revision = yss_project_identity::ResourceRevision::new(7);
        let patch = ProjectDataPatch::DeclareGraph {
            path: path.clone(),
            revision,
        };

        match patch {
            ProjectDataPatch::DeclareGraph {
                path: actual_path,
                revision: actual_revision,
            } => {
                assert_eq!(actual_path, path);
                assert_eq!(actual_revision, revision);
            }
            _ => panic!("declare-graph patch changed variant"),
        }
    }
}
