use std::sync::Arc;

#[cfg(test)]
use crate::application::execution::ApplicationSessionSlot;
use crate::application::execution::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use crate::graph::error::GraphMutationError;
use crate::graph::mutation::{GraphMutation, PlannedGraphMutation, plan_graph_mutation};
use crate::graph::resource_catalog::ResourceCatalogSnapshot;
use crate::project::project_state::graph_operation::{
    GraphCommitReceipt, GraphInvalidationSet, GraphOperationCapture, ProjectGraphCommitError,
    ProjectGraphOperationError, ProjectHistoryStatus,
};
use crate::project::{OperationId, ProjectGraphHistoryChange, ProjectInstanceId};
use yss_graph_document::{GraphDocument, GraphResourcePath, GraphRevision};

#[derive(Clone, Debug)]
pub struct GraphMutationRequest {
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
    expected_revision: GraphRevision,
    operation_id: OperationId,
    candidate: GraphDocument,
}

impl GraphMutationRequest {
    pub fn new(
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        expected_revision: GraphRevision,
        operation_id: OperationId,
        candidate: GraphDocument,
    ) -> Self {
        Self {
            project_instance_id,
            graph_path,
            expected_revision,
            operation_id,
            candidate,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphMutationApplicationReceipt {
    pub project_instance_id: ProjectInstanceId,
    pub operation_id: OperationId,
    pub from_revision: GraphRevision,
    pub to_revision: GraphRevision,
    pub history: ProjectHistoryStatus,
    pub history_change: Option<ProjectGraphHistoryChange>,
    pub invalidations: GraphInvalidationSet,
}

impl From<GraphCommitReceipt> for GraphMutationApplicationReceipt {
    fn from(receipt: GraphCommitReceipt) -> Self {
        Self {
            project_instance_id: receipt.project_instance_id,
            operation_id: receipt.operation_id,
            from_revision: receipt.from_revision,
            to_revision: receipt.to_revision,
            history: receipt.history,
            history_change: receipt.history_change,
            invalidations: receipt.invalidations,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GraphMutationApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("captured application session changed")]
    SessionChanged,
    #[error(transparent)]
    Project(#[from] ProjectGraphOperationError),
    #[error(transparent)]
    Graph(#[from] GraphMutationError),
    #[error(transparent)]
    Commit(#[from] ProjectGraphCommitError),
}

impl ApplicationState {
    pub fn mutate_graph(
        &self,
        request: GraphMutationRequest,
    ) -> Result<GraphMutationApplicationReceipt, GraphMutationApplicationError> {
        let captured = self.capture_session()?;
        mutate_graph_in_session(self, &captured, request)
    }
}

pub(crate) fn mutate_graph_in_session(
    application: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    request: GraphMutationRequest,
) -> Result<GraphMutationApplicationReceipt, GraphMutationApplicationError> {
    let capture = captured.project().capture_graph_operation(
        &request.project_instance_id,
        &request.graph_path,
        request.expected_revision,
        request.operation_id,
    )?;
    let planned = plan_captured_graph_mutation(
        &capture,
        request.candidate,
        captured.graph().resource_catalog(),
    )?;
    let candidate_document = planned.into_candidate_document();
    commit_captured_graph_candidate(application, captured, capture, candidate_document)
}

pub(crate) fn commit_captured_graph_candidate(
    application: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    capture: GraphOperationCapture,
    candidate_document: Arc<GraphDocument>,
) -> Result<GraphMutationApplicationReceipt, GraphMutationApplicationError> {
    application
        .revalidate_captured_session(captured)
        .map_err(map_session_revalidation)?;
    let operation_id = capture.operation_id();
    let authority = capture.into_authority();
    captured
        .project()
        .commit_graph_candidate(authority, operation_id, candidate_document)
        .map(Into::into)
        .map_err(GraphMutationApplicationError::Commit)
}

fn map_session_revalidation(error: SessionRevalidationError) -> GraphMutationApplicationError {
    match error {
        SessionRevalidationError::Unavailable(error) => {
            GraphMutationApplicationError::SessionCapture(error)
        }
        SessionRevalidationError::Changed => GraphMutationApplicationError::SessionChanged,
    }
}

pub(crate) fn plan_captured_graph_mutation(
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
    .map_err(GraphMutationApplicationError::Graph)
}

#[cfg(all(test, any()))]
mod tests {
    use super::{
        ApplicationSessionSlot, ApplicationState, GraphMutationApplicationError,
        GraphMutationRequest, SessionCaptureError, plan_captured_graph_mutation,
    };
    use crate::graph::resource_catalog::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};
    use crate::project::project_state::graph_operation::ProjectGraphCommitError;
    use crate::project::{
        GraphDocumentKind, GraphResourceDocument, OperationId, ProjectData, ProjectInstanceId,
        fixtures,
    };
    use std::collections::BTreeMap;
    use std::sync::{Arc, Barrier};
    use yss_graph_document::{DocumentNode, GraphDocument, GraphRevision, NodeId, NodePosition};
    use yss_graph_protocol::NodeTypeId;

    fn empty_catalog() -> ResourceCatalogSnapshot {
        ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            ResourceCatalogFingerprint::from_bytes([0; 32]),
        )
    }

    #[test]
    fn public_mutate_graph_reports_typed_capture_error_without_a_session() {
        let application = ApplicationState::new(Arc::new(ApplicationSessionSlot::new()));
        let request = GraphMutationRequest::new(
            ProjectInstanceId::new(),
            yss_graph_document::GraphResourcePath::new("events/Inactive.yssbi-event")
                .expect("fixture path is valid"),
            GraphRevision::INITIAL,
            OperationId::new(),
            GraphDocument::default(),
        );

        assert!(matches!(
            application.mutate_graph(request),
            Err(GraphMutationApplicationError::SessionCapture(
                SessionCaptureError::Inactive
            ))
        ));
    }

    #[test]
    fn graph_commit_publishes_project_facts_after_candidate_handoff() {
        let graph_path = yss_graph_document::GraphResourcePath::new("events/Committed.yssbi-event")
            .expect("fixture path is valid");
        let mut project = ProjectData::new();
        project.graphs.insert(
            graph_path.clone(),
            GraphResourceDocument::new("Committed", GraphDocumentKind::Event),
        );
        let fixture = fixtures::TempProject::activate("graph-commit-facts", project);
        let state = fixture.state().clone();
        let project_instance_id = state
            .capture_project_session()
            .expect("fixture project is active")
            .instance_id;
        let operation_id = OperationId::new();
        let capture = state
            .capture_graph_operation(
                &project_instance_id,
                &graph_path,
                GraphRevision::INITIAL,
                operation_id,
            )
            .expect("graph capture is admitted");
        let mut candidate = capture.document.as_ref().clone();
        let node_id = NodeId::new();
        candidate.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: NodeTypeId::new("yssbi.constant.int64")
                    .expect("fixture node type is valid"),
                position: NodePosition { x: 1.0, y: 2.0 },
                parameters: BTreeMap::new(),
                user_label: None,
            },
        );
        let planned = plan_captured_graph_mutation(&capture, candidate, &empty_catalog())
            .expect("Graph planner accepts the captured basis");
        let candidate_document = planned.into_candidate_document();
        let authority = capture.into_authority();

        let receipt = state
            .commit_graph_candidate(authority, operation_id, candidate_document)
            .expect("Project authority accepts the candidate");

        assert_eq!(receipt.from_revision, GraphRevision::INITIAL);
        assert_eq!(receipt.to_revision, GraphRevision::new(1));
        assert!(receipt.invalidations.graph);
        assert!(receipt.invalidations.history);
        let history_change = receipt
            .history_change
            .expect("successful candidate has a history fact");
        assert_eq!(history_change.before.document.nodes.len(), 0);
        assert_eq!(history_change.after.document.nodes.len(), 1);
        assert_eq!(
            state
                .get_data()
                .expect("fixture project remains active")
                .graphs[&graph_path]
                .document,
            history_change.after.document
        );
    }

    #[test]
    fn graph_commit_rejects_authority_changed_at_barrier_without_candidate_effects() {
        let graph_path = yss_graph_document::GraphResourcePath::new("events/Barrier.yssbi-event")
            .expect("fixture path is valid");
        let mut project = ProjectData::new();
        project.graphs.insert(
            graph_path.clone(),
            GraphResourceDocument::new("Barrier", GraphDocumentKind::Event),
        );
        let fixture = fixtures::TempProject::activate("graph-commit-barrier", project);
        let state = fixture.state().clone();
        let project_instance_id = state
            .capture_project_session()
            .expect("fixture project is active")
            .instance_id;
        let operation_id = OperationId::new();
        let capture = state
            .capture_graph_operation(
                &project_instance_id,
                &graph_path,
                GraphRevision::INITIAL,
                operation_id,
            )
            .expect("graph capture is admitted");
        let planned = plan_captured_graph_mutation(
            &capture,
            capture.document.as_ref().clone(),
            &empty_catalog(),
        )
        .expect("Graph planner accepts the captured basis");
        let candidate_document = planned.into_candidate_document();
        let authority = capture.into_authority();

        let barrier = Arc::new(Barrier::new(2));
        let concurrent_state = state.clone();
        let concurrent_path = graph_path.clone();
        let concurrent_barrier = Arc::clone(&barrier);
        let concurrent = std::thread::spawn(move || {
            concurrent_barrier.wait();
            concurrent_state
                .insert_graph(
                    concurrent_path,
                    GraphResourceDocument::new("Concurrent", GraphDocumentKind::Event),
                )
                .expect("barrier mutation is valid");
        });
        barrier.wait();
        concurrent.join().expect("barrier worker completed");

        let error = state
            .commit_graph_candidate(authority, operation_id, candidate_document)
            .expect_err("authority changed after lock-free planning");
        assert!(matches!(
            error,
            ProjectGraphCommitError::StaleAuthority { .. }
        ));
        let current = state.get_data().expect("fixture project remains active");
        assert_eq!(current.graphs[&graph_path].name, "Concurrent");
        assert_eq!(
            current.graphs[&graph_path].document.revision,
            GraphRevision::INITIAL
        );
    }
}
