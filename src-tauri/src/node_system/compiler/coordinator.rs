use crate::node_system::analysis::{
    CompilationBasis, CompileId, CompileProjection, ResourceVersionSet,
};
use crate::node_system::document::{GraphResourcePath, GraphRevision};
use crate::node_system::registry::RegistryFingerprint;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

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
            active.cancellation.cancel();
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
        let outcome = self.classify(task, current_basis);
        let plan_outcome = products
            .plan
            .as_ref()
            .filter(|_| !products.has_blocking_diagnostics)
            .map(|_| outcome);
        if outcome == PublishOutcome::Current {
            self.published_analysis = Some(CompileProjection {
                graph_path: self.graph_path.clone(),
                basis: task.basis.clone(),
                compile_id: task.compile_id,
                payload: products.analysis,
            });
            self.published_plan = if products.has_blocking_diagnostics {
                None
            } else {
                products.plan.map(|payload| CompileProjection {
                    graph_path: self.graph_path.clone(),
                    basis: task.basis.clone(),
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

    fn classify(
        &self,
        task: &CompilationTask,
        current_basis: &CompilationBasis<GraphRevision>,
    ) -> PublishOutcome {
        if task.cancellation.is_cancelled()
            || self.active.as_ref().map(|active| active.compile_id) != Some(task.compile_id)
        {
            PublishOutcome::Cancelled
        } else if &task.basis != current_basis {
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
}

impl<Analysis, Plan> Default for CompileCoordinator<Analysis, Plan> {
    fn default() -> Self {
        Self {
            next_compile_id: AtomicU64::new(1),
            slots: Mutex::new(BTreeMap::new()),
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
        let compile_id = CompileId::new(self.next_compile_id.fetch_add(1, Ordering::Relaxed));
        self.slots
            .lock()
            .expect("compile coordinator lock poisoned")
            .entry(graph_path.clone())
            .or_insert_with(|| CompilationSlot::new(graph_path))
            .request(compile_id, basis)
    }

    pub fn finish(
        &self,
        graph_path: &GraphResourcePath,
        compile_id: CompileId,
    ) -> Option<CompilationTask> {
        self.slots
            .lock()
            .expect("compile coordinator lock poisoned")
            .get_mut(graph_path)
            .and_then(|slot| slot.finish(compile_id))
    }

    pub fn publish(
        &self,
        task: &CompilationTask,
        current_basis: &CompilationBasis<GraphRevision>,
        products: CompileProducts<Analysis, Plan>,
    ) -> PublishReport {
        let mut slots = self
            .slots
            .lock()
            .expect("compile coordinator lock poisoned");
        let Some(slot) = slots.get_mut(&task.graph_path) else {
            return PublishReport {
                analysis: PublishOutcome::Cancelled,
                plan: products
                    .plan
                    .as_ref()
                    .filter(|_| !products.has_blocking_diagnostics)
                    .map(|_| PublishOutcome::Cancelled),
            };
        };
        slot.publish(task, current_basis, products)
    }
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::analysis::{ResourceKey, ResourceVersion};

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
        }
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
            ScheduleOutcome::Start(_) => panic!("second task must wait for cancelled active task"),
        };
        let third_basis = basis(3, 1, "1");
        let third_id = match coordinator.request(path(), third_basis.clone()) {
            ScheduleOutcome::Coalesced { compile_id } => compile_id,
            ScheduleOutcome::Start(_) => panic!("third task must replace pending task"),
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
}
