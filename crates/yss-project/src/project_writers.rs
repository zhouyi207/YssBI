use crate::ProjectOperationError;
use crate::manifest::ProjectManifest;
use crate::{ProjectSession, ProjectState, ProjectTransactionContext};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use yss_chart_document::{ChartDocument, ChartResourcePath};
use yss_filesystem::{FilesystemTransaction, StagedFilesystemMutation};
use yss_graph_document::GraphResourcePath;
use yss_project_history::{ChartResourceKey, ResourceKey};
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
            let relative = path.to_string_lossy().replace('\\', "/");
            let graph = GraphResourcePath::new(relative).map_err(|error| error.to_string())?;
            crate::project_io::parse_graph_resource_document(contents, path, graph.kind())
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        Some(CHART_EXTENSION) => serde_json::from_slice::<ChartDocument>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string()),
        Some(yss_project_layout::MIND_EXTENSION) => {
            use yss_project_model::file::FileContent;
            yss_project_model::mind::MindPath::parse(&path.to_string_lossy().replace('\\', "/"))?;
            yss_project_model::mind::MindDocument::decode(contents).map(|_| ())
        }
        Some(yss_project_layout::DOC_EXTENSION) => {
            use yss_project_model::file::FileContent;
            yss_project_model::doc::DocPath::parse(&path.to_string_lossy().replace('\\', "/"))?;
            yss_project_model::doc::DocDocument::decode(contents).map(|_| ())
        }
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
        let (session, authority_generation, (data, graph_resource_revisions, chart_revisions)) =
            self.capture_writer_input(expected_project_instance_id, |data| {
                (
                    data.clone(),
                    self.graph_resource_revisions.read().unwrap().clone(),
                    self.chart_revisions.read().unwrap().clone(),
                )
            })?;
        Ok(WriterSnapshot {
            session,
            data,
            graph_resource_revisions,
            chart_revisions,
            authority_generation,
        })
    }

    pub(crate) fn capture_writer_input<T>(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        capture: impl FnOnce(&ProjectData) -> T,
    ) -> Result<(ProjectSession, u64, T), ProjectOperationError> {
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
        let data = self.project_data.read().unwrap();
        Ok((session, publication.authority_generation(), capture(&data)))
    }

    pub(crate) fn validate_writer_authority(
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
        Ok(())
    }

    pub(crate) fn validate_writer_context(
        &self,
        context: &ProjectTransactionContext,
        authority_generation: u64,
    ) -> Result<(), ProjectOperationError> {
        self.validate_writer_authority(context, authority_generation)?;
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
                ResourceKey::Mind(path) => Path::new(path.0.as_ref()),
                ResourceKey::Doc(path) => Path::new(path.0.as_ref()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use yss_data_contract::{DataValue, ValueType};
    use yss_graph_document::{ConstantId, GraphConstant, GraphResourceKind};
    use yss_project_model::GraphResourceDocument;

    #[test]
    fn save_as_rejects_invalid_constants_in_an_unloaded_graph() {
        let fixture =
            crate::fixtures::TempProject::activate("copy-invalid-constant", ProjectData::new());
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let path = GraphResourcePath::new("events/Invalid.yssbi-event").unwrap();
        let mut graph = GraphResourceDocument::new("Invalid", GraphResourceKind::EventGraph);
        let id = ConstantId::new();
        std::sync::Arc::make_mut(&mut graph.document)
            .constants
            .insert(
                id,
                GraphConstant {
                    id,
                    name: "invalid_table".into(),
                    data_type: ValueType::DataFrame,
                    data_value: DataValue::String("not-json".into()),
                    tabular: None,
                    description: String::new(),
                    tags: vec![],
                },
            );
        let mut file_data = ProjectData::new();
        file_data.graphs.insert(path.clone(), graph);
        crate::fixtures::write_graph(&file_data, session.root.as_path().to_str().unwrap(), &path)
            .unwrap();
        assert!(!state.has_resident_graph(&path).unwrap());
        let source = session.root.as_path().join(path.as_str());
        let before = std::fs::read(&source).unwrap();
        let destination = session.root.as_path().parent().unwrap().join(format!(
            "yssbi-copy-invalid-destination-{}",
            uuid::Uuid::new_v4()
        ));
        let result = state.save_project_as_transaction(
            &session.instance_id,
            &destination,
            OperationId::new(),
        );
        let destination_survived = destination.exists();
        if destination_survived {
            std::fs::remove_dir_all(&destination).unwrap();
        }
        assert!(
            matches!(
                result,
                Err(ProjectOperationError::TransactionPrepareFailed { .. })
            ),
            "Save As must validate unloaded graph constants before copying them; got {result:?}"
        );
        assert!(
            !destination_survived,
            "failed validation left a copied project behind"
        );
        assert_eq!(std::fs::read(source).unwrap(), before);
        assert!(!state.has_resident_graph(&path).unwrap());
    }
}
