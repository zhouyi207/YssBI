//! Authoritative project state for normalized node-system graph documents.

use crate::ProjectOperationError;
use crate::{
    PreparedProjectActivation, ProjectSession, ProjectStore, ProjectTransactionContext,
    ResourceRenameOwnershipLease,
};
use std::sync::{Arc, Mutex, RwLock};
use yss_chart_document::{ChartDocument, ChartResourcePath};
use yss_filesystem::{FilesystemCoordinator, NormalizedRoot};
use yss_graph_document::GraphResourcePath;
use yss_project_history::{MutationRequest, ProjectResourceMutationError, ResourceKey};
use yss_project_identity::ProjectInstanceId;
use yss_project_identity::{OperationId, ResourceRevision};
use yss_project_model::{GraphResourceDocument, ProjectData};
use yss_resource_lifecycle::ResourceLifecycleRegistry;

mod activation;
mod authority;
mod function_mutation;
mod graph_editing;
#[path = "project_state/graph_lifecycle.rs"]
mod graph_lifecycle;
mod graph_operation;
mod graph_references;
pub use graph_editing::{
    GraphEditCommandKind, GraphEditCommandReceipt, GraphEditCorrelation, GraphEditVersion,
    GraphEditingSnapshot, GraphEditingState, GraphHistoryAction,
};
mod lifecycle;
mod resource_patch;
mod resource_publication;
pub(super) use authority::{
    ActivationGenerationTransition, MutationPublication, PreparedPublicationAdvance,
    ProjectAuthorityExpectation,
};
pub use graph_operation::{
    GraphCommitReceipt, GraphInvalidationSet, GraphOperationAuthority, GraphOperationCapture,
    ProjectGraphCommitError, ProjectGraphOperationError, ProjectGraphOperationSource,
    ProjectGraphSaveError,
};
use resource_patch::CommittedResourceMutation;
use resource_publication::{
    affected_projection_paths, canonical_resource_lifecycle_events, chart_publication_deltas,
    normalize_function_patch_revisions, normalize_function_resource_revision,
    patch_projection_paths, validate_chart_path_insertion,
};
pub(super) use resource_publication::{checked_resource_revision, validate_context_revisions};

mod state;
pub use state::ProjectState;

#[cfg(any(test, feature = "test-support"))]
mod test_support;
#[cfg(any(test, feature = "test-support"))]
use test_support::ActivationPublicationTestHook;
#[cfg(test)]
use test_support::GraphLoadAfterReadTestHook;
#[cfg(any(test, feature = "test-support"))]
pub use test_support::ProjectActivationTestHook;
#[cfg(any(test, feature = "test-support"))]
use test_support::ProjectStateTestHooks;

type ActivationPanicPayload = Box<dyn std::any::Any + Send + 'static>;

impl ProjectState {
    pub fn get_data(&self) -> Result<ProjectData, ProjectOperationError> {
        self.ensure_project_operational()?;
        Ok(self.project_data.read().unwrap().clone())
    }

    pub fn has_resident_graph(
        &self,
        graph: &GraphResourcePath,
    ) -> Result<bool, ProjectOperationError> {
        self.ensure_project_operational()?;
        Ok(self.project_data.read().unwrap().graphs.contains_key(graph))
    }

    /// Read one resident resource without loading a declared graph from disk.
    pub fn read_resident_graph(
        &self,
        graph: &GraphResourcePath,
    ) -> Result<Option<GraphResourceDocument>, ProjectOperationError> {
        self.ensure_project_operational()?;
        Ok(self.project_data.read().unwrap().graphs.get(graph).cloned())
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
        std::collections::HashMap<GraphResourcePath, yss_project_identity::ResourceRevision>,
        std::collections::HashMap<ChartResourcePath, yss_project_identity::ResourceRevision>,
    ) {
        (
            self.graph_resource_revisions.read().unwrap().clone(),
            self.chart_revisions.read().unwrap().clone(),
        )
    }

    pub fn chart_creation_snapshot(
        &self,
    ) -> Result<(Vec<String>, Option<String>), ProjectOperationError> {
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
