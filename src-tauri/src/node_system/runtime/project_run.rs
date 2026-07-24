use super::CancellationToken;
use crate::node_system::analysis::{ProjectSessionId, RunId};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex};

#[derive(Default)]
struct ProjectRuns {
    active: BTreeMap<ProjectSessionId, BTreeMap<RunId, CancellationToken>>,
    preparing: BTreeMap<ProjectSessionId, BTreeMap<u64, CancellationToken>>,
    draining: BTreeSet<ProjectSessionId>,
}

#[derive(Default)]
pub struct ProjectRunRegistry {
    next_pre_run_id: AtomicU64,
    runs: Mutex<ProjectRuns>,
    drained: Condvar,
}

impl ProjectRunRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn track_pre_run(
        &self,
        project: ProjectSessionId,
        cancellation: CancellationToken,
    ) -> Result<ProjectPreRunRegistration<'_>, ProjectRunRegistrationError> {
        let mut runs = self.runs.lock().unwrap_or_else(|error| error.into_inner());
        if runs.draining.contains(&project) {
            cancellation.cancel();
            return Err(ProjectRunRegistrationError::ProjectDraining(project));
        }
        let registration_id = self.next_pre_run_id.fetch_add(1, Ordering::Relaxed) + 1;
        runs.preparing
            .entry(project.clone())
            .or_default()
            .insert(registration_id, cancellation);
        Ok(ProjectPreRunRegistration {
            registry: self,
            project,
            registration_id,
        })
    }

    pub fn track(
        &self,
        project: ProjectSessionId,
        run_id: RunId,
        cancellation: CancellationToken,
    ) -> Result<ProjectRunRegistration<'_>, ProjectRunRegistrationError> {
        let mut runs = self.runs.lock().unwrap_or_else(|error| error.into_inner());
        if runs.draining.contains(&project) {
            cancellation.cancel();
            return Err(ProjectRunRegistrationError::ProjectDraining(project));
        }
        runs.active
            .entry(project.clone())
            .or_default()
            .insert(run_id, cancellation);
        Ok(ProjectRunRegistration {
            registry: self,
            project,
            run_id,
        })
    }

    pub fn cancel_and_drain(&self, project: &ProjectSessionId) {
        let mut runs = self.runs.lock().unwrap_or_else(|error| error.into_inner());
        runs.draining.insert(project.clone());
        if let Some(preparing) = runs.preparing.get(project) {
            for cancellation in preparing.values() {
                cancellation.cancel();
            }
        }
        if let Some(active) = runs.active.get(project) {
            for cancellation in active.values() {
                cancellation.cancel();
            }
        }
        while runs.active.contains_key(project) || runs.preparing.contains_key(project) {
            runs = self
                .drained
                .wait(runs)
                .unwrap_or_else(|error| error.into_inner());
        }
        runs.draining.remove(project);
    }

    pub fn active_run_count(&self) -> usize {
        self.runs
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .active
            .values()
            .map(BTreeMap::len)
            .sum()
    }

    fn release_pre_run(&self, project: &ProjectSessionId, registration_id: u64) {
        let mut runs = self.runs.lock().unwrap_or_else(|error| error.into_inner());
        let remove_project = if let Some(preparing) = runs.preparing.get_mut(project) {
            preparing.remove(&registration_id);
            preparing.is_empty()
        } else {
            false
        };
        if remove_project {
            runs.preparing.remove(project);
            self.drained.notify_all();
        }
    }

    fn release(&self, project: &ProjectSessionId, run_id: RunId) {
        let mut runs = self.runs.lock().unwrap_or_else(|error| error.into_inner());
        let remove_project = if let Some(active) = runs.active.get_mut(project) {
            active.remove(&run_id);
            active.is_empty()
        } else {
            false
        };
        if remove_project {
            runs.active.remove(project);
            self.drained.notify_all();
        }
    }
}

pub struct ProjectPreRunRegistration<'a> {
    registry: &'a ProjectRunRegistry,
    project: ProjectSessionId,
    registration_id: u64,
}

impl Drop for ProjectPreRunRegistration<'_> {
    fn drop(&mut self) {
        self.registry
            .release_pre_run(&self.project, self.registration_id);
    }
}

pub struct ProjectRunRegistration<'a> {
    registry: &'a ProjectRunRegistry,
    project: ProjectSessionId,
    run_id: RunId,
}

impl Drop for ProjectRunRegistration<'_> {
    fn drop(&mut self) {
        self.registry.release(&self.project, self.run_id);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectRunRegistrationError {
    ProjectDraining(ProjectSessionId),
}

impl fmt::Display for ProjectRunRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProjectDraining(project) => {
                write!(
                    formatter,
                    "project session '{}' is draining",
                    project.as_str()
                )
            }
        }
    }
}

impl std::error::Error for ProjectRunRegistrationError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn project_drain_cancels_graph_compiler_with_the_pre_run_token() {
        struct EmptyResources;

        impl crate::node_system::compiler::ResourceSnapshot for EmptyResources {
            fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
                Default::default()
            }
        }

        let registry = Arc::new(ProjectRunRegistry::new());
        let session = ProjectSessionId::new("project-a");
        let cancellation = CancellationToken::new();
        let compile_cancellation =
            crate::node_system::compiler::CompileCancellationToken::from_shared(
                cancellation.shared_flag(),
            );
        let lease = registry
            .track_pre_run(session.clone(), cancellation.clone())
            .unwrap();
        let drained = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let worker_registry = Arc::clone(&registry);
        let worker_session = session.clone();
        let worker_drained = Arc::clone(&drained);
        let worker = std::thread::spawn(move || {
            worker_registry.cancel_and_drain(&worker_session);
            worker_drained.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        for _ in 0..100 {
            if cancellation.is_cancelled() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(cancellation.is_cancelled());
        assert!(!drained.load(std::sync::atomic::Ordering::SeqCst));

        let (provider, _) = crate::node_system::catalog::build_builtin_provider();
        let mut registry_builder = crate::node_system::registry::NodeRegistryBuilder::new();
        registry_builder.register_provider(provider).unwrap();
        let node_registry = registry_builder.freeze().unwrap();
        let resources = EmptyResources;
        let compiler = crate::node_system::compiler::GraphCompiler::new(&node_registry, &resources);
        let snapshot = compiler.snapshot(
            crate::node_system::document::GraphResourcePath("events/cancelled".into()),
            &crate::node_system::document::GraphDocument::default(),
        );
        assert!(
            compiler
                .compile_snapshot(&snapshot, &compile_cancellation)
                .is_err()
        );

        drop(lease);
        worker.join().unwrap();
        assert!(drained.load(std::sync::atomic::Ordering::SeqCst));
    }
}
