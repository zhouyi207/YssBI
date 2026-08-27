use crate::data_contract::DataType;
use crate::database_contract::DatabaseId;
use crate::tabular::contract::TabularColumnName;
use polars::prelude::{DataFrame, DataType as PolarsDataType};

use super::DuckDbColumnMeta;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct DatabaseRuntimeRevision(u64);

impl DatabaseRuntimeRevision {
    pub(crate) const INITIAL: Self = Self(0);

    pub(crate) const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    #[allow(
        dead_code,
        reason = "revision projection is staged for the database session API"
    )]
    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct DatabaseSchemaRevision(u64);

impl DatabaseSchemaRevision {
    pub(crate) const INITIAL: Self = Self(0);

    pub(crate) const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    #[allow(
        dead_code,
        reason = "revision projection is staged for the database session API"
    )]
    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DatabaseColumnFact {
    name: TabularColumnName,
    data_type: DataType,
    nullable: bool,
}

impl DatabaseColumnFact {
    pub(crate) fn name(&self) -> &TabularColumnName {
        &self.name
    }

    pub(crate) fn data_type(&self) -> &DataType {
        &self.data_type
    }

    #[allow(
        dead_code,
        reason = "nullability projection is staged for schema consumers"
    )]
    pub(crate) const fn nullable(&self) -> bool {
        self.nullable
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DatabaseSchemaFact {
    database: DatabaseId,
    runtime_revision: DatabaseRuntimeRevision,
    schema_revision: DatabaseSchemaRevision,
    columns: Box<[DatabaseColumnFact]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub(crate) enum DatabaseSchemaFactError {
    #[error("database schema contains an invalid column name")]
    InvalidColumnName,
}

impl DatabaseSchemaFact {
    #[cfg(test)]
    pub(crate) fn from_columns(
        database: DatabaseId,
        runtime_revision: u64,
        schema_revision: u64,
        columns: Box<[DatabaseColumnFact]>,
    ) -> Self {
        Self {
            database,
            runtime_revision: DatabaseRuntimeRevision::from_existing(runtime_revision),
            schema_revision: DatabaseSchemaRevision::from_existing(schema_revision),
            columns,
        }
    }

    pub(crate) fn empty(database: DatabaseId, runtime_revision: u64, schema_revision: u64) -> Self {
        Self {
            database,
            runtime_revision: DatabaseRuntimeRevision::from_existing(runtime_revision),
            schema_revision: DatabaseSchemaRevision::from_existing(schema_revision),
            columns: Box::new([]),
        }
    }

    pub(crate) fn from_dataframe(
        database: &DatabaseId,
        dataframe: &DataFrame,
    ) -> Result<Self, DatabaseSchemaFactError> {
        let columns = dataframe
            .columns()
            .iter()
            .map(|column| {
                let name = column.name().to_string();
                Ok(DatabaseColumnFact {
                    name: canonical_column_name(&name)?,
                    data_type: polars_dtype_to_data_type(column.dtype()),
                    nullable: column.null_count() > 0,
                })
            })
            .collect::<Result<Vec<_>, DatabaseSchemaFactError>>()?;

        Ok(Self {
            database: database.clone(),
            runtime_revision: DatabaseRuntimeRevision::INITIAL,
            schema_revision: DatabaseSchemaRevision::INITIAL,
            columns: columns.into_boxed_slice(),
        })
    }

    pub(crate) fn from_duckdb(
        database: &DatabaseId,
        columns: &[DuckDbColumnMeta],
    ) -> Result<Self, DatabaseSchemaFactError> {
        let columns = columns
            .iter()
            .map(|column| {
                Ok(DatabaseColumnFact {
                    name: canonical_column_name(&column.name)?,
                    data_type: polars_type_string_to_data_type(&column.dtype),
                    nullable: true,
                })
            })
            .collect::<Result<Vec<_>, DatabaseSchemaFactError>>()?;

        Ok(Self {
            database: database.clone(),
            runtime_revision: DatabaseRuntimeRevision::INITIAL,
            schema_revision: DatabaseSchemaRevision::INITIAL,
            columns: columns.into_boxed_slice(),
        })
    }

    #[allow(
        dead_code,
        reason = "database projection is staged for the database session API"
    )]
    pub(crate) fn database(&self) -> &DatabaseId {
        &self.database
    }

    #[allow(
        dead_code,
        reason = "revision projection is staged for the database session API"
    )]
    pub(crate) const fn runtime_revision(&self) -> DatabaseRuntimeRevision {
        self.runtime_revision
    }

    #[allow(
        dead_code,
        reason = "revision projection is staged for the database session API"
    )]
    pub(crate) const fn schema_revision(&self) -> DatabaseSchemaRevision {
        self.schema_revision
    }

    pub(crate) fn columns(&self) -> &[DatabaseColumnFact] {
        &self.columns
    }
}

#[cfg(test)]
pub(crate) fn database_column_fact_fixture(
    name: TabularColumnName,
    data_type: DataType,
    nullable: bool,
) -> DatabaseColumnFact {
    DatabaseColumnFact {
        name,
        data_type,
        nullable,
    }
}

fn canonical_column_name(name: &str) -> Result<TabularColumnName, DatabaseSchemaFactError> {
    TabularColumnName::try_from(name).map_err(|_| DatabaseSchemaFactError::InvalidColumnName)
}

fn polars_dtype_to_data_type(dtype: &PolarsDataType) -> DataType {
    match dtype {
        PolarsDataType::Boolean => DataType::Boolean,
        PolarsDataType::Int8
        | PolarsDataType::Int16
        | PolarsDataType::Int32
        | PolarsDataType::Int64
        | PolarsDataType::UInt8
        | PolarsDataType::UInt16
        | PolarsDataType::UInt32
        | PolarsDataType::UInt64 => DataType::Int64,
        PolarsDataType::Float32 | PolarsDataType::Float64 | PolarsDataType::Decimal(_, _) => {
            DataType::Float64
        }
        PolarsDataType::String => DataType::String,
        PolarsDataType::Date => DataType::Date,
        PolarsDataType::Datetime(_, _) => DataType::Datetime,
        PolarsDataType::Time => DataType::Time,
        PolarsDataType::Categorical(_, _) | PolarsDataType::Enum(_, _) => DataType::Categorical,
        _ => DataType::Any,
    }
}

fn polars_type_string_to_data_type(source: &str) -> DataType {
    let value = source.trim();
    if value.is_empty() {
        return DataType::Any;
    }

    match value {
        "Boolean" => DataType::Boolean,
        "Int8" | "Int16" | "Int32" | "Int64" | "UInt8" | "UInt16" | "UInt32" | "UInt64" => {
            DataType::Int64
        }
        "Float32" | "Float64" => DataType::Float64,
        "String" | "Utf8" => DataType::String,
        "Date" => DataType::Date,
        "Time" => DataType::Time,
        _ if value.starts_with("Datetime(") || value.starts_with("DateTime(") => DataType::Datetime,
        _ if value.starts_with("Time") => DataType::Time,
        _ if value.starts_with("Decimal(") => DataType::Float64,
        _ if value.starts_with("Categorical(") || value.starts_with("Enum(") => {
            DataType::Categorical
        }
        _ => DataType::Any,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn database_schema_facts_map_to_column_info_dto_wire() {
        let database = DatabaseId::from_existing("database-id".into());
        let source_columns = [
            DuckDbColumnMeta {
                name: "value".to_string(),
                dtype: "Int64".to_string(),
            },
            DuckDbColumnMeta {
                name: "label".to_string(),
                dtype: "String".to_string(),
            },
        ];
        let fact = DatabaseSchemaFact::from_duckdb(&database, &source_columns)
            .expect("test database schema should be valid");

        let wire = serde_json::to_value(crate::schema::column_info_from_schema(fact.columns()))
            .expect("column info DTOs should serialize");

        assert_eq!(
            wire,
            json!([
                { "name": "value", "type": "Int64" },
                { "name": "label", "type": "String" },
            ])
        );
    }
}
