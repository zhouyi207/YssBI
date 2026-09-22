use std::collections::BTreeSet;
use std::num::NonZeroU64;

use crate::declaration_observation_for;
use crate::error::{DatabaseError, DatabaseOperation};
use crate::runtime::{
    DatabaseCommittedRegistration, DatabasePreparedRegistration, DatabaseRuntimeCommittedChange,
    DatabaseRuntimeCompensationFailureCode, DatabaseRuntimeSession, DatabaseRuntimeSnapshot,
};
use arrow::datatypes::DataType;
use yss_data_contract::{TabularColumnName, TabularScalar, TabularSnapshot};
use yss_database_contract::EditState;
use yss_database_contract::{
    DatabaseDeclarationObservation, DatabaseDeclarationObservationSet, DatabaseId,
    DatabaseSessionIdentity,
};
use yss_database_schema::{DatabaseRuntimeRevision, DatabaseSchemaFact, DatabaseSchemaRevision};

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

pub struct DatabaseArrowSnapshot {
    pub schema: arrow::datatypes::SchemaRef,
    pub batches: Vec<arrow::record_batch::RecordBatch>,
    pub row_count: usize,
    runtime_revision: DatabaseRuntimeRevision,
}
impl DatabaseArrowSnapshot {
    pub fn runtime_revision(&self) -> DatabaseRuntimeRevision {
        self.runtime_revision
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DatabaseMetaSnapshot {
    name: Box<str>,
    schema: DatabaseSchemaFact,
    row_count: usize,
}

impl DatabaseMetaSnapshot {
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
    rows: TabularSnapshot,
    row_ids: Vec<i64>,
}

impl DatabasePageSnapshot {
    pub fn rows(&self) -> &TabularSnapshot {
        &self.rows
    }

    pub fn row_ids(&self) -> &[i64] {
        &self.row_ids
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
    DeleteDatabase,
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
    SetColumnSemantic {
        name: Box<str>,
        semantic: yss_data_contract::ColumnSemantic,
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
}

impl DatabaseMutationRequest {
    pub fn new(
        database: DatabaseId,
        expected_runtime_revision: DatabaseRuntimeRevision,
        declaration_transition: DatabaseDeclarationTransition,
    ) -> Self {
        Self {
            database,
            expected_runtime_revision,
            declaration_transition,
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
    registration: DatabaseCommittedRegistration,
}

impl CommittedDatabaseRuntimeChange {
    pub fn outcome(&self) -> &DatabaseRuntimeChangeOutcome {
        &self.outcome
    }

    pub fn confirm(self) {
        self.registration.confirm();
    }

    pub fn compensate(mut self) -> DatabaseCompensationAttempt {
        match self.registration.compensate() {
            Ok(()) => DatabaseCompensationAttempt::Restored,
            Err(code) => DatabaseCompensationAttempt::Retryable {
                owner: self,
                failure: compensation_failure(code),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatabaseCompensationFailureCode {
    StaleRuntimeRevision,
    Driver,
}

pub enum DatabaseCompensationAttempt {
    Restored,
    Retryable {
        owner: CommittedDatabaseRuntimeChange,
        failure: DatabaseCompensationFailureCode,
    },
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

pub fn arrow_snapshot(
    session: &DatabaseRuntimeSession,
    request: DatabaseDataSnapshotRequest,
) -> Result<DatabaseArrowSnapshot, DatabaseError> {
    let (_lease, _) = session.capture_operation(DatabaseOperation::DataSnapshot)?;
    let basis = session.capture_query_basis(&request.database)?;
    let instance = session.physical_instance(&request.database)?;
    let relation = instance
        .query()
        .and_then(|query| query.relation().map_err(Into::into))
        .map_err(|error| {
            DatabaseError::dataset(
                DatabaseOperation::DataSnapshot,
                Some(request.database.clone()),
                error,
            )
        })?;
    let selected = match request.columns {
        DatabaseColumnSelection::All => relation
            .schema()
            .fields()
            .iter()
            .map(|field| field.name().as_str().into())
            .collect::<Vec<Box<str>>>(),
        DatabaseColumnSelection::Selected(columns) => columns
            .iter()
            .map(|column| column.as_str().into())
            .collect(),
    };
    if selected.is_empty() || selected.iter().collect::<BTreeSet<_>>().len() != selected.len() {
        return Err(DatabaseError::invalid_request(
            DatabaseOperation::DataSnapshot,
            Some(request.database),
        ));
    }
    let schema = relation
        .project(&selected)
        .map_err(|error| {
            DatabaseError::dataset(
                DatabaseOperation::DataSnapshot,
                Some(request.database.clone()),
                error.into(),
            )
        })?
        .schema();
    let names = selected.iter().map(AsRef::as_ref).collect::<Vec<_>>();
    let batches = instance
        .read_arrow_columns(
            &names,
            request.offset,
            request.limit,
            &crate::database_instance::query_control(128 * 1024 * 1024),
        )
        .map_err(|error| {
            DatabaseError::dataset(
                DatabaseOperation::DataSnapshot,
                Some(request.database.clone()),
                error,
            )
        })?;
    revalidate_query_basis(session, &basis)?;
    let row_count = batches
        .iter()
        .try_fold(0usize, |count, batch| count.checked_add(batch.num_rows()))
        .ok_or_else(|| {
            DatabaseError::invalid_request(DatabaseOperation::DataSnapshot, Some(request.database))
        })?;
    Ok(DatabaseArrowSnapshot {
        schema,
        batches,
        row_count,
        runtime_revision: basis.runtime_revision,
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
    dataset_overview_with_control(
        session,
        database,
        &crate::database_instance::query_control(16 * 1024 * 1024),
    )
}

pub fn dataset_overview_with_control(
    session: &DatabaseRuntimeSession,
    database: DatabaseId,
    control: &yss_relational_contract::RelationControl,
) -> Result<yss_dataset_profile::DatasetOverview, DatabaseError> {
    let (_lease, runtime_snapshot) = session.capture_operation(DatabaseOperation::Query)?;
    if !runtime_snapshot.revisions.contains_key(&database) {
        return Err(DatabaseError::not_found(
            DatabaseOperation::Query,
            Some(database),
        ));
    }
    session.read_physical_dataset_overview(&database, control)
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
    physical: &crate::runtime::PreparedDatabasePhysicalMutation,
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
    let storage = physical.storage_recovery()?;
    if storage.publication.dataset != request.database {
        return Err(DatabaseError::invalid_request(
            DatabaseOperation::PrepareMutation,
            Some(request.database),
        ));
    }
    let schema_changed = physical.schema_changed()?;
    let registration = session.begin_prepare(DatabaseOperation::PrepareMutation, storage)?;
    Ok(PreparedDatabaseRuntimeChange {
        session: session.identity().clone(),
        generation: session.generation(),
        database: request.database,
        expected_runtime_revision: request.expected_runtime_revision,
        expected_observation: request.declaration_transition.expected,
        next_observation: request.declaration_transition.next,
        schema_changed,
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
        database,
        runtime_revision,
    } = session.commit_prepared(
        prepared.registration,
        prepared.database,
        prepared.expected_runtime_revision.get(),
        prepared.expected_observation,
        prepared.next_observation,
        prepared.schema_changed,
    )?;
    Ok(CommittedDatabaseRuntimeChange {
        outcome: DatabaseRuntimeChangeOutcome {
            database,
            runtime_revision: DatabaseRuntimeRevision::from_existing(runtime_revision),
        },
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
    /// Resolve abandoned storage handoffs only after admission closes. SQLite operation
    /// records prove whether the prepared data committed, including an uncertain I/O outcome.
    pub fn resolve_storage_recoveries(&self) -> Result<usize, DatabaseError> {
        let mut resolved = 0;
        for record in self.runtime_recovery_requirements().into_iter().rev() {
            let storage = &record.storage;
            let committed = storage
                .store
                .publication_committed(&storage.publication)
                .map_err(|error| {
                    DatabaseError::dataset(
                        DatabaseOperation::Recovery,
                        Some(record.database().clone()),
                        error,
                    )
                })?;
            let mut claim = self
                .claim_runtime_recovery(record.recovery_id())
                .map_err(|_| {
                    DatabaseError::conflict(
                        DatabaseOperation::Recovery,
                        Some(record.database().clone()),
                    )
                })?;
            if committed {
                claim.confirm();
            } else {
                claim.compensate().map_err(|_| {
                    DatabaseError::conflict(
                        DatabaseOperation::Recovery,
                        Some(record.database().clone()),
                    )
                })?;
            }
            resolved += 1;
        }
        Ok(resolved)
    }
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
}

fn declaration_observations_match(
    current: &DatabaseDeclarationObservationSet,
    expected: &DatabaseDeclarationObservationSet,
) -> bool {
    current == expected
}

fn compensation_failure(
    code: DatabaseRuntimeCompensationFailureCode,
) -> DatabaseCompensationFailureCode {
    match code {
        DatabaseRuntimeCompensationFailureCode::StaleRuntimeRevision => {
            DatabaseCompensationFailureCode::StaleRuntimeRevision
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::DatabaseRuntimeRegistry;
    use crate::test_support::{DatasetFixture, SALES_ID};
    use std::time::Instant;
    use yss_database_contract::{
        DatabaseDecl, DatabaseDeclarationFingerprint, DatabaseDeclarationRevision, DatabaseEngine,
        DatabaseSessionOpenRequest,
    };

    fn declaration(id: &str) -> DatabaseDecl {
        DatabaseDecl {
            id: DatabaseId::from_existing(id.into()),
            engine: DatabaseEngine::Dataset {},
            schema_version: 1,
            required: false,
            name: id.into(),
        }
    }

    fn session_with(identity: &str) -> DatabaseRuntimeSession {
        let declaration = declaration(SALES_ID);
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
                vec![declaration].into(),
                observations,
            ))
            .unwrap()
    }

    fn session_with_table(identity: &str) -> (DatasetFixture, DatabaseRuntimeSession) {
        let fixture = DatasetFixture::single_i64(SALES_ID, "value", vec![1]);
        let declaration = fixture.instance.decl.clone();
        let observations = DatabaseDeclarationObservationSet::try_from_iter([(
            declaration.id.clone(),
            DatabaseDeclarationObservation::new(
                DatabaseDeclarationRevision::from_existing(1),
                DatabaseDeclarationFingerprint::from_decl(&declaration),
            ),
        )])
        .unwrap();
        let session = DatabaseRuntimeRegistry::new()
            .open_session_with_instances(
                DatabaseSessionOpenRequest::new(
                    DatabaseSessionIdentity::from_existing(identity.into()),
                    NonZeroU64::new(1).unwrap(),
                    vec![declaration].into(),
                    observations,
                ),
                [fixture.instance.clone()],
            )
            .unwrap();
        (fixture, session)
    }

    fn prepare_change(
        session: &DatabaseRuntimeSession,
        operation: DatabaseMutationOperation,
        operation_id: &str,
    ) -> (
        PreparedDatabaseRuntimeChange,
        crate::runtime::PreparedDatabasePhysicalMutation,
    ) {
        let database = DatabaseId::from_existing(SALES_ID.into());
        let observation = declaration_observation_for(&session.observations(), &database)
            .unwrap()
            .clone();
        let physical = session
            .prepare_physical_mutation(&database, &operation, operation_id)
            .unwrap();
        let request = DatabaseMutationRequest::new(
            database.clone(),
            session.runtime_revision(&database).unwrap(),
            DatabaseDeclarationTransition {
                expected: observation.clone(),
                next: observation,
            },
        );
        let prepared = prepare_database_runtime_change(session, request, &physical).unwrap();
        (prepared, physical)
    }

    #[test]
    fn confirmed_edits_batch_garbage_collection_and_save_collects_immediately() {
        let (fixture, session) = session_with_table("batched-gc");
        let id = fixture.instance.decl.id.clone();
        let files = fixture.instance.snapshot().unwrap().file_paths();
        let dataset_directory = files[0].parent().unwrap().parent().unwrap();
        let orphan = || {
            let directory = dataset_directory.join(uuid::Uuid::new_v4().to_string());
            std::fs::create_dir(&directory).unwrap();
            let file = directory.join("part-000000.parquet");
            std::fs::write(&file, b"abandoned preparation").unwrap();
            file
        };
        let apply = |operation, operation_id: &str| {
            let mut prepared = session
                .prepare_physical_mutation(&id, &operation, operation_id)
                .unwrap();
            prepared.commit().unwrap();
            prepared.confirm().unwrap();
        };
        let abandoned = orphan();
        for index in 0..63 {
            apply(
                DatabaseMutationOperation::RenameDatabase {
                    name: "Sales".into(),
                },
                &format!("edit-{index}"),
            );
        }
        assert!(
            abandoned.exists(),
            "ordinary edits must not sweep the directory each time"
        );
        apply(
            DatabaseMutationOperation::RenameDatabase {
                name: "Sales".into(),
            },
            "edit-63",
        );
        assert!(
            !abandoned.exists(),
            "periodic collection must eventually reclaim garbage"
        );

        let abandoned = orphan();
        apply(
            DatabaseMutationOperation::EditCell {
                row: 0,
                column: "value".into(),
                value: yss_data_contract::TabularScalar::Null,
                row_id: Some(0),
            },
            "cell-edit",
        );
        assert!(session.read_physical_edit_state(&id).unwrap().can_undo);
        assert!(abandoned.exists());
        apply(DatabaseMutationOperation::Save, "save");
        assert!(
            !abandoned.exists(),
            "save must collect without waiting for the edit interval"
        );
        assert!(!session.read_physical_edit_state(&id).unwrap().can_undo);
        assert!(files.iter().all(|file| file.is_file()));
    }

    #[test]
    fn catalog_snapshot_revalidation_is_session_and_revision_exact() {
        let (_fixture, first_session) = session_with_table("session");
        let snapshot = catalog_snapshot(&first_session).unwrap();
        revalidate_catalog_snapshot(&first_session, &snapshot).unwrap();
        let other = session_with("other-session");
        assert_eq!(
            revalidate_catalog_snapshot(&other, &snapshot)
                .unwrap_err()
                .code(),
            crate::error::DatabaseErrorCode::Conflict
        );
        let apply = |operation, operation_id: &str| {
            let (prepared, mut physical) = prepare_change(&first_session, operation, operation_id);
            let committed = commit_database_runtime_change(&first_session, prepared).unwrap();
            physical.commit().unwrap();
            committed.confirm();
            physical.confirm().unwrap();
        };
        apply(
            DatabaseMutationOperation::EditCell {
                row: 0,
                column: "value".into(),
                value: TabularScalar::Null,
                row_id: Some(0),
            },
            "edit",
        );
        assert_eq!(
            revalidate_catalog_snapshot(&first_session, &snapshot)
                .unwrap_err()
                .code(),
            crate::error::DatabaseErrorCode::Conflict
        );

        let fresh = catalog_snapshot(&first_session).unwrap();
        apply(
            DatabaseMutationOperation::RenameColumn {
                old_name: "value".into(),
                new_name: "value".into(),
            },
            "unchanged-schema",
        );
        assert_eq!(
            revalidate_catalog_snapshot(&first_session, &fresh)
                .unwrap_err()
                .code(),
            crate::error::DatabaseErrorCode::Conflict,
            "an unchanged physical schema must not advance the schema revision"
        );
        let fresh = catalog_snapshot(&first_session).unwrap();
        apply(
            DatabaseMutationOperation::AddColumn {
                name: "new_column".into(),
                data_type: DataType::Utf8,
            },
            "add-column",
        );
        assert_eq!(
            revalidate_catalog_snapshot(&first_session, &fresh)
                .unwrap_err()
                .code(),
            crate::error::DatabaseErrorCode::Schema
        );
    }

    #[test]
    fn arrow_snapshots_reject_empty_and_duplicate_column_selections() {
        let (_fixture, session) = session_with_table("session");
        let column = TabularColumnName::try_from("value").unwrap();
        for columns in [Vec::new(), vec![column.clone(), column]] {
            let error = arrow_snapshot(
                &session,
                DatabaseDataSnapshotRequest {
                    database: DatabaseId::from_existing(SALES_ID.into()),
                    columns: DatabaseColumnSelection::Selected(columns.into_boxed_slice()),
                    offset: 0,
                    limit: 1,
                },
            )
            .err()
            .expect("invalid column selection must fail");
            assert_eq!(
                error.code(),
                crate::error::DatabaseErrorCode::InvalidRequest
            );
        }
    }

    #[test]
    fn mutation_registration_and_abandoned_recovery_are_explicitly_drained() {
        let (_fixture, session) = session_with_table("recovery");
        let observation = session.observations().iter().next().unwrap().1.clone();
        let operation = DatabaseMutationOperation::EditCell {
            row: 0,
            column: "value".into(),
            value: TabularScalar::Null,
            row_id: Some(0),
        };
        let mut physical = session
            .prepare_physical_mutation(
                &DatabaseId::from_existing(SALES_ID.into()),
                &operation,
                "abandoned-edit",
            )
            .unwrap();
        assert!(!physical.requires_recovery());
        let prepared = prepare_database_runtime_change(
            &session,
            DatabaseMutationRequest {
                database: DatabaseId::from_existing(SALES_ID.into()),
                expected_runtime_revision: DatabaseRuntimeRevision::INITIAL,
                declaration_transition: DatabaseDeclarationTransition {
                    expected: observation.clone(),
                    next: observation,
                },
            },
            &physical,
        )
        .unwrap();
        assert_eq!(session.outstanding_work().pending_prepares(), 1);

        let committed = commit_database_runtime_change(&session, prepared).unwrap();
        assert_eq!(session.outstanding_work().pending_prepares(), 0);
        assert_eq!(session.outstanding_work().committed_changes(), 1);
        physical.commit().unwrap();
        assert!(physical.requires_recovery());
        drop(physical);

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

        assert_eq!(session.resolve_storage_recoveries().unwrap(), 1);
        assert_eq!(session.resolve_storage_recoveries().unwrap(), 0);
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
    fn concurrent_preparations_reject_stale_commit_and_retry_after_compensation() {
        let (_fixture, session) = session_with_table("concurrent-prepares");
        let database = DatabaseId::from_existing(SALES_ID.into());
        let original = page_snapshot(&session, database.clone(), 0, 1).unwrap();
        let edit = || DatabaseMutationOperation::EditCell {
            row: 0,
            column: "value".into(),
            value: TabularScalar::Null,
            row_id: Some(0),
        };
        let (first, first_physical) = prepare_change(&session, edit(), "first");
        let (second, second_physical) = prepare_change(&session, edit(), "second");
        assert_eq!(session.outstanding_work().pending_prepares(), 2);
        let first = commit_database_runtime_change(&session, first).unwrap();
        let error = commit_database_runtime_change(&session, second)
            .err()
            .expect("a proposal captured before the first registration must be stale");
        assert_eq!(error.code(), crate::error::DatabaseErrorCode::Conflict);
        assert_eq!(session.outstanding_work().pending_prepares(), 0);
        assert_eq!(session.outstanding_work().committed_changes(), 1);
        assert_eq!(
            session.capture_query_basis(&database).unwrap_err().code(),
            crate::error::DatabaseErrorCode::Conflict
        );
        assert!(matches!(
            first.compensate(),
            DatabaseCompensationAttempt::Restored
        ));
        drop(first_physical);
        drop(second_physical);
        assert_eq!(session.outstanding_work(), Default::default());
        assert_eq!(session.runtime_revision(&database).unwrap().get(), 0);
        assert_eq!(
            page_snapshot(&session, database.clone(), 0, 1).unwrap(),
            original
        );

        let (retry, mut physical) = prepare_change(&session, edit(), "retry");
        let committed = commit_database_runtime_change(&session, retry).unwrap();
        physical.commit().unwrap();
        committed.confirm();
        physical.confirm().unwrap();
        assert_eq!(session.outstanding_work(), Default::default());
        assert_eq!(
            page_snapshot(&session, database, 0, 1)
                .unwrap()
                .rows()
                .columns()[0]
                .values(),
            &[TabularScalar::Null]
        );
    }
}
