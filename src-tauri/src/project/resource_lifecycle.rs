use crate::project::{
    GraphResourcePath, ProjectFilesystemError, ProjectInstanceId, ProjectSession,
    WorksheetResourcePath,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum LifecycleResourcePath {
    Graph(GraphResourcePath),
    Worksheet(WorksheetResourcePath),
}

impl From<&GraphResourcePath> for LifecycleResourcePath {
    fn from(path: &GraphResourcePath) -> Self {
        Self::Graph(path.clone())
    }
}

impl From<&WorksheetResourcePath> for LifecycleResourcePath {
    fn from(path: &WorksheetResourcePath) -> Self {
        Self::Worksheet(path.clone())
    }
}

impl From<&LifecycleResourcePath> for LifecycleResourcePath {
    fn from(path: &LifecycleResourcePath) -> Self {
        path.clone()
    }
}

impl std::fmt::Display for LifecycleResourcePath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graph(path) => formatter.write_str(path.as_str()),
            Self::Worksheet(path) => formatter.write_str(path.as_str()),
        }
    }
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
    pub resource_path: LifecycleResourcePath,
    pub token: u64,
    pub intent: ResourceLifecycleIntent,
    registration_id: u64,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct ResourceLifecycleKey {
    project_instance_id: ProjectInstanceId,
    resource_path: LifecycleResourcePath,
}

impl ResourceLifecycleOwner {
    fn key(&self) -> ResourceLifecycleKey {
        ResourceLifecycleKey {
            project_instance_id: self.project_instance_id.clone(),
            resource_path: self.resource_path.clone(),
        }
    }

    pub(crate) fn graph_path(&self) -> &GraphResourcePath {
        match &self.resource_path {
            LifecycleResourcePath::Graph(path) => path,
            LifecycleResourcePath::Worksheet(_) => {
                unreachable!("worksheet lifecycle owner cannot drive graph runtime state")
            }
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
pub(crate) struct ResourceLifecycleState {
    owners: HashMap<ResourceLifecycleKey, u64>,
    client_tokens: HashMap<ResourceLifecycleKey, u64>,
    issued_internal_tokens: HashMap<ResourceLifecycleKey, u64>,
    registrations: HashMap<u64, ResourceLifecycleRegistration>,
    next_registration_id: u64,
}

#[derive(Clone, Default)]
pub struct ResourceLifecycleRegistry {
    state: Arc<Mutex<ResourceLifecycleState>>,
}

impl ResourceLifecycleRegistry {
    pub fn register(
        &self,
        session: &ProjectSession,
        resource_path: impl Into<LifecycleResourcePath>,
        token: u64,
        intent: ResourceLifecycleIntent,
    ) -> Result<ResourceLifecycleGuard, ProjectFilesystemError> {
        let mut state = self.lock_state();
        self.register_locked(
            &mut state,
            session,
            resource_path.into(),
            token,
            intent,
            Some(token),
        )
    }

    pub(crate) fn allocate_and_register(
        &self,
        session: &ProjectSession,
        resource_path: impl Into<LifecycleResourcePath>,
        intent: ResourceLifecycleIntent,
    ) -> Result<ResourceLifecycleGuard, ProjectFilesystemError> {
        let mut state = self.lock_state();
        let resource_path = resource_path.into();
        let key = ResourceLifecycleKey {
            project_instance_id: session.instance_id.clone(),
            resource_path: resource_path.clone(),
        };
        let current_token = state
            .owners
            .get(&key)
            .and_then(|registration_id| state.registrations.get(registration_id))
            .map(|registration| registration.owner.token);
        let latest_issued = state.issued_internal_tokens.get(&key).copied();
        let token = current_token
            .into_iter()
            .chain(latest_issued)
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| ProjectFilesystemError::FilesystemTransactionBusy {
                message: format!("resource lifecycle token exhausted for '{resource_path}'"),
            })?;
        let guard =
            self.register_locked(&mut state, session, resource_path, token, intent, None)?;
        state.issued_internal_tokens.insert(key, token);
        Ok(guard)
    }

    fn register_locked(
        &self,
        state: &mut ResourceLifecycleState,
        session: &ProjectSession,
        resource_path: LifecycleResourcePath,
        token: u64,
        intent: ResourceLifecycleIntent,
        client_token: Option<u64>,
    ) -> Result<ResourceLifecycleGuard, ProjectFilesystemError> {
        let next_registration_id = state.next_registration_id.checked_add(1).ok_or_else(|| {
            ProjectFilesystemError::FilesystemTransactionBusy {
                message: "resource lifecycle registration identity exhausted".into(),
            }
        })?;
        let owner = ResourceLifecycleOwner {
            project_instance_id: session.instance_id.clone(),
            resource_path,
            token,
            intent,
            registration_id: next_registration_id,
        };
        let key = owner.key();
        let predecessor = state.owners.get(&key).copied();
        let current =
            predecessor.and_then(|registration_id| state.registrations.get(&registration_id));
        validate_registration(
            current.map(|registration| &registration.owner),
            &owner,
            state.client_tokens.get(&key).copied(),
            client_token,
        )?;
        state.next_registration_id = next_registration_id;
        state.registrations.insert(
            next_registration_id,
            ResourceLifecycleRegistration {
                owner: owner.clone(),
                predecessor,
                state: ResourceLifecycleRegistrationState::Live,
            },
        );
        if let Some(client_token) = client_token {
            state.client_tokens.insert(key.clone(), client_token);
        }
        state.owners.insert(key, next_registration_id);
        Ok(ResourceLifecycleGuard {
            registry: self.clone(),
            owner,
            armed: true,
        })
    }

    pub fn validate(&self, owner: &ResourceLifecycleOwner) -> Result<(), ProjectFilesystemError> {
        self.boundary().validate(owner)
    }

    pub fn clear_for_project(&self, project_instance_id: &ProjectInstanceId) {
        let mut state = self.lock_state();
        state
            .owners
            .retain(|key, _| &key.project_instance_id != project_instance_id);
        state
            .client_tokens
            .retain(|key, _| &key.project_instance_id != project_instance_id);
        state
            .issued_internal_tokens
            .retain(|key, _| &key.project_instance_id != project_instance_id);
        state.registrations.retain(|_, registration| {
            &registration.owner.project_instance_id != project_instance_id
        });
    }

    pub(crate) fn boundary(&self) -> ResourceLifecycleBoundary<'_> {
        self.boundary_recovering().0
    }

    pub(crate) fn boundary_recovering(&self) -> (ResourceLifecycleBoundary<'_>, bool) {
        let (state, recovered) = match self.state.lock() {
            Ok(state) => (state, false),
            Err(error) => (error.into_inner(), true),
        };
        (ResourceLifecycleBoundary { state }, recovered)
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

    fn lock_state(&self) -> MutexGuard<'_, ResourceLifecycleState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

pub(crate) struct ResourceLifecycleBoundary<'a> {
    state: MutexGuard<'a, ResourceLifecycleState>,
}

impl ResourceLifecycleBoundary<'_> {
    pub(crate) fn validate(
        &self,
        owner: &ResourceLifecycleOwner,
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
        guard: &mut ResourceLifecycleGuard,
        intent: ResourceLifecycleIntent,
    ) -> Result<ResourceLifecycleOwner, ProjectFilesystemError> {
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
        self.state
            .registrations
            .retain(|id, registration| *id == registration_id || registration.owner.key() != key);
        guard.armed = false;
        Ok(committed)
    }

    pub(crate) fn take_state(&mut self) -> ResourceLifecycleState {
        std::mem::take(&mut *self.state)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResourceLifecycleOperation {
    pub(crate) session: ProjectSession,
    pub(crate) owner: ResourceLifecycleOwner,
}

impl ResourceLifecycleOperation {
    pub(crate) fn from_guard(session: ProjectSession, guard: &ResourceLifecycleGuard) -> Self {
        Self {
            owner: guard.owner.clone(),
            session,
        }
    }

    pub(crate) fn stale_error(&self) -> ProjectFilesystemError {
        ProjectFilesystemError::StaleProjectLifecycle {
            message: format!(
                "stale project lifecycle for resource '{}' in project instance '{}'",
                self.owner.resource_path, self.owner.project_instance_id
            ),
        }
    }
}

pub(crate) struct ResourceRenameOwnershipLease {
    pub(crate) operation: ResourceLifecycleOperation,
    guard: ResourceLifecycleGuard,
}

impl ResourceRenameOwnershipLease {
    pub(crate) fn new(
        operation: ResourceLifecycleOperation,
        guard: ResourceLifecycleGuard,
    ) -> Self {
        Self { operation, guard }
    }

    pub(crate) fn commit_with_boundary(
        &mut self,
        boundary: &mut ResourceLifecycleBoundary<'_>,
    ) -> Result<(), ProjectFilesystemError> {
        boundary.commit_guard(&mut self.guard, ResourceLifecycleIntent::Unload)?;
        Ok(())
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
            || registration.state != ResourceLifecycleRegistrationState::Abandoned
    });
}

fn validate_registration(
    current: Option<&ResourceLifecycleOwner>,
    next: &ResourceLifecycleOwner,
    latest_client_token: Option<u64>,
    client_token: Option<u64>,
) -> Result<(), ProjectFilesystemError> {
    if current.is_some_and(|owner| owner.intent == ResourceLifecycleIntent::Rename) {
        return Err(ProjectFilesystemError::FilesystemTransactionBusy {
            message: format!("rename is active for '{}'", next.resource_path),
        });
    }
    if client_token.is_some_and(|token| latest_client_token.is_some_and(|latest| token <= latest)) {
        return Err(stale_owner_error(next));
    }
    Ok(())
}

fn stale_owner_error(owner: &ResourceLifecycleOwner) -> ProjectFilesystemError {
    ProjectFilesystemError::StaleResourceLifecycle {
        message: format!(
            "stale resource lifecycle token {} for '{}' in project instance '{}'",
            owner.token, owner.resource_path, owner.project_instance_id
        ),
    }
}

#[cfg(test)]
mod tests {
    use crate::project::{
        GraphDocumentKind, GraphResourceDocument, GraphResourcePath, NormalizedProjectRoot,
        ProjectData, ProjectInstanceId, ProjectSession, ProjectState, WorksheetResourcePath,
        fixtures,
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
    fn graph_and_worksheet_paths_have_independent_lifecycle_owners() {
        let registry = super::ResourceLifecycleRegistry::default();
        let session = session("independent-resource-kinds");
        let graph = super::LifecycleResourcePath::Graph(
            GraphResourcePath::new("events/Shared.yssbi-event").unwrap(),
        );
        let worksheet = super::LifecycleResourcePath::Worksheet(
            WorksheetResourcePath::parse("worksheets/Shared.yssbi-worksheet").unwrap(),
        );

        let graph_guard = registry
            .register(&session, &graph, 1, super::ResourceLifecycleIntent::Rename)
            .unwrap();
        let worksheet_guard = registry
            .register(
                &session,
                &worksheet,
                1,
                super::ResourceLifecycleIntent::Rename,
            )
            .unwrap();

        registry.validate(graph_guard.owner()).unwrap();
        registry.validate(worksheet_guard.owner()).unwrap();
        assert_eq!(registry.entry_count(), 2);
    }

    #[test]
    fn client_tokens_are_monotonic_per_resource_path() {
        let registry = super::ResourceLifecycleRegistry::default();
        let session = session("monotonic-resource-token");
        let worksheet = super::LifecycleResourcePath::Worksheet(
            WorksheetResourcePath::parse("worksheets/Report.yssbi-worksheet").unwrap(),
        );
        let first = registry
            .register(
                &session,
                &worksheet,
                7,
                super::ResourceLifecycleIntent::Rename,
            )
            .unwrap();
        drop(first);

        let error = registry
            .register(
                &session,
                &worksheet,
                7,
                super::ResourceLifecycleIntent::Rename,
            )
            .unwrap_err();

        assert_eq!(error.code(), "stale_resource_lifecycle");
        registry
            .register(
                &session,
                &worksheet,
                8,
                super::ResourceLifecycleIntent::Rename,
            )
            .unwrap();
    }

    #[test]
    fn clearing_project_removes_graph_and_worksheet_lifecycle_ownership() {
        let registry = super::ResourceLifecycleRegistry::default();
        let session = session("clear-shared-lifecycle");
        let graph = super::LifecycleResourcePath::Graph(
            GraphResourcePath::new("events/Clear.yssbi-event").unwrap(),
        );
        let worksheet = super::LifecycleResourcePath::Worksheet(
            WorksheetResourcePath::parse("worksheets/Clear.yssbi-worksheet").unwrap(),
        );
        let graph_guard = registry
            .register(&session, &graph, 1, super::ResourceLifecycleIntent::Load)
            .unwrap();
        let worksheet_guard = registry
            .register(
                &session,
                &worksheet,
                1,
                super::ResourceLifecycleIntent::Rename,
            )
            .unwrap();

        registry.clear_for_project(&session.instance_id);

        assert_eq!(registry.entry_count(), 0);
        assert_eq!(
            registry.validate(graph_guard.owner()).unwrap_err().code(),
            "stale_resource_lifecycle"
        );
        assert_eq!(
            registry
                .validate(worksheet_guard.owner())
                .unwrap_err()
                .code(),
            "stale_resource_lifecycle"
        );
    }

    #[test]
    fn tokens_do_not_pollute_other_paths_or_resource_kinds() {
        let registry = super::ResourceLifecycleRegistry::default();
        let session = session("resource-token-isolation");
        let graph = super::LifecycleResourcePath::Graph(
            GraphResourcePath::new("events/Report.yssbi-event").unwrap(),
        );
        let worksheet = super::LifecycleResourcePath::Worksheet(
            WorksheetResourcePath::parse("worksheets/Report.yssbi-worksheet").unwrap(),
        );
        let other_worksheet = super::LifecycleResourcePath::Worksheet(
            WorksheetResourcePath::parse("worksheets/Other.yssbi-worksheet").unwrap(),
        );

        registry
            .register(&session, &graph, 40, super::ResourceLifecycleIntent::Load)
            .unwrap();
        registry
            .register(
                &session,
                &worksheet,
                1,
                super::ResourceLifecycleIntent::Rename,
            )
            .unwrap();
        registry
            .register(
                &session,
                &other_worksheet,
                1,
                super::ResourceLifecycleIntent::Rename,
            )
            .unwrap();
    }

    #[test]
    fn duplicate_same_token_and_intent_registration_is_rejected() {
        let registry = super::ResourceLifecycleRegistry::default();
        let session = session("duplicate-registration");
        let graph_path = GraphResourcePath::new("events/Shared.yssbi-event").unwrap();
        let owner = registry
            .register(
                &session,
                &graph_path,
                7,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();

        let error = registry
            .register(
                &session,
                &graph_path,
                7,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap_err();

        assert_eq!(error.code(), "stale_resource_lifecycle");
        registry.validate(owner.owner()).unwrap();
    }

    #[test]
    fn superseded_guards_drop_without_restoring_abandoned_owner_in_both_orders() {
        let session = session("guard-drop-orders");
        let graph_path = GraphResourcePath::new("events/Shared.yssbi-event").unwrap();

        for drop_old_first in [true, false] {
            let registry = super::ResourceLifecycleRegistry::default();
            let old = registry
                .register(
                    &session,
                    &graph_path,
                    1,
                    super::ResourceLifecycleIntent::Load,
                )
                .unwrap();
            let current = registry
                .register(
                    &session,
                    &graph_path,
                    2,
                    super::ResourceLifecycleIntent::Load,
                )
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
        let registry = super::ResourceLifecycleRegistry::default();
        let session = session("three-level-live-ancestor");
        let graph_path = GraphResourcePath::new("events/Shared.yssbi-event").unwrap();
        let ancestor = registry
            .register(
                &session,
                &graph_path,
                1,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();
        let middle = registry
            .register(
                &session,
                &graph_path,
                2,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();
        let newest = registry
            .register(
                &session,
                &graph_path,
                3,
                super::ResourceLifecycleIntent::Load,
            )
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
        let registry = super::ResourceLifecycleRegistry::default();
        let session = session("three-level-committed-ancestor");
        let graph_path = GraphResourcePath::new("events/Shared.yssbi-event").unwrap();
        let mut ancestor = registry
            .register(
                &session,
                &graph_path,
                1,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();
        let ancestor_owner = ancestor.owner().clone();
        registry
            .boundary()
            .commit_guard(&mut ancestor, super::ResourceLifecycleIntent::Load)
            .unwrap();
        let middle = registry
            .register(
                &session,
                &graph_path,
                2,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();
        let newest = registry
            .register(
                &session,
                &graph_path,
                3,
                super::ResourceLifecycleIntent::Load,
            )
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
            let registry = super::ResourceLifecycleRegistry::default();
            let mut guards = [
                Some(
                    registry
                        .register(
                            &session,
                            &graph_path,
                            1,
                            super::ResourceLifecycleIntent::Load,
                        )
                        .unwrap(),
                ),
                Some(
                    registry
                        .register(
                            &session,
                            &graph_path,
                            2,
                            super::ResourceLifecycleIntent::Load,
                        )
                        .unwrap(),
                ),
                Some(
                    registry
                        .register(
                            &session,
                            &graph_path,
                            3,
                            super::ResourceLifecycleIntent::Load,
                        )
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
                                "stale_resource_lifecycle"
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

        let registry = super::ResourceLifecycleRegistry::default();
        let ancestor = registry
            .register(
                &session,
                &graph_path,
                1,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();
        let mut middle = registry
            .register(
                &session,
                &graph_path,
                2,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();
        let middle_owner = middle.owner().clone();
        registry
            .boundary()
            .commit_guard(&mut middle, super::ResourceLifecycleIntent::Load)
            .unwrap();
        let newest = registry
            .register(
                &session,
                &graph_path,
                3,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();
        drop(ancestor);
        drop(newest);
        registry.validate(&middle_owner).unwrap();
        assert_eq!(registry.entry_count(), 1);
        assert_eq!(registry.registration_count(), 1);

        let registry = super::ResourceLifecycleRegistry::default();
        let ancestor = registry
            .register(
                &session,
                &graph_path,
                1,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();
        let middle = registry
            .register(
                &session,
                &graph_path,
                2,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();
        let mut newest = registry
            .register(
                &session,
                &graph_path,
                3,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();
        let newest_owner = newest.owner().clone();
        registry
            .boundary()
            .commit_guard(&mut newest, super::ResourceLifecycleIntent::Load)
            .unwrap();
        drop(middle);
        drop(ancestor);
        registry.validate(&newest_owner).unwrap();
        assert_eq!(registry.entry_count(), 1);
        assert_eq!(registry.registration_count(), 1);
    }

    #[test]
    fn abandoned_internal_owner_does_not_reuse_its_issued_token() {
        let registry = super::ResourceLifecycleRegistry::default();
        let session = session("internal-token-abandon-reallocate");
        let graph_path = GraphResourcePath::new("functions/Shared.yssbi-function").unwrap();

        let first = registry
            .allocate_and_register(&session, &graph_path, super::ResourceLifecycleIntent::Load)
            .unwrap();
        assert_eq!(first.owner().token, 1);
        drop(first);

        let second = registry
            .allocate_and_register(&session, &graph_path, super::ResourceLifecycleIntent::Load)
            .unwrap();
        assert_eq!(second.owner().token, 2);
    }

    #[test]
    fn internal_token_watermarks_are_isolated_by_project_and_resource() {
        let registry = super::ResourceLifecycleRegistry::default();
        let first_project = session("internal-token-first-project");
        let second_project = session("internal-token-second-project");
        let shared_path = GraphResourcePath::new("functions/Shared.yssbi-function").unwrap();
        let other_path = GraphResourcePath::new("functions/Other.yssbi-function").unwrap();

        let first = registry
            .allocate_and_register(
                &first_project,
                &shared_path,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();
        assert_eq!(first.owner().token, 1);
        drop(first);
        let advanced = registry
            .allocate_and_register(
                &first_project,
                &shared_path,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();
        assert_eq!(advanced.owner().token, 2);

        let other_project = registry
            .allocate_and_register(
                &second_project,
                &shared_path,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();
        let other_resource = registry
            .allocate_and_register(
                &first_project,
                &other_path,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();
        assert_eq!(other_project.owner().token, 1);
        assert_eq!(other_resource.owner().token, 1);
    }

    #[test]
    fn clearing_project_resets_client_and_internal_token_watermarks() {
        let registry = super::ResourceLifecycleRegistry::default();
        let session = session("clear-token-watermark-domains");
        let graph_path = GraphResourcePath::new("functions/Shared.yssbi-function").unwrap();

        let client = registry
            .register(
                &session,
                &graph_path,
                5,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();
        let internal = registry
            .allocate_and_register(&session, &graph_path, super::ResourceLifecycleIntent::Load)
            .unwrap();
        assert_eq!(internal.owner().token, 6);
        drop(client);
        drop(internal);

        registry.clear_for_project(&session.instance_id);

        let reset_internal = registry
            .allocate_and_register(&session, &graph_path, super::ResourceLifecycleIntent::Load)
            .unwrap();
        assert_eq!(reset_internal.owner().token, 1);
        drop(reset_internal);
        registry
            .register(
                &session,
                &graph_path,
                1,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();
    }

    #[test]
    fn internal_function_load_does_not_advance_the_client_token_high_watermark() {
        let registry = super::ResourceLifecycleRegistry::default();
        let session = session("internal-client-token-domains");
        let graph_path = GraphResourcePath::new("functions/Shared.yssbi-function").unwrap();

        let external = registry
            .register(
                &session,
                &graph_path,
                5,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();
        let internal = registry
            .allocate_and_register(&session, &graph_path, super::ResourceLifecycleIntent::Load)
            .unwrap();
        assert_eq!(internal.owner().token, 6);

        let next_external = registry
            .register(
                &session,
                &graph_path,
                6,
                super::ResourceLifecycleIntent::Unload,
            )
            .unwrap();
        assert_eq!(
            registry
                .register(
                    &session,
                    &graph_path,
                    6,
                    super::ResourceLifecycleIntent::Load
                )
                .unwrap_err()
                .code(),
            "stale_resource_lifecycle"
        );
        assert_eq!(
            registry.validate(external.owner()).unwrap_err().code(),
            "stale_resource_lifecycle"
        );
        assert_eq!(
            registry.validate(internal.owner()).unwrap_err().code(),
            "stale_resource_lifecycle"
        );
        registry.validate(next_external.owner()).unwrap();
    }

    #[test]
    fn function_load_token_allocation_and_registration_are_atomic() {
        let registry = super::ResourceLifecycleRegistry::default();
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
                    .allocate_and_register(
                        &session,
                        &graph_path,
                        super::ResourceLifecycleIntent::Load,
                    )
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
            "stale_resource_lifecycle"
        );
        registry.validate(guards[1].owner()).unwrap();
    }

    #[test]
    fn old_project_load_unload_and_rename_tokens_never_match_replacement_project() {
        let registry = super::ResourceLifecycleRegistry::default();
        let old = session("old");
        let replacement = session("replacement");
        let paths = [
            GraphResourcePath::new("events/Load.yssbi-event").unwrap(),
            GraphResourcePath::new("events/Unload.yssbi-event").unwrap(),
            GraphResourcePath::new("events/Rename.yssbi-event").unwrap(),
        ];
        let intents = [
            super::ResourceLifecycleIntent::Load,
            super::ResourceLifecycleIntent::Unload,
            super::ResourceLifecycleIntent::Rename,
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
                "stale_resource_lifecycle"
            );
        }
        for guard in &replacement_guards {
            registry.validate(guard.owner()).unwrap();
        }
    }

    #[test]
    fn load_rejects_owned_projection_snapshot_after_project_replacement() {
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

        let error = state
            .load_graph_projection(&project_instance_id, &graph_path, 1, "en-US")
            .unwrap_err();

        assert!(error.to_string().contains("stale_project_lifecycle"));
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
        let registry = super::ResourceLifecycleRegistry::default();
        let session = session("intent-exclusion");
        let graph_path = GraphResourcePath::new("events/Shared.yssbi-event").unwrap();
        let load = registry
            .register(
                &session,
                &graph_path,
                1,
                super::ResourceLifecycleIntent::Load,
            )
            .unwrap();
        let unload = registry
            .register(
                &session,
                &graph_path,
                2,
                super::ResourceLifecycleIntent::Unload,
            )
            .unwrap();

        assert_eq!(
            registry.validate(load.owner()).unwrap_err().code(),
            "stale_resource_lifecycle"
        );
        assert_eq!(
            registry
                .register(
                    &session,
                    &graph_path,
                    2,
                    super::ResourceLifecycleIntent::Load,
                )
                .unwrap_err()
                .code(),
            "stale_resource_lifecycle"
        );
        drop(unload);
        let rename = registry
            .register(
                &session,
                &graph_path,
                3,
                super::ResourceLifecycleIntent::Rename,
            )
            .unwrap();
        assert_eq!(
            registry
                .register(
                    &session,
                    &graph_path,
                    4,
                    super::ResourceLifecycleIntent::Load,
                )
                .unwrap_err()
                .code(),
            "filesystem_transaction_busy"
        );
        registry.validate(rename.owner()).unwrap();
    }
}
