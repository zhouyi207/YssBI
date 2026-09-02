//! Authoritative project state for normalized node-system graph documents.

use crate::{
    PreparedProjectActivation, ProjectSession, ProjectStore, ProjectTransactionContext,
    ResourceRenameOwnershipLease, load_project_graph_from_file,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, RwLock};
use yss_chart_document::{ChartDocument, ChartResourcePath};
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
    ProjectGraphSaveError, ProjectHistoryStatus,
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
