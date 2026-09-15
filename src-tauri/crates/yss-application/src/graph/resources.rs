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

fn build_graph_shell(
    path: &GraphResourcePath,
    name: String,
    kind: GraphResourceKind,
) -> Result<GraphResourceDocument, ResourceMutationApplicationError> {
    let mut resource = GraphResourceDocument::new(name, kind);
    let shell_types: &[(&str, f64)] = match kind {
        GraphResourceKind::Event => &[],
        GraphResourceKind::Function => &[
            ("yssbi.project.function.entry", 120.0),
            ("yssbi.project.function.return", 560.0),
        ],
    };
    for (node_type, x) in shell_types {
        let id = yss_graph_document::NodeId::new();
        let parameters = if kind == GraphResourceKind::Function {
            [(
                yss_node_protocol::ParameterKey::new("function").map_err(|error| {
                    ResourceMutationApplicationError::Project(
                        ProjectOperationError::TransactionPrepareFailed {
                            message: error.to_string(),
                        },
                    )
                })?,
                serde_json::Value::String(path.as_str().to_owned()),
            )]
            .into_iter()
            .collect()
        } else {
            Default::default()
        };
        resource.document.nodes.insert(
            id,
            yss_graph_document::DocumentNode {
                id,
                node_type: yss_node_protocol::NodeTypeId::new(*node_type).map_err(|error| {
                    ResourceMutationApplicationError::Project(
                        ProjectOperationError::TransactionPrepareFailed {
                            message: error.to_string(),
                        },
                    )
                })?,
                position: yss_graph_document::NodePosition { x: *x, y: 160.0 },
                parameters,
                user_label: None,
            },
        );
    }
    Ok(resource)
}

impl ApplicationState {
    pub fn create_graph_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        name: String,
        kind: GraphResourceKind,
        operation_id: OperationId,
    ) -> Result<CommittedResourceMutation, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let (path, unique_name) =
            captured
                .project()
                .allocate_graph_path(&project_instance_id, &name, kind)?;
        let resource = build_graph_shell(&path, unique_name, kind)?;
        let resource_name = resource.name.clone();
        let result = captured.project().create_graph_resource(
            &project_instance_id,
            &resource_name,
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
    ) -> Result<CommittedResourceMutation, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured.project().duplicate_graph_resource(
            &project_instance_id,
            &graph_path,
            expected_revision,
            operation_id,
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
            &project_instance_id,
            &graph_path,
            expected_revision,
            &new_name,
            lifecycle_token,
            operation_id,
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
    ) -> Result<(), ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        captured.project().unload_graph_resource_for_lifecycle(
            &project_instance_id,
            &graph_path,
            lifecycle_token,
        )?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        captured
            .execution()
            .invalidate_graph_results(graph_path.as_str());
        Ok(())
    }

    pub fn update_function_signature(
        &self,
        project_instance_id: ProjectInstanceId,
        function_path: GraphResourcePath,
        locale: String,
        request: MutationRequest<FunctionDocumentPatch>,
    ) -> Result<CommittedResourceMutation, ResourceMutationApplicationError> {
        let _ = locale;
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured
            .project()
            .update_function_signature(&project_instance_id, &function_path, request)
            .map_err(ResourceMutationApplicationError::Resource)?;
        self.revalidate_captured_session(&captured)
            .map_err(ResourceMutationApplicationError::SessionChanged)?;
        let result = committed_resource_mutation_from_project(result);
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
