use std::sync::Arc;

use crate::project::resource_mutations::ResourceOperationReservation;
use crate::project::{
    ProjectFilesystemError, ProjectGraphHistoryChange, ProjectGraphHistoryState,
    ProjectGraphResidency, ProjectHistoryTransaction, ProjectSession,
};
use yss_graph_document::{GraphDocument, GraphResourcePath, GraphRevision};
use yss_project_identity::ProjectInstanceId;

use super::state::ProjectState;

pub struct GraphOperationCapture {
    pub graph_path: GraphResourcePath,
    pub document: Arc<GraphDocument>,
    pub revision: GraphRevision,
    pub residency: ProjectGraphResidency,
    authority: GraphOperationAuthority,
}

impl GraphOperationCapture {
    pub(crate) fn into_authority(self) -> GraphOperationAuthority {
        self.authority
    }

    pub(crate) fn operation_id(&self) -> yss_project_identity::OperationId {
        self.authority.operation_id
    }
}

pub(crate) struct GraphOperationAuthority {
    session: ProjectSession,
    graph_path: GraphResourcePath,
    revision: GraphRevision,
    authority_generation: u64,
    operation_id: yss_project_identity::OperationId,
    reservation: ResourceOperationReservation,
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

impl ProjectState {
    pub(crate) fn capture_graph_operation(
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

    pub(crate) fn commit_graph_candidate(
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
        if activation_generation_before % 2 != 0 {
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
                || activation_generation_after % 2 != 0
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
