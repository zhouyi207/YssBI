use super::{CancellationToken, RunError};
use crate::node_system::runtime::RunId;
use crate::project::ProjectSessionId;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

#[derive(Default)]
struct ProjectRuns {
    active: BTreeMap<ProjectSessionId, BTreeMap<RunId, CancellationToken>>,
    preparing: BTreeMap<ProjectSessionId, BTreeMap<u64, CancellationToken>>,
    finalizing: BTreeMap<ProjectSessionId, BTreeMap<u64, CancellationToken>>,
    draining: BTreeMap<ProjectSessionId, BTreeSet<uuid::Uuid>>,
}

pub struct ProjectRunRegistry {
    next_pre_run_id: AtomicU64,
    runs: Mutex<ProjectRuns>,
    drained: Condvar,
}

impl Default for ProjectRunRegistry {
    fn default() -> Self {
        Self {
            next_pre_run_id: AtomicU64::new(1),
            runs: Mutex::new(ProjectRuns::default()),
            drained: Condvar::new(),
        }
    }
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
        if runs.draining.contains_key(&project) {
            cancellation.cancel();
            return Err(ProjectRunRegistrationError::ProjectDraining(project));
        }
        let registration_id = self
            .next_pre_run_id
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                std::num::NonZeroU64::new(current)?.get().checked_add(1)
            })
            .map_err(|_| {
                cancellation.cancel();
                ProjectRunRegistrationError::RuntimeIdExhausted
            })?;
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
        if runs.draining.contains_key(&project) {
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

    pub fn cancel_run(&self, project: &ProjectSessionId, run_id: RunId) -> bool {
        let cancellation = self
            .runs
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .active
            .get(project)
            .and_then(|runs| runs.get(&run_id))
            .cloned();
        if let Some(cancellation) = cancellation {
            cancellation.cancel();
            true
        } else {
            false
        }
    }

    pub fn begin_drain(self: &Arc<Self>, project: &ProjectSessionId) -> ProjectRunDrainGuard {
        let mut runs = self.runs.lock().unwrap_or_else(|error| error.into_inner());
        let drain_id = uuid::Uuid::new_v4();
        runs.draining
            .entry(project.clone())
            .or_default()
            .insert(drain_id);
        if let Some(preparing) = runs.preparing.get(project) {
            for cancellation in preparing.values() {
                cancellation.cancel();
            }
        }
        let finalizing = runs
            .finalizing
            .get(project)
            .map(|registrations| registrations.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        if let Some(active) = runs.active.get(project) {
            for cancellation in active.values() {
                if !finalizing
                    .iter()
                    .any(|protected| cancellation.shares_state_with(protected))
                {
                    cancellation.cancel();
                }
            }
        }
        self.drained.notify_all();
        while runs.active.contains_key(project)
            || runs.preparing.contains_key(project)
            || runs.finalizing.contains_key(project)
        {
            runs = self
                .drained
                .wait(runs)
                .unwrap_or_else(|error| error.into_inner());
        }
        ProjectRunDrainGuard {
            registry: Arc::clone(self),
            project: project.clone(),
            drain_id,
        }
    }

    pub fn cancel_and_drain(self: &Arc<Self>, project: &ProjectSessionId) {
        drop(self.begin_drain(project));
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

    #[cfg(test)]
    pub(crate) fn wait_until_draining_for_test(
        &self,
        project: &ProjectSessionId,
        timeout: std::time::Duration,
    ) -> bool {
        let runs = self.runs.lock().unwrap_or_else(|error| error.into_inner());
        let (runs, _) = self
            .drained
            .wait_timeout_while(runs, timeout, |runs| !runs.draining.contains_key(project))
            .unwrap_or_else(|error| error.into_inner());
        runs.draining.contains_key(project)
    }

    #[cfg(test)]
    fn drain_count_for_test(&self, project: &ProjectSessionId) -> usize {
        self.runs
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .draining
            .get(project)
            .map(BTreeSet::len)
            .unwrap_or_default()
    }

    fn begin_finalization(
        &self,
        project: &ProjectSessionId,
        registration_id: u64,
        cancellation: &CancellationToken,
    ) -> Result<ProjectRunFinalization<'_>, ProjectRunRegistrationError> {
        let mut runs = self.runs.lock().unwrap_or_else(|error| error.into_inner());
        if runs.draining.contains_key(project) || cancellation.is_cancelled() {
            cancellation.cancel();
            return Err(ProjectRunRegistrationError::ProjectDraining(
                project.clone(),
            ));
        }
        let remove_project = if let Some(preparing) = runs.preparing.get_mut(project) {
            preparing.remove(&registration_id);
            preparing.is_empty()
        } else {
            false
        };
        if remove_project {
            runs.preparing.remove(project);
        }
        runs.finalizing
            .entry(project.clone())
            .or_default()
            .insert(registration_id, cancellation.clone());
        Ok(ProjectRunFinalization {
            registry: self,
            project: project.clone(),
            registration_id,
        })
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

    fn release_finalization(&self, project: &ProjectSessionId, registration_id: u64) {
        let mut runs = self.runs.lock().unwrap_or_else(|error| error.into_inner());
        let remove_project = if let Some(finalizing) = runs.finalizing.get_mut(project) {
            finalizing.remove(&registration_id);
            finalizing.is_empty()
        } else {
            false
        };
        if remove_project {
            runs.finalizing.remove(project);
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

    fn release_drain(&self, project: &ProjectSessionId, drain_id: uuid::Uuid) {
        let mut runs = self.runs.lock().unwrap_or_else(|error| error.into_inner());
        let remove_project = if let Some(drain_ids) = runs.draining.get_mut(project) {
            drain_ids.remove(&drain_id);
            drain_ids.is_empty()
        } else {
            false
        };
        if remove_project {
            runs.draining.remove(project);
        }
    }
}

pub struct ProjectRunDrainGuard {
    registry: Arc<ProjectRunRegistry>,
    project: ProjectSessionId,
    drain_id: uuid::Uuid,
}

impl Drop for ProjectRunDrainGuard {
    fn drop(&mut self) {
        self.registry.release_drain(&self.project, self.drain_id);
    }
}

pub struct ProjectPreRunRegistration<'a> {
    registry: &'a ProjectRunRegistry,
    project: ProjectSessionId,
    registration_id: u64,
}

impl ProjectPreRunRegistration<'_> {
    pub fn begin_finalization(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<ProjectRunFinalization<'_>, ProjectRunRegistrationError> {
        self.registry
            .begin_finalization(&self.project, self.registration_id, cancellation)
    }
}

impl Drop for ProjectPreRunRegistration<'_> {
    fn drop(&mut self) {
        self.registry
            .release_pre_run(&self.project, self.registration_id);
    }
}

pub struct ProjectRunFinalization<'a> {
    registry: &'a ProjectRunRegistry,
    project: ProjectSessionId,
    registration_id: u64,
}

impl Drop for ProjectRunFinalization<'_> {
    fn drop(&mut self) {
        self.registry
            .release_finalization(&self.project, self.registration_id);
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
    RuntimeIdExhausted,
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
            Self::RuntimeIdExhausted => {
                formatter.write_str("project pre-run registration ID space is exhausted")
            }
        }
    }
}

impl std::error::Error for ProjectRunRegistrationError {}

impl From<ProjectRunRegistrationError> for RunError {
    fn from(error: ProjectRunRegistrationError) -> Self {
        match error {
            ProjectRunRegistrationError::ProjectDraining(project) => {
                Self::ProjectDraining(project.as_str().into())
            }
            ProjectRunRegistrationError::RuntimeIdExhausted => Self::RuntimeIdExhausted,
        }
    }
}

#[cfg(all(test, any()))]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn pre_run_registration_exhaustion_is_typed_and_permanent() {
        let registry = ProjectRunRegistry {
            next_pre_run_id: AtomicU64::new(u64::MAX - 1),
            ..ProjectRunRegistry::default()
        };
        let session = ProjectSessionId::new("pre-run-id-exhaustion");
        let _last_registration = registry
            .track_pre_run(session.clone(), CancellationToken::new())
            .unwrap();

        for _ in 0..2 {
            assert_eq!(
                registry
                    .track_pre_run(session.clone(), CancellationToken::new())
                    .err(),
                Some(ProjectRunRegistrationError::RuntimeIdExhausted)
            );
        }
    }

    #[test]
    fn cancel_run_targets_only_the_requested_project_run() {
        let registry = ProjectRunRegistry::new();
        let target_session = ProjectSessionId::new("target");
        let other_session = ProjectSessionId::new("other");
        let target = CancellationToken::new();
        let other = CancellationToken::new();
        let _target_run = registry
            .track(target_session.clone(), RunId::new(41), target.clone())
            .unwrap();
        let _other_run = registry
            .track(other_session.clone(), RunId::new(41), other.clone())
            .unwrap();

        assert!(registry.cancel_run(&target_session, RunId::new(41)));
        assert!(target.is_cancelled());
        assert!(!other.is_cancelled());
        assert!(!registry.cancel_run(&target_session, RunId::new(99)));
    }

    #[test]
    fn nested_drain_guards_keep_admission_closed_until_last_drop() {
        let registry = Arc::new(ProjectRunRegistry::new());
        let session = ProjectSessionId::new("nested-drain");
        let first = registry.begin_drain(&session);
        let second = registry.begin_drain(&session);

        assert_eq!(registry.drain_count_for_test(&session), 2);
        assert!(matches!(
            registry.track_pre_run(session.clone(), CancellationToken::new()),
            Err(ProjectRunRegistrationError::ProjectDraining(_))
        ));
        assert!(matches!(
            registry.track(session.clone(), RunId::new(1), CancellationToken::new()),
            Err(ProjectRunRegistrationError::ProjectDraining(_))
        ));

        drop(first);
        assert_eq!(registry.drain_count_for_test(&session), 1);
        assert!(matches!(
            registry.track_pre_run(session.clone(), CancellationToken::new()),
            Err(ProjectRunRegistrationError::ProjectDraining(_))
        ));
        assert!(matches!(
            registry.track(session.clone(), RunId::new(2), CancellationToken::new()),
            Err(ProjectRunRegistrationError::ProjectDraining(_))
        ));

        drop(second);
        assert_eq!(registry.drain_count_for_test(&session), 0);
        drop(
            registry
                .track_pre_run(session.clone(), CancellationToken::new())
                .unwrap(),
        );
        drop(
            registry
                .track(session, RunId::new(3), CancellationToken::new())
                .unwrap(),
        );
    }

    #[test]
    fn drain_waits_for_an_already_started_finalization() {
        let registry = Arc::new(ProjectRunRegistry::new());
        let session = ProjectSessionId::new("finalizing");
        let cancellation = CancellationToken::new();
        let pre_run = registry
            .track_pre_run(session.clone(), cancellation.clone())
            .unwrap();
        let active = registry
            .track(session.clone(), RunId::new(1), cancellation.clone())
            .unwrap();
        let finalization = pre_run.begin_finalization(&cancellation).unwrap();
        let drain_registry = Arc::clone(&registry);
        let drain_session = session.clone();
        let (drained_tx, drained_rx) = std::sync::mpsc::channel();
        let drain = std::thread::spawn(move || {
            drain_registry.cancel_and_drain(&drain_session);
            drained_tx.send(()).unwrap();
        });

        assert!(registry.wait_until_draining_for_test(&session, std::time::Duration::from_secs(2)));
        assert!(
            drained_rx
                .recv_timeout(std::time::Duration::from_millis(50))
                .is_err()
        );
        assert!(!cancellation.is_cancelled());

        drop(finalization);
        assert!(
            drained_rx
                .recv_timeout(std::time::Duration::from_millis(50))
                .is_err()
        );
        drop(active);
        drained_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        drain.join().unwrap();
    }

    #[test]
    fn drain_started_before_finalization_rejects_the_commit_gate() {
        let registry = Arc::new(ProjectRunRegistry::new());
        let session = ProjectSessionId::new("draining-before-finalization");
        let cancellation = CancellationToken::new();
        let pre_run = registry
            .track_pre_run(session.clone(), cancellation.clone())
            .unwrap();
        let drain_registry = Arc::clone(&registry);
        let drain_session = session.clone();
        let drain = std::thread::spawn(move || {
            drain_registry.cancel_and_drain(&drain_session);
        });

        assert!(registry.wait_until_draining_for_test(&session, std::time::Duration::from_secs(2)));
        assert!(matches!(
            pre_run.begin_finalization(&cancellation),
            Err(ProjectRunRegistrationError::ProjectDraining(_))
        ));
        drop(pre_run);
        drain.join().unwrap();
    }

    #[test]
    fn project_drain_cancels_graph_compiler_with_the_pre_run_token() {
        struct EmptyResources;

        impl crate::graph::compiler::engine::ResourceSnapshot for EmptyResources {
            fn versions(&self) -> yss_graph_analysis_contract::ResourceVersionSet {
                Default::default()
            }
        }

        let registry = Arc::new(ProjectRunRegistry::new());
        let session = ProjectSessionId::new("project-a");
        let cancellation = CancellationToken::new();
        let compile_cancellation =
            crate::graph::compiler::engine::CompileCancellationToken::from_shared(
                cancellation.shared_flag(),
            );
        let lease = registry
            .track_pre_run(session.clone(), cancellation.clone())
            .unwrap();
        let (drained_tx, drained_rx) = std::sync::mpsc::channel();
        let worker_registry = Arc::clone(&registry);
        let worker_session = session.clone();
        let worker = std::thread::spawn(move || {
            worker_registry.cancel_and_drain(&worker_session);
            drained_tx.send(()).unwrap();
        });

        assert!(
            registry.wait_until_draining_for_test(&session, std::time::Duration::from_secs(5),),
            "project run registry must enter draining"
        );
        assert!(cancellation.is_cancelled());
        assert!(drained_rx.try_recv().is_err());

        let node_registry = yss_graph_catalog::build_builtin_node_system()
            .unwrap()
            .registry;
        let resources = EmptyResources;
        let compiler =
            crate::graph::compiler::engine::GraphCompiler::new(node_registry.as_ref(), &resources);
        let snapshot = compiler.snapshot(
            yss_graph_document::GraphResourcePath::new("events/cancelled.yssbi-event").unwrap(),
            &yss_graph_document::GraphDocument::default(),
        );
        assert!(
            compiler
                .compile_snapshot(&snapshot, &compile_cancellation)
                .is_err()
        );

        drop(lease);
        drained_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("project drain must finish after the pre-run lease drops");
        worker.join().unwrap();
    }
}
