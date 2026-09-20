//! Data-dependent column selection is resolved at consumption, never during graph planning.
use crate::relation::DataFusionRelation;
use arrow::datatypes::{Schema, SchemaRef};
use datafusion::{common::Column, logical_expr::Expr};
use futures_util::StreamExt;
use std::sync::{Arc, OnceLock};
use yss_relational_contract::*;

fn selected(schema: &Schema, columns: &[Box<str>]) -> Result<Vec<usize>, RelationError> {
    if columns.is_empty() {
        return Ok((0..schema.fields().len()).collect());
    }
    let mut seen = std::collections::BTreeSet::new();
    columns
        .iter()
        .map(|name| {
            if !seen.insert(name.as_ref()) {
                return Err(RelationError::InvalidInput);
            }
            schema
                .index_of(name)
                .map_err(|_| RelationError::InvalidInput)
        })
        .collect()
}

pub(crate) fn rows(
    source: &DataFusionRelation,
    columns: &[Box<str>],
    mode: DropNaMode,
) -> Result<RelationHandle, RelationError> {
    let indexes = selected(&source.schema, columns)?;
    let missing = indexes
        .into_iter()
        .map(|i| Expr::Column(Column::from_name(source.schema.field(i).name().clone())).is_null())
        .reduce(|a, b| match mode {
            DropNaMode::Any => a.or(b),
            DropNaMode::All => a.and(b),
        });
    if let Some(missing) = missing {
        let frame = source
            .domain
            .as_ref()
            .clone()
            .filter(source.expand_expression(missing.is_not_true())?)
            .map_err(|_| RelationError::InvalidPlan)?;
        source.derived(frame, source.schema.clone(), false)
    } else {
        source.project(
            &source
                .schema
                .fields()
                .iter()
                .map(|f| f.name().as_str().into())
                .collect::<Vec<_>>(),
        )
    }
}

pub(crate) fn columns(
    source: &DataFusionRelation,
    columns: &[Box<str>],
    mode: DropNaMode,
) -> Result<RelationHandle, RelationError> {
    selected(&source.schema, columns)?;
    let source_handle = source.project(
        &source
            .schema
            .fields()
            .iter()
            .map(|f| f.name().as_str().into())
            .collect::<Vec<_>>(),
    )?;
    Ok(DeferredRelation::handle(
        source_handle,
        source.executor.clone(),
        Operation::DropColumns(columns.to_vec(), mode),
    ))
}

// These requests only bridge the data-dependent schema boundary. Once resolved, all
// relational planning and optimization remains in the native engine.
#[derive(Clone)]
enum Operation {
    DropColumns(Vec<Box<str>>, DropNaMode),
    DropRows(Vec<Box<str>>, DropNaMode),
    Project(Vec<Box<str>>),
    Filter(RelationPredicate, bool),
    Limit(usize, usize),
    Rename(String, String),
}
#[derive(Clone)]
struct DeferredRelation {
    source: RelationHandle,
    executor: Arc<dyn RelationExecutor>,
    operation: Operation,
    resolved: Arc<OnceLock<RelationHandle>>,
}
impl DeferredRelation {
    fn handle(
        source: RelationHandle,
        executor: Arc<dyn RelationExecutor>,
        operation: Operation,
    ) -> RelationHandle {
        RelationHandle::new(
            Arc::new(Self {
                source,
                executor: executor.clone(),
                operation,
                resolved: Arc::new(OnceLock::new()),
            }),
            executor,
        )
    }
    fn next(&self, operation: Operation) -> RelationHandle {
        let source = RelationHandle::new(Arc::new(self.clone()), self.executor.clone());
        Self::handle(source, self.executor.clone(), operation)
    }
    async fn materialize_schema(
        &self,
        control: RelationControl,
    ) -> Result<RelationHandle, RelationError> {
        control.check()?;
        if let Some(resolved) = self.resolved.get() {
            return Ok(resolved.clone());
        }
        let source = self.source.resolve(control.clone()).await?;
        let resolved = match &self.operation {
            Operation::DropColumns(columns, mode) => {
                resolve_columns(&source, columns, *mode, &control).await?
            }
            Operation::DropRows(columns, mode) => source.drop_na_rows(columns, *mode)?,
            Operation::Project(columns) => source.project(columns)?,
            Operation::Filter(predicate, drop) => {
                if *drop {
                    source.drop_rows(predicate)?
                } else {
                    source.filter(predicate)?
                }
            }
            Operation::Limit(offset, limit) => source.limit(*offset, *limit)?,
            Operation::Rename(old, new) => source.rename(old, new)?,
        };
        control.check()?;
        // Cache only a successful plan over the immutable snapshot, never batches or failures.
        let _ = self.resolved.set(resolved.clone());
        Ok(resolved)
    }
}

async fn resolve_columns(
    source: &RelationHandle,
    columns: &[Box<str>],
    mode: DropNaMode,
    control: &RelationControl,
) -> Result<RelationHandle, RelationError> {
    let schema = source.schema();
    let indexes = selected(&schema, columns)?;
    if indexes.len().saturating_mul(2) > control.max_input_bytes {
        return Err(RelationError::MemoryLimitExceeded);
    }
    if indexes.is_empty() {
        return Ok(source.clone());
    }
    let names = indexes
        .iter()
        .map(|i| schema.field(*i).name().as_str().into())
        .collect::<Vec<Box<str>>>();
    let scan = source.project(&names)?;
    let mut stream = scan.stream(control.clone()).await?;
    let mut has_null = vec![false; indexes.len()];
    let mut has_value = vec![false; indexes.len()];
    let mut has_rows = false;
    while let Some(batch) = crate::relation::controlled(stream.next(), control).await? {
        let batch = batch?;
        if batch.get_array_memory_size() > control.max_input_bytes {
            return Err(RelationError::MemoryLimitExceeded);
        }
        has_rows |= batch.num_rows() > 0;
        for (i, array) in batch.columns().iter().enumerate() {
            let nulls = array.logical_null_count();
            has_null[i] |= nulls > 0;
            has_value[i] |= nulls < array.len();
        }
    }
    control.check()?;
    let dropped = indexes
        .iter()
        .enumerate()
        .filter_map(|(i, index)| {
            (has_rows
                && match mode {
                    DropNaMode::Any => has_null[i],
                    DropNaMode::All => !has_value[i],
                })
            .then_some(*index)
        })
        .collect::<std::collections::BTreeSet<_>>();
    let retained = schema
        .fields()
        .iter()
        .enumerate()
        .filter(|(i, _)| !dropped.contains(i))
        .map(|(_, f)| f.name().as_str().into())
        .collect::<Vec<Box<str>>>();
    source.project(&retained)
}

impl RelationPlan for DeferredRelation {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn bindings(&self) -> &[RelationBinding] {
        self.source.bindings()
    }
    fn schema_is_deferred(&self) -> bool {
        true
    }
    fn schema(&self) -> SchemaRef {
        Arc::new(Schema::empty())
    }
    fn resolve(&self, control: RelationControl) -> RelationFuture<'_, Option<RelationHandle>> {
        Box::pin(async move { self.materialize_schema(control).await.map(Some) })
    }
    fn stream(&self, control: RelationControl) -> RelationFuture<'_, RelationBatchStream> {
        Box::pin(async move {
            self.materialize_schema(control.clone())
                .await?
                .stream(control)
                .await
        })
    }
    fn drop_na_rows(
        &self,
        columns: &[Box<str>],
        mode: DropNaMode,
    ) -> Result<RelationHandle, RelationError> {
        Ok(self.next(Operation::DropRows(columns.to_vec(), mode)))
    }
    fn drop_na_columns(
        &self,
        columns: &[Box<str>],
        mode: DropNaMode,
    ) -> Result<RelationHandle, RelationError> {
        Ok(self.next(Operation::DropColumns(columns.to_vec(), mode)))
    }
    fn project(&self, columns: &[Box<str>]) -> Result<RelationHandle, RelationError> {
        Ok(self.next(Operation::Project(columns.to_vec())))
    }
    fn filter(&self, predicate: &RelationPredicate) -> Result<RelationHandle, RelationError> {
        Ok(self.next(Operation::Filter(predicate.clone(), false)))
    }
    fn drop_rows(&self, predicate: &RelationPredicate) -> Result<RelationHandle, RelationError> {
        Ok(self.next(Operation::Filter(predicate.clone(), true)))
    }
    fn limit(&self, offset: usize, limit: usize) -> Result<RelationHandle, RelationError> {
        Ok(self.next(Operation::Limit(offset, limit)))
    }
    fn rename(&self, old: &str, new: &str) -> Result<RelationHandle, RelationError> {
        Ok(self.next(Operation::Rename(old.into(), new.into())))
    }
    fn concat_rows(
        &self,
        _others: &[RelationHandle],
        _mode: yss_data_contract::table::RowConcatMode,
    ) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn concat_columns(&self, _others: &[RelationHandle]) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn join(
        &self,
        _right: &RelationHandle,
        _spec: &yss_data_contract::table::TableJoin,
    ) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn boolean_series(
        &self,
        _operation: BooleanOperation,
        _operands: &[BooleanOperand],
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn compare_series(
        &self,
        _operation: ComparisonOperation,
        _operands: &[ComparisonOperand],
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn select_series(&self, _column: &str) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn project_series(
        &self,
        _series: &[SeriesHandle],
        _names: Option<&[Box<str>]>,
    ) -> Result<RelationHandle, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn numeric_series(
        &self,
        _operation: NumericOperation,
        _operands: &[SeriesOperand],
        _output_type: NumericType,
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        Err(RelationError::InvalidInput)
    }
    fn convert_series(
        &self,
        _series: &SeriesHandle,
        _conversion: yss_data_contract::SemanticConversion,
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        Err(RelationError::InvalidInput)
    }
}
