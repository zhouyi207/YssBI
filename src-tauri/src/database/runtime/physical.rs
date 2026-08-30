use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use polars::prelude::{AnyValue, DataFrame};
use std::path::Path;

use crate::database::database_instance::DatabaseInstance;
use crate::database::database_state::DatabaseState;
use crate::database::error::{DatabaseDriverError, DatabaseError, DatabaseOperation};
use crate::database::schema_snapshot::{DatabaseColumnFact, DatabaseSchemaFact};
use crate::database::session_api::DatabaseMutationOperation;
use yss_database_contract::{DatabaseDecl, DatabaseId};
use yss_tabular_contract::{
    FiniteTabularDecimal, TabularColumn, TabularColumnName, TabularScalar, TabularSnapshot,
};

pub(crate) struct DatabaseRuntimeDataSnapshot {
    pub(crate) columns: Box<[DatabaseColumnFact]>,
    pub(crate) rows: TabularSnapshot,
}

pub(crate) struct DatabaseRuntimePageSnapshot {
    pub(crate) rows: TabularSnapshot,
    pub(crate) row_ids: Vec<i64>,
}

pub(crate) struct DatabaseRuntimePhysicalState {
    instances: Mutex<BTreeMap<DatabaseId, DatabaseInstance>>,
}

impl DatabaseRuntimePhysicalState {
    pub(crate) fn empty() -> Arc<Self> {
        Arc::new(Self {
            instances: Mutex::new(BTreeMap::new()),
        })
    }

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

    pub(crate) fn read_columns(
        &self,
        database: &DatabaseId,
        requested: Option<&[TabularColumnName]>,
        offset: usize,
        limit: usize,
    ) -> Result<DatabaseRuntimeDataSnapshot, DatabaseError> {
        // Clone only the runtime handle under the lock. Loaded data keeps its Arc-backed
        // dataframe, while DuckDB keeps only the path and table metadata. All physical I/O and
        // value conversion happen after the lock is released.
        let mut instance = self.instance_snapshot(database)?.ok_or_else(|| {
            DatabaseError::unsupported(DatabaseOperation::DataSnapshot, Some(database.clone()))
        })?;

        let schema = schema_for_instance(&instance, database)?;
        let selected = select_columns(&schema, requested, database)?;
        let names = selected
            .iter()
            .map(|column| column.name().as_str())
            .collect::<Vec<_>>();
        let dataframe = instance.load_columns(&names).map_err(|error| {
            DatabaseError::driver(
                DatabaseOperation::DataSnapshot,
                Some(database.clone()),
                DatabaseDriverError::Polars(error),
            )
        })?;
        let dataframe = slice_dataframe(dataframe, offset, limit, database)?;
        let rows = dataframe_to_snapshot(&dataframe, database)?;

        Ok(DatabaseRuntimeDataSnapshot {
            columns: selected.into_boxed_slice(),
            rows,
        })
    }

    pub(crate) fn read_schema(
        &self,
        database: &DatabaseId,
    ) -> Result<Option<DatabaseSchemaFact>, DatabaseError> {
        let Some(instance) = self.instance_snapshot(database)? else {
            return Ok(None);
        };
        schema_for_instance(&instance, database).map(Some)
    }

    pub(crate) fn read_metadata(
        &self,
        database: &DatabaseId,
    ) -> Result<DatabaseRuntimeMetadata, DatabaseError> {
        let instance = self.instance_snapshot(database)?.ok_or_else(|| {
            DatabaseError::not_found(DatabaseOperation::Query, Some(database.clone()))
        })?;
        let row_count = match &instance.state {
            DatabaseState::Loaded { dataframe, .. } => dataframe.height(),
            DatabaseState::DuckDb { row_count, .. } => *row_count,
            DatabaseState::Failed { .. } => {
                return Err(DatabaseError::unsupported(
                    DatabaseOperation::Query,
                    Some(database.clone()),
                ));
            }
        };
        let schema = schema_for_instance(&instance, database).map_err(|error| {
            DatabaseError::schema(DatabaseOperation::Query, error.resource().cloned())
        })?;
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
        let mut instance = self.instance_snapshot(database)?.ok_or_else(|| {
            DatabaseError::not_found(DatabaseOperation::Query, Some(database.clone()))
        })?;
        let page = instance
            .query_page_with_rowids(offset, limit)
            .map_err(|error| {
                DatabaseError::driver(
                    DatabaseOperation::Query,
                    Some(database.clone()),
                    DatabaseDriverError::Polars(error),
                )
            })?;
        let rows = dataframe_to_snapshot(&page.dataframe, database)?;
        Ok(DatabaseRuntimePageSnapshot {
            rows,
            row_ids: page.row_ids,
        })
    }

    pub(crate) fn read_column_stats(
        &self,
        database: &DatabaseId,
    ) -> Result<Vec<yss_dataset_profile::ColumnStats>, DatabaseError> {
        let mut instance = self.instance_snapshot(database)?.ok_or_else(|| {
            DatabaseError::not_found(DatabaseOperation::Query, Some(database.clone()))
        })?;
        instance.compute_column_stats().map_err(|error| {
            DatabaseError::driver(
                DatabaseOperation::Query,
                Some(database.clone()),
                DatabaseDriverError::Polars(error),
            )
        })
    }

    pub(crate) fn read_column_distributions(
        &self,
        database: &DatabaseId,
    ) -> Result<Vec<yss_dataset_profile::ColumnDistribution>, DatabaseError> {
        let mut instance = self.instance_snapshot(database)?.ok_or_else(|| {
            DatabaseError::not_found(DatabaseOperation::Query, Some(database.clone()))
        })?;
        instance.compute_column_distributions().map_err(|error| {
            DatabaseError::driver(
                DatabaseOperation::Query,
                Some(database.clone()),
                DatabaseDriverError::Polars(error),
            )
        })
    }

    pub(crate) fn read_dataset_overview(
        &self,
        database: &DatabaseId,
    ) -> Result<yss_dataset_profile::DatasetOverview, DatabaseError> {
        let mut instance = self.instance_snapshot(database)?.ok_or_else(|| {
            DatabaseError::not_found(DatabaseOperation::Query, Some(database.clone()))
        })?;
        instance.compute_dataset_overview().map_err(|error| {
            DatabaseError::driver(
                DatabaseOperation::Query,
                Some(database.clone()),
                DatabaseDriverError::Polars(error),
            )
        })
    }

    pub(crate) fn read_edit_state(
        &self,
        database: &DatabaseId,
    ) -> Result<crate::database::EditState, DatabaseError> {
        let instance = self.instance_snapshot(database)?.ok_or_else(|| {
            DatabaseError::not_found(DatabaseOperation::Query, Some(database.clone()))
        })?;
        Ok(instance.edit_state())
    }

    pub(crate) fn export_to_path(
        &self,
        database: &DatabaseId,
        path: &Path,
        format: crate::database::DatabaseExportFormat,
    ) -> Result<(), DatabaseError> {
        let instance = self.instance_snapshot(database)?.ok_or_else(|| {
            DatabaseError::not_found(DatabaseOperation::Query, Some(database.clone()))
        })?;
        instance.export_to_path(path, format).map_err(|error| {
            DatabaseError::driver(
                DatabaseOperation::Query,
                Some(database.clone()),
                DatabaseDriverError::Operation(error.into_boxed_str()),
            )
        })
    }

    pub(crate) fn remove_database(
        &self,
        database: &DatabaseId,
        project_root: &Path,
    ) -> Result<(), DatabaseError> {
        let instance = self.instance_snapshot(database)?.ok_or_else(|| {
            DatabaseError::not_found(DatabaseOperation::CommitMutation, Some(database.clone()))
        })?;
        crate::database::remove_duckdb_table_if_needed(&instance.decl.engine, Some(project_root))
            .map_err(|error| {
                DatabaseError::driver(
                    DatabaseOperation::CommitMutation,
                    Some(database.clone()),
                    DatabaseDriverError::Operation(error.into_boxed_str()),
                )
            })
    }

    pub(crate) fn prepare_mutation(
        self: &Arc<Self>,
        database: &DatabaseId,
        operation: &DatabaseMutationOperation,
    ) -> Result<PreparedDatabasePhysicalMutation, DatabaseError> {
        let before = self.instance_snapshot(database)?.ok_or_else(|| {
            DatabaseError::not_found(DatabaseOperation::PrepareMutation, Some(database.clone()))
        })?;
        let mut after = before.clone();
        apply_mutation(&mut after, operation).map_err(|error| {
            DatabaseError::driver(
                DatabaseOperation::PrepareMutation,
                Some(database.clone()),
                DatabaseDriverError::Operation(error.into_boxed_str()),
            )
        })?;
        Ok(PreparedDatabasePhysicalMutation {
            physical: Arc::clone(self),
            database: database.clone(),
            before,
            after,
            operation: operation.clone(),
        })
    }

    pub(crate) fn install_mutation(&self, mutation: &PreparedDatabasePhysicalMutation) {
        self.instances
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(mutation.database.clone(), mutation.after.clone());
    }

    pub(crate) fn restore_mutation(&self, mutation: &PreparedDatabasePhysicalMutation) {
        self.instances
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(mutation.database.clone(), mutation.before.clone());
    }

    pub(crate) fn instances_for_replacement(&self) -> Vec<DatabaseInstance> {
        self.instances
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .cloned()
            .collect()
    }

    fn instance_snapshot(
        &self,
        database: &DatabaseId,
    ) -> Result<Option<DatabaseInstance>, DatabaseError> {
        let instances = self
            .instances
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        Ok(instances.get(database).cloned())
    }
}

pub(crate) struct DatabaseRuntimeMetadata {
    pub(crate) name: Box<str>,
    pub(crate) schema: DatabaseSchemaFact,
    pub(crate) row_count: usize,
}

pub(crate) struct PreparedDatabasePhysicalMutation {
    physical: Arc<DatabaseRuntimePhysicalState>,
    database: DatabaseId,
    before: DatabaseInstance,
    after: DatabaseInstance,
    operation: DatabaseMutationOperation,
}

impl PreparedDatabasePhysicalMutation {
    pub(crate) fn edit_state(&self) -> crate::database::EditState {
        self.after.edit_state()
    }

    pub(crate) fn rollback(&self) -> Result<(), DatabaseError> {
        if !matches!(self.before.state, DatabaseState::DuckDb { .. }) {
            self.physical.restore_mutation(self);
            return Ok(());
        }

        let mut current = self.after.clone();
        let result = match &self.operation {
            DatabaseMutationOperation::Undo => current.redo_edit(),
            DatabaseMutationOperation::RenameDatabase { .. } => {
                current.rename_display_name(&self.before.decl.name)
            }
            _ => current.undo_edit(),
        };
        result.map_err(|error| {
            DatabaseError::driver(
                DatabaseOperation::CommitMutation,
                Some(self.database.clone()),
                DatabaseDriverError::Operation(error.into_boxed_str()),
            )
        })?;
        self.physical.restore_mutation(self);
        Ok(())
    }
}

fn apply_mutation(
    instance: &mut DatabaseInstance,
    operation: &DatabaseMutationOperation,
) -> Result<crate::database::EditState, String> {
    match operation {
        DatabaseMutationOperation::EditCell {
            row,
            column,
            value,
            row_id,
        } => instance.edit_cell(*row, column, tabular_scalar_to_json(value), *row_id),
        DatabaseMutationOperation::AddRow { index } => instance.add_row(Some(*index)),
        DatabaseMutationOperation::DeleteRows { indices, row_ids } => {
            instance.delete_rows(indices, row_ids.as_deref())
        }
        DatabaseMutationOperation::AddColumn { name, data_type } => {
            instance.add_column(name, &data_type.to_string())
        }
        DatabaseMutationOperation::DeleteColumn { name } => instance.delete_column(name),
        DatabaseMutationOperation::CastColumn {
            name,
            data_type,
            force,
        } => instance.cast_column(name, &data_type.to_string(), *force),
        DatabaseMutationOperation::RenameColumn { old_name, new_name } => {
            instance.rename_column(old_name, new_name)
        }
        DatabaseMutationOperation::RenameDatabase { name } => instance.rename_display_name(name),
        DatabaseMutationOperation::Undo => instance.undo_edit(),
        DatabaseMutationOperation::Redo => instance.redo_edit(),
        DatabaseMutationOperation::Save => instance.save_changes(None),
    }
}

fn tabular_scalar_to_json(value: &TabularScalar) -> serde_json::Value {
    match value {
        TabularScalar::Null => serde_json::Value::Null,
        TabularScalar::Bool(value) => serde_json::Value::Bool(*value),
        TabularScalar::Integer(value) => serde_json::Value::Number((*value).into()),
        TabularScalar::Unsigned(value) => serde_json::Value::Number((*value).into()),
        TabularScalar::Decimal(value) => serde_json::Number::from_f64(value.as_f64())
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        TabularScalar::String(value) => serde_json::Value::String(value.to_string()),
    }
}

fn schema_for_instance(
    instance: &DatabaseInstance,
    database: &DatabaseId,
) -> Result<DatabaseSchemaFact, DatabaseError> {
    match &instance.state {
        DatabaseState::DuckDb { columns, .. } => DatabaseSchemaFact::from_duckdb(database, columns)
            .map_err(|_| {
                DatabaseError::schema(DatabaseOperation::DataSnapshot, Some(database.clone()))
            }),
        DatabaseState::Loaded { dataframe, .. } => {
            DatabaseSchemaFact::from_dataframe(database, dataframe).map_err(|_| {
                DatabaseError::schema(DatabaseOperation::DataSnapshot, Some(database.clone()))
            })
        }
        DatabaseState::Failed { .. } => Err(DatabaseError::unsupported(
            DatabaseOperation::DataSnapshot,
            Some(database.clone()),
        )),
    }
}

fn select_columns(
    schema: &DatabaseSchemaFact,
    requested: Option<&[TabularColumnName]>,
    database: &DatabaseId,
) -> Result<Vec<DatabaseColumnFact>, DatabaseError> {
    let Some(requested) = requested else {
        return Ok(schema.columns().to_vec());
    };
    if requested.is_empty() {
        return Err(DatabaseError::invalid_request(
            DatabaseOperation::DataSnapshot,
            Some(database.clone()),
        ));
    }

    let mut seen = std::collections::BTreeSet::new();
    requested
        .iter()
        .map(|column| {
            if !seen.insert(column.clone()) {
                return Err(DatabaseError::invalid_request(
                    DatabaseOperation::DataSnapshot,
                    Some(database.clone()),
                ));
            }
            schema
                .columns()
                .iter()
                .find(|fact| fact.name() == column)
                .cloned()
                .ok_or_else(|| {
                    DatabaseError::not_found(
                        DatabaseOperation::DataSnapshot,
                        Some(database.clone()),
                    )
                })
        })
        .collect()
}

fn slice_dataframe(
    dataframe: DataFrame,
    offset: usize,
    limit: usize,
    database: &DatabaseId,
) -> Result<DataFrame, DatabaseError> {
    let start = offset.min(dataframe.height());
    let count = limit.min(dataframe.height().saturating_sub(start));
    let start = i64::try_from(start).map_err(|_| {
        DatabaseError::invalid_request(DatabaseOperation::DataSnapshot, Some(database.clone()))
    })?;
    Ok(dataframe.slice(start, count))
}

fn dataframe_to_snapshot(
    dataframe: &DataFrame,
    database: &DatabaseId,
) -> Result<TabularSnapshot, DatabaseError> {
    let columns = dataframe
        .columns()
        .iter()
        .map(|column| {
            let series = column.as_materialized_series();
            let values = (0..series.len())
                .map(|row| {
                    series
                        .get(row)
                        .map_err(|error| {
                            DatabaseError::driver(
                                DatabaseOperation::DataSnapshot,
                                Some(database.clone()),
                                DatabaseDriverError::Polars(error),
                            )
                        })
                        .and_then(tabular_scalar)
                })
                .collect::<Result<Vec<_>, _>>()?;
            Ok(TabularColumn::new(
                TabularColumnName::try_from(series.name().as_str()).map_err(|_| {
                    DatabaseError::schema(DatabaseOperation::DataSnapshot, Some(database.clone()))
                })?,
                values.into_boxed_slice(),
            ))
        })
        .collect::<Result<Vec<_>, DatabaseError>>()?;

    TabularSnapshot::try_from_columns(columns.into_boxed_slice()).map_err(|_| {
        DatabaseError::invalid_request(DatabaseOperation::DataSnapshot, Some(database.clone()))
    })
}

fn tabular_scalar(value: AnyValue<'_>) -> Result<TabularScalar, DatabaseError> {
    let scalar = match value {
        AnyValue::Null => TabularScalar::Null,
        AnyValue::Boolean(value) => TabularScalar::Bool(value),
        AnyValue::Int8(value) => TabularScalar::Integer(i64::from(value)),
        AnyValue::Int16(value) => TabularScalar::Integer(i64::from(value)),
        AnyValue::Int32(value) => TabularScalar::Integer(i64::from(value)),
        AnyValue::Int64(value) => TabularScalar::Integer(value),
        AnyValue::UInt8(value) => TabularScalar::Unsigned(u64::from(value)),
        AnyValue::UInt16(value) => TabularScalar::Unsigned(u64::from(value)),
        AnyValue::UInt32(value) => TabularScalar::Unsigned(u64::from(value)),
        AnyValue::UInt64(value) => TabularScalar::Unsigned(value),
        AnyValue::Int128(value) => finite_decimal(value as f64),
        AnyValue::UInt128(value) => finite_decimal(value as f64),
        AnyValue::Float32(value) => finite_decimal(f64::from(value)),
        AnyValue::Float64(value) => finite_decimal(value),
        AnyValue::Date(value) => TabularScalar::Integer(i64::from(value)),
        AnyValue::Datetime(value, _, _) | AnyValue::DatetimeOwned(value, _, _) => {
            TabularScalar::Integer(value)
        }
        AnyValue::Duration(value, _) | AnyValue::Time(value) => TabularScalar::Integer(value),
        AnyValue::Decimal(value, _, scale) => {
            let Some(scale) = i32::try_from(scale).ok() else {
                return Err(unsupported_scalar());
            };
            finite_decimal((value as f64) / 10_f64.powi(scale))
        }
        AnyValue::String(value) => TabularScalar::String(value.into()),
        AnyValue::StringOwned(value) => TabularScalar::String(value.to_string().into()),
        _ => return Err(unsupported_scalar()),
    };
    Ok(scalar)
}

fn finite_decimal(value: f64) -> TabularScalar {
    FiniteTabularDecimal::try_from(value)
        .map(TabularScalar::Decimal)
        .unwrap_or(TabularScalar::Null)
}

fn unsupported_scalar() -> DatabaseError {
    DatabaseError::unsupported(DatabaseOperation::DataSnapshot, None)
}
