//! Backend-neutral runtime database schema facts and revision projections.
//!
//! This crate owns the typed schema/revision projection shared by database sessions, Graph
//! contracts, and transport adapters. Physical adapters map their exact storage schemas into
//! this semantic vocabulary; these facts are never used to reconstruct storage types.

use yss_data_contract::TabularColumnName;
use yss_data_contract::{ColumnSemantic, ValueType};
use yss_database_contract::DatabaseId;

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
    data_type: ValueType,
    nullable: bool,
    display_type: Box<str>,
    semantic: Option<ColumnSemantic>,
    physical_type: Option<Box<str>>,
}

impl DatabaseColumnFact {
    pub fn new(name: TabularColumnName, data_type: ValueType, nullable: bool) -> Self {
        Self {
            name,
            display_type: data_type.to_string().into(),
            data_type,
            nullable,
            semantic: None,
            physical_type: None,
        }
    }

    pub fn name(&self) -> &TabularColumnName {
        &self.name
    }

    pub fn data_type(&self) -> &ValueType {
        &self.data_type
    }

    pub fn with_display_type(mut self, display_type: impl Into<Box<str>>) -> Self {
        self.display_type = display_type.into();
        self
    }

    pub fn display_type(&self) -> &str {
        &self.display_type
    }

    pub fn with_semantic(mut self, semantic: ColumnSemantic) -> Self {
        self.semantic = Some(semantic);
        self
    }

    pub fn semantic(&self) -> Option<&ColumnSemantic> {
        self.semantic.as_ref()
    }

    pub fn with_physical_type(mut self, physical_type: String) -> Self {
        self.physical_type = Some(physical_type.into());
        self
    }

    pub fn physical_type(&self) -> &str {
        self.physical_type.as_deref().unwrap_or(&self.display_type)
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
