use arrow::array::{Array, Float64Array, Int64Array, StringArray};
use arrow::datatypes::{DataType, Field};
use arrow::record_batch::RecordBatch;
use datafusion::common::{DataFusionError, ScalarValue};
use datafusion::dataframe::DataFrame;
use datafusion::functions_aggregate::expr_fn::{
    avg, count, count_distinct, max, median, min, stddev,
};
use datafusion::logical_expr::{
    Expr,
    expr_fn::{cast, when},
};
use datafusion::prelude::lit;
use futures_util::StreamExt;
use yss_dataset_profile::{
    CategoryCount, ColumnDistribution, ColumnStats, DEFAULT_HISTOGRAM_BIN_COUNT,
    DEFAULT_TOP_CATEGORY_COUNT, DataCompleteness, DatasetOverview, HistogramBin,
    NumericColumnStats, NumericDistribution, ProfileColumnKind, SchemaOverview, SizeShape,
    StringColumnStats, StringDistribution, format_histogram_bin_label,
};
use yss_relational_contract::{RelationControl, RelationError};

use crate::{DatasetQuery, dataset::column, relation::controlled};

fn plan(_: DataFusionError) -> RelationError {
    RelationError::InvalidPlan
}
fn count_all() -> Expr {
    count(lit(1_i64))
}
fn conditional_count(condition: Expr) -> Result<Expr, RelationError> {
    Ok(count(
        when(condition, lit(1_i64))
            .otherwise(lit(ScalarValue::Int64(None)))
            .map_err(plan)?,
    ))
}
fn kind(field: &Field) -> ProfileColumnKind {
    match field.data_type() {
        data_type if data_type.is_numeric() => ProfileColumnKind::Numeric,
        DataType::Dictionary(..) => ProfileColumnKind::Categorical,
        DataType::Date32
        | DataType::Date64
        | DataType::Timestamp(..)
        | DataType::Time32(..)
        | DataType::Time64(..)
        | DataType::Duration(..) => ProfileColumnKind::Temporal,
        DataType::Boolean => ProfileColumnKind::Boolean,
        _ => ProfileColumnKind::String,
    }
}
fn finite_value(name: &str) -> Result<Expr, RelationError> {
    let value = cast(column(None, name), DataType::Float64);
    let finite = value
        .clone()
        .gt_eq(lit(f64::MIN))
        .and(value.clone().lt_eq(lit(f64::MAX)));
    when(finite, value)
        .otherwise(lit(ScalarValue::Float64(None)))
        .map_err(plan)
}
fn integer(batch: &RecordBatch, col: usize, row: usize) -> Result<usize, RelationError> {
    let array = batch
        .column(col)
        .as_any()
        .downcast_ref::<Int64Array>()
        .ok_or(RelationError::InvalidInput)?;
    if row >= array.len() || array.is_null(row) {
        return Err(RelationError::InvalidInput);
    }
    usize::try_from(array.value(row)).map_err(|_| RelationError::InvalidInput)
}
fn number(batch: &RecordBatch, col: usize) -> Result<Option<f64>, RelationError> {
    let array = batch
        .column(col)
        .as_any()
        .downcast_ref::<Float64Array>()
        .ok_or(RelationError::InvalidInput)?;
    if array.is_empty() {
        return Err(RelationError::InvalidInput);
    }
    Ok((!array.is_null(0))
        .then(|| array.value(0))
        .filter(|value| value.is_finite()))
}

impl DatasetQuery {
    fn profile_fields(&self) -> Result<Vec<&Field>, RelationError> {
        let rows = yss_tabular_arrow::dataset_row_columns(&self.schema)
            .map_err(|_| RelationError::InvalidInput)?
            .ok_or(RelationError::InvalidInput)?;
        Ok(self
            .schema
            .fields()
            .iter()
            .filter(|field| field.name() != &rows.row_id && field.name() != &rows.display_order)
            .map(AsRef::as_ref)
            .collect())
    }

    // Only aggregate rows/top groups cross this boundary. The native plan scans and accounts
    // for aggregate memory under the same RuntimeEnv as pages and execution inputs.
    fn profile_rows(
        &self,
        frame: DataFrame,
        max_rows: usize,
        control: &RelationControl,
    ) -> Result<RecordBatch, RelationError> {
        let schema = std::sync::Arc::new(frame.schema().as_arrow().clone());
        self.engine
            .runtime
            .as_ref()
            .ok_or(RelationError::QueryFailed)?
            .block_on(async {
                let mut batches = Vec::new();
                let mut rows = 0usize;
                let mut bytes = 0usize;
                let mut stream = controlled(frame.execute_stream(), control)
                    .await?
                    .map_err(crate::relation::query_error)?;
                while let Some(batch) = controlled(stream.next(), control).await? {
                    let batch = batch.map_err(crate::relation::query_error)?;
                    rows = rows
                        .checked_add(batch.num_rows())
                        .ok_or(RelationError::MemoryLimitExceeded)?;
                    bytes = bytes
                        .checked_add(batch.get_array_memory_size())
                        .ok_or(RelationError::MemoryLimitExceeded)?;
                    if rows > max_rows || bytes > control.max_input_bytes {
                        return Err(RelationError::MemoryLimitExceeded);
                    }
                    batches.push(batch);
                }
                arrow::compute::concat_batches(&schema, &batches)
                    .map_err(|_| RelationError::QueryFailed)
            })
    }

    pub fn column_stats(
        &self,
        control: &RelationControl,
    ) -> Result<Vec<ColumnStats>, RelationError> {
        self.profile_fields()?
            .into_iter()
            .map(|field| {
                if kind(field) == ProfileColumnKind::Numeric {
                    self.numeric_stats(field, control)
                } else {
                    self.string_stats(field, control)
                }
            })
            .collect()
    }

    fn numeric_stats(
        &self,
        field: &Field,
        control: &RelationControl,
    ) -> Result<ColumnStats, RelationError> {
        let value = finite_value(field.name())?;
        let expressions = vec![
            count_all(),
            count(column(None, field.name())),
            min(value.clone()),
            max(value.clone()),
            avg(value.clone()),
            median(value.clone()),
            stddev(value),
        ];
        let frame = self
            .frame
            .clone()
            .aggregate(Vec::new(), expressions)
            .map_err(plan)?;
        let batch = self.profile_rows(frame, 1, control)?;
        let count = integer(&batch, 0, 0)?;
        let std = number(&batch, 6)?;
        Ok(ColumnStats::Numeric(NumericColumnStats {
            column_name: field.name().clone(),
            column_type: yss_tabular_arrow::data_type_name(field.data_type()),
            kind: "numeric",
            count,
            null_count: count
                .checked_sub(integer(&batch, 1, 0)?)
                .ok_or(RelationError::InvalidInput)?,
            min: number(&batch, 2)?,
            max: number(&batch, 3)?,
            mean: number(&batch, 4)?,
            median: number(&batch, 5)?,
            std,
            variance: std
                .map(|value| value * value)
                .filter(|value| value.is_finite()),
        }))
    }

    fn string_counts(
        &self,
        field: &Field,
        control: &RelationControl,
    ) -> Result<RecordBatch, RelationError> {
        let value = cast(column(None, field.name()), DataType::Utf8);
        let expressions = vec![
            count_all(),
            count(column(None, field.name())),
            conditional_count(value.clone().eq(lit("")))?,
            count_distinct(value),
        ];
        self.profile_rows(
            self.frame
                .clone()
                .aggregate(Vec::new(), expressions)
                .map_err(plan)?,
            1,
            control,
        )
    }

    fn top_categories(
        &self,
        field: &Field,
        limit: usize,
        control: &RelationControl,
    ) -> Result<Vec<CategoryCount>, RelationError> {
        let value = column(None, "value");
        let frame = self
            .frame
            .clone()
            .select([cast(column(None, field.name()), DataType::Utf8).alias("value")])
            .and_then(|frame| {
                frame.filter(
                    value
                        .clone()
                        .is_not_null()
                        .and(value.clone().not_eq(lit(""))),
                )
            })
            .and_then(|frame| {
                frame.aggregate(vec![value.clone()], vec![count_all().alias("frequency")])
            })
            .and_then(|frame| {
                frame.sort(vec![
                    column(None, "frequency").sort(false, false),
                    value.sort(true, false),
                ])
            })
            .and_then(|frame| frame.limit(0, Some(limit)))
            .map_err(plan)?;
        let batch = self.profile_rows(frame, limit, control)?;
        let labels = batch
            .column(0)
            .as_any()
            .downcast_ref::<StringArray>()
            .ok_or(RelationError::InvalidInput)?;
        (0..batch.num_rows())
            .map(|row| {
                Ok(CategoryCount {
                    label: labels.value(row).to_owned(),
                    value: integer(&batch, 1, row)?,
                })
            })
            .collect()
    }

    fn string_stats(
        &self,
        field: &Field,
        control: &RelationControl,
    ) -> Result<ColumnStats, RelationError> {
        let batch = self.string_counts(field, control)?;
        let count = integer(&batch, 0, 0)?;
        let non_null = integer(&batch, 1, 0)?;
        let empty_count = integer(&batch, 2, 0)?;
        let valid = non_null
            .checked_sub(empty_count)
            .ok_or(RelationError::InvalidInput)?;
        let mode = self.top_categories(field, 1, control)?.pop();
        Ok(ColumnStats::String(StringColumnStats {
            column_name: field.name().clone(),
            column_type: yss_tabular_arrow::data_type_name(field.data_type()),
            kind: "string",
            count,
            null_count: count
                .checked_sub(non_null)
                .ok_or(RelationError::InvalidInput)?,
            empty_count,
            valid_ratio: if count == 0 {
                0.0
            } else {
                valid as f64 / count as f64
            },
            unique: integer(&batch, 3, 0)?,
            mode_count: mode.as_ref().map_or(0, |mode| mode.value),
            mode: mode.map(|mode| mode.label),
        }))
    }

    pub fn column_distributions(
        &self,
        control: &RelationControl,
    ) -> Result<Vec<ColumnDistribution>, RelationError> {
        self.profile_fields()?
            .into_iter()
            .map(|field| {
                if kind(field) == ProfileColumnKind::Numeric {
                    self.numeric_distribution(field, control)
                } else {
                    let summary = self.string_counts(field, control)?;
                    let valid = integer(&summary, 1, 0)?
                        .checked_sub(integer(&summary, 2, 0)?)
                        .ok_or(RelationError::InvalidInput)?;
                    let categories =
                        self.top_categories(field, DEFAULT_TOP_CATEGORY_COUNT, control)?;
                    let top = categories.iter().try_fold(0usize, |sum, category| {
                        sum.checked_add(category.value)
                            .ok_or(RelationError::InvalidInput)
                    })?;
                    Ok(ColumnDistribution::String(StringDistribution {
                        column_name: field.name().clone(),
                        kind: "string",
                        categories,
                        other_count: valid.checked_sub(top).ok_or(RelationError::InvalidInput)?,
                    }))
                }
            })
            .collect()
    }

    fn numeric_distribution(
        &self,
        field: &Field,
        control: &RelationControl,
    ) -> Result<ColumnDistribution, RelationError> {
        let value = finite_value(field.name())?;
        let bounds = self
            .frame
            .clone()
            .aggregate(
                Vec::new(),
                vec![min(value.clone()), max(value.clone()), count(value.clone())],
            )
            .map_err(plan)?;
        let bounds = self.profile_rows(bounds, 1, control)?;
        let bins = match (number(&bounds, 0)?, number(&bounds, 1)?) {
            (Some(lo), Some(hi)) if (hi - lo).abs() < f64::EPSILON => vec![HistogramBin {
                label: format!("{lo:.2}"),
                count: integer(&bounds, 2, 0)?,
            }],
            (Some(lo), Some(hi)) => {
                let count = DEFAULT_HISTOGRAM_BIN_COUNT;
                // Convex interpolation keeps finite bounds finite even when hi - lo overflows.
                let bounds = (0..=count)
                    .map(|index| {
                        let t = index as f64 / count as f64;
                        lo * (1.0 - t) + hi * t
                    })
                    .collect::<Vec<_>>();
                let expressions = (0..count)
                    .map(|index| {
                        let upper = if index + 1 == count {
                            value.clone().lt_eq(lit(bounds[index + 1]))
                        } else {
                            value.clone().lt(lit(bounds[index + 1]))
                        };
                        Ok(
                            conditional_count(value.clone().gt_eq(lit(bounds[index])).and(upper))?
                                .alias(format!("bin_{index}")),
                        )
                    })
                    .collect::<Result<Vec<_>, RelationError>>()?;
                let counts = self.profile_rows(
                    self.frame
                        .clone()
                        .aggregate(Vec::new(), expressions)
                        .map_err(plan)?,
                    1,
                    control,
                )?;
                let precision = usize::from(hi / count as f64 - lo / (count as f64) < 1.0) + 1;
                (0..count)
                    .map(|index| {
                        Ok(HistogramBin {
                            label: format_histogram_bin_label(
                                bounds[index],
                                bounds[index + 1],
                                precision,
                                index + 1 == count,
                            ),
                            count: integer(&counts, index, 0)?,
                        })
                    })
                    .collect::<Result<Vec<_>, RelationError>>()?
            }
            _ => Vec::new(),
        };
        Ok(ColumnDistribution::Numeric(NumericDistribution {
            column_name: field.name().clone(),
            kind: "numeric",
            bins,
        }))
    }

    pub fn dataset_overview(
        &self,
        control: &RelationControl,
    ) -> Result<DatasetOverview, RelationError> {
        let fields = self.profile_fields()?;
        let mut schema = SchemaOverview {
            numeric_cols: 0,
            categorical_cols: 0,
            string_cols: 0,
            datetime_cols: 0,
            bool_cols: 0,
        };
        let mut expressions = vec![count_all().alias("rows")];
        let mut any_null = lit(false);
        for (index, field) in fields.iter().enumerate() {
            match kind(field) {
                ProfileColumnKind::Numeric => schema.numeric_cols += 1,
                ProfileColumnKind::Categorical => schema.categorical_cols += 1,
                ProfileColumnKind::String => schema.string_cols += 1,
                ProfileColumnKind::Temporal => schema.datetime_cols += 1,
                ProfileColumnKind::Boolean => schema.bool_cols += 1,
            }
            let null = column(None, field.name()).is_null();
            expressions.push(conditional_count(null.clone())?.alias(format!("null_{index}")));
            any_null = any_null.or(null);
        }
        expressions.push(conditional_count(any_null)?.alias("rows_with_nulls"));
        let batch = self.profile_rows(
            self.frame
                .clone()
                .aggregate(Vec::new(), expressions)
                .map_err(plan)?,
            1,
            control,
        )?;
        let n_rows = integer(&batch, 0, 0)?;
        let mut total_nulls = 0usize;
        let mut cols_with_nulls = 0;
        for index in 0..fields.len() {
            let nulls = integer(&batch, index + 1, 0)?;
            total_nulls = total_nulls
                .checked_add(nulls)
                .ok_or(RelationError::InvalidInput)?;
            cols_with_nulls += usize::from(nulls > 0);
        }
        let cells = n_rows
            .checked_mul(fields.len())
            .ok_or(RelationError::InvalidInput)?;
        Ok(DatasetOverview {
            size_shape: SizeShape {
                n_rows,
                n_columns: fields.len(),
                estimated_dataframe_memory_bytes: None,
                duplicated_rows: None,
            },
            schema_overview: schema,
            data_completeness: DataCompleteness {
                total_nulls,
                null_ratio: if cells == 0 {
                    0.0
                } else {
                    total_nulls as f64 / cells as f64
                },
                cols_with_nulls,
                rows_with_nulls: integer(&batch, fields.len() + 1, 0)?,
            },
        })
    }
}
