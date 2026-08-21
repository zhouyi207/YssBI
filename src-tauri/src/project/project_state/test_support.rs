use super::*;

pub(super) type ProjectionTestHook = Arc<dyn Fn() -> Result<(), String> + Send + Sync>;
pub(super) type CommittedResourceCompletionTestHook = Arc<dyn Fn() + Send + Sync>;
pub(super) type ProjectionEnvironmentCaptureTestHook = Arc<dyn Fn() + Send + Sync>;
pub(super) type MutationPublicationTestHook = Arc<dyn Fn() + Send + Sync>;
pub(super) type DurableHistoryTestHook = Arc<dyn Fn() + Send + Sync>;
pub(super) type CompilePublicationTestHook = Arc<dyn Fn() + Send + Sync>;
pub(super) type ExecutionTestHook = Arc<dyn Fn() + Send + Sync>;
pub(super) type ProductionRelationalBackendFactory =
    Arc<dyn Fn() -> Arc<dyn crate::node_system::runtime::RelationalBackend> + Send + Sync>;
pub(super) type TraceQueryTestHook = Arc<dyn Fn() + Send + Sync>;
pub(super) type VariableStagingTestHook = Arc<dyn Fn() + Send + Sync>;
pub(super) type VariableAuthorityAssignmentPanicTestHook = Arc<dyn Fn() + Send + Sync>;
pub(crate) type ProjectActivationTestHook = Arc<dyn Fn() + Send + Sync>;
pub(super) type ActivationPublicationTestHook = Arc<dyn Fn() + Send + Sync>;
pub(super) type LifecycleLockTestHook = Arc<dyn Fn() + Send + Sync>;
pub(super) type ComputationSettingsPublicationTestHook = Arc<dyn Fn() + Send + Sync>;

#[derive(Default)]
pub(in crate::project) struct ProjectStateTestHooks {
    pub(in crate::project) graph_rename_io_checkpoint:
        Arc<RwLock<Option<Arc<dyn Fn() + Send + Sync>>>>,
    pub(in crate::project) graph_move_history_io_checkpoint:
        Arc<RwLock<Option<Arc<dyn Fn() + Send + Sync>>>>,
    pub(in crate::project) function_load_checkpoint: Arc<
        RwLock<Option<Arc<dyn Fn(&crate::node_system::runtime::CancellationToken) + Send + Sync>>>,
    >,
    pub(in crate::project) production_relational_observer:
        Arc<RwLock<Option<Arc<crate::node_system::runtime::ProductionRelationalObserver>>>>,
    pub(in crate::project) production_relational_backend_factory:
        Arc<RwLock<Option<ProductionRelationalBackendFactory>>>,
    pub(in crate::project) project_resource_lease_observer:
        Arc<RwLock<Option<crate::node_system::runtime::ProjectResourceLeaseObserver>>>,
    pub(in crate::project) projection_test_hook: Arc<RwLock<Option<ProjectionTestHook>>>,
    pub(in crate::project) committed_resource_completion_test_hook:
        Arc<RwLock<Option<CommittedResourceCompletionTestHook>>>,
    pub(in crate::project) projection_environment_capture_test_hook:
        Arc<RwLock<Option<ProjectionEnvironmentCaptureTestHook>>>,
    pub(in crate::project) projection_environment_after_path_data_test_hook:
        Arc<RwLock<Option<ProjectionEnvironmentCaptureTestHook>>>,
    pub(in crate::project) resource_mutation_test_hook:
        Arc<RwLock<Option<crate::project::resource_mutations::ResourceMutationTestHook>>>,
    pub(in crate::project) mutation_publication_test_hook:
        Arc<RwLock<Option<MutationPublicationTestHook>>>,
    pub(in crate::project) history_after_routing_test_hook:
        Arc<RwLock<Option<DurableHistoryTestHook>>>,
    pub(in crate::project) history_after_preparation_test_hook:
        Arc<RwLock<Option<DurableHistoryTestHook>>>,
    pub(in crate::project) history_after_disk_commit_test_hook:
        Arc<RwLock<Option<DurableHistoryTestHook>>>,
    pub(in crate::project) catalog_mutation_before_publication_test_hook:
        Arc<RwLock<Option<MutationPublicationTestHook>>>,
    pub(in crate::project) compile_capture_after_environment_test_hook:
        Arc<RwLock<Option<CompilePublicationTestHook>>>,
    pub(in crate::project) compile_after_source_capture_test_hook:
        Arc<RwLock<Option<CompilePublicationTestHook>>>,
    pub(in crate::project) compile_before_authority_gate_test_hook:
        Arc<RwLock<Option<CompilePublicationTestHook>>>,
    pub(in crate::project) compile_coalesced_before_wait_test_hook:
        Arc<RwLock<Option<CompilePublicationTestHook>>>,
    pub(in crate::project) execution_before_final_gate_test_hook:
        Arc<RwLock<Option<ExecutionTestHook>>>,
    pub(in crate::project) execution_before_run_test_hook: Arc<RwLock<Option<ExecutionTestHook>>>,
    pub(in crate::project) execution_before_commit_gate_test_hook:
        Arc<RwLock<Option<ExecutionTestHook>>>,
    pub(in crate::project) trace_query_after_snapshot_test_hook:
        Arc<RwLock<Option<TraceQueryTestHook>>>,
    pub(in crate::project) variable_staging_test_hook: Arc<RwLock<Option<VariableStagingTestHook>>>,
    pub(in crate::project) variable_authority_assignment_panic_test_hook:
        Arc<RwLock<Option<VariableAuthorityAssignmentPanicTestHook>>>,
    pub(in crate::project) project_activation_test_hook:
        Arc<RwLock<Option<ProjectActivationTestHook>>>,
    pub(in crate::project) activation_store_replaced_test_hook:
        Arc<RwLock<Option<ActivationPublicationTestHook>>>,
    pub(in crate::project) activation_publication_panic_test_hook:
        Arc<RwLock<Option<ActivationPublicationTestHook>>>,
    pub(in crate::project) activation_preparation_after_read_test_hook:
        Arc<RwLock<Option<ActivationPublicationTestHook>>>,
    pub(in crate::project) computation_settings_publication_test_hook:
        Arc<RwLock<Option<ComputationSettingsPublicationTestHook>>>,
}
