use super::ProjectDatabaseError;
use crate::project_state::ProjectAuthorityExpectation;
use crate::{ProjectOperationError, ProjectState};
use yss_database_contract::{
    DatabaseDecl, DatabaseDeclarationFingerprint, DatabaseDeclarationObservation,
    DatabaseDeclarationObservationSet, DatabaseDeclarationRevision,
};
use yss_filesystem::NormalizedRoot;
use yss_project_identity::{ProjectInstanceId, ProjectSessionId};

/// A detached declaration view with the authority that produced it.
pub struct ProjectDatabaseSnapshot {
    identity: ProjectAuthorityExpectation,
    authority_generation: u64,
    declarations: Box<[DatabaseDecl]>,
    observations: DatabaseDeclarationObservationSet,
}

impl ProjectDatabaseSnapshot {
    pub fn project_instance_id(&self) -> &ProjectInstanceId {
        &self.identity.project_instance_id
    }

    pub fn project_session_id(&self) -> &ProjectSessionId {
        &self.identity.project_session_id
    }

    pub fn authority_generation(&self) -> u64 {
        self.authority_generation
    }

    pub fn root(&self) -> Option<&NormalizedRoot> {
        self.identity.project_root.as_ref()
    }

    pub fn declarations(&self) -> &[DatabaseDecl] {
        &self.declarations
    }

    pub fn observations(&self) -> &DatabaseDeclarationObservationSet {
        &self.observations
    }
}

impl ProjectState {
    pub fn read_database_declaration(
        &self,
        project_instance_id: &ProjectInstanceId,
        id: &str,
    ) -> Result<DatabaseDecl, ProjectDatabaseError> {
        self.ensure_project_operational()?;
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != project_instance_id.as_str() {
            return Err(ProjectOperationError::StaleProjectLifecycle {
                message: "project changed before database declaration read".into(),
            }
            .into());
        }
        self.project_data
            .read()
            .unwrap()
            .databases
            .get(id)
            .cloned()
            .ok_or(ProjectDatabaseError::DatabaseNotFound)
    }

    pub fn read_database_snapshot(&self) -> Result<ProjectDatabaseSnapshot, ProjectOperationError> {
        self.ensure_project_operational()?;
        let publication = self.mutation_publication.lock().unwrap();
        let identity = self.activation_identity.read().unwrap();
        if publication.project_instance_id != identity.project_instance_id.as_str() {
            return Err(ProjectOperationError::StaleProjectLifecycle {
                message: "project changed before database declaration capture".into(),
            });
        }
        let data = self.project_data.read().unwrap();
        let revisions = self.database_authority_revisions.read().unwrap();
        let observations = data
            .databases
            .values()
            .map(|declaration| {
                let revision =
                    revisions
                        .get(declaration.id.as_str())
                        .copied()
                        .ok_or_else(|| ProjectOperationError::CatalogResourceStale {
                            message: "database declaration is missing its revision authority"
                                .into(),
                        })?;
                Ok((
                    declaration.id.clone(),
                    DatabaseDeclarationObservation::new(
                        DatabaseDeclarationRevision::from_existing(revision),
                        DatabaseDeclarationFingerprint::from_decl(declaration),
                    ),
                ))
            })
            .collect::<Result<Vec<_>, ProjectOperationError>>()?;
        let observations =
            DatabaseDeclarationObservationSet::try_from_iter(observations).map_err(|_| {
                ProjectOperationError::CatalogResourceStale {
                    message: "database declarations contain conflicting identities".into(),
                }
            })?;
        Ok(ProjectDatabaseSnapshot {
            identity: identity.clone(),
            authority_generation: publication.authority_generation(),
            declarations: data.databases.values().cloned().collect(),
            observations,
        })
    }

    pub fn revalidate_database_snapshot(
        &self,
        snapshot: &ProjectDatabaseSnapshot,
    ) -> Result<(), ProjectOperationError> {
        self.ensure_project_operational()?;
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != snapshot.project_instance_id().as_str() {
            return Err(ProjectOperationError::StaleProjectLifecycle {
                message: "project changed after database declaration capture".into(),
            });
        }
        if publication.authority_generation() != snapshot.authority_generation {
            return Err(ProjectOperationError::CatalogResourceStale {
                message: "database declaration authority changed before publication".into(),
            });
        }
        Ok(())
    }
}
