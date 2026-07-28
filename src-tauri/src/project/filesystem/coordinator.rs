use super::{NormalizedProjectRoot, ProjectFilesystemError};
use std::collections::BTreeSet;
use std::sync::{Arc, Condvar, Mutex};

#[derive(Default)]
struct RootLeaseState {
    reserved: BTreeSet<NormalizedProjectRoot>,
    #[cfg(test)]
    release_trace: Vec<NormalizedProjectRoot>,
    #[cfg(test)]
    next_wait_observer: Option<std::sync::mpsc::Sender<()>>,
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
        let mut state = self
            .registry
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
        state.reserved.extend(roots.iter().cloned());
        drop(state);

        Ok(ProjectFilesystemLeaseSet {
            coordinator: self.clone(),
            roots: roots.into_iter().collect(),
        })
    }

    #[cfg(test)]
    pub(super) fn observe_next_wait(&self) -> std::sync::mpsc::Receiver<()> {
        let (sender, receiver) = std::sync::mpsc::channel();
        self.registry
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .next_wait_observer = Some(sender);
        receiver
    }

    #[cfg(test)]
    pub(crate) fn is_reserved_for_test(&self, root: &NormalizedProjectRoot) -> bool {
        self.registry
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .reserved
            .contains(root)
    }

    #[cfg(test)]
    pub(super) fn release_trace(&self) -> Vec<NormalizedProjectRoot> {
        self.registry
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .release_trace
            .clone()
    }
}

pub struct ProjectFilesystemLeaseSet {
    coordinator: ProjectFilesystemCoordinator,
    roots: Vec<NormalizedProjectRoot>,
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
        let mut state = self
            .coordinator
            .registry
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for root in self.roots.iter().rev() {
            state.reserved.remove(root);
            #[cfg(test)]
            state.release_trace.push(root.clone());
        }
        drop(state);
        self.coordinator.registry.available.notify_all();
    }
}
