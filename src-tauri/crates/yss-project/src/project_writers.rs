use crate::ProjectOperationError;
use crate::manifest::ProjectManifest;
use crate::{GraphResourceFile, ProjectSession, ProjectState, ProjectTransactionContext};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use yss_chart_document::{ChartDocument, ChartResourcePath};
use yss_filesystem::{FilesystemTransaction, StagedFilesystemMutation};
use yss_graph_document::GraphResourcePath;
use yss_project_history::{ChartResourceKey, FunctionResourceKey, ResourceKey};
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::{OperationId, ResourceRevision};
use yss_project_layout::{CHART_EXTENSION, PROJECT_METADATA_FILE};
use yss_project_model::{ProjectData, ProjectDataPatch};
use yss_resource_naming::{ResourceName, allocate_unique_resource_name};

#[path = "project_writers/charts.rs"]
mod charts;

#[derive(Debug, Clone)]
pub struct ProjectResourceMutationFacts {
    operation_id: OperationId,
    project_instance_id: ProjectInstanceId,
    publication_revision: u64,
    moves: Box<[ProjectResourceMove]>,
    deltas: Box<[yss_project_history::ResourceDeltaEvent]>,
    projection_status: ProjectProjectionStatus,
}

impl ProjectResourceMutationFacts {
    pub(crate) fn new(
        operation_id: OperationId,
        project_instance_id: ProjectInstanceId,
        publication_revision: u64,
        moves: impl Into<Box<[ProjectResourceMove]>>,
        deltas: impl Into<Box<[yss_project_history::ResourceDeltaEvent]>>,
        projection_status: ProjectProjectionStatus,
    ) -> Self {
        Self {
            operation_id,
            project_instance_id,
            publication_revision,
            moves: moves.into(),
            deltas: deltas.into(),
            projection_status,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectResourceMove {
    pub from: Box<str>,
    pub to: Box<str>,
    pub kind: yss_project_history::ResourceLifecycleKind,
    pub name: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectProjectionStatus {
    Complete {
        expected_graph_paths: Box<[GraphResourcePath]>,
    },
    Incomplete {
        invalidated_graph_paths: Box<[GraphResourcePath]>,
    },
}

impl ProjectResourceMutationFacts {
    pub fn into_parts(self) -> ProjectResourceMutationParts {
        ProjectResourceMutationParts {
            operation_id: self.operation_id,
            project_instance_id: self.project_instance_id,
            publication_revision: self.publication_revision,
            moves: self.moves,
            deltas: self.deltas,
            projection_status: self.projection_status,
        }
    }
}

pub struct ProjectResourceMutationParts {
    pub operation_id: OperationId,
    pub project_instance_id: ProjectInstanceId,
    pub publication_revision: u64,
    pub moves: Box<[ProjectResourceMove]>,
    pub deltas: Box<[yss_project_history::ResourceDeltaEvent]>,
    pub projection_status: ProjectProjectionStatus,
}

pub(crate) struct WriterSnapshot {
    pub(crate) session: ProjectSession,
    pub(crate) data: ProjectData,
    pub(crate) graph_resource_revisions:
        std::collections::HashMap<GraphResourcePath, ResourceRevision>,
    pub(crate) chart_revisions: std::collections::HashMap<ChartResourcePath, ResourceRevision>,
    pub(crate) authority_generation: u64,
}

fn graph_key(path: &GraphResourcePath) -> ResourceKey {
    ResourceKey::Graph(path.clone())
}

fn function_key(path: &GraphResourcePath) -> ResourceKey {
    ResourceKey::Function(FunctionResourceKey(path.as_str().into()))
}

fn chart_key(path: &ChartResourcePath) -> ResourceKey {
    ResourceKey::Chart(ChartResourceKey(path.as_str().into()))
}

pub(crate) fn context(
    state: &ProjectState,
    session: ProjectSession,
    operation_id: OperationId,
    expected_revisions: BTreeMap<ResourceKey, ResourceRevision>,
    expected_absent_resources: BTreeSet<ResourceKey>,
) -> ProjectTransactionContext {
    ProjectTransactionContext {
        affected_resources: expected_revisions.keys().cloned().collect(),
        session,
        operation_id,
        expected_revisions,
        expected_absent_resources,
        recovery_marker: Some(state.project_recovery_marker()),
    }
}

fn prepare_error(error: impl ToString) -> ProjectOperationError {
    ProjectOperationError::TransactionPrepareFailed {
        message: error.to_string(),
    }
}

pub(crate) fn validate_document(path: &Path, contents: &[u8]) -> Result<(), String> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("yssbi-event" | "yssbi-function") => {
            serde_json::from_slice::<GraphResourceFile>(contents)
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        Some(CHART_EXTENSION) => serde_json::from_slice::<ChartDocument>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string()),
        _ if path == Path::new(PROJECT_METADATA_FILE) => {
            serde_json::from_slice::<ProjectManifest>(contents)
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        _ => Err(format!(
            "unsupported project document target '{}'",
            path.display()
        )),
    }
}

impl ProjectState {
    pub(crate) fn capture_writer_snapshot(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
    ) -> Result<WriterSnapshot, ProjectOperationError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectOperationError::StaleProjectLifecycle {
                message: "writer project instance is stale".into(),
            });
        }
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != session.instance_id.as_str() {
            return Err(ProjectOperationError::StaleProjectLifecycle {
                message: "project changed during writer snapshot".into(),
            });
        }
        let data = self.project_data.read().unwrap().clone();
        let graph_resource_revisions = self.graph_resource_revisions.read().unwrap().clone();
        let chart_revisions = self.chart_revisions.read().unwrap().clone();
        let snapshot = WriterSnapshot {
            session,
            data,
            graph_resource_revisions,
            chart_revisions,
            authority_generation: publication.authority_generation(),
        };
        drop(publication);
        Ok(snapshot)
    }

    pub(crate) fn validate_writer_context(
        &self,
        context: &ProjectTransactionContext,
        authority_generation: u64,
    ) -> Result<(), ProjectOperationError> {
        self.validate_project_session(&context.session)?;
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != context.session.instance_id.as_str()
            || publication.authority_generation() != authority_generation
        {
            return Err(ProjectOperationError::StaleProjectLifecycle {
                message: "project authority changed while writer was waiting".into(),
            });
        }
        let data = self.project_data.read().unwrap();
        let graph_resource_revisions = self.graph_resource_revisions.read().unwrap();
        let chart_revisions = self.chart_revisions.read().unwrap();
        super::project_state::validate_context_revisions(
            context,
            &data,
            &graph_resource_revisions,
            &chart_revisions,
        )?;
        drop(chart_revisions);
        drop(graph_resource_revisions);
        drop(data);
        drop(publication);
        for (resource, must_exist) in context
            .affected_resources
            .iter()
            .map(|resource| (resource, true))
            .chain(
                context
                    .expected_absent_resources
                    .iter()
                    .map(|resource| (resource, false)),
            )
        {
            let path = match resource {
                ResourceKey::Graph(path) => Path::new(path.as_str()),
                ResourceKey::Chart(path) => Path::new(path.0.as_ref()),
                _ => continue,
            };
            let present = match std::fs::symlink_metadata(context.session.root.as_path().join(path))
            {
                Ok(_) => true,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
                Err(error) => return Err(prepare_error(error)),
            };
            if present != must_exist {
                return Err(ProjectOperationError::ResourceRevisionConflict {
                    message: format!("resource presence changed for {resource:?}"),
                });
            }
        }
        Ok(())
    }

}
