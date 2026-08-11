use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::database::{DatabaseDecl, DatabaseEngine};
use crate::project::{GraphResourcePath, WorksheetResourcePath};

use super::{ProjectError, ProjectState};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum RevealProjectResourceRequest {
    Graph {
        graph_path: String,
    },
    Database {
        database_id: String,
    },
    Worksheet {
        worksheet_path: WorksheetResourcePath,
    },
}

impl RevealProjectResourceRequest {
    pub fn from_parts(kind: &str, resource_id: String) -> Result<Self, String> {
        match kind {
            "graph" => Ok(Self::Graph {
                graph_path: resource_id,
            }),
            "database" => Ok(Self::Database {
                database_id: resource_id,
            }),
            "worksheet" => WorksheetResourcePath::parse(&resource_id)
                .map(|worksheet_path| Self::Worksheet { worksheet_path })
                .map_err(|error| error.to_string()),
            other => Err(format!("Unknown resource kind: {other}")),
        }
    }
}

pub fn resolve_reveal_path(
    state: &ProjectState,
    request: RevealProjectResourceRequest,
) -> Result<PathBuf, ProjectError> {
    let session = state
        .capture_project_session()
        .map_err(|error| ProjectError::InvalidProjectFormat(error.to_string()))?;
    let data = state
        .get_data()
        .map_err(|error| ProjectError::InvalidProjectFormat(error.to_string()))?;
    let _filesystem_lease = state
        .filesystem()
        .acquire(session.root.clone())
        .map_err(|error| ProjectError::InvalidProjectFormat(error.to_string()))?;
    state
        .validate_project_session(&session)
        .map_err(|error| ProjectError::InvalidProjectFormat(error.to_string()))?;
    let root = session.root.as_path();
    match request {
        RevealProjectResourceRequest::Graph { graph_path } => {
            absolute_path_for_graph(root, &graph_path)
        }
        RevealProjectResourceRequest::Database { database_id } => {
            absolute_path_for_database(root, &data.databases, &database_id)
        }
        RevealProjectResourceRequest::Worksheet { worksheet_path } => data
            .worksheets
            .contains_key(&worksheet_path)
            .then(|| root.join(worksheet_path.relative_path()))
            .ok_or_else(|| {
                ProjectError::InvalidProjectFormat(format!(
                    "Worksheet '{}' not found",
                    worksheet_path.as_str()
                ))
            }),
    }
}

pub fn absolute_path_for_graph(root: &Path, graph_path: &str) -> Result<PathBuf, ProjectError> {
    let graph_path = GraphResourcePath::new(graph_path)?;
    super::find_graph_document_path(root, &graph_path)?
        .map(|(path, _, _)| path)
        .ok_or_else(|| {
            ProjectError::InvalidProjectFormat(format!("Graph '{graph_path}' not found"))
        })
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
