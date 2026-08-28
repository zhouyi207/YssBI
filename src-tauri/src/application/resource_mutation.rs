use std::sync::Arc;

use thiserror::Error;

use super::execution::session_slot::{
    ApplicationSession, ApplicationSessionRefreshError, ApplicationState, SessionCaptureError,
    SessionRevalidationError,
};
use crate::event::ResourceMutationResultDto;
use crate::graph_document::GraphResourcePath;
use crate::node_system::document::{FunctionDocumentPatch, MutationRequest};
use crate::project::project_writers::ProjectSaveResultDto;
use crate::project::{
    GraphDocumentKind, OperationId, ProjectFilesystemError, ProjectInstanceId, ResourceRevision,
};

#[derive(Debug, Error)]
pub enum ResourceMutationApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error(transparent)]
    Project(#[from] ProjectFilesystemError),
    #[error("graph resource mutation conflicted")]
    Mutation(#[source] crate::node_system::document::MutationConflict),
    #[error("captured application session changed")]
    SessionChanged(#[source] SessionRevalidationError),
    #[error("application session refresh failed")]
    SessionRefresh(#[source] ApplicationSessionRefreshError),
}

impl ApplicationState {
    pub fn create_graph_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        name: String,
        kind: GraphDocumentKind,
        operation_id: OperationId,
    ) -> Result<ResourceMutationResultDto, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured.project().create_graph_resource_transaction(
            &project_instance_id,
            &name,
            kind,
            operation_id,
        )?;
        self.refresh_resource_session()?;
        Ok(result)
    }

    pub fn duplicate_graph_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<ResourceMutationResultDto, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured.project().duplicate_graph_resource_transaction(
            &project_instance_id,
            &graph_path,
            expected_revision,
            operation_id,
        )?;
        self.refresh_resource_session()?;
        Ok(result)
    }

    pub fn remove_graph_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<ResourceMutationResultDto, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured.project().remove_graph_resource_transaction(
            &project_instance_id,
            &graph_path,
            expected_revision,
            operation_id,
        )?;
        self.refresh_resource_session()?;
        Ok(result)
    }

    pub fn rename_graph_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        expected_revision: ResourceRevision,
        new_name: String,
        lifecycle_token: u64,
        operation_id: OperationId,
    ) -> Result<ResourceMutationResultDto, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured.project().rename_graph_resource_transaction(
            &project_instance_id,
            &graph_path,
            expected_revision,
            &new_name,
            lifecycle_token,
            operation_id,
        )?;
        self.refresh_resource_session()?;
        Ok(result)
    }

    pub fn save_graph_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        graph_path: GraphResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<ProjectSaveResultDto, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured.project().save_graph_document(
            &project_instance_id,
            &graph_path,
            expected_revision,
            operation_id,
        )?;
        self.refresh_resource_session()?;
        Ok(result)
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
        self.refresh_resource_session()?;
        Ok(())
    }

    pub fn update_function_signature(
        &self,
        project_instance_id: ProjectInstanceId,
        function_path: GraphResourcePath,
        locale: String,
        request: MutationRequest<FunctionDocumentPatch>,
    ) -> Result<ResourceMutationResultDto, ResourceMutationApplicationError> {
        let captured = self.capture_resource_session(&project_instance_id)?;
        let result = captured
            .project()
            .update_function_signature_observed(
                &project_instance_id,
                &function_path,
                &locale,
                request,
                |_| {},
            )
            .map_err(ResourceMutationApplicationError::Mutation)?;
        self.refresh_resource_session()?;
        Ok(result)
    }

    fn capture_resource_session(
        &self,
        project_instance_id: &ProjectInstanceId,
    ) -> Result<Arc<ApplicationSession>, ResourceMutationApplicationError> {
        let captured = self.capture_session()?;
        if captured.project_instance_id() != project_instance_id {
            return Err(ResourceMutationApplicationError::Project(
                ProjectFilesystemError::StaleProjectLifecycle {
                    message: "resource mutation project instance is stale".into(),
                },
            ));
        }
        Ok(captured)
    }

    fn refresh_resource_session(&self) -> Result<(), ResourceMutationApplicationError> {
        self.refresh_current_project()
            .map_err(ResourceMutationApplicationError::SessionRefresh)
    }
}
