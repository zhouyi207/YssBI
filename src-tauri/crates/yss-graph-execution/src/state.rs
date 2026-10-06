//! Session-scoped ownership of plans, runs, results and work admission.
mod active_run;
mod admission;
mod control;
mod dispatch;
mod run_lifecycle;
mod scheduler;
#[cfg(feature = "test-support")]
mod test_support;

pub use crate::error::{ExecutePreparedError, ExecutionAdmissionError, OperationExecutionError};
pub use crate::run_registry::ExecutionCancelOutcome;
pub use active_run::ActiveExecutionRun;
pub use admission::{
    ExecutionDrainControl, ExecutionDrainOutcome, ExecutionOutstandingWork, ExecutionWorkLease,
};
pub use control::RunExecutionControl;
pub use dispatch::{ExecutedPreparedRun, ExecutionResultRequest, PreparedExecutionEvent};

use crate::finalization::ExecutionFinalizationHandoff;
use crate::identity::{ExecutionSessionId, RuntimeGeneration};
use crate::result::{
    GraphResultCacheState, GraphResultInputs, ResultId, ResultRunBasis, StoredResultSnapshot,
};
use crate::result_store::ResultStore;
use crate::run_registry::{RunRegistry, RunRegistryError, RunState};
use admission::SessionAdmission;
use scheduler::NeutralPlanExecutor;
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use yss_node_kernel::KernelRegistry;

/// Session-local execution state. Composition installs one instance per
/// Application session and replaces it atomically with that session.
pub struct ExecutionRuntimeState {
    pub(crate) graph_plans: crate::graph_preparation::GraphPlanCache,
    session_id: ExecutionSessionId,
    generation: RuntimeGeneration,
    admission: Arc<SessionAdmission>,
    results: ResultStore,
    runs: RunRegistry,
    executor: NeutralPlanExecutor,
    next_result_id: AtomicU64,
}

impl ExecutionRuntimeState {
    pub fn new(
        session_id: ExecutionSessionId,
        generation: RuntimeGeneration,
        kernels: Arc<KernelRegistry>,
        relations: Arc<dyn yss_relational_contract::RelationFactory>,
    ) -> Self {
        Self {
            graph_plans: crate::graph_preparation::GraphPlanCache::default(),
            session_id,
            generation,
            admission: Arc::new(SessionAdmission::default()),
            results: ResultStore::new(),
            runs: RunRegistry::new(),
            executor: NeutralPlanExecutor { kernels, relations },
            next_result_id: AtomicU64::new(1),
        }
    }

    pub fn kernels(&self) -> &KernelRegistry {
        &self.executor.kernels
    }

    pub fn session_id(&self) -> ExecutionSessionId {
        self.session_id
    }

    pub fn generation(&self) -> RuntimeGeneration {
        self.generation
    }

    pub fn close_admission(&self) {
        self.admission.close();
    }

    pub fn query_result(&self, result_id: ResultId) -> Option<StoredResultSnapshot> {
        self.results.get(result_id)
    }

    pub fn query_graph_results(&self, graph: &str, limit: usize) -> Vec<StoredResultSnapshot> {
        self.results.query_graph_results(graph, limit)
    }

    pub fn query_graph_result_entries(
        &self,
        graph: &str,
        run: Option<crate::run_registry::RunId>,
    ) -> Vec<crate::result::ResultReadSnapshot> {
        self.results.query_graph_result_entries(graph, run)
    }

    pub fn query_result_with_validity(
        &self,
        id: ResultId,
    ) -> Option<crate::result::ResultReadSnapshot> {
        self.results.query_result_with_validity(id)
    }

    pub fn query_pin_result(
        &self,
        output: &crate::plan::PlanOutputRef,
    ) -> Option<StoredResultSnapshot> {
        self.results.query_pin_result(output)
    }

    pub fn result_schema_candidates(&self, graph: &str) -> Vec<StoredResultSnapshot> {
        self.results.schema_candidates(graph)
    }

    pub fn matching_schema_results(
        &self,
        graph: &str,
        inputs: &GraphResultInputs,
    ) -> BTreeMap<crate::plan::PlanOutputRef, ResultId> {
        self.results.matching_schema_results(graph, inputs)
    }

    pub fn runs(&self) -> &RunRegistry {
        &self.runs
    }

    fn allocate_result_id(&self) -> Result<ResultId, ExecutePreparedError> {
        self.next_result_id
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |current| {
                current.checked_add(1)
            })
            .map(ResultId::from_existing)
            .map_err(|_| ExecutePreparedError::ResultIdentityExhausted)
    }

    pub fn cancel_run(&self, run_id: crate::run_registry::RunId) -> ExecutionCancelOutcome {
        self.runs.cancel(run_id)
    }

    pub fn observe_graph_result_inputs(
        &self,
        graph: &str,
        inputs: GraphResultInputs,
    ) -> GraphResultCacheState {
        self.results.observe_graph_inputs(graph, inputs)
    }

    pub fn capture_result_run_basis(
        &self,
        graph: &str,
        inputs: GraphResultInputs,
    ) -> Option<ResultRunBasis> {
        self.results.capture_run_basis(graph, inputs)
    }

    pub fn query_result_cache_states(
        &self,
        graph: &str,
        semantic_input_hash: &[u8; 32],
    ) -> Option<GraphResultCacheState> {
        self.results.query_cache_states(graph, semantic_input_hash)
    }

    /// A summary at this revision includes all result changes preceding this read.
    pub fn result_revision(&self) -> u64 {
        self.results.revision()
    }

    pub fn result_resource_keys(&self, graph: &str) -> std::collections::BTreeSet<Box<str>> {
        self.results.resource_keys(graph)
    }

    pub fn observe_result_resource_versions(
        &self,
        versions: &BTreeMap<Box<str>, Option<[u8; 32]>>,
    ) {
        self.results.observe_resource_versions(versions);
    }

    pub fn invalidate_graph_results(&self, graph: &str) {
        self.graph_plans.remove(graph);
        self.results.invalidate_graph(graph);
    }

    pub fn retain_result(
        &self,
        result_id: ResultId,
        lease_id: uuid::Uuid,
        owner: &str,
        handoff: Option<&str>,
    ) -> Result<StoredResultSnapshot, crate::result::ResultRetentionError> {
        self.results.retain(result_id, lease_id, owner, handoff)
    }

    pub fn claim_result_lease(
        &self,
        lease_id: uuid::Uuid,
        owner: &str,
    ) -> Result<StoredResultSnapshot, crate::result::ResultRetentionError> {
        self.results.claim(lease_id, owner)
    }

    pub fn release_result_lease(
        &self,
        lease_id: uuid::Uuid,
        owner: &str,
    ) -> Result<(), crate::result::ResultRetentionError> {
        self.results.release(lease_id, owner)
    }

    pub fn reconcile_result_leases(
        &self,
        owner: &str,
        active: &std::collections::BTreeSet<uuid::Uuid>,
    ) {
        self.results.reconcile(owner, active);
    }

    pub fn close_result_owner(&self, owner: &str) {
        self.results.close_owner(owner);
    }

    pub fn publish_committed_results(&self, handoff: &ExecutionFinalizationHandoff) -> bool {
        self.results
            .publish(handoff.results(), handoff.observation_intents())
    }

    pub fn finalize_run_success(
        &self,
        run_id: crate::run_registry::RunId,
    ) -> Result<(), RunRegistryError> {
        self.runs.transition(run_id, RunState::Succeeded)
    }

    pub fn finalize_run_failure(
        &self,
        run_id: crate::run_registry::RunId,
    ) -> Result<(), RunRegistryError> {
        self.runs.transition(run_id, RunState::Failed)
    }

    pub fn finalize_run_cancelled(
        &self,
        run_id: crate::run_registry::RunId,
    ) -> Result<(), RunRegistryError> {
        self.runs.transition(run_id, RunState::Cancelled)
    }

    pub fn admit(&self) -> Result<ExecutionWorkLease, ExecutionAdmissionError> {
        self.admission.admit()
    }

    pub fn drain(&self, control: &ExecutionDrainControl) -> ExecutionDrainOutcome {
        self.admission.drain(control)
    }

    pub fn cancel_and_drain(&self, control: &ExecutionDrainControl) -> ExecutionDrainOutcome {
        self.admission.cancel_and_close(&self.runs);
        self.drain(control)
    }
}

#[cfg(test)]
mod tests;
