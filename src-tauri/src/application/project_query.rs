use std::sync::Arc;

use serde::Serialize;
use thiserror::Error;

use super::execution::session_slot::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use crate::application::database_schema::project_databases_variables;
use crate::event::ProjectActivationResultDto;
use crate::project::{
    ProjectError, ProjectFilesystemError, ProjectIndex, ProjectInstanceId,
    RevealProjectResourceRequest, format_path_for_user_path, normalize_existing_path,
    resolve_reveal_path,
};
use crate::schema::DatabasesVariablesDTO;

#[derive(Debug, Error)]
pub enum ProjectQueryApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("project query belongs to another project instance")]
    ProjectIdentityMismatch { requested: ProjectInstanceId },
    #[error(transparent)]
    Project(#[from] ProjectFilesystemError),
    #[error(transparent)]
    ProjectRead(#[from] ProjectError),
    #[error("project resource reference is invalid")]
    InvalidResourceReference,
    #[error("project resource was not found")]
    ResourceNotFound,
    #[error("captured application session changed during project query")]
    SessionChanged(#[source] SessionRevalidationError),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectActivation {
    pub path: String,
    pub project_instance_id: String,
    pub activation_revision: u64,
}

impl From<ProjectActivation> for ProjectActivationResultDto {
    fn from(value: ProjectActivation) -> Self {
        Self {
            path: value.path,
            project_instance_id: value.project_instance_id,
            activation_revision: value.activation_revision,
        }
    }
}

impl ApplicationState {
    pub fn query_project_databases_variables(
        &self,
    ) -> Result<DatabasesVariablesDTO, ProjectQueryApplicationError> {
        let captured = self.capture_session()?;
        let result = project_databases_variables(captured.project())?;
        self.revalidate_captured_session(&captured)
            .map_err(ProjectQueryApplicationError::SessionChanged)?;
        Ok(result)
    }

    pub fn query_current_project_activation(
        &self,
    ) -> Result<ProjectActivationResultDto, ProjectQueryApplicationError> {
        let captured = self.capture_session()?;
        let activation_revision = captured.project().activation_revision();
        let path = captured.project().get_path().ok_or(
            ProjectQueryApplicationError::ProjectIdentityMismatch {
                requested: captured.project_instance_id().clone(),
            },
        )?;
        let project_session = captured.project().capture_project_session()?;
        captured
            .project()
            .validate_project_session(&project_session)?;
        self.revalidate_captured_session(&captured)
            .map_err(ProjectQueryApplicationError::SessionChanged)?;
        Ok(ProjectActivation {
            path: normalize_existing_path(&path).unwrap_or(path),
            project_instance_id: captured.project_instance_id().to_string(),
            activation_revision,
        }
        .into())
    }

    pub fn query_project_path(&self) -> Result<Option<String>, ProjectQueryApplicationError> {
        let captured = self.capture_session()?;
        let path = captured.project().get_path();
        self.revalidate_captured_session(&captured)
            .map_err(ProjectQueryApplicationError::SessionChanged)?;
        Ok(path.map(|path| normalize_existing_path(&path).unwrap_or(path)))
    }

    pub fn query_project_index(
        &self,
        project_instance_id: ProjectInstanceId,
    ) -> Result<ProjectIndex, ProjectQueryApplicationError> {
        let captured = self.capture_project_session(&project_instance_id)?;
        let result = captured
            .project()
            .read_project_index(&project_instance_id)?;
        self.revalidate_captured_session(&captured)
            .map_err(ProjectQueryApplicationError::SessionChanged)?;
        Ok(result)
    }

    pub fn reveal_project_resource(
        &self,
        kind: String,
        resource_id: String,
    ) -> Result<String, ProjectQueryApplicationError> {
        let captured = self.capture_session()?;
        let request = RevealProjectResourceRequest::from_parts(&kind, resource_id)
            .map_err(|_| ProjectQueryApplicationError::InvalidResourceReference)?;
        let path = resolve_reveal_path(captured.project(), request)?;
        if !path.exists() {
            return Err(ProjectQueryApplicationError::ResourceNotFound);
        }
        self.revalidate_captured_session(&captured)
            .map_err(ProjectQueryApplicationError::SessionChanged)?;
        Ok(format_path_for_user_path(&path))
    }

    fn capture_project_session(
        &self,
        project_instance_id: &ProjectInstanceId,
    ) -> Result<Arc<ApplicationSession>, ProjectQueryApplicationError> {
        let captured = self.capture_session()?;
        if captured.project_instance_id() != project_instance_id {
            return Err(ProjectQueryApplicationError::ProjectIdentityMismatch {
                requested: project_instance_id.clone(),
            });
        }
        Ok(captured)
    }
}
