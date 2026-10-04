//! Native, budgeted aggregate plans. No row scans occur while constructing these plans.
use crate::{dataset::column, relation::DataFusionRelation};
use arrow::array::{Array, Float64Array};
use arrow::datatypes::{DataType, Field, Schema};
use datafusion::common::{DataFusionError, ScalarValue};
use datafusion::dataframe::DataFrame;
use datafusion::functions_aggregate::expr_fn::{avg, count, count_distinct, max, min, stddev, sum};
use datafusion::functions_aggregate::percentile_cont::percentile_cont;
use datafusion::logical_expr::{
    ColumnarValue, Expr, JoinType, Volatility, create_udf,
    expr_fn::{cast, when},
};
use datafusion::prelude::lit;
use std::{collections::BTreeSet, sync::Arc};
use yss_data_contract::{
    SemanticType,
    aggregation::{AggregateOperation, ColumnAggregate, DESCRIPTION_FIELDS, supports_description},
};
use yss_relational_contract::{RelationError, RelationHandle};

fn plan(_: DataFusionError) -> RelationError {
    RelationError::InvalidPlan
}
fn col(name: &str) -> Expr {
    column(None, name)
}
fn null(dtype: DataType) -> Expr {
    lit(ScalarValue::try_from(&dtype).expect("scalar type"))
}
fn rows() -> Expr {
    count(lit(1_i64))
}

/// Null is legitimate missing data; overflow and inexact numeric conversion are failures.
fn finite(value: Expr, dtype: &DataType) -> Expr {
    create_udf(
        "yssbi_aggregate_finite",
        vec![dtype.clone()],
        DataType::Float64,
        Volatility::Immutable,
        Arc::new(|args| {
            let arrays = ColumnarValue::values_to_arrays(args)?;
            let array =
                yss_database_arrow::lossless_cast(arrays[0].as_ref(), &DataType::Float64, false)
                    .map_err(|_| {
                        DataFusionError::External(Box::new(RelationError::InvalidInput))
                    })?;
            let values = array
                .as_any()
                .downcast_ref::<Float64Array>()
                .ok_or_else(|| DataFusionError::External(Box::new(RelationError::InvalidInput)))?;
            if values.iter().flatten().any(|value| !value.is_finite()) {
                return Err(DataFusionError::External(Box::new(
                    RelationError::NonFiniteResult,
                )));
            }
            if matches!(args[0], ColumnarValue::Scalar(_)) {
                Ok(ColumnarValue::Scalar(ScalarValue::try_from_array(
                    array.as_ref(),
                    0,
                )?))
            } else {
                Ok(ColumnarValue::Array(array))
            }
        }),
    )
    .call(vec![value])
}

fn checked(value: Expr) -> Expr {
    finite(value, &DataType::Float64)
}
fn quantile(value: Expr, probability: f64) -> Expr {
    percentile_cont(value.sort(true, false), lit(probability))
}
fn semantic(field: &Field) -> Result<SemanticType, RelationError> {
    yss_database_arrow::column_semantic(field)
        .map(|s| s.kind)
        .map_err(|_| RelationError::InvalidInput)
}

fn order(field: &Field, name: &str) -> Result<Expr, RelationError> {
    let metadata =
        yss_database_arrow::column_semantic(field).map_err(|_| RelationError::InvalidInput)?;
    if metadata.kind != SemanticType::Ordinal {
        return Ok(col(name));
    }
    let mut rank = null(DataType::Int64);
    for (index, level) in metadata.values.iter().enumerate().rev() {
        rank = when(
            cast(col(name), DataType::Utf8).eq(lit(level.value.clone())),
            lit(index as i64),
        )
        .otherwise(rank)
        .map_err(plan)?;
    }
    Ok(rank)
}

impl DataFusionRelation {
    fn aggregate_result(
        &self,
        frame: DataFrame,
        fields: Option<Vec<Field>>,
        order: Vec<datafusion::logical_expr::expr::Sort>,
    ) -> Result<RelationHandle, RelationError> {
        let schema = Arc::new(match fields {
            Some(fields) => Schema::new(fields),
            None => frame.schema().as_arrow().clone(),
        });
        Self::new(
            frame,
            schema,
            self.bindings.clone(),
            self.lease.clone(),
            self.executor.clone(),
            false,
            order,
        )
        .and_then(Self::into_handle)
    }

    pub(crate) fn group_aggregate(
        &self,
        keys: &[Box<str>],
        columns: &[ColumnAggregate],
    ) -> Result<RelationHandle, RelationError> {
        if keys.is_empty() {
            return Err(RelationError::InvalidInput);
        }
        let mut names = BTreeSet::new();
        let mut fields = Vec::new();
        let mut groups = Vec::new();
        let mut sorting = Vec::new();
        for key in keys {
            if !names.insert(key.to_string()) {
                return Err(RelationError::InvalidInput);
            }
            let field = self
                .schema
                .field_with_name(key)
                .map_err(|_| RelationError::InvalidInput)?;
            fields.push(field.clone());
            groups.push(col(key));
            sorting.push(order(field, key)?.sort(true, false));
        }
        if !names.insert("row_count".into()) {
            return Err(RelationError::InvalidInput);
        }
        fields.push(Field::new("row_count", DataType::Int64, false));
        let mut aggregates = vec![rows().alias("row_count")];
        for item in columns {
            let field = self
                .schema
                .field_with_name(&item.column)
                .map_err(|_| RelationError::InvalidInput)?;
            if !item.operation.accepts(semantic(field)?) {
                return Err(RelationError::InvalidInput);
            }
            let name = item.operation.output_name(&item.column);
            if !names.insert(name.clone()) {
                return Err(RelationError::InvalidInput);
            }
            let value = if item.operation == AggregateOperation::Count {
                col(&item.column)
            } else {
                finite(col(&item.column), field.data_type())
            };
            let expression = match item.operation {
                AggregateOperation::Count => count(value),
                AggregateOperation::Sum => checked(sum(value)),
                AggregateOperation::Mean => checked(avg(value)),
                AggregateOperation::Min => checked(min(value)),
                AggregateOperation::Max => checked(max(value)),
                AggregateOperation::Std => checked(stddev(value)),
                AggregateOperation::Median => checked(quantile(value, 0.5)),
            };
            fields.push(Field::new(
                &name,
                if item.operation == AggregateOperation::Count {
                    DataType::Int64
                } else {
                    DataType::Float64
                },
                true,
            ));
            aggregates.push(expression.alias(name));
        }
        let frame = self
            .frame
            .clone()
            .aggregate(groups, aggregates)
            .and_then(|frame| frame.sort(sorting.clone()))
            .map_err(plan)?;
        self.aggregate_result(frame, Some(fields), sorting)
    }

    pub(crate) fn frequency_table(
        &self,
        name: &str,
        include_null: bool,
    ) -> Result<RelationHandle, RelationError> {
        let field = self
            .schema
            .field_with_name(name)
            .map_err(|_| RelationError::InvalidInput)?;
        if !supports_description(semantic(field)?) {
            return Err(RelationError::InvalidInput);
        }
        let mut source = self
            .frame
            .clone()
            .select([col(name).alias("value")])
            .map_err(plan)?;
        if !include_null {
            source = source.filter(col("value").is_not_null()).map_err(plan)?;
        }
        let total = source
            .clone()
            .aggregate(vec![], vec![rows().alias("total")])
            .map_err(plan)?;
        let counts = source
            .aggregate(vec![col("value")], vec![rows().alias("frequency")])
            .map_err(plan)?;
        let sorting = vec![order(field, "value")?.sort(true, false)];
        let frame = counts
            .join_on(total, JoinType::Inner, [lit(true)])
            .and_then(|frame| {
                frame.select([
                    col("value"),
                    col("frequency"),
                    (cast(col("frequency"), DataType::Float64)
                        / cast(col("total"), DataType::Float64))
                    .alias("proportion"),
                ])
            })
            .and_then(|frame| frame.sort(sorting.clone()))
            .map_err(plan)?;
        self.aggregate_result(
            frame,
            Some(vec![
                field.clone().with_name("value"),
                Field::new("frequency", DataType::Int64, false),
                Field::new("proportion", DataType::Float64, false),
            ]),
            sorting,
        )
    }

    pub(crate) fn describe_columns(&self) -> Result<RelationHandle, RelationError> {
        let mut fields = Vec::new();
        for field in self.schema.fields() {
            if supports_description(semantic(field)?) {
                fields.push(field.as_ref());
            }
        }
        if fields.is_empty() {
            return Err(RelationError::InvalidInput);
        }
        let mut combined: Option<DataFrame> = None;
        for (index, field) in fields.into_iter().enumerate() {
            let kind = semantic(field)?;
            let mut expressions = vec![
                count(col(field.name())).alias("count"),
                (rows() - count(col(field.name()))).alias("missing"),
            ];
            let numeric = kind == SemanticType::Numeric;
            if numeric {
                let value = finite(col(field.name()), field.data_type());
                expressions.extend([
                    checked(avg(value.clone())).alias("mean"),
                    checked(stddev(value.clone())).alias("std"),
                    checked(min(value.clone())).alias("min"),
                    checked(quantile(value.clone(), 0.25)).alias("q25"),
                    checked(quantile(value.clone(), 0.5)).alias("median"),
                    checked(quantile(value.clone(), 0.75)).alias("q75"),
                    checked(max(value)).alias("max"),
                ]);
            } else {
                expressions.push(count_distinct(col(field.name())).alias("unique"));
            }
            let summary = self
                .frame
                .clone()
                .aggregate(vec![], expressions)
                .map_err(plan)?;
            let projection = DESCRIPTION_FIELDS
                .iter()
                .map(|(name, _)| {
                    let value = match *name {
                        "column" => lit(field.name().clone()),
                        "semantic" => lit(kind.as_str()),
                        "count" | "missing" => col(name),
                        "unique" if numeric => null(DataType::Int64),
                        "unique" => col(name),
                        _ if numeric => col(name),
                        _ => null(DataType::Float64),
                    };
                    value.alias(*name)
                })
                .chain(std::iter::once(
                    lit(index as i64).alias("__description_order"),
                ))
                .collect::<Vec<_>>();
            let summary = summary.select(projection).map_err(plan)?;
            combined = Some(match combined {
                Some(frame) => frame.union(summary).map_err(plan)?,
                None => summary,
            });
        }
        let frame = combined
            .ok_or(RelationError::InvalidInput)?
            .sort(vec![col("__description_order").sort(true, false)])
            .and_then(|frame| frame.select(DESCRIPTION_FIELDS.iter().map(|(name, _)| col(name))))
            .map_err(plan)?;
        // Each row is already ordered by the selected source-column index, not by its label.
        self.aggregate_result(frame, None, vec![])
    }
}
