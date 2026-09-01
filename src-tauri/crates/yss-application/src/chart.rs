use thiserror::Error;

use super::events::{CommittedResourceMutation, committed_resource_mutation_from_project};
use super::execution::session_slot::{
    ApplicationSessionRefreshError, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use yss_chart_document::{ChartDocument, ChartResourcePath};
use yss_project_filesystem::ProjectFilesystemError;
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};
use yss_resource_naming::ResourceName;

#[derive(Debug, Error)]
pub enum ChartApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error(transparent)]
    Project(#[from] ProjectFilesystemError),
    #[error("captured application session changed during chart operation")]
    SessionChanged(#[source] SessionRevalidationError),
    #[error("application session refresh failed")]
    SessionRefresh(#[source] ApplicationSessionRefreshError),
}

impl ApplicationState {
    pub fn create_chart_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        operation_id: OperationId,
        name: String,
        database_id: Option<String>,
    ) -> Result<CommittedResourceMutation, ChartApplicationError> {
        let captured = self.capture_chart_session(&project_instance_id)?;
        let name = ResourceName::parse(&name).map_err(ProjectFilesystemError::from)?;
        let result = captured.project().create_chart_resource(
            &project_instance_id,
            &name,
            database_id,
            operation_id,
        )?;
        self.refresh_chart_session()?;
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn duplicate_chart_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        operation_id: OperationId,
        chart_path: ChartResourcePath,
        expected_revision: ResourceRevision,
    ) -> Result<CommittedResourceMutation, ChartApplicationError> {
        let captured = self.capture_chart_session(&project_instance_id)?;
        let result = captured.project().duplicate_chart_resource(
            &project_instance_id,
            &chart_path,
            expected_revision,
            operation_id,
        )?;
        self.refresh_chart_session()?;
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn load_chart_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        chart_path: ChartResourcePath,
    ) -> Result<ChartDocument, ChartApplicationError> {
        let captured = self.capture_chart_session(&project_instance_id)?;
        let result = captured
            .project()
            .load_chart_document(&project_instance_id, &chart_path)?;
        self.revalidate_captured_session(&captured)
            .map_err(ChartApplicationError::SessionChanged)?;
        Ok(result)
    }

    pub fn save_chart_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        operation_id: OperationId,
        chart_path: ChartResourcePath,
        expected_revision: ResourceRevision,
        document: ChartDocument,
    ) -> Result<CommittedResourceMutation, ChartApplicationError> {
        let captured = self.capture_chart_session(&project_instance_id)?;
        let result = captured.project().save_chart_document(
            &project_instance_id,
            &chart_path,
            expected_revision,
            operation_id,
            document,
        )?;
        self.refresh_chart_session()?;
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn rename_chart_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        operation_id: OperationId,
        chart_path: ChartResourcePath,
        expected_revision: ResourceRevision,
        new_name: String,
        lifecycle_token: u64,
    ) -> Result<CommittedResourceMutation, ChartApplicationError> {
        let captured = self.capture_chart_session(&project_instance_id)?;
        let new_name = ResourceName::parse(&new_name).map_err(ProjectFilesystemError::from)?;
        let result = captured.project().rename_chart_resource(
            &project_instance_id,
            &chart_path,
            expected_revision,
            &new_name,
            lifecycle_token,
            operation_id,
        )?;
        self.refresh_chart_session()?;
        Ok(committed_resource_mutation_from_project(result))
    }

    pub fn remove_chart_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        operation_id: OperationId,
        chart_path: ChartResourcePath,
        expected_revision: ResourceRevision,
    ) -> Result<CommittedResourceMutation, ChartApplicationError> {
        let captured = self.capture_chart_session(&project_instance_id)?;
        let result = captured.project().remove_chart_resource(
            &project_instance_id,
            &chart_path,
            expected_revision,
            operation_id,
        )?;
        self.refresh_chart_session()?;
        Ok(committed_resource_mutation_from_project(result))
    }

    fn capture_chart_session(
        &self,
        project_instance_id: &ProjectInstanceId,
    ) -> Result<std::sync::Arc<super::execution::ApplicationSession>, ChartApplicationError> {
        let captured = self.capture_session()?;
        if captured.project_instance_id() != project_instance_id {
            return Err(ChartApplicationError::Project(
                ProjectFilesystemError::StaleProjectLifecycle {
                    message: "chart project instance is stale".into(),
                },
            ));
        }
        Ok(captured)
    }

    fn refresh_chart_session(&self) -> Result<(), ChartApplicationError> {
        self.refresh_current_project()
            .map_err(ChartApplicationError::SessionRefresh)
    }
}
