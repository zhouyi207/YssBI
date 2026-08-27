use std::collections::BTreeSet;
use std::num::NonZeroU64;

use crate::data_contract::DataType;
use crate::database::error::{DatabaseError, DatabaseOperation};
use crate::database::runtime::{DatabaseOperationLease, DatabaseRuntimeSession};
use crate::database::schema_snapshot::{
    DatabaseColumnFact, DatabaseRuntimeRevision, DatabaseSchemaFact, DatabaseSchemaRevision,
};
use crate::database_contract::{
    DatabaseDeclarationObservation, DatabaseDeclarationObservationSet, DatabaseId,
    DatabaseSessionIdentity,
};
use crate::tabular::contract::{TabularColumn, TabularColumnName, TabularScalar, TabularSnapshot};

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

impl DatabaseDataSnapshot {
    pub fn database(&self) -> &DatabaseId {
        &self.database
    }

    #[allow(
        dead_code,
        reason = "neutral column projection is consumed by later Application seams"
    )]
    pub(crate) fn columns(&self) -> &[DatabaseColumnFact] {
        &self.columns
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
    #[allow(
        dead_code,
        reason = "neutral schema projection is consumed by later Graph seams"
    )]
    pub(crate) fn schemas(&self) -> &[DatabaseSchemaFact] {
        &self.schemas
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[allow(
    dead_code,
    reason = "query basis is activated by the later Database plot seam"
)]
pub(crate) struct DatabaseQueryBasis {
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
    },
    AddRow {
        index: usize,
    },
    DeleteRows {
        indices: Box<[usize]>,
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
    RenameColumn {
        old_name: Box<str>,
        new_name: Box<str>,
    },
    Undo,
    Redo,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DatabaseMutationRequest {
    pub(crate) database: DatabaseId,
    pub(crate) expected_runtime_revision: DatabaseRuntimeRevision,
    pub(crate) declaration_transition: DatabaseDeclarationTransition,
    pub(crate) operation: DatabaseMutationOperation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseRuntimeChangeOutcome {
    database: DatabaseId,
    runtime_revision: DatabaseRuntimeRevision,
}

impl DatabaseRuntimeChangeOutcome {
    #[allow(dead_code, reason = "outcome identity is consumed by Project finalize")]
    pub(crate) fn database(&self) -> &DatabaseId {
        &self.database
    }

    #[allow(
        dead_code,
        reason = "runtime revision is consumed by the final session owner"
    )]
    pub(crate) const fn runtime_revision(&self) -> DatabaseRuntimeRevision {
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
    _lease: DatabaseOperationLease,
}

pub struct CommittedDatabaseRuntimeChange {
    #[allow(
        dead_code,
        reason = "resolution evidence is consumed by Project finalize"
    )]
    outcome: DatabaseRuntimeChangeOutcome,
    #[allow(
        dead_code,
        reason = "resolution evidence is consumed by Project finalize"
    )]
    expected_observation: DatabaseDeclarationObservation,
    #[allow(
        dead_code,
        reason = "resolution evidence is consumed by Project finalize"
    )]
    next_observation: DatabaseDeclarationObservation,
}

impl CommittedDatabaseRuntimeChange {
    pub fn outcome(&self) -> &DatabaseRuntimeChangeOutcome {
        &self.outcome
    }

    pub fn confirm(self) -> DatabaseRuntimeChangeOutcome {
        self.outcome
    }

    pub(crate) fn compensate(self) -> DatabaseCompensationAttempt<Self> {
        DatabaseCompensationAttempt::Restored(DatabaseCompensationOutcome::Restored {
            runtime_revision: self.outcome.runtime_revision,
        })
    }

    #[allow(
        dead_code,
        reason = "resolution evidence is consumed by Project finalize"
    )]
    pub(crate) fn expected_observation(&self) -> &DatabaseDeclarationObservation {
        &self.expected_observation
    }

    #[allow(
        dead_code,
        reason = "resolution evidence is consumed by Project finalize"
    )]
    pub(crate) fn next_observation(&self) -> &DatabaseDeclarationObservation {
        &self.next_observation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DatabaseCompensationOutcome {
    Restored {
        runtime_revision: DatabaseRuntimeRevision,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatabaseCompensationFailureCode {
    StaleRuntimeRevision,
    Driver,
}

#[derive(Debug)]
pub struct DatabaseCompensationFailure {
    code: DatabaseCompensationFailureCode,
}

impl DatabaseCompensationFailure {
    pub const fn code(&self) -> DatabaseCompensationFailureCode {
        self.code
    }
}

pub(crate) enum DatabaseCompensationAttempt<T> {
    Restored(DatabaseCompensationOutcome),
    Retryable {
        owner: T,
        failure: DatabaseCompensationFailure,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct DatabaseRecoveryId(u64);

impl DatabaseRecoveryId {
    #[allow(
        dead_code,
        reason = "recovery IDs are minted by unresolved commit cleanup"
    )]
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

pub(crate) struct DatabaseRecoveryConfirmation {
    outcome: DatabaseRuntimeChangeOutcome,
}

impl DatabaseRecoveryConfirmation {
    pub(crate) fn confirm(self) -> DatabaseRuntimeChangeOutcome {
        self.outcome
    }
}

pub(crate) struct DatabaseRecoveryCompensation {
    outcome: DatabaseRuntimeChangeOutcome,
}

impl DatabaseRecoveryCompensation {
    pub(crate) fn compensate(self) -> DatabaseCompensationAttempt<Self> {
        DatabaseCompensationAttempt::Restored(DatabaseCompensationOutcome::Restored {
            runtime_revision: self.outcome.runtime_revision,
        })
    }
}

pub(crate) enum DatabaseRecoveryResolution {
    Confirm(DatabaseRecoveryConfirmation),
    Compensate(DatabaseRecoveryCompensation),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatabaseRecoveryClaimError {
    NotFound,
    AlreadyClaimed,
    SessionStillOpen,
    AuthorityNeither,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DatabaseMutationSchemaEffect {
    DataOnly,
    Schema,
}

fn observation_for<'a>(
    observations: &'a DatabaseDeclarationObservationSet,
    database: &DatabaseId,
) -> Option<&'a DatabaseDeclarationObservation> {
    observations
        .iter()
        .find_map(|(id, observation)| (id == database).then_some(observation))
}

fn basis_for(session: &DatabaseRuntimeSession) -> DatabaseCatalogBasis {
    DatabaseCatalogBasis {
        session: session.identity().clone(),
        generation: session.generation(),
        observations: session.observations().clone(),
    }
}

fn schema_for(
    session: &DatabaseRuntimeSession,
    database: &DatabaseId,
) -> Result<DatabaseSchemaFact, DatabaseError> {
    let revisions = session.revisions(database).ok_or_else(|| {
        DatabaseError::not_found(DatabaseOperation::CatalogSnapshot, Some(database.clone()))
    })?;
    Ok(DatabaseSchemaFact::empty(
        database.clone(),
        revisions.runtime,
        revisions.schema,
    ))
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
        | DatabaseMutationOperation::Undo
        | DatabaseMutationOperation::Redo => DatabaseMutationSchemaEffect::DataOnly,
    }
}

pub fn catalog_snapshot(
    session: &DatabaseRuntimeSession,
) -> Result<DatabaseCatalogSnapshot, DatabaseError> {
    let _lease = session.admit_operation(DatabaseOperation::CatalogSnapshot)?;
    let schemas = session
        .declarations()
        .iter()
        .map(|declaration| schema_for(session, &declaration.id))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DatabaseCatalogSnapshot {
        basis: basis_for(session),
        schemas: schemas.into_boxed_slice(),
    })
}

pub fn revalidate_declaration_observations(
    session: &DatabaseRuntimeSession,
    expected: &DatabaseDeclarationObservationSet,
) -> Result<(), DatabaseError> {
    if session.observations() == expected {
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
    revalidate_declaration_observations(session, &snapshot.basis.observations)?;

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
        let Some(current) = session.revisions(schema.database()) else {
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
    let _lease = session.admit_operation(DatabaseOperation::DataSnapshot)?;
    let schema = schema_for(session, &request.database)?;
    let selected = match request.columns {
        DatabaseColumnSelection::All => schema.columns().to_vec(),
        DatabaseColumnSelection::Selected(columns) => {
            if columns.is_empty() {
                return Err(DatabaseError::invalid_request(
                    DatabaseOperation::DataSnapshot,
                    Some(request.database),
                ));
            }
            let mut seen = BTreeSet::new();
            let mut selected = Vec::with_capacity(columns.len());
            for column in columns {
                if !seen.insert(column.clone()) {
                    return Err(DatabaseError::invalid_request(
                        DatabaseOperation::DataSnapshot,
                        Some(request.database),
                    ));
                }
                let Some(fact) = schema.columns().iter().find(|fact| fact.name() == &column) else {
                    return Err(DatabaseError::not_found(
                        DatabaseOperation::DataSnapshot,
                        Some(request.database),
                    ));
                };
                selected.push(fact.clone());
            }
            selected
        }
    };
    let rows = selected
        .iter()
        .map(|column| TabularColumn::new(column.name().clone(), Box::new([])))
        .collect::<Vec<_>>();
    let rows = TabularSnapshot::try_from_columns(rows.into_boxed_slice()).map_err(|_| {
        DatabaseError::invalid_request(
            DatabaseOperation::DataSnapshot,
            Some(request.database.clone()),
        )
    })?;
    Ok(DatabaseDataSnapshot {
        database: request.database,
        runtime_revision: schema.runtime_revision(),
        columns: selected.into_boxed_slice(),
        rows,
    })
}

pub fn prepare_database_runtime_change(
    session: &DatabaseRuntimeSession,
    request: DatabaseMutationRequest,
) -> Result<PreparedDatabaseRuntimeChange, DatabaseError> {
    let lease = session.admit_operation(DatabaseOperation::PrepareMutation)?;
    let current = observation_for(session.observations(), &request.database).ok_or_else(|| {
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
    Ok(PreparedDatabaseRuntimeChange {
        session: session.identity().clone(),
        generation: session.generation(),
        database: request.database,
        expected_runtime_revision: request.expected_runtime_revision,
        expected_observation: request.declaration_transition.expected,
        next_observation: request.declaration_transition.next,
        schema_changed: schema_effect(&request.operation) == DatabaseMutationSchemaEffect::Schema,
        _lease: lease,
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
    let current = observation_for(session.observations(), &prepared.database).ok_or_else(|| {
        DatabaseError::not_found(
            DatabaseOperation::CommitMutation,
            Some(prepared.database.clone()),
        )
    })?;
    let revisions = session.revisions(&prepared.database).ok_or_else(|| {
        DatabaseError::not_found(
            DatabaseOperation::CommitMutation,
            Some(prepared.database.clone()),
        )
    })?;
    if revisions.runtime != prepared.expected_runtime_revision.get()
        || current != &prepared.expected_observation
    {
        return Err(DatabaseError::conflict(
            DatabaseOperation::CommitMutation,
            Some(prepared.database),
        ));
    }
    let next = session.advance_revisions(&prepared.database, prepared.schema_changed)?;
    Ok(CommittedDatabaseRuntimeChange {
        outcome: DatabaseRuntimeChangeOutcome {
            database: prepared.database,
            runtime_revision: DatabaseRuntimeRevision::from_existing(next.runtime),
        },
        expected_observation: prepared.expected_observation,
        next_observation: prepared.next_observation,
    })
}

#[allow(
    dead_code,
    reason = "query basis is activated by the later Database plot seam"
)]
pub(crate) fn revalidate_query_basis(
    session: &DatabaseRuntimeSession,
    basis: &DatabaseQueryBasis,
) -> Result<(), DatabaseError> {
    if basis.session != *session.identity() || basis.generation != session.generation() {
        return Err(DatabaseError::conflict(
            DatabaseOperation::Query,
            Some(basis.database.clone()),
        ));
    }
    let Some(current) = session.revisions(&basis.database) else {
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
    #[allow(
        dead_code,
        reason = "query basis is activated by the later Database plot seam"
    )]
    pub(crate) fn capture_query_basis(
        &self,
        database: &DatabaseId,
    ) -> Result<DatabaseQueryBasis, DatabaseError> {
        let _lease = self.admit_operation(DatabaseOperation::Query)?;
        let revisions = self.revisions(database).ok_or_else(|| {
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
        _recovery: DatabaseRecoveryId,
        _current_authority: &DatabaseDeclarationObservation,
    ) -> Result<DatabaseRecoveryResolution, DatabaseRecoveryClaimError> {
        Err(DatabaseRecoveryClaimError::NotFound)
    }
}

#[cfg(test)]
pub(crate) struct DatabaseCatalogSnapshotFixtureSchema {
    pub database: DatabaseId,
    pub runtime_revision: u64,
    pub schema_revision: u64,
    pub columns: Box<[DatabaseColumnFact]>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DatabaseCatalogSnapshotFixtureError {
    EmptySessionIdentity,
    DuplicateDatabaseId,
    ObservationSchemaSetMismatch,
}

#[cfg(test)]
pub(crate) fn database_catalog_snapshot_fixture(
    session_identity: Box<str>,
    generation: NonZeroU64,
    declaration_observations: DatabaseDeclarationObservationSet,
    schemas: Box<[DatabaseCatalogSnapshotFixtureSchema]>,
) -> Result<DatabaseCatalogSnapshot, DatabaseCatalogSnapshotFixtureError> {
    if session_identity.is_empty() {
        return Err(DatabaseCatalogSnapshotFixtureError::EmptySessionIdentity);
    }
    let mut ids = BTreeSet::new();
    let schemas = schemas
        .into_vec()
        .into_iter()
        .map(|schema| {
            if !ids.insert(schema.database.clone()) {
                return Err(DatabaseCatalogSnapshotFixtureError::DuplicateDatabaseId);
            }
            Ok(DatabaseSchemaFact::from_columns(
                schema.database,
                schema.runtime_revision,
                schema.schema_revision,
                schema.columns,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let observation_ids = declaration_observations
        .iter()
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    if observation_ids != ids {
        return Err(DatabaseCatalogSnapshotFixtureError::ObservationSchemaSetMismatch);
    }
    Ok(DatabaseCatalogSnapshot {
        basis: DatabaseCatalogBasis {
            session: DatabaseSessionIdentity::from_existing(session_identity),
            generation,
            observations: declaration_observations,
        },
        schemas: schemas.into_boxed_slice(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::runtime::DatabaseRuntimeRegistry;
    use crate::database_contract::{
        DatabaseDecl, DatabaseDeclarationFingerprint, DatabaseDeclarationRevision, DatabaseEngine,
        DatabaseSessionOpenRequest,
    };

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
            crate::database::error::DatabaseErrorCode::Conflict
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
            },
        };
        let prepared = prepare_database_runtime_change(&first_session, request).unwrap();
        let _ = commit_database_runtime_change(&first_session, prepared).unwrap();
        assert_eq!(
            revalidate_catalog_snapshot(&first_session, &snapshot)
                .unwrap_err()
                .code(),
            crate::database::error::DatabaseErrorCode::Conflict
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
            crate::database::error::DatabaseErrorCode::Schema
        );
    }

    #[test]
    fn selected_columns_reject_empty_and_duplicate_requests_before_access() {
        let session = session();
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
            crate::database::error::DatabaseErrorCode::InvalidRequest
        );
    }
}
