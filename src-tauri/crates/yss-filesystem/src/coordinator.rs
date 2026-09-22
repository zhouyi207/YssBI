use crate::{FilesystemError, NormalizedRoot};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Condvar, Mutex};

#[derive(Default)]
struct RootLeaseState {
    reserved: BTreeSet<NormalizedRoot>,
    lifecycle_closed: BTreeSet<NormalizedRoot>,
    admitted: BTreeMap<NormalizedRoot, usize>,
}

#[derive(Default)]
struct RootLeaseRegistry {
    state: Mutex<RootLeaseState>,
    available: Condvar,
}

#[cfg(any(test, feature = "test-support"))]
#[derive(Default)]
struct FilesystemTestControls {
    fault: Mutex<Option<crate::FilesystemFaultPoint>>,
    rollback_fault: Mutex<bool>,
    rollback_hook: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    before_remove_hook: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    before_move_target_delete_hook: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

#[derive(Clone, Default)]
pub struct FilesystemCoordinator {
    registry: Arc<RootLeaseRegistry>,
    #[cfg(any(test, feature = "test-support"))]
    test_controls: Arc<FilesystemTestControls>,
}

impl FilesystemCoordinator {
    pub fn acquire(&self, root: NormalizedRoot) -> Result<FilesystemLeaseSet, FilesystemError> {
        self.acquire_many([root])
    }

    pub fn acquire_many<I>(&self, roots: I) -> Result<FilesystemLeaseSet, FilesystemError>
    where
        I: IntoIterator<Item = NormalizedRoot>,
    {
        let roots = roots.into_iter().collect::<BTreeSet<_>>();
        let mut state = self.lock_state();
        if let Some(root) = roots
            .iter()
            .find(|root| state.lifecycle_closed.contains(*root))
        {
            return Err(admission_closed(root));
        }
        for root in &roots {
            *state.admitted.entry(root.clone()).or_default() += 1;
        }
        state = self.wait_until_available(state, &roots);
        state.reserved.extend(roots.iter().cloned());
        drop(state);

        Ok(FilesystemLeaseSet {
            coordinator: self.clone(),
            roots: roots.into_iter().collect(),
            owns_admission: true,
        })
    }

    pub fn begin_root_lifecycle(
        &self,
        root: NormalizedRoot,
    ) -> Result<RootLifecycleGuard, FilesystemError> {
        let mut state = self.lock_state();
        if !state.lifecycle_closed.insert(root.clone()) {
            return Err(admission_closed(&root));
        }
        let roots = BTreeSet::from([root.clone()]);
        state = self.wait_until_available(state, &roots);
        state.reserved.insert(root.clone());
        drop(state);

        Ok(RootLifecycleGuard {
            coordinator: self.clone(),
            root: root.clone(),
            lease: Some(FilesystemLeaseSet {
                coordinator: self.clone(),
                roots: vec![root],
                owns_admission: false,
            }),
        })
    }

    fn acquire_lifecycle_lease(&self, root: &NormalizedRoot) -> FilesystemLeaseSet {
        let roots = BTreeSet::from([root.clone()]);
        let mut state = self.lock_state();
        state = self.wait_until_available(state, &roots);
        state.reserved.insert(root.clone());
        drop(state);
        FilesystemLeaseSet {
            coordinator: self.clone(),
            roots: vec![root.clone()],
            owns_admission: false,
        }
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, RootLeaseState> {
        self.registry
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn wait_until_available<'a>(
        &self,
        mut state: std::sync::MutexGuard<'a, RootLeaseState>,
        roots: &BTreeSet<NormalizedRoot>,
    ) -> std::sync::MutexGuard<'a, RootLeaseState> {
        while roots.iter().any(|root| state.reserved.contains(root)) {
            state = self
                .registry
                .available
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        state
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_filesystem_fault(&self, fault: Option<crate::FilesystemFaultPoint>) {
        *self.test_controls.fault.lock().unwrap() = fault;
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_filesystem_rollback_fault(&self, enabled: bool) {
        *self.test_controls.rollback_fault.lock().unwrap() = enabled;
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_filesystem_rollback_test_hook(&self, hook: Option<Arc<dyn Fn() + Send + Sync>>) {
        *self.test_controls.rollback_hook.lock().unwrap() = hook;
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_before_remove_mutation_hook(&self, hook: Option<Arc<dyn Fn() + Send + Sync>>) {
        *self.test_controls.before_remove_hook.lock().unwrap() = hook;
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn set_before_move_target_delete_hook(&self, hook: Option<Arc<dyn Fn() + Send + Sync>>) {
        *self
            .test_controls
            .before_move_target_delete_hook
            .lock()
            .unwrap() = hook;
    }

    #[cfg(any(test, feature = "test-support"))]
    fn take_fault(&self, point: crate::FilesystemFaultPoint) -> bool {
        let mut fault = self.test_controls.fault.lock().unwrap();
        if *fault == Some(point) {
            *fault = None;
            true
        } else {
            false
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    fn take_rollback_fault(&self) -> bool {
        std::mem::take(&mut *self.test_controls.rollback_fault.lock().unwrap())
    }

    #[cfg(any(test, feature = "test-support"))]
    fn run_rollback_hook(&self) {
        let hook = self.test_controls.rollback_hook.lock().unwrap().clone();
        if let Some(hook) = hook {
            hook();
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    fn run_before_remove_hook(&self) {
        let hook = self.test_controls.before_remove_hook.lock().unwrap().take();
        if let Some(hook) = hook {
            hook();
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    fn run_before_move_target_delete_hook(&self) {
        let hook = self
            .test_controls
            .before_move_target_delete_hook
            .lock()
            .unwrap()
            .take();
        if let Some(hook) = hook {
            hook();
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn lifecycle_state_for_test(&self, root: &NormalizedRoot) -> (bool, bool, usize) {
        let state = self.lock_state();
        (
            state.lifecycle_closed.contains(root),
            state.reserved.contains(root),
            state.admitted.get(root).copied().unwrap_or(0),
        )
    }
}

pub struct FilesystemLeaseSet {
    coordinator: FilesystemCoordinator,
    roots: Vec<NormalizedRoot>,
    owns_admission: bool,
}

impl FilesystemLeaseSet {
    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn take_fault(&self, point: crate::FilesystemFaultPoint) -> bool {
        self.coordinator.take_fault(point)
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn take_rollback_fault(&self) -> bool {
        self.coordinator.take_rollback_fault()
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn run_rollback_hook(&self) {
        self.coordinator.run_rollback_hook();
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn run_before_remove_hook(&self) {
        self.coordinator.run_before_remove_hook();
    }

    #[cfg(any(test, feature = "test-support"))]
    pub(super) fn run_before_move_target_delete_hook(&self) {
        self.coordinator.run_before_move_target_delete_hook();
    }

    pub fn roots(&self) -> &[NormalizedRoot] {
        &self.roots
    }

    pub fn contains(&self, root: &NormalizedRoot) -> bool {
        self.roots.binary_search(root).is_ok()
    }
}

impl Drop for FilesystemLeaseSet {
    fn drop(&mut self) {
        let mut state = self.coordinator.lock_state();
        for root in self.roots.iter().rev() {
            state.reserved.remove(root);
            if self.owns_admission
                && let Some(count) = state.admitted.get_mut(root)
            {
                *count -= 1;
                if *count == 0 {
                    state.admitted.remove(root);
                }
            }
        }
        drop(state);
        self.coordinator.registry.available.notify_all();
    }
}

pub struct RootLifecycleGuard {
    coordinator: FilesystemCoordinator,
    root: NormalizedRoot,
    lease: Option<FilesystemLeaseSet>,
}

impl RootLifecycleGuard {
    pub fn release_initial_and_drain(&mut self) {
        self.lease.take();
        let mut state = self.coordinator.lock_state();
        while state.admitted.get(&self.root).copied().unwrap_or(0) != 0 {
            state = self
                .coordinator
                .registry
                .available
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    pub fn acquire_final(&mut self) -> Result<(), FilesystemError> {
        if self.lease.is_none() {
            self.lease = Some(self.coordinator.acquire_lifecycle_lease(&self.root));
        }
        Ok(())
    }

    pub fn holds_lease(&self) -> bool {
        self.lease.is_some()
    }
}

impl Drop for RootLifecycleGuard {
    fn drop(&mut self) {
        self.lease.take();
        let mut state = self.coordinator.lock_state();
        state.lifecycle_closed.remove(&self.root);
        drop(state);
        self.coordinator.registry.available.notify_all();
    }
}

fn admission_closed(root: &NormalizedRoot) -> FilesystemError {
    FilesystemError::RootAdmissionClosed {
        message: format!(
            "new operations are rejected for '{}'",
            root.as_path().display()
        ),
    }
}
