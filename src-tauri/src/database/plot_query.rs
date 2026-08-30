use std::sync::Arc;

use polars::prelude::{AnyValue, DataType as PolarsDataType, PlSmallStr, PolarsResult, Series};
use thiserror::Error;

use crate::database::error::{
    DatabaseDriverError, DatabaseError, DatabaseErrorCode, DatabaseOperation,
};
use crate::database::runtime::DatabaseRuntimeSession;
use crate::database::schema_snapshot::DatabaseColumnFact;
use crate::database::session_api::{
    self, DatabaseColumnSelection, DatabaseDataSnapshotRequest, DatabaseQueryBasis,
};
use crate::tabular::contract::{TabularColumn, TabularColumnName, TabularScalar};
use yss_data_contract::DataType;
use yss_database_contract::DatabaseId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NumericColumnKind {
    Number,
    Date,
    Datetime,
}

struct PlotQueryBasis {
    query: DatabaseQueryBasis,
    database: DatabaseId,
}

pub struct NumericColumnPair {
    basis: PlotQueryBasis,
    x: Arc<[Option<f64>]>,
    y: Arc<[Option<f64>]>,
    x_label: Option<Box<str>>,
    y_label: Option<Box<str>>,
    x_kind: NumericColumnKind,
    y_kind: NumericColumnKind,
}

impl NumericColumnPair {
    pub fn x(&self) -> &[Option<f64>] {
        &self.x
    }

    pub fn y(&self) -> &[Option<f64>] {
        &self.y
    }

    pub fn x_label(&self) -> Option<&str> {
        self.x_label.as_deref()
    }

    pub fn y_label(&self) -> Option<&str> {
        self.y_label.as_deref()
    }

    pub fn x_kind(&self) -> NumericColumnKind {
        self.x_kind
    }

    pub fn y_kind(&self) -> NumericColumnKind {
        self.y_kind
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DatabasePlotQueryErrorKind {
    AdmissionClosed,
    SessionMismatch,
    GenerationMismatch,
    DatabaseNotFound,
    RuntimeRevisionMismatch,
    SchemaRevisionMismatch,
    ColumnMaterializationFailed,
}

#[derive(Debug, Error)]
#[error("database plot query failed")]
pub struct DatabasePlotQueryError {
    kind: DatabasePlotQueryErrorKind,
    database: DatabaseId,
    column: Option<TabularColumnName>,
    #[source]
    source: Option<DatabaseError>,
}

impl DatabasePlotQueryError {
    pub fn kind(&self) -> DatabasePlotQueryErrorKind {
        self.kind
    }

    pub fn database(&self) -> &DatabaseId {
        &self.database
    }

    pub fn column(&self) -> Option<&TabularColumnName> {
        self.column.as_ref()
    }
}

pub fn read_numeric_column_pair(
    session: &DatabaseRuntimeSession,
    database: &DatabaseId,
    x_column: &TabularColumnName,
    y_column: &TabularColumnName,
) -> Result<NumericColumnPair, DatabasePlotQueryError> {
    let basis = session
        .capture_query_basis(database)
        .map_err(|error| map_database_error(error, database, None, ErrorContext::Capture))?;
    let snapshot = session_api::data_snapshot(
        session,
        DatabaseDataSnapshotRequest {
            database: database.clone(),
            columns: DatabaseColumnSelection::Selected([x_column.clone(), y_column.clone()].into()),
            offset: 0,
            limit: usize::MAX,
        },
    )
    .map_err(|error| map_database_error(error, database, None, ErrorContext::Read))?;

    if snapshot.rows().row_count() == 0 {
        return Err(materialization_error(
            database,
            None,
            DatabaseError::invalid_request(DatabaseOperation::Query, Some(database.clone())),
        ));
    }

    let x_fact = snapshot
        .columns()
        .iter()
        .find(|column| column.name() == x_column)
        .ok_or_else(|| {
            materialization_error(
                database,
                Some(x_column),
                DatabaseError::not_found(DatabaseOperation::Query, Some(database.clone())),
            )
        })?;
    let y_fact = snapshot
        .columns()
        .iter()
        .find(|column| column.name() == y_column)
        .ok_or_else(|| {
            materialization_error(
                database,
                Some(y_column),
                DatabaseError::not_found(DatabaseOperation::Query, Some(database.clone())),
            )
        })?;
    let x_source = snapshot
        .rows()
        .columns()
        .iter()
        .find(|column| column.name() == x_column)
        .ok_or_else(|| {
            materialization_error(
                database,
                Some(x_column),
                DatabaseError::not_found(DatabaseOperation::Query, Some(database.clone())),
            )
        })?;
    let y_source = snapshot
        .rows()
        .columns()
        .iter()
        .find(|column| column.name() == y_column)
        .ok_or_else(|| {
            materialization_error(
                database,
                Some(y_column),
                DatabaseError::not_found(DatabaseOperation::Query, Some(database.clone())),
            )
        })?;

    let x_series = tabular_column_to_series(x_source).map_err(|error| {
        materialization_error(
            database,
            Some(x_column),
            DatabaseError::driver(
                DatabaseOperation::Query,
                Some(database.clone()),
                DatabaseDriverError::Polars(error),
            ),
        )
    })?;
    let y_series = tabular_column_to_series(y_source).map_err(|error| {
        materialization_error(
            database,
            Some(y_column),
            DatabaseError::driver(
                DatabaseOperation::Query,
                Some(database.clone()),
                DatabaseDriverError::Polars(error),
            ),
        )
    })?;

    pair_from_fact_series(
        basis, database, x_fact, y_fact, &x_series, &y_series, x_column, y_column,
    )
}

pub fn revalidate_numeric_column_pair(
    session: &DatabaseRuntimeSession,
    pair: &NumericColumnPair,
) -> Result<(), DatabasePlotQueryError> {
    session_api::revalidate_query_basis(session, &pair.basis.query).map_err(|error| {
        map_database_error(error, &pair.basis.database, None, ErrorContext::Revalidate)
    })
}

#[derive(Clone, Copy)]
enum ErrorContext {
    Capture,
    Read,
    Revalidate,
}

fn map_database_error(
    error: DatabaseError,
    database: &DatabaseId,
    column: Option<&TabularColumnName>,
    context: ErrorContext,
) -> DatabasePlotQueryError {
    let kind = match error.code() {
        DatabaseErrorCode::AdmissionClosed => DatabasePlotQueryErrorKind::AdmissionClosed,
        DatabaseErrorCode::Conflict => match context {
            ErrorContext::Revalidate => DatabasePlotQueryErrorKind::SessionMismatch,
            ErrorContext::Capture | ErrorContext::Read => {
                DatabasePlotQueryErrorKind::RuntimeRevisionMismatch
            }
        },
        DatabaseErrorCode::NotFound => DatabasePlotQueryErrorKind::DatabaseNotFound,
        DatabaseErrorCode::Schema => DatabasePlotQueryErrorKind::SchemaRevisionMismatch,
        DatabaseErrorCode::InvalidRequest
        | DatabaseErrorCode::Constraint
        | DatabaseErrorCode::Unsupported
        | DatabaseErrorCode::Driver
        | DatabaseErrorCode::Cancelled
        | DatabaseErrorCode::Deadline => DatabasePlotQueryErrorKind::ColumnMaterializationFailed,
    };
    DatabasePlotQueryError {
        kind,
        database: database.clone(),
        column: column.cloned(),
        source: Some(error),
    }
}

fn materialization_error(
    database: &DatabaseId,
    column: Option<&TabularColumnName>,
    source: DatabaseError,
) -> DatabasePlotQueryError {
    DatabasePlotQueryError {
        kind: DatabasePlotQueryErrorKind::ColumnMaterializationFailed,
        database: database.clone(),
        column: column.cloned(),
        source: Some(source),
    }
}

fn pair_from_fact_series(
    basis: DatabaseQueryBasis,
    database: &DatabaseId,
    x_fact: &DatabaseColumnFact,
    y_fact: &DatabaseColumnFact,
    x_series: &Series,
    y_series: &Series,
    x_column: &TabularColumnName,
    y_column: &TabularColumnName,
) -> Result<NumericColumnPair, DatabasePlotQueryError> {
    let x_kind = numeric_kind(x_fact.data_type());
    let y_kind = numeric_kind(y_fact.data_type());
    let x = series_to_numeric_values(x_series, x_kind).map_err(|error| {
        materialization_error(
            database,
            Some(x_column),
            DatabaseError::driver(
                DatabaseOperation::Query,
                Some(database.clone()),
                DatabaseDriverError::Polars(error),
            ),
        )
    })?;
    let y = series_to_numeric_values(y_series, y_kind).map_err(|error| {
        materialization_error(
            database,
            Some(y_column),
            DatabaseError::driver(
                DatabaseOperation::Query,
                Some(database.clone()),
                DatabaseDriverError::Polars(error),
            ),
        )
    })?;
    Ok(NumericColumnPair {
        basis: PlotQueryBasis {
            query: basis,
            database: database.clone(),
        },
        x,
        y,
        x_label: label_for_series(x_series),
        y_label: label_for_series(y_series),
        x_kind,
        y_kind,
    })
}

fn series_to_numeric_values(
    series: &Series,
    kind: NumericColumnKind,
) -> PolarsResult<Arc<[Option<f64>]>> {
    let casted = match kind {
        NumericColumnKind::Date => series
            .cast(&PolarsDataType::Int32)?
            .cast(&PolarsDataType::Float64)?,
        NumericColumnKind::Datetime => series
            .cast(&PolarsDataType::Int64)?
            .cast(&PolarsDataType::Float64)?,
        NumericColumnKind::Number if matches!(series.dtype(), PolarsDataType::Time) => series
            .cast(&PolarsDataType::Int64)?
            .cast(&PolarsDataType::Float64)?,
        NumericColumnKind::Number => series.cast(&PolarsDataType::Float64)?,
    };
    let values = casted.f64()?.into_iter().collect::<Vec<_>>();
    Ok(Arc::from(values.into_boxed_slice()))
}

fn tabular_column_to_series(column: &TabularColumn) -> PolarsResult<Series> {
    let values = column
        .values()
        .iter()
        .map(tabular_scalar_to_any_value)
        .collect::<Vec<_>>();
    Series::from_any_values(PlSmallStr::from(column.name().as_str()), &values, false)
}

fn tabular_scalar_to_any_value(value: &TabularScalar) -> AnyValue<'static> {
    match value {
        TabularScalar::Null => AnyValue::Null,
        TabularScalar::Bool(value) => AnyValue::Boolean(*value),
        TabularScalar::Integer(value) => AnyValue::Int64(*value),
        TabularScalar::Unsigned(value) => AnyValue::UInt64(*value),
        TabularScalar::Decimal(value) => AnyValue::Float64(value.as_f64()),
        TabularScalar::String(value) => AnyValue::StringOwned(value.to_string().into()),
    }
}

fn numeric_kind(data_type: &DataType) -> NumericColumnKind {
    match data_type {
        DataType::Date => NumericColumnKind::Date,
        DataType::Datetime => NumericColumnKind::Datetime,
        DataType::Boolean
        | DataType::Int64
        | DataType::Float64
        | DataType::String
        | DataType::Time
        | DataType::Categorical
        | DataType::Array(_)
        | DataType::Object
        | DataType::DataFrame
        | DataType::DataSeries(_)
        | DataType::Struct(_)
        | DataType::OneOf(_)
        | DataType::Any => NumericColumnKind::Number,
    }
}

fn label_for_series(series: &Series) -> Option<Box<str>> {
    (!series.name().is_empty()).then(|| series.name().as_str().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::runtime::DatabaseRuntimeRegistry;
    use crate::database::{DatabaseInstance, DatabaseState, EditHistory};
    use polars::prelude::{AnyValue, Column, DataFrame, TimeUnit};
    use std::num::NonZeroU64;
    use std::sync::Arc;
    use yss_database_contract::{
        DatabaseDecl, DatabaseDeclarationFingerprint, DatabaseDeclarationObservation,
        DatabaseDeclarationObservationSet, DatabaseDeclarationRevision, DatabaseEngine,
        DatabaseSessionIdentity, DatabaseSessionOpenRequest,
    };

    fn session_with_loaded_data(identity: &str) -> DatabaseRuntimeSession {
        let declaration = DatabaseDecl {
            id: DatabaseId::from_existing("sales".into()),
            engine: DatabaseEngine::InMemory {
                name: "sales".into(),
            },
            schema_version: 1,
            required: false,
            name: "Sales".into(),
        };
        let date = Series::from_any_values(
            PlSmallStr::from("observed_date"),
            &[AnyValue::Date(1), AnyValue::Date(2)],
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
        let dataframe = Arc::new(
            DataFrame::new(2, vec![Column::from(date), Column::from(datetime)])
                .expect("test dataframe is valid"),
        );
        let instance = DatabaseInstance {
            decl: declaration.clone(),
            state: DatabaseState::Loaded {
                dataframe: dataframe.clone(),
                original: dataframe,
                history: EditHistory::new(),
            },
        };
        let observations = DatabaseDeclarationObservationSet::try_from_iter([(
            declaration.id.clone(),
            DatabaseDeclarationObservation::new(
                DatabaseDeclarationRevision::from_existing(1),
                DatabaseDeclarationFingerprint::from_decl(&declaration),
            ),
        )])
        .expect("test declaration observations are valid");
        DatabaseRuntimeRegistry::new()
            .open_session_with_instances(
                DatabaseSessionOpenRequest::new(
                    DatabaseSessionIdentity::from_existing(identity.into()),
                    NonZeroU64::new(1).expect("test generation is non-zero"),
                    None,
                    vec![declaration].into(),
                    observations,
                ),
                [instance],
            )
            .expect("test database session is valid")
    }

    #[test]
    fn loaded_runtime_materializer_casts_temporal_columns_for_plot_reads() {
        let session = session_with_loaded_data("plot-session");
        let database = DatabaseId::from_existing("sales".into());
        let date_column =
            TabularColumnName::try_from("observed_date").expect("test column name is valid");
        let datetime_column =
            TabularColumnName::try_from("observed_at").expect("test column name is valid");
        let pair = read_numeric_column_pair(&session, &database, &date_column, &datetime_column)
            .expect("loaded temporal columns materialize");
        assert_eq!(pair.x_label(), Some("observed_date"));
        assert_eq!(pair.y_label(), Some("observed_at"));
        assert_eq!(pair.x_kind(), NumericColumnKind::Date);
        assert_eq!(pair.y_kind(), NumericColumnKind::Datetime);
        assert_eq!(pair.x(), &[Some(1.0), Some(2.0)]);
        assert_eq!(pair.y(), &[Some(1.0), Some(2.0)]);
    }
}
