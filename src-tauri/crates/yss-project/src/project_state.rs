//! Authoritative project state for normalized node-system graph documents.

use crate::{
    PreparedProjectActivation, ProjectSession, ProjectStore, ProjectTransactionContext,
    ResourceRenameOwnershipLease, load_project_graph_from_file,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, RwLock};
use yss_chart_document::{ChartDocument, ChartResourcePath};
use yss_computation_settings::{
    ComputationSettingsMutationReceipt, ComputationSettingsMutationRequest,
    ComputationSettingsSnapshot,
};
use yss_graph_document::GraphResourcePath;
use yss_project_filesystem::{
    NormalizedProjectRoot, ProjectFilesystemCoordinator, ProjectFilesystemError,
    ProjectFilesystemTransaction, StagedFilesystemMutation,
};
use yss_project_history::{
    HistoryMutation, HistoryStatusDto, MutationRequest, ProjectDocumentState, ProjectHistory,
    ProjectHistoryMutationError, ProjectHistoryTransaction, ResourceKey,
};
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::{HistoryEntryId, OperationId, ResourceRevision};
use yss_project_manifest::ProjectManifest;
use yss_project_model::{GraphResourceDocument, ProjectData};
use yss_resource_lifecycle::ResourceLifecycleRegistry;

mod activation;
mod authority;
#[path = "project_state/graph_lifecycle.rs"]
mod graph_lifecycle;
mod graph_operation;
mod history;
mod lifecycle;
mod resource_history;
mod resource_patch;
mod variable_effects;
#[allow(unused_imports)]
pub(super) use activation::PublishedProjectActivation;
pub(super) use authority::{
    ActivationGenerationTransition, MutationPublication, PreparedPublicationAdvance,
    ProjectAuthorityExpectation, ProjectAuthoritySnapshot, VariableStagingBasis,
};
pub(crate) use authority::{VariablePresence, VariableRevisionEntry};
pub use graph_operation::{
    GraphCommitReceipt, GraphInvalidationSet, GraphOperationAuthority, GraphOperationCapture,
    ProjectGraphCommitError, ProjectGraphOperationError, ProjectGraphOperationSource,
    ProjectHistoryStatus,
};
use history::GraphMoveHistoryPayload;
pub(super) use history::{project_documents, replace_project_documents};
use resource_history::{
    affected_projection_paths, authoritative_function_revision,
    canonical_resource_lifecycle_events, chart_history_publication, checked_graph_revision,
    normalize_function_patch_revisions, patch_projection_paths, preflight_resource_patch_graphs,
    validate_chart_path_insertion, variable_scope_references_path,
};
pub(super) use resource_history::{checked_resource_revision, validate_context_revisions};
use resource_patch::CommittedResourceMutation;
pub(super) use variable_effects::{
    install_variable_effect_snapshots, validate_variable_effect_document,
    variable_effect_filesystem_mutations, variable_history_scope, variable_scope_graph_path,
};

mod state;
pub use state::ProjectState;

#[cfg(any(test, feature = "test-support"))]
mod test_support;
#[cfg(any(test, feature = "test-support"))]
use test_support::ActivationPublicationTestHook;
#[cfg(any(test, feature = "test-support"))]
pub use test_support::ProjectActivationTestHook;
#[cfg(any(test, feature = "test-support"))]
use test_support::ProjectStateTestHooks;
#[cfg(test)]
use test_support::{GraphLoadAfterReadTestHook, VariableStagingTestHook};

type ActivationPanicPayload = Box<dyn std::any::Any + Send + 'static>;

#[cfg(any(test, feature = "test-support"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VariableRevisionTestSnapshot {
    pub revision: ResourceRevision,
    pub present: bool,
}

impl ProjectState {
    pub fn get_data(&self) -> Result<ProjectData, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        Ok(self.project_data.read().unwrap().clone())
    }

    pub fn get_computation_settings(
        &self,
    ) -> Result<ComputationSettingsSnapshot, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let publication = self.mutation_publication.lock().unwrap();
        let data = self.project_data.read().unwrap();
        Ok(ComputationSettingsSnapshot {
            project_instance_id: ProjectInstanceId::from_existing(
                publication.project_instance_id.clone(),
            ),
            settings_revision: publication.computation_settings_revision,
            publication_revision: publication.resource_revision,
            settings: data.computation_settings.clone(),
        })
    }

    pub fn update_computation_settings_transaction(
        &self,
        request: ComputationSettingsMutationRequest,
    ) -> Result<ComputationSettingsMutationReceipt, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        request.settings.validate().map_err(|error| {
            ProjectFilesystemError::TransactionPrepareFailed {
                message: error.to_string(),
            }
        })?;
        let session = self.capture_project_session()?;
        if session.instance_id != request.project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "computation settings request belongs to another project".into(),
            });
        }
        let (authority_generation, current_revision, mut next_data) = {
            let publication = self.mutation_publication.lock().unwrap();
            let data = self.project_data.read().unwrap();
            if publication.computation_settings_revision != request.expected_revision {
                return Err(ProjectFilesystemError::ResourceRevisionConflict {
                    message: format!(
                        "expected computation settings revision {}, current revision is {}",
                        request.expected_revision, publication.computation_settings_revision
                    ),
                });
            }
            (
                publication.authority_generation(),
                publication.computation_settings_revision,
                data.clone(),
            )
        };
        next_data.computation_settings = request.settings.clone();
        let contents =
            crate::project_io::serialize_project_manifest(&next_data).map_err(|error| {
                ProjectFilesystemError::TransactionPrepareFailed {
                    message: error.to_string(),
                }
            })?;
        let lease = self.filesystem.acquire(session.root.clone())?;
        let context = ProjectTransactionContext {
            session: session.clone(),
            operation_id: request.operation_id,
            affected_resources: Vec::new(),
            expected_revisions: Default::default(),
            expected_absent_resources: Default::default(),
            recovery_marker: Some(self.project_recovery_marker()),
        };
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.filesystem_context(),
            lease,
            vec![StagedFilesystemMutation::Write {
                relative_path: yss_project_layout::PROJECT_METADATA_FILE.into(),
                contents,
            }],
            |_, staged| {
                serde_json::from_slice::<ProjectManifest>(staged)
                    .map(|_| ())
                    .map_err(|error| error.to_string())
            },
        )?;
        self.validate_project_session(&session)?;
        let committed = prepared.commit()?;

        let publication_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut publication = self.mutation_publication.lock().unwrap();
            let mut data = self.project_data.write().unwrap();
            self.ensure_project_operational()?;
            if publication.project_instance_id != request.project_instance_id.as_str()
                || publication.authority_generation() != authority_generation
            {
                return Err(ProjectFilesystemError::StaleProjectLifecycle {
                    message: "project authority changed during computation settings commit".into(),
                });
            }
            if publication.computation_settings_revision != current_revision {
                return Err(ProjectFilesystemError::ResourceRevisionConflict {
                    message: "computation settings changed during commit".into(),
                });
            }
            let next_settings_revision = current_revision
                .checked_add(1)
                .ok_or(ProjectFilesystemError::ComputationSettingsRevisionExhausted)?;
            let publication_advance = publication.prepare_resource_revision()?;
            data.computation_settings = request.settings.clone();
            publication.computation_settings_revision = next_settings_revision;
            let publication_revision = publication.commit_prepared(publication_advance);
            Ok(ComputationSettingsMutationReceipt {
                project_instance_id: request.project_instance_id.clone(),
                operation_id: request.operation_id,
                settings_revision: publication.computation_settings_revision,
                publication_revision,
                settings: request.settings.clone(),
            })
        }));

        match publication_result {
            Ok(Ok(receipt)) => {
                committed.finalize();
                Ok(receipt)
            }
            Ok(Err(error)) => {
                committed.rollback()?;
                Err(error)
            }
            Err(_) => {
                committed.rollback()?;
                Err(ProjectFilesystemError::TransactionCommitFailed {
                    message: "computation settings authority publication failed".into(),
                })
            }
        }
    }

    pub(super) fn capture_variable_staging_basis(
        &self,
        publication: &MutationPublication,
    ) -> Result<VariableStagingBasis, ProjectFilesystemError> {
        let identity = self.activation_identity.read().unwrap();
        let root = identity.project_root.clone().ok_or_else(|| {
            ProjectFilesystemError::StaleProjectLifecycle {
                message: "no project is active while staging variable mutation".into(),
            }
        })?;
        Ok(VariableStagingBasis {
            session: ProjectSession {
                instance_id: identity.project_instance_id.clone(),
                root,
            },
            authority_generation: publication.authority_generation(),
        })
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn validate_variable_staging_basis(
        &self,
        publication: &MutationPublication,
        basis: &VariableStagingBasis,
    ) -> Result<(), ProjectFilesystemError> {
        let identity = self.activation_identity.read().unwrap();
        if publication.authority_generation() != basis.authority_generation
            || publication.project_instance_id != basis.session.instance_id.as_str()
            || identity.project_instance_id != basis.session.instance_id
            || identity.project_root.as_ref() != Some(&basis.session.root)
        {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project session or authority changed while staging variable mutation"
                    .into(),
            });
        }
        Ok(())
    }

    pub fn project_instance_id(&self) -> String {
        self.mutation_publication
            .lock()
            .unwrap()
            .project_instance_id
            .clone()
    }

    pub fn project_session_id(&self) -> yss_project_identity::ProjectSessionId {
        self.project_store
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .project_session_id
            .clone()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn history_status(&self) -> HistoryStatusDto {
        let _publication = self.mutation_publication.lock().unwrap();
        self.history.read().unwrap().status()
    }

    pub fn history_status_for_project(
        &self,
        project_instance_id: &ProjectInstanceId,
    ) -> Result<HistoryStatusDto, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != project_instance_id.as_str() {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "caller project changed before History status read".into(),
            });
        }
        Ok(self.history.read().unwrap().status())
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn run_project_activation_test_hook(&self) {
        if let Some(hook) = self
            .test_hooks
            .project_activation_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(any(test, feature = "test-support")))]
    pub(super) fn run_project_activation_test_hook(&self) {}

    #[cfg(any(test, feature = "test-support"))]
    fn run_activation_store_replaced_test_hook(&self) -> Option<ActivationPanicPayload> {
        self.test_hooks
            .activation_store_replaced_test_hook
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .and_then(|hook| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| hook())).err()
            })
    }

    #[cfg(not(any(test, feature = "test-support")))]
    fn run_activation_store_replaced_test_hook(&self) -> Option<ActivationPanicPayload> {
        None
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn run_variable_staging_test_hook(&self) {
        if let Some(hook) = self
            .test_hooks
            .variable_staging_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(test)]
    pub(crate) fn set_graph_load_after_read_test_hook(&self, hook: GraphLoadAfterReadTestHook) {
        *self
            .test_hooks
            .graph_load_after_read_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn run_graph_load_after_read_test_hook(&self) {
        if let Some(hook) = self
            .test_hooks
            .graph_load_after_read_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    pub(super) fn run_graph_load_after_read_test_hook(&self) {}

    #[cfg(test)]
    pub(crate) fn set_variable_staging_test_hook(&self, hook: VariableStagingTestHook) {
        *self.test_hooks.variable_staging_test_hook.write().unwrap() = Some(hook);
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_project_activation_test_hook(&self, hook: ProjectActivationTestHook) {
        *self
            .test_hooks
            .project_activation_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_activation_store_replaced_test_hook(&self, hook: ActivationPublicationTestHook) {
        *self
            .test_hooks
            .activation_store_replaced_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn authority_generation_for_test(&self) -> u64 {
        self.mutation_publication
            .lock()
            .unwrap()
            .authority_generation
    }

    pub fn activation_revision(&self) -> u64 {
        self.activation_generation
            .load(std::sync::atomic::Ordering::Acquire)
            / 2
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn revision_state_for_test(
        &self,
    ) -> (
        std::collections::HashMap<GraphResourcePath, yss_graph_document::GraphRevision>,
        std::collections::HashMap<
            yss_variable_contract::VariableId,
            yss_project_identity::ResourceRevision,
        >,
        std::collections::HashMap<ChartResourcePath, yss_project_identity::ResourceRevision>,
    ) {
        (
            self.graph_revisions.read().unwrap().clone(),
            self.variable_revisions
                .read()
                .unwrap()
                .iter()
                .map(|(id, entry)| (*id, entry.revision))
                .collect(),
            self.chart_revisions.read().unwrap().clone(),
        )
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn variable_revision_snapshot_for_test(
        &self,
        id: &yss_variable_contract::VariableId,
    ) -> Option<VariableRevisionTestSnapshot> {
        self.variable_revisions
            .read()
            .unwrap()
            .get(id)
            .map(|entry| VariableRevisionTestSnapshot {
                revision: entry.revision,
                present: entry.is_present(),
            })
    }

    pub fn chart_creation_snapshot(
        &self,
    ) -> Result<(Vec<String>, Option<String>), ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let data = self.project_data.read().unwrap();
        Ok((
            data.charts
                .keys()
                .map(|path| path.display_name().as_str().to_string())
                .collect(),
            data.databases.keys().next().cloned(),
        ))
    }
}
