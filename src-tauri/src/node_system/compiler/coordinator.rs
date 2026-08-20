use super::{DemandPlanError, ExecutionPlanBasis, NormalizedExecutionDemand};
use crate::node_system::analysis::{
    CompilationBasis, CompileId, CompileProjection, ResourceVersionSet,
};
use crate::node_system::document::{GraphResourcePath, GraphRevision};
use crate::node_system::plan::{ExecutionDemand, ExecutionPlan};
use crate::node_system::registry::RegistryFingerprint;
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileCancelled;

impl std::fmt::Display for CompileCancelled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("compilation was cancelled")
    }
}

impl std::error::Error for CompileCancelled {}

#[derive(Debug, Clone, Default)]
pub struct CompileCancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CompileCancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_shared(cancelled: Arc<AtomicBool>) -> Self {
        Self { cancelled }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    pub fn checkpoint(&self) -> Result<(), CompileCancelled> {
        if self.is_cancelled() {
            Err(CompileCancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompilationTask {
    pub graph_path: GraphResourcePath,
    pub basis: CompilationBasis<GraphRevision>,
    pub compile_id: CompileId,
    pub cancellation: CompileCancellationToken,
}

#[derive(Debug, Clone)]
pub enum ScheduleOutcome {
    Start(CompilationTask),
    Coalesced { compile_id: CompileId },
    Exhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PublishOutcome {
    Current,
    Stale,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishReport {
    pub analysis: PublishOutcome,
    pub plan: Option<PublishOutcome>,
}

#[derive(Debug, Clone)]
pub struct CompileProducts<Analysis, Plan> {
    pub analysis: Analysis,
    pub has_blocking_diagnostics: bool,
    pub plan: Option<Plan>,
}

const DEMAND_VARIANT_CACHE_CAPACITY: usize = 16;

#[derive(Debug)]
struct BoundedVariantCache<K, V> {
    capacity: usize,
    insertion_order: VecDeque<K>,
    entries: BTreeMap<K, V>,
}

impl<K: Clone + Ord, V> BoundedVariantCache<K, V> {
    fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "variant cache capacity must be positive");
        Self {
            capacity,
            insertion_order: VecDeque::with_capacity(capacity),
            entries: BTreeMap::new(),
        }
    }

    fn get(&self, key: &K) -> Option<&V> {
        self.entries.get(key)
    }

    fn insert(&mut self, key: K, value: V) {
        if self.entries.contains_key(&key) {
            self.entries.insert(key, value);
            return;
        }
        if self.entries.len() == self.capacity
            && let Some(evicted) = self.insertion_order.pop_front()
        {
            self.entries.remove(&evicted);
        }
        self.insertion_order.push_back(key.clone());
        self.entries.insert(key, value);
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.len()
    }
}

#[derive(Debug, Clone)]
pub struct SelectedExecutionPlan {
    pub plan: Arc<ExecutionPlan>,
    pub normalized_demand: NormalizedExecutionDemand,
    pub selection_digest: [u8; 32],
}

#[derive(Debug)]
pub struct PublishedExecutionPlan {
    full_plan: Option<ExecutionPlan>,
    execution_basis: ExecutionPlanBasis,
    variants: Mutex<BoundedVariantCache<NormalizedExecutionDemand, Arc<ExecutionPlan>>>,
}

impl PublishedExecutionPlan {
    pub fn new(full_plan: Option<ExecutionPlan>, execution_basis: ExecutionPlanBasis) -> Self {
        Self {
            full_plan,
            execution_basis,
            variants: Mutex::new(BoundedVariantCache::new(DEMAND_VARIANT_CACHE_CAPACITY)),
        }
    }

    pub fn full_plan(&self) -> Option<&ExecutionPlan> {
        self.full_plan.as_ref()
    }

    pub fn select(
        &self,
        demand: &ExecutionDemand,
    ) -> Result<SelectedExecutionPlan, DemandPlanError> {
        let normalized_demand = self.execution_basis.normalize_demand(demand)?;
        let selection_digest = normalized_demand.digest()?;
        if let Some(plan) = self
            .variants
            .lock()
            .expect("demand variant cache lock poisoned")
            .get(&normalized_demand)
            .cloned()
        {
            return Ok(SelectedExecutionPlan {
                plan,
                normalized_demand,
                selection_digest,
            });
        }

        let plan = Arc::new(self.execution_basis.derive_plan(demand)?);
        self.variants
            .lock()
            .expect("demand variant cache lock poisoned")
            .insert(normalized_demand.clone(), Arc::clone(&plan));
        Ok(SelectedExecutionPlan {
            plan,
            normalized_demand,
            selection_digest,
        })
    }

    #[cfg(test)]
    pub(crate) fn cached_variant_count(&self) -> usize {
        self.variants
            .lock()
            .expect("demand variant cache lock poisoned")
            .len()
    }
}

#[derive(Debug)]
pub struct CompilationSlot<Analysis, Plan> {
    graph_path: GraphResourcePath,
    active: Option<CompilationTask>,
    pending: Option<CompilationTask>,
    published_analysis: Option<CompileProjection<Analysis>>,
    published_plan: Option<CompileProjection<Plan>>,
}

impl<Analysis, Plan> CompilationSlot<Analysis, Plan> {
    pub fn new(graph_path: GraphResourcePath) -> Self {
        Self {
            graph_path,
            active: None,
            pending: None,
            published_analysis: None,
            published_plan: None,
        }
    }

    pub fn request(
        &mut self,
        compile_id: CompileId,
        basis: CompilationBasis<GraphRevision>,
    ) -> ScheduleOutcome {
        let task = CompilationTask {
            graph_path: self.graph_path.clone(),
            basis,
            compile_id,
            cancellation: CompileCancellationToken::new(),
        };
        if let Some(active) = &self.active {
            if active.basis == task.basis && !active.cancellation.is_cancelled() {
                return ScheduleOutcome::Coalesced {
                    compile_id: active.compile_id,
                };
            }
            active.cancellation.cancel();
            if let Some(pending) = &self.pending {
                if pending.basis == task.basis {
                    return ScheduleOutcome::Coalesced {
                        compile_id: pending.compile_id,
                    };
                }
            }
            if let Some(replaced) = self.pending.replace(task) {
                replaced.cancellation.cancel();
            }
            return ScheduleOutcome::Coalesced { compile_id };
        }
        self.active = Some(task.clone());
        ScheduleOutcome::Start(task)
    }

    pub fn finish(&mut self, compile_id: CompileId) -> Option<CompilationTask> {
        if self.active.as_ref().map(|task| task.compile_id) != Some(compile_id) {
            return None;
        }
        self.active = self.pending.take();
        self.active.clone()
    }

    pub fn publish(
        &mut self,
        task: &CompilationTask,
        current_basis: &CompilationBasis<GraphRevision>,
        products: CompileProducts<Analysis, Plan>,
    ) -> PublishReport {
        self.publish_tracked(
            task,
            current_basis,
            &current_basis.resource_versions,
            &task.basis,
            products,
        )
    }

    pub fn publish_tracked(
        &mut self,
        task: &CompilationTask,
        current_basis: &CompilationBasis<GraphRevision>,
        current_resource_versions: &ResourceVersionSet,
        final_basis: &CompilationBasis<GraphRevision>,
        products: CompileProducts<Analysis, Plan>,
    ) -> PublishReport {
        self.publish_with_observations(
            task,
            current_basis,
            &states_from_versions(final_basis, current_resource_versions),
            final_basis,
            products,
        )
    }

    pub fn publish_with_observations(
        &mut self,
        task: &CompilationTask,
        current_basis: &CompilationBasis<GraphRevision>,
        current_resource_states: &crate::node_system::analysis::ResourceObservationSet,
        final_basis: &CompilationBasis<GraphRevision>,
        products: CompileProducts<Analysis, Plan>,
    ) -> PublishReport {
        let outcome = self.classify(task, current_basis, current_resource_states, final_basis);
        let plan_outcome = products
            .plan
            .as_ref()
            .filter(|_| !products.has_blocking_diagnostics)
            .map(|_| outcome);
        if outcome == PublishOutcome::Current {
            self.published_analysis = Some(CompileProjection {
                graph_path: self.graph_path.clone(),
                basis: final_basis.clone(),
                compile_id: task.compile_id,
                payload: products.analysis,
            });
            self.published_plan = if products.has_blocking_diagnostics {
                None
            } else {
                products.plan.map(|payload| CompileProjection {
                    graph_path: self.graph_path.clone(),
                    basis: final_basis.clone(),
                    compile_id: task.compile_id,
                    payload,
                })
            };
        }
        PublishReport {
            analysis: outcome,
            plan: plan_outcome,
        }
    }

    pub fn published_analysis(&self) -> Option<&CompileProjection<Analysis>> {
        self.published_analysis.as_ref()
    }

    pub fn published_plan(&self) -> Option<&CompileProjection<Plan>> {
        self.published_plan.as_ref()
    }

    fn cancel_work(&mut self) {
        if let Some(active) = self.active.take() {
            active.cancellation.cancel();
        }
        if let Some(pending) = self.pending.take() {
            pending.cancellation.cancel();
        }
        self.published_analysis = None;
        self.published_plan = None;
    }

    fn has_task(&self, basis: &CompilationBasis<GraphRevision>, compile_id: CompileId) -> bool {
        self.active
            .iter()
            .chain(self.pending.iter())
            .any(|task| task.compile_id == compile_id && &task.basis == basis)
    }

    #[cfg(test)]
    fn current_products(
        &self,
        graph_path: &GraphResourcePath,
        basis: &CompilationBasis<GraphRevision>,
    ) -> Option<(CompileProjection<Analysis>, Option<CompileProjection<Plan>>)>
    where
        Analysis: Clone,
        Plan: Clone,
    {
        self.current_products_tracked(graph_path, basis, &basis.resource_versions)
    }

    fn candidate_products(
        &self,
        graph_path: &GraphResourcePath,
        basis: &CompilationBasis<GraphRevision>,
    ) -> Option<(CompileProjection<Analysis>, Option<CompileProjection<Plan>>)>
    where
        Analysis: Clone,
        Plan: Clone,
    {
        let analysis = self.published_analysis.as_ref().filter(|analysis| {
            &analysis.graph_path == graph_path
                && analysis.basis.graph_revision == basis.graph_revision
                && analysis.basis.registry_fingerprint == basis.registry_fingerprint
        })?;
        let plan = self
            .published_plan
            .as_ref()
            .filter(|plan| {
                plan.graph_path == analysis.graph_path
                    && plan.basis == analysis.basis
                    && plan.compile_id == analysis.compile_id
            })
            .cloned();
        Some((analysis.clone(), plan))
    }

    fn current_products_tracked(
        &self,
        graph_path: &GraphResourcePath,
        basis: &CompilationBasis<GraphRevision>,
        current_resource_versions: &ResourceVersionSet,
    ) -> Option<(CompileProjection<Analysis>, Option<CompileProjection<Plan>>)>
    where
        Analysis: Clone,
        Plan: Clone,
    {
        let published_basis = self
            .published_analysis
            .as_ref()
            .map(|analysis| &analysis.basis)?;
        let current_resource_states =
            states_from_versions(published_basis, current_resource_versions);
        self.current_products_with_observations(graph_path, basis, &current_resource_states)
    }

    fn current_products_with_observations(
        &self,
        graph_path: &GraphResourcePath,
        basis: &CompilationBasis<GraphRevision>,
        current_resource_states: &crate::node_system::analysis::ResourceObservationSet,
    ) -> Option<(CompileProjection<Analysis>, Option<CompileProjection<Plan>>)>
    where
        Analysis: Clone,
        Plan: Clone,
    {
        let analysis = self.published_analysis.as_ref().filter(|analysis| {
            &analysis.graph_path == graph_path
                && basis_is_current(&analysis.basis, basis, current_resource_states)
        })?;
        let plan = self
            .published_plan
            .as_ref()
            .filter(|plan| {
                plan.graph_path == analysis.graph_path
                    && plan.basis == analysis.basis
                    && plan.compile_id == analysis.compile_id
            })
            .cloned();
        Some((analysis.clone(), plan))
    }

    fn classify(
        &self,
        task: &CompilationTask,
        current_basis: &CompilationBasis<GraphRevision>,
        current_resource_states: &crate::node_system::analysis::ResourceObservationSet,
        final_basis: &CompilationBasis<GraphRevision>,
    ) -> PublishOutcome {
        if task.cancellation.is_cancelled()
            || self.active.as_ref().map(|active| active.compile_id) != Some(task.compile_id)
        {
            PublishOutcome::Cancelled
        } else if &task.basis != current_basis
            || !basis_is_current(final_basis, current_basis, current_resource_states)
        {
            PublishOutcome::Stale
        } else {
            PublishOutcome::Current
        }
    }
}

#[derive(Debug)]
pub struct CompileCoordinator<Analysis, Plan> {
    next_compile_id: AtomicU64,
    slots: Mutex<BTreeMap<GraphResourcePath, CompilationSlot<Analysis, Plan>>>,
    changed: Condvar,
}

impl<Analysis, Plan> Default for CompileCoordinator<Analysis, Plan> {
    fn default() -> Self {
        Self {
            next_compile_id: AtomicU64::new(1),
            slots: Mutex::new(BTreeMap::new()),
            changed: Condvar::new(),
        }
    }
}

impl<Analysis, Plan> CompileCoordinator<Analysis, Plan> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request(
        &self,
        graph_path: GraphResourcePath,
        basis: CompilationBasis<GraphRevision>,
    ) -> ScheduleOutcome {
        let Ok(compile_id) = crate::node_system::allocate_nonzero_id(&self.next_compile_id) else {
            return ScheduleOutcome::Exhausted;
        };
        let compile_id = CompileId::new(compile_id.get());
        let outcome = self
            .slots
            .lock()
            .expect("compile coordinator lock poisoned")
            .entry(graph_path.clone())
            .or_insert_with(|| CompilationSlot::new(graph_path))
            .request(compile_id, basis);
        self.changed.notify_all();
        outcome
    }

    pub fn finish(
        &self,
        graph_path: &GraphResourcePath,
        compile_id: CompileId,
    ) -> Option<CompilationTask> {
        let next = self
            .slots
            .lock()
            .expect("compile coordinator lock poisoned")
            .get_mut(graph_path)
            .and_then(|slot| slot.finish(compile_id));
        self.changed.notify_all();
        next
    }

    pub fn publish(
        &self,
        task: &CompilationTask,
        current_basis: &CompilationBasis<GraphRevision>,
        products: CompileProducts<Analysis, Plan>,
    ) -> PublishReport {
        self.publish_tracked(
            task,
            current_basis,
            &current_basis.resource_versions,
            &task.basis,
            products,
        )
    }

    pub fn publish_tracked(
        &self,
        task: &CompilationTask,
        current_basis: &CompilationBasis<GraphRevision>,
        current_resource_versions: &ResourceVersionSet,
        final_basis: &CompilationBasis<GraphRevision>,
        products: CompileProducts<Analysis, Plan>,
    ) -> PublishReport {
        self.publish_with_observations(
            task,
            current_basis,
            &states_from_versions(final_basis, current_resource_versions),
            final_basis,
            products,
        )
    }

    pub fn publish_with_observations(
        &self,
        task: &CompilationTask,
        current_basis: &CompilationBasis<GraphRevision>,
        current_resource_states: &crate::node_system::analysis::ResourceObservationSet,
        final_basis: &CompilationBasis<GraphRevision>,
        products: CompileProducts<Analysis, Plan>,
    ) -> PublishReport {
        let report = {
            let mut slots = self
                .slots
                .lock()
                .expect("compile coordinator lock poisoned");
            match slots.get_mut(&task.graph_path) {
                Some(slot) => slot.publish_with_observations(
                    task,
                    current_basis,
                    current_resource_states,
                    final_basis,
                    products,
                ),
                None => PublishReport {
                    analysis: PublishOutcome::Cancelled,
                    plan: products
                        .plan
                        .as_ref()
                        .filter(|_| !products.has_blocking_diagnostics)
                        .map(|_| PublishOutcome::Cancelled),
                },
            }
        };
        self.changed.notify_all();
        report
    }

    pub fn invalidate(&self, graph_path: &GraphResourcePath) {
        if let Some(mut slot) = self
            .slots
            .lock()
            .expect("compile coordinator lock poisoned")
            .remove(graph_path)
        {
            slot.cancel_work();
        }
        self.changed.notify_all();
    }

    pub fn invalidate_all(&self) {
        let mut slots = self
            .slots
            .lock()
            .expect("compile coordinator lock poisoned");
        for slot in slots.values_mut() {
            slot.cancel_work();
        }
        slots.clear();
        drop(slots);
        self.changed.notify_all();
    }

    #[cfg(test)]
    pub(crate) fn contains_slot_for_test(&self, graph_path: &GraphResourcePath) -> bool {
        self.slots
            .lock()
            .expect("compile coordinator lock poisoned")
            .contains_key(graph_path)
    }
}

impl<Analysis: Clone, Plan: Clone> CompileCoordinator<Analysis, Plan> {
    pub fn get_candidate(
        &self,
        graph_path: &GraphResourcePath,
        basis: &CompilationBasis<GraphRevision>,
    ) -> Option<(CompileProjection<Analysis>, Option<CompileProjection<Plan>>)> {
        self.slots
            .lock()
            .expect("compile coordinator lock poisoned")
            .get(graph_path)
            .and_then(|slot| slot.candidate_products(graph_path, basis))
    }

    pub fn get_current(
        &self,
        graph_path: &GraphResourcePath,
        basis: &CompilationBasis<GraphRevision>,
    ) -> Option<(CompileProjection<Analysis>, Option<CompileProjection<Plan>>)> {
        self.get_current_tracked(graph_path, basis, &basis.resource_versions)
    }

    pub fn get_current_tracked(
        &self,
        graph_path: &GraphResourcePath,
        basis: &CompilationBasis<GraphRevision>,
        current_resource_versions: &ResourceVersionSet,
    ) -> Option<(CompileProjection<Analysis>, Option<CompileProjection<Plan>>)> {
        self.slots
            .lock()
            .expect("compile coordinator lock poisoned")
            .get(graph_path)
            .and_then(|slot| {
                slot.current_products_tracked(graph_path, basis, current_resource_versions)
            })
    }

    pub fn get_current_with_observations(
        &self,
        graph_path: &GraphResourcePath,
        basis: &CompilationBasis<GraphRevision>,
        current_resource_states: &crate::node_system::analysis::ResourceObservationSet,
    ) -> Option<(CompileProjection<Analysis>, Option<CompileProjection<Plan>>)> {
        self.slots
            .lock()
            .expect("compile coordinator lock poisoned")
            .get(graph_path)
            .and_then(|slot| {
                slot.current_products_with_observations(graph_path, basis, current_resource_states)
            })
    }

    pub fn wait_for_candidate(
        &self,
        graph_path: &GraphResourcePath,
        basis: &CompilationBasis<GraphRevision>,
        compile_id: CompileId,
    ) -> Option<(CompileProjection<Analysis>, Option<CompileProjection<Plan>>)> {
        let mut slots = self
            .slots
            .lock()
            .expect("compile coordinator lock poisoned");
        loop {
            let slot = slots.get(graph_path)?;
            if let Some(products) = slot.candidate_products(graph_path, basis)
                && products.0.compile_id == compile_id
            {
                return Some(products);
            }
            if !slot.has_task(basis, compile_id) {
                return None;
            }
            slots = self
                .changed
                .wait(slots)
                .expect("compile coordinator lock poisoned while waiting");
        }
    }

    pub fn wait_for_current(
        &self,
        graph_path: &GraphResourcePath,
        basis: &CompilationBasis<GraphRevision>,
        compile_id: CompileId,
    ) -> Option<(CompileProjection<Analysis>, Option<CompileProjection<Plan>>)> {
        self.wait_for_current_tracked(graph_path, basis, &basis.resource_versions, compile_id)
    }

    pub fn wait_for_current_tracked(
        &self,
        graph_path: &GraphResourcePath,
        basis: &CompilationBasis<GraphRevision>,
        current_resource_versions: &ResourceVersionSet,
        compile_id: CompileId,
    ) -> Option<(CompileProjection<Analysis>, Option<CompileProjection<Plan>>)> {
        let mut slots = self
            .slots
            .lock()
            .expect("compile coordinator lock poisoned");
        loop {
            let slot = slots.get(graph_path)?;
            if let Some(products) =
                slot.current_products_tracked(graph_path, basis, current_resource_versions)
            {
                if products.0.compile_id == compile_id {
                    return Some(products);
                }
            }
            if !slot.has_task(basis, compile_id) {
                return None;
            }
            slots = self
                .changed
                .wait(slots)
                .expect("compile coordinator lock poisoned while waiting");
        }
    }
}

fn states_from_versions(
    published: &CompilationBasis<GraphRevision>,
    current_resource_versions: &ResourceVersionSet,
) -> crate::node_system::analysis::ResourceObservationSet {
    let mut states = current_resource_versions
        .iter()
        .map(|(key, version)| {
            (
                key.clone(),
                crate::node_system::analysis::ResourceObservedState::Present(version.clone()),
            )
        })
        .collect::<crate::node_system::analysis::ResourceObservationSet>();
    for (key, state) in &published.resource_observations {
        if matches!(
            state,
            crate::node_system::analysis::ResourceObservedState::Absent(_)
        ) {
            states.entry(key.clone()).or_insert_with(|| {
                crate::node_system::analysis::ResourceObservedState::Absent(None)
            });
        }
    }
    states
}

fn basis_is_current(
    published: &CompilationBasis<GraphRevision>,
    request: &CompilationBasis<GraphRevision>,
    current_resource_states: &crate::node_system::analysis::ResourceObservationSet,
) -> bool {
    published.graph_revision == request.graph_revision
        && published.registry_fingerprint == request.registry_fingerprint
        && published.resource_versions.iter().all(|(key, version)| {
            current_resource_states.get(key)
                == Some(
                    &crate::node_system::analysis::ResourceObservedState::Present(version.clone()),
                )
        })
        && published
            .resource_observations
            .iter()
            .all(|(key, state)| current_resource_states.get(key) == Some(state))
}

pub fn compilation_basis(
    graph_revision: GraphRevision,
    registry_fingerprint: RegistryFingerprint,
    resource_versions: ResourceVersionSet,
) -> CompilationBasis<GraphRevision> {
    CompilationBasis {
        graph_revision,
        registry_fingerprint,
        resource_versions,
        resource_observations: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::analysis::{ResourceKey, ResourceVersion};
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    fn path() -> GraphResourcePath {
        GraphResourcePath("events/main".into())
    }

    fn basis(revision: u64, registry: u8, resource: &str) -> CompilationBasis<GraphRevision> {
        compilation_basis(
            GraphRevision::new(revision),
            RegistryFingerprint::from_bytes([registry; 32]),
            BTreeMap::from([(
                ResourceKey::new("functions/shared"),
                ResourceVersion::new(resource),
            )]),
        )
    }

    fn started(outcome: ScheduleOutcome) -> CompilationTask {
        match outcome {
            ScheduleOutcome::Start(task) => task,
            ScheduleOutcome::Coalesced { .. } => panic!("expected task to start"),
            ScheduleOutcome::Exhausted => panic!("compile ID unexpectedly exhausted"),
        }
    }

    #[test]
    fn compile_id_exhaustion_rejects_scheduling_without_creating_a_slot() {
        let coordinator = CompileCoordinator::<(), ()>::new();
        coordinator
            .next_compile_id
            .store(u64::MAX, Ordering::Relaxed);

        for _ in 0..2 {
            assert!(matches!(
                coordinator.request(path(), basis(1, 1, "1")),
                ScheduleOutcome::Exhausted
            ));
        }
        assert!(!coordinator.contains_slot_for_test(&path()));
    }

    #[test]
    fn compile_cancellation_can_share_the_registered_run_flag() {
        let flag = Arc::new(AtomicBool::new(false));
        let cancellation = CompileCancellationToken::from_shared(Arc::clone(&flag));
        assert!(cancellation.checkpoint().is_ok());
        flag.store(true, Ordering::Release);
        assert_eq!(cancellation.checkpoint(), Err(CompileCancelled));
    }

    #[test]
    fn edit_during_compile_cancels_old_request_and_coalesces_latest_pending() {
        let coordinator = CompileCoordinator::<&str, &str>::new();
        let first = started(coordinator.request(path(), basis(1, 1, "1")));
        let second_id = match coordinator.request(path(), basis(2, 1, "1")) {
            ScheduleOutcome::Coalesced { compile_id } => compile_id,
            _ => panic!("second task must wait for cancelled active task"),
        };
        let third_basis = basis(3, 1, "1");
        let third_id = match coordinator.request(path(), third_basis.clone()) {
            ScheduleOutcome::Coalesced { compile_id } => compile_id,
            _ => panic!("third task must replace pending task"),
        };

        let report = coordinator.publish(
            &first,
            &third_basis,
            CompileProducts {
                analysis: "old analysis",
                has_blocking_diagnostics: false,
                plan: Some("old plan"),
            },
        );
        assert_eq!(report.analysis, PublishOutcome::Cancelled);
        assert_ne!(second_id, third_id);
        let promoted = coordinator.finish(&path(), first.compile_id).unwrap();
        assert_eq!(promoted.compile_id, third_id);
        assert_eq!(promoted.basis, third_basis);
    }

    #[test]
    fn observed_missing_resource_appearance_makes_publication_stale() {
        let coordinator = CompileCoordinator::<&str, &str>::new();
        let request = compilation_basis(
            GraphRevision::INITIAL,
            RegistryFingerprint::from_bytes([3; 32]),
            BTreeMap::new(),
        );
        let task = started(coordinator.request(path(), request.clone()));
        let key = ResourceKey::new("functions/missing");
        let mut final_basis = request.clone();
        final_basis.resource_observations.insert(
            key.clone(),
            crate::node_system::analysis::ResourceObservedState::Absent(None),
        );
        coordinator.publish_tracked(
            &task,
            &request,
            &BTreeMap::new(),
            &final_basis,
            CompileProducts {
                analysis: "missing analysis",
                has_blocking_diagnostics: true,
                plan: None,
            },
        );
        coordinator.finish(&path(), task.compile_id);

        assert!(
            coordinator
                .get_current_tracked(&path(), &request, &BTreeMap::new())
                .is_some()
        );
        assert!(
            coordinator
                .get_current_tracked(
                    &path(),
                    &request,
                    &BTreeMap::from([(key, ResourceVersion::new("appeared"))]),
                )
                .is_none()
        );
    }

    #[test]
    fn observed_tombstone_generation_must_match_exactly() {
        let coordinator = CompileCoordinator::<&str, &str>::new();
        let request = compilation_basis(
            GraphRevision::INITIAL,
            RegistryFingerprint::from_bytes([4; 32]),
            BTreeMap::new(),
        );
        let task = started(coordinator.request(path(), request.clone()));
        let key = ResourceKey::new("variables/missing");
        let tombstone = ResourceVersion::new("revision:7");
        let mut final_basis = request.clone();
        final_basis.resource_observations.insert(
            key.clone(),
            crate::node_system::analysis::ResourceObservedState::Absent(Some(tombstone.clone())),
        );
        coordinator.publish_with_observations(
            &task,
            &request,
            &BTreeMap::from([(
                key.clone(),
                crate::node_system::analysis::ResourceObservedState::Absent(Some(
                    tombstone.clone(),
                )),
            )]),
            &final_basis,
            CompileProducts {
                analysis: "missing analysis",
                has_blocking_diagnostics: true,
                plan: None,
            },
        );
        coordinator.finish(&path(), task.compile_id);

        assert!(
            coordinator
                .get_current_tracked(&path(), &request, &BTreeMap::new())
                .is_none()
        );
        assert!(
            coordinator
                .get_current_with_observations(
                    &path(),
                    &request,
                    &BTreeMap::from([(
                        key.clone(),
                        crate::node_system::analysis::ResourceObservedState::Absent(Some(
                            tombstone
                        )),
                    )]),
                )
                .is_some()
        );
        assert!(
            coordinator
                .get_current_with_observations(
                    &path(),
                    &request,
                    &BTreeMap::from([(
                        key,
                        crate::node_system::analysis::ResourceObservedState::Absent(Some(
                            ResourceVersion::new("revision:8"),
                        )),
                    )]),
                )
                .is_none()
        );
    }

    #[test]
    fn same_revision_with_registry_or_resource_change_is_stale() {
        let mut slot = CompilationSlot::<&str, &str>::new(path());
        let task = started(slot.request(CompileId::new(1), basis(7, 1, "1")));

        let registry_report = slot.publish(
            &task,
            &basis(7, 2, "1"),
            CompileProducts {
                analysis: "analysis",
                has_blocking_diagnostics: false,
                plan: Some("plan"),
            },
        );
        assert_eq!(registry_report.analysis, PublishOutcome::Stale);

        let resource_report = slot.publish(
            &task,
            &basis(7, 1, "2"),
            CompileProducts {
                analysis: "analysis",
                has_blocking_diagnostics: false,
                plan: Some("plan"),
            },
        );
        assert_eq!(resource_report.analysis, PublishOutcome::Stale);
        assert!(slot.published_analysis().is_none());
        assert!(slot.published_plan().is_none());
    }

    #[test]
    fn blocking_analysis_is_published_without_a_plan() {
        let current = basis(1, 1, "1");
        let mut slot = CompilationSlot::new(path());
        let task = started(slot.request(CompileId::new(1), current.clone()));

        let report = slot.publish(
            &task,
            &current,
            CompileProducts {
                analysis: "blocking analysis",
                has_blocking_diagnostics: true,
                plan: Some("must not publish"),
            },
        );

        assert_eq!(report.analysis, PublishOutcome::Current);
        assert_eq!(report.plan, None);
        assert_eq!(
            slot.published_analysis().unwrap().payload,
            "blocking analysis"
        );
        assert!(slot.published_plan().is_none());
    }

    #[test]
    fn cancellation_is_observed_at_a_bounded_checkpoint() {
        let token = CompileCancellationToken::new();
        let canceller = token.clone();
        let mut observed_at = None;
        for checkpoint in 0..8 {
            if checkpoint == 3 {
                canceller.cancel();
            }
            if token.checkpoint().is_err() {
                observed_at = Some(checkpoint);
                break;
            }
        }

        assert_eq!(observed_at, Some(3));
    }

    #[test]
    fn matching_published_products_are_reused_by_exact_basis() {
        let coordinator = CompileCoordinator::<String, String>::new();
        let current = basis(1, 1, "1");
        let task = started(coordinator.request(path(), current.clone()));
        coordinator.publish(
            &task,
            &current,
            CompileProducts {
                analysis: "analysis".into(),
                has_blocking_diagnostics: false,
                plan: Some("plan".into()),
            },
        );

        let (analysis, plan) = coordinator.get_current(&path(), &current).unwrap();
        assert_eq!(analysis.compile_id, task.compile_id);
        assert_eq!(analysis.payload, "analysis");
        assert_eq!(plan.unwrap().payload, "plan");
        assert!(
            coordinator
                .get_current(&path(), &basis(1, 2, "1"))
                .is_none()
        );
        assert!(
            coordinator
                .get_current(&path(), &basis(1, 1, "2"))
                .is_none()
        );
    }

    #[test]
    fn demand_variant_cache_is_bounded_and_eviction_only_loses_reuse() {
        let mut cache = BoundedVariantCache::new(2);
        cache.insert(1_u8, "first");
        cache.insert(2_u8, "second");
        assert_eq!(cache.get(&1), Some(&"first"));

        cache.insert(3_u8, "third");

        assert_eq!(cache.len(), 2);
        assert_eq!(cache.get(&1), None);
        assert_eq!(cache.get(&2), Some(&"second"));
        assert_eq!(cache.get(&3), Some(&"third"));
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct VariantProduct {
        variants: Box<[&'static str]>,
    }

    #[test]
    fn populated_variants_are_not_current_for_registry_or_resource_basis_changes() {
        for changed in [basis(7, 2, "1"), basis(7, 1, "2")] {
            let mut slot = CompilationSlot::<&str, VariantProduct>::new(path());
            let original = basis(7, 1, "1");
            let old = started(slot.request(CompileId::new(1), original.clone()));
            slot.publish(
                &old,
                &original,
                CompileProducts {
                    analysis: "old analysis",
                    has_blocking_diagnostics: false,
                    plan: Some(VariantProduct {
                        variants: Box::new(["default", "preview"]),
                    }),
                },
            );
            assert_eq!(
                slot.current_products(&path(), &original)
                    .unwrap()
                    .1
                    .unwrap()
                    .payload
                    .variants
                    .len(),
                2,
            );
            assert!(slot.current_products(&path(), &changed).is_none());
            assert_eq!(slot.published_analysis().unwrap().payload, "old analysis");

            assert!(slot.finish(old.compile_id).is_none());
            let replacement = started(slot.request(CompileId::new(2), changed.clone()));
            slot.publish(
                &replacement,
                &changed,
                CompileProducts {
                    analysis: "new analysis",
                    has_blocking_diagnostics: false,
                    plan: Some(VariantProduct {
                        variants: Box::new([]),
                    }),
                },
            );
            let current = slot.current_products(&path(), &changed).unwrap();
            assert_eq!(current.0.payload, "new analysis");
            assert!(current.1.unwrap().payload.variants.is_empty());
        }
    }

    #[test]
    fn same_active_basis_joins_without_cancelling_or_duplicating_work() {
        let coordinator = CompileCoordinator::<(), ()>::new();
        let current = basis(1, 1, "1");
        let active = started(coordinator.request(path(), current.clone()));

        let joined_id = match coordinator.request(path(), current) {
            ScheduleOutcome::Coalesced { compile_id } => compile_id,
            _ => panic!("same-basis request must join active work"),
        };

        assert_eq!(joined_id, active.compile_id);
        assert!(!active.cancellation.is_cancelled());
        assert!(coordinator.finish(&path(), active.compile_id).is_none());
    }

    #[test]
    fn latest_different_basis_replaces_exactly_one_pending_task() {
        let coordinator = CompileCoordinator::<(), ()>::new();
        let active = started(coordinator.request(path(), basis(1, 1, "1")));
        let replaced_id = match coordinator.request(path(), basis(2, 1, "1")) {
            ScheduleOutcome::Coalesced { compile_id } => compile_id,
            _ => panic!("different basis must wait behind active work"),
        };
        let latest_basis = basis(3, 1, "1");
        let latest_id = match coordinator.request(path(), latest_basis.clone()) {
            ScheduleOutcome::Coalesced { compile_id } => compile_id,
            _ => panic!("latest basis must replace pending work"),
        };
        let joined_latest_id = match coordinator.request(path(), latest_basis.clone()) {
            ScheduleOutcome::Coalesced { compile_id } => compile_id,
            _ => panic!("same pending basis must join pending work"),
        };

        assert!(active.cancellation.is_cancelled());
        assert_ne!(replaced_id, latest_id);
        assert_eq!(joined_latest_id, latest_id);
        let promoted = coordinator.finish(&path(), active.compile_id).unwrap();
        assert_eq!(promoted.compile_id, latest_id);
        assert_eq!(promoted.basis, latest_basis);
    }

    #[test]
    fn waiter_wakes_when_matching_products_publish() {
        let coordinator = Arc::new(CompileCoordinator::<String, String>::new());
        let current = basis(1, 1, "1");
        let task = started(coordinator.request(path(), current.clone()));
        let (ready_tx, ready_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        let waiter = Arc::clone(&coordinator);
        let waiter_basis = current.clone();
        let compile_id = task.compile_id;
        let handle = thread::spawn(move || {
            ready_tx.send(()).unwrap();
            result_tx
                .send(waiter.wait_for_current(&path(), &waiter_basis, compile_id))
                .unwrap();
        });

        ready_rx.recv().unwrap();
        coordinator.publish(
            &task,
            &current,
            CompileProducts {
                analysis: "analysis".into(),
                has_blocking_diagnostics: false,
                plan: Some("plan".into()),
            },
        );

        let published = result_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("waiter did not wake after publish")
            .unwrap();
        assert_eq!(published.0.compile_id, compile_id);
        assert_eq!(published.1.unwrap().payload, "plan");
        handle.join().unwrap();
    }

    #[test]
    fn waiter_wakes_and_returns_none_when_its_slot_is_invalidated() {
        let coordinator = Arc::new(CompileCoordinator::<String, String>::new());
        let current = basis(1, 1, "1");
        let task = started(coordinator.request(path(), current.clone()));
        let (ready_tx, ready_rx) = mpsc::channel();
        let (result_tx, result_rx) = mpsc::channel();
        let waiter = Arc::clone(&coordinator);
        let compile_id = task.compile_id;
        let handle = thread::spawn(move || {
            ready_tx.send(()).unwrap();
            result_tx
                .send(waiter.wait_for_current(&path(), &current, compile_id))
                .unwrap();
        });

        ready_rx.recv().unwrap();
        coordinator.invalidate(&path());

        assert!(
            result_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("waiter did not wake after invalidation")
                .is_none()
        );
        assert!(task.cancellation.is_cancelled());
        handle.join().unwrap();
    }

    #[test]
    fn graph_invalidation_cancels_work_clears_products_and_rejects_stale_workers() {
        let coordinator = CompileCoordinator::<String, String>::new();
        let current = basis(1, 1, "1");
        let stale = started(coordinator.request(path(), current.clone()));
        coordinator.publish(
            &stale,
            &current,
            CompileProducts {
                analysis: "stale analysis".into(),
                has_blocking_diagnostics: false,
                plan: Some("stale plan".into()),
            },
        );

        coordinator.invalidate(&path());
        assert!(stale.cancellation.is_cancelled());
        assert!(coordinator.get_current(&path(), &current).is_none());

        let replacement = started(coordinator.request(path(), current.clone()));
        coordinator.publish(
            &replacement,
            &current,
            CompileProducts {
                analysis: "replacement analysis".into(),
                has_blocking_diagnostics: false,
                plan: Some("replacement plan".into()),
            },
        );
        let stale_report = coordinator.publish(
            &stale,
            &current,
            CompileProducts {
                analysis: "restored stale analysis".into(),
                has_blocking_diagnostics: false,
                plan: Some("restored stale plan".into()),
            },
        );

        assert_eq!(stale_report.analysis, PublishOutcome::Cancelled);
        let products = coordinator.get_current(&path(), &current).unwrap();
        assert_eq!(products.0.compile_id, replacement.compile_id);
        assert_eq!(products.1.unwrap().payload, "replacement plan");
    }

    #[test]
    fn all_invalidation_cancels_every_graph_and_clears_products() {
        let coordinator = CompileCoordinator::<String, String>::new();
        let other_path = GraphResourcePath("functions/other".into());
        let current = basis(1, 1, "1");
        let first = started(coordinator.request(path(), current.clone()));
        let second = started(coordinator.request(other_path.clone(), current.clone()));
        for task in [&first, &second] {
            coordinator.publish(
                task,
                &current,
                CompileProducts {
                    analysis: "analysis".into(),
                    has_blocking_diagnostics: false,
                    plan: Some("plan".into()),
                },
            );
        }

        coordinator.invalidate_all();

        assert!(first.cancellation.is_cancelled());
        assert!(second.cancellation.is_cancelled());
        assert!(coordinator.get_current(&path(), &current).is_none());
        assert!(coordinator.get_current(&other_path, &current).is_none());
    }

    #[test]
    fn stale_completion_cannot_restore_plan_after_newer_blocking_publication() {
        let coordinator = CompileCoordinator::<String, String>::new();
        let old_basis = basis(1, 1, "1");
        let old = started(coordinator.request(path(), old_basis.clone()));
        let new_basis = basis(2, 1, "1");
        let new_id = match coordinator.request(path(), new_basis.clone()) {
            ScheduleOutcome::Coalesced { compile_id } => compile_id,
            _ => panic!("new basis must wait behind active work"),
        };
        let new = coordinator.finish(&path(), old.compile_id).unwrap();
        assert_eq!(new.compile_id, new_id);
        coordinator.publish(
            &new,
            &new_basis,
            CompileProducts {
                analysis: "blocking analysis".into(),
                has_blocking_diagnostics: true,
                plan: Some("must be cleared".into()),
            },
        );

        let stale_report = coordinator.publish(
            &old,
            &old_basis,
            CompileProducts {
                analysis: "old analysis".into(),
                has_blocking_diagnostics: false,
                plan: Some("old plan".into()),
            },
        );

        assert_eq!(stale_report.analysis, PublishOutcome::Cancelled);
        let current = coordinator.get_current(&path(), &new_basis).unwrap();
        assert_eq!(current.0.compile_id, new_id);
        assert!(current.1.is_none());
    }
}
