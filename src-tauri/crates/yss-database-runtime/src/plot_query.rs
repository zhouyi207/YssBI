use std::sync::Arc;

use arrow::array::Float64Array;
use arrow::datatypes::{DataType as ArrowDataType, TimeUnit};
use thiserror::Error;

use crate::error::{DatabaseError, DatabaseErrorCode, DatabaseOperation};
use crate::runtime::DatabaseRuntimeSession;
use crate::session_api::{self, DatabaseQueryBasis};
use yss_data_contract::DataType;
use yss_database_contract::DatabaseId;
use yss_tabular_contract::TabularColumnName;

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
    source: Option<Box<DatabaseError>>,
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
    let _admission = session
        .admit_operation(DatabaseOperation::Query)
        .map_err(|error| map_database_error(error, database, None, ErrorContext::Read))?;
    let instance = session
        .physical_instance(database)
        .map_err(|error| map_database_error(error, database, None, ErrorContext::Read))?;
    let facts = instance.data_schema().map_err(|error| {
        materialization_error(
            database,
            None,
            DatabaseError::dataset(DatabaseOperation::Query, Some(database.clone()), error),
        )
    })?;
    let kind = |name: &TabularColumnName| {
        facts
            .columns()
            .iter()
            .find(|column| column.name() == name)
            .map(|field| numeric_kind(field.data_type()))
            .ok_or_else(|| {
                materialization_error(
                    database,
                    Some(name),
                    DatabaseError::not_found(DatabaseOperation::Query, Some(database.clone())),
                )
            })
    };
    let x_kind = kind(x_column)?;
    let y_kind = kind(y_column)?;
    let batches = instance
        .read_arrow_columns(
            &[x_column.as_str(), y_column.as_str()],
            0,
            usize::MAX,
            &crate::database_instance::query_control(128 * 1024 * 1024),
        )
        .map_err(|error| {
            materialization_error(
                database,
                None,
                DatabaseError::dataset(DatabaseOperation::Query, Some(database.clone()), error),
            )
        })?;
    let mut x = Vec::new();
    let mut y = Vec::new();
    for batch in batches {
        for (index, output) in [(0, &mut x), (1, &mut y)] {
            let column = batch.column(index);
            let physical = match column.data_type() {
                ArrowDataType::Timestamp(_, timezone) => arrow::compute::cast(
                    column.as_ref(),
                    &ArrowDataType::Timestamp(TimeUnit::Microsecond, timezone.clone()),
                )
                .and_then(|array| arrow::compute::cast(array.as_ref(), &ArrowDataType::Int64)),
                data_type if data_type.is_temporal() => {
                    arrow::compute::cast(column.as_ref(), &ArrowDataType::Int64)
                }
                _ => Ok(column.clone()),
            }
            .and_then(|array| arrow::compute::cast(array.as_ref(), &ArrowDataType::Float64))
            .map_err(|error| {
                materialization_error(
                    database,
                    None,
                    DatabaseError::dataset(
                        DatabaseOperation::Query,
                        Some(database.clone()),
                        error.into(),
                    ),
                )
            })?;
            let values = physical
                .as_any()
                .downcast_ref::<Float64Array>()
                .ok_or_else(|| {
                    materialization_error(
                        database,
                        None,
                        DatabaseError::schema(DatabaseOperation::Query, Some(database.clone())),
                    )
                })?;
            let scale = if matches!(column.data_type(), ArrowDataType::Date64) {
                86_400_000.0
            } else {
                1.0
            };
            output.extend(values.iter().map(|value| value.map(|value| value / scale)));
        }
    }
    if x.is_empty() {
        return Err(materialization_error(
            database,
            None,
            DatabaseError::invalid_request(DatabaseOperation::Query, Some(database.clone())),
        ));
    }
    session_api::revalidate_query_basis(session, &basis)
        .map_err(|error| map_database_error(error, database, None, ErrorContext::Revalidate))?;
    Ok(NumericColumnPair {
        basis: PlotQueryBasis {
            query: basis,
            database: database.clone(),
        },
        x: x.into(),
        y: y.into(),
        x_label: Some(x_column.as_str().into()),
        y_label: Some(y_column.as_str().into()),
        x_kind,
        y_kind,
    })
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
        source: Some(Box::new(error)),
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
        source: Some(Box::new(source)),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::DatabaseRuntimeRegistry;
    use crate::test_support::{DatasetFixture, SALES_ID};
    use arrow::array::{Date32Array, TimestampMicrosecondArray};
    use arrow::datatypes::{Field, Schema};
    use arrow::record_batch::RecordBatch;
    use std::num::NonZeroU64;
    use yss_database_contract::{
        DatabaseDeclarationFingerprint, DatabaseDeclarationObservation,
        DatabaseDeclarationObservationSet, DatabaseDeclarationRevision, DatabaseSessionIdentity,
        DatabaseSessionOpenRequest,
    };
    fn session_with_temporal_data(identity: &str) -> (DatasetFixture, DatabaseRuntimeSession) {
        let schema = Arc::new(Schema::new(vec![
            Field::new("observed_date", ArrowDataType::Date32, true),
            Field::new(
                "observed_at",
                ArrowDataType::Timestamp(TimeUnit::Microsecond, None),
                true,
            ),
        ]));
        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(Date32Array::from(vec![1, 2])),
                Arc::new(TimestampMicrosecondArray::from(vec![1000, 2000])),
            ],
        )
        .unwrap();
        let fixture = DatasetFixture::new(SALES_ID, batch);
        let declaration = fixture.instance.decl.clone();
        let observations = DatabaseDeclarationObservationSet::try_from_iter([(
            declaration.id.clone(),
            DatabaseDeclarationObservation::new(
                DatabaseDeclarationRevision::from_existing(1),
                DatabaseDeclarationFingerprint::from_decl(&declaration),
            ),
        )])
        .expect("test declaration observations are valid");
        let session = DatabaseRuntimeRegistry::new()
            .open_session_with_instances(
                DatabaseSessionOpenRequest::new(
                    DatabaseSessionIdentity::from_existing(identity.into()),
                    NonZeroU64::new(1).expect("test generation is non-zero"),
                    None,
                    vec![declaration].into(),
                    observations,
                ),
                [fixture.instance.clone()],
            )
            .expect("test database session is valid");
        (fixture, session)
    }

    #[test]
    fn runtime_materializer_preserves_plot_day_and_microsecond_encodings() {
        let (_fixture, session) = session_with_temporal_data("plot-session");
        let database = DatabaseId::from_existing(SALES_ID.into());
        let date_column =
            TabularColumnName::try_from("observed_date").expect("test column name is valid");
        let datetime_column =
            TabularColumnName::try_from("observed_at").expect("test column name is valid");
        let pair = read_numeric_column_pair(&session, &database, &date_column, &datetime_column)
            .expect("Arrow temporal columns materialize");
        assert_eq!(pair.x_label(), Some("observed_date"));
        assert_eq!(pair.y_label(), Some("observed_at"));
        assert_eq!(pair.x_kind(), NumericColumnKind::Date);
        assert_eq!(pair.y_kind(), NumericColumnKind::Datetime);
        assert_eq!(pair.x(), &[Some(1.0), Some(2.0)]);
        assert_eq!(pair.y(), &[Some(1_000.0), Some(2_000.0)]);
    }
}
