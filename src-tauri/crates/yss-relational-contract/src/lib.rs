//! Live, snapshot-bound relation ports. Persisted Graph documents never contain these handles.

mod dataset;
pub use dataset::{DatasetColumnPatch, DatasetOverlay, DatasetRelationInput};
mod series;
mod transform;
pub use series::{
    BooleanOperand, BooleanOperation, ComparisonOperand, NumericOperation, NumericTolerance,
    NumericType, SeriesHandle, SeriesOperand, SeriesPlan,
};
pub use transform::*;
pub use yss_data_contract::ComparisonOperation;

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
    pub data: yss_data_contract::TabularSnapshot,
    pub row_count: usize,
    pub columns: Box<[RelationColumn]>,
    pub has_more: bool,
}

/// Import immutable, already materialized columns into the caller's shared query engine.
/// Literal relations have no dataset bindings and never authorize external resource access.
/// Materialized Arrow fields carry resolved semantic metadata and physical representations.
pub trait RelationFactory: Send + Sync {
    /// Literal vectors use their explicit zero-based positions, independently of dataset rows.
    fn literal_series(
        self: Arc<Self>,
        _field: arrow_schema::Field,
        _values: Arc<dyn arrow_array::Array>,
        _control: &RelationControl,
    ) -> Result<SeriesHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn integer_range(
        self: Arc<Self>,
        _start: i64,
        _end: i64,
        _step: i64,
        _control: &RelationControl,
    ) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn materialize(
        self: Arc<Self>,
        data: arrow_array::RecordBatch,
        control: &RelationControl,
    ) -> Result<RelationHandle, RelationError>;
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
    #[error("semantic conversion is invalid or loses information")]
    InvalidConversion,
    #[error("series do not share the same relation and row alignment")]
    UnalignedSeries,
    #[error("positional series have different lengths")]
    ShapeMismatch,
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
    pub value: Option<yss_data_contract::FilterLiteral>,
}

/// Which Null pattern removes a row or column. NaN and empty text are ordinary values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropNaMode {
    Any,
    All,
}

pub trait RelationPlan: Send + Sync {
    /// Explicit literal/range coordinates only; dataset lengths never prove alignment.
    fn positional_length(&self) -> Option<usize> {
        None
    }
    fn shares_row_domain(&self, _other: &RelationHandle) -> bool {
        false
    }
    fn transform_series(
        &self,
        _series: &SeriesHandle,
        _operation: &SeriesTransform,
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn reduce_series(
        &self,
        _series: &SeriesHandle,
        _operation: SeriesReduction,
    ) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn choose_series(
        &self,
        _condition: &SeriesHandle,
        _when_true: &SeriesOperand,
        _when_false: &SeriesOperand,
        _output_field: Option<&arrow_schema::Field>,
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn concatenate_text(
        &self,
        _operands: &[SeriesOperand],
        _separator: &str,
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn sort_rows(&self, _columns: &[SortColumn]) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn deduplicate(
        &self,
        _keys: &[Box<str>],
        _keep: DuplicateKeep,
    ) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn set_column(
        &self,
        _name: &str,
        _series: &SeriesHandle,
    ) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn filter_mask(
        &self,
        _mask: &SeriesHandle,
        _drop_matches: bool,
    ) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn unpivot(&self, _spec: &UnpivotSpec) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn pivot(&self, _spec: &PivotSpec) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn resample(&self, _spec: &ResampleSpec) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn encode_series(
        &self,
        _series: &SeriesHandle,
        _levels: &[PivotLevel],
        _reference: Option<&str>,
    ) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn align_grid(
        &self,
        _time: &str,
        _entity: Option<&str>,
        _interval: i64,
        _max_bytes: usize,
    ) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn aggregate(
        &self,
        _keys: &[Box<str>],
        _columns: &[yss_data_contract::aggregation::ColumnAggregate],
    ) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn describe(&self, _columns: &[Box<str>]) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn frequency(
        &self,
        _column: &str,
        _include_null: bool,
    ) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn compare_series_with_tolerance(
        &self,
        _operation: ComparisonOperation,
        _operands: &[ComparisonOperand],
        _tolerance: NumericTolerance,
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        Err(RelationError::InvalidInput)
    }
    /// Deferred schemas are unavailable until consumption; `schema()` is then an empty marker.
    fn schema_is_deferred(&self) -> bool {
        false
    }
    fn resolve(&self, _control: RelationControl) -> RelationFuture<'_, Option<RelationHandle>> {
        Box::pin(async { Ok(None) })
    }
    fn drop_na_rows(
        &self,
        _columns: &[Box<str>],
        _mode: DropNaMode,
    ) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn drop_na_columns(
        &self,
        _columns: &[Box<str>],
        _mode: DropNaMode,
    ) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn as_any(&self) -> &dyn std::any::Any;
    fn concat_rows(
        &self,
        others: &[RelationHandle],
        mode: yss_data_contract::table::RowConcatMode,
    ) -> Result<RelationHandle, RelationError>;
    fn concat_columns(&self, others: &[RelationHandle]) -> Result<RelationHandle, RelationError>;
    fn join(
        &self,
        right: &RelationHandle,
        spec: &yss_data_contract::table::TableJoin,
    ) -> Result<RelationHandle, RelationError>;
    fn boolean_series(
        &self,
        operation: BooleanOperation,
        operands: &[BooleanOperand],
    ) -> Result<Arc<dyn SeriesPlan>, RelationError>;
    fn compare_series(
        &self,
        operation: ComparisonOperation,
        operands: &[ComparisonOperand],
    ) -> Result<Arc<dyn SeriesPlan>, RelationError>;
    fn bindings(&self) -> &[RelationBinding];
    fn schema(&self) -> SchemaRef;
    fn project(&self, columns: &[Box<str>]) -> Result<RelationHandle, RelationError>;
    fn filter(&self, predicate: &RelationPredicate) -> Result<RelationHandle, RelationError>;
    fn drop_rows(&self, predicate: &RelationPredicate) -> Result<RelationHandle, RelationError>;
    fn limit(&self, offset: usize, limit: usize) -> Result<RelationHandle, RelationError>;
    fn rename(&self, old: &str, new: &str) -> Result<RelationHandle, RelationError>;
    fn select_series(&self, column: &str) -> Result<Arc<dyn SeriesPlan>, RelationError>;
    fn project_series(
        &self,
        series: &[SeriesHandle],
        names: Option<&[Box<str>]>,
    ) -> Result<RelationHandle, RelationError>;
    fn numeric_series(
        &self,
        operation: NumericOperation,
        operands: &[SeriesOperand],
        output_type: NumericType,
    ) -> Result<Arc<dyn SeriesPlan>, RelationError>;
    fn convert_series(
        &self,
        series: &SeriesHandle,
        conversion: yss_data_contract::SemanticConversion,
    ) -> Result<Arc<dyn SeriesPlan>, RelationError>;
    fn stream(&self, control: RelationControl) -> RelationFuture<'_, RelationBatchStream>;
}

#[derive(Clone)]
pub struct RelationHandle {
    plan: Arc<dyn RelationPlan>,
    executor: Arc<dyn RelationExecutor>,
}

impl RelationHandle {
    pub fn shares_row_domain(&self, other: &Self) -> bool {
        self == other || self.plan.shares_row_domain(other)
    }
    pub fn check_row_domain(&self, other: &Self) -> Result<(), RelationError> {
        if self.shares_row_domain(other) {
            return Ok(());
        }
        if let (Some(left), Some(right)) = (
            self.plan.positional_length(),
            other.plan.positional_length(),
        ) && left != right
        {
            return Err(RelationError::ShapeMismatch);
        }
        Err(RelationError::UnalignedSeries)
    }
    pub fn transform_series(
        &self,
        series: &SeriesHandle,
        operation: &SeriesTransform,
    ) -> Result<SeriesHandle, RelationError> {
        Ok(SeriesHandle::new(
            self.clone(),
            self.plan.transform_series(series, operation)?,
        ))
    }
    pub fn reduce_series(
        &self,
        series: &SeriesHandle,
        operation: SeriesReduction,
    ) -> Result<Self, RelationError> {
        self.plan.reduce_series(series, operation)
    }
    pub fn choose_series(
        &self,
        condition: &SeriesHandle,
        when_true: &SeriesOperand,
        when_false: &SeriesOperand,
        output_field: Option<&arrow_schema::Field>,
    ) -> Result<SeriesHandle, RelationError> {
        Ok(SeriesHandle::new(
            self.clone(),
            self.plan
                .choose_series(condition, when_true, when_false, output_field)?,
        ))
    }
    pub fn concatenate_text(
        &self,
        operands: &[SeriesOperand],
        separator: &str,
    ) -> Result<SeriesHandle, RelationError> {
        Ok(SeriesHandle::new(
            self.clone(),
            self.plan.concatenate_text(operands, separator)?,
        ))
    }
    pub fn sort_rows(&self, columns: &[SortColumn]) -> Result<Self, RelationError> {
        self.plan.sort_rows(columns)
    }
    pub fn deduplicate(
        &self,
        keys: &[Box<str>],
        keep: DuplicateKeep,
    ) -> Result<Self, RelationError> {
        self.plan.deduplicate(keys, keep)
    }
    pub fn set_column(&self, name: &str, series: &SeriesHandle) -> Result<Self, RelationError> {
        self.plan.set_column(name, series)
    }
    pub fn filter_mask(
        &self,
        mask: &SeriesHandle,
        drop_matches: bool,
    ) -> Result<Self, RelationError> {
        self.plan.filter_mask(mask, drop_matches)
    }
    pub fn unpivot(&self, spec: &UnpivotSpec) -> Result<Self, RelationError> {
        self.plan.unpivot(spec)
    }
    pub fn pivot(&self, spec: &PivotSpec) -> Result<Self, RelationError> {
        self.plan.pivot(spec)
    }
    pub fn resample(&self, spec: &ResampleSpec) -> Result<Self, RelationError> {
        self.plan.resample(spec)
    }
    pub fn encode_series(
        &self,
        series: &SeriesHandle,
        levels: &[PivotLevel],
        reference: Option<&str>,
    ) -> Result<Self, RelationError> {
        self.plan.encode_series(series, levels, reference)
    }
    pub fn align_grid(
        &self,
        time: &str,
        entity: Option<&str>,
        interval: i64,
        max_bytes: usize,
    ) -> Result<Self, RelationError> {
        self.plan.align_grid(time, entity, interval, max_bytes)
    }
    pub fn aggregate(
        &self,
        keys: &[Box<str>],
        columns: &[yss_data_contract::aggregation::ColumnAggregate],
    ) -> Result<Self, RelationError> {
        self.plan.aggregate(keys, columns)
    }
    pub fn describe(&self, columns: &[Box<str>]) -> Result<Self, RelationError> {
        self.plan.describe(columns)
    }
    pub fn frequency(&self, column: &str, include_null: bool) -> Result<Self, RelationError> {
        self.plan.frequency(column, include_null)
    }
    pub fn compare_series_with_tolerance(
        &self,
        operation: ComparisonOperation,
        operands: &[ComparisonOperand],
        tolerance: NumericTolerance,
    ) -> Result<SeriesHandle, RelationError> {
        if operands.len() != 2
            || !operands
                .iter()
                .any(|v| matches!(v, ComparisonOperand::Series(_)))
        {
            return Err(RelationError::InvalidInput);
        }
        for operand in operands {
            if let ComparisonOperand::Series(series) = operand {
                self.check_row_domain(series.relation())?;
            }
        }
        Ok(SeriesHandle::new(
            self.clone(),
            self.plan
                .compare_series_with_tolerance(operation, operands, tolerance)?,
        ))
    }
    /// Consume Arrow batches on an existing compute worker, with adapter-owned async execution.
    pub fn visit_batches(
        &self,
        control: &RelationControl,
        visitor: &mut dyn FnMut(RecordBatch) -> Result<(), RelationError>,
    ) -> Result<(), RelationError> {
        self.executor.visit_batches(self, control, visitor)
    }
    pub fn schema_is_deferred(&self) -> bool {
        self.plan.schema_is_deferred()
    }
    /// Resolve data-dependent columns only at an execution/consumption boundary.
    pub fn resolve(&self, control: RelationControl) -> RelationFuture<'_, Self> {
        Box::pin(async move {
            control.check()?;
            Ok(self
                .plan
                .resolve(control)
                .await?
                .unwrap_or_else(|| self.clone()))
        })
    }
    pub fn drop_na_rows(
        &self,
        columns: &[Box<str>],
        mode: DropNaMode,
    ) -> Result<Self, RelationError> {
        self.plan.drop_na_rows(columns, mode)
    }
    pub fn drop_na_columns(
        &self,
        columns: &[Box<str>],
        mode: DropNaMode,
    ) -> Result<Self, RelationError> {
        self.plan.drop_na_columns(columns, mode)
    }

    pub fn boolean_series(
        &self,
        operation: BooleanOperation,
        operands: &[BooleanOperand],
    ) -> Result<SeriesHandle, RelationError> {
        if operands.len() != operation.arity()
            || !operands
                .iter()
                .any(|v| matches!(v, BooleanOperand::Series(_)))
        {
            return Err(RelationError::InvalidInput);
        }
        for operand in operands {
            if let BooleanOperand::Series(series) = operand {
                self.check_row_domain(series.relation())?;
            }
        }
        Ok(SeriesHandle::new(
            self.clone(),
            self.plan.boolean_series(operation, operands)?,
        ))
    }
    pub fn compare_series(
        &self,
        operation: ComparisonOperation,
        operands: &[ComparisonOperand],
    ) -> Result<SeriesHandle, RelationError> {
        if operands.len() != 2
            || !operands
                .iter()
                .any(|v| matches!(v, ComparisonOperand::Series(_)))
        {
            return Err(RelationError::InvalidInput);
        }
        for operand in operands {
            if let ComparisonOperand::Series(series) = operand {
                self.check_row_domain(series.relation())?;
            }
        }
        Ok(SeriesHandle::new(
            self.clone(),
            self.plan.compare_series(operation, operands)?,
        ))
    }
    pub fn new(plan: Arc<dyn RelationPlan>, executor: Arc<dyn RelationExecutor>) -> Self {
        Self { plan, executor }
    }
    pub fn bindings(&self) -> &[RelationBinding] {
        self.plan.bindings()
    }
    pub fn plan(&self) -> &dyn RelationPlan {
        self.plan.as_ref()
    }
    pub fn concat_rows(
        &self,
        others: &[Self],
        mode: yss_data_contract::table::RowConcatMode,
    ) -> Result<Self, RelationError> {
        if others.is_empty() {
            return Err(RelationError::InvalidInput);
        }
        self.plan.concat_rows(others, mode)
    }
    pub fn concat_columns(&self, others: &[Self]) -> Result<Self, RelationError> {
        if others.is_empty() {
            return Err(RelationError::InvalidInput);
        }
        self.plan.concat_columns(others)
    }
    pub fn join(
        &self,
        right: &Self,
        spec: &yss_data_contract::table::TableJoin,
    ) -> Result<Self, RelationError> {
        if !spec.is_valid() {
            return Err(RelationError::InvalidInput);
        }
        self.plan.join(right, spec)
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
    /// Remove only matching rows, retaining false and unknown predicate results.
    pub fn drop_rows(&self, predicate: &RelationPredicate) -> Result<Self, RelationError> {
        self.plan.drop_rows(predicate)
    }
    pub fn drop_columns(&self, columns: &[Box<str>]) -> Result<Self, RelationError> {
        let schema = self.schema();
        let selected: std::collections::BTreeSet<_> = columns.iter().map(AsRef::as_ref).collect();
        if columns.is_empty()
            || selected.len() != columns.len()
            || columns.iter().any(|name| schema.index_of(name).is_err())
        {
            return Err(RelationError::InvalidInput);
        }
        let retained: Vec<Box<str>> = schema
            .fields()
            .iter()
            .filter(|field| !selected.contains(field.name().as_str()))
            .map(|field| field.name().as_str().into())
            .collect();
        if retained.is_empty() {
            return Err(RelationError::InvalidInput);
        }
        self.project(&retained)
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
        for series in series {
            self.check_row_domain(series.relation())?;
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
        self.plan.project_series(series, None)
    }
    pub fn assemble_series(
        &self,
        series: &[SeriesHandle],
        names: &[Box<str>],
    ) -> Result<Self, RelationError> {
        if series.is_empty()
            || series.len() != names.len()
            || names.iter().any(|n| n.is_empty())
            || names
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != names.len()
        {
            return Err(RelationError::InvalidInput);
        }
        self.plan.project_series(series, Some(names))
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
        for operand in operands {
            if let SeriesOperand::Series(series) = operand {
                self.check_row_domain(series.relation())?;
            }
        }
        Ok(SeriesHandle::new(
            self.clone(),
            self.plan.numeric_series(operation, operands, output_type)?,
        ))
    }

    pub fn convert_series(
        &self,
        series: &SeriesHandle,
        conversion: yss_data_contract::SemanticConversion,
    ) -> Result<SeriesHandle, RelationError> {
        self.check_row_domain(series.relation())?;
        Ok(SeriesHandle::new(
            self.clone(),
            self.plan.convert_series(series, conversion)?,
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
            .field("bindings", &self.bindings())
            .finish_non_exhaustive()
    }
}

/// Synchronous callers run on compute workers. The adapter consumes the asynchronous Arrow
/// stream in one joint projection and enforces the separately supplied statistics memory budget.
pub trait RelationExecutor: Send + Sync {
    fn visit_batches(
        &self,
        _relation: &RelationHandle,
        _control: &RelationControl,
        _visitor: &mut dyn FnMut(RecordBatch) -> Result<(), RelationError>,
    ) -> Result<(), RelationError> {
        Err(RelationError::InvalidInput)
    }
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
