use thiserror::Error;

use super::events::{CommittedResourceMutation, committed_resource_mutation_from_project};
use super::execution::session_slot::{
    ApplicationSessionRefreshError, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use crate::project::{
    OperationId, ProjectFilesystemError, ProjectInstanceId, ResourceName, ResourceRevision,
    WorksheetDocument, WorksheetResourcePath,
};

#[derive(Debug, Error)]
pub enum WorksheetApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error(transparent)]
    Project(#[from] ProjectFilesystemError),
    #[error("captured application session changed during worksheet operation")]
    SessionChanged(#[source] SessionRevalidationError),
    #[error("application session refresh failed")]
    SessionRefresh(#[source] ApplicationSessionRefreshError),
}

impl ApplicationState {
    pub fn create_worksheet_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        operation_id: OperationId,
        name: String,
        database_id: Option<String>,
    ) -> Result<CommittedResourceMutation, WorksheetApplicationError> {
        let captured = self.capture_worksheet_session(&project_instance_id)?;
        let name = ResourceName::parse(&name).map_err(ProjectFilesystemError::from)?;
        let result = captured.project().create_worksheet_resource_facts(
            &project_instance_id,
            &name,
            database_id,
            operation_id,
        )?;
        self.refresh_worksheet_session()?;
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn duplicate_worksheet_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        operation_id: OperationId,
        worksheet_path: WorksheetResourcePath,
        expected_revision: ResourceRevision,
    ) -> Result<CommittedResourceMutation, WorksheetApplicationError> {
        let captured = self.capture_worksheet_session(&project_instance_id)?;
        let result = captured.project().duplicate_worksheet_resource_facts(
            &project_instance_id,
            &worksheet_path,
            expected_revision,
            operation_id,
        )?;
        self.refresh_worksheet_session()?;
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn load_worksheet_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        worksheet_path: WorksheetResourcePath,
    ) -> Result<WorksheetDocument, WorksheetApplicationError> {
        let captured = self.capture_worksheet_session(&project_instance_id)?;
        let result = captured
            .project()
            .load_worksheet_document(&project_instance_id, &worksheet_path)?;
        self.revalidate_captured_session(&captured)
            .map_err(WorksheetApplicationError::SessionChanged)?;
        Ok(result)
    }

    pub fn save_worksheet_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        operation_id: OperationId,
        worksheet_path: WorksheetResourcePath,
        expected_revision: ResourceRevision,
        document: WorksheetDocument,
    ) -> Result<CommittedResourceMutation, WorksheetApplicationError> {
        let captured = self.capture_worksheet_session(&project_instance_id)?;
        let result = captured.project().save_worksheet_document_facts(
            &project_instance_id,
            &worksheet_path,
            expected_revision,
            operation_id,
            document,
        )?;
        self.refresh_worksheet_session()?;
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn rename_worksheet_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        operation_id: OperationId,
        worksheet_path: WorksheetResourcePath,
        expected_revision: ResourceRevision,
        new_name: String,
        lifecycle_token: u64,
    ) -> Result<CommittedResourceMutation, WorksheetApplicationError> {
        let captured = self.capture_worksheet_session(&project_instance_id)?;
        let new_name = ResourceName::parse(&new_name).map_err(ProjectFilesystemError::from)?;
        let result = captured.project().rename_worksheet_resource_facts(
            &project_instance_id,
            &worksheet_path,
            expected_revision,
            &new_name,
            lifecycle_token,
            operation_id,
        )?;
        self.refresh_worksheet_session()?;
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn remove_worksheet_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        operation_id: OperationId,
        worksheet_path: WorksheetResourcePath,
        expected_revision: ResourceRevision,
    ) -> Result<CommittedResourceMutation, WorksheetApplicationError> {
        let captured = self.capture_worksheet_session(&project_instance_id)?;
        let result = captured.project().remove_worksheet_resource_facts(
            &project_instance_id,
            &worksheet_path,
            expected_revision,
            operation_id,
        )?;
        self.refresh_worksheet_session()?;
        Ok(committed_resource_mutation_from_project(result))
    }

    fn capture_worksheet_session(
        &self,
        project_instance_id: &ProjectInstanceId,
    ) -> Result<std::sync::Arc<super::execution::ApplicationSession>, WorksheetApplicationError>
    {
        let captured = self.capture_session()?;
        if captured.project_instance_id() != project_instance_id {
            return Err(WorksheetApplicationError::Project(
                ProjectFilesystemError::StaleProjectLifecycle {
                    message: "worksheet project instance is stale".into(),
                },
            ));
        }
        Ok(captured)
    }

    fn refresh_worksheet_session(&self) -> Result<(), WorksheetApplicationError> {
        self.refresh_current_project()
            .map_err(WorksheetApplicationError::SessionRefresh)
    }
}
