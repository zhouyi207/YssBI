use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VariablePresence {
    Present,
    Deleted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct VariableRevisionEntry {
    pub(crate) revision: crate::node_system::document::ResourceRevision,
    pub(crate) presence: VariablePresence,
}

impl VariableRevisionEntry {
    pub(crate) const fn present(revision: crate::node_system::document::ResourceRevision) -> Self {
        Self {
            revision,
            presence: VariablePresence::Present,
        }
    }

    pub(crate) const fn deleted(revision: crate::node_system::document::ResourceRevision) -> Self {
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
