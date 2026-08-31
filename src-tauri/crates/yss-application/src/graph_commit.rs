use std::sync::Arc;

use crate::execution::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use yss_graph_document::GraphDocument;
use yss_project::{GraphCommitReceipt, GraphOperationCapture, ProjectGraphCommitError};

#[derive(Debug, thiserror::Error)]
pub enum GraphCommitApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("captured application session changed")]
    SessionChanged,
    #[error(transparent)]
    Commit(#[from] ProjectGraphCommitError),
}

pub(crate) fn commit_captured_graph_candidate(
    application: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    capture: GraphOperationCapture,
    candidate_document: Arc<GraphDocument>,
) -> Result<GraphCommitReceipt, GraphCommitApplicationError> {
    application
        .revalidate_captured_session(captured)
        .map_err(map_session_revalidation)?;
    let operation_id = capture.operation_id();
    let authority = capture.into_authority();
    captured
        .project()
        .commit_graph_candidate(authority, operation_id, candidate_document)
        .map_err(GraphCommitApplicationError::Commit)
}

fn map_session_revalidation(error: SessionRevalidationError) -> GraphCommitApplicationError {
    match error {
        SessionRevalidationError::Unavailable(error) => {
            GraphCommitApplicationError::SessionCapture(error)
        }
        SessionRevalidationError::Changed => GraphCommitApplicationError::SessionChanged,
    }
}
