//! Platform-neutral progress contract for project discovery and cleanup.

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
