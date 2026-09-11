//! Live, snapshot-bound relation ports. Persisted Graph documents never contain these handles.

mod dataset;
pub use dataset::{DatasetColumnPatch, DatasetOverlay, DatasetRelationInput};
mod series;
pub use series::{NumericOperation, NumericType, SeriesHandle, SeriesOperand, SeriesPlan};

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

use arrow_array::RecordBatch;
use arrow_schema::SchemaRef;
use futures_core::Stream;
use yss_database_contract::DatabaseId;

pub type RelationFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, RelationError>> + Send + 'a>>;
pub type RelationBatchStream =
    Pin<Box<dyn Stream<Item = Result<RecordBatch, RelationError>> + Send>>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationBinding {
    pub project_session: Box<str>,
    pub dataset: DatabaseId,
    pub snapshot: Box<str>,
    pub revision: u64,
}

#[derive(Debug, Clone)]
pub struct RelationControl {
    pub cancellation: Arc<AtomicBool>,
    pub deadline: Instant,
    pub max_input_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationColumn {
    pub name: Box<str>,
    pub data_type: Box<str>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RelationPage {
    pub data: yss_tabular_contract::TabularSnapshot,
    pub row_count: usize,
    pub columns: Box<[RelationColumn]>,
    pub has_more: bool,
}

impl RelationControl {
    pub fn check(&self) -> Result<(), RelationError> {
        if self.cancellation.load(Ordering::Acquire) {
            return Err(RelationError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(RelationError::DeadlineExceeded);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RelationError {
    #[error("relation plan is invalid")]
    InvalidPlan,
    #[error("relation input is invalid")]
    InvalidInput,
    #[error("series do not share the same relation and row alignment")]
    UnalignedSeries,
    #[error("relation source is unavailable")]
    SourceUnavailable,
    #[error("relation query failed")]
    QueryFailed,
    #[error("relation query was cancelled")]
    Cancelled,
    #[error("relation query deadline was exceeded")]
    DeadlineExceeded,
    #[error("statistical input memory budget was exceeded")]
    MemoryLimitExceeded,
    #[error("division by zero")]
    DivisionByZero,
    #[error("numeric result is not finite or representable")]
    NonFiniteResult,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RelationLiteral {
    Boolean(bool),
    Integer(i64),
    Decimal(Box<str>),
    String(Box<str>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationComparison {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    IsNull,
    IsNotNull,
}

/// An operation request, not a second plan/optimizer representation.
#[derive(Debug, Clone, PartialEq)]
pub struct RelationPredicate {
    pub column: Box<str>,
    pub comparison: RelationComparison,
    pub value: Option<RelationLiteral>,
}

pub trait RelationPlan: Send + Sync {
    fn binding(&self) -> &RelationBinding;
    fn schema(&self) -> SchemaRef;
    fn project(&self, columns: &[Box<str>]) -> Result<RelationHandle, RelationError>;
    fn filter(&self, predicate: &RelationPredicate) -> Result<RelationHandle, RelationError>;
    fn limit(&self, offset: usize, limit: usize) -> Result<RelationHandle, RelationError>;
    fn rename(&self, old: &str, new: &str) -> Result<RelationHandle, RelationError>;
    fn select_series(&self, column: &str) -> Result<Arc<dyn SeriesPlan>, RelationError>;
    fn project_series(&self, series: &[SeriesHandle]) -> Result<RelationHandle, RelationError>;
    fn numeric_series(
        &self,
        operation: NumericOperation,
        operands: &[SeriesOperand],
        output_type: NumericType,
    ) -> Result<Arc<dyn SeriesPlan>, RelationError>;
    fn stream(&self, control: RelationControl) -> RelationFuture<'_, RelationBatchStream>;
}

#[derive(Clone)]
pub struct RelationHandle {
    plan: Arc<dyn RelationPlan>,
    executor: Arc<dyn RelationExecutor>,
}

impl RelationHandle {
    pub fn new(plan: Arc<dyn RelationPlan>, executor: Arc<dyn RelationExecutor>) -> Self {
        Self { plan, executor }
    }
    pub fn binding(&self) -> &RelationBinding {
        self.plan.binding()
    }
    pub fn schema(&self) -> SchemaRef {
        self.plan.schema()
    }
    pub fn project(&self, columns: &[Box<str>]) -> Result<Self, RelationError> {
        self.plan.project(columns)
    }
    pub fn filter(&self, predicate: &RelationPredicate) -> Result<Self, RelationError> {
        self.plan.filter(predicate)
    }
    pub fn limit(&self, offset: usize, limit: usize) -> Result<Self, RelationError> {
        self.plan.limit(offset, limit)
    }
    pub fn rename(&self, old: &str, new: &str) -> Result<Self, RelationError> {
        self.plan.rename(old, new)
    }
    pub fn stream(&self, control: RelationControl) -> RelationFuture<'_, RelationBatchStream> {
        self.plan.stream(control)
    }
    pub fn numeric_columns(
        &self,
        series: &[SeriesHandle],
        control: &RelationControl,
    ) -> Result<Vec<Vec<f64>>, RelationError> {
        if series.iter().any(|series| series.relation() != self) {
            return Err(RelationError::UnalignedSeries);
        }
        self.executor.numeric_columns(series, control)
    }
    pub fn select_series(&self, column: &str) -> Result<SeriesHandle, RelationError> {
        self.schema()
            .index_of(column)
            .map_err(|_| RelationError::InvalidInput)?;
        Ok(SeriesHandle::new(
            self.clone(),
            self.plan.select_series(column)?,
        ))
    }

    pub fn project_series(&self, series: &[SeriesHandle]) -> Result<Self, RelationError> {
        if series.is_empty() {
            return Err(RelationError::InvalidInput);
        }
        if series.iter().any(|series| series.relation() != self) {
            return Err(RelationError::UnalignedSeries);
        }
        self.plan.project_series(series)
    }

    pub fn numeric_series(
        &self,
        operation: NumericOperation,
        operands: &[SeriesOperand],
        output_type: NumericType,
    ) -> Result<SeriesHandle, RelationError> {
        if !operands
            .iter()
            .any(|value| matches!(value, SeriesOperand::Series(_)))
        {
            return Err(RelationError::InvalidInput);
        }
        if operands.iter().any(
            |value| matches!(value, SeriesOperand::Series(series) if series.relation() != self),
        ) {
            return Err(RelationError::UnalignedSeries);
        }
        Ok(SeriesHandle::new(
            self.clone(),
            self.plan.numeric_series(operation, operands, output_type)?,
        ))
    }

    pub fn page(
        &self,
        offset: usize,
        limit: usize,
        control: &RelationControl,
    ) -> Result<RelationPage, RelationError> {
        self.executor.page(self, offset, limit, control)
    }
}

impl PartialEq for RelationHandle {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.plan, &other.plan)
    }
}

impl fmt::Debug for RelationHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RelationHandle")
            .field("binding", self.binding())
            .finish_non_exhaustive()
    }
}

/// Synchronous callers run on compute workers. The adapter consumes the asynchronous Arrow
/// stream in one joint projection and enforces the separately supplied statistics memory budget.
pub trait RelationExecutor: Send + Sync {
    fn page(
        &self,
        relation: &RelationHandle,
        offset: usize,
        limit: usize,
        control: &RelationControl,
    ) -> Result<RelationPage, RelationError>;
    fn numeric_columns(
        &self,
        series: &[SeriesHandle],
        control: &RelationControl,
    ) -> Result<Vec<Vec<f64>>, RelationError>;
}
