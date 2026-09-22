use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use crate::database_instance::{PreparedInstanceMutation, query_control};
use arrow::array::Int64Array;
use std::path::Path;
use yss_database_store::{DatasetPublication, DatasetStore, DatasetStoreError};

use crate::DatabaseInstance;
use crate::error::{DatabaseError, DatabaseOperation};
use crate::session_api::DatabaseMutationOperation;
use yss_data_contract::{TabularColumn, TabularSnapshot};
use yss_database_contract::EditState;
use yss_database_contract::{DatabaseDecl, DatabaseExportFormat, DatabaseId};
use yss_database_schema::{DatabaseColumnFact, DatabaseSchemaFact};

pub(crate) struct DatabaseRuntimePageSnapshot {
    pub(crate) rows: TabularSnapshot,
    pub(crate) row_ids: Vec<i64>,
}

pub(crate) struct DatabaseRuntimePhysicalState {
    instances: Mutex<BTreeMap<DatabaseId, DatabaseInstance>>,
}

impl DatabaseRuntimePhysicalState {
    pub(crate) fn from_instances(
        declarations: &[DatabaseDecl],
        instances: impl IntoIterator<Item = DatabaseInstance>,
    ) -> Result<Arc<Self>, DatabaseError> {
        let declarations = declarations
            .iter()
            .map(|declaration| (declaration.id.clone(), declaration))
            .collect::<BTreeMap<_, _>>();
        let mut bound = BTreeMap::new();

        for instance in instances {
            let database = instance.decl.id.clone();
            let Some(declaration) = declarations.get(&database) else {
                return Err(DatabaseError::invalid_request(
                    DatabaseOperation::OpenSession,
                    Some(database),
                ));
            };
            if *declaration != &instance.decl || bound.contains_key(&database) {
                return Err(DatabaseError::invalid_request(
                    DatabaseOperation::OpenSession,
                    Some(database),
                ));
            }
            bound.insert(database, instance);
        }

        Ok(Arc::new(Self {
            instances: Mutex::new(bound),
        }))
    }

    pub(crate) fn read_schema(
        &self,
        database: &DatabaseId,
    ) -> Result<Option<DatabaseSchemaFact>, DatabaseError> {
        self.instance_snapshot(database)
            .map(|instance| {
                instance
                    .data_schema()
                    .map_err(|error| failure(database, DatabaseOperation::CatalogSnapshot, error))
            })
            .transpose()
    }
    pub(crate) fn read_metadata(
        &self,
        database: &DatabaseId,
    ) -> Result<DatabaseRuntimeMetadata, DatabaseError> {
        let instance = self.required_instance(database)?;
        let schema = instance
            .data_schema()
            .map_err(|error| failure(database, DatabaseOperation::Query, error))?;
        let row_count = instance
            .row_count()
            .map_err(|error| failure(database, DatabaseOperation::Query, error))?;
        Ok(DatabaseRuntimeMetadata {
            name: instance.decl.name,
            schema,
            row_count,
        })
    }
    pub(crate) fn read_page(
        &self,
        database: &DatabaseId,
        offset: usize,
        limit: usize,
    ) -> Result<DatabaseRuntimePageSnapshot, DatabaseError> {
        let instance = self.required_instance(database)?;
        let schema = instance
            .data_schema()
            .map_err(|error| failure(database, DatabaseOperation::Query, error))?;
        let count = limit.min(
            instance
                .row_count()
                .map_err(|error| failure(database, DatabaseOperation::Query, error))?
                .saturating_sub(offset),
        );
        if count == 0 {
            return Ok(DatabaseRuntimePageSnapshot {
                rows: empty_snapshot(schema.columns(), database)?,
                row_ids: Vec::new(),
            });
        }
        let page = instance
            .query()
            .and_then(|query| {
                query
                    .page(offset, count, &query_control(16 * 1024 * 1024))
                    .map_err(Into::into)
            })
            .map_err(|error| failure(database, DatabaseOperation::Query, error))?;
        let roles = yss_database_arrow::dataset_row_columns(
            &instance
                .snapshot()
                .map_err(|error| failure(database, DatabaseOperation::Query, error))?
                .metadata()
                .schema,
        )
        .map_err(|_| DatabaseError::schema(DatabaseOperation::Query, Some(database.clone())))?
        .ok_or_else(|| DatabaseError::schema(DatabaseOperation::Query, Some(database.clone())))?;
        let mut row_ids = Vec::new();
        let mut values = schema
            .columns()
            .iter()
            .map(|_| Vec::new())
            .collect::<Vec<_>>();
        for batch in page.batches {
            let ids = batch
                .column_by_name(&roles.row_id)
                .and_then(|array| array.as_any().downcast_ref::<Int64Array>())
                .ok_or_else(|| {
                    DatabaseError::schema(DatabaseOperation::Query, Some(database.clone()))
                })?;
            row_ids.extend(ids.values().iter().copied());
            for (column, values) in schema.columns().iter().zip(&mut values) {
                let array = batch
                    .column_by_name(column.name().as_str())
                    .ok_or_else(|| {
                        DatabaseError::schema(DatabaseOperation::Query, Some(database.clone()))
                    })?;
                for value in yss_database_arrow::array_to_json(array.as_ref()).map_err(|_| {
                    DatabaseError::schema(DatabaseOperation::Query, Some(database.clone()))
                })? {
                    values.push(serde_json::from_value(value).map_err(|_| {
                        DatabaseError::schema(DatabaseOperation::Query, Some(database.clone()))
                    })?);
                }
            }
        }
        let columns = schema
            .columns()
            .iter()
            .zip(values)
            .map(|(column, values)| {
                TabularColumn::new(column.name().clone(), values.into_boxed_slice())
            })
            .collect();
        let rows = TabularSnapshot::try_from_columns(columns)
            .map_err(|_| DatabaseError::schema(DatabaseOperation::Query, Some(database.clone())))?;
        Ok(DatabaseRuntimePageSnapshot { rows, row_ids })
    }
    pub(crate) fn read_column_stats(
        &self,
        database: &DatabaseId,
    ) -> Result<Vec<yss_dataset_profile::ColumnStats>, DatabaseError> {
        self.required_instance(database)?
            .query()
            .and_then(|query| {
                query
                    .column_stats(&query_control(16 * 1024 * 1024))
                    .map_err(Into::into)
            })
            .map_err(|error| failure(database, DatabaseOperation::Query, error))
    }
    pub(crate) fn read_column_distributions(
        &self,
        database: &DatabaseId,
    ) -> Result<Vec<yss_dataset_profile::ColumnDistribution>, DatabaseError> {
        self.required_instance(database)?
            .query()
            .and_then(|query| {
                query
                    .column_distributions(&query_control(16 * 1024 * 1024))
                    .map_err(Into::into)
            })
            .map_err(|error| failure(database, DatabaseOperation::Query, error))
    }
    pub(crate) fn read_dataset_overview(
        &self,
        database: &DatabaseId,
        control: &yss_relational_contract::RelationControl,
    ) -> Result<yss_dataset_profile::DatasetOverview, DatabaseError> {
        self.required_instance(database)?
            .query()
            .and_then(|query| query.dataset_overview(control).map_err(Into::into))
            .map_err(|error| failure(database, DatabaseOperation::Query, error))
    }
    pub(crate) fn read_edit_state(
        &self,
        database: &DatabaseId,
    ) -> Result<EditState, DatabaseError> {
        Ok(self.required_instance(database)?.edit_state())
    }
    pub(crate) fn export_to_path(
        &self,
        database: &DatabaseId,
        path: &Path,
        format: DatabaseExportFormat,
    ) -> Result<(), DatabaseError> {
        self.required_instance(database)?
            .export_to_path(path, format)
            .map_err(|_| DatabaseError::driver(DatabaseOperation::Query, Some(database.clone())))
    }
    pub(crate) fn prepare_mutation(
        self: &Arc<Self>,
        database: &DatabaseId,
        operation: &DatabaseMutationOperation,
        operation_id: &str,
    ) -> Result<PreparedDatabasePhysicalMutation, DatabaseError> {
        let before = self.required_instance(database)?;
        let pending = before
            .prepare_mutation(operation, operation_id)
            .map_err(|error| failure(database, DatabaseOperation::PrepareMutation, error))?;
        let (store, _) = pending.recovery();
        let edit_state = before.edit_state();
        Ok(PreparedDatabasePhysicalMutation {
            physical: self.clone(),
            database: database.clone(),
            store,
            edit_state,
            pending: Some(pending),
            publication: None,
            commit_uncertain: false,
            collect_garbage: matches!(
                operation,
                DatabaseMutationOperation::Save | DatabaseMutationOperation::DeleteDatabase
            ),
        })
    }
    fn install_instance(&self, instance: DatabaseInstance) {
        self.instances
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(instance.decl.id.clone(), instance);
    }
    pub(crate) fn instances_for_replacement(&self) -> Vec<DatabaseInstance> {
        self.instances
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .cloned()
            .collect()
    }
    pub(crate) fn required_instance(
        &self,
        database: &DatabaseId,
    ) -> Result<DatabaseInstance, DatabaseError> {
        self.instance_snapshot(database).ok_or_else(|| {
            DatabaseError::not_found(DatabaseOperation::Query, Some(database.clone()))
        })
    }
    fn instance_snapshot(&self, database: &DatabaseId) -> Option<DatabaseInstance> {
        self.instances
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(database)
            .cloned()
    }
}

pub(crate) struct DatabaseRuntimeMetadata {
    pub(crate) name: Box<str>,
    pub(crate) schema: DatabaseSchemaFact,
    pub(crate) row_count: usize,
}

pub struct PreparedDatabasePhysicalMutation {
    physical: Arc<DatabaseRuntimePhysicalState>,
    database: DatabaseId,
    store: Arc<DatasetStore>,
    edit_state: EditState,
    pending: Option<PreparedInstanceMutation>,
    publication: Option<DatasetPublication>,
    commit_uncertain: bool,
    collect_garbage: bool,
}
impl PreparedDatabasePhysicalMutation {
    pub(crate) fn schema_changed(&self) -> Result<bool, DatabaseError> {
        self.pending
            .as_ref()
            .ok_or_else(|| {
                DatabaseError::conflict(
                    DatabaseOperation::PrepareMutation,
                    Some(self.database.clone()),
                )
            })
            .map(PreparedInstanceMutation::schema_changed)
    }
    pub(crate) fn storage_recovery(
        &self,
    ) -> Result<super::registry::DatasetStorageRecovery, DatabaseError> {
        let pending = self.pending.as_ref().ok_or_else(|| {
            DatabaseError::conflict(
                DatabaseOperation::PrepareMutation,
                Some(self.database.clone()),
            )
        })?;
        let (store, publication) = pending.recovery();
        Ok(super::registry::DatasetStorageRecovery { store, publication })
    }
    pub fn edit_state(&self) -> EditState {
        self.pending.as_ref().map_or_else(
            || self.edit_state.clone(),
            PreparedInstanceMutation::edit_state,
        )
    }
    pub fn commit(&mut self) -> Result<(), DatabaseError> {
        let pending = self.pending.take().ok_or_else(|| {
            DatabaseError::conflict(
                DatabaseOperation::CommitMutation,
                Some(self.database.clone()),
            )
        })?;
        let (after, publication) = match pending.commit() {
            Ok(value) => value,
            Err(error) => {
                self.commit_uncertain = matches!(error, DatasetStoreError::CommitUncertain(_));
                return Err(failure(
                    &self.database,
                    DatabaseOperation::CommitMutation,
                    error,
                ));
            }
        };
        self.edit_state = after.edit_state();
        self.publication = Some(publication);
        self.physical.install_instance(after);
        Ok(())
    }
    fn acknowledge(&self) -> Result<(), DatabaseError> {
        let publication = self.publication.as_ref().ok_or_else(|| {
            DatabaseError::conflict(
                DatabaseOperation::CommitMutation,
                Some(self.database.clone()),
            )
        })?;
        self.store
            .acknowledge_publication(publication)
            .map_err(|error| failure(&self.database, DatabaseOperation::CommitMutation, error))
    }
    pub fn confirm(self) -> Result<(), DatabaseError> {
        self.acknowledge()?;
        let store = self.store.clone();
        let database = self.database.clone();
        let collect_garbage = self.collect_garbage;
        drop(self);
        if collect_garbage {
            store.collect_garbage()
        } else {
            store.collect_garbage_if_due()
        }
        .map_err(|error| failure(&database, DatabaseOperation::Recovery, error))?;
        Ok(())
    }
    /// A durable or uncertain commit must be resolved from the catalog before runtime compensation.
    pub fn requires_recovery(&self) -> bool {
        self.publication.is_some() || self.commit_uncertain
    }
}
fn failure(
    database: &DatabaseId,
    operation: DatabaseOperation,
    error: DatasetStoreError,
) -> DatabaseError {
    DatabaseError::dataset(operation, Some(database.clone()), error)
}
fn empty_snapshot(
    columns: &[DatabaseColumnFact],
    database: &DatabaseId,
) -> Result<TabularSnapshot, DatabaseError> {
    TabularSnapshot::try_from_columns(
        columns
            .iter()
            .map(|column| TabularColumn::new(column.name().clone(), Box::new([])))
            .collect(),
    )
    .map_err(|_| DatabaseError::schema(DatabaseOperation::Query, Some(database.clone())))
}
