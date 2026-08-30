use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::project) struct ProjectAuthorityExpectation {
    pub(in crate::project) project_instance_id: ProjectInstanceId,
    pub(in crate::project) project_root: Option<NormalizedProjectRoot>,
    pub(in crate::project) project_session_id: yss_project_identity::ProjectSessionId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::project) struct ProjectAuthoritySnapshot {
    pub(in crate::project) project_instance_id: ProjectInstanceId,
    pub(in crate::project) authority_generation: u64,
}

impl ProjectAuthoritySnapshot {
    pub(in crate::project) fn matches_publication(self, publication: &MutationPublication) -> bool {
        self.project_instance_id.as_str() == publication.project_instance_id
            && self.authority_generation == publication.authority_generation()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VariablePresence {
    Present,
    Deleted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VariableRevisionEntry {
    pub(crate) revision: yss_project_identity::ResourceRevision,
    pub(crate) presence: VariablePresence,
}

impl VariableRevisionEntry {
    pub(crate) const fn present(revision: yss_project_identity::ResourceRevision) -> Self {
        Self {
            revision,
            presence: VariablePresence::Present,
        }
    }

    pub(crate) const fn deleted(revision: yss_project_identity::ResourceRevision) -> Self {
        Self {
            revision,
            presence: VariablePresence::Deleted,
        }
    }

    pub(crate) const fn is_present(self) -> bool {
        matches!(self.presence, VariablePresence::Present)
    }
}
pub(in crate::project) struct MutationPublication {
    pub(in crate::project) project_instance_id: String,
    pub(in crate::project) resource_revision: u64,
    pub(in crate::project) authority_generation: u64,
    pub(in crate::project) computation_settings_revision: u64,
}

pub(in crate::project) struct VariableStagingBasis {
    pub(in crate::project) session: ProjectSession,
    pub(in crate::project) authority_generation: u64,
}

#[derive(Clone, Copy)]
pub(in crate::project) struct PreparedPublicationAdvance {
    previous_resource_revision: u64,
    next_resource_revision: u64,
    previous_authority_generation: u64,
    next_authority_generation: u64,
}

impl Default for MutationPublication {
    fn default() -> Self {
        Self {
            project_instance_id: uuid::Uuid::new_v4().to_string(),
            resource_revision: 0,
            authority_generation: 0,
            computation_settings_revision: 0,
        }
    }
}

impl MutationPublication {
    pub(in crate::project) fn authority_generation(&self) -> u64 {
        self.authority_generation
    }

    pub(in crate::project) fn prepare_authority_generation(
        &self,
    ) -> Result<PreparedPublicationAdvance, ProjectFilesystemError> {
        let next_authority_generation = self
            .authority_generation
            .checked_add(1)
            .ok_or(ProjectFilesystemError::AuthorityGenerationExhausted)?;
        Ok(PreparedPublicationAdvance {
            previous_resource_revision: self.resource_revision,
            next_resource_revision: self.resource_revision,
            previous_authority_generation: self.authority_generation,
            next_authority_generation,
        })
    }

    pub(in crate::project) fn prepare_resource_revision(
        &self,
    ) -> Result<PreparedPublicationAdvance, ProjectFilesystemError> {
        let next_resource_revision = self
            .resource_revision
            .checked_add(1)
            .ok_or(ProjectFilesystemError::PublicationRevisionExhausted)?;
        let next_authority_generation = self
            .authority_generation
            .checked_add(1)
            .ok_or(ProjectFilesystemError::AuthorityGenerationExhausted)?;
        Ok(PreparedPublicationAdvance {
            previous_resource_revision: self.resource_revision,
            next_resource_revision,
            previous_authority_generation: self.authority_generation,
            next_authority_generation,
        })
    }

    pub(in crate::project) fn commit_prepared(
        &mut self,
        prepared: PreparedPublicationAdvance,
    ) -> u64 {
        debug_assert_eq!(self.resource_revision, prepared.previous_resource_revision);
        debug_assert_eq!(
            self.authority_generation,
            prepared.previous_authority_generation
        );
        self.resource_revision = prepared.next_resource_revision;
        self.authority_generation = prepared.next_authority_generation;
        prepared.next_resource_revision
    }

    #[cfg(test)]
    pub(in crate::project) fn advance_authority_generation(&mut self) {
        let prepared = self
            .prepare_authority_generation()
            .expect("test authority generation is available");
        self.commit_prepared(prepared);
    }

    #[cfg(test)]
    pub(in crate::project) fn allocate_resource_revision(
        &mut self,
    ) -> Result<u64, ProjectFilesystemError> {
        let prepared = self.prepare_resource_revision()?;
        Ok(self.commit_prepared(prepared))
    }

    pub(in crate::project) fn reset_to(&mut self, project_instance_id: String) -> String {
        let previous = std::mem::replace(&mut self.project_instance_id, project_instance_id);
        self.resource_revision = 0;
        self.authority_generation = 0;
        self.computation_settings_revision = 0;
        previous
    }
}

impl ProjectState {
    pub(in crate::project) fn current_projection_environment_expectation(
        &self,
    ) -> ProjectAuthorityExpectation {
        self.activation_identity
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub(in crate::project) fn projection_environment_expectation_for_identity(
        &self,
        project_instance_id: &str,
        project_root: &NormalizedProjectRoot,
    ) -> Result<ProjectAuthorityExpectation, String> {
        let expected = self.current_projection_environment_expectation();
        if expected.project_instance_id.as_str() != project_instance_id
            || expected.project_root.as_ref() != Some(project_root)
        {
            return Err("stale_project_lifecycle: project changed before authority capture".into());
        }
        Ok(expected)
    }

    pub(in crate::project) fn capture_project_authority_for_session(
        &self,
        session: &ProjectSession,
    ) -> Result<ProjectAuthoritySnapshot, ProjectFilesystemError> {
        let expected = self
            .projection_environment_expectation_for_identity(
                session.instance_id.as_str(),
                &session.root,
            )
            .map_err(|message| ProjectFilesystemError::TransactionPrepareFailed { message })?;
        let generation = self
            .activation_generation
            .load(std::sync::atomic::Ordering::Acquire);
        if generation % 2 != 0 {
            return Err(ProjectFilesystemError::FilesystemTransactionBusy {
                message: "project authority publication is in progress".into(),
            });
        }
        let publication = self
            .mutation_publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if publication.project_instance_id != expected.project_instance_id.as_str()
            || publication.authority_generation != generation
        {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project authority changed during capture".into(),
            });
        }
        Ok(ProjectAuthoritySnapshot {
            project_instance_id: expected.project_instance_id,
            authority_generation: publication.authority_generation(),
        })
    }
}
pub(in crate::project) struct ActivationGenerationTransition {
    generation: Arc<std::sync::atomic::AtomicU64>,
    armed: bool,
}

impl ActivationGenerationTransition {
    pub(in crate::project) fn begin(
        generation: &Arc<std::sync::atomic::AtomicU64>,
    ) -> Result<Self, ProjectFilesystemError> {
        use std::sync::atomic::Ordering;

        let current = generation.load(Ordering::Acquire);
        if current % 2 != 0 {
            return Err(ProjectFilesystemError::FilesystemTransactionBusy {
                message: "project activation publication is already in progress".into(),
            });
        }
        let changing = current
            .checked_add(1)
            .filter(|_| current.checked_add(2).is_some())
            .ok_or(ProjectFilesystemError::ActivationGenerationExhausted)?;
        if generation
            .compare_exchange(current, changing, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(ProjectFilesystemError::FilesystemTransactionBusy {
                message: "project activation publication is already in progress".into(),
            });
        }
        Ok(Self {
            generation: Arc::clone(generation),
            armed: true,
        })
    }

    pub(in crate::project) fn complete(mut self) {
        self.finish_generation();
        self.armed = false;
    }

    fn finish_generation(&self) {
        if self
            .generation
            .fetch_update(
                std::sync::atomic::Ordering::Release,
                std::sync::atomic::Ordering::Relaxed,
                |current| (current % 2 != 0).then(|| current.checked_add(1)).flatten(),
            )
            .is_err()
        {
            tracing::error!(
                target: "yssbi::project::activation",
                diagnostic_domain = "system",
                diagnostic_event = "activationGenerationTransitionInvalid",
                "Project activation generation transition could not be completed"
            );
        }
    }
}

impl Drop for ActivationGenerationTransition {
    fn drop(&mut self) {
        if self.armed {
            self.finish_generation();
            self.armed = false;
        }
    }
}
