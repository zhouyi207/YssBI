use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use uuid::Uuid;

use crate::database::{DatabaseDecl, DatabaseEngine};
use crate::graph::GraphId;

use super::{ProjectError, ProjectState, project_root_from_path, worksheet_absolute_path};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum RevealProjectResourceRequest {
    Graph { graph_id: String },
    Database { database_id: String },
    Worksheet { worksheet_id: String },
}

impl RevealProjectResourceRequest {
    pub fn from_parts(kind: &str, resource_id: String) -> Result<Self, String> {
        match kind {
            "graph" => Ok(Self::Graph {
                graph_id: resource_id,
            }),
            "database" => Ok(Self::Database {
                database_id: resource_id,
            }),
            "worksheet" => Ok(Self::Worksheet {
                worksheet_id: resource_id,
            }),
            other => Err(format!("Unknown resource kind: {other}")),
        }
    }
}

pub fn resolve_reveal_path(
    state: &ProjectState,
    request: RevealProjectResourceRequest,
) -> Result<PathBuf, ProjectError> {
    let project_path = state
        .get_path()
        .ok_or_else(|| ProjectError::InvalidProjectFormat("No project is open".into()))?;
    let root = project_root_from_path(&project_path);

    match request {
        RevealProjectResourceRequest::Graph { graph_id } => {
            absolute_path_for_graph(root.as_path(), &graph_id)
        }
        RevealProjectResourceRequest::Database { database_id } => {
            let databases = state.get_data().databases;
            absolute_path_for_database(root.as_path(), &databases, &database_id)
        }
        RevealProjectResourceRequest::Worksheet { worksheet_id } => {
            worksheet_absolute_path(root.as_path(), &worksheet_id)?.ok_or_else(|| {
                ProjectError::InvalidProjectFormat(format!("Worksheet '{worksheet_id}' not found"))
            })
        }
    }
}

pub fn absolute_path_for_graph(root: &Path, graph_id: &str) -> Result<PathBuf, ProjectError> {
    let graph_id = parse_graph_id(graph_id)?;
    super::find_graph_document_path(root, &graph_id)?
        .map(|(path, _, _)| path)
        .ok_or_else(|| ProjectError::InvalidProjectFormat(format!("Graph '{graph_id}' not found")))
}

pub fn absolute_path_for_database(
    root: &Path,
    databases: &HashMap<String, DatabaseDecl>,
    database_id: &str,
) -> Result<PathBuf, ProjectError> {
    let decl = databases.get(database_id).ok_or_else(|| {
        ProjectError::InvalidProjectFormat(format!("Database '{database_id}' not found"))
    })?;

    let path = match &decl.engine {
        DatabaseEngine::DuckDb { path, .. } => root.join(path),
        DatabaseEngine::Csv { path, .. }
        | DatabaseEngine::Parquet { path, .. }
        | DatabaseEngine::Excel { path, .. } => {
            let file = Path::new(path);
            if file.is_absolute() {
                file.to_path_buf()
            } else {
                root.join(file)
            }
        }
        DatabaseEngine::Sql {
            connection_string, ..
        } => PathBuf::from(connection_string),
        DatabaseEngine::InMemory { .. } => {
            return Err(ProjectError::InvalidProjectFormat(
                "In-memory datasets have no file on disk".into(),
            ));
        }
    };

    Ok(path)
}

fn parse_graph_id(graph_id: &str) -> Result<GraphId, ProjectError> {
    Uuid::parse_str(graph_id).map(GraphId::from).map_err(|e| {
        ProjectError::InvalidProjectFormat(format!("Invalid graph id '{graph_id}': {e}"))
    })
}
