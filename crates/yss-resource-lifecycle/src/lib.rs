//! Project-scoped resource lifecycle ownership: the newest registration for a resource owns it.
//!
//! Filesystem publication and project-session validation remain caller concerns.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use thiserror::Error;
use yss_graph_document::GraphResourcePath;
use yss_project_identity::ProjectInstanceId;

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ResourceLifecycleError {
    #[error("resource lifecycle transaction is busy: {message}")]
    TransactionBusy { message: String },
    #[error("stale resource lifecycle: {message}")]
    StaleLifecycle { message: String },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceLifecycleIntent {
    Load,
    Unload,
    Rename,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceLifecycleOwner {
    pub project_instance_id: ProjectInstanceId,
    pub resource_path: GraphResourcePath,
    pub intent: ResourceLifecycleIntent,
    registration_id: u64,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ResourceLifecycleKey {
    project_instance_id: ProjectInstanceId,
    resource_path: GraphResourcePath,
}

impl ResourceLifecycleOwner {
    fn matches_key(&self, key: &ResourceLifecycleKey) -> bool {
        self.project_instance_id == key.project_instance_id
            && self.resource_path == key.resource_path
    }

    fn key(&self) -> ResourceLifecycleKey {
        ResourceLifecycleKey {
            project_instance_id: self.project_instance_id.clone(),
            resource_path: self.resource_path.clone(),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum ResourceLifecycleRegistrationState {
    Live,
    Committed,
    Abandoned,
}

#[derive(Clone)]
struct ResourceLifecycleRegistration {
    owner: ResourceLifecycleOwner,
    predecessor: Option<u64>,
    state: ResourceLifecycleRegistrationState,
}

#[derive(Default)]
pub struct ResourceLifecycleState {
    owners: HashMap<ResourceLifecycleKey, u64>,
    registrations: HashMap<u64, ResourceLifecycleRegistration>,
    next_registration_id: u64,
}

#[derive(Clone, Default)]
pub struct ResourceLifecycleRegistry {
    state: Arc<Mutex<ResourceLifecycleState>>,
}

impl ResourceLifecycleRegistry {
    /// Supersedes the resource's current owner unless a rename holds it.
    pub fn register(
        &self,
        project_instance_id: &ProjectInstanceId,
        resource_path: &GraphResourcePath,
        intent: ResourceLifecycleIntent,
    ) -> Result<ResourceLifecycleGuard, ResourceLifecycleError> {
        let mut state = self.lock_state();
        let registration_id = state.next_registration_id.checked_add(1).ok_or_else(|| {
            ResourceLifecycleError::TransactionBusy {
                message: "resource lifecycle registration identity exhausted".into(),
            }
        })?;
        let owner = ResourceLifecycleOwner {
            project_instance_id: project_instance_id.clone(),
            resource_path: resource_path.clone(),
            intent,
            registration_id,
        };
        let key = owner.key();
        let predecessor = state.owners.get(&key).copied();
        if predecessor
            .and_then(|registration_id| state.registrations.get(&registration_id))
            .is_some_and(|current| current.owner.intent == ResourceLifecycleIntent::Rename)
        {
            return Err(ResourceLifecycleError::TransactionBusy {
                message: format!("rename is active for '{resource_path}'"),
            });
        }
        state.next_registration_id = registration_id;
        state.registrations.insert(
            registration_id,
            ResourceLifecycleRegistration {
                owner: owner.clone(),
                predecessor,
                state: ResourceLifecycleRegistrationState::Live,
            },
        );
        state.owners.insert(key, registration_id);
        Ok(ResourceLifecycleGuard {
            registry: self.clone(),
            owner,
            armed: true,
        })
    }

    pub fn validate(&self, owner: &ResourceLifecycleOwner) -> Result<(), ResourceLifecycleError> {
        self.boundary().validate(owner)
    }

    pub fn boundary(&self) -> ResourceLifecycleBoundary<'_> {
        self.boundary_recovering().0
    }

    pub fn boundary_recovering(&self) -> (ResourceLifecycleBoundary<'_>, bool) {
        let (state, recovered) = match self.state.lock() {
            Ok(state) => (state, false),
            Err(error) => (error.into_inner(), true),
        };
        (
            ResourceLifecycleBoundary {
                state,
                registry_state: &self.state,
            },
            recovered,
        )
    }

    pub fn clear_poison(&self) {
        self.state.clear_poison();
    }

    #[cfg(test)]
    pub(crate) fn entry_count(&self) -> usize {
        self.lock_state().owners.len()
    }

    #[cfg(test)]
    fn registration_count(&self) -> usize {
        self.lock_state().registrations.len()
    }

    fn lock_state(&self) -> MutexGuard<'_, ResourceLifecycleState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

pub struct ResourceLifecycleBoundary<'a> {
    state: MutexGuard<'a, ResourceLifecycleState>,
    registry_state: &'a Arc<Mutex<ResourceLifecycleState>>,
}

impl ResourceLifecycleBoundary<'_> {
    pub fn validate(&self, owner: &ResourceLifecycleOwner) -> Result<(), ResourceLifecycleError> {
        if self
            .state
            .owners
            .get(&owner.key())
            .is_some_and(|registration_id| *registration_id == owner.registration_id)
            && self
                .state
                .registrations
                .get(&owner.registration_id)
                .is_some_and(|registration| registration.owner == *owner)
        {
            Ok(())
        } else {
            Err(stale_owner_error(owner))
        }
    }

    pub fn commit_guard(
        &mut self,
        guard: &mut ResourceLifecycleGuard,
        intent: ResourceLifecycleIntent,
    ) -> Result<ResourceLifecycleOwner, ResourceLifecycleError> {
        if !Arc::ptr_eq(self.registry_state, &guard.registry.state) {
            return Err(stale_owner_error(&guard.owner));
        }
        self.validate(&guard.owner)?;
        let mut committed = guard.owner.clone();
        committed.intent = intent;
        let registration_id = guard.owner.registration_id;
        let key = committed.key();
        let registration = self
            .state
            .registrations
            .get_mut(&registration_id)
            .expect("validated lifecycle registration must exist");
        registration.owner = committed.clone();
        registration.predecessor = None;
        registration.state = ResourceLifecycleRegistrationState::Committed;
        self.state.registrations.retain(|id, registration| {
            *id == registration_id || !registration.owner.matches_key(&key)
        });
        guard.armed = false;
        Ok(committed)
    }

    pub fn take_state(&mut self) -> ResourceLifecycleState {
        // Retired guards can outlive project activation, so their registration
        // identities must never be reused within this registry.
        let empty = ResourceLifecycleState {
            next_registration_id: self.state.next_registration_id,
            ..Default::default()
        };
        std::mem::replace(&mut *self.state, empty)
    }
}

pub struct ResourceLifecycleGuard {
    registry: ResourceLifecycleRegistry,
    owner: ResourceLifecycleOwner,
    armed: bool,
}

impl ResourceLifecycleGuard {
    pub fn owner(&self) -> &ResourceLifecycleOwner {
        &self.owner
    }
}

impl std::fmt::Debug for ResourceLifecycleGuard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResourceLifecycleGuard")
            .field("owner", &self.owner)
            .field("armed", &self.armed)
            .finish()
    }
}

impl Drop for ResourceLifecycleGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let mut state = self.registry.lock_state();
        let registration_id = self.owner.registration_id;
        let key = self.owner.key();
        let Some(registration) = state.registrations.get_mut(&registration_id) else {
            return;
        };
        registration.state = ResourceLifecycleRegistrationState::Abandoned;
        if state.owners.get(&key) == Some(&registration_id) {
            if let Some(predecessor) = nearest_eligible_predecessor(&state, registration_id) {
                state.owners.insert(key.clone(), predecessor);
            } else {
                state.owners.remove(&key);
            }
        }
        compact_registration_chain(&mut state, &key);
    }
}

fn nearest_eligible_predecessor(
    state: &ResourceLifecycleState,
    registration_id: u64,
) -> Option<u64> {
    let predecessor = state.registrations.get(&registration_id)?.predecessor;
    nearest_eligible_registration(state, predecessor)
}

fn nearest_eligible_registration(
    state: &ResourceLifecycleState,
    mut registration_id: Option<u64>,
) -> Option<u64> {
    while let Some(current_id) = registration_id {
        let registration = state.registrations.get(&current_id)?;
        match registration.state {
            ResourceLifecycleRegistrationState::Live
            | ResourceLifecycleRegistrationState::Committed => {
                return Some(current_id);
            }
            ResourceLifecycleRegistrationState::Abandoned => {
                registration_id = registration.predecessor;
            }
        }
    }
    None
}

fn compact_registration_chain(state: &mut ResourceLifecycleState, key: &ResourceLifecycleKey) {
    let registration_ids = state
        .registrations
        .iter()
        .filter(|(_, registration)| registration.owner.matches_key(key))
        .map(|(registration_id, _)| *registration_id)
        .collect::<Vec<_>>();
    for registration_id in &registration_ids {
        let predecessor = state
            .registrations
            .get(registration_id)
            .and_then(|registration| registration.predecessor);
        let predecessor = nearest_eligible_registration(state, predecessor);
        if let Some(registration) = state.registrations.get_mut(registration_id) {
            registration.predecessor = predecessor;
        }
    }
    state.registrations.retain(|_, registration| {
        !registration.owner.matches_key(key)
            || registration.state != ResourceLifecycleRegistrationState::Abandoned
    });
}

fn stale_owner_error(owner: &ResourceLifecycleOwner) -> ResourceLifecycleError {
    ResourceLifecycleError::StaleLifecycle {
        message: format!(
            "stale resource lifecycle owner {} for '{}' in project instance '{}'",
            owner.registration_id, owner.resource_path, owner.project_instance_id
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use yss_graph_document::GraphResourcePath;

    fn project(label: &str) -> ProjectInstanceId {
        ProjectInstanceId::from_existing(format!("project-{label}"))
    }

    fn path(value: &str) -> GraphResourcePath {
        GraphResourcePath::new(value).unwrap()
    }

    fn assert_stale(error: ResourceLifecycleError) {
        assert!(matches!(
            error,
            ResourceLifecycleError::StaleLifecycle { .. }
        ));
    }

    fn assert_busy(error: ResourceLifecycleError) {
        assert!(matches!(
            error,
            ResourceLifecycleError::TransactionBusy { .. }
        ));
    }

    #[test]
    fn commit_rejects_a_guard_from_another_registry_and_preserves_its_release() {
        let first = ResourceLifecycleRegistry::default();
        let second = ResourceLifecycleRegistry::default();
        let session = project("registry-origin");
        let path = path("events/Shared.yssbi-event");
        let own = first
            .register(&session, &path, ResourceLifecycleIntent::Load)
            .unwrap();
        let mut foreign = second
            .register(&session, &path, ResourceLifecycleIntent::Load)
            .unwrap();
        let foreign_owner = foreign.owner().clone();

        let error = first
            .boundary()
            .commit_guard(&mut foreign, ResourceLifecycleIntent::Unload)
            .expect_err("foreign guards cannot be committed");
        assert_stale(error);
        first.validate(own.owner()).unwrap();
        second.validate(&foreign_owner).unwrap();
        drop(foreign);
        assert_stale(second.validate(&foreign_owner).unwrap_err());
    }

    #[test]
    fn distinct_graph_paths_have_independent_lifecycle_owners() {
        let registry = ResourceLifecycleRegistry::default();
        let session = project("independent-resource-paths");
        let event_guard = registry
            .register(
                &session,
                &path("events/Shared.yssbi-event"),
                ResourceLifecycleIntent::Rename,
            )
            .unwrap();
        let function_guard = registry
            .register(
                &session,
                &path("functions/Shared.yssbi-function"),
                ResourceLifecycleIntent::Rename,
            )
            .unwrap();

        registry.validate(event_guard.owner()).unwrap();
        registry.validate(function_guard.owner()).unwrap();
        assert_eq!(registry.entry_count(), 2);
    }

    #[test]
    fn retirement_removes_every_lifecycle_ownership() {
        let registry = ResourceLifecycleRegistry::default();
        let session = project("clear-shared-lifecycle");
        let loaded = registry
            .register(
                &session,
                &path("events/Clear.yssbi-event"),
                ResourceLifecycleIntent::Load,
            )
            .unwrap();
        let renamed = registry
            .register(
                &session,
                &path("events/Renamed.yssbi-event"),
                ResourceLifecycleIntent::Rename,
            )
            .unwrap();

        drop(registry.boundary_recovering().0.take_state());

        assert_eq!(registry.entry_count(), 0);
        assert_stale(registry.validate(loaded.owner()).unwrap_err());
        assert_stale(registry.validate(renamed.owner()).unwrap_err());
    }

    #[test]
    fn three_level_supersession_skips_abandoned_middle_and_restores_live_ancestor() {
        let registry = ResourceLifecycleRegistry::default();
        let session = project("three-level-live-ancestor");
        let graph_path = path("events/Shared.yssbi-event");
        let register = || {
            registry
                .register(&session, &graph_path, ResourceLifecycleIntent::Load)
                .unwrap()
        };
        let ancestor = register();
        let middle = register();
        let newest = register();

        drop(middle);
        drop(newest);

        registry.validate(ancestor.owner()).unwrap();
        drop(ancestor);
        assert_eq!(registry.entry_count(), 0);
        assert_eq!(registry.registration_count(), 0);
    }

    #[test]
    fn three_level_supersession_restores_committed_ancestor_past_abandoned_middle() {
        let registry = ResourceLifecycleRegistry::default();
        let session = project("three-level-committed-ancestor");
        let graph_path = path("events/Shared.yssbi-event");
        let register = || {
            registry
                .register(&session, &graph_path, ResourceLifecycleIntent::Load)
                .unwrap()
        };
        let mut ancestor = register();
        let ancestor_owner = ancestor.owner().clone();
        registry
            .boundary()
            .commit_guard(&mut ancestor, ResourceLifecycleIntent::Load)
            .unwrap();
        let middle = register();
        let newest = register();

        drop(middle);
        drop(newest);

        registry.validate(&ancestor_owner).unwrap();
        assert_eq!(registry.entry_count(), 1);
        assert_eq!(registry.registration_count(), 1);
    }

    #[test]
    fn concurrent_registrations_leave_exactly_one_owner() {
        let registry = ResourceLifecycleRegistry::default();
        let session = project("atomic-registration");
        let graph_path = path("functions/Shared.yssbi-function");
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let workers = (0..2)
            .map(|_| {
                let registry = registry.clone();
                let session = session.clone();
                let graph_path = graph_path.clone();
                let barrier = Arc::clone(&barrier);
                std::thread::spawn(move || {
                    barrier.wait();
                    registry
                        .register(&session, &graph_path, ResourceLifecycleIntent::Load)
                        .unwrap()
                })
            })
            .collect::<Vec<_>>();

        barrier.wait();
        let guards = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>();

        let valid = guards
            .iter()
            .filter(|guard| registry.validate(guard.owner()).is_ok())
            .count();
        assert_eq!(valid, 1);
    }

    #[test]
    fn retired_project_guard_cannot_abandon_a_replacement_registration() {
        let registry = ResourceLifecycleRegistry::default();
        let path = path("events/Shared.yssbi-event");
        let retired = registry
            .register(&project("retired"), &path, ResourceLifecycleIntent::Load)
            .unwrap();

        drop(registry.boundary().take_state());

        let replacement = project("replacement");
        let current = registry
            .register(&replacement, &path, ResourceLifecycleIntent::Load)
            .unwrap();
        let superseding = registry
            .register(&replacement, &path, ResourceLifecycleIntent::Load)
            .unwrap();
        assert_stale(registry.validate(retired.owner()).unwrap_err());

        drop(retired);
        drop(superseding);

        registry.validate(current.owner()).unwrap();
        drop(current);
        assert_eq!(registry.entry_count(), 0);
        assert_eq!(registry.registration_count(), 0);
    }

    #[test]
    fn old_project_owners_never_match_the_replacement_project() {
        let registry = ResourceLifecycleRegistry::default();
        let paths = [
            path("events/Load.yssbi-event"),
            path("events/Unload.yssbi-event"),
            path("events/Rename.yssbi-event"),
        ];
        let intents = [
            ResourceLifecycleIntent::Load,
            ResourceLifecycleIntent::Unload,
            ResourceLifecycleIntent::Rename,
        ];
        let register = |project: &ProjectInstanceId| {
            paths
                .iter()
                .zip(intents)
                .map(|(path, intent)| registry.register(project, path, intent).unwrap())
                .collect::<Vec<_>>()
        };
        let old_owners = register(&project("old"))
            .iter()
            .map(|guard| guard.owner().clone())
            .collect::<Vec<_>>();

        drop(registry.boundary_recovering().0.take_state());
        let replacement_guards = register(&project("replacement"));

        for owner in &old_owners {
            assert_stale(registry.validate(owner).unwrap_err());
        }
        for guard in &replacement_guards {
            registry.validate(guard.owner()).unwrap();
        }
    }

    #[test]
    fn unload_supersedes_load_and_rename_excludes_later_registrations() {
        let registry = ResourceLifecycleRegistry::default();
        let session = project("intent-exclusion");
        let graph_path = path("events/Shared.yssbi-event");
        let load = registry
            .register(&session, &graph_path, ResourceLifecycleIntent::Load)
            .unwrap();
        let unload = registry
            .register(&session, &graph_path, ResourceLifecycleIntent::Unload)
            .unwrap();

        assert_stale(registry.validate(load.owner()).unwrap_err());
        drop(unload);
        let rename = registry
            .register(&session, &graph_path, ResourceLifecycleIntent::Rename)
            .unwrap();
        assert_busy(
            registry
                .register(&session, &graph_path, ResourceLifecycleIntent::Load)
                .unwrap_err(),
        );
        registry.validate(rename.owner()).unwrap();
    }
}
