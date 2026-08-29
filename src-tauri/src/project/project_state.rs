//! Authoritative project state for normalized node-system graph documents.

use crate::graph_document::GraphResourcePath;
use crate::graph_document::NodeId;
use crate::project::resource_patch::ResourceDocumentPatch;
use crate::project::{
    GraphResourceDocument, NormalizedProjectRoot, PreparedProjectActivation, ProjectData,
    ProjectFilesystemCoordinator, ProjectFilesystemError, ProjectFilesystemTransaction,
    ProjectInstanceId, ProjectSession, ProjectStore, ProjectTransactionContext,
    ResourceLifecycleIntent, ResourceLifecycleOperation, ResourceLifecycleRegistry,
    ResourceRenameOwnershipLease, StagedFilesystemMutation, WorksheetResourcePath,
    load_project_graph_from_file,
};
use crate::project::{HistoryEntryId, OperationId, ResourceRevision};
use crate::project::{
    HistoryMutation, HistoryStatusDto, MutationRequest, ProjectDocumentState, ProjectHistory,
    ProjectHistoryMutationError, ProjectHistoryTransaction, ResourceKey,
};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, RwLock};

mod activation;
mod authority;
#[path = "project_state/graph_lifecycle_application.rs"]
mod graph_lifecycle;
pub(crate) mod graph_operation;
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
use history::GraphMoveHistoryPayload;
pub(super) use history::{project_documents, replace_project_documents};
use resource_history::{
    affected_projection_paths, authoritative_function_revision,
    canonical_resource_lifecycle_events, checked_graph_revision, graph_document_references_path,
    normalize_function_patch_revisions, normalize_loaded_function_resource_revision,
    patch_projection_paths, preflight_resource_patch_graphs, validate_graph_resource,
    validate_worksheet_path_insertion, variable_scope_references_path,
    worksheet_history_publication,
};
pub(super) use resource_history::{
    checked_resource_revision, normalize_function_resource_revision, validate_context_revisions,
};
use resource_patch::CommittedResourceMutation;
#[cfg(test)]
pub(super) use variable_effects::variable_effect_run_error;
#[cfg(test)]
pub(super) use variable_effects::{VariableEffectCommitError, VariableEffectCommitResult};
pub(super) use variable_effects::{
    install_variable_effect_snapshots, validate_variable_effect_document,
    variable_effect_filesystem_mutations, variable_history_scope, variable_scope_graph_path,
};

mod state;
pub use state::ProjectState;

#[cfg(test)]
mod test_support;
#[cfg(test)]
pub(crate) use test_support::ProjectActivationTestHook;
#[cfg(test)]
use test_support::ProjectStateTestHooks;
#[cfg(test)]
use test_support::{
    ActivationPublicationTestHook, CommittedResourceCompletionTestHook, CompilePublicationTestHook,
    ComputationSettingsPublicationTestHook, DurableHistoryTestHook, ExecutionTestHook,
    LifecycleLockTestHook, MutationPublicationTestHook, ProductionRelationalBackendFactory,
    ProjectionEnvironmentCaptureTestHook, ProjectionTestHook,
    VariableAuthorityAssignmentPanicTestHook, VariableStagingTestHook,
};

type ActivationPanicPayload = Box<dyn std::any::Any + Send + 'static>;

impl ProjectState {
    #[cfg(test)]
    pub(crate) fn current_run_registry(
        &self,
    ) -> (
        std::sync::Arc<crate::node_system::runtime::ProjectRunRegistry>,
        crate::project::ProjectSessionId,
    ) {
        let store = self
            .project_store
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (
            std::sync::Arc::clone(&store.runs),
            store.project_session_id.clone(),
        )
    }

    pub fn get_data(&self) -> Result<ProjectData, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        Ok(self.project_data.read().unwrap().clone())
    }

    pub fn get_computation_settings(
        &self,
    ) -> Result<crate::project::ComputationSettingsSnapshot, ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let publication = self.mutation_publication.lock().unwrap();
        let data = self.project_data.read().unwrap();
        Ok(crate::project::ComputationSettingsSnapshot {
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
        request: crate::project::ComputationSettingsMutationRequest,
    ) -> Result<crate::project::ComputationSettingsMutationReceipt, ProjectFilesystemError> {
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
        let contents = crate::project::project_io::serialize_project_manifest(&next_data).map_err(
            |error| ProjectFilesystemError::TransactionPrepareFailed {
                message: error.to_string(),
            },
        )?;
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
            context,
            lease,
            vec![StagedFilesystemMutation::Write {
                relative_path: crate::project::PROJECT_METADATA_FILE.into(),
                contents,
            }],
            |_, staged| {
                serde_json::from_slice::<crate::project::project_io::ProjectManifest>(staged)
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
            #[cfg(test)]
            if let Some(hook) = self
                .test_hooks
                .computation_settings_publication_test_hook
                .write()
                .unwrap()
                .take()
            {
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| hook())).is_err() {
                    return Err(ProjectFilesystemError::TransactionCommitFailed {
                        message: "computation settings authority publication failed".into(),
                    });
                }
            }
            let next_settings_revision = current_revision
                .checked_add(1)
                .ok_or(ProjectFilesystemError::ComputationSettingsRevisionExhausted)?;
            let publication_advance = publication.prepare_resource_revision()?;
            data.computation_settings = request.settings.clone();
            publication.computation_settings_revision = next_settings_revision;
            let publication_revision = publication.commit_prepared(publication_advance);
            Ok(crate::project::ComputationSettingsMutationReceipt {
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

    #[cfg(test)]
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

    #[cfg(test)]
    pub(crate) fn history_status(&self) -> HistoryStatusDto {
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

    #[cfg(test)]
    pub(crate) fn history_lengths_for_test(&self) -> (usize, usize) {
        let _publication = self.mutation_publication.lock().unwrap();
        let history = self.history.read().unwrap();
        (history.undo_len(), history.redo_len())
    }

    #[cfg(test)]
    pub(crate) fn history_head_id_for_test(&self, undo: bool) -> Option<HistoryEntryId> {
        let _publication = self.mutation_publication.lock().unwrap();
        let history = self.history.read().unwrap();
        if undo {
            history.next_undo()
        } else {
            history.next_redo()
        }
        .map(|transaction| transaction.history_id)
    }

    #[cfg(test)]
    fn run_mutation_publication_test_hook(&self) {
        if let Some(hook) = self
            .test_hooks
            .mutation_publication_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    fn run_mutation_publication_test_hook(&self) {}

    #[cfg(test)]
    fn run_history_after_routing_test_hook(&self) {
        if let Some(hook) = self
            .test_hooks
            .history_after_routing_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    fn run_history_after_routing_test_hook(&self) {}

    #[cfg(test)]
    fn run_history_after_preparation_test_hook(&self) {
        if let Some(hook) = self
            .test_hooks
            .history_after_preparation_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    fn run_history_after_preparation_test_hook(&self) {}

    #[cfg(test)]
    fn run_history_after_disk_commit_test_hook(&self) {
        if let Some(hook) = self
            .test_hooks
            .history_after_disk_commit_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    fn run_history_after_disk_commit_test_hook(&self) {}

    #[cfg(test)]
    fn run_catalog_mutation_before_publication_test_hook(&self) {
        if let Some(hook) = self
            .test_hooks
            .catalog_mutation_before_publication_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    fn run_catalog_mutation_before_publication_test_hook(&self) {}

    #[cfg(test)]
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

    #[cfg(not(test))]
    pub(super) fn run_project_activation_test_hook(&self) {}

    #[cfg(test)]
    fn run_activation_publication_panic_test_hook(&self) -> Option<ActivationPanicPayload> {
        self.test_hooks
            .activation_publication_panic_test_hook
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .and_then(|hook| {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| hook())).err()
            })
    }

    #[cfg(not(test))]
    fn run_activation_publication_panic_test_hook(&self) -> Option<ActivationPanicPayload> {
        None
    }

    #[cfg(test)]
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

    #[cfg(not(test))]
    fn run_activation_store_replaced_test_hook(&self) -> Option<ActivationPanicPayload> {
        None
    }

    #[cfg(test)]
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
    pub(super) fn run_activation_preparation_after_read_test_hook(&self) {
        if let Some(hook) = self
            .test_hooks
            .activation_preparation_after_read_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    pub(super) fn run_activation_preparation_after_read_test_hook(&self) {}

    #[cfg(test)]
    pub(crate) fn set_production_relational_observer(
        &self,
        observer: Arc<crate::node_system::runtime::ProductionRelationalObserver>,
    ) {
        *self
            .test_hooks
            .production_relational_observer
            .write()
            .unwrap() = Some(observer);
    }

    #[cfg(test)]
    pub(crate) fn set_production_relational_backend_factory(
        &self,
        factory: ProductionRelationalBackendFactory,
    ) {
        *self
            .test_hooks
            .production_relational_backend_factory
            .write()
            .unwrap() = Some(factory);
    }

    #[cfg(test)]
    pub(crate) fn set_project_resource_lease_observer(
        &self,
        observer: crate::node_system::runtime::ProjectResourceLeaseObserver,
    ) {
        *self
            .test_hooks
            .project_resource_lease_observer
            .write()
            .unwrap() = Some(observer);
    }

    #[cfg(test)]
    pub(crate) fn set_projection_test_hook(&self, hook: ProjectionTestHook) {
        *self.test_hooks.projection_test_hook.write().unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn set_committed_resource_completion_test_hook(
        &self,
        hook: CommittedResourceCompletionTestHook,
    ) {
        *self
            .test_hooks
            .committed_resource_completion_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn set_projection_environment_capture_test_hook(
        &self,
        hook: ProjectionEnvironmentCaptureTestHook,
    ) {
        *self
            .test_hooks
            .projection_environment_capture_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn set_projection_environment_after_path_data_test_hook(
        &self,
        hook: ProjectionEnvironmentCaptureTestHook,
    ) {
        *self
            .test_hooks
            .projection_environment_after_path_data_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(crate) fn set_mutation_publication_test_hook(&self, hook: MutationPublicationTestHook) {
        *self
            .test_hooks
            .mutation_publication_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn set_history_after_routing_test_hook(&self, hook: DurableHistoryTestHook) {
        *self
            .test_hooks
            .history_after_routing_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn set_history_after_preparation_test_hook(&self, hook: DurableHistoryTestHook) {
        *self
            .test_hooks
            .history_after_preparation_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn set_history_after_disk_commit_test_hook(&self, hook: DurableHistoryTestHook) {
        *self
            .test_hooks
            .history_after_disk_commit_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn set_catalog_mutation_before_publication_test_hook(
        &self,
        hook: MutationPublicationTestHook,
    ) {
        *self
            .test_hooks
            .catalog_mutation_before_publication_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn set_compile_after_source_capture_test_hook(
        &self,
        hook: CompilePublicationTestHook,
    ) {
        *self
            .test_hooks
            .compile_after_source_capture_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn run_compile_after_source_capture_test_hook(&self) {
        if let Some(hook) = self
            .test_hooks
            .compile_after_source_capture_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    pub(super) fn run_compile_after_source_capture_test_hook(&self) {}

    #[cfg(test)]
    pub(super) fn set_compile_before_authority_gate_test_hook(
        &self,
        hook: CompilePublicationTestHook,
    ) {
        *self
            .test_hooks
            .compile_before_authority_gate_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn run_compile_capture_after_environment_test_hook(&self) {
        if let Some(hook) = self
            .test_hooks
            .compile_capture_after_environment_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    pub(super) fn run_compile_capture_after_environment_test_hook(&self) {}

    #[cfg(test)]
    pub(super) fn run_compile_before_authority_gate_test_hook(&self) {
        if let Some(hook) = self
            .test_hooks
            .compile_before_authority_gate_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    pub(super) fn run_compile_before_authority_gate_test_hook(&self) {}

    #[cfg(test)]
    pub(super) fn set_compile_coalesced_before_wait_test_hook(
        &self,
        hook: CompilePublicationTestHook,
    ) {
        *self
            .test_hooks
            .compile_coalesced_before_wait_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(super) fn run_compile_coalesced_before_wait_test_hook(&self) {
        if let Some(hook) = self
            .test_hooks
            .compile_coalesced_before_wait_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    pub(super) fn run_compile_coalesced_before_wait_test_hook(&self) {}

    #[cfg(test)]
    pub(super) fn set_execution_before_final_gate_test_hook(&self, hook: ExecutionTestHook) {
        *self
            .test_hooks
            .execution_before_final_gate_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    fn run_execution_before_final_gate_test_hook(&self) {
        if let Some(hook) = self
            .test_hooks
            .execution_before_final_gate_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    fn run_execution_before_final_gate_test_hook(&self) {}

    #[cfg(test)]
    pub(super) fn set_execution_before_run_test_hook(&self, hook: ExecutionTestHook) {
        *self
            .test_hooks
            .execution_before_run_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    fn run_execution_before_run_test_hook(&self) {
        if let Some(hook) = self
            .test_hooks
            .execution_before_run_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    fn run_execution_before_run_test_hook(&self) {}

    #[cfg(test)]
    pub(super) fn set_execution_before_commit_gate_test_hook(&self, hook: ExecutionTestHook) {
        *self
            .test_hooks
            .execution_before_commit_gate_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    fn run_execution_before_commit_gate_test_hook(&self) {
        if let Some(hook) = self
            .test_hooks
            .execution_before_commit_gate_test_hook
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    fn run_execution_before_commit_gate_test_hook(&self) {}

    #[cfg(test)]
    pub(crate) fn append_history_head_for_test(&self) {
        let path =
            crate::graph_document::GraphResourcePath::new("events/ConcurrentHistory.yssbi-event")
                .unwrap();
        self.history.write().unwrap().record_committed_transaction(
            ProjectHistoryTransaction::graph_move(
                crate::project::OperationId::new(),
                path.clone(),
                path,
                serde_json::Value::Null,
            ),
        );
    }

    #[cfg(test)]
    pub(crate) fn set_graph_move_history_io_checkpoint(&self, hook: Arc<dyn Fn() + Send + Sync>) {
        *self
            .test_hooks
            .graph_move_history_io_checkpoint
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    fn run_graph_move_history_io_checkpoint(&self) {
        if let Some(hook) = self
            .test_hooks
            .graph_move_history_io_checkpoint
            .read()
            .unwrap()
            .clone()
        {
            hook();
        }
    }

    #[cfg(not(test))]
    fn run_graph_move_history_io_checkpoint(&self) {}

    #[cfg(test)]
    pub(crate) fn set_variable_staging_test_hook(&self, hook: VariableStagingTestHook) {
        *self.test_hooks.variable_staging_test_hook.write().unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(crate) fn set_variable_authority_assignment_panic_for_test(
        &self,
        hook: VariableAuthorityAssignmentPanicTestHook,
    ) {
        *self
            .test_hooks
            .variable_authority_assignment_panic_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(crate) fn set_project_activation_test_hook(&self, hook: ProjectActivationTestHook) {
        *self
            .test_hooks
            .project_activation_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(crate) fn set_activation_store_replaced_test_hook(
        &self,
        hook: ActivationPublicationTestHook,
    ) {
        *self
            .test_hooks
            .activation_store_replaced_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(crate) fn set_activation_publication_panic_test_hook(
        &self,
        hook: ActivationPublicationTestHook,
    ) {
        *self
            .test_hooks
            .activation_publication_panic_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(crate) fn set_activation_preparation_after_read_test_hook(
        &self,
        hook: ActivationPublicationTestHook,
    ) {
        *self
            .test_hooks
            .activation_preparation_after_read_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(crate) fn set_computation_settings_publication_test_hook(
        &self,
        hook: ComputationSettingsPublicationTestHook,
    ) {
        *self
            .test_hooks
            .computation_settings_publication_test_hook
            .write()
            .unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(crate) fn poison_project_path_for_test(&self) {
        let _guard = self.project_path.write().unwrap();
        panic!("injected project path poison");
    }

    #[cfg(test)]
    pub(crate) fn publication_state_for_test(&self) -> (String, u64, u64) {
        let publication = self.mutation_publication.lock().unwrap();
        (
            publication.project_instance_id.clone(),
            publication.resource_revision,
            publication.authority_generation(),
        )
    }

    #[cfg(test)]
    pub(crate) fn authority_generation_for_test(&self) -> u64 {
        self.mutation_publication
            .lock()
            .unwrap()
            .authority_generation
    }

    pub(crate) fn activation_revision(&self) -> u64 {
        self.activation_generation
            .load(std::sync::atomic::Ordering::Acquire)
            / 2
    }

    #[cfg(test)]
    pub(crate) fn activation_generation_for_test(&self) -> u64 {
        self.activation_generation
            .load(std::sync::atomic::Ordering::Acquire)
    }

    #[cfg(test)]
    pub(crate) fn revision_state_for_test(
        &self,
    ) -> (
        std::collections::HashMap<GraphResourcePath, crate::graph_document::GraphRevision>,
        std::collections::HashMap<crate::variable::VariableId, crate::project::ResourceRevision>,
        std::collections::HashMap<WorksheetResourcePath, crate::project::ResourceRevision>,
    ) {
        (
            self.graph_revisions.read().unwrap().clone(),
            self.variable_revisions
                .read()
                .unwrap()
                .iter()
                .map(|(id, entry)| (*id, entry.revision))
                .collect(),
            self.worksheet_revisions.read().unwrap().clone(),
        )
    }

    #[cfg(test)]
    pub(crate) fn variable_revision_entry_for_test(
        &self,
        id: &crate::variable::VariableId,
    ) -> Option<VariableRevisionEntry> {
        self.variable_revisions.read().unwrap().get(id).copied()
    }

    #[cfg(test)]
    pub(crate) fn runtime_identity_sessions_for_test(
        &self,
    ) -> (
        crate::project::ProjectSessionId,
        crate::project::ProjectSessionId,
    ) {
        let _publication = self.mutation_publication.lock().unwrap();
        let runtime = self
            .project_store
            .read()
            .unwrap()
            .project_session_id
            .clone();
        let identity = self
            .activation_identity
            .read()
            .unwrap()
            .project_session_id
            .clone();
        (runtime, identity)
    }

    #[cfg(test)]
    pub(crate) fn try_current_pre_run_admission_for_test(&self) -> Option<bool> {
        let store = self.project_store.try_read().ok()?;
        let result = store.runs.track_pre_run(
            store.project_session_id.clone(),
            crate::node_system::runtime::CancellationToken::new(),
        );
        let accepted = result.is_ok();
        drop(result);
        Some(accepted)
    }

    #[cfg(test)]
    pub(crate) fn try_current_run_admission_for_test(&self) -> Option<bool> {
        let store = self.project_store.try_read().ok()?;
        let result = store.runs.track(
            store.project_session_id.clone(),
            crate::node_system::runtime::RunId::new(9_004),
            crate::node_system::runtime::CancellationToken::new(),
        );
        let accepted = result.is_ok();
        drop(result);
        Some(accepted)
    }

    #[cfg(test)]
    pub(crate) fn resource_lifecycle_entry_count(&self) -> usize {
        self.resource_lifecycle.entry_count()
    }

    #[cfg(test)]
    pub(crate) fn activation_publication_guards_are_available_for_test(&self) -> bool {
        self.mutation_publication.try_lock().is_ok()
            && self.project_path.try_write().is_ok()
            && self.resource_lifecycle.boundary_is_available()
            && self.project_data.try_write().is_ok()
            && self.project_store.try_write().is_ok()
            && self.graph_revisions.try_write().is_ok()
            && self.variable_revisions.try_write().is_ok()
            && self.worksheet_revisions.try_write().is_ok()
            && self.activation_identity.try_write().is_ok()
            && self.recovery_marker.boundary_is_available()
            && self.history.try_write().is_ok()
    }

    #[cfg(test)]
    pub(crate) fn set_graph_rename_io_checkpoint(&self, hook: LifecycleLockTestHook) {
        *self.test_hooks.graph_rename_io_checkpoint.write().unwrap() = Some(hook);
    }

    #[cfg(test)]
    pub(crate) fn initialize_worksheet_revision_for_test(
        &self,
        worksheet_path: &WorksheetResourcePath,
    ) {
        self.worksheet_revisions.write().unwrap().insert(
            worksheet_path.clone(),
            crate::project::ResourceRevision::INITIAL,
        );
    }

    #[cfg(test)]
    pub(crate) fn validate_catalog_snapshot_current(
        &self,
        snapshot: &crate::project::CatalogProjectSnapshot,
    ) -> Result<(), ProjectFilesystemError> {
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != snapshot.project_instance_id.as_str()
            || publication.authority_generation() != snapshot.authority_generation
        {
            return Err(ProjectFilesystemError::CatalogResourceStale {
                message: "catalog or graph authority changed during compatibility resolution"
                    .into(),
            });
        }
        Ok(())
    }

    #[cfg(test)]
    pub fn localized_catalog_snapshot(
        &self,
        project_instance_id: &ProjectInstanceId,
        locale: &str,
    ) -> Result<crate::schema::catalog::LocalizedCatalogDto, ProjectFilesystemError> {
        let snapshot = self.catalog_snapshot(project_instance_id)?;
        let registry_fingerprint = snapshot.registry.fingerprint().to_string();
        let localized = snapshot.catalog.localize_with_resources(
            snapshot.registry.as_ref(),
            locale,
            &snapshot.resources,
        );

        let mut result = crate::schema::catalog::LocalizedCatalogDto::from(localized);
        result.project_instance_id = snapshot.project_instance_id.as_str().into();
        result.registry_fingerprint = registry_fingerprint.into();
        result.resource_publication_revision = snapshot.resource_publication_revision;
        Ok(result)
    }

    pub fn worksheet_creation_snapshot(
        &self,
    ) -> Result<(Vec<String>, Option<String>), ProjectFilesystemError> {
        self.ensure_project_operational()?;
        let data = self.project_data.read().unwrap();
        Ok((
            data.worksheets
                .keys()
                .map(|path| path.display_name().as_str().to_string())
                .collect(),
            data.databases.keys().next().cloned(),
        ))
    }

    #[cfg(test)]
    pub(super) fn set_function_load_checkpoint(
        &self,
        checkpoint: Arc<dyn Fn(&crate::node_system::runtime::CancellationToken) + Send + Sync>,
    ) {
        *self.test_hooks.function_load_checkpoint.write().unwrap() = Some(checkpoint);
    }

    #[cfg(test)]
    pub fn result(
        &self,
        result_id: crate::node_system::runtime::ResultId,
    ) -> Result<Option<Arc<crate::node_system::runtime::StoredResult>>, ProjectFilesystemError>
    {
        self.ensure_project_operational()?;
        let results = self.project_store.read().unwrap().results.clone();
        Ok(results.result(result_id))
    }

    #[cfg(test)]
    pub fn pin_result_history(
        &self,
        output: &crate::execution::plan::legacy::GraphOutputRef,
    ) -> Result<
        Box<
            [(
                crate::node_system::runtime::PinResultEntry,
                Arc<crate::node_system::runtime::StoredResult>,
            )],
        >,
        ProjectFilesystemError,
    > {
        self.ensure_project_operational()?;
        let results = self.project_store.read().unwrap().results.clone();
        results
            .pin_history(output)
            .into_vec()
            .into_iter()
            .map(|entry| {
                let result = results.result(entry.result_id).ok_or_else(|| {
                    ProjectFilesystemError::ResultSourceReadFailed {
                        message: format!(
                            "pin history references missing result {}",
                            entry.result_id.get()
                        ),
                    }
                })?;
                Ok((entry, result))
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Vec::into_boxed_slice)
    }
}
