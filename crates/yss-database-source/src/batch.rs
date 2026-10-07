use arrow::array::*;
use arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use arrow::record_batch::RecordBatch;
use sqlx::{Column as SqlxColumn, Database, TypeInfo};
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum SqlSourceError {
    #[error("failed to initialize the SQL source runtime")]
    RuntimeInit(#[source] std::io::Error),
    #[error("failed to start the SQL source runtime worker")]
    RuntimeThread(#[source] std::io::Error),
    #[error("the SQL source runtime worker panicked")]
    RuntimePanicked,
    #[error("invalid {engine} connection settings")]
    InvalidConnection {
        engine: &'static str,
        #[source]
        source: sqlx::Error,
    },
    #[error("failed to connect to {engine}")]
    Connect {
        engine: &'static str,
        #[source]
        source: sqlx::Error,
    },
    #[error("failed to {operation} from {engine}")]
    Query {
        engine: &'static str,
        operation: &'static str,
        #[source]
        source: sqlx::Error,
    },
    #[error("failed to decode {engine} column '{column}' as {source_type}")]
    Decode {
        engine: &'static str,
        column: String,
        source_type: String,
        #[source]
        source: sqlx::Error,
    },
    #[error("{engine} column '{column}' uses unsupported type {source_type}")]
    UnsupportedColumnType {
        engine: &'static str,
        column: String,
        source_type: String,
    },
    #[error("the SQL source returned an inconsistent row shape")]
    InconsistentRowShape,
    #[error("failed to build the SQL source Arrow batch")]
    Batch(#[from] arrow::error::ArrowError),
    #[error("SQL source read was interrupted or exceeded its budget")]
    Control(#[from] yss_relational_contract::RelationError),
    #[error("SQL source reader was closed")]
    Closed,
}

impl SqlSourceError {
    pub(crate) fn invalid_connection(engine: &'static str, source: sqlx::Error) -> Self {
        Self::InvalidConnection { engine, source }
    }

    pub(crate) fn connect(engine: &'static str, source: sqlx::Error) -> Self {
        Self::Connect { engine, source }
    }

    pub(crate) fn query(
        engine: &'static str,
        operation: &'static str,
        source: sqlx::Error,
    ) -> Self {
        Self::Query {
            engine,
            operation,
            source,
        }
    }

    pub(crate) fn decode(engine: &'static str, column: &ColumnSpec, source: sqlx::Error) -> Self {
        Self::Decode {
            engine,
            column: column.name.clone(),
            source_type: column.source_type.clone(),
            source,
        }
    }

    pub(crate) fn unsupported(
        engine: &'static str,
        column: impl Into<String>,
        source_type: impl Into<String>,
    ) -> Self {
        Self::UnsupportedColumnType {
            engine,
            column: column.into(),
            source_type: source_type.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColumnKind {
    Boolean,
    Int8,
    Int16,
    Int32,
    Int64,
    UInt8,
    UInt16,
    UInt32,
    UInt64,
    Float32,
    Float64,
    String,
    Binary,
}

impl ColumnKind {
    fn arrow_type(self) -> DataType {
        match self {
            Self::Boolean => DataType::Boolean,
            Self::Int8 => DataType::Int8,
            Self::Int16 => DataType::Int16,
            Self::Int32 => DataType::Int32,
            Self::Int64 => DataType::Int64,
            Self::UInt8 => DataType::UInt8,
            Self::UInt16 => DataType::UInt16,
            Self::UInt32 => DataType::UInt32,
            Self::UInt64 => DataType::UInt64,
            Self::Float32 => DataType::Float32,
            Self::Float64 => DataType::Float64,
            Self::String => DataType::Utf8,
            Self::Binary => DataType::Binary,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ColumnSpec {
    pub(crate) name: String,
    pub(crate) source_type: String,
    pub(crate) kind: ColumnKind,
}

impl ColumnSpec {
    pub(crate) fn new(name: &str, source_type: &str, kind: ColumnKind) -> Self {
        Self {
            name: name.to_string(),
            source_type: source_type.to_string(),
            kind,
        }
    }
}

pub(crate) fn raw_column_metadata<DB: Database>(columns: &[DB::Column]) -> Vec<(String, String)>
where
    DB::Column: SqlxColumn<Database = DB>,
    DB::TypeInfo: TypeInfo,
{
    columns
        .iter()
        .map(|column| {
            (
                column.name().to_string(),
                column.type_info().name().to_string(),
            )
        })
        .collect()
}

pub(crate) struct BatchBuilder {
    pub columns: Vec<ColumnSpec>,
    pub schema: SchemaRef,
    builders: Vec<Box<dyn ArrayBuilder>>,
    pub rows: usize,
    pub bytes: usize,
}

impl BatchBuilder {
    pub fn new(columns: Vec<ColumnSpec>) -> Self {
        let schema = Arc::new(Schema::new(
            columns
                .iter()
                .map(|column| Field::new(&column.name, column.kind.arrow_type(), true))
                .collect::<Vec<_>>(),
        ));
        let builders = columns
            .iter()
            .map(|column| make_builder(&column.kind.arrow_type(), 1024))
            .collect();
        Self {
            columns,
            schema,
            builders,
            rows: 0,
            bytes: 0,
        }
    }
    pub fn append_row<R: sqlx::Row>(
        &mut self,
        row: &R,
        decode: impl Fn(&R, usize, &ColumnSpec, &mut dyn ArrayBuilder) -> Result<usize, SqlSourceError>,
    ) -> Result<(), SqlSourceError> {
        if row.len() != self.columns.len() {
            return Err(SqlSourceError::InconsistentRowShape);
        }
        for (index, (column, builder)) in self.columns.iter().zip(&mut self.builders).enumerate() {
            self.bytes = self
                .bytes
                .checked_add(decode(row, index, column, builder.as_mut())?)
                .ok_or(yss_relational_contract::RelationError::MemoryLimitExceeded)?;
        }
        self.rows += 1;
        Ok(())
    }
    pub fn finish(&mut self) -> Result<RecordBatch, SqlSourceError> {
        let arrays = self
            .builders
            .iter_mut()
            .map(|builder| builder.finish())
            .collect();
        self.rows = 0;
        self.bytes = 0;
        RecordBatch::try_new(self.schema.clone(), arrays).map_err(Into::into)
    }
}

macro_rules! append_value {
    ($builder:expr, $kind:ty, $decoded:expr, $null:expr, $size:expr) => {{
        let decoded = if $null { None } else { Some($decoded?) };
        let size = decoded.as_ref().map_or(1, $size);
        $builder
            .as_any_mut()
            .downcast_mut::<$kind>()
            .ok_or(SqlSourceError::InconsistentRowShape)?
            .append_option(decoded);
        Ok(size)
    }};
}
