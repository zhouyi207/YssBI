use super::*;

impl ProjectState {
    pub fn get_path(&self) -> Option<String> {
        self.project_path.read().unwrap().clone()
    }

    pub(crate) fn filesystem(&self) -> &ProjectFilesystemCoordinator {
        &self.filesystem
    }

    pub fn acquire_filesystem_lease(
        &self,
        root: NormalizedProjectRoot,
    ) -> Result<yss_project_filesystem::ProjectFilesystemLeaseSet, ProjectFilesystemError> {
        self.filesystem.acquire(root)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn filesystem_for_test(&self) -> &ProjectFilesystemCoordinator {
        &self.filesystem
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_project_filesystem_fault(
        &self,
        fault: Option<yss_project_filesystem::ProjectFilesystemFaultPoint>,
    ) {
        self.filesystem.set_project_filesystem_fault(fault);
    }

    pub(crate) fn project_recovery_marker(&self) -> yss_project_filesystem::ProjectRecoveryMarker {
        self.recovery_marker.clone()
    }

    pub fn ensure_project_operational(&self) -> Result<(), ProjectFilesystemError> {
        match self.recovery_marker.error() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub(super) fn ensure_mutation_operational(&self) -> Result<(), ProjectHistoryMutationError> {
        self.ensure_project_operational().map_err(|error| {
            ProjectHistoryMutationError::RecoveryRequired(error.to_string().into())
        })
    }

    pub fn capture_project_session(&self) -> Result<ProjectSession, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let (instance_id, project_path) = {
            let publication = self.mutation_publication.lock().unwrap();
            let path = self.project_path.read().unwrap();
            let project_path =
                path.clone()
                    .ok_or_else(|| ProjectFilesystemError::StaleProjectLifecycle {
                        message: "no project is active".into(),
                    })?;
            (publication.project_instance_id.clone(), project_path)
        };
        let root = NormalizedProjectRoot::from_project_path(project_path)?;
        Ok(ProjectSession {
            instance_id: ProjectInstanceId::from_existing(instance_id),
            root,
        })
    }

    pub fn validate_project_session(
        &self,
        session: &ProjectSession,
    ) -> Result<(), ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let project_path = {
            let publication = self.mutation_publication.lock().unwrap();
            let path = self.project_path.read().unwrap();
            if publication.project_instance_id != session.instance_id.as_str() {
                return Err(ProjectFilesystemError::StaleProjectLifecycle {
                    message: "project instance changed".into(),
                });
            }
            path.clone()
                .ok_or_else(|| ProjectFilesystemError::StaleProjectLifecycle {
                    message: "project was closed".into(),
                })?
        };
        let current_root = NormalizedProjectRoot::from_project_path(project_path)?;
        if current_root != session.root {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project root changed".into(),
            });
        }
        Ok(())
    }

    pub(crate) fn coherent_project_read_snapshot(
        &self,
        session: &ProjectSession,
    ) -> Result<(String, u64, HistoryStatusDto, ProjectData), ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let publication = self.mutation_publication.lock().unwrap();
        let path = self.project_path.read().unwrap();
        let identity = self.activation_identity.read().unwrap();
        if publication.project_instance_id != session.instance_id.as_str()
            || identity.project_instance_id != session.instance_id
            || identity.project_root.as_ref() != Some(&session.root)
            || path.is_none()
        {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project changed before read publication".into(),
            });
        }
        let data = self.project_data.read().unwrap().clone();
        let history = self.history.read().unwrap().status();
        self.ensure_project_operational()?;
        Ok((
            publication.project_instance_id.clone(),
            publication.resource_revision,
            history,
            data,
        ))
    }
}
