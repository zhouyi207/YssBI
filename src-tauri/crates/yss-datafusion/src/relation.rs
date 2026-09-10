use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use arrow::datatypes::{DataType, Schema, SchemaRef};
use datafusion::common::{Column, ScalarValue};
use datafusion::dataframe::DataFrame;
use datafusion::logical_expr::Expr;
use futures_util::StreamExt;
use yss_relational_contract::{
    RelationBatchStream, RelationBinding, RelationComparison, RelationControl, RelationError,
    RelationExecutor, RelationFuture, RelationHandle, RelationLiteral, RelationPlan,
    RelationPredicate,
};

pub(crate) struct DataFusionRelation {
    frame: DataFrame,
    // Native CASE/projection rewrites do not reliably retain field metadata. Keep the exact
    // Arrow schema derived from the source, validate native types, and attach it at the stream.
    schema: SchemaRef,
    binding: RelationBinding,
    lease: Arc<dyn Send + Sync>,
    executor: Arc<dyn RelationExecutor>,
}

impl DataFusionRelation {
    pub(crate) fn handle(
        frame: DataFrame,
        schema: SchemaRef,
        binding: RelationBinding,
        lease: Arc<dyn Send + Sync>,
        executor: Arc<dyn RelationExecutor>,
    ) -> Result<RelationHandle, RelationError> {
        let native = frame.schema().as_arrow();
        if native.fields().len() != schema.fields().len()
            || native
                .fields()
                .iter()
                .zip(schema.fields())
                .any(|(native, exact)| {
                    native.name() != exact.name() || native.data_type() != exact.data_type()
                })
        {
            return Err(RelationError::InvalidPlan);
        }
        Ok(RelationHandle::new(
            Arc::new(Self {
                frame,
                schema,
                binding,
                lease,
                executor: executor.clone(),
            }),
            executor,
        ))
    }
    fn derived(
        &self,
        frame: DataFrame,
        schema: SchemaRef,
    ) -> Result<RelationHandle, RelationError> {
        Self::handle(
            frame,
            schema,
            self.binding.clone(),
            self.lease.clone(),
            self.executor.clone(),
        )
    }
}

impl RelationPlan for DataFusionRelation {
    fn binding(&self) -> &RelationBinding {
        &self.binding
    }
    fn schema(&self) -> SchemaRef {
        self.schema.clone()
    }
    fn project(&self, columns: &[Box<str>]) -> Result<RelationHandle, RelationError> {
        let projection = columns
            .iter()
            .map(|name| {
                self.schema
                    .index_of(name)
                    .map_err(|_| RelationError::InvalidInput)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let schema = Arc::new(
            self.schema
                .project(&projection)
                .map_err(|_| RelationError::InvalidInput)?,
        );
        let expressions = columns
            .iter()
            .map(|name| Expr::Column(Column::from_name(name.to_string())));
        self.frame
            .clone()
            .select(expressions)
            .map_err(|_| RelationError::InvalidPlan)
            .and_then(|frame| self.derived(frame, schema))
    }
    fn filter(&self, predicate: &RelationPredicate) -> Result<RelationHandle, RelationError> {
        let schema = self.schema();
        let field = schema
            .field_with_name(&predicate.column)
            .map_err(|_| RelationError::InvalidInput)?;
        let column = Expr::Column(Column::from_name(predicate.column.to_string()));
        let expression = match (predicate.comparison, &predicate.value) {
            (RelationComparison::IsNull, None) => column.is_null(),
            (RelationComparison::IsNotNull, None) => column.is_not_null(),
            (RelationComparison::IsNull | RelationComparison::IsNotNull, _) | (_, None) => {
                return Err(RelationError::InvalidInput);
            }
            (comparison, Some(value)) => {
                let value = match value {
                    RelationLiteral::Boolean(value) => ScalarValue::Boolean(Some(*value)),
                    RelationLiteral::Integer(value) => ScalarValue::Int64(Some(*value)),
                    RelationLiteral::Decimal(value) => {
                        if matches!(
                            field.data_type(),
                            DataType::Decimal32(..)
                                | DataType::Decimal64(..)
                                | DataType::Decimal128(..)
                                | DataType::Decimal256(..)
                        ) {
                            let array = yss_tabular_arrow::json_to_array(
                                field,
                                &[serde_json::Value::String(value.to_string())],
                            )
                            .map_err(|_| RelationError::InvalidInput)?;
                            ScalarValue::try_from_array(array.as_ref(), 0)
                                .map_err(|_| RelationError::InvalidInput)?
                        } else {
                            ScalarValue::Float64(Some(
                                value
                                    .parse::<f64>()
                                    .ok()
                                    .filter(|v| v.is_finite())
                                    .ok_or(RelationError::InvalidInput)?,
                            ))
                        }
                    }
                    RelationLiteral::String(value) => {
                        let scalar = ScalarValue::Utf8(Some(value.to_string()));
                        if field.data_type().is_temporal() {
                            scalar
                                .cast_to(field.data_type())
                                .map_err(|_| RelationError::InvalidInput)?
                        } else {
                            scalar
                        }
                    }
                };
                let value = Expr::Literal(value, None);
                match comparison {
                    RelationComparison::Equal => column.eq(value),
                    RelationComparison::NotEqual => column.not_eq(value),
                    RelationComparison::Less => column.lt(value),
                    RelationComparison::LessEqual => column.lt_eq(value),
                    RelationComparison::Greater => column.gt(value),
                    RelationComparison::GreaterEqual => column.gt_eq(value),
                    _ => return Err(RelationError::InvalidInput),
                }
            }
        };
        self.frame
            .clone()
            .filter(expression)
            .map_err(|_| RelationError::InvalidPlan)
            .and_then(|frame| self.derived(frame, self.schema.clone()))
    }
    fn limit(&self, offset: usize, limit: usize) -> Result<RelationHandle, RelationError> {
        self.frame
            .clone()
            .limit(offset, Some(limit))
            .map_err(|_| RelationError::InvalidPlan)
            .and_then(|frame| self.derived(frame, self.schema.clone()))
    }
    fn rename(&self, old: &str, new: &str) -> Result<RelationHandle, RelationError> {
        if new.trim().is_empty()
            || self.schema.index_of(old).is_err()
            || (old != new && self.schema.index_of(new).is_ok())
        {
            return Err(RelationError::InvalidInput);
        }
        let fields = self
            .schema
            .fields()
            .iter()
            .map(|field| {
                if field.name() == old {
                    Arc::new(field.as_ref().clone().with_name(new))
                } else {
                    field.clone()
                }
            })
            .collect::<Vec<_>>();
        let schema = Arc::new(Schema::new_with_metadata(
            fields,
            self.schema.metadata().clone(),
        ));
        self.frame
            .clone()
            .with_column_renamed(old, new)
            .map_err(|_| RelationError::InvalidPlan)
            .and_then(|frame| self.derived(frame, schema))
    }
    fn stream(&self, control: RelationControl) -> RelationFuture<'_, RelationBatchStream> {
        Box::pin(async move {
            let stream = controlled(self.frame.clone().execute_stream(), &control)
                .await?
                .map_err(crate::relation::query_error)?;
            let lease = self.lease.clone();
            let executor = self.executor.clone();
            let schema = self.schema();
            let stream = futures_util::stream::try_unfold(
                (stream, control, lease, executor, schema),
                |(mut stream, control, lease, executor, schema)| async move {
                    let next = controlled(stream.next(), &control).await?;
                    next.map(|batch| {
                        batch
                            .and_then(|batch| {
                                arrow::record_batch::RecordBatch::try_new_with_options(
                                    schema.clone(),
                                    batch.columns().to_vec(),
                                    &arrow::record_batch::RecordBatchOptions::new()
                                        .with_row_count(Some(batch.num_rows())),
                                )
                                .map_err(Into::into)
                            })
                            .map(|batch| (batch, (stream, control, lease, executor, schema)))
                            .map_err(crate::relation::query_error)
                    })
                    .transpose()
                },
            );
            Ok(Box::pin(stream) as RelationBatchStream)
        })
    }
}

pub(crate) fn query_error(error: datafusion::common::DataFusionError) -> RelationError {
    fn classify(error: &datafusion::common::DataFusionError) -> RelationError {
        use datafusion::common::DataFusionError;
        match error {
            DataFusionError::ResourcesExhausted(_) => RelationError::MemoryLimitExceeded,
            DataFusionError::Context(_, inner) => classify(inner),
            DataFusionError::Shared(inner) => classify(inner),
            _ => RelationError::QueryFailed,
        }
    }
    classify(&error)
}

pub(crate) async fn controlled<T>(
    future: impl Future<Output = T>,
    control: &RelationControl,
) -> Result<T, RelationError> {
    tokio::pin!(future);
    loop {
        control.check()?;
        tokio::select! {
            result = &mut future => { control.check()?; return Ok(result); }
            _ = tokio::time::sleep(Duration::from_millis(20)) => {}
        }
    }
}
