use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use yss_chart_document::{ChartResourcePath, ChartResourcePathError};
use yss_database_contract::{DatabaseDecl, DatabaseEngine};
use yss_graph_document::GraphResourcePath;

use super::{ProjectError, ProjectState};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum RevealProjectResourceRequest {
    #[serde(rename = "event_graph")]
    EventGraph {
        path: GraphResourcePath,
    },
    #[serde(rename = "function_graph")]
    FunctionGraph {
        path: GraphResourcePath,
    },
    Mind {
        path: yss_project_model::mind::MindPath,
    },
    Doc {
        path: yss_project_model::doc::DocPath,
    },
    Database {
        database_id: String,
    },
    Chart {
        chart_path: ChartResourcePath,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RevealProjectResourceError {
    #[error("unknown project resource kind '{kind}'")]
    UnknownKind { kind: Box<str> },
    #[error("invalid project file path")]
    InvalidFilePath,
    #[error("chart resource path is invalid")]
    InvalidChartPath(#[from] ChartResourcePathError),
}

impl RevealProjectResourceRequest {
    pub fn from_parts(kind: &str, resource_id: String) -> Result<Self, RevealProjectResourceError> {
        match kind {
            "event_graph" => Ok(Self::EventGraph {
                path: checked_node_path(
                    resource_id,
                    yss_graph_document::GraphResourceKind::EventGraph,
                )?,
            }),
            "function_graph" => Ok(Self::FunctionGraph {
                path: checked_node_path(
                    resource_id,
                    yss_graph_document::GraphResourceKind::FunctionGraph,
                )?,
            }),
            "mind" => Ok(Self::Mind {
                path: yss_project_model::mind::MindPath::parse(&resource_id)
                    .map_err(|_| RevealProjectResourceError::InvalidFilePath)?,
            }),
            "doc" => Ok(Self::Doc {
                path: yss_project_model::doc::DocPath::parse(&resource_id)
                    .map_err(|_| RevealProjectResourceError::InvalidFilePath)?,
            }),
            "database" => Ok(Self::Database {
                database_id: resource_id,
            }),
            "chart" => ChartResourcePath::parse(&resource_id)
                .map(|chart_path| Self::Chart { chart_path })
                .map_err(RevealProjectResourceError::from),
            other => Err(RevealProjectResourceError::UnknownKind { kind: other.into() }),
        }
    }
}

fn checked_node_path(
    value: String,
    kind: yss_graph_document::GraphResourceKind,
) -> Result<GraphResourcePath, RevealProjectResourceError> {
    let path =
        GraphResourcePath::new(value).map_err(|_| RevealProjectResourceError::InvalidFilePath)?;
    if path.kind() != kind {
        return Err(RevealProjectResourceError::InvalidFilePath);
    }
    Ok(path)
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
        RevealProjectResourceRequest::EventGraph { path }
        | RevealProjectResourceRequest::FunctionGraph { path } => {
            absolute_path_for_graph(root, path.as_str())
        }
        RevealProjectResourceRequest::Mind { path } => data
            .minds
            .contains_key(&path)
            .then(|| root.join(path.as_str()))
            .ok_or_else(|| ProjectError::InvalidProjectFormat("file not found".into())),
        RevealProjectResourceRequest::Doc { path } => data
            .docs
            .contains_key(&path)
            .then(|| root.join(path.as_str()))
            .ok_or_else(|| ProjectError::InvalidProjectFormat("file not found".into())),
        RevealProjectResourceRequest::Database { database_id } => {
            absolute_path_for_database(root, &data.databases, &database_id)
        }
        RevealProjectResourceRequest::Chart { chart_path } => data
            .charts
            .contains_key(&chart_path)
            .then(|| root.join(chart_path.relative_path()))
            .ok_or_else(|| {
                ProjectError::InvalidProjectFormat(format!(
                    "Chart '{}' not found",
                    chart_path.as_str()
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
        DatabaseEngine::Dataset {} => root
            .join(yss_project_layout::DATABASE_DIR)
            .join("datasets")
            .join(decl.id.as_str()),
    };

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reveal_request_rejects_invalid_parts_with_typed_errors() {
        assert!(matches!(
            RevealProjectResourceRequest::from_parts("unknown", "resource".into()),
            Err(RevealProjectResourceError::UnknownKind { kind }) if kind.as_ref() == "unknown"
        ));
        assert!(matches!(
            RevealProjectResourceRequest::from_parts("chart", "outside".into()),
            Err(RevealProjectResourceError::InvalidChartPath(
                ChartResourcePathError::WrongDirectory
            ))
        ));
    }
}
