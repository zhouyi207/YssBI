use std::collections::BTreeSet;
use std::num::NonZeroU64;

use crate::declaration_observation_for;
use crate::error::{DatabaseError, DatabaseOperation};
use crate::runtime::{
    DatabaseCommittedRegistration, DatabasePreparedRegistration, DatabaseRuntimeChangeRecord,
    DatabaseRuntimeCommittedChange, DatabaseRuntimeCompensationFailureCode,
    DatabaseRuntimeRecoveryClaim, DatabaseRuntimeRecoveryClaimError,
    DatabaseRuntimeRecoveryResolutionKind, DatabaseRuntimeSession, DatabaseRuntimeSnapshot,
};
use yss_data_contract::DataType;
use yss_database_contract::{
    DatabaseDeclarationObservation, DatabaseDeclarationObservationSet, DatabaseId,
    DatabaseSessionIdentity,
};
use yss_database_edit::EditState;
use yss_database_schema::{
    DatabaseColumnFact, DatabaseRuntimeRevision, DatabaseSchemaFact, DatabaseSchemaRevision,
};
use yss_tabular_contract::{TabularColumnName, TabularScalar, TabularSnapshot};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DatabaseColumnSelection {
    All,
    Selected(Box<[TabularColumnName]>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseDataSnapshotRequest {
    pub database: DatabaseId,
    pub columns: DatabaseColumnSelection,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DatabaseDataSnapshot {
    database: DatabaseId,
    runtime_revision: DatabaseRuntimeRevision,
    columns: Box<[DatabaseColumnFact]>,
    rows: TabularSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DatabaseMetaSnapshot {
    database: DatabaseId,
    name: Box<str>,
    schema: DatabaseSchemaFact,
    row_count: usize,
}

impl DatabaseMetaSnapshot {
    pub fn database(&self) -> &DatabaseId {
        &self.database
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn schema(&self) -> &DatabaseSchemaFact {
        &self.schema
    }

    pub const fn row_count(&self) -> usize {
        self.row_count
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DatabasePageSnapshot {
    database: DatabaseId,
    rows: TabularSnapshot,
    row_ids: Vec<i64>,
}

impl DatabasePageSnapshot {
    pub fn database(&self) -> &DatabaseId {
        &self.database
    }

    pub fn rows(&self) -> &TabularSnapshot {
        &self.rows
    }

    pub fn row_ids(&self) -> &[i64] {
        &self.row_ids
    }
}

impl DatabaseDataSnapshot {
    pub fn database(&self) -> &DatabaseId {
        &self.database
    }

    pub fn columns(&self) -> &[DatabaseColumnFact] {
        &self.columns
    }

    pub const fn runtime_revision(&self) -> DatabaseRuntimeRevision {
        self.runtime_revision
    }

    pub fn rows(&self) -> &TabularSnapshot {
        &self.rows
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DatabaseCatalogBasis {
    session: DatabaseSessionIdentity,
    generation: NonZeroU64,
    observations: DatabaseDeclarationObservationSet,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseCatalogSnapshot {
    basis: DatabaseCatalogBasis,
    schemas: Box<[DatabaseSchemaFact]>,
}

impl DatabaseCatalogSnapshot {
    pub fn schemas(&self) -> &[DatabaseSchemaFact] {
        &self.schemas
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseQueryBasis {
    session: DatabaseSessionIdentity,
    generation: NonZeroU64,
    database: DatabaseId,
    runtime_revision: DatabaseRuntimeRevision,
    schema_revision: DatabaseSchemaRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseDeclarationTransition {
    pub expected: DatabaseDeclarationObservation,
    pub next: DatabaseDeclarationObservation,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DatabaseMutationOperation {
    EditCell {
        row: usize,
        column: Box<str>,
        value: TabularScalar,
        row_id: Option<i64>,
    },
    AddRow {
        index: usize,
    },
    DeleteRows {
        indices: Box<[usize]>,
        row_ids: Option<Box<[i64]>>,
    },
    AddColumn {
        name: Box<str>,
        data_type: DataType,
    },
    DeleteColumn {
        name: Box<str>,
    },
    CastColumn {
        name: Box<str>,
        data_type: DataType,
        force: bool,
    },
    RenameDatabase {
        name: Box<str>,
    },
    RenameColumn {
        old_name: Box<str>,
        new_name: Box<str>,
    },
    Undo,
    Redo,
    Save,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DatabaseMutationRequest {
    database: DatabaseId,
    expected_runtime_revision: DatabaseRuntimeRevision,
    declaration_transition: DatabaseDeclarationTransition,
    operation: DatabaseMutationOperation,
}

impl DatabaseMutationRequest {
    pub fn new(
        database: DatabaseId,
        expected_runtime_revision: DatabaseRuntimeRevision,
        declaration_transition: DatabaseDeclarationTransition,
        operation: DatabaseMutationOperation,
    ) -> Self {
        Self {
            database,
            expected_runtime_revision,
            declaration_transition,
            operation,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseRuntimeChangeOutcome {
    database: DatabaseId,
    runtime_revision: DatabaseRuntimeRevision,
}

impl DatabaseRuntimeChangeOutcome {
    pub fn database(&self) -> &DatabaseId {
        &self.database
    }

    pub const fn runtime_revision(&self) -> DatabaseRuntimeRevision {
        self.runtime_revision
    }
}

pub struct PreparedDatabaseRuntimeChange {
    session: DatabaseSessionIdentity,
    generation: NonZeroU64,
    database: DatabaseId,
    expected_runtime_revision: DatabaseRuntimeRevision,
    expected_observation: DatabaseDeclarationObservation,
    next_observation: DatabaseDeclarationObservation,
    schema_changed: bool,
    registration: DatabasePreparedRegistration,
}

pub struct CommittedDatabaseRuntimeChange {
    outcome: DatabaseRuntimeChangeOutcome,
    expected_observation: DatabaseDeclarationObservation,
    next_observation: DatabaseDeclarationObservation,
    registration: DatabaseCommittedRegistration,
}

impl CommittedDatabaseRuntimeChange {
    pub fn outcome(&self) -> &DatabaseRuntimeChangeOutcome {
        &self.outcome
    }

    pub fn confirm(self) -> DatabaseRuntimeChangeOutcome {
        self.registration.confirm();
        self.outcome
    }

    pub fn compensate(mut self) -> DatabaseCompensationAttempt<Self> {
        match self.registration.compensate() {
            Ok(runtime_revision) => {
                DatabaseCompensationAttempt::Restored(DatabaseCompensationOutcome::Restored {
                    runtime_revision: DatabaseRuntimeRevision::from_existing(runtime_revision),
                })
            }
            Err(code) => DatabaseCompensationAttempt::Retryable {
                owner: self,
                failure: compensation_failure(code),
            },
        }
    }

    pub fn expected_observation(&self) -> &DatabaseDeclarationObservation {
        &self.expected_observation
    }

    pub fn next_observation(&self) -> &DatabaseDeclarationObservation {
        &self.next_observation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatabaseCompensationOutcome {
    Restored {
        runtime_revision: DatabaseRuntimeRevision,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatabaseCompensationFailureCode {
    StaleRuntimeRevision,
    Driver,
}

#[derive(Debug, thiserror::Error)]
#[error("database compensation failed")]
pub struct DatabaseCompensationFailure {
    code: DatabaseCompensationFailureCode,
}

impl DatabaseCompensationFailure {
    pub const fn code(&self) -> DatabaseCompensationFailureCode {
        self.code
    }
}

pub enum DatabaseCompensationAttempt<T> {
    Restored(DatabaseCompensationOutcome),
    Retryable {
        owner: T,
        failure: DatabaseCompensationFailure,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DatabaseRecoveryId(u64);

impl DatabaseRecoveryId {
    pub(crate) const fn from_existing(value: u64) -> Self {
        Self(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseRecoveryRequirement {
    recovery: DatabaseRecoveryId,
    database: DatabaseId,
    outcome: DatabaseRuntimeChangeOutcome,
    expected_observation: DatabaseDeclarationObservation,
    next_observation: DatabaseDeclarationObservation,
}

impl DatabaseRecoveryRequirement {
    pub fn recovery(&self) -> DatabaseRecoveryId {
        self.recovery
    }

    pub fn database(&self) -> &DatabaseId {
        &self.database
    }

    pub fn outcome(&self) -> &DatabaseRuntimeChangeOutcome {
        &self.outcome
    }

    pub fn expected_observation(&self) -> &DatabaseDeclarationObservation {
        &self.expected_observation
    }

    pub fn next_observation(&self) -> &DatabaseDeclarationObservation {
        &self.next_observation
    }
}

pub struct DatabaseRecoveryConfirmation {
    claim: DatabaseRuntimeRecoveryClaim,
    outcome: DatabaseRuntimeChangeOutcome,
}

impl DatabaseRecoveryConfirmation {
    pub fn confirm(self) -> DatabaseRuntimeChangeOutcome {
        self.claim.confirm();
        self.outcome
    }
}

pub struct DatabaseRecoveryCompensation {
    claim: DatabaseRuntimeRecoveryClaim,
    outcome: DatabaseRuntimeChangeOutcome,
}

impl DatabaseRecoveryCompensation {
    pub fn outcome(&self) -> &DatabaseRuntimeChangeOutcome {
        &self.outcome
    }

    pub fn compensate(mut self) -> DatabaseCompensationAttempt<Self> {
        match self.claim.compensate() {
            Ok(runtime_revision) => {
                DatabaseCompensationAttempt::Restored(DatabaseCompensationOutcome::Restored {
                    runtime_revision: DatabaseRuntimeRevision::from_existing(runtime_revision),
                })
            }
            Err(code) => DatabaseCompensationAttempt::Retryable {
                owner: self,
                failure: compensation_failure(code),
            },
        }
    }
}

pub enum DatabaseRecoveryResolution {
    Confirm(DatabaseRecoveryConfirmation),
    Compensate(DatabaseRecoveryCompensation),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DatabaseRecoveryClaimError {
    #[error("database recovery record was not found")]
    NotFound,
    #[error("database recovery record is already claimed")]
    AlreadyClaimed,
    #[error("database session admission is still open")]
    SessionStillOpen,
    #[error("database authority matches neither recovery branch")]
    AuthorityNeither,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DatabaseMutationSchemaEffect {
    DataOnly,
    Schema,
}

fn basis_for(
    session: &DatabaseRuntimeSession,
    snapshot: &DatabaseRuntimeSnapshot,
) -> DatabaseCatalogBasis {
    DatabaseCatalogBasis {
        session: session.identity().clone(),
        generation: session.generation(),
        observations: snapshot.observations.clone(),
    }
}

fn schema_effect(operation: &DatabaseMutationOperation) -> DatabaseMutationSchemaEffect {
    match operation {
        DatabaseMutationOperation::AddColumn { .. }
        | DatabaseMutationOperation::DeleteColumn { .. }
        | DatabaseMutationOperation::CastColumn { .. }
        | DatabaseMutationOperation::RenameColumn { .. } => DatabaseMutationSchemaEffect::Schema,
        DatabaseMutationOperation::EditCell { .. }
        | DatabaseMutationOperation::AddRow { .. }
        | DatabaseMutationOperation::DeleteRows { .. }
        | DatabaseMutationOperation::RenameDatabase { .. }
        | DatabaseMutationOperation::Undo
        | DatabaseMutationOperation::Redo
        | DatabaseMutationOperation::Save => DatabaseMutationSchemaEffect::DataOnly,
    }
}

pub fn catalog_snapshot(
    session: &DatabaseRuntimeSession,
) -> Result<DatabaseCatalogSnapshot, DatabaseError> {
    let (_lease, runtime_snapshot) =
        session.capture_operation(DatabaseOperation::CatalogSnapshot)?;
    let schemas = session
        .declarations()
        .iter()
        .map(|declaration| {
            let revisions = runtime_snapshot
                .revisions
                .get(&declaration.id)
                .copied()
                .ok_or_else(|| {
                    DatabaseError::not_found(
                        DatabaseOperation::CatalogSnapshot,
                        Some(declaration.id.clone()),
                    )
                })?;
            let schema = session.read_physical_schema(&declaration.id)?;
            Ok(schema
                .map(|schema| schema.with_revisions(revisions.runtime, revisions.schema))
                .unwrap_or_else(|| {
                    DatabaseSchemaFact::empty(
                        declaration.id.clone(),
                        revisions.runtime,
                        revisions.schema,
                    )
                }))
        })
        .collect::<Result<Vec<_>, DatabaseError>>()?;
    Ok(DatabaseCatalogSnapshot {
        basis: basis_for(session, &runtime_snapshot),
        schemas: schemas.into_boxed_slice(),
    })
}

pub fn revalidate_declaration_observations(
    session: &DatabaseRuntimeSession,
    expected: &DatabaseDeclarationObservationSet,
) -> Result<(), DatabaseError> {
    let runtime_snapshot = session.runtime_snapshot();
    if declaration_observations_match(&runtime_snapshot.observations, expected) {
        Ok(())
    } else {
        Err(DatabaseError::conflict(
            DatabaseOperation::CatalogSnapshot,
            None,
        ))
    }
}

pub fn revalidate_catalog_snapshot(
    session: &DatabaseRuntimeSession,
    snapshot: &DatabaseCatalogSnapshot,
) -> Result<(), DatabaseError> {
    if snapshot.basis.session != *session.identity()
        || snapshot.basis.generation != session.generation()
    {
        return Err(DatabaseError::conflict(
            DatabaseOperation::CatalogSnapshot,
            None,
        ));
    }
    let runtime_snapshot = session.runtime_snapshot();
    if !declaration_observations_match(&runtime_snapshot.observations, &snapshot.basis.observations)
    {
        return Err(DatabaseError::conflict(
            DatabaseOperation::CatalogSnapshot,
            None,
        ));
    }

    let current_ids = session
        .declarations()
        .iter()
        .map(|declaration| declaration.id.clone())
        .collect::<BTreeSet<_>>();
    let snapshot_ids = snapshot
        .schemas
        .iter()
        .map(|schema| schema.database().clone())
        .collect::<BTreeSet<_>>();
    if current_ids != snapshot_ids {
        return Err(DatabaseError::schema(
            DatabaseOperation::CatalogSnapshot,
            None,
        ));
    }
    for schema in &snapshot.schemas {
        let Some(current) = runtime_snapshot.revisions.get(schema.database()).copied() else {
            return Err(DatabaseError::schema(
                DatabaseOperation::CatalogSnapshot,
                Some(schema.database().clone()),
            ));
        };
        if schema.schema_revision().get() != current.schema {
            return Err(DatabaseError::schema(
                DatabaseOperation::CatalogSnapshot,
                Some(schema.database().clone()),
            ));
        }
        if schema.runtime_revision().get() != current.runtime {
            return Err(DatabaseError::conflict(
                DatabaseOperation::CatalogSnapshot,
                Some(schema.database().clone()),
            ));
        }
    }
    Ok(())
}

pub fn data_snapshot(
    session: &DatabaseRuntimeSession,
    request: DatabaseDataSnapshotRequest,
) -> Result<DatabaseDataSnapshot, DatabaseError> {
    let (_lease, runtime_snapshot) = session.capture_operation(DatabaseOperation::DataSnapshot)?;
    let runtime_revision = runtime_snapshot
        .revisions
        .get(&request.database)
        .copied()
        .ok_or_else(|| {
            DatabaseError::not_found(
                DatabaseOperation::DataSnapshot,
                Some(request.database.clone()),
            )
        })?;
    let requested = match &request.columns {
        DatabaseColumnSelection::All => None,
        DatabaseColumnSelection::Selected(columns) => Some(columns.as_ref()),
    };
    let physical = session.read_physical_snapshot(
        &request.database,
        requested,
        request.offset,
        request.limit,
    )?;
    Ok(DatabaseDataSnapshot {
        database: request.database,
        runtime_revision: DatabaseRuntimeRevision::from_existing(runtime_revision.runtime),
        columns: physical.columns,
        rows: physical.rows,
    })
}

pub fn metadata_snapshot(
    session: &DatabaseRuntimeSession,
    database: DatabaseId,
) -> Result<DatabaseMetaSnapshot, DatabaseError> {
    let (_lease, runtime_snapshot) = session.capture_operation(DatabaseOperation::Query)?;
    if !runtime_snapshot.revisions.contains_key(&database) {
        return Err(DatabaseError::not_found(
            DatabaseOperation::Query,
            Some(database),
        ));
    }
    let _declaration = session
        .declarations()
        .iter()
        .find(|declaration| declaration.id == database)
        .ok_or_else(|| {
            DatabaseError::not_found(DatabaseOperation::Query, Some(database.clone()))
        })?;
    let physical = session.read_physical_metadata(&database)?;
    Ok(DatabaseMetaSnapshot {
        database,
        name: physical.name,
        schema: physical.schema,
        row_count: physical.row_count,
    })
}

pub fn page_snapshot(
    session: &DatabaseRuntimeSession,
    database: DatabaseId,
    offset: usize,
    limit: usize,
) -> Result<DatabasePageSnapshot, DatabaseError> {
    let (_lease, runtime_snapshot) = session.capture_operation(DatabaseOperation::Query)?;
    if !runtime_snapshot.revisions.contains_key(&database) {
        return Err(DatabaseError::not_found(
            DatabaseOperation::Query,
            Some(database),
        ));
    }
    let page = session.read_physical_page(&database, offset, limit)?;
    Ok(DatabasePageSnapshot {
        database,
        rows: page.rows,
        row_ids: page.row_ids,
    })
}

pub fn column_statistics(
    session: &DatabaseRuntimeSession,
    database: DatabaseId,
) -> Result<Vec<yss_dataset_profile::ColumnStats>, DatabaseError> {
    let (_lease, runtime_snapshot) = session.capture_operation(DatabaseOperation::Query)?;
    if !runtime_snapshot.revisions.contains_key(&database) {
        return Err(DatabaseError::not_found(
            DatabaseOperation::Query,
            Some(database),
        ));
    }
    session.read_physical_column_stats(&database)
}

pub fn column_distributions(
    session: &DatabaseRuntimeSession,
    database: DatabaseId,
) -> Result<Vec<yss_dataset_profile::ColumnDistribution>, DatabaseError> {
    let (_lease, runtime_snapshot) = session.capture_operation(DatabaseOperation::Query)?;
    if !runtime_snapshot.revisions.contains_key(&database) {
        return Err(DatabaseError::not_found(
            DatabaseOperation::Query,
            Some(database),
        ));
    }
    session.read_physical_column_distributions(&database)
}

pub fn dataset_overview(
    session: &DatabaseRuntimeSession,
    database: DatabaseId,
) -> Result<yss_dataset_profile::DatasetOverview, DatabaseError> {
    let (_lease, runtime_snapshot) = session.capture_operation(DatabaseOperation::Query)?;
    if !runtime_snapshot.revisions.contains_key(&database) {
        return Err(DatabaseError::not_found(
            DatabaseOperation::Query,
            Some(database),
        ));
    }
    session.read_physical_dataset_overview(&database)
}

pub fn edit_state(
    session: &DatabaseRuntimeSession,
    database: DatabaseId,
) -> Result<EditState, DatabaseError> {
    let (_lease, runtime_snapshot) = session.capture_operation(DatabaseOperation::Query)?;
    if !runtime_snapshot.revisions.contains_key(&database) {
        return Err(DatabaseError::not_found(
            DatabaseOperation::Query,
            Some(database),
        ));
    }
    session.read_physical_edit_state(&database)
}

pub fn prepare_database_runtime_change(
    session: &DatabaseRuntimeSession,
    request: DatabaseMutationRequest,
) -> Result<PreparedDatabaseRuntimeChange, DatabaseError> {
    let current_observations = session.observations();
    let current = declaration_observation_for(&current_observations, &request.database)
        .ok_or_else(|| {
            DatabaseError::not_found(
                DatabaseOperation::PrepareMutation,
                Some(request.database.clone()),
            )
        })?;
    let revisions = session.revisions(&request.database).ok_or_else(|| {
        DatabaseError::not_found(
            DatabaseOperation::PrepareMutation,
            Some(request.database.clone()),
        )
    })?;
    if revisions.runtime != request.expected_runtime_revision.get()
        || current != &request.declaration_transition.expected
    {
        return Err(DatabaseError::conflict(
            DatabaseOperation::PrepareMutation,
            Some(request.database),
        ));
    }
    let registration = session.begin_prepare(DatabaseOperation::PrepareMutation)?;
    Ok(PreparedDatabaseRuntimeChange {
        session: session.identity().clone(),
        generation: session.generation(),
        database: request.database,
        expected_runtime_revision: request.expected_runtime_revision,
        expected_observation: request.declaration_transition.expected,
        next_observation: request.declaration_transition.next,
        schema_changed: schema_effect(&request.operation) == DatabaseMutationSchemaEffect::Schema,
        registration,
    })
}

pub fn commit_database_runtime_change(
    session: &DatabaseRuntimeSession,
    prepared: PreparedDatabaseRuntimeChange,
) -> Result<CommittedDatabaseRuntimeChange, DatabaseError> {
    if prepared.session != *session.identity() || prepared.generation != session.generation() {
        return Err(DatabaseError::conflict(
            DatabaseOperation::CommitMutation,
            Some(prepared.database),
        ));
    }
    let DatabaseRuntimeCommittedChange {
        registration,
        record,
    } = session.commit_prepared(
        prepared.registration,
        prepared.database,
        prepared.expected_runtime_revision.get(),
        prepared.expected_observation.clone(),
        prepared.next_observation.clone(),
        prepared.schema_changed,
    )?;
    Ok(CommittedDatabaseRuntimeChange {
        outcome: DatabaseRuntimeChangeOutcome {
            database: record.database().clone(),
            runtime_revision: DatabaseRuntimeRevision::from_existing(
                record.after_runtime_revision(),
            ),
        },
        expected_observation: prepared.expected_observation,
        next_observation: prepared.next_observation,
        registration,
    })
}

pub fn revalidate_query_basis(
    session: &DatabaseRuntimeSession,
    basis: &DatabaseQueryBasis,
) -> Result<(), DatabaseError> {
    if basis.session != *session.identity() || basis.generation != session.generation() {
        return Err(DatabaseError::conflict(
            DatabaseOperation::Query,
            Some(basis.database.clone()),
        ));
    }
    let runtime_snapshot = session.runtime_snapshot();
    let Some(current) = runtime_snapshot.revisions.get(&basis.database).copied() else {
        return Err(DatabaseError::not_found(
            DatabaseOperation::Query,
            Some(basis.database.clone()),
        ));
    };
    if current.runtime != basis.runtime_revision.get() {
        return Err(DatabaseError::conflict(
            DatabaseOperation::Query,
            Some(basis.database.clone()),
        ));
    }
    if current.schema != basis.schema_revision.get() {
        return Err(DatabaseError::schema(
            DatabaseOperation::Query,
            Some(basis.database.clone()),
        ));
    }
    Ok(())
}

impl DatabaseRuntimeSession {
    pub fn capture_query_basis(
        &self,
        database: &DatabaseId,
    ) -> Result<DatabaseQueryBasis, DatabaseError> {
        let (_lease, runtime_snapshot) = self.capture_operation(DatabaseOperation::Query)?;
        let revisions = runtime_snapshot
            .revisions
            .get(database)
            .copied()
            .ok_or_else(|| {
                DatabaseError::not_found(DatabaseOperation::Query, Some(database.clone()))
            })?;
        Ok(DatabaseQueryBasis {
            session: self.identity().clone(),
            generation: self.generation(),
            database: database.clone(),
            runtime_revision: DatabaseRuntimeRevision::from_existing(revisions.runtime),
            schema_revision: DatabaseSchemaRevision::from_existing(revisions.schema),
        })
    }

    pub fn claim_recovery(
        &self,
        recovery: DatabaseRecoveryId,
        current_authority: &DatabaseDeclarationObservation,
    ) -> Result<DatabaseRecoveryResolution, DatabaseRecoveryClaimError> {
        let claim = self
            .claim_runtime_recovery(recovery.0, current_authority)
            .map_err(map_recovery_claim_error)?;
        let record = claim.record().clone();
        let outcome = DatabaseRuntimeChangeOutcome {
            database: record.database().clone(),
            runtime_revision: DatabaseRuntimeRevision::from_existing(
                record.after_runtime_revision(),
            ),
        };
        match claim.kind() {
            DatabaseRuntimeRecoveryResolutionKind::Confirm => {
                Ok(DatabaseRecoveryResolution::Confirm(
                    DatabaseRecoveryConfirmation { claim, outcome },
                ))
            }
            DatabaseRuntimeRecoveryResolutionKind::Compensate => {
                Ok(DatabaseRecoveryResolution::Compensate(
                    DatabaseRecoveryCompensation { claim, outcome },
                ))
            }
        }
    }

    pub fn recovery_requirements(&self) -> Vec<DatabaseRecoveryRequirement> {
        self.runtime_recovery_requirements()
            .iter()
            .map(recovery_requirement)
            .collect()
    }
}

fn declaration_observations_match(
    current: &DatabaseDeclarationObservationSet,
    expected: &DatabaseDeclarationObservationSet,
) -> bool {
    current == expected
}

fn compensation_failure(
    code: DatabaseRuntimeCompensationFailureCode,
) -> DatabaseCompensationFailure {
    DatabaseCompensationFailure {
        code: match code {
            DatabaseRuntimeCompensationFailureCode::StaleRuntimeRevision => {
                DatabaseCompensationFailureCode::StaleRuntimeRevision
            }
        },
    }
}

fn recovery_requirement(record: &DatabaseRuntimeChangeRecord) -> DatabaseRecoveryRequirement {
    DatabaseRecoveryRequirement {
        recovery: DatabaseRecoveryId::from_existing(record.recovery_id()),
        database: record.database().clone(),
        outcome: DatabaseRuntimeChangeOutcome {
            database: record.database().clone(),
            runtime_revision: DatabaseRuntimeRevision::from_existing(
                record.after_runtime_revision(),
            ),
        },
        expected_observation: record.expected_observation().clone(),
        next_observation: record.next_observation().clone(),
    }
}

fn map_recovery_claim_error(
    error: DatabaseRuntimeRecoveryClaimError,
) -> DatabaseRecoveryClaimError {
    match error {
        DatabaseRuntimeRecoveryClaimError::NotFound => DatabaseRecoveryClaimError::NotFound,
        DatabaseRuntimeRecoveryClaimError::AlreadyClaimed => {
            DatabaseRecoveryClaimError::AlreadyClaimed
        }
        DatabaseRuntimeRecoveryClaimError::SessionStillOpen => {
            DatabaseRecoveryClaimError::SessionStillOpen
        }
        DatabaseRuntimeRecoveryClaimError::AuthorityNeither => {
            DatabaseRecoveryClaimError::AuthorityNeither
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::DatabaseRuntimeRegistry;
    use crate::{DatabaseInstance, DatabaseState};
    use std::time::Instant;
    use yss_database_contract::{
        DatabaseDecl, DatabaseDeclarationFingerprint, DatabaseDeclarationRevision, DatabaseEngine,
        DatabaseSessionOpenRequest,
    };
    use yss_database_edit::EditHistory;

    fn declaration(id: &str) -> DatabaseDecl {
        DatabaseDecl {
            id: DatabaseId::from_existing(id.into()),
            engine: DatabaseEngine::InMemory { name: id.into() },
            schema_version: 1,
            required: false,
            name: id.into(),
        }
    }

    fn session() -> DatabaseRuntimeSession {
        session_with("session")
    }

    fn session_with(identity: &str) -> DatabaseRuntimeSession {
        let declaration = declaration("sales");
        let observations = DatabaseDeclarationObservationSet::try_from_iter([(
            declaration.id.clone(),
            DatabaseDeclarationObservation::new(
                DatabaseDeclarationRevision::from_existing(1),
                DatabaseDeclarationFingerprint::from_decl(&declaration),
            ),
        )])
        .unwrap();
        DatabaseRuntimeRegistry::new()
            .open_session(DatabaseSessionOpenRequest::new(
                DatabaseSessionIdentity::from_existing(identity.into()),
                NonZeroU64::new(1).unwrap(),
                None,
                vec![declaration].into(),
                observations,
            ))
            .unwrap()
    }

    fn session_with_loaded_instance(identity: &str) -> DatabaseRuntimeSession {
        let declaration = declaration("sales");
        let observations = DatabaseDeclarationObservationSet::try_from_iter([(
            declaration.id.clone(),
            DatabaseDeclarationObservation::new(
                DatabaseDeclarationRevision::from_existing(1),
                DatabaseDeclarationFingerprint::from_decl(&declaration),
            ),
        )])
        .unwrap();
        let dataframe = polars::df!("value" => &[1_i64]).expect("test dataframe is valid");
        let instance = DatabaseInstance {
            decl: declaration.clone(),
            state: DatabaseState::Loaded {
                dataframe: std::sync::Arc::new(dataframe.clone()),
                original: std::sync::Arc::new(dataframe),
                history: EditHistory::new(),
            },
        };
        DatabaseRuntimeRegistry::new()
            .open_session_with_instances(
                DatabaseSessionOpenRequest::new(
                    DatabaseSessionIdentity::from_existing(identity.into()),
                    NonZeroU64::new(1).unwrap(),
                    None,
                    vec![declaration].into(),
                    observations,
                ),
                [instance],
            )
            .unwrap()
    }

    #[test]
    fn catalog_snapshot_revalidation_is_session_and_revision_exact() {
        let first_session = session();
        let snapshot = catalog_snapshot(&first_session).unwrap();
        revalidate_catalog_snapshot(&first_session, &snapshot).unwrap();
        let other = session_with("other-session");
        assert_eq!(
            revalidate_catalog_snapshot(&other, &snapshot)
                .unwrap_err()
                .code(),
            crate::error::DatabaseErrorCode::Conflict
        );

        let request = DatabaseMutationRequest {
            database: DatabaseId::from_existing("sales".into()),
            expected_runtime_revision: DatabaseRuntimeRevision::INITIAL,
            declaration_transition: DatabaseDeclarationTransition {
                expected: first_session
                    .observations()
                    .iter()
                    .next()
                    .unwrap()
                    .1
                    .clone(),
                next: first_session
                    .observations()
                    .iter()
                    .next()
                    .unwrap()
                    .1
                    .clone(),
            },
            operation: DatabaseMutationOperation::EditCell {
                row: 0,
                column: "value".into(),
                value: TabularScalar::Null,
                row_id: None,
            },
        };
        let prepared = prepare_database_runtime_change(&first_session, request).unwrap();
        let _ = commit_database_runtime_change(&first_session, prepared).unwrap();
        assert_eq!(
            revalidate_catalog_snapshot(&first_session, &snapshot)
                .unwrap_err()
                .code(),
            crate::error::DatabaseErrorCode::Conflict
        );

        let fresh = catalog_snapshot(&first_session).unwrap();
        let schema_request = DatabaseMutationRequest {
            database: DatabaseId::from_existing("sales".into()),
            expected_runtime_revision: DatabaseRuntimeRevision::from_existing(1),
            declaration_transition: DatabaseDeclarationTransition {
                expected: first_session
                    .observations()
                    .iter()
                    .next()
                    .unwrap()
                    .1
                    .clone(),
                next: first_session
                    .observations()
                    .iter()
                    .next()
                    .unwrap()
                    .1
                    .clone(),
            },
            operation: DatabaseMutationOperation::AddColumn {
                name: "new_column".into(),
                data_type: DataType::String,
            },
        };
        let prepared = prepare_database_runtime_change(&first_session, schema_request).unwrap();
        let _ = commit_database_runtime_change(&first_session, prepared).unwrap();
        assert_eq!(
            revalidate_catalog_snapshot(&first_session, &fresh)
                .unwrap_err()
                .code(),
            crate::error::DatabaseErrorCode::Schema
        );
    }

    #[test]
    fn selected_columns_reject_empty_and_duplicate_requests_before_access() {
        let session = session_with_loaded_instance("session");
        let empty = data_snapshot(
            &session,
            DatabaseDataSnapshotRequest {
                database: DatabaseId::from_existing("sales".into()),
                columns: DatabaseColumnSelection::Selected(Box::new([])),
                offset: 0,
                limit: 1,
            },
        )
        .unwrap_err();
        assert_eq!(
            empty.code(),
            crate::error::DatabaseErrorCode::InvalidRequest
        );
    }

    #[test]
    fn mutation_registration_and_abandoned_recovery_are_explicitly_drained() {
        let session = session();
        let observation = session.observations().iter().next().unwrap().1.clone();
        let prepared = prepare_database_runtime_change(
            &session,
            DatabaseMutationRequest {
                database: DatabaseId::from_existing("sales".into()),
                expected_runtime_revision: DatabaseRuntimeRevision::INITIAL,
                declaration_transition: DatabaseDeclarationTransition {
                    expected: observation.clone(),
                    next: observation,
                },
                operation: DatabaseMutationOperation::EditCell {
                    row: 0,
                    column: "value".into(),
                    value: TabularScalar::Null,
                    row_id: None,
                },
            },
        )
        .unwrap();
        assert_eq!(session.outstanding_work().pending_prepares(), 1);

        let committed = commit_database_runtime_change(&session, prepared).unwrap();
        assert_eq!(session.outstanding_work().pending_prepares(), 0);
        assert_eq!(session.outstanding_work().committed_changes(), 1);

        assert_eq!(
            session.close_admission(),
            crate::runtime::DatabaseAdmissionCloseOutcome::Closed
        );
        assert_eq!(
            session
                .admit_operation(DatabaseOperation::Query)
                .unwrap_err()
                .code(),
            crate::error::DatabaseErrorCode::AdmissionClosed
        );

        drop(committed);
        assert_eq!(session.outstanding_work().committed_changes(), 0);
        assert_eq!(session.outstanding_work().recoveries(), 1);
        assert!(matches!(
            session.drain(&crate::runtime::DatabaseSessionDrainControl::new(
                crate::runtime::DatabaseDrainDeadline::at(Instant::now()),
            )),
            crate::runtime::DatabaseDrainOutcome::TimedOut { outstanding }
                if outstanding.recoveries() == 1
        ));

        let requirement = session.recovery_requirements().into_iter().next().unwrap();
        let current = session.observations().iter().next().unwrap().1.clone();
        let resolution = session
            .claim_recovery(requirement.recovery(), &current)
            .unwrap();
        match resolution {
            DatabaseRecoveryResolution::Confirm(confirmation) => {
                confirmation.confirm();
            }
            DatabaseRecoveryResolution::Compensate(_) => {
                panic!("same next observation must choose confirmation")
            }
        }
        assert_eq!(session.outstanding_work(), Default::default());
        assert_eq!(
            session.drain(&crate::runtime::DatabaseSessionDrainControl::new(
                crate::runtime::DatabaseDrainDeadline::at(
                    Instant::now() + std::time::Duration::from_secs(1),
                ),
            )),
            crate::runtime::DatabaseDrainOutcome::Drained {
                outstanding: Default::default(),
            }
        );
    }

    #[test]
    fn compensation_keeps_a_stale_owner_retryable_until_the_newer_change_is_restored() {
        let session = session();
        let expected = session.observations().iter().next().unwrap().1.clone();
        let next = DatabaseDeclarationObservation::new(
            yss_database_contract::DatabaseDeclarationRevision::from_existing(2),
            expected.fingerprint().clone(),
        );
        let first = commit_database_runtime_change(
            &session,
            prepare_database_runtime_change(
                &session,
                DatabaseMutationRequest {
                    database: DatabaseId::from_existing("sales".into()),
                    expected_runtime_revision: DatabaseRuntimeRevision::INITIAL,
                    declaration_transition: DatabaseDeclarationTransition {
                        expected: expected.clone(),
                        next: next.clone(),
                    },
                    operation: DatabaseMutationOperation::EditCell {
                        row: 0,
                        column: "value".into(),
                        value: TabularScalar::Null,
                        row_id: None,
                    },
                },
            )
            .unwrap(),
        )
        .unwrap();

        let newest_expected = session.observations().iter().next().unwrap().1.clone();
        let newest = DatabaseDeclarationObservation::new(
            yss_database_contract::DatabaseDeclarationRevision::from_existing(3),
            newest_expected.fingerprint().clone(),
        );
        let second = commit_database_runtime_change(
            &session,
            prepare_database_runtime_change(
                &session,
                DatabaseMutationRequest {
                    database: DatabaseId::from_existing("sales".into()),
                    expected_runtime_revision: DatabaseRuntimeRevision::from_existing(1),
                    declaration_transition: DatabaseDeclarationTransition {
                        expected: newest_expected,
                        next: newest,
                    },
                    operation: DatabaseMutationOperation::EditCell {
                        row: 1,
                        column: "value".into(),
                        value: TabularScalar::Null,
                        row_id: None,
                    },
                },
            )
            .unwrap(),
        )
        .unwrap();

        let first = match first.compensate() {
            DatabaseCompensationAttempt::Retryable { owner, failure } => {
                assert_eq!(
                    failure.code(),
                    DatabaseCompensationFailureCode::StaleRuntimeRevision
                );
                owner
            }
            DatabaseCompensationAttempt::Restored(_) => {
                panic!("an older committed owner must observe the newer runtime revision")
            }
        };
        assert_eq!(session.outstanding_work().committed_changes(), 2);

        assert!(matches!(
            second.compensate(),
            DatabaseCompensationAttempt::Restored(_)
        ));
        assert!(matches!(
            first.compensate(),
            DatabaseCompensationAttempt::Restored(_)
        ));
        assert_eq!(session.outstanding_work(), Default::default());
    }
}
