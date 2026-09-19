use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use arrow::datatypes::{DataType, Schema, SchemaRef};
use datafusion::common::{Column, ScalarValue};
use datafusion::dataframe::DataFrame;
use datafusion::logical_expr::Expr;
use futures_util::StreamExt;
use yss_relational_contract::{
    NumericOperation, NumericType, RelationBatchStream, RelationBinding, RelationComparison,
    RelationControl, RelationError, RelationExecutor, RelationFuture, RelationHandle,
    RelationLiteral, RelationPlan, RelationPredicate, SeriesHandle, SeriesOperand, SeriesPlan,
};

pub(crate) struct DataFusionRelation {
    pub(crate) frame: DataFrame,
    // Native CASE/projection rewrites do not reliably retain field metadata. Keep the exact
    // Arrow schema derived from the source, validate native types, and attach it at the stream.
    pub(crate) schema: SchemaRef,
    pub(crate) bindings: Arc<[RelationBinding]>,
    pub(crate) lease: Arc<dyn Send + Sync>,
    pub(crate) executor: Arc<dyn RelationExecutor>,
    pub(crate) ordered_single_file: bool,
    pub(crate) domain: Arc<DataFrame>,
    pub(crate) columns: Vec<Expr>,
    pub(crate) domain_order: Vec<datafusion::logical_expr::expr::Sort>,
}

impl DataFusionRelation {
    fn filter_rows(
        &self,
        predicate: &RelationPredicate,
        drop_matches: bool,
    ) -> Result<RelationHandle, RelationError> {
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
                let ordering = matches!(
                    comparison,
                    RelationComparison::Less
                        | RelationComparison::LessEqual
                        | RelationComparison::Greater
                        | RelationComparison::GreaterEqual
                );
                let semantic = yss_tabular_arrow::column_semantic(field)
                    .map_err(|_| RelationError::InvalidInput)?;
                if ordering
                    && matches!(
                        semantic.kind,
                        yss_tabular_arrow::SemanticType::Categorical
                            | yss_tabular_arrow::SemanticType::Binary
                            | yss_tabular_arrow::SemanticType::Identifier
                    )
                {
                    return Err(RelationError::InvalidInput);
                }
                if ordering && semantic.kind == yss_tabular_arrow::SemanticType::Ordinal {
                    let value = match value {
                        RelationLiteral::Boolean(value) => serde_json::json!(value),
                        RelationLiteral::Integer(value) => serde_json::json!(value),
                        RelationLiteral::Decimal(value) | RelationLiteral::String(value) => {
                            serde_json::json!(value)
                        }
                    };
                    let array = yss_tabular_arrow::json_to_array(field, &[value])
                        .map_err(|_| RelationError::InvalidInput)?;
                    let array = arrow::compute::cast(array.as_ref(), &DataType::Utf8)
                        .map_err(|_| RelationError::InvalidInput)?;
                    let text = array
                        .as_any()
                        .downcast_ref::<arrow::array::StringArray>()
                        .ok_or(RelationError::InvalidInput)?
                        .value(0);
                    let rank = semantic
                        .values
                        .iter()
                        .position(|value| value.value == text)
                        .ok_or(RelationError::InvalidInput)?;
                    let selected = semantic
                        .values
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| match comparison {
                            RelationComparison::Less => *index < rank,
                            RelationComparison::LessEqual => *index <= rank,
                            RelationComparison::Greater => *index > rank,
                            RelationComparison::GreaterEqual => *index >= rank,
                            _ => false,
                        })
                        .map(|(_, value)| {
                            Expr::Literal(ScalarValue::Utf8(Some(value.value.clone())), None)
                        })
                        .collect();
                    datafusion::logical_expr::expr_fn::cast(column, DataType::Utf8)
                        .in_list(selected, false)
                } else {
                    let value = match value {
                        RelationLiteral::Boolean(value) => ScalarValue::Boolean(Some(*value)),
                        RelationLiteral::Integer(value) if field.data_type().is_floating() => {
                            let exact = yss_tabular_arrow::lossless_cast(
                                &arrow::array::Int64Array::from(vec![*value]),
                                field.data_type(),
                                false,
                            )
                            .map_err(|_| RelationError::InvalidInput)?;
                            ScalarValue::try_from_array(exact.as_ref(), 0)
                                .map_err(|_| RelationError::InvalidInput)?
                        }
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
                            if field.data_type().is_temporal() {
                                let array = yss_tabular_arrow::json_to_array(
                                    field,
                                    &[serde_json::Value::String(value.to_string())],
                                )
                                .map_err(|_| RelationError::InvalidInput)?;
                                ScalarValue::try_from_array(array.as_ref(), 0)
                                    .map_err(|_| RelationError::InvalidInput)?
                            } else {
                                ScalarValue::Utf8(Some(value.to_string()))
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
            }
        };
        let expression = if drop_matches {
            expression.is_not_true()
        } else {
            expression
        };
        self.domain
            .as_ref()
            .clone()
            .filter(self.expand_expression(expression)?)
            .map_err(|_| RelationError::InvalidPlan)
            .and_then(|frame| self.derived(frame, self.schema.clone(), false))
    }

    pub(crate) fn handle(
        frame: DataFrame,
        schema: SchemaRef,
        bindings: Arc<[RelationBinding]>,
        lease: Arc<dyn Send + Sync>,
        executor: Arc<dyn RelationExecutor>,
        ordered_single_file: bool,
        domain_order: Vec<datafusion::logical_expr::expr::Sort>,
    ) -> Result<RelationHandle, RelationError> {
        let columns = schema
            .fields()
            .iter()
            .map(|field| Expr::Column(Column::from_name(field.name().clone())))
            .collect::<Vec<_>>();
        let domain = Arc::new(frame);
        let frame = domain
            .as_ref()
            .clone()
            .select(columns.clone())
            .map_err(|_| RelationError::InvalidPlan)?;
        Self {
            domain,
            domain_order,
            columns,
            frame,
            schema,
            bindings,
            lease,
            executor,
            ordered_single_file,
        }
        .into_handle()
    }

    pub(crate) fn into_handle(self) -> Result<RelationHandle, RelationError> {
        let native = self.frame.schema().as_arrow();
        if native.fields().len() != self.schema.fields().len()
            || native
                .fields()
                .iter()
                .zip(self.schema.fields())
                .any(|(native, exact)| {
                    native.name() != exact.name() || native.data_type() != exact.data_type()
                })
        {
            return Err(RelationError::InvalidPlan);
        }
        let executor = self.executor.clone();
        Ok(RelationHandle::new(Arc::new(self), executor))
    }
    fn derived(
        &self,
        frame: DataFrame,
        schema: SchemaRef,
        ordered_single_file: bool,
    ) -> Result<RelationHandle, RelationError> {
        let domain = Arc::new(frame);
        let frame = domain
            .as_ref()
            .clone()
            .select(
                self.columns
                    .iter()
                    .zip(schema.fields())
                    .map(|(expr, field)| expr.clone().alias(field.name())),
            )
            .map_err(|_| RelationError::InvalidPlan)?;
        Self {
            domain,
            domain_order: self.domain_order.clone(),
            columns: self.columns.clone(),
            frame,
            schema,
            bindings: self.bindings.clone(),
            lease: self.lease.clone(),
            executor: self.executor.clone(),
            ordered_single_file,
        }
        .into_handle()
    }
}

impl RelationPlan for DataFusionRelation {
    fn boolean_series(
        &self,
        operation: yss_relational_contract::BooleanOperation,
        operands: &[yss_relational_contract::BooleanOperand],
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        crate::series::boolean(operation, operands)
    }
    fn bindings(&self) -> &[RelationBinding] {
        &self.bindings
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn schema(&self) -> SchemaRef {
        self.schema.clone()
    }
    fn project(&self, columns: &[Box<str>]) -> Result<RelationHandle, RelationError> {
        let indexes = columns
            .iter()
            .map(|name| {
                self.schema
                    .index_of(name)
                    .map_err(|_| RelationError::InvalidInput)
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.project_expressions(
            indexes
                .iter()
                .map(|i| self.schema.field(*i).clone())
                .collect(),
            indexes.iter().map(|i| self.columns[*i].clone()).collect(),
        )
    }
    fn filter(&self, predicate: &RelationPredicate) -> Result<RelationHandle, RelationError> {
        self.filter_rows(predicate, false)
    }
    fn drop_rows(&self, predicate: &RelationPredicate) -> Result<RelationHandle, RelationError> {
        self.filter_rows(predicate, true)
    }
    fn limit(&self, offset: usize, limit: usize) -> Result<RelationHandle, RelationError> {
        // This limit fixes the scan strategy. A later limit must not turn an existing
        // large-offset query into a single-partition prefix scan.
        crate::limit_frame(
            self.domain.as_ref().clone(),
            offset,
            limit,
            self.ordered_single_file,
        )
        .and_then(|frame| self.derived(frame, self.schema.clone(), false))
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
        self.project_expressions(
            schema.fields().iter().map(|f| f.as_ref().clone()).collect(),
            self.columns.clone(),
        )
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

    fn select_series(&self, column: &str) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        let field = self
            .schema
            .field_with_name(column)
            .map_err(|_| RelationError::InvalidInput)?;
        Ok(crate::series::column(field))
    }

    fn project_series(
        &self,
        series: &[SeriesHandle],
        output_names: Option<&[Box<str>]>,
    ) -> Result<RelationHandle, RelationError> {
        let mut names = Vec::new();
        let mut fields = Vec::new();
        let mut expressions = Vec::new();
        for (index, series) in series.iter().enumerate() {
            let owner = series
                .relation()
                .plan()
                .as_any()
                .downcast_ref::<Self>()
                .ok_or(RelationError::InvalidInput)?;
            if !Arc::ptr_eq(&self.domain, &owner.domain) {
                return Err(RelationError::UnalignedSeries);
            }
            let mut name = output_names.map_or_else(
                || series.column().to_owned(),
                |names| names[index].to_string(),
            );
            while names.contains(&name) {
                name.push('_');
            }
            names.push(name.clone());
            fields.push(if output_names.is_some() {
                crate::composition::derived_field(series.plan().field(), &name)
            } else {
                series.plan().field().clone().with_name(&name)
            });
            expressions.push(owner.expand_expression(crate::series::expression(series)?)?);
        }
        self.project_expressions(fields, expressions)
    }
    fn concat_rows(
        &self,
        others: &[RelationHandle],
        mode: yss_data_contract::table::RowConcatMode,
    ) -> Result<RelationHandle, RelationError> {
        self.compose_rows(others, mode)
    }
    fn concat_columns(&self, others: &[RelationHandle]) -> Result<RelationHandle, RelationError> {
        self.compose_columns(others)
    }
    fn join(
        &self,
        right: &RelationHandle,
        spec: &yss_data_contract::table::TableJoin,
    ) -> Result<RelationHandle, RelationError> {
        self.compose_join(right, spec)
    }

    fn numeric_series(
        &self,
        operation: NumericOperation,
        operands: &[SeriesOperand],
        output_type: NumericType,
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        crate::series::arithmetic(operation, operands, output_type)
    }

    fn compare_series(
        &self,
        operation: yss_relational_contract::ComparisonOperation,
        operands: &[yss_relational_contract::ComparisonOperand],
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        crate::comparison::compare(operation, operands)
    }

    fn convert_series(
        &self,
        series: &SeriesHandle,
        conversion: yss_data_contract::SemanticConversion,
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        crate::series::convert(series, conversion)
    }
}

pub(crate) fn query_error(error: datafusion::common::DataFusionError) -> RelationError {
    fn classify(error: &datafusion::common::DataFusionError) -> RelationError {
        use datafusion::common::DataFusionError;
        match error {
            DataFusionError::ResourcesExhausted(_) => RelationError::MemoryLimitExceeded,
            DataFusionError::Context(_, inner) => classify(inner),
            DataFusionError::Shared(inner) => classify(inner),
            DataFusionError::External(inner) => inner
                .downcast_ref::<RelationError>()
                .copied()
                .unwrap_or(RelationError::QueryFailed),
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
