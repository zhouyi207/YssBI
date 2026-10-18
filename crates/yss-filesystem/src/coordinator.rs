use crate::NormalizedRoot;
use std::collections::BTreeSet;
use std::sync::{Arc, Condvar, Mutex};

#[derive(Default)]
struct RootLeaseState {
    reserved: BTreeSet<NormalizedRoot>,
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
    pub fn acquire(&self, root: NormalizedRoot) -> FilesystemLeaseSet {
        self.acquire_many([root])
    }

    pub fn acquire_many<I>(&self, roots: I) -> FilesystemLeaseSet
    where
        I: IntoIterator<Item = NormalizedRoot>,
    {
        let roots = roots.into_iter().collect::<BTreeSet<_>>();
        let mut state = self.lock_state();
        state = self.wait_until_available(state, &roots);
        state.reserved.extend(roots.iter().cloned());
        drop(state);

        FilesystemLeaseSet {
            coordinator: self.clone(),
            roots: roots.into_iter().collect(),
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
}

pub struct FilesystemLeaseSet {
    coordinator: FilesystemCoordinator,
    roots: Vec<NormalizedRoot>,
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

    pub fn contains(&self, root: &NormalizedRoot) -> bool {
        self.roots.binary_search(root).is_ok()
    }
}

impl Drop for FilesystemLeaseSet {
    fn drop(&mut self) {
        let mut state = self.coordinator.lock_state();
        for root in self.roots.iter().rev() {
            state.reserved.remove(root);
        }
        drop(state);
        self.coordinator.registry.available.notify_all();
    }
}
