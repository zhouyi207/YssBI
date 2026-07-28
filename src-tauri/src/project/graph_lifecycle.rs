use crate::project::{
    GraphResourcePath, ProjectFilesystemError, ProjectInstanceId, ProjectSession,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GraphLifecycleIntent {
    Load,
    Unload,
    Rename,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphLifecycleOwner {
    pub project_instance_id: ProjectInstanceId,
    pub graph_path: GraphResourcePath,
    pub token: u64,
    pub intent: GraphLifecycleIntent,
    registration_id: u64,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct GraphLifecycleKey {
    project_instance_id: ProjectInstanceId,
    graph_path: GraphResourcePath,
}

impl GraphLifecycleOwner {
    fn key(&self) -> GraphLifecycleKey {
        GraphLifecycleKey {
            project_instance_id: self.project_instance_id.clone(),
            graph_path: self.graph_path.clone(),
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum GraphLifecycleRegistrationState {
    Live,
    Committed,
    Abandoned,
}

#[derive(Clone)]
struct GraphLifecycleRegistration {
    owner: GraphLifecycleOwner,
    predecessor: Option<u64>,
    state: GraphLifecycleRegistrationState,
}

#[derive(Default)]
pub(crate) struct GraphLifecycleState {
    owners: HashMap<GraphLifecycleKey, u64>,
    registrations: HashMap<u64, GraphLifecycleRegistration>,
    next_registration_id: u64,
}

#[derive(Clone, Default)]
pub struct GraphLifecycleRegistry {
    state: Arc<Mutex<GraphLifecycleState>>,
}

impl GraphLifecycleRegistry {
    pub fn register(
        &self,
        session: &ProjectSession,
        graph_path: &GraphResourcePath,
        token: u64,
        intent: GraphLifecycleIntent,
    ) -> Result<GraphLifecycleGuard, ProjectFilesystemError> {
        let mut state = self.lock_state();
        self.register_locked(&mut state, session, graph_path, token, intent)
    }

    pub(crate) fn allocate_and_register(
        &self,
        session: &ProjectSession,
        graph_path: &GraphResourcePath,
        intent: GraphLifecycleIntent,
    ) -> Result<GraphLifecycleGuard, ProjectFilesystemError> {
        let mut state = self.lock_state();
        let key = GraphLifecycleKey {
            project_instance_id: session.instance_id.clone(),
            graph_path: graph_path.clone(),
        };
        let token = state
            .owners
            .get(&key)
            .and_then(|registration_id| state.registrations.get(registration_id))
            .map(|registration| {
                registration.owner.token.checked_add(1).ok_or_else(|| {
                    ProjectFilesystemError::FilesystemTransactionBusy {
                        message: format!("graph lifecycle token exhausted for '{graph_path}'"),
                    }
                })
            })
            .transpose()?
            .unwrap_or(1);
        self.register_locked(&mut state, session, graph_path, token, intent)
    }

    fn register_locked(
        &self,
        state: &mut GraphLifecycleState,
        session: &ProjectSession,
        graph_path: &GraphResourcePath,
        token: u64,
        intent: GraphLifecycleIntent,
    ) -> Result<GraphLifecycleGuard, ProjectFilesystemError> {
        let next_registration_id = state.next_registration_id.checked_add(1).ok_or_else(|| {
            ProjectFilesystemError::FilesystemTransactionBusy {
                message: "graph lifecycle registration identity exhausted".into(),
            }
        })?;
        let owner = GraphLifecycleOwner {
            project_instance_id: session.instance_id.clone(),
            graph_path: graph_path.clone(),
            token,
            intent,
            registration_id: next_registration_id,
        };
        let key = owner.key();
        let predecessor = state.owners.get(&key).copied();
        let current =
            predecessor.and_then(|registration_id| state.registrations.get(&registration_id));
        validate_registration(current.map(|registration| &registration.owner), &owner)?;
        state.next_registration_id = next_registration_id;
        state.registrations.insert(
            next_registration_id,
            GraphLifecycleRegistration {
                owner: owner.clone(),
                predecessor,
                state: GraphLifecycleRegistrationState::Live,
            },
        );
        state.owners.insert(key, next_registration_id);
        Ok(GraphLifecycleGuard {
            registry: self.clone(),
            owner,
            armed: true,
        })
    }

    pub fn validate(&self, owner: &GraphLifecycleOwner) -> Result<(), ProjectFilesystemError> {
        self.boundary().validate(owner)
    }

    pub fn clear_for_project(&self, project_instance_id: &ProjectInstanceId) {
        let mut state = self.lock_state();
        state
            .owners
            .retain(|key, _| &key.project_instance_id != project_instance_id);
        state.registrations.retain(|_, registration| {
            &registration.owner.project_instance_id != project_instance_id
        });
    }

    pub(crate) fn boundary(&self) -> GraphLifecycleBoundary<'_> {
        self.boundary_recovering().0
    }

    pub(crate) fn boundary_recovering(&self) -> (GraphLifecycleBoundary<'_>, bool) {
        let (state, recovered) = match self.state.lock() {
            Ok(state) => (state, false),
            Err(error) => (error.into_inner(), true),
        };
        (GraphLifecycleBoundary { state }, recovered)
    }

    pub(crate) fn clear_poison(&self) {
        self.state.clear_poison();
    }

    #[cfg(test)]
    pub(crate) fn entry_count(&self) -> usize {
        self.lock_state().owners.len()
    }

    #[cfg(test)]
    pub(crate) fn boundary_is_available(&self) -> bool {
        self.state.try_lock().is_ok()
    }

    #[cfg(test)]
    fn registration_count(&self) -> usize {
        self.lock_state().registrations.len()
    }

    fn lock_state(&self) -> MutexGuard<'_, GraphLifecycleState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

pub(crate) struct GraphLifecycleBoundary<'a> {
    state: MutexGuard<'a, GraphLifecycleState>,
}

impl GraphLifecycleBoundary<'_> {
    pub(crate) fn validate(
        &self,
        owner: &GraphLifecycleOwner,
    ) -> Result<(), ProjectFilesystemError> {
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

    pub(crate) fn commit_guard(
        &mut self,
        guard: &mut GraphLifecycleGuard,
        intent: GraphLifecycleIntent,
    ) -> Result<GraphLifecycleOwner, ProjectFilesystemError> {
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
        registration.state = GraphLifecycleRegistrationState::Committed;
        self.state
            .registrations
            .retain(|id, registration| *id == registration_id || registration.owner.key() != key);
        guard.armed = false;
        Ok(committed)
    }

    pub(crate) fn take_state(&mut self) -> GraphLifecycleState {
        std::mem::take(&mut *self.state)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GraphLifecycleOperation {
    pub(crate) session: ProjectSession,
    pub(crate) owner: GraphLifecycleOwner,
}

impl GraphLifecycleOperation {
    pub(crate) fn from_guard(session: ProjectSession, guard: &GraphLifecycleGuard) -> Self {
        Self {
            owner: guard.owner.clone(),
            session,
        }
    }

    pub(crate) fn stale_error(&self) -> ProjectFilesystemError {
        ProjectFilesystemError::StaleProjectLifecycle {
            message: format!(
                "stale project lifecycle for graph '{}' in project instance '{}'",
                self.owner.graph_path, self.owner.project_instance_id
            ),
        }
    }
}

pub(crate) struct GraphRenameOwnershipLease {
    pub(crate) operation: GraphLifecycleOperation,
    guard: GraphLifecycleGuard,
}

impl GraphRenameOwnershipLease {
    pub(crate) fn new(operation: GraphLifecycleOperation, guard: GraphLifecycleGuard) -> Self {
        Self { operation, guard }
    }

    pub(crate) fn commit_with_boundary(
        &mut self,
        boundary: &mut GraphLifecycleBoundary<'_>,
    ) -> Result<(), ProjectFilesystemError> {
        boundary.commit_guard(&mut self.guard, GraphLifecycleIntent::Unload)?;
        Ok(())
    }
}

pub struct GraphLifecycleGuard {
    registry: GraphLifecycleRegistry,
    owner: GraphLifecycleOwner,
    armed: bool,
}

impl GraphLifecycleGuard {
    pub fn owner(&self) -> &GraphLifecycleOwner {
        &self.owner
    }
}

impl std::fmt::Debug for GraphLifecycleGuard {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("GraphLifecycleGuard")
            .field("owner", &self.owner)
            .field("armed", &self.armed)
            .finish()
    }
}

impl Drop for GraphLifecycleGuard {
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
        registration.state = GraphLifecycleRegistrationState::Abandoned;
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

fn nearest_eligible_predecessor(state: &GraphLifecycleState, registration_id: u64) -> Option<u64> {
    let predecessor = state.registrations.get(&registration_id)?.predecessor;
    nearest_eligible_registration(state, predecessor)
}

fn nearest_eligible_registration(
    state: &GraphLifecycleState,
    mut registration_id: Option<u64>,
) -> Option<u64> {
    while let Some(current_id) = registration_id {
        let registration = state.registrations.get(&current_id)?;
        match registration.state {
            GraphLifecycleRegistrationState::Live | GraphLifecycleRegistrationState::Committed => {
                return Some(current_id);
            }
            GraphLifecycleRegistrationState::Abandoned => {
                registration_id = registration.predecessor;
            }
        }
    }
    None
}

fn compact_registration_chain(state: &mut GraphLifecycleState, key: &GraphLifecycleKey) {
    let registration_ids = state
        .registrations
        .iter()
        .filter(|(_, registration)| registration.owner.key() == *key)
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
        registration.owner.key() != *key
            || registration.state != GraphLifecycleRegistrationState::Abandoned
    });
}

fn validate_registration(
    current: Option<&GraphLifecycleOwner>,
    next: &GraphLifecycleOwner,
) -> Result<(), ProjectFilesystemError> {
    let Some(current) = current else {
        return Ok(());
    };
    if current.intent == GraphLifecycleIntent::Rename {
        return Err(ProjectFilesystemError::FilesystemTransactionBusy {
            message: format!("rename is active for '{}'", next.graph_path),
        });
    }
    if next.token <= current.token {
        return Err(stale_owner_error(next));
    }
    Ok(())
}

fn stale_owner_error(owner: &GraphLifecycleOwner) -> ProjectFilesystemError {
    ProjectFilesystemError::StaleProjectLifecycle {
        message: format!(
            "stale graph lifecycle token {} for '{}' in project instance '{}'",
            owner.token, owner.graph_path, owner.project_instance_id
        ),
    }
}

#[cfg(test)]
mod tests {
    use crate::project::{
        GraphDocumentKind, GraphResourceDocument, GraphResourcePath, NormalizedProjectRoot,
        ProjectData, ProjectInstanceId, ProjectSession, ProjectState, fixtures,
    };
    use std::sync::Arc;

    fn session(label: &str) -> ProjectSession {
        ProjectSession {
            instance_id: ProjectInstanceId::new(),
            root: NormalizedProjectRoot::from_project_path(std::env::temp_dir().join(format!(
                "yssbi-lifecycle-session-{label}-{}",
                uuid::Uuid::new_v4()
            )))
            .unwrap(),
        }
    }

    #[test]
    fn duplicate_same_token_and_intent_registration_is_rejected() {
        let registry = super::GraphLifecycleRegistry::default();
        let session = session("duplicate-registration");
        let graph_path = GraphResourcePath::new("events/Shared.yssbi-event").unwrap();
        let owner = registry
            .register(&session, &graph_path, 7, super::GraphLifecycleIntent::Load)
            .unwrap();

        let error = registry
            .register(&session, &graph_path, 7, super::GraphLifecycleIntent::Load)
            .unwrap_err();

        assert_eq!(error.code(), "stale_project_lifecycle");
        registry.validate(owner.owner()).unwrap();
    }

    #[test]
    fn superseded_guards_drop_without_restoring_abandoned_owner_in_both_orders() {
        let session = session("guard-drop-orders");
        let graph_path = GraphResourcePath::new("events/Shared.yssbi-event").unwrap();

        for drop_old_first in [true, false] {
            let registry = super::GraphLifecycleRegistry::default();
            let old = registry
                .register(&session, &graph_path, 1, super::GraphLifecycleIntent::Load)
                .unwrap();
            let current = registry
                .register(&session, &graph_path, 2, super::GraphLifecycleIntent::Load)
                .unwrap();

            if drop_old_first {
                drop(old);
                registry.validate(current.owner()).unwrap();
                drop(current);
            } else {
                drop(current);
                registry.validate(old.owner()).unwrap();
                drop(old);
            }

            assert_eq!(registry.entry_count(), 0, "drop_old_first={drop_old_first}");
        }
    }

    #[test]
    fn three_level_supersession_skips_abandoned_middle_and_restores_live_ancestor() {
        let registry = super::GraphLifecycleRegistry::default();
        let session = session("three-level-live-ancestor");
        let graph_path = GraphResourcePath::new("events/Shared.yssbi-event").unwrap();
        let ancestor = registry
            .register(&session, &graph_path, 1, super::GraphLifecycleIntent::Load)
            .unwrap();
        let middle = registry
            .register(&session, &graph_path, 2, super::GraphLifecycleIntent::Load)
            .unwrap();
        let newest = registry
            .register(&session, &graph_path, 3, super::GraphLifecycleIntent::Load)
            .unwrap();

        drop(middle);
        drop(newest);

        registry.validate(ancestor.owner()).unwrap();
        drop(ancestor);
        assert_eq!(registry.entry_count(), 0);
        assert_eq!(registry.registration_count(), 0);
    }

    #[test]
    fn three_level_supersession_restores_committed_ancestor_past_abandoned_middle() {
        let registry = super::GraphLifecycleRegistry::default();
        let session = session("three-level-committed-ancestor");
        let graph_path = GraphResourcePath::new("events/Shared.yssbi-event").unwrap();
        let mut ancestor = registry
            .register(&session, &graph_path, 1, super::GraphLifecycleIntent::Load)
            .unwrap();
        let ancestor_owner = ancestor.owner().clone();
        registry
            .boundary()
            .commit_guard(&mut ancestor, super::GraphLifecycleIntent::Load)
            .unwrap();
        let middle = registry
            .register(&session, &graph_path, 2, super::GraphLifecycleIntent::Load)
            .unwrap();
        let newest = registry
            .register(&session, &graph_path, 3, super::GraphLifecycleIntent::Load)
            .unwrap();

        drop(middle);
        drop(newest);

        registry.validate(&ancestor_owner).unwrap();
        assert_eq!(registry.entry_count(), 1);
        assert_eq!(registry.registration_count(), 1);
    }

    #[test]
    fn three_level_guard_drop_orders_restore_latest_live_owner_without_leaking_records() {
        let session = session("three-level-drop-orders");
        let graph_path = GraphResourcePath::new("events/Shared.yssbi-event").unwrap();

        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let registry = super::GraphLifecycleRegistry::default();
            let mut guards = [
                Some(
                    registry
                        .register(&session, &graph_path, 1, super::GraphLifecycleIntent::Load)
                        .unwrap(),
                ),
                Some(
                    registry
                        .register(&session, &graph_path, 2, super::GraphLifecycleIntent::Load)
                        .unwrap(),
                ),
                Some(
                    registry
                        .register(&session, &graph_path, 3, super::GraphLifecycleIntent::Load)
                        .unwrap(),
                ),
            ];
            let owners = guards
                .each_ref()
                .map(|guard| guard.as_ref().unwrap().owner().clone());

            for dropped in order {
                drop(guards[dropped].take());
                if let Some(current) = guards.iter().rposition(Option::is_some) {
                    registry.validate(&owners[current]).unwrap();
                    for (index, owner) in owners.iter().enumerate() {
                        if index != current {
                            assert_eq!(
                                registry.validate(owner).unwrap_err().code(),
                                "stale_project_lifecycle"
                            );
                        }
                    }
                } else {
                    assert_eq!(registry.entry_count(), 0);
                    assert_eq!(registry.registration_count(), 0);
                }
            }
        }
    }

    #[test]
    fn commits_truncate_obsolete_ancestors_without_late_guard_resurrection() {
        let session = session("three-level-commit-orders");
        let graph_path = GraphResourcePath::new("events/Shared.yssbi-event").unwrap();

        let registry = super::GraphLifecycleRegistry::default();
        let ancestor = registry
            .register(&session, &graph_path, 1, super::GraphLifecycleIntent::Load)
            .unwrap();
        let mut middle = registry
            .register(&session, &graph_path, 2, super::GraphLifecycleIntent::Load)
            .unwrap();
        let middle_owner = middle.owner().clone();
        registry
            .boundary()
            .commit_guard(&mut middle, super::GraphLifecycleIntent::Load)
            .unwrap();
        let newest = registry
            .register(&session, &graph_path, 3, super::GraphLifecycleIntent::Load)
            .unwrap();
        drop(ancestor);
        drop(newest);
        registry.validate(&middle_owner).unwrap();
        assert_eq!(registry.entry_count(), 1);
        assert_eq!(registry.registration_count(), 1);

        let registry = super::GraphLifecycleRegistry::default();
        let ancestor = registry
            .register(&session, &graph_path, 1, super::GraphLifecycleIntent::Load)
            .unwrap();
        let middle = registry
            .register(&session, &graph_path, 2, super::GraphLifecycleIntent::Load)
            .unwrap();
        let mut newest = registry
            .register(&session, &graph_path, 3, super::GraphLifecycleIntent::Load)
            .unwrap();
        let newest_owner = newest.owner().clone();
        registry
            .boundary()
            .commit_guard(&mut newest, super::GraphLifecycleIntent::Load)
            .unwrap();
        drop(middle);
        drop(ancestor);
        registry.validate(&newest_owner).unwrap();
        assert_eq!(registry.entry_count(), 1);
        assert_eq!(registry.registration_count(), 1);
    }

    #[test]
    fn function_load_token_allocation_and_registration_are_atomic() {
        let registry = super::GraphLifecycleRegistry::default();
        let session = session("atomic-allocation");
        let graph_path = GraphResourcePath::new("functions/Shared.yssbi-function").unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let mut workers = Vec::new();

        for _ in 0..2 {
            let registry = registry.clone();
            let session = session.clone();
            let graph_path = graph_path.clone();
            let barrier = Arc::clone(&barrier);
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                registry
                    .allocate_and_register(&session, &graph_path, super::GraphLifecycleIntent::Load)
                    .unwrap()
            }));
        }

        barrier.wait();
        let mut guards = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>();
        guards.sort_by_key(|guard| guard.owner().token);

        assert_eq!(guards[0].owner().token, 1);
        assert_eq!(guards[1].owner().token, 2);
        assert_eq!(
            registry.validate(guards[0].owner()).unwrap_err().code(),
            "stale_project_lifecycle"
        );
        registry.validate(guards[1].owner()).unwrap();
    }

    #[test]
    fn old_project_load_unload_and_rename_tokens_never_match_replacement_project() {
        let registry = super::GraphLifecycleRegistry::default();
        let old = session("old");
        let replacement = session("replacement");
        let paths = [
            GraphResourcePath::new("events/Load.yssbi-event").unwrap(),
            GraphResourcePath::new("events/Unload.yssbi-event").unwrap(),
            GraphResourcePath::new("events/Rename.yssbi-event").unwrap(),
        ];
        let intents = [
            super::GraphLifecycleIntent::Load,
            super::GraphLifecycleIntent::Unload,
            super::GraphLifecycleIntent::Rename,
        ];
        let old_guards = paths
            .iter()
            .zip(intents)
            .map(|(path, intent)| registry.register(&old, path, 11, intent).unwrap())
            .collect::<Vec<_>>();
        let old_owners = old_guards
            .iter()
            .map(|guard| guard.owner().clone())
            .collect::<Vec<_>>();

        registry.clear_for_project(&old.instance_id);
        let replacement_guards = paths
            .iter()
            .zip(intents)
            .map(|(path, intent)| registry.register(&replacement, path, 11, intent).unwrap())
            .collect::<Vec<_>>();

        for owner in &old_owners {
            assert_eq!(
                registry.validate(owner).unwrap_err().code(),
                "stale_project_lifecycle"
            );
        }
        for guard in &replacement_guards {
            registry.validate(guard.owner()).unwrap();
        }
    }

    #[test]
    fn load_returns_projection_from_its_owned_committed_snapshot() {
        let graph_path = GraphResourcePath::new("events/Shared.yssbi-event").unwrap();
        let root = std::env::temp_dir().join(format!(
            "yssbi-lifecycle-projection-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut project_a = ProjectData::new();
        project_a.graphs.insert(
            graph_path.clone(),
            GraphResourceDocument::new("Shared", GraphDocumentKind::Event),
        );
        fixtures::write_project(&project_a, root.to_string_lossy().as_ref()).unwrap();
        fixtures::write_graph(&project_a, root.to_string_lossy().as_ref(), &graph_path).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
        let project_instance_id = state.capture_project_session().unwrap().instance_id;

        let mut project_b = project_a;
        project_b
            .graphs
            .get_mut(&graph_path)
            .unwrap()
            .document
            .revision = crate::node_system::document::GraphRevision::new(42);
        let replacement_state = state.clone();
        state.set_projection_test_hook(Arc::new(move || {
            replacement_state.activate_project_fixture("project-b".into(), project_b.clone());
            Ok(())
        }));

        let projection = state
            .load_graph_projection(&project_instance_id, &graph_path, 1, "en-US")
            .unwrap();

        assert_eq!(projection.source_revision, 0);
        assert_eq!(
            state.get_data().unwrap().graphs[&graph_path]
                .document
                .revision
                .get(),
            42
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn graph_load_reads_nested_layout_without_mutating_disk() {
        let root = std::env::temp_dir().join(format!(
            "yssbi-lifecycle-nested-load-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let flat_path = GraphResourcePath::new("events/Legacy.yssbi-event").unwrap();
        let nested_path = GraphResourcePath::new("events/Nested/Legacy.yssbi-event").unwrap();
        let mut project = ProjectData::new();
        project.graphs.insert(
            flat_path.clone(),
            GraphResourceDocument::new("Legacy", GraphDocumentKind::Event),
        );
        fixtures::write_project(&project, root.to_string_lossy().as_ref()).unwrap();
        fixtures::write_graph(&project, root.to_string_lossy().as_ref(), &flat_path).unwrap();
        let nested_dir = root.join("events/Nested");
        std::fs::create_dir_all(&nested_dir).unwrap();
        let nested_file = root.join(nested_path.as_str());
        let flattened_file = root.join(flat_path.as_str());
        std::fs::rename(&flattened_file, &nested_file).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
        let project_instance_id = state.capture_project_session().unwrap().instance_id;

        let projection = state
            .load_graph_projection(&project_instance_id, &nested_path, 1, "en-US")
            .unwrap();

        assert_eq!(projection.graph_path.as_ref(), nested_path.as_str());
        assert!(nested_file.is_file(), "graph load moved the nested graph");
        assert!(
            nested_dir.is_dir(),
            "graph load removed the nested directory"
        );
        assert!(
            !flattened_file.exists(),
            "graph load created a flattened graph"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unload_and_rename_intents_exclude_load_for_the_same_owner() {
        let registry = super::GraphLifecycleRegistry::default();
        let session = session("intent-exclusion");
        let graph_path = GraphResourcePath::new("events/Shared.yssbi-event").unwrap();
        let load = registry
            .register(&session, &graph_path, 1, super::GraphLifecycleIntent::Load)
            .unwrap();
        let unload = registry
            .register(
                &session,
                &graph_path,
                2,
                super::GraphLifecycleIntent::Unload,
            )
            .unwrap();

        assert_eq!(
            registry.validate(load.owner()).unwrap_err().code(),
            "stale_project_lifecycle"
        );
        assert_eq!(
            registry
                .register(&session, &graph_path, 2, super::GraphLifecycleIntent::Load,)
                .unwrap_err()
                .code(),
            "stale_project_lifecycle"
        );
        drop(unload);
        let rename = registry
            .register(
                &session,
                &graph_path,
                3,
                super::GraphLifecycleIntent::Rename,
            )
            .unwrap();
        assert_eq!(
            registry
                .register(&session, &graph_path, 4, super::GraphLifecycleIntent::Load,)
                .unwrap_err()
                .code(),
            "filesystem_transaction_busy"
        );
        registry.validate(rename.owner()).unwrap();
    }
}
