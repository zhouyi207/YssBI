use polars::prelude::{AnyValue, DataFrame, DataType, PlSmallStr, Series};
use sqlx::{Column as SqlxColumn, Database, TypeInfo};

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
    #[error("failed to build the SQL source DataFrame")]
    DataFrame(#[source] polars::error::PolarsError),
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
    fn polars_dtype(self) -> DataType {
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
            Self::String => DataType::String,
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

pub(crate) fn empty_column_data(column_count: usize) -> Vec<Vec<AnyValue<'static>>> {
    (0..column_count).map(|_| Vec::new()).collect()
}

pub(crate) fn build_dataframe(
    columns: &[ColumnSpec],
    data: Vec<Vec<AnyValue<'static>>>,
) -> Result<DataFrame, SqlSourceError> {
    if columns.len() != data.len() {
        return Err(SqlSourceError::InconsistentRowShape);
    }
    let height = data.first().map_or(0, Vec::len);
    if data.iter().any(|column| column.len() != height) {
        return Err(SqlSourceError::InconsistentRowShape);
    }

    let series = columns
        .iter()
        .zip(data)
        .map(|(column, values)| {
            let name = PlSmallStr::from_str(&column.name);
            let dtype = column.kind.polars_dtype();
            if values.iter().all(|value| matches!(value, AnyValue::Null)) {
                return Ok(Series::full_null(name, height, &dtype));
            }

            let series =
                Series::from_any_values(name, &values, true).map_err(SqlSourceError::DataFrame)?;
            if series.dtype() == &dtype {
                Ok(series)
            } else {
                series.cast(&dtype).map_err(SqlSourceError::DataFrame)
            }
        })
        .collect::<Result<Vec<_>, SqlSourceError>>()?;
    let frame_columns = series.into_iter().map(Into::into).collect();
    DataFrame::new(height, frame_columns).map_err(SqlSourceError::DataFrame)
}
