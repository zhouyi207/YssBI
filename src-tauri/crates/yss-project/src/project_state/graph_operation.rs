use std::collections::HashMap;
use std::sync::Arc;

use crate::ProjectSession;
use yss_graph_document::{GraphDocument, GraphResourcePath, GraphRevision};
use yss_project_filesystem::{
    ProjectFilesystemError, ProjectFilesystemTransaction, ProjectFilesystemTransactionContext,
    StagedFilesystemMutation,
};
use yss_project_history::{
    ProjectGraphHistoryChange, ProjectGraphHistoryState, ProjectGraphResidency,
    ProjectHistoryTransaction,
};
use yss_project_identity::ProjectInstanceId;
use yss_project_operation::ProjectOperationReservation;

use super::state::ProjectState;

pub struct GraphOperationCapture {
    pub graph_path: GraphResourcePath,
    pub document: Arc<GraphDocument>,
    pub revision: GraphRevision,
    pub residency: ProjectGraphResidency,
    authority: GraphOperationAuthority,
}

impl GraphOperationCapture {
    pub fn into_authority(self) -> GraphOperationAuthority {
        self.authority
    }

    pub fn operation_id(&self) -> yss_project_identity::OperationId {
        self.authority.operation_id
    }
}

pub struct GraphOperationAuthority {
    session: ProjectSession,
    graph_path: GraphResourcePath,
    revision: GraphRevision,
    authority_generation: u64,
    operation_id: yss_project_identity::OperationId,
    reservation: ProjectOperationReservation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectHistoryStatus {
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphInvalidationSet {
    pub graph: bool,
    pub history: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphCommitReceipt {
    pub project_instance_id: ProjectInstanceId,
    pub operation_id: yss_project_identity::OperationId,
    pub from_revision: GraphRevision,
    pub to_revision: GraphRevision,
    pub history: ProjectHistoryStatus,
    pub history_change: Option<ProjectGraphHistoryChange>,
    pub invalidations: GraphInvalidationSet,
}

#[derive(Debug, thiserror::Error)]
#[error("graph operation capture failed")]
pub struct ProjectGraphOperationSource {
    #[source]
    source: ProjectFilesystemError,
}

impl ProjectGraphOperationSource {
    fn new(source: ProjectFilesystemError) -> Self {
        Self { source }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectGraphOperationError {
    #[error("graph mutation targets another project instance")]
    ProjectIdentityMismatch { requested: ProjectInstanceId },
    #[error("graph mutation source is unavailable")]
    GraphUnavailable { graph: GraphResourcePath },
    #[error("graph revision changed before planning")]
    RevisionConflict {
        graph: GraphResourcePath,
        expected: GraphRevision,
        current: GraphRevision,
    },
    #[error("graph resource lifecycle changed before planning")]
    ResourceLifecycleChanged { graph: GraphResourcePath },
    #[error("graph operation ownership changed before planning")]
    OperationOwnershipChanged {
        operation_id: yss_project_identity::OperationId,
    },
    #[error("project lifecycle admission is closed")]
    AdmissionClosed,
    #[error("project recovery is required")]
    RecoveryRequired,
    #[error("graph operation capture failed")]
    Internal(#[source] ProjectGraphOperationSource),
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectGraphCommitError {
    #[error("graph operation authority is stale")]
    StaleAuthority {
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        expected_revision: GraphRevision,
        current_revision: GraphRevision,
    },
    #[error("graph revision is exhausted")]
    RevisionExhausted {
        graph_path: GraphResourcePath,
        revision: GraphRevision,
    },
    #[error("graph lifecycle changed during the operation")]
    LifecycleChanged { graph_path: GraphResourcePath },
    #[error("graph operation ownership changed")]
    OperationOwnershipChanged {
        operation_id: yss_project_identity::OperationId,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum ProjectGraphSaveError {
    #[error(transparent)]
    Filesystem(#[from] ProjectFilesystemError),
    #[error(transparent)]
    Commit(#[from] ProjectGraphCommitError),
}

impl ProjectState {
    /// Capture the current graph without accepting a caller-authored revision.
    /// The revision remains an internal token for the atomic overwrite commit.
    pub fn capture_graph_overwrite_operation(
        &self,
        project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        operation_id: yss_project_identity::OperationId,
    ) -> Result<GraphOperationCapture, ProjectGraphOperationError> {
        for attempt in 0..3 {
            let revision = self
                .get_data()
                .map_err(|source| {
                    ProjectGraphOperationError::Internal(ProjectGraphOperationSource::new(source))
                })?
                .graphs
                .get(graph_path)
                .map(|resource| resource.document.revision)
                .ok_or_else(|| ProjectGraphOperationError::GraphUnavailable {
                    graph: graph_path.clone(),
                })?;
            match self.capture_graph_operation(
                project_instance_id,
                graph_path,
                revision,
                operation_id,
            ) {
                Err(ProjectGraphOperationError::RevisionConflict { .. }) if attempt < 2 => {}
                result => return result,
            }
        }
        unreachable!("overwrite capture loop returns on its final attempt")
    }

    pub fn capture_graph_operation(
        &self,
        project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        expected_revision: GraphRevision,
        operation_id: yss_project_identity::OperationId,
    ) -> Result<GraphOperationCapture, ProjectGraphOperationError> {
        self.ensure_project_operational()
            .map_err(capture_lifecycle_error)?;
        let session = self
            .capture_project_session()
            .map_err(capture_session_error)?;
        if session.instance_id != *project_instance_id {
            return Err(ProjectGraphOperationError::ProjectIdentityMismatch {
                requested: project_instance_id.clone(),
            });
        }

        let (authority_generation, document, revision) = {
            let publication = self
                .mutation_publication
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if publication.project_instance_id != project_instance_id.as_str() {
                return Err(ProjectGraphOperationError::ProjectIdentityMismatch {
                    requested: project_instance_id.clone(),
                });
            }
            let data = self
                .project_data
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let graph = data.graphs.get(graph_path).ok_or_else(|| {
                ProjectGraphOperationError::GraphUnavailable {
                    graph: graph_path.clone(),
                }
            })?;
            let revision = graph.document.revision;
            if revision != expected_revision {
                return Err(ProjectGraphOperationError::RevisionConflict {
                    graph: graph_path.clone(),
                    expected: expected_revision,
                    current: revision,
                });
            }
            let revisions = self
                .graph_revisions
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if revisions.get(graph_path) != Some(&revision) {
                return Err(ProjectGraphOperationError::ResourceLifecycleChanged {
                    graph: graph_path.clone(),
                });
            }
            (
                publication.authority_generation(),
                Arc::new(graph.document.clone()),
                revision,
            )
        };

        let reservation = self
            .reserve_resource_operation(project_instance_id, operation_id)
            .map_err(|error| match error {
                ProjectFilesystemError::DuplicateOperation { .. } => {
                    ProjectGraphOperationError::OperationOwnershipChanged { operation_id }
                }
                ProjectFilesystemError::ProjectRecoveryRequired { .. } => {
                    ProjectGraphOperationError::RecoveryRequired
                }
                ProjectFilesystemError::StaleProjectLifecycle { .. }
                | ProjectFilesystemError::StaleResourceLifecycle { .. }
                | ProjectFilesystemError::ProjectLifecycleAdmissionClosed { .. } => {
                    ProjectGraphOperationError::ResourceLifecycleChanged {
                        graph: graph_path.clone(),
                    }
                }
                source => {
                    ProjectGraphOperationError::Internal(ProjectGraphOperationSource::new(source))
                }
            })?;

        Ok(GraphOperationCapture {
            graph_path: graph_path.clone(),
            document,
            revision,
            residency: ProjectGraphResidency::Loaded,
            authority: GraphOperationAuthority {
                session,
                graph_path: graph_path.clone(),
                revision,
                authority_generation,
                operation_id,
                reservation,
            },
        })
    }

    pub fn commit_graph_candidate(
        &self,
        authority: GraphOperationAuthority,
        operation_id: yss_project_identity::OperationId,
        candidate_document: Arc<GraphDocument>,
    ) -> Result<GraphCommitReceipt, ProjectGraphCommitError> {
        let GraphOperationAuthority {
            session,
            graph_path,
            revision,
            authority_generation,
            operation_id: admitted_operation,
            reservation,
        } = authority;
        if admitted_operation != operation_id {
            return Err(ProjectGraphCommitError::OperationOwnershipChanged { operation_id });
        }
        self.ensure_project_operational().map_err(|_| {
            ProjectGraphCommitError::LifecycleChanged {
                graph_path: graph_path.clone(),
            }
        })?;
        let activation_generation_before = self
            .activation_generation
            .load(std::sync::atomic::Ordering::Acquire);
        if !activation_generation_before.is_multiple_of(2) {
            return Err(ProjectGraphCommitError::LifecycleChanged {
                graph_path: graph_path.clone(),
            });
        }
        let identity = self
            .activation_identity
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if identity.project_instance_id != session.instance_id
            || identity.project_root.as_ref() != Some(&session.root)
        {
            return Err(ProjectGraphCommitError::LifecycleChanged {
                graph_path: graph_path.clone(),
            });
        }
        drop(identity);
        let history_before = self
            .history
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .status();

        let result = (|| {
            let mut publication = self
                .mutation_publication
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let (recovery, _) = self.recovery_marker.boundary_recovering();
            if recovery.is_some() {
                return Err(ProjectGraphCommitError::LifecycleChanged {
                    graph_path: graph_path.clone(),
                });
            }
            if publication.project_instance_id != session.instance_id.as_str() {
                return Err(ProjectGraphCommitError::LifecycleChanged {
                    graph_path: graph_path.clone(),
                });
            }
            let mut data = self
                .project_data
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let graph = data.graphs.get(&graph_path).ok_or_else(|| {
                ProjectGraphCommitError::LifecycleChanged {
                    graph_path: graph_path.clone(),
                }
            })?;
            let current_revision = graph.document.revision;
            if publication.authority_generation() != authority_generation
                || current_revision != revision
                || candidate_document.revision != revision
            {
                return Err(ProjectGraphCommitError::StaleAuthority {
                    project_instance_id: session.instance_id.clone(),
                    graph_path: graph_path.clone(),
                    expected_revision: revision,
                    current_revision,
                });
            }
            let revisions = self
                .graph_revisions
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if revisions.get(&graph_path) != Some(&current_revision) {
                return Err(ProjectGraphCommitError::LifecycleChanged {
                    graph_path: graph_path.clone(),
                });
            }
            drop(revisions);
            let activation_generation_after = self
                .activation_generation
                .load(std::sync::atomic::Ordering::Acquire);
            if activation_generation_after != activation_generation_before
                || !activation_generation_after.is_multiple_of(2)
            {
                return Err(ProjectGraphCommitError::LifecycleChanged {
                    graph_path: graph_path.clone(),
                });
            }

            if candidate_document.as_ref() == &graph.document {
                return Ok((
                    GraphCommitReceipt {
                        project_instance_id: session.instance_id.clone(),
                        operation_id,
                        from_revision: revision,
                        to_revision: revision,
                        history: ProjectHistoryStatus {
                            can_undo: history_before.can_undo,
                            can_redo: history_before.can_redo,
                        },
                        history_change: None,
                        invalidations: GraphInvalidationSet {
                            graph: false,
                            history: false,
                        },
                    },
                    reservation,
                ));
            }

            let publication_advance = publication.prepare_authority_generation().map_err(|_| {
                ProjectGraphCommitError::LifecycleChanged {
                    graph_path: graph_path.clone(),
                }
            })?;
            let next_revision = revision.checked_next().map_err(|_| {
                ProjectGraphCommitError::RevisionExhausted {
                    graph_path: graph_path.clone(),
                    revision,
                }
            })?;
            let before_document = graph.document.clone();
            let mut after_document = candidate_document.as_ref().clone();
            after_document.revision = next_revision;
            let history_change = ProjectGraphHistoryChange {
                graph_path: graph_path.clone(),
                before: ProjectGraphHistoryState {
                    document: before_document,
                    revision,
                    residency: ProjectGraphResidency::Loaded,
                },
                after: ProjectGraphHistoryState {
                    document: after_document.clone(),
                    revision: next_revision,
                    residency: ProjectGraphResidency::Loaded,
                },
            };
            let mut resource = graph.clone();
            resource.document = after_document;
            Self::install_validated_resident_graph(&mut data, graph_path.clone(), resource);

            let mut graph_revisions = self
                .graph_revisions
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            graph_revisions.insert(graph_path.clone(), next_revision);
            drop(graph_revisions);

            let mut history = self
                .history
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            history.record_committed_transaction(ProjectHistoryTransaction::graph_change(
                operation_id,
                history_change.clone(),
            ));
            let history_status = history.status();

            publication.commit_prepared(publication_advance);
            Ok((
                GraphCommitReceipt {
                    project_instance_id: session.instance_id,
                    operation_id,
                    from_revision: revision,
                    to_revision: next_revision,
                    history: ProjectHistoryStatus {
                        can_undo: history_status.can_undo,
                        can_redo: history_status.can_redo,
                    },
                    history_change: Some(history_change),
                    invalidations: GraphInvalidationSet {
                        graph: true,
                        history: true,
                    },
                },
                reservation,
            ))
        })();

        match result {
            Ok((receipt, reservation)) => {
                reservation.complete();
                Ok(receipt)
            }
            Err(error) => Err(error),
        }
    }

    /// Persist and install a complete graph candidate as one overwrite
    /// transaction. A stale Rust authority rolls the filesystem write back.
    pub fn save_graph_candidate(
        &self,
        capture: GraphOperationCapture,
        operation_id: yss_project_identity::OperationId,
        candidate_document: Arc<GraphDocument>,
    ) -> Result<GraphCommitReceipt, ProjectGraphSaveError> {
        let graph_path = capture.graph_path.clone();
        let session = capture.authority.session.clone();
        let changed = candidate_document.as_ref() != capture.document.as_ref();
        let persisted_revision = if changed {
            capture.revision.checked_next().map_err(|_| {
                ProjectGraphSaveError::Commit(ProjectGraphCommitError::RevisionExhausted {
                    graph_path: graph_path.clone(),
                    revision: capture.revision,
                })
            })?
        } else {
            capture.revision
        };

        let data = self.get_data()?;
        let mut resource = data.graphs.get(&graph_path).cloned().ok_or_else(|| {
            ProjectFilesystemError::StaleResourceLifecycle {
                message: format!("graph '{graph_path}' is not resident"),
            }
        })?;
        let mut persisted_document = candidate_document.as_ref().clone();
        persisted_document.revision = persisted_revision;
        resource.document = persisted_document;
        let local_variables = data
            .variables
            .iter()
            .filter(|(_, variable)| match &variable.scope {
                yss_variable_contract::VariableScope::Global => false,
                yss_variable_contract::VariableScope::Event { event_path }
                | yss_variable_contract::VariableScope::Function {
                    function_path: event_path,
                } => event_path == graph_path.as_str(),
            })
            .map(|(id, variable)| (*id, variable.clone()))
            .collect::<HashMap<_, _>>();
        let contents =
            crate::project_io::serialize_graph_resource_document(&resource, local_variables)
                .map_err(|error| ProjectFilesystemError::TransactionPrepareFailed {
                    message: error.to_string(),
                })?;
        let filesystem_lease = self.filesystem().acquire(session.root.clone())?;
        self.validate_project_session(&session)?;
        let prepared = ProjectFilesystemTransaction::prepare(
            ProjectFilesystemTransactionContext {
                root: session.root,
                operation_id,
                recovery_marker: Some(self.project_recovery_marker()),
            },
            filesystem_lease,
            vec![StagedFilesystemMutation::Write {
                relative_path: graph_path.as_str().into(),
                contents,
            }],
        )?;
        let committed = prepared.commit()?;

        match self.commit_graph_candidate(
            capture.into_authority(),
            operation_id,
            candidate_document,
        ) {
            Ok(receipt) => {
                committed.finalize();
                Ok(receipt)
            }
            Err(error) => {
                committed.rollback()?;
                Err(ProjectGraphSaveError::Commit(error))
            }
        }
    }
}

fn capture_lifecycle_error(error: ProjectFilesystemError) -> ProjectGraphOperationError {
    match error {
        ProjectFilesystemError::ProjectRecoveryRequired { .. } => {
            ProjectGraphOperationError::RecoveryRequired
        }
        ProjectFilesystemError::StaleProjectLifecycle { .. }
        | ProjectFilesystemError::ProjectLifecycleAdmissionClosed { .. } => {
            ProjectGraphOperationError::AdmissionClosed
        }
        source => ProjectGraphOperationError::Internal(ProjectGraphOperationSource::new(source)),
    }
}

fn capture_session_error(error: ProjectFilesystemError) -> ProjectGraphOperationError {
    capture_lifecycle_error(error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_graph_document::{DocumentNode, NodeId, NodePosition};
    use yss_graph_protocol::NodeTypeId;
    use yss_project_model::{GraphResourceDocument, ProjectData};

    #[test]
    fn overwrite_save_captures_revision_internally_and_installs_complete_candidate() {
        let graph_path = GraphResourcePath::new("events/Main.yssbi-event").unwrap();
        let node_id = NodeId::new();
        let mut resource =
            GraphResourceDocument::new("Main", yss_graph_document::GraphResourceKind::Event);
        resource.document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: NodeTypeId::new("yssbi.tests.node").unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: Default::default(),
                user_label: None,
            },
        );
        let mut project = ProjectData::new();
        project.graphs.insert(graph_path.clone(), resource);
        let fixture = crate::fixtures::TempProject::activate("graph-overwrite-save", project);
        let session = fixture.state().capture_project_session().unwrap();
        let operation_id = yss_project_identity::OperationId::new();
        let capture = fixture
            .state()
            .capture_graph_overwrite_operation(&session.instance_id, &graph_path, operation_id)
            .unwrap();
        let mut candidate = capture.document.as_ref().clone();
        candidate.nodes.get_mut(&node_id).unwrap().position = NodePosition { x: 24.0, y: 36.0 };

        let receipt = fixture
            .state()
            .save_graph_candidate(capture, operation_id, Arc::new(candidate))
            .unwrap();

        assert_eq!(receipt.from_revision, GraphRevision::INITIAL);
        assert_eq!(receipt.to_revision, GraphRevision::new(1));
        assert_eq!(
            fixture.state().get_data().unwrap().graphs[&graph_path]
                .document
                .nodes[&node_id]
                .position,
            NodePosition { x: 24.0, y: 36.0 }
        );
    }
}
