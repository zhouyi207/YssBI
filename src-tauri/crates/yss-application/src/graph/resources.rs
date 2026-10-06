use super::inputs::GraphContractMappingError;
use crate::events::{CommittedResourceMutation, committed_resource_mutation_from_project};
use crate::session::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use std::sync::Arc;
use thiserror::Error;
use yss_database_runtime::error::DatabaseError;
use yss_graph_document::{GraphResourceKind, GraphResourcePath};
use yss_graph_editor::MutationConflict;
use yss_graph_editor::projection::EditorProjectionError;
use yss_project::ProjectOperationError;
use yss_project_history::{FunctionDocumentPatch, MutationRequest};
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};
use yss_project_model::GraphResourceDocument;

#[derive(Debug, Error)]
pub enum ResourceMutationApplicationError {
    #[error("graph editing is busy")]
    EditingBusy,
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error(transparent)]
    Project(#[from] ProjectOperationError),
    #[error("graph resource mutation conflicted")]
    Mutation(#[source] MutationConflict),
    #[error("project resource mutation conflicted")]
    Resource(#[source] yss_project_history::ProjectResourceMutationError),
    #[error("graph operation capture failed")]
    GraphOperation(#[source] yss_project::ProjectGraphOperationError),
    #[error("graph operation commit failed")]
    GraphCommit(#[source] yss_project::ProjectGraphCommitError),
    #[error("graph resource is unavailable")]
    GraphUnavailable { graph: GraphResourcePath },
    #[error("project catalog facts could not be captured")]
    Catalog(#[source] crate::graph::catalog::ProjectCatalogReadError),
    #[error("database catalog snapshot failed")]
    Database(#[source] DatabaseError),
    #[error("graph catalog mapping failed")]
    Contract(#[source] GraphContractMappingError),
    #[error("editor projection failed")]
    Projection(#[source] EditorProjectionError),
    #[error("captured application session changed")]
    SessionChanged(#[source] SessionRevalidationError),
}

impl From<super::inputs::GraphInputError> for ResourceMutationApplicationError {
    fn from(error: super::inputs::GraphInputError) -> Self {
        match error {
            super::inputs::GraphInputError::Catalog(error) => Self::Catalog(error),
            super::inputs::GraphInputError::Database(error) => Self::Database(error),
            super::inputs::GraphInputError::Contract(error) => Self::Contract(error),
        }
    }
}

fn build_function_graph(
    path: &GraphResourcePath,
    name: String,
) -> Result<GraphResourceDocument, ResourceMutationApplicationError> {
    let mut resource = GraphResourceDocument::new(name, GraphResourceKind::FunctionGraph);
    for (node_type, x) in [
        ("yssbi.project.function.entry", 120.0),
        ("yssbi.project.function.return", 560.0),
    ] {
        let id = yss_graph_document::NodeId::new();
        let parameter = yss_node_protocol::ParameterKey::new("function").map_err(|error| {
            ResourceMutationApplicationError::Project(
                ProjectOperationError::TransactionPrepareFailed {
                    message: error.to_string(),
                },
            )
        })?;
        resource.document.nodes.insert(
            id,
            yss_graph_document::DocumentNode {
                id,
                node_type: yss_node_protocol::NodeTypeId::new(node_type).map_err(|error| {
                    ResourceMutationApplicationError::Project(
                        ProjectOperationError::TransactionPrepareFailed {
                            message: error.to_string(),
                        },
                    )
                })?,
                position: yss_graph_document::NodePosition { x, y: 160.0 },
                parameters: [(parameter, serde_json::Value::String(path.as_str().into()))]
                    .into_iter()
                    .collect(),
                user_label: None,
            },
        );
    }
    Ok(resource)
}

impl ApplicationState {
    pub fn create_event_graph(
        &self,
        project_instance_id: ProjectInstanceId,
        name: String,
        operation_id: OperationId,
    ) -> Result<CommittedResourceMutation, ResourceMutationApplicationError> {
        self.create_node_file(
            project_instance_id,
            name,
            operation_id,
            GraphResourceKind::EventGraph,
            |_, name| {
                Ok(GraphResourceDocument::new(
                    name,
                    GraphResourceKind::EventGraph,
                ))
            },
        )
    }
    pub fn create_function_graph(
        &self,
        project_instance_id: ProjectInstanceId,
        name: String,
        operation_id: OperationId,
    ) -> Result<CommittedResourceMutation, ResourceMutationApplicationError> {
        self.create_node_file(
            project_instance_id,
            name,
            operation_id,
            GraphResourceKind::FunctionGraph,
            build_function_graph,
        )
    }
    fn create_node_file(
        &self,
        project_instance_id: ProjectInstanceId,
        name: String,
        operation_id: OperationId,
        kind: GraphResourceKind,
        build: impl FnOnce(
            &GraphResourcePath,
            String,
        ) -> Result<GraphResourceDocument, ResourceMutationApplicationError>,
    ) -> Result<CommittedResourceMutation, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let (path, name) =
            captured
                .project()
                .allocate_graph_path(&project_instance_id, &name, kind)?;
        let resource = build(&path, name)?;
        let result = captured.project().create_graph_resource(
            &project_instance_id,
            &resource.name.clone(),
            resource,
            operation_id,
        )?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn duplicate_graph_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
        name: Option<String>,
    ) -> Result<CommittedResourceMutation, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured.project().duplicate_graph_resource(
            captured.graph().registry(),
            &project_instance_id,
            &graph_path,
            expected_revision,
            operation_id,
            name,
        )?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn remove_graph_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<CommittedResourceMutation, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured.project().remove_graph_resource(
            &project_instance_id,
            &graph_path,
            expected_revision,
            operation_id,
        )?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        captured
            .execution()
            .invalidate_graph_results(graph_path.as_str());
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn rename_graph_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        expected_revision: ResourceRevision,
        new_name: String,
        lifecycle_token: u64,
        operation_id: OperationId,
    ) -> Result<CommittedResourceMutation, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured.project().rename_graph_resource(
            captured.graph().registry(),
            &project_instance_id,
            yss_project::GraphResourceRenameRequest {
                graph_path: &graph_path,
                expected_revision,
                new_name: &new_name,
                lifecycle_token,
                operation_id,
            },
        )?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        captured
            .execution()
            .invalidate_graph_results(graph_path.as_str());
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn unload_graph_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        lifecycle_token: u64,
        discard_version: Option<yss_project::GraphEditVersion>,
    ) -> Result<bool, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let removed = captured.project().unload_graph_resource_for_lifecycle(
            &project_instance_id,
            &graph_path,
            lifecycle_token,
            discard_version,
        )?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        if removed {
            captured
                .execution()
                .invalidate_graph_results(graph_path.as_str());
        }
        Ok(removed)
    }

    pub fn update_function_signature(
        &self,
        project_instance_id: ProjectInstanceId,
        function_path: GraphResourcePath,
        request: MutationRequest<FunctionDocumentPatch>,
    ) -> Result<CommittedResourceMutation, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured
            .project()
            .update_function_signature(
                captured.graph().registry(),
                &project_instance_id,
                &function_path,
                request,
            )
            .map_err(ResourceMutationApplicationError::Resource)?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        let result = committed_resource_mutation_from_project(result);
        for path in result.projection_status.affected_graph_paths() {
            if let Ok(snapshot) = captured
                .project()
                .read_graph_editing(&project_instance_id, path)
            {
                captured.publish_graph_activity(crate::graph::editing::GraphActivity::Changed {
                    graph_path: path.as_str().into(),
                    editing: snapshot.state,
                });
            }
        }
        for graph in result.projection_status.affected_graph_paths() {
            captured
                .execution()
                .invalidate_graph_results(graph.as_str());
        }
        Ok(result)
    }

    pub(super) fn capture_resource_session(
        &self,
        project_instance_id: &ProjectInstanceId,
    ) -> Result<Arc<ApplicationSession>, ResourceMutationApplicationError> {
        let captured = self.capture_session()?;
        if captured.project_instance_id() != project_instance_id {
            return Err(ResourceMutationApplicationError::Project(
                ProjectOperationError::StaleProjectLifecycle {
                    message: "resource mutation project instance is stale".into(),
                },
            ));
        }
        Ok(captured)
    }
}
