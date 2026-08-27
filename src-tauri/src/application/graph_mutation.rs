use std::sync::Arc;

use crate::graph::mutation::{GraphMutation, PlannedGraphMutation, plan_graph_mutation};
use crate::graph::resource_catalog::ResourceCatalogSnapshot;
use crate::graph_document::{GraphDocument, GraphResourcePath, GraphRevision};
use crate::project::project_state::graph_operation::{
    GraphCommitReceipt, GraphOperationCapture, ProjectGraphCommitError,
};

#[derive(Debug, thiserror::Error)]
pub enum GraphMutationApplicationError {
    #[error("graph mutation capture failed")]
    Capture(#[source] ProjectGraphCommitError),
    #[error("graph mutation planning failed")]
    Plan(#[source] crate::graph::error::GraphMutationError),
}

pub fn plan_captured_graph_mutation(
    capture: &GraphOperationCapture,
    candidate: GraphDocument,
    catalog: &ResourceCatalogSnapshot,
) -> Result<PlannedGraphMutation, GraphMutationApplicationError> {
    plan_graph_mutation(
        capture.document.as_ref(),
        capture.revision,
        GraphMutation::ReplaceDocument { candidate },
        catalog,
    )
    .map_err(GraphMutationApplicationError::Plan)
}

pub fn graph_path(capture: &GraphOperationCapture) -> &GraphResourcePath {
    &capture.graph_path
}

pub fn graph_revision(capture: &GraphOperationCapture) -> GraphRevision {
    capture.revision
}

pub fn candidate_arc(planned: PlannedGraphMutation) -> Arc<GraphDocument> {
    planned.into_candidate_document()
}

pub fn commit_receipt_placeholder(
    _capture: &GraphOperationCapture,
) -> Result<GraphCommitReceipt, GraphMutationApplicationError> {
    Err(GraphMutationApplicationError::Capture(
        ProjectGraphCommitError::OperationOwnershipChanged,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn application_planner_has_a_single_candidate_handoff() {
        let _ = graph_revision;
        let _ = graph_path;
        let _ = candidate_arc;
        let _ = commit_receipt_placeholder;
    }
}
