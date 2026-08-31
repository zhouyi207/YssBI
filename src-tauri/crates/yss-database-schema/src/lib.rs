//! Canonical runtime database schema facts and physical-schema normalization.
//!
//! This crate owns the typed schema/revision projection shared by database sessions, Graph
//! contracts, and transport adapters. It converts Polars and DuckDB metadata into the canonical
//! [`yss_data_contract::DataType`] vocabulary without owning session or Application authority.

use polars::prelude::{DataFrame, DataType as PolarsDataType};
use yss_data_contract::DataType;
use yss_database_contract::DatabaseId;
use yss_duckdb::DuckDbColumnMeta;
use yss_tabular_contract::TabularColumnName;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DatabaseRuntimeRevision(u64);

impl DatabaseRuntimeRevision {
    pub const INITIAL: Self = Self(0);

    pub const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DatabaseSchemaRevision(u64);

impl DatabaseSchemaRevision {
    pub const INITIAL: Self = Self(0);

    pub const fn from_existing(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseColumnFact {
    name: TabularColumnName,
    data_type: DataType,
    nullable: bool,
}

impl DatabaseColumnFact {
    pub fn new(name: TabularColumnName, data_type: DataType, nullable: bool) -> Self {
        Self {
            name,
            data_type,
            nullable,
        }
    }

    pub fn name(&self) -> &TabularColumnName {
        &self.name
    }

    pub fn data_type(&self) -> &DataType {
        &self.data_type
    }

    pub const fn nullable(&self) -> bool {
        self.nullable
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DatabaseSchemaFact {
    database: DatabaseId,
    runtime_revision: DatabaseRuntimeRevision,
    schema_revision: DatabaseSchemaRevision,
    columns: Box<[DatabaseColumnFact]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DatabaseSchemaFactError {
    #[error("database schema contains an invalid column name")]
    InvalidColumnName,
}

impl DatabaseSchemaFact {
    pub fn from_columns(
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

    pub fn with_revisions(self, runtime_revision: u64, schema_revision: u64) -> Self {
        Self {
            runtime_revision: DatabaseRuntimeRevision::from_existing(runtime_revision),
            schema_revision: DatabaseSchemaRevision::from_existing(schema_revision),
            ..self
        }
    }

    pub fn empty(database: DatabaseId, runtime_revision: u64, schema_revision: u64) -> Self {
        Self::from_columns(database, runtime_revision, schema_revision, Box::new([]))
    }

    pub fn from_dataframe(
        database: &DatabaseId,
        dataframe: &DataFrame,
    ) -> Result<Self, DatabaseSchemaFactError> {
        let columns = dataframe
            .columns()
            .iter()
            .map(|column| {
                let name = column.name().to_string();
                Ok(DatabaseColumnFact::new(
                    canonical_column_name(&name)?,
                    polars_dtype_to_data_type(column.dtype()),
                    column.null_count() > 0,
                ))
            })
            .collect::<Result<Vec<_>, DatabaseSchemaFactError>>()?;

        Ok(Self::from_columns(
            database.clone(),
            DatabaseRuntimeRevision::INITIAL.get(),
            DatabaseSchemaRevision::INITIAL.get(),
            columns.into_boxed_slice(),
        ))
    }

    pub fn from_duckdb(
        database: &DatabaseId,
        columns: &[DuckDbColumnMeta],
    ) -> Result<Self, DatabaseSchemaFactError> {
        let columns = columns
            .iter()
            .map(|column| {
                Ok(DatabaseColumnFact::new(
                    canonical_column_name(&column.name)?,
                    logical_type_name_to_data_type(&column.dtype),
                    true,
                ))
            })
            .collect::<Result<Vec<_>, DatabaseSchemaFactError>>()?;

        Ok(Self::from_columns(
            database.clone(),
            DatabaseRuntimeRevision::INITIAL.get(),
            DatabaseSchemaRevision::INITIAL.get(),
            columns.into_boxed_slice(),
        ))
    }

    pub fn database(&self) -> &DatabaseId {
        &self.database
    }

    pub const fn runtime_revision(&self) -> DatabaseRuntimeRevision {
        self.runtime_revision
    }

    pub const fn schema_revision(&self) -> DatabaseSchemaRevision {
        self.schema_revision
    }

    pub fn columns(&self) -> &[DatabaseColumnFact] {
        &self.columns
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

fn logical_type_name_to_data_type(source: &str) -> DataType {
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
        "DateTime" | "Datetime" => DataType::Datetime,
        "Time" => DataType::Time,
        "Categorical" | "Enum" => DataType::Categorical,
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
    use polars::prelude::{AnyValue, Column, PlSmallStr, Series, TimeUnit};

    #[test]
    fn duckdb_timestamp_metadata_maps_to_datetime_without_any_fallback() {
        let database = DatabaseId::from_existing("database-id".into());
        let source_columns = [
            DuckDbColumnMeta {
                name: "created_at".into(),
                dtype: "DateTime".into(),
            },
            DuckDbColumnMeta {
                name: "updated_at".into(),
                dtype: "Datetime(ms)".into(),
            },
            DuckDbColumnMeta {
                name: "category".into(),
                dtype: "Categorical".into(),
            },
        ];

        let schema = DatabaseSchemaFact::from_duckdb(&database, &source_columns)
            .expect("DuckDB metadata is valid");
        assert_eq!(schema.columns()[0].data_type(), &DataType::Datetime);
        assert_eq!(schema.columns()[1].data_type(), &DataType::Datetime);
        assert_eq!(schema.columns()[2].data_type(), &DataType::Categorical);
    }

    #[test]
    fn dataframe_schema_preserves_temporal_types_nullability_and_revisions() {
        let date = Series::from_any_values(
            PlSmallStr::from("observed_date"),
            &[AnyValue::Date(1), AnyValue::Null],
            false,
        )
        .expect("test date series is valid");
        let datetime = Series::from_any_values(
            PlSmallStr::from("observed_at"),
            &[
                AnyValue::DatetimeOwned(1, TimeUnit::Milliseconds, None),
                AnyValue::DatetimeOwned(2, TimeUnit::Milliseconds, None),
            ],
            false,
        )
        .expect("test datetime series is valid");
        let dataframe = DataFrame::new(2, vec![Column::from(date), Column::from(datetime)])
            .expect("test dataframe is valid");
        let database = DatabaseId::from_existing("database-id".into());

        let schema = DatabaseSchemaFact::from_dataframe(&database, &dataframe)
            .expect("DataFrame metadata is valid")
            .with_revisions(7, 3);

        assert_eq!(schema.database(), &database);
        assert_eq!(schema.runtime_revision().get(), 7);
        assert_eq!(schema.schema_revision().get(), 3);
        assert_eq!(schema.columns()[0].data_type(), &DataType::Date);
        assert!(schema.columns()[0].nullable());
        assert_eq!(schema.columns()[1].data_type(), &DataType::Datetime);
        assert!(!schema.columns()[1].nullable());
    }

    #[test]
    fn invalid_physical_column_names_fail_closed() {
        let database = DatabaseId::from_existing("database-id".into());
        let error = DatabaseSchemaFact::from_duckdb(
            &database,
            &[DuckDbColumnMeta {
                name: "".into(),
                dtype: "String".into(),
            }],
        )
        .expect_err("invalid column names must not enter schema facts");
        assert_eq!(error, DatabaseSchemaFactError::InvalidColumnName);
    }
}
