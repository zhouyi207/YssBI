use std::sync::Arc;

use thiserror::Error;

use super::execution::session_slot::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use crate::database::schema_snapshot::DatabaseSchemaFact;
use crate::database_contract::DatabaseDecl;
use crate::project::{
    ProjectError, ProjectFilesystemError, ProjectIndex, ProjectInstanceId,
    RevealProjectResourceRequest, format_path_for_user_path, normalize_existing_path,
    resolve_reveal_path,
};
use crate::variable::VariableInstance;

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
    #[error("database catalog could not be read")]
    Database(#[from] crate::database::error::DatabaseError),
    #[error("project resource reference is invalid")]
    InvalidResourceReference,
    #[error("project resource was not found")]
    ResourceNotFound,
    #[error("captured application session changed during project query")]
    SessionChanged(#[source] SessionRevalidationError),
}

#[derive(Debug, Clone)]
pub struct ProjectActivation {
    pub path: String,
    pub project_instance_id: ProjectInstanceId,
    pub activation_revision: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectDatabaseQueryFact {
    pub(crate) declaration: DatabaseDecl,
    pub(crate) schema: DatabaseSchemaFact,
}

#[derive(Debug, Clone)]
pub(crate) struct ProjectDatabasesVariablesSnapshot {
    databases: Box<[ProjectDatabaseQueryFact]>,
    variables: Box<[VariableInstance]>,
}

impl ProjectDatabasesVariablesSnapshot {
    pub(crate) fn databases(&self) -> &[ProjectDatabaseQueryFact] {
        &self.databases
    }

    pub(crate) fn variables(&self) -> &[VariableInstance] {
        &self.variables
    }
}

impl ApplicationState {
    pub fn query_project_databases_variables(
        &self,
    ) -> Result<ProjectDatabasesVariablesSnapshot, ProjectQueryApplicationError> {
        let captured = self.capture_session()?;
        let data = captured.project().get_data()?;
        let catalog = crate::database::session_api::catalog_snapshot(captured.database())?;
        let databases = data
            .databases
            .iter()
            .map(|(id, declaration)| {
                let schema = catalog
                    .schemas()
                    .iter()
                    .find(|schema| schema.database().as_str() == id)
                    .cloned()
                    .unwrap_or_else(|| DatabaseSchemaFact::empty(declaration.id.clone(), 0, 0));
                ProjectDatabaseQueryFact {
                    declaration: declaration.clone(),
                    schema,
                }
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let variables = data
            .variables
            .into_values()
            .collect::<Vec<_>>()
            .into_boxed_slice();
        crate::database::session_api::revalidate_catalog_snapshot(captured.database(), &catalog)?;
        self.revalidate_captured_session(&captured)
            .map_err(ProjectQueryApplicationError::SessionChanged)?;
        Ok(ProjectDatabasesVariablesSnapshot {
            databases,
            variables,
        })
    }

    pub fn query_current_project_activation(
        &self,
    ) -> Result<ProjectActivation, ProjectQueryApplicationError> {
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
            project_instance_id: captured.project_instance_id().clone(),
            activation_revision,
        })
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
