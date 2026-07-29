use super::{NormalizedProjectRoot, ProjectFilesystemError};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Condvar, Mutex};

#[derive(Default)]
struct RootLeaseState {
    reserved: BTreeSet<NormalizedProjectRoot>,
    lifecycle_closed: BTreeSet<NormalizedProjectRoot>,
    admitted: BTreeMap<NormalizedProjectRoot, usize>,
    #[cfg(test)]
    release_trace: Vec<NormalizedProjectRoot>,
    #[cfg(test)]
    next_wait_observer: Option<std::sync::mpsc::Sender<()>>,
    #[cfg(test)]
    next_lifecycle_drain_observer: Option<std::sync::mpsc::Sender<()>>,
    #[cfg(test)]
    acquire_many_observer: Option<std::sync::mpsc::Sender<Vec<NormalizedProjectRoot>>>,
}

#[derive(Default)]
struct RootLeaseRegistry {
    state: Mutex<RootLeaseState>,
    available: Condvar,
}

#[derive(Clone, Default)]
pub struct ProjectFilesystemCoordinator {
    registry: Arc<RootLeaseRegistry>,
}

impl ProjectFilesystemCoordinator {
    pub fn acquire(
        &self,
        root: NormalizedProjectRoot,
    ) -> Result<ProjectFilesystemLeaseSet, ProjectFilesystemError> {
        self.acquire_many([root])
    }

    pub fn acquire_many<I>(
        &self,
        roots: I,
    ) -> Result<ProjectFilesystemLeaseSet, ProjectFilesystemError>
    where
        I: IntoIterator<Item = NormalizedProjectRoot>,
    {
        let roots = roots.into_iter().collect::<BTreeSet<_>>();
        let mut state = self.lock_state();
        #[cfg(test)]
        if let Some(observer) = state.acquire_many_observer.as_ref() {
            let _ = observer.send(roots.iter().cloned().collect());
        }
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

        Ok(ProjectFilesystemLeaseSet {
            coordinator: self.clone(),
            roots: roots.into_iter().collect(),
            owns_admission: true,
        })
    }

    pub fn begin_root_lifecycle(
        &self,
        root: NormalizedProjectRoot,
    ) -> Result<ProjectRootLifecycleGuard, ProjectFilesystemError> {
        let mut state = self.lock_state();
        if !state.lifecycle_closed.insert(root.clone()) {
            return Err(admission_closed(&root));
        }
        let roots = BTreeSet::from([root.clone()]);
        state = self.wait_until_available(state, &roots);
        state.reserved.insert(root.clone());
        drop(state);

        Ok(ProjectRootLifecycleGuard {
            coordinator: self.clone(),
            root: root.clone(),
            lease: Some(ProjectFilesystemLeaseSet {
                coordinator: self.clone(),
                roots: vec![root],
                owns_admission: false,
            }),
        })
    }

    fn acquire_lifecycle_lease(&self, root: &NormalizedProjectRoot) -> ProjectFilesystemLeaseSet {
        let roots = BTreeSet::from([root.clone()]);
        let mut state = self.lock_state();
        state = self.wait_until_available(state, &roots);
        state.reserved.insert(root.clone());
        drop(state);
        ProjectFilesystemLeaseSet {
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
        roots: &BTreeSet<NormalizedProjectRoot>,
    ) -> std::sync::MutexGuard<'a, RootLeaseState> {
        while roots.iter().any(|root| state.reserved.contains(root)) {
            #[cfg(test)]
            if let Some(observer) = state.next_wait_observer.take() {
                let _ = observer.send(());
            }
            state = self
                .registry
                .available
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        state
    }

    #[cfg(test)]
    pub(crate) fn observe_acquire_many_attempts(
        &self,
    ) -> std::sync::mpsc::Receiver<Vec<NormalizedProjectRoot>> {
        let (sender, receiver) = std::sync::mpsc::channel();
        self.lock_state().acquire_many_observer = Some(sender);
        receiver
    }

    #[cfg(test)]
    pub(crate) fn observe_next_wait(&self) -> std::sync::mpsc::Receiver<()> {
        let (sender, receiver) = std::sync::mpsc::channel();
        self.lock_state().next_wait_observer = Some(sender);
        receiver
    }

    #[cfg(test)]
    pub(crate) fn observe_next_lifecycle_drain(&self) -> std::sync::mpsc::Receiver<()> {
        let (sender, receiver) = std::sync::mpsc::channel();
        self.lock_state().next_lifecycle_drain_observer = Some(sender);
        receiver
    }

    #[cfg(test)]
    pub(crate) fn lifecycle_state_for_test(
        &self,
        root: &NormalizedProjectRoot,
    ) -> (bool, bool, usize) {
        let state = self.lock_state();
        (
            state.lifecycle_closed.contains(root),
            state.reserved.contains(root),
            state.admitted.get(root).copied().unwrap_or(0),
        )
    }

    #[cfg(test)]
    pub(crate) fn is_reserved_for_test(&self, root: &NormalizedProjectRoot) -> bool {
        self.lock_state().reserved.contains(root)
    }

    #[cfg(test)]
    pub(super) fn release_trace(&self) -> Vec<NormalizedProjectRoot> {
        self.lock_state().release_trace.clone()
    }
}

pub struct ProjectFilesystemLeaseSet {
    coordinator: ProjectFilesystemCoordinator,
    roots: Vec<NormalizedProjectRoot>,
    owns_admission: bool,
}

impl ProjectFilesystemLeaseSet {
    pub fn roots(&self) -> &[NormalizedProjectRoot] {
        &self.roots
    }

    pub fn contains(&self, root: &NormalizedProjectRoot) -> bool {
        self.roots.binary_search(root).is_ok()
    }
}

impl Drop for ProjectFilesystemLeaseSet {
    fn drop(&mut self) {
        let mut state = self.coordinator.lock_state();
        for root in self.roots.iter().rev() {
            state.reserved.remove(root);
            if self.owns_admission {
                if let Some(count) = state.admitted.get_mut(root) {
                    *count -= 1;
                    if *count == 0 {
                        state.admitted.remove(root);
                    }
                }
            }
            #[cfg(test)]
            state.release_trace.push(root.clone());
        }
        drop(state);
        self.coordinator.registry.available.notify_all();
    }
}

pub struct ProjectRootLifecycleGuard {
    coordinator: ProjectFilesystemCoordinator,
    root: NormalizedProjectRoot,
    lease: Option<ProjectFilesystemLeaseSet>,
}

impl ProjectRootLifecycleGuard {
    pub fn release_initial_and_drain(&mut self) {
        self.lease.take();
        let mut state = self.coordinator.lock_state();
        #[cfg(test)]
        if let Some(observer) = state.next_lifecycle_drain_observer.take() {
            let _ = observer.send(());
        }
        while state.admitted.get(&self.root).copied().unwrap_or(0) != 0 {
            state = self
                .coordinator
                .registry
                .available
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    pub fn acquire_final(&mut self) -> Result<(), ProjectFilesystemError> {
        if self.lease.is_none() {
            self.lease = Some(self.coordinator.acquire_lifecycle_lease(&self.root));
        }
        Ok(())
    }

    pub fn holds_lease(&self) -> bool {
        self.lease.is_some()
    }
}

impl Drop for ProjectRootLifecycleGuard {
    fn drop(&mut self) {
        self.lease.take();
        let mut state = self.coordinator.lock_state();
        state.lifecycle_closed.remove(&self.root);
        drop(state);
        self.coordinator.registry.available.notify_all();
    }
}

fn admission_closed(root: &NormalizedProjectRoot) -> ProjectFilesystemError {
    ProjectFilesystemError::ProjectLifecycleAdmissionClosed {
        message: format!(
            "new operations are rejected for '{}'",
            root.as_path().display()
        ),
    }
}
