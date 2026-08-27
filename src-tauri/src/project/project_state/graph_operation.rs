use std::sync::Arc;

use crate::graph_document::{GraphDocument, GraphResourcePath, GraphRevision};

use super::state::ProjectState;
use crate::node_system::document::HistoryStatusDto;
use crate::project::{OperationId, ProjectGraphResidency};

#[derive(Clone)]
pub struct GraphOperationCapture {
    pub graph_path: GraphResourcePath,
    pub document: Arc<GraphDocument>,
    pub revision: GraphRevision,
    pub residency: ProjectGraphResidency,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphInvalidationSet {
    pub graph: bool,
    pub history: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphCommitReceipt {
    pub project_instance_id: crate::project::ProjectInstanceId,
    pub operation_id: OperationId,
    pub from_revision: GraphRevision,
    pub to_revision: GraphRevision,
    pub history: HistoryStatusDto,
    pub invalidations: GraphInvalidationSet,
}

#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ProjectGraphCommitError {
    #[error("graph operation authority is stale")]
    StaleAuthority,
    #[error("graph revision is exhausted")]
    RevisionExhausted,
    #[error("graph lifecycle changed during the operation")]
    LifecycleChanged,
    #[error("graph operation ownership changed")]
    OperationOwnershipChanged,
}

impl ProjectState {
    pub(crate) fn capture_graph_operation(
        &self,
        graph_path: &GraphResourcePath,
    ) -> Result<GraphOperationCapture, ProjectGraphCommitError> {
        let session = self
            .capture_project_session()
            .map_err(|_| ProjectGraphCommitError::LifecycleChanged)?;
        let data = self
            .get_data()
            .map_err(|_| ProjectGraphCommitError::StaleAuthority)?;
        let graph = data
            .graphs
            .get(graph_path)
            .ok_or(ProjectGraphCommitError::StaleAuthority)?;
        Ok(GraphOperationCapture {
            graph_path: graph_path.clone(),
            document: Arc::new(graph.document.clone()),
            revision: graph.document.revision,
            residency: ProjectGraphResidency::Loaded,
        })
    }
}
