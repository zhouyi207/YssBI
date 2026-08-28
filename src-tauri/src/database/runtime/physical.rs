use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use polars::prelude::{AnyValue, DataFrame};

use crate::database::database_instance::DatabaseInstance;
use crate::database::database_state::DatabaseState;
use crate::database::error::{DatabaseDriverError, DatabaseError, DatabaseOperation};
use crate::database::schema_snapshot::{DatabaseColumnFact, DatabaseSchemaFact};
use crate::database_contract::{DatabaseDecl, DatabaseId};
use crate::tabular::contract::{
    FiniteTabularDecimal, TabularColumn, TabularColumnName, TabularScalar, TabularSnapshot,
};

pub(crate) struct DatabaseRuntimeDataSnapshot {
    pub(crate) columns: Box<[DatabaseColumnFact]>,
    pub(crate) rows: TabularSnapshot,
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
