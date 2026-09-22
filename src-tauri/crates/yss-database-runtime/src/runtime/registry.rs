use super::{DatabaseDrainOutcome, DatabaseOperationLease, DatabaseOutstandingWork};
use crate::declaration_observation_for;
use crate::error::{DatabaseError, DatabaseOperation};
use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex, PoisonError};
use yss_database_contract::{
    DatabaseDecl, DatabaseDeclarationObservation, DatabaseDeclarationObservationSet, DatabaseId,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DatabaseRuntimeRevisions {
    pub(crate) runtime: u64,
    pub(crate) schema: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct DatabaseRuntimeSnapshot {
    pub(crate) observations: DatabaseDeclarationObservationSet,
    pub(crate) revisions: BTreeMap<DatabaseId, DatabaseRuntimeRevisions>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DatabaseRuntimeLifecycle {
    Open,
    AdmissionClosed,
    Draining,
    Drained,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DatabaseCommittedRecordState {
    Committed,
    Available,
    Claimed,
}

#[derive(Clone)]
pub(crate) struct DatasetStorageRecovery {
    pub store: Arc<yss_database_store::DatasetStore>,
    pub publication: yss_database_store::DatasetPublication,
}
impl std::fmt::Debug for DatasetStorageRecovery {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DatasetStorageRecovery")
            .field("publication", &self.publication)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DatabaseRuntimeChangeRecord {
    recovery_id: u64,
    database: DatabaseId,
    before: DatabaseRuntimeRevisions,
    after: DatabaseRuntimeRevisions,
    expected_observation: DatabaseDeclarationObservation,
    next_observation: DatabaseDeclarationObservation,
    state: DatabaseCommittedRecordState,
    pub(crate) storage: DatasetStorageRecovery,
}

impl DatabaseRuntimeChangeRecord {
    pub(crate) fn recovery_id(&self) -> u64 {
        self.recovery_id
    }

    pub(crate) fn database(&self) -> &DatabaseId {
        &self.database
    }
}

pub(crate) struct DatabaseRuntimeCommittedChange {
    pub(crate) registration: DatabaseCommittedRegistration,
    pub(crate) database: DatabaseId,
    pub(crate) runtime_revision: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DatabaseRuntimeCompensationFailureCode {
    StaleRuntimeRevision,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum DatabaseRuntimeRecoveryClaimError {
    #[error("database recovery record was not found")]
    NotFound,
    #[error("database recovery record is already claimed")]
    AlreadyClaimed,
    #[error("database session admission is still open")]
    SessionStillOpen,
}

pub(crate) struct DatabaseRuntimeRecoveryClaim {
    runtime: Arc<DatabaseSessionRuntime>,
    recovery_id: u64,
    active: bool,
}

impl DatabaseRuntimeRecoveryClaim {
    pub(crate) fn confirm(mut self) {
        self.active = false;
        self.runtime.resolve_recovery(self.recovery_id);
    }

    pub(crate) fn compensate(&mut self) -> Result<(), DatabaseRuntimeCompensationFailureCode> {
        self.runtime
            .restore_change(self.recovery_id, DatabaseCommittedRecordState::Claimed)?;
        self.active = false;
        Ok(())
    }
}

impl Drop for DatabaseRuntimeRecoveryClaim {
    fn drop(&mut self) {
        if self.active {
            self.runtime.release_recovery_claim(self.recovery_id);
        }
    }
}

pub(crate) struct DatabaseCommittedRegistration {
    runtime: Arc<DatabaseSessionRuntime>,
    recovery_id: u64,
    active: bool,
}

impl DatabaseCommittedRegistration {
    pub(crate) fn confirm(mut self) {
        self.active = false;
        self.runtime.resolve_committed(self.recovery_id);
    }

    pub(crate) fn compensate(&mut self) -> Result<(), DatabaseRuntimeCompensationFailureCode> {
        self.runtime
            .restore_change(self.recovery_id, DatabaseCommittedRecordState::Committed)?;
        self.active = false;
        Ok(())
    }
}

impl Drop for DatabaseCommittedRegistration {
    fn drop(&mut self) {
        if self.active {
            self.runtime.abandon_committed(self.recovery_id);
        }
    }
}

pub(crate) struct DatabasePreparedRegistration {
    runtime: Arc<DatabaseSessionRuntime>,
    active: bool,
    storage: DatasetStorageRecovery,
}

impl std::fmt::Debug for DatabasePreparedRegistration {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DatabasePreparedRegistration")
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

impl DatabasePreparedRegistration {
    fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for DatabasePreparedRegistration {
    fn drop(&mut self) {
        if self.active {
            self.runtime.release_prepare();
        }
    }
}

pub(crate) struct DatabaseSessionRuntime {
    state: Mutex<DatabaseRuntimeState>,
    changed: Condvar,
}

struct DatabaseRuntimeState {
    lifecycle: DatabaseRuntimeLifecycle,
    observations: DatabaseDeclarationObservationSet,
    revisions: BTreeMap<DatabaseId, DatabaseRuntimeRevisions>,
    next_recovery_id: u64,
    changes: BTreeMap<u64, DatabaseRuntimeChangeRecord>,
    outstanding: DatabaseOutstandingWork,
}

impl DatabaseSessionRuntime {
    pub(crate) fn new(
        declarations: &[DatabaseDecl],
        observations: DatabaseDeclarationObservationSet,
    ) -> Arc<Self> {
        let revisions = declarations
            .iter()
            .map(|declaration| (declaration.id.clone(), DatabaseRuntimeRevisions::default()))
            .collect();
        Arc::new(Self {
            state: Mutex::new(DatabaseRuntimeState {
                lifecycle: DatabaseRuntimeLifecycle::Open,
                observations,
                revisions,
                next_recovery_id: 0,
                changes: BTreeMap::new(),
                outstanding: DatabaseOutstandingWork::default(),
            }),
            changed: Condvar::new(),
        })
    }

    pub(crate) fn observations(&self) -> DatabaseDeclarationObservationSet {
        lock_or_recover(&self.state).observations.clone()
    }

    pub(crate) fn revisions(&self, database: &DatabaseId) -> Option<DatabaseRuntimeRevisions> {
        lock_or_recover(&self.state)
            .revisions
            .get(database)
            .copied()
    }

    pub(crate) fn snapshot(&self) -> DatabaseRuntimeSnapshot {
        let state = lock_or_recover(&self.state);
        DatabaseRuntimeSnapshot {
            observations: state.observations.clone(),
            revisions: state.revisions.clone(),
        }
    }

    pub(crate) fn capture_operation(
        self: &Arc<Self>,
        operation: DatabaseOperation,
    ) -> Result<(DatabaseOperationLease, DatabaseRuntimeSnapshot), DatabaseError> {
        let mut state = lock_or_recover(&self.state);
        admit(&mut state, operation)?;
        if !state.changes.is_empty() {
            return Err(DatabaseError::conflict(operation, None));
        }
        state.outstanding.increment_operation_lease();
        let snapshot = DatabaseRuntimeSnapshot {
            observations: state.observations.clone(),
            revisions: state.revisions.clone(),
        };
        drop(state);
        Ok((
            DatabaseOperationLease {
                runtime: Arc::clone(self),
                active: true,
            },
            snapshot,
        ))
    }

    pub(crate) fn admit_operation(
        self: &Arc<Self>,
        operation: DatabaseOperation,
    ) -> Result<DatabaseOperationLease, DatabaseError> {
        let mut state = lock_or_recover(&self.state);
        admit(&mut state, operation)?;
        state.outstanding.increment_operation_lease();
        drop(state);
        Ok(DatabaseOperationLease {
            runtime: Arc::clone(self),
            active: true,
        })
    }

    pub(crate) fn begin_prepare(
        self: &Arc<Self>,
        operation: DatabaseOperation,
        storage: DatasetStorageRecovery,
    ) -> Result<DatabasePreparedRegistration, DatabaseError> {
        let mut state = lock_or_recover(&self.state);
        admit(&mut state, operation)?;
        if !state.changes.is_empty() {
            return Err(DatabaseError::conflict(operation, None));
        }
        state.outstanding.increment_pending_prepare();
        drop(state);
        Ok(DatabasePreparedRegistration {
            runtime: Arc::clone(self),
            active: true,
            storage,
        })
    }

    pub(crate) fn commit_prepared(
        self: &Arc<Self>,
        mut registration: DatabasePreparedRegistration,
        database: DatabaseId,
        expected_runtime_revision: u64,
        expected_observation: DatabaseDeclarationObservation,
        next_observation: DatabaseDeclarationObservation,
        schema_changed: bool,
    ) -> Result<DatabaseRuntimeCommittedChange, DatabaseError> {
        if !Arc::ptr_eq(self, &registration.runtime) {
            return Err(DatabaseError::conflict(
                DatabaseOperation::CommitMutation,
                Some(database),
            ));
        }

        let mut state = lock_or_recover(&self.state);
        let Some(current_revisions) = state.revisions.get(&database).copied() else {
            return Err(DatabaseError::not_found(
                DatabaseOperation::CommitMutation,
                Some(database),
            ));
        };
        let Some(current_observation) = declaration_observation_for(&state.observations, &database)
        else {
            return Err(DatabaseError::not_found(
                DatabaseOperation::CommitMutation,
                Some(database),
            ));
        };
        if current_revisions.runtime != expected_runtime_revision
            || current_observation != &expected_observation
        {
            return Err(DatabaseError::conflict(
                DatabaseOperation::CommitMutation,
                Some(database),
            ));
        }
        if next_observation.revision().get() < current_observation.revision().get() {
            return Err(DatabaseError::conflict(
                DatabaseOperation::CommitMutation,
                Some(database),
            ));
        }

        let mut after = current_revisions;
        after.runtime = after.runtime.checked_add(1).ok_or_else(|| {
            DatabaseError::conflict(DatabaseOperation::CommitMutation, Some(database.clone()))
        })?;
        if schema_changed {
            after.schema = after.schema.checked_add(1).ok_or_else(|| {
                DatabaseError::conflict(DatabaseOperation::CommitMutation, Some(database.clone()))
            })?;
        }
        let recovery_id = state.next_recovery_id.checked_add(1).ok_or_else(|| {
            DatabaseError::conflict(DatabaseOperation::CommitMutation, Some(database.clone()))
        })?;
        let next_observations = replace_observation(
            &state.observations,
            &database,
            next_observation.clone(),
        )
        .map_err(|_| {
            DatabaseError::conflict(DatabaseOperation::CommitMutation, Some(database.clone()))
        })?;

        registration.disarm();
        state.outstanding.move_prepare_to_committed();
        state.next_recovery_id = recovery_id;
        state.revisions.insert(database.clone(), after);
        state.observations = next_observations;
        let record = DatabaseRuntimeChangeRecord {
            recovery_id,
            database: database.clone(),
            before: current_revisions,
            after,
            expected_observation,
            next_observation,
            state: DatabaseCommittedRecordState::Committed,
            storage: registration.storage.clone(),
        };
        state.changes.insert(recovery_id, record);
        drop(state);
        Ok(DatabaseRuntimeCommittedChange {
            registration: DatabaseCommittedRegistration {
                runtime: Arc::clone(self),
                recovery_id,
                active: true,
            },
            database,
            runtime_revision: after.runtime,
        })
    }

    #[cfg(test)]
    pub(crate) fn outstanding(&self) -> DatabaseOutstandingWork {
        lock_or_recover(&self.state).outstanding
    }

    pub(crate) fn close_admission(&self) -> super::DatabaseAdmissionCloseOutcome {
        let mut state = lock_or_recover(&self.state);
        match state.lifecycle {
            DatabaseRuntimeLifecycle::Open => {
                state.lifecycle = DatabaseRuntimeLifecycle::AdmissionClosed;
                super::DatabaseAdmissionCloseOutcome::Closed
            }
            DatabaseRuntimeLifecycle::AdmissionClosed
            | DatabaseRuntimeLifecycle::Draining
            | DatabaseRuntimeLifecycle::Drained => {
                super::DatabaseAdmissionCloseOutcome::AlreadyClosed
            }
        }
    }

    pub(crate) fn drain(
        &self,
        control: &super::DatabaseSessionDrainControl,
    ) -> DatabaseDrainOutcome {
        let mut state = lock_or_recover(&self.state);
        match state.lifecycle {
            DatabaseRuntimeLifecycle::Open | DatabaseRuntimeLifecycle::AdmissionClosed => {
                state.lifecycle = DatabaseRuntimeLifecycle::Draining;
            }
            DatabaseRuntimeLifecycle::Draining | DatabaseRuntimeLifecycle::Drained => {}
        }

        loop {
            if state.outstanding.is_empty() {
                state.lifecycle = DatabaseRuntimeLifecycle::Drained;
                return DatabaseDrainOutcome::Drained {
                    outstanding: state.outstanding,
                };
            }

            let Some(remaining) = control.deadline().remaining(std::time::Instant::now()) else {
                return DatabaseDrainOutcome::TimedOut {
                    outstanding: state.outstanding,
                };
            };
            let (next_state, wait_result) = self
                .changed
                .wait_timeout(state, remaining)
                .unwrap_or_else(PoisonError::into_inner);
            state = next_state;
            if wait_result.timed_out() && !state.outstanding.is_empty() {
                return DatabaseDrainOutcome::TimedOut {
                    outstanding: state.outstanding,
                };
            }
        }
    }

    pub(crate) fn recovery_requirements(&self) -> Vec<DatabaseRuntimeChangeRecord> {
        lock_or_recover(&self.state)
            .changes
            .values()
            .filter(|change| {
                matches!(
                    change.state,
                    DatabaseCommittedRecordState::Available | DatabaseCommittedRecordState::Claimed
                )
            })
            .cloned()
            .collect()
    }

    pub(crate) fn claim_recovery(
        self: &Arc<Self>,
        recovery_id: u64,
    ) -> Result<DatabaseRuntimeRecoveryClaim, DatabaseRuntimeRecoveryClaimError> {
        let mut state = lock_or_recover(&self.state);
        if state.lifecycle == DatabaseRuntimeLifecycle::Open {
            return Err(DatabaseRuntimeRecoveryClaimError::SessionStillOpen);
        }
        let Some(record) = state.changes.get_mut(&recovery_id) else {
            return Err(DatabaseRuntimeRecoveryClaimError::NotFound);
        };
        if record.state != DatabaseCommittedRecordState::Available {
            return Err(DatabaseRuntimeRecoveryClaimError::AlreadyClaimed);
        }
        record.state = DatabaseCommittedRecordState::Claimed;
        Ok(DatabaseRuntimeRecoveryClaim {
            runtime: Arc::clone(self),
            recovery_id,
            active: true,
        })
    }

    fn release_operation_lease(&self) {
        let mut state = lock_or_recover(&self.state);
        state.outstanding.release_operation_lease();
        drop(state);
        self.changed.notify_all();
    }

    fn release_prepare(&self) {
        let mut state = lock_or_recover(&self.state);
        state.outstanding.release_pending_prepare();
        drop(state);
        self.changed.notify_all();
    }

    fn resolve_committed(&self, recovery_id: u64) {
        let mut state = lock_or_recover(&self.state);
        if let Some(record) = state.changes.remove(&recovery_id) {
            debug_assert_eq!(
                record.state,
                DatabaseCommittedRecordState::Committed,
                "committed registration must resolve exactly once"
            );
            state.outstanding.release_committed();
        }
        drop(state);
        self.changed.notify_all();
    }

    fn abandon_committed(&self, recovery_id: u64) {
        let mut state = lock_or_recover(&self.state);
        if let Some(record) = state.changes.get_mut(&recovery_id)
            && record.state == DatabaseCommittedRecordState::Committed
        {
            record.state = DatabaseCommittedRecordState::Available;
            state.outstanding.release_committed();
            state.outstanding.add_recovery();
        }
        drop(state);
        self.changed.notify_all();
    }

    fn release_recovery_claim(&self, recovery_id: u64) {
        let mut state = lock_or_recover(&self.state);
        if let Some(record) = state.changes.get_mut(&recovery_id)
            && record.state == DatabaseCommittedRecordState::Claimed
        {
            record.state = DatabaseCommittedRecordState::Available;
        }
        drop(state);
        self.changed.notify_all();
    }

    fn resolve_recovery(&self, recovery_id: u64) {
        let mut state = lock_or_recover(&self.state);
        if let Some(record) = state.changes.remove(&recovery_id) {
            debug_assert_eq!(
                record.state,
                DatabaseCommittedRecordState::Claimed,
                "recovery claim must resolve exactly once"
            );
            state.outstanding.release_recovery();
        }
        drop(state);
        self.changed.notify_all();
    }

    fn restore_change(
        &self,
        recovery_id: u64,
        expected_state: DatabaseCommittedRecordState,
    ) -> Result<(), DatabaseRuntimeCompensationFailureCode> {
        let mut state = lock_or_recover(&self.state);
        let Some(record) = state.changes.get(&recovery_id).cloned() else {
            return Err(DatabaseRuntimeCompensationFailureCode::StaleRuntimeRevision);
        };
        if record.state != expected_state {
            return Err(DatabaseRuntimeCompensationFailureCode::StaleRuntimeRevision);
        }
        let Some(current_revisions) = state.revisions.get(&record.database).copied() else {
            return Err(DatabaseRuntimeCompensationFailureCode::StaleRuntimeRevision);
        };
        let Some(current_observation) =
            declaration_observation_for(&state.observations, &record.database)
        else {
            return Err(DatabaseRuntimeCompensationFailureCode::StaleRuntimeRevision);
        };
        if current_revisions != record.after || current_observation != &record.next_observation {
            return Err(DatabaseRuntimeCompensationFailureCode::StaleRuntimeRevision);
        }
        let restored_observations = replace_observation(
            &state.observations,
            &record.database,
            record.expected_observation.clone(),
        )
        .map_err(|_| DatabaseRuntimeCompensationFailureCode::StaleRuntimeRevision)?;
        state
            .revisions
            .insert(record.database.clone(), record.before);
        state.observations = restored_observations;
        state.changes.remove(&recovery_id);
        match expected_state {
            DatabaseCommittedRecordState::Committed => state.outstanding.release_committed(),
            DatabaseCommittedRecordState::Claimed => state.outstanding.release_recovery(),
            DatabaseCommittedRecordState::Available => {
                return Err(DatabaseRuntimeCompensationFailureCode::StaleRuntimeRevision);
            }
        }
        drop(state);
        self.changed.notify_all();
        Ok(())
    }
}

impl Drop for DatabaseOperationLease {
    fn drop(&mut self) {
        if self.active {
            self.active = false;
            self.runtime.release_operation_lease();
        }
    }
}

fn admit(
    state: &mut DatabaseRuntimeState,
    operation: DatabaseOperation,
) -> Result<(), DatabaseError> {
    if state.lifecycle != DatabaseRuntimeLifecycle::Open {
        return Err(DatabaseError::admission_closed(operation, None));
    }
    Ok(())
}

fn replace_observation(
    observations: &DatabaseDeclarationObservationSet,
    database: &DatabaseId,
    replacement: DatabaseDeclarationObservation,
) -> Result<DatabaseDeclarationObservationSet, ()> {
    let mut found = false;
    let entries = observations
        .iter()
        .map(|(id, observation)| {
            if id == database {
                found = true;
                (id.clone(), replacement.clone())
            } else {
                (id.clone(), observation.clone())
            }
        })
        .collect::<Vec<_>>();
    if !found {
        return Err(());
    }
    DatabaseDeclarationObservationSet::try_from_iter(entries).map_err(|_| ())
}

fn lock_or_recover<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
