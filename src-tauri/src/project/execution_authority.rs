use std::sync::Arc;
use std::time::Instant;

use crate::graph_document::{GraphDocument, GraphResourcePath};

#[derive(Clone, Debug)]
pub struct ProjectExecutionRequest {
    pub project_instance_id: crate::project::ProjectInstanceId,
    pub graph_path: GraphResourcePath,
}

#[derive(Clone, Debug)]
pub struct ProjectExecutionAuthority {
    project_instance_id: crate::project::ProjectInstanceId,
    graph_path: GraphResourcePath,
    document: Arc<GraphDocument>,
    authority_generation: u64,
}

impl ProjectExecutionAuthority {
    pub fn project_instance_id(&self) -> &crate::project::ProjectInstanceId {
        &self.project_instance_id
    }

    pub fn graph_path(&self) -> &GraphResourcePath {
        &self.graph_path
    }

    pub fn document(&self) -> &GraphDocument {
        &self.document
    }

    pub const fn authority_generation(&self) -> u64 {
        self.authority_generation
    }
}

pub struct PreparedProjectExecution {
    authority: ProjectExecutionAuthority,
}

impl PreparedProjectExecution {
    pub fn authority(&self) -> &ProjectExecutionAuthority {
        &self.authority
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CandidateProjectEffects {
    pub graph: Option<GraphDocument>,
}

pub struct PreparedEffectCommit {
    authority: ProjectExecutionAuthority,
    effects: CandidateProjectEffects,
}

pub struct CommittedProjectEffects {
    project_instance_id: crate::project::ProjectInstanceId,
}

impl CommittedProjectEffects {
    pub fn project_instance_id(&self) -> &crate::project::ProjectInstanceId {
        &self.project_instance_id
    }
}

#[derive(Clone)]
pub struct ProjectEffectCommitControl {
    pub cancellation: Arc<std::sync::atomic::AtomicBool>,
    pub deadline: Instant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProjectExecutionPreparationError {
    #[error("project execution authority is unavailable")]
    Unavailable,
    #[error("project execution request belongs to another project")]
    ProjectIdentityMismatch,
    #[error("requested graph is unavailable")]
    GraphUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ProjectEffectCommitError {
    #[error("project effect commit authority is stale")]
    StaleAuthority,
    #[error("project effect commit was cancelled")]
    Cancelled,
    #[error("project effect commit deadline was exceeded")]
    Deadline,
    #[error("project effect candidate is invalid")]
    InvalidCandidate,
}

impl crate::project::ProjectState {
    pub fn prepare_execution(
        &self,
        request: ProjectExecutionRequest,
    ) -> Result<PreparedProjectExecution, ProjectExecutionPreparationError> {
        let session = self
            .capture_project_session()
            .map_err(|_| ProjectExecutionPreparationError::Unavailable)?;
        if session.instance_id != request.project_instance_id {
            return Err(ProjectExecutionPreparationError::ProjectIdentityMismatch);
        }
        let data = self
            .get_data()
            .map_err(|_| ProjectExecutionPreparationError::Unavailable)?;
        let graph = data
            .graphs
            .get(&request.graph_path)
            .ok_or(ProjectExecutionPreparationError::GraphUnavailable)?;
        Ok(PreparedProjectExecution {
            authority: ProjectExecutionAuthority {
                project_instance_id: session.instance_id,
                graph_path: request.graph_path,
                document: Arc::new(graph.document.clone()),
                authority_generation: self.activation_revision(),
            },
        })
    }

    pub fn prepare_execution_effects(
        &self,
        authority: &ProjectExecutionAuthority,
        effects: CandidateProjectEffects,
    ) -> Result<PreparedEffectCommit, ProjectEffectCommitError> {
        if effects
            .graph
            .as_ref()
            .is_some_and(|graph| graph.revision != authority.document.revision)
        {
            return Err(ProjectEffectCommitError::InvalidCandidate);
        }
        Ok(PreparedEffectCommit {
            authority: authority.clone(),
            effects,
        })
    }

    pub fn finalize_execution_effects(
        &self,
        prepared: PreparedEffectCommit,
        control: &ProjectEffectCommitControl,
    ) -> Result<CommittedProjectEffects, ProjectEffectCommitError> {
        if control
            .cancellation
            .load(std::sync::atomic::Ordering::Acquire)
        {
            return Err(ProjectEffectCommitError::Cancelled);
        }
        if control.deadline <= Instant::now() {
            return Err(ProjectEffectCommitError::Deadline);
        }
        let current = self
            .capture_project_session()
            .map_err(|_| ProjectEffectCommitError::StaleAuthority)?;
        if current.instance_id != prepared.authority.project_instance_id {
            return Err(ProjectEffectCommitError::StaleAuthority);
        }
        let _ = prepared.effects;
        Ok(CommittedProjectEffects {
            project_instance_id: current.instance_id,
        })
    }
}
