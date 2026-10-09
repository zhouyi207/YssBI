use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use yss_chart_document::{ChartResourcePath, ChartResourcePathError};
use yss_database_contract::{DatabaseDecl, DatabaseEngine};
use yss_graph_document::{GraphResourceKind, GraphResourcePath};

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
    kind: GraphResourceKind,
) -> Result<GraphResourcePath, RevealProjectResourceError> {
    let path =
        GraphResourcePath::new(value).map_err(|_| RevealProjectResourceError::InvalidFilePath)?;
    validate_node_path_kind(&path, kind)?;
    Ok(path)
}

fn validate_node_path_kind(
    path: &GraphResourcePath,
    kind: GraphResourceKind,
) -> Result<(), RevealProjectResourceError> {
    if path.kind() != kind {
        return Err(RevealProjectResourceError::InvalidFilePath);
    }
    Ok(())
}

fn reveal_error(error: impl ToString) -> ProjectError {
    ProjectError::InvalidProjectFormat(error.to_string())
}

pub fn resolve_reveal_path(
    state: &ProjectState,
    request: RevealProjectResourceRequest,
) -> Result<PathBuf, ProjectError> {
    let session = state.capture_project_session().map_err(reveal_error)?;
    let _filesystem_lease = state
        .filesystem()
        .acquire(session.root.clone())
        .map_err(reveal_error)?;
    state
        .validate_project_session(&session)
        .map_err(reveal_error)?;
    let root = session.root.as_path();
    let path = match request {
        RevealProjectResourceRequest::EventGraph { path } => {
            validate_node_path_kind(&path, GraphResourceKind::EventGraph).map_err(reveal_error)?;
            absolute_path_for_graph(root, path.as_str())
        }
        RevealProjectResourceRequest::FunctionGraph { path } => {
            validate_node_path_kind(&path, GraphResourceKind::FunctionGraph)
                .map_err(reveal_error)?;
            absolute_path_for_graph(root, path.as_str())
        }
        RevealProjectResourceRequest::Mind { path } => state
            .coherent_project_read(&session, |data, _| {
                data.minds
                    .contains_key(&path)
                    .then(|| root.join(path.as_str()))
                    .ok_or_else(|| ProjectError::InvalidProjectFormat("file not found".into()))
            })
            .map_err(reveal_error)?,
        RevealProjectResourceRequest::Doc { path } => state
            .coherent_project_read(&session, |data, _| {
                data.docs
                    .contains_key(&path)
                    .then(|| root.join(path.as_str()))
                    .ok_or_else(|| ProjectError::InvalidProjectFormat("file not found".into()))
            })
            .map_err(reveal_error)?,
        RevealProjectResourceRequest::Database { database_id } => state
            .coherent_project_read(&session, |data, _| {
                absolute_path_for_database(root, &data.databases, &database_id)
            })
            .map_err(reveal_error)?,
        RevealProjectResourceRequest::Chart { chart_path } => state
            .coherent_project_read(&session, |data, _| {
                data.charts
                    .contains_key(&chart_path)
                    .then(|| root.join(chart_path.relative_path()))
                    .ok_or_else(|| {
                        ProjectError::InvalidProjectFormat(format!(
                            "Chart '{}' not found",
                            chart_path.as_str()
                        ))
                    })
            })
            .map_err(reveal_error)?,
    }?;
    state
        .validate_project_session(&session)
        .map_err(reveal_error)?;
    Ok(path)
}

pub fn absolute_path_for_graph(root: &Path, graph_path: &str) -> Result<PathBuf, ProjectError> {
    let graph_path = GraphResourcePath::new(graph_path)?;
    super::scan_graph_resource_index(root)?
        .get_by_path(graph_path.as_str())
        .map(|entry| root.join(entry.path.as_str()))
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
    fn reveal_checks_the_graph_kind_of_direct_and_deserialized_requests() {
        use yss_graph_document::GraphResourceKind;
        use yss_project_model::{GraphResourceDocument, ProjectData};

        let event = GraphResourcePath::new("events/Event.yssbi-event").unwrap();
        let function = GraphResourcePath::new("functions/Function.yssbi-function").unwrap();
        let mut data = ProjectData::new();
        data.graphs.insert(
            event.clone(),
            GraphResourceDocument::new("Event", GraphResourceKind::EventGraph),
        );
        data.graphs.insert(
            function.clone(),
            GraphResourceDocument::new("Function", GraphResourceKind::FunctionGraph),
        );
        let fixture = crate::fixtures::TempProject::activate("reveal-graph-kind", data);
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        for (kind, path) in [("event_graph", &event), ("function_graph", &function)] {
            let request =
                RevealProjectResourceRequest::from_parts(kind, path.as_str().into()).unwrap();
            assert_eq!(
                resolve_reveal_path(state, request).unwrap(),
                session.root.as_path().join(path.as_str())
            );
        }
        for request in [
            RevealProjectResourceRequest::EventGraph {
                path: function.clone(),
            },
            RevealProjectResourceRequest::FunctionGraph {
                path: event.clone(),
            },
            serde_json::from_value(serde_json::json!({
                "kind": "event_graph", "path": function.as_str()
            }))
            .unwrap(),
            serde_json::from_value(serde_json::json!({
                "kind": "function_graph", "path": event.as_str()
            }))
            .unwrap(),
        ] {
            assert!(matches!(
                resolve_reveal_path(state, request),
                Err(ProjectError::InvalidProjectFormat(_))
            ));
        }
    }

    #[test]
    fn reveal_graph_path_does_not_decode_the_graph_body() {
        let fixture = crate::fixtures::TempProject::activate(
            "reveal-damaged-graph",
            yss_project_model::ProjectData::new(),
        );
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let path = GraphResourcePath::new("events/Damaged.yssbi-event").unwrap();
        let absolute = session.root.as_path().join(path.as_str());
        std::fs::write(&absolute, b"damaged graph body").unwrap();

        assert_eq!(
            absolute_path_for_graph(session.root.as_path(), path.as_str()).unwrap(),
            absolute
        );
        assert_eq!(
            resolve_reveal_path(state, RevealProjectResourceRequest::EventGraph { path }).unwrap(),
            absolute
        );
        assert!(state.get_data().unwrap().graphs.is_empty());
    }

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
