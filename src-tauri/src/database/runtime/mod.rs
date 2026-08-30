mod physical;
mod registry;

use crate::database::error::{DatabaseError, DatabaseOperation};
use crate::database::schema_snapshot::DatabaseSchemaFact;
pub(crate) use physical::PreparedDatabasePhysicalMutation;
use physical::{
    DatabaseRuntimeDataSnapshot, DatabaseRuntimeMetadata, DatabaseRuntimePageSnapshot,
    DatabaseRuntimePhysicalState,
};
use registry::DatabaseSessionRuntime;
use std::fmt;
use std::num::NonZeroU64;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use yss_database_contract::{
    DatabaseDecl, DatabaseDeclarationObservationSet, DatabaseId, DatabaseSessionIdentity,
    DatabaseSessionOpenRequest, DatabaseSessionOpenRequestParts,
};

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
    physical: Arc<DatabaseRuntimePhysicalState>,
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
        self.open_session_with_physical(request, DatabaseRuntimePhysicalState::empty())
    }

    pub(crate) fn open_session_with_instances(
        &self,
        request: DatabaseSessionOpenRequest,
        instances: impl IntoIterator<Item = crate::database::database_instance::DatabaseInstance>,
    ) -> Result<DatabaseRuntimeSession, DatabaseError> {
        let DatabaseSessionOpenRequestParts {
            identity,
            generation,
            root,
            declarations,
            observations,
            ..
        } = request
            .into_validated_parts()
            .map_err(|_| DatabaseError::invalid_request(DatabaseOperation::OpenSession, None))?;
        let physical = DatabaseRuntimePhysicalState::from_instances(&declarations, instances)?;
        Ok(DatabaseRuntimeSession {
            runtime: DatabaseSessionRuntime::new(&declarations, observations),
            physical,
            basis: DatabaseSessionBasis {
                identity,
                generation,
                _root: root,
                declarations,
            },
        })
    }

    fn open_session_with_physical(
        &self,
        request: DatabaseSessionOpenRequest,
        physical: Arc<DatabaseRuntimePhysicalState>,
    ) -> Result<DatabaseRuntimeSession, DatabaseError> {
        let DatabaseSessionOpenRequestParts {
            identity,
            generation,
            root,
            declarations,
            observations,
            ..
        } = request
            .into_validated_parts()
            .map_err(|_| DatabaseError::invalid_request(DatabaseOperation::OpenSession, None))?;
        Ok(DatabaseRuntimeSession {
            runtime: DatabaseSessionRuntime::new(&declarations, observations),
            physical,
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
        expected_observation: yss_database_contract::DatabaseDeclarationObservation,
        next_observation: yss_database_contract::DatabaseDeclarationObservation,
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

    pub(crate) fn read_physical_snapshot(
        &self,
        database: &DatabaseId,
        requested: Option<&[crate::tabular::contract::TabularColumnName]>,
        offset: usize,
        limit: usize,
    ) -> Result<DatabaseRuntimeDataSnapshot, DatabaseError> {
        self.physical
            .read_columns(database, requested, offset, limit)
    }

    pub(crate) fn read_physical_schema(
        &self,
        database: &DatabaseId,
    ) -> Result<Option<DatabaseSchemaFact>, DatabaseError> {
        self.physical.read_schema(database)
    }

    pub(crate) fn read_physical_metadata(
        &self,
        database: &DatabaseId,
    ) -> Result<DatabaseRuntimeMetadata, DatabaseError> {
        self.physical.read_metadata(database)
    }

    pub(crate) fn read_physical_page(
        &self,
        database: &DatabaseId,
        offset: usize,
        limit: usize,
    ) -> Result<DatabaseRuntimePageSnapshot, DatabaseError> {
        self.physical.read_page(database, offset, limit)
    }

    pub(crate) fn read_physical_column_stats(
        &self,
        database: &DatabaseId,
    ) -> Result<Vec<crate::database::ColumnStats>, DatabaseError> {
        self.physical.read_column_stats(database)
    }

    pub(crate) fn read_physical_column_distributions(
        &self,
        database: &DatabaseId,
    ) -> Result<Vec<crate::database::ColumnDistribution>, DatabaseError> {
        self.physical.read_column_distributions(database)
    }

    pub(crate) fn read_physical_dataset_overview(
        &self,
        database: &DatabaseId,
    ) -> Result<crate::database::DatasetOverview, DatabaseError> {
        self.physical.read_dataset_overview(database)
    }

    pub(crate) fn read_physical_edit_state(
        &self,
        database: &DatabaseId,
    ) -> Result<crate::database::EditState, DatabaseError> {
        self.physical.read_edit_state(database)
    }

    pub(crate) fn export_physical_to_path(
        &self,
        database: &DatabaseId,
        path: &std::path::Path,
        format: crate::database::DatabaseExportFormat,
    ) -> Result<(), DatabaseError> {
        self.physical.export_to_path(database, path, format)
    }

    pub(crate) fn remove_physical_database(
        &self,
        database: &DatabaseId,
        project_root: &std::path::Path,
    ) -> Result<(), DatabaseError> {
        self.physical.remove_database(database, project_root)
    }

    pub(crate) fn prepare_physical_mutation(
        &self,
        database: &DatabaseId,
        operation: &crate::database::session_api::DatabaseMutationOperation,
    ) -> Result<PreparedDatabasePhysicalMutation, DatabaseError> {
        self.physical.prepare_mutation(database, operation)
    }

    pub(crate) fn install_physical_mutation(&self, mutation: &PreparedDatabasePhysicalMutation) {
        self.physical.install_mutation(mutation);
    }

    pub(crate) fn instances_for_replacement(&self) -> Vec<crate::database::DatabaseInstance> {
        self.physical.instances_for_replacement()
    }

    pub(crate) fn restore_physical_mutation(
        &self,
        mutation: &PreparedDatabasePhysicalMutation,
    ) -> Result<(), DatabaseError> {
        mutation.rollback()
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
        current_authority: &yss_database_contract::DatabaseDeclarationObservation,
    ) -> Result<DatabaseRuntimeRecoveryClaim, DatabaseRuntimeRecoveryClaimError> {
        self.runtime.claim_recovery(recovery_id, current_authority)
    }
}
