mod registry;

use crate::database::error::{DatabaseError, DatabaseOperation};
use crate::database_contract::{
    DatabaseDecl, DatabaseDeclarationObservationSet, DatabaseId, DatabaseSessionIdentity,
    DatabaseSessionOpenRequest,
};
use registry::DatabaseSessionRuntime;
use std::fmt;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(crate) use registry::{
    DatabaseCommittedRegistration, DatabasePreparedRegistration, DatabaseRuntimeChangeRecord,
    DatabaseRuntimeCommittedChange, DatabaseRuntimeCompensationFailureCode,
    DatabaseRuntimeRecoveryClaim, DatabaseRuntimeRecoveryClaimError,
    DatabaseRuntimeRecoveryResolutionKind, DatabaseRuntimeRevisions, DatabaseRuntimeSnapshot,
};

#[derive(Clone, Default)]
pub struct DatabaseRuntimeRegistry;

pub struct DatabaseRuntimeSession {
    basis: DatabaseSessionBasis,
    runtime: Arc<DatabaseSessionRuntime>,
}

struct DatabaseSessionBasis {
    identity: DatabaseSessionIdentity,
    generation: NonZeroU64,
    _root: Option<PathBuf>,
    declarations: Arc<[DatabaseDecl]>,
}

#[derive(Clone)]
pub struct DatabaseSessionDrainControl {
    deadline: DatabaseDrainDeadline,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DatabaseDrainDeadline(Instant);

impl DatabaseDrainDeadline {
    pub fn at(deadline: Instant) -> Self {
        Self(deadline)
    }

    pub(crate) fn remaining(&self, now: Instant) -> Option<Duration> {
        self.0.checked_duration_since(now)
    }
}

impl DatabaseSessionDrainControl {
    pub fn new(deadline: DatabaseDrainDeadline) -> Self {
        Self { deadline }
    }

    pub fn deadline(&self) -> DatabaseDrainDeadline {
        self.deadline
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatabaseAdmissionCloseOutcome {
    Closed,
    AlreadyClosed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatabaseDrainOutcome {
    Drained {
        outstanding: DatabaseOutstandingWork,
    },
    TimedOut {
        outstanding: DatabaseOutstandingWork,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DatabaseOutstandingWork {
    operation_leases: usize,
    pending_prepares: usize,
    committed_changes: usize,
    recoveries: usize,
}

impl DatabaseOutstandingWork {
    #[cfg(test)]
    pub(crate) const fn operation_leases(self) -> usize {
        self.operation_leases
    }

    #[cfg(test)]
    pub(crate) const fn pending_prepares(self) -> usize {
        self.pending_prepares
    }

    #[cfg(test)]
    pub(crate) const fn committed_changes(self) -> usize {
        self.committed_changes
    }

    #[cfg(test)]
    pub(crate) const fn recoveries(self) -> usize {
        self.recoveries
    }

    fn is_empty(self) -> bool {
        self.operation_leases == 0
            && self.pending_prepares == 0
            && self.committed_changes == 0
            && self.recoveries == 0
    }

    fn increment_operation_lease(&mut self) {
        self.operation_leases = self.operation_leases.saturating_add(1);
    }

    fn release_operation_lease(&mut self) {
        debug_assert!(self.operation_leases > 0);
        self.operation_leases = self.operation_leases.saturating_sub(1);
    }

    fn increment_pending_prepare(&mut self) {
        self.pending_prepares = self.pending_prepares.saturating_add(1);
    }

    fn release_pending_prepare(&mut self) {
        debug_assert!(self.pending_prepares > 0);
        self.pending_prepares = self.pending_prepares.saturating_sub(1);
    }

    fn move_prepare_to_committed(&mut self) {
        debug_assert!(self.pending_prepares > 0);
        self.pending_prepares = self.pending_prepares.saturating_sub(1);
        self.committed_changes = self.committed_changes.saturating_add(1);
    }

    fn release_committed(&mut self) {
        debug_assert!(self.committed_changes > 0);
        self.committed_changes = self.committed_changes.saturating_sub(1);
    }

    fn add_recovery(&mut self) {
        self.recoveries = self.recoveries.saturating_add(1);
    }

    fn release_recovery(&mut self) {
        debug_assert!(self.recoveries > 0);
        self.recoveries = self.recoveries.saturating_sub(1);
    }
}

#[must_use = "database operation leases are released when this guard is dropped"]
pub(crate) struct DatabaseOperationLease {
    pub(crate) runtime: Arc<DatabaseSessionRuntime>,
    pub(crate) active: bool,
}

impl fmt::Debug for DatabaseOperationLease {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DatabaseOperationLease")
            .field("active", &self.active)
            .finish_non_exhaustive()
    }
}

impl DatabaseRuntimeRegistry {
    pub fn new() -> Self {
        Self
    }

    pub fn open_session(
        &self,
        request: DatabaseSessionOpenRequest,
    ) -> Result<DatabaseRuntimeSession, DatabaseError> {
        request
            .validate()
            .map_err(|_| DatabaseError::invalid_request(DatabaseOperation::OpenSession, None))?;
        let (identity, generation, root, declarations, observations) = request.into_parts();
        Ok(DatabaseRuntimeSession {
            runtime: DatabaseSessionRuntime::new(&declarations, observations),
            basis: DatabaseSessionBasis {
                identity,
                generation,
                _root: root,
                declarations,
            },
        })
    }
}

impl DatabaseRuntimeSession {
    pub fn identity(&self) -> &DatabaseSessionIdentity {
        &self.basis.identity
    }

    pub fn generation(&self) -> NonZeroU64 {
        self.basis.generation
    }

    pub(crate) fn declarations(&self) -> &[DatabaseDecl] {
        &self.basis.declarations
    }

    pub(crate) fn observations(&self) -> DatabaseDeclarationObservationSet {
        self.runtime.observations()
    }

    pub(crate) fn revisions(&self, database: &DatabaseId) -> Option<DatabaseRuntimeRevisions> {
        self.runtime.revisions(database)
    }

    pub(crate) fn capture_operation(
        &self,
        operation: DatabaseOperation,
    ) -> Result<(DatabaseOperationLease, DatabaseRuntimeSnapshot), DatabaseError> {
        self.runtime.capture_operation(operation)
    }

    pub(crate) fn admit_operation(
        &self,
        operation: DatabaseOperation,
    ) -> Result<DatabaseOperationLease, DatabaseError> {
        self.runtime.admit_operation(operation)
    }

    pub(crate) fn begin_prepare(
        &self,
        operation: DatabaseOperation,
    ) -> Result<DatabasePreparedRegistration, DatabaseError> {
        self.runtime.begin_prepare(operation)
    }

    pub(crate) fn commit_prepared(
        &self,
        registration: DatabasePreparedRegistration,
        database: DatabaseId,
        expected_runtime_revision: u64,
        expected_observation: crate::database_contract::DatabaseDeclarationObservation,
        next_observation: crate::database_contract::DatabaseDeclarationObservation,
        schema_changed: bool,
    ) -> Result<DatabaseRuntimeCommittedChange, DatabaseError> {
        self.runtime.commit_prepared(
            registration,
            database,
            expected_runtime_revision,
            expected_observation,
            next_observation,
            schema_changed,
        )
    }

    pub(crate) fn runtime_snapshot(&self) -> DatabaseRuntimeSnapshot {
        self.runtime.snapshot()
    }

    #[cfg(test)]
    pub(crate) fn outstanding_work(&self) -> DatabaseOutstandingWork {
        self.runtime.outstanding()
    }

    pub fn close_admission(&self) -> DatabaseAdmissionCloseOutcome {
        self.runtime.close_admission()
    }

    pub fn drain(&self, control: &DatabaseSessionDrainControl) -> DatabaseDrainOutcome {
        self.runtime.drain(control)
    }

    pub(crate) fn runtime_recovery_requirements(&self) -> Vec<DatabaseRuntimeChangeRecord> {
        self.runtime.recovery_requirements()
    }

    pub(crate) fn claim_runtime_recovery(
        &self,
        recovery_id: u64,
        current_authority: &crate::database_contract::DatabaseDeclarationObservation,
    ) -> Result<DatabaseRuntimeRecoveryClaim, DatabaseRuntimeRecoveryClaimError> {
        self.runtime.claim_recovery(recovery_id, current_authority)
    }
}
