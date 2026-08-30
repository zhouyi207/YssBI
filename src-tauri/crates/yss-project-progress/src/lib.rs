//! Platform-neutral progress contract for project discovery and cleanup.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectProgress {
    Scan(ProjectScanProgress),
    Cleanup(ProjectCleanupProgress),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectScanProgress {
    Scanning,
    Discovered { count: usize },
    Registering { current: usize, total: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProjectCleanupProgress {
    Checking { current: usize, total: usize },
    Removing { removed: usize, total: usize },
}

/// Output port implemented by delivery adapters such as a bounded IPC queue.
pub trait ProjectProgressSink: Send + Sync {
    fn publish(&self, progress: ProjectProgress);
}

/// Cancellation capability shared by project discovery and cleanup tasks.
///
/// The atomic flag stays private so callers cannot accidentally use a weaker
/// memory ordering or compare unrelated task handles by value.
#[derive(Clone, Debug)]
pub struct ProjectTaskCancellation {
    cancelled: Arc<AtomicBool>,
}

impl ProjectTaskCancellation {
    fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    fn is_same_task(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.cancelled, &other.cancelled)
    }
}

/// Owns the currently cancellable project picker task, if one is active.
pub struct ProjectTaskCancellationRegistry {
    active: Mutex<Option<ProjectTaskCancellation>>,
}

impl ProjectTaskCancellationRegistry {
    pub const fn new() -> Self {
        Self {
            active: Mutex::new(None),
        }
    }

    pub fn begin(&self) -> ProjectTaskCancellation {
        let cancellation = ProjectTaskCancellation::new();
        let previous = self
            .active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .replace(cancellation.clone());
        if let Some(previous) = previous {
            previous.cancel();
        }
        cancellation
    }

    pub fn cancel_active(&self) {
        if let Some(cancellation) = self
            .active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
        {
            cancellation.cancel();
        }
    }

    pub fn end(&self, cancellation: &ProjectTaskCancellation) {
        let mut active = self
            .active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if active
            .as_ref()
            .is_some_and(|current| current.is_same_task(cancellation))
        {
            *active = None;
        }
    }
}

impl Default for ProjectTaskCancellationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::ProjectTaskCancellationRegistry;

    #[test]
    fn active_task_can_be_cancelled_and_ended() {
        let registry = ProjectTaskCancellationRegistry::new();
        let cancellation = registry.begin();
        assert!(!cancellation.is_cancelled());

        registry.cancel_active();
        assert!(cancellation.is_cancelled());

        registry.end(&cancellation);
        let next = registry.begin();
        assert!(!next.is_cancelled());
    }

    #[test]
    fn beginning_new_task_cancels_previous_without_losing_new_active_task() {
        let registry = ProjectTaskCancellationRegistry::new();
        let stale = registry.begin();
        let current = registry.begin();

        assert!(stale.is_cancelled());
        assert!(!current.is_cancelled());

        registry.end(&stale);
        registry.cancel_active();

        assert!(current.is_cancelled());
    }

    #[test]
    fn ending_current_task_removes_it_from_active_registry() {
        let registry = ProjectTaskCancellationRegistry::new();
        let cancellation = registry.begin();

        registry.end(&cancellation);
        registry.cancel_active();

        assert!(!cancellation.is_cancelled());
    }
}
