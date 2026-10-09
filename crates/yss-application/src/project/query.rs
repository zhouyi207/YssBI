use std::{collections::HashMap, path::PathBuf, sync::Arc};

use thiserror::Error;

use crate::session::{
    ApplicationSession, ApplicationState, SessionCaptureError, SessionRevalidationError,
};
use yss_database_contract::DatabaseDecl;
use yss_database_schema::DatabaseSchemaFact;
use yss_project::ProjectOperationError;
use yss_project::{ProjectError, ProjectIndex, RevealProjectResourceRequest, resolve_reveal_path};
use yss_project_identity::ProjectInstanceId;
use yss_project_registry::normalize_existing_path;

#[cfg(test)]
mod tests;

#[derive(Debug, Error)]
pub enum ProjectQueryApplicationError {
    #[error(transparent)]
    SessionCapture(#[from] SessionCaptureError),
    #[error("project query belongs to another project instance")]
    ProjectIdentityMismatch { requested: ProjectInstanceId },
    #[error(transparent)]
    Project(#[from] ProjectOperationError),
    #[error(transparent)]
    ProjectRead(#[from] ProjectError),
    #[error(transparent)]
    Catalog(#[from] crate::graph::catalog::CatalogQueryApplicationError),
    #[error("database catalog could not be read")]
    Database(#[from] yss_database_runtime::error::DatabaseError),
    #[error("project resource was not found")]
    ResourceNotFound,
    #[error("captured application session changed during project query")]
    SessionChanged(#[source] SessionRevalidationError),
}

#[derive(Debug)]
pub struct ProjectIndexSnapshot {
    pub index: ProjectIndex,
    pub activity_panels: Vec<crate::activity_panel::ActivityPanelDocument>,
}

#[derive(Debug, Clone)]
pub struct ProjectActivation {
    pub path: String,
    pub project_instance_id: ProjectInstanceId,
    pub activation_revision: u64,
}

#[derive(Debug, Clone)]
pub struct ProjectDatabaseQueryFact {
    pub declaration: DatabaseDecl,
    pub schema: DatabaseSchemaFact,
}

#[derive(Debug, Clone)]
pub struct ProjectDatabasesSnapshot {
    databases: Box<[ProjectDatabaseQueryFact]>,
}

impl ProjectDatabasesSnapshot {
    pub fn databases(&self) -> &[ProjectDatabaseQueryFact] {
        &self.databases
    }
}

impl ApplicationState {
    pub fn query_project_databases(
        &self,
        project_instance_id: ProjectInstanceId,
        expected_publication_revision: u64,
    ) -> Result<ProjectDatabasesSnapshot, ProjectQueryApplicationError> {
        let captured = self.capture_project_session(&project_instance_id)?;
        self.query_project_databases_in_session(&captured, Some(expected_publication_revision))
    }

    pub(crate) fn query_project_databases_in_session(
        &self,
        captured: &Arc<ApplicationSession>,
        expected_publication_revision: Option<u64>,
    ) -> Result<ProjectDatabasesSnapshot, ProjectQueryApplicationError> {
        self.revalidate_captured_session(captured)
            .map_err(ProjectQueryApplicationError::SessionChanged)?;
        let declarations = captured.project().read_database_snapshot()?;
        if declarations.project_instance_id() != captured.project_instance_id()
            || declarations.project_session_id() != captured.project_session_id()
        {
            return Err(ProjectQueryApplicationError::ProjectIdentityMismatch {
                requested: captured.project_instance_id().clone(),
            });
        }
        let revalidate_project = || match expected_publication_revision {
            Some(revision) => captured.project().validate_project_index_version(
                captured.project_instance_id(),
                revision,
                declarations.authority_generation(),
            ),
            None => captured
                .project()
                .revalidate_database_snapshot(&declarations),
        };
        revalidate_project()?;
        let catalog = yss_database_runtime::session_api::catalog_snapshot(captured.database())?;
        yss_database_runtime::session_api::revalidate_declaration_observations(
            captured.database(),
            declarations.observations(),
        )?;
        let schemas = catalog
            .schemas()
            .iter()
            .map(|schema| (schema.database(), schema))
            .collect::<HashMap<_, _>>();
        let databases = declarations
            .declarations()
            .iter()
            .map(|declaration| {
                let schema = schemas
                    .get(&declaration.id)
                    .copied()
                    .cloned()
                    .ok_or_else(|| ProjectOperationError::CatalogResourceStale {
                        message: "database declaration is missing its runtime schema".into(),
                    })?;
                Ok(ProjectDatabaseQueryFact {
                    declaration: declaration.clone(),
                    schema,
                })
            })
            .collect::<Result<Vec<_>, ProjectOperationError>>()?
            .into_boxed_slice();
        yss_database_runtime::session_api::revalidate_catalog_snapshot(
            captured.database(),
            &catalog,
        )?;
        revalidate_project()?;
        self.revalidate_captured_session(captured)
            .map_err(ProjectQueryApplicationError::SessionChanged)?;
        Ok(ProjectDatabasesSnapshot { databases })
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

    pub fn query_project_path(
        &self,
        project_instance_id: ProjectInstanceId,
    ) -> Result<Option<String>, ProjectQueryApplicationError> {
        let captured = self.capture_project_session(&project_instance_id)?;
        let path = captured.project().get_path();
        self.revalidate_captured_session(&captured)
            .map_err(ProjectQueryApplicationError::SessionChanged)?;
        Ok(path.map(|path| normalize_existing_path(&path).unwrap_or(path)))
    }

    pub fn query_project_index(
        &self,
        project_instance_id: ProjectInstanceId,
        locale: &str,
        include_nodes: bool,
    ) -> Result<ProjectIndexSnapshot, ProjectQueryApplicationError> {
        let captured = self.capture_project_session(&project_instance_id)?;
        let index = captured
            .project()
            .read_project_index(&project_instance_id)?;
        let mut activity_panels = vec![crate::activity_panel::project_activity_panel(Some(&index))];
        if include_nodes {
            let facts =
                crate::graph::catalog::localized_project_facts_from_index(&captured, index.clone())
                    .map_err(crate::graph::catalog::CatalogQueryApplicationError::from)?;
            let catalog = crate::graph::catalog::localized_node_catalog_from_facts(
                self, &captured, facts, locale,
            )?;
            activity_panels.push(crate::activity_panel::nodes_activity_panel_from_catalog(
                catalog,
            ));
        }
        captured.project().validate_project_index_version(
            &project_instance_id,
            index.publication_revision,
            index.authority_generation(),
        )?;
        self.revalidate_captured_session(&captured)
            .map_err(ProjectQueryApplicationError::SessionChanged)?;
        Ok(ProjectIndexSnapshot {
            index,
            activity_panels,
        })
    }

    pub fn reveal_project_resource(
        &self,
        project_instance_id: ProjectInstanceId,
        request: RevealProjectResourceRequest,
    ) -> Result<PathBuf, ProjectQueryApplicationError> {
        let captured = self.capture_project_session(&project_instance_id)?;
        let path = resolve_reveal_path(captured.project(), request)?;
        if !path.exists() {
            return Err(ProjectQueryApplicationError::ResourceNotFound);
        }
        self.revalidate_captured_session(&captured)
            .map_err(ProjectQueryApplicationError::SessionChanged)?;
        Ok(dunce::simplified(&path).to_path_buf())
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
