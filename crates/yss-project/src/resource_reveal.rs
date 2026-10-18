use std::collections::HashMap;
use std::path::{Path, PathBuf};

use yss_database_contract::{DatabaseDecl, DatabaseEngine};
use yss_graph_document::{GraphResourceKind, GraphResourcePath};

use super::{ProjectError, ProjectState, file_resources::ResourceFile};
use yss_project_model::file::FilePath;

#[derive(Debug, Clone)]
pub enum RevealProjectResourceRequest {
    EventGraph {
        path: GraphResourcePath,
    },
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
        path: yss_project_model::chart::ChartPath,
    },
}

fn validate_node_path_kind(
    path: &GraphResourcePath,
    kind: GraphResourceKind,
) -> Result<(), ProjectError> {
    if path.kind() != kind {
        return Err(reveal_error(
            "graph path does not match the requested graph kind",
        ));
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
    let _filesystem_lease = state.filesystem().acquire(session.root.clone());
    state
        .validate_project_session(&session)
        .map_err(reveal_error)?;
    let root = session.root.as_path();
    let path = match request {
        RevealProjectResourceRequest::EventGraph { path } => {
            validate_node_path_kind(&path, GraphResourceKind::EventGraph)?;
            absolute_path_for_graph(root, path.as_str())
        }
        RevealProjectResourceRequest::FunctionGraph { path } => {
            validate_node_path_kind(&path, GraphResourceKind::FunctionGraph)?;
            absolute_path_for_graph(root, path.as_str())
        }
        RevealProjectResourceRequest::Mind { path } => {
            resident_file_path(state, &session, root, &path)?
        }
        RevealProjectResourceRequest::Doc { path } => {
            resident_file_path(state, &session, root, &path)?
        }
        RevealProjectResourceRequest::Chart { path } => {
            resident_file_path(state, &session, root, &path)?
        }
        RevealProjectResourceRequest::Database { database_id } => state
            .coherent_project_read(&session, |data, _| {
                absolute_path_for_database(root, &data.databases, &database_id)
            })
            .map_err(reveal_error)?,
    }?;
    state
        .validate_project_session(&session)
        .map_err(reveal_error)?;
    Ok(path)
}

fn resident_file_path<T: ResourceFile>(
    state: &ProjectState,
    session: &crate::ProjectSession,
    root: &Path,
    path: &FilePath<T>,
) -> Result<Result<PathBuf, ProjectError>, ProjectError> {
    state
        .coherent_project_read(session, |data, _| {
            T::files(data)
                .contains_key(path)
                .then(|| root.join(path.as_str()))
                .ok_or_else(|| ProjectError::InvalidProjectFormat("file not found".into()))
        })
        .map_err(reveal_error)
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
    fn reveal_checks_the_requested_graph_kind() {
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
        for (request, path) in [
            (
                RevealProjectResourceRequest::EventGraph {
                    path: event.clone(),
                },
                &event,
            ),
            (
                RevealProjectResourceRequest::FunctionGraph {
                    path: function.clone(),
                },
                &function,
            ),
        ] {
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
}
