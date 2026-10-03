//! Native scalar and window expressions over an unchanged, proven row domain.
use crate::{
    relation::DataFusionRelation,
    series::{DataFusionSeries, expression},
};
use arrow::{
    array::{Array, Float64Array},
    datatypes::{DataType, Field, Schema},
};
use datafusion::functions::expr_fn as scalar;
use datafusion::functions_aggregate::expr_fn as aggregate;
use datafusion::functions_window::expr_fn as window;
use datafusion::{
    common::{Column, DataFusionError, ScalarValue},
    logical_expr::{
        ColumnarValue, Expr, ExprFunctionExt, Volatility, WindowFrame, WindowFrameBound,
        WindowFrameUnits, create_udf,
        expr::{NullTreatment, Sort, WindowFunction},
        expr_fn::{cast, when},
    },
    prelude::lit,
};
use std::sync::Arc;
use yss_data_contract::{ColumnSemantic, SemanticType, TabularScalar};
use yss_relational_contract::*;

pub(crate) fn col(name: &str) -> Expr {
    Expr::Column(Column::from_name(name.to_owned()))
}
pub(crate) fn plan(_: DataFusionError) -> RelationError {
    RelationError::InvalidPlan
}
pub(crate) fn null(dtype: &DataType) -> Result<Expr, RelationError> {
    Ok(lit(ScalarValue::try_from(dtype).map_err(plan)?))
}
pub(crate) fn field(
    name: &str,
    dtype: DataType,
    semantic: SemanticType,
    nullable: bool,
) -> Result<Field, RelationError> {
    let field = Field::new(name, dtype, nullable);
    let inferred =
        yss_database_arrow::column_semantic(&field).map_err(|_| RelationError::InvalidInput)?;
    yss_database_arrow::with_column_semantic(
        field,
        &if inferred.kind == semantic {
            inferred
        } else {
            ColumnSemantic::new(semantic)
        },
    )
    .map_err(|_| RelationError::InvalidInput)
}

pub(crate) fn literal(value: &TabularScalar, dtype: &DataType) -> Result<Expr, RelationError> {
    let scalar = match value {
        TabularScalar::Null => return null(dtype),
        TabularScalar::Bool(v) => ScalarValue::Boolean(Some(*v)),
        TabularScalar::Integer(v) => ScalarValue::Int64(Some(*v)),
        TabularScalar::Unsigned(v) => ScalarValue::UInt64(Some(*v)),
        TabularScalar::Float64(v) => ScalarValue::Float64(Some(v.as_f64())),
        TabularScalar::String(v) => ScalarValue::Utf8(Some(v.to_string())),
    };
    let array = scalar.to_array_of_size(1).map_err(plan)?;
    let array = yss_database_arrow::lossless_cast(array.as_ref(), dtype, false)
        .map_err(|_| RelationError::InvalidInput)?;
    Ok(lit(
        ScalarValue::try_from_array(array.as_ref(), 0).map_err(plan)?
    ))
}

pub(crate) fn ordered_value(value: Expr, field: &Field) -> Result<Expr, RelationError> {
    let semantic =
        yss_database_arrow::column_semantic(field).map_err(|_| RelationError::InvalidInput)?;
    if semantic.kind != SemanticType::Ordinal {
        return Ok(value);
    }
    if semantic.values.is_empty() {
        return Err(RelationError::InvalidInput);
    }
    let mut rank = null(&DataType::Int64)?;
    for (index, level) in semantic.values.iter().enumerate().rev() {
        rank = when(
            value.clone().eq(literal(
                &TabularScalar::String(level.value.clone().into()),
                field.data_type(),
            )?),
            lit(index as i64),
        )
        .otherwise(rank)
        .map_err(plan)?;
    }
    Ok(rank)
}

pub(crate) fn truncate_date(
    value: Expr,
    dtype: &DataType,
    unit: &str,
) -> Result<Expr, RelationError> {
    let timestamp = match dtype {
        DataType::Timestamp(unit, None) => DataType::Timestamp(*unit, None),
        DataType::Date32 | DataType::Date64 => {
            DataType::Timestamp(arrow::datatypes::TimeUnit::Millisecond, None)
        }
        _ => return Err(RelationError::InvalidInput),
    };
    Ok(cast(
        scalar::date_trunc(lit(unit), cast(value, timestamp)),
        dtype.clone(),
    ))
}

/// Batch validation remains an expression in the engine; it never collects an input column.
pub(crate) fn finite(value: Expr, dtype: &DataType, positive: bool) -> Expr {
    create_udf(
        if positive {
            "yssbi_positive_scale"
        } else {
            "yssbi_nullable_finite"
        },
        vec![dtype.clone()],
        DataType::Float64,
        Volatility::Immutable,
        Arc::new(move |args| {
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
            for value in values.iter() {
                if positive && value.is_none_or(|v| v <= 0.0) {
                    return Err(DataFusionError::External(Box::new(
                        RelationError::InvalidInput,
                    )));
                }
                if value.is_some_and(|v| !v.is_finite()) {
                    return Err(DataFusionError::External(Box::new(
                        RelationError::NonFiniteResult,
                    )));
                }
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

pub(crate) fn checked(
    value: Expr,
    condition: Expr,
    dtype: &DataType,
    error: RelationError,
) -> Expr {
    create_udf(
        "yssbi_transform_checked",
        vec![dtype.clone(), DataType::Boolean],
        dtype.clone(),
        Volatility::Immutable,
        Arc::new(move |args| {
            let arrays = ColumnarValue::values_to_arrays(args)?;
            let conditions = arrays[1]
                .as_any()
                .downcast_ref::<arrow::array::BooleanArray>()
                .ok_or_else(|| DataFusionError::External(Box::new(RelationError::InvalidInput)))?;
            if conditions.iter().any(|v| v != Some(true)) {
                return Err(DataFusionError::External(Box::new(error)));
            }
            Ok(if matches!(args[0], ColumnarValue::Scalar(_)) {
                ColumnarValue::Scalar(ScalarValue::try_from_array(arrays[0].as_ref(), 0)?)
            } else {
                ColumnarValue::Array(arrays[0].clone())
            })
        }),
    )
    .call(vec![value, condition])
}

pub(crate) fn over(
    expr: Expr,
    partition: Vec<Expr>,
    order: Vec<Sort>,
    frame: WindowFrame,
) -> Result<Expr, RelationError> {
    let mut window = match expr {
        Expr::AggregateFunction(function) => {
            let mut win = WindowFunction::new(function.func, function.params.args);
            win.params.null_treatment = function.params.null_treatment;
            win.params.filter = function.params.filter;
            win.params.distinct = function.params.distinct;
            win
        }
        Expr::WindowFunction(window) => *window,
        _ => return Err(RelationError::InvalidInput),
    };
    window.params.partition_by = partition;
    window.params.order_by = order;
    window.params.window_frame = frame;
    Ok(Expr::WindowFunction(Box::new(window)))
}
pub(crate) fn whole() -> WindowFrame {
    WindowFrame::new_bounds(
        WindowFrameUnits::Rows,
        WindowFrameBound::Preceding(ScalarValue::UInt64(None)),
        WindowFrameBound::Following(ScalarValue::UInt64(None)),
    )
}
fn prefix() -> WindowFrame {
    WindowFrame::new_bounds(
        WindowFrameUnits::Rows,
        WindowFrameBound::Preceding(ScalarValue::UInt64(None)),
        WindowFrameBound::CurrentRow,
    )
}

impl DataFusionRelation {
    pub(crate) fn series_expression(&self, series: &SeriesHandle) -> Result<Expr, RelationError> {
        let owner = series
            .relation()
            .plan()
            .as_any()
            .downcast_ref::<Self>()
            .ok_or(RelationError::InvalidInput)?;
        if !Arc::ptr_eq(&self.domain, &owner.domain) {
            return Err(RelationError::UnalignedSeries);
        }
        expression(series)
    }
    pub(crate) fn operand(
        &self,
        operand: &SeriesOperand,
        dtype: &DataType,
    ) -> Result<Expr, RelationError> {
        match operand {
            SeriesOperand::Scalar(value) => literal(value, dtype),
            SeriesOperand::Series(series) if series.plan().field().data_type() == dtype => {
                self.series_expression(series)
            }
            _ => Err(RelationError::InvalidInput),
        }
    }
    pub(crate) fn compatible_operand(
        &self,
        operand: &SeriesOperand,
        field: &Field,
    ) -> Result<Expr, RelationError> {
        if let SeriesOperand::Series(series) = operand
            && yss_database_arrow::column_semantic(field)
                .map_err(|_| RelationError::InvalidInput)?
                != yss_database_arrow::column_semantic(series.plan().field())
                    .map_err(|_| RelationError::InvalidInput)?
        {
            return Err(RelationError::InvalidInput);
        }
        let expression = self.operand(operand, field.data_type())?;
        if let Expr::Literal(value, _) = &expression {
            yss_database_arrow::validate_semantic_array(
                field,
                value.to_array_of_size(1).map_err(plan)?.as_ref(),
            )
            .map_err(|_| RelationError::InvalidInput)?;
        }
        Ok(expression)
    }
    pub(crate) fn window_keys(
        &self,
        spec: &SeriesWindow,
    ) -> Result<(Vec<Expr>, Vec<Sort>), RelationError> {
        let owner = if let Some(context) = &spec.context {
            let context = context
                .plan()
                .as_any()
                .downcast_ref::<Self>()
                .ok_or(RelationError::InvalidInput)?;
            if !Arc::ptr_eq(&self.domain, &context.domain) {
                return Err(RelationError::UnalignedSeries);
            }
            context
        } else {
            self
        };
        let value = |name: &str| {
            owner
                .schema
                .index_of(name)
                .map(|i| owner.columns[i].clone())
                .map_err(|_| RelationError::InvalidInput)
        };
        let partition = spec
            .partition_by
            .iter()
            .map(|name| value(name))
            .collect::<Result<Vec<_>, _>>()?;
        let mut order = spec
            .order_by
            .iter()
            .map(|s| {
                let field = owner
                    .schema
                    .field_with_name(&s.column)
                    .map_err(|_| RelationError::InvalidInput)?;
                Ok(ordered_value(value(&s.column)?, field)?.sort(s.ascending, s.nulls_first))
            })
            .collect::<Result<Vec<_>, _>>()?;
        order.extend(self.domain_order.clone());
        Ok((partition, order))
    }
    pub(crate) fn transformed_series(
        &self,
        series: &SeriesHandle,
        operation: &SeriesTransform,
    ) -> Result<Arc<dyn SeriesPlan>, RelationError> {
        let source = series.plan().field();
        let mut output = crate::composition::derived_field(source, "result").with_nullable(true);
        let mut value = self.series_expression(series)?;
        let numeric = || {
            if yss_database_arrow::is_numeric_field(source) {
                Ok(finite(value.clone(), source.data_type(), false))
            } else {
                Err(RelationError::InvalidInput)
            }
        };
        use SeriesTransform::*;
        let expression = match operation {
            Impute { method } => {
                output = field("result", DataType::Float64, SemanticType::Numeric, false)?;
                crate::imputation::numeric(numeric()?, *method)?
            }
            IsNull { invert } => {
                output = field("result", DataType::Boolean, SemanticType::Binary, false)?;
                if *invert {
                    value.is_not_null()
                } else {
                    value.is_null()
                }
            }
            Fill { replacement } => {
                scalar::coalesce(vec![value, self.compatible_operand(replacement, source)?])
            }
            Map {
                from,
                to,
                keep_unmatched,
            } => {
                if from.is_empty()
                    || from.len() != to.len()
                    || from.iter().enumerate().any(|(i, v)| from[..i].contains(v))
                {
                    return Err(RelationError::InvalidInput);
                }
                let codes = from
                    .iter()
                    .zip(to)
                    .map(|(from, to)| {
                        Ok((
                            literal_code(from, source.data_type())?,
                            literal_code(to, source.data_type())?,
                        ))
                    })
                    .collect::<Result<Vec<_>, RelationError>>()?;
                if codes.iter().enumerate().any(|(index, (code, _))| {
                    codes[..index].iter().any(|(previous, _)| previous == code)
                }) {
                    return Err(RelationError::InvalidInput);
                }
                let mut result = if *keep_unmatched {
                    value.clone()
                } else {
                    null(source.data_type())?
                };
                for (from, to) in from.iter().zip(to).rev() {
                    let condition = if *from == TabularScalar::Null {
                        value.clone().is_null()
                    } else {
                        value.clone().eq(literal(from, source.data_type())?)
                    };
                    result = when(condition, literal(to, source.data_type())?)
                        .otherwise(result)
                        .map_err(plan)?;
                }
                let mut semantic = yss_database_arrow::column_semantic(source)
                    .map_err(|_| RelationError::InvalidInput)?;
                let remap = |code: &str| {
                    codes
                        .iter()
                        .find(|(from, _)| from.as_deref() == Some(code))
                        .map(|(_, to)| to.clone())
                        .unwrap_or_else(|| keep_unmatched.then(|| code.to_owned()))
                };
                semantic.values = semantic
                    .values
                    .into_iter()
                    .filter_map(|mut level| {
                        let code = remap(&level.value)?;
                        if code != level.value {
                            level.label = code.clone();
                        }
                        level.value = code;
                        Some(level)
                    })
                    .collect();
                if matches!(
                    semantic.kind,
                    SemanticType::Categorical | SemanticType::Ordinal | SemanticType::Binary
                ) {
                    for (_, to) in &codes {
                        if let Some(code) = to
                            && !semantic.values.iter().any(|level| &level.value == code)
                        {
                            semantic.values.push(yss_data_contract::SemanticValue {
                                value: code.clone(),
                                label: code.clone(),
                            });
                        }
                    }
                }
                let positive = semantic.positive_value.as_deref().or_else(|| {
                    (semantic.kind == SemanticType::Binary
                        && source.data_type() == &DataType::Boolean)
                        .then_some("true")
                });
                semantic.positive_value = positive.and_then(remap);
                semantic.numeric = None;
                let mut seen = std::collections::BTreeSet::new();
                semantic
                    .values
                    .retain(|level| seen.insert(level.value.clone()));
                output = yss_database_arrow::with_column_semantic(output, &semantic)
                    .map_err(|_| RelationError::InvalidInput)?;
                let mut metadata = output.metadata().clone();
                if let Some(base) = metadata
                    .remove("yssbi.dummy_base_level")
                    .and_then(|base| remap(&base))
                {
                    metadata.insert("yssbi.dummy_base_level".into(), base);
                }
                output = output.with_metadata(metadata);
                result
            }
            Trim | Lower | Upper | Replace { .. } | Substring { .. } | SplitPart { .. } => {
                if yss_database_arrow::column_semantic(source)
                    .map_err(|_| RelationError::InvalidInput)?
                    .kind
                    != SemanticType::Text
                {
                    return Err(RelationError::InvalidInput);
                }
                output = field("result", DataType::Utf8, SemanticType::Text, true)?;
                value = cast(value, DataType::Utf8);
                match operation {
                    Trim => scalar::btrim(vec![value]),
                    Lower => scalar::lower(value),
                    Upper => scalar::upper(value),
                    Replace { from, to } => {
                        scalar::replace(value, lit(from.as_ref()), lit(to.as_ref()))
                    }
                    Substring { start, length } if *start > 0 && *length >= 0 => {
                        scalar::substring(value, lit(*start), lit(*length))
                    }
                    SplitPart { separator, part } if !separator.is_empty() && *part != 0 => {
                        scalar::split_part(value, lit(separator.as_ref()), lit(*part))
                    }
                    _ => return Err(RelationError::InvalidInput),
                }
            }
            DatePart { part }
            | DateTruncate { unit: part }
            | DateAdd { unit: part, .. }
            | DateDifference { unit: part, .. } => {
                if yss_database_arrow::column_semantic(source)
                    .map_err(|_| RelationError::InvalidInput)?
                    .kind
                    != SemanticType::Datetime
                {
                    return Err(RelationError::InvalidInput);
                }
                match operation {
                    DatePart { .. } => {
                        output = field("result", DataType::Float64, SemanticType::Numeric, true)?;
                        cast(
                            scalar::date_part(lit(part.as_ref()), value),
                            DataType::Float64,
                        )
                    }
                    DateTruncate { .. } => truncate_date(value, source.data_type(), part)?,
                    DateAdd { amount, .. } => {
                        let tick = temporal_tick_nanos(source.data_type())?;
                        if source.data_type() == &DataType::Date32
                            && !matches!(part.as_ref(), "year" | "month" | "week" | "day")
                            && *amount != 0
                        {
                            return Err(RelationError::InvalidInput);
                        }
                        if !matches!(part.as_ref(), "year" | "month")
                            && (i128::from(*amount) * i128::from(duration_nanos(part)?))
                                % i128::from(tick)
                                != 0
                        {
                            return Err(RelationError::InvalidInput);
                        }
                        let interval = interval(part, *amount)?;
                        cast(value + lit(interval), source.data_type().clone())
                    }
                    DateDifference { other, .. } => {
                        let divisor = duration_nanos(part)?;
                        let other = match other {
                            SeriesOperand::Series(other) => {
                                if yss_database_arrow::column_semantic(other.plan().field())
                                    .map_err(|_| RelationError::InvalidInput)?
                                    .kind
                                    != SemanticType::Datetime
                                {
                                    return Err(RelationError::InvalidInput);
                                }
                                temporal_nanos(
                                    self.series_expression(other)?,
                                    other.plan().field().data_type(),
                                )?
                            }
                            _ => temporal_nanos(
                                self.operand(other, source.data_type())?,
                                source.data_type(),
                            )?,
                        };
                        output = field("result", DataType::Float64, SemanticType::Numeric, true)?;
                        cast(
                            temporal_nanos(value, source.data_type())? - other,
                            DataType::Float64,
                        ) / lit(divisor as f64)
                    }
                    _ => unreachable!(),
                }
            }
            Clip { lower, upper } => {
                if !lower.is_finite() || !upper.is_finite() || lower > upper {
                    return Err(RelationError::InvalidInput);
                }
                let x = numeric()?;
                output = field("result", DataType::Float64, SemanticType::Numeric, true)?;
                when(x.clone().lt(lit(*lower)), lit(*lower))
                    .when(x.clone().gt(lit(*upper)), lit(*upper))
                    .otherwise(x)
                    .map_err(plan)?
            }
            Bin { edges, labels } => {
                if edges.iter().any(|v| !v.is_finite())
                    || edges.windows(2).any(|v| v[0] >= v[1])
                    || labels.len() != edges.len() + 1
                    || labels
                        .iter()
                        .collect::<std::collections::BTreeSet<_>>()
                        .len()
                        != labels.len()
                {
                    return Err(RelationError::InvalidInput);
                }
                let x = numeric()?;
                let mut semantic = ColumnSemantic::new(SemanticType::Ordinal);
                semantic.values = labels
                    .iter()
                    .map(|name| yss_data_contract::SemanticValue {
                        value: name.to_string(),
                        label: name.to_string(),
                    })
                    .collect();
                output = yss_database_arrow::with_column_semantic(
                    Field::new("result", DataType::Utf8, true),
                    &semantic,
                )
                .map_err(|_| RelationError::InvalidInput)?;
                let mut result = lit(labels.last().ok_or(RelationError::InvalidInput)?.as_ref());
                for (edge, label) in edges.iter().zip(labels).rev() {
                    result = when(x.clone().lt(lit(*edge)), lit(label.as_ref()))
                        .otherwise(result)
                        .map_err(plan)?;
                }
                when(x.is_null(), null(&DataType::Utf8)?)
                    .otherwise(result)
                    .map_err(plan)?
            }
            Standardize => {
                let x = numeric()?;
                let mean = over(aggregate::avg(x.clone()), vec![], vec![], whole())?;
                let sd = finite(
                    over(aggregate::stddev(x.clone()), vec![], vec![], whole())?,
                    &DataType::Float64,
                    true,
                );
                output = field("result", DataType::Float64, SemanticType::Numeric, true)?;
                finite((x - mean) / sd, &DataType::Float64, false)
            }
            InverseStandardize {
                mean,
                standard_deviation,
            } => {
                if !mean.is_finite()
                    || !standard_deviation.is_finite()
                    || *standard_deviation <= 0.0
                {
                    return Err(RelationError::InvalidInput);
                }
                output = field("result", DataType::Float64, SemanticType::Numeric, true)?;
                finite(
                    numeric()? * lit(*standard_deviation) + lit(*mean),
                    &DataType::Float64,
                    false,
                )
            }
            DummyInformation { base_level } => {
                let semantic = yss_database_arrow::column_semantic(source)
                    .map_err(|_| RelationError::InvalidInput)?;
                if !matches!(
                    semantic.kind,
                    SemanticType::Categorical | SemanticType::Ordinal | SemanticType::Binary
                ) {
                    return Err(RelationError::InvalidInput);
                }
                let mut metadata = output.metadata().clone();
                metadata.insert("yssbi.dummy_base_level".into(), base_level.to_string());
                output = output.with_metadata(metadata);
                if base_level.is_empty() {
                    value
                } else {
                    let base = literal(
                        &TabularScalar::String(base_level.clone()),
                        source.data_type(),
                    )?;
                    let exists = over(
                        aggregate::sum(
                            when(value.clone().eq(base), lit(1_i64))
                                .otherwise(lit(0_i64))
                                .map_err(plan)?,
                        ),
                        vec![],
                        vec![],
                        whole(),
                    )?
                    .gt(lit(0_i64));
                    checked(
                        value,
                        exists,
                        source.data_type(),
                        RelationError::InvalidInput,
                    )
                }
            }
            Difference { window: spec, .. }
            | PercentChange { window: spec, .. }
            | Shift { window: spec, .. }
            | Rolling { window: spec, .. }
            | Cumulative { window: spec, .. }
            | Rank { window: spec, .. }
            | FillDirection { window: spec, .. } => {
                if matches!(
                    operation,
                    Difference { .. } | PercentChange { .. } | Rolling { .. } | Cumulative { .. }
                ) && !yss_database_arrow::is_numeric_field(source)
                {
                    return Err(RelationError::InvalidInput);
                }
                let (partition, order) = self.window_keys(spec)?;
                if spec.require_unique_keys {
                    let owner = spec
                        .context
                        .as_ref()
                        .and_then(|r| r.plan().as_any().downcast_ref::<Self>())
                        .unwrap_or(self);
                    let keys = spec
                        .partition_by
                        .iter()
                        .chain(spec.order_by.iter().map(|s| &s.column))
                        .map(|name| {
                            owner
                                .schema
                                .index_of(name)
                                .map(|i| owner.columns[i].clone())
                                .map_err(|_| RelationError::InvalidInput)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let valid = keys
                        .iter()
                        .cloned()
                        .map(Expr::is_not_null)
                        .reduce(Expr::and)
                        .ok_or(RelationError::InvalidInput)?
                        .and(
                            over(aggregate::count(lit(1_i64)), keys, vec![], whole())?
                                .eq(lit(1_i64)),
                        );
                    value = checked(
                        value,
                        valid,
                        source.data_type(),
                        RelationError::InvalidInput,
                    );
                }
                let lag = |x: Expr, n: usize, lead: bool| -> Result<Expr, RelationError> {
                    let n = i64::try_from(n).map_err(|_| RelationError::InvalidInput)?;
                    if n <= 0 {
                        return Err(RelationError::InvalidInput);
                    }
                    over(
                        if lead {
                            window::lead(x, Some(n), None)
                        } else {
                            window::lag(x, Some(n), None)
                        },
                        partition.clone(),
                        order.clone(),
                        whole(),
                    )
                };
                match operation {
                    Shift { periods, lead, .. } => lag(value, *periods, *lead)?,
                    FillDirection { forward, .. } => {
                        let expr = aggregate::last_value(value.clone(), vec![])
                            .null_treatment(NullTreatment::IgnoreNulls)
                            .build()
                            .map_err(plan)?;
                        // Reverse a causal prefix for backward fill; this also permits
                        // DataFusion's bounded last-value accumulator for both directions.
                        let order = if *forward {
                            order
                        } else {
                            order
                                .into_iter()
                                .map(|s| s.expr.sort(!s.asc, !s.nulls_first))
                                .collect()
                        };
                        over(expr, partition, order, prefix())?
                    }
                    Rank {
                        dense,
                        descending,
                        nulls_first,
                        ..
                    } => {
                        // Ties use the requested rank keys, without the stability tie breaker.
                        let rank_order = if spec.order_by.is_empty() {
                            vec![
                                ordered_value(value.clone(), source)?
                                    .sort(!descending, *nulls_first),
                            ]
                        } else {
                            order[..order.len() - self.domain_order.len()].to_vec()
                        };
                        output = field("result", DataType::UInt64, SemanticType::Numeric, false)?;
                        over(
                            if *dense {
                                window::dense_rank()
                            } else {
                                window::rank()
                            },
                            partition,
                            rank_order,
                            whole(),
                        )?
                    }
                    Difference { order: n, .. } => {
                        let x = finite(value, source.data_type(), false);
                        output = field("result", DataType::Float64, SemanticType::Numeric, true)?;
                        over(
                            crate::difference::expression(x, *n)?,
                            partition,
                            order,
                            prefix(),
                        )?
                    }
                    PercentChange { periods, .. } => {
                        let x = finite(value, source.data_type(), false);
                        let previous = lag(x.clone(), *periods, false)?;
                        output = field("result", DataType::Float64, SemanticType::Numeric, true)?;
                        when(previous.clone().eq(lit(0.0)), null(&DataType::Float64)?)
                            .otherwise(finite(x / previous - lit(1.0), &DataType::Float64, false))
                            .map_err(plan)?
                    }
                    Rolling {
                        operation: aggregate,
                        size,
                        min_periods,
                        ..
                    } => {
                        if *size == 0 || *min_periods == 0 || min_periods > size {
                            return Err(RelationError::InvalidInput);
                        }
                        let x = finite(value, source.data_type(), false);
                        let frame = WindowFrame::new_bounds(
                            WindowFrameUnits::Rows,
                            WindowFrameBound::Preceding(ScalarValue::UInt64(Some(
                                (*size - 1) as u64,
                            ))),
                            WindowFrameBound::CurrentRow,
                        );
                        output = field("result", DataType::Float64, SemanticType::Numeric, true)?;
                        window_statistic(x, *aggregate, partition, order, frame, *min_periods)?
                    }
                    Cumulative {
                        operation: aggregate,
                        ..
                    } => {
                        output = field("result", DataType::Float64, SemanticType::Numeric, true)?;
                        window_statistic(
                            finite(value, source.data_type(), false),
                            *aggregate,
                            partition,
                            order,
                            prefix(),
                            1,
                        )?
                    }
                    _ => unreachable!(),
                }
            }
        };
        Ok(Arc::new(DataFusionSeries {
            expression,
            field: Arc::new(output),
        }))
    }

    pub(crate) fn reduction(
        &self,
        series: &SeriesHandle,
        operation: SeriesReduction,
    ) -> Result<RelationHandle, RelationError> {
        let source = series.plan().field();
        if !matches!(operation, SeriesReduction::Length | SeriesReduction::Count)
            && !yss_database_arrow::is_numeric_field(source)
        {
            return Err(RelationError::InvalidInput);
        }
        let value = self.series_expression(series)?;
        let input = self.select_native(vec![value.alias("__yssbi_value")])?;
        let value = col("__yssbi_value");
        let numeric = || finite(value.clone(), source.data_type(), false);
        let expressions = match operation {
            SeriesReduction::Length => vec![aggregate::count(lit(1_i64)).alias("value")],
            SeriesReduction::Count => vec![aggregate::count(value).alias("value")],
            SeriesReduction::Sum => vec![
                aggregate::sum(if source.data_type().is_integer() {
                    cast(value, DataType::Decimal128(38, 0))
                } else {
                    numeric()
                })
                .alias("value"),
            ],
            SeriesReduction::Mean => vec![aggregate::avg(numeric()).alias("value")],
            SeriesReduction::StandardizationStatistics => vec![
                aggregate::avg(numeric()).alias("mean"),
                aggregate::stddev(numeric()).alias("standard_deviation"),
            ],
        };
        let mut frame = input.aggregate(vec![], expressions).map_err(plan)?;
        if operation == SeriesReduction::Sum {
            let dtype = frame.schema().as_arrow().field(0).data_type().clone();
            frame = frame
                .select(vec![
                    scalar::coalesce(vec![
                        col("value"),
                        literal(&TabularScalar::Integer(0), &dtype)?,
                    ])
                    .alias("value"),
                ])
                .map_err(plan)?;
        }
        let fields = frame
            .schema()
            .as_arrow()
            .fields()
            .iter()
            .map(|f| {
                field(
                    f.name(),
                    f.data_type().clone(),
                    SemanticType::Numeric,
                    f.is_nullable(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::handle(
            frame,
            Arc::new(Schema::new(fields)),
            self.bindings.clone(),
            self.lease.clone(),
            self.executor.clone(),
            false,
            vec![],
        )
    }
}

fn window_aggregate(operation: WindowOperation, x: Expr) -> Expr {
    match operation {
        WindowOperation::Sum => aggregate::sum(x),
        WindowOperation::Mean => aggregate::avg(x),
        WindowOperation::Min => aggregate::min(x),
        WindowOperation::Max => aggregate::max(x),
        WindowOperation::StandardDeviation => aggregate::stddev(x),
    }
}
pub(crate) fn lossless(value: Expr, from: &DataType, to: &DataType) -> Expr {
    let output = to.clone();
    create_udf(
        "yssbi_transform_lossless",
        vec![from.clone()],
        to.clone(),
        Volatility::Immutable,
        Arc::new(move |args| {
            let arrays = ColumnarValue::values_to_arrays(args)?;
            let result = yss_database_arrow::lossless_cast(arrays[0].as_ref(), &output, false)
                .map_err(|_| DataFusionError::External(Box::new(RelationError::InvalidInput)))?;
            if matches!(args[0], ColumnarValue::Scalar(_)) {
                Ok(ColumnarValue::Scalar(ScalarValue::try_from_array(
                    result.as_ref(),
                    0,
                )?))
            } else {
                Ok(ColumnarValue::Array(result))
            }
        }),
    )
    .call(vec![value])
}
fn window_statistic(
    x: Expr,
    operation: WindowOperation,
    partition: Vec<Expr>,
    order: Vec<Sort>,
    frame: WindowFrame,
    min_periods: usize,
) -> Result<Expr, RelationError> {
    let result = over(
        window_aggregate(operation, x.clone()),
        partition.clone(),
        order.clone(),
        frame.clone(),
    )?;
    let count = over(aggregate::count(x), partition, order, frame)?;
    when(
        count.gt_eq(lit(
            i64::try_from(min_periods).map_err(|_| RelationError::InvalidInput)?
        )),
        finite(result, &DataType::Float64, false),
    )
    .otherwise(null(&DataType::Float64)?)
    .map_err(plan)
}
fn literal_code(value: &TabularScalar, dtype: &DataType) -> Result<Option<String>, RelationError> {
    let Expr::Literal(value, _) = literal(value, dtype)? else {
        return Err(RelationError::InvalidInput);
    };
    let array = value.to_array_of_size(1).map_err(plan)?;
    if array.is_null(0) {
        return Ok(None);
    }
    let text = arrow::compute::cast(array.as_ref(), &DataType::Utf8)
        .map_err(|_| RelationError::InvalidInput)?;
    Ok(Some(
        text.as_any()
            .downcast_ref::<arrow::array::StringArray>()
            .ok_or(RelationError::InvalidInput)?
            .value(0)
            .to_owned(),
    ))
}
fn duration_nanos(unit: &str) -> Result<i64, RelationError> {
    Ok(match unit {
        "nanosecond" => 1,
        "microsecond" => 1_000,
        "millisecond" => 1_000_000,
        "second" => 1_000_000_000,
        "minute" => 60_000_000_000,
        "hour" => 3_600_000_000_000,
        "day" => 86_400_000_000_000,
        "week" => 604_800_000_000_000,
        _ => return Err(RelationError::InvalidInput),
    })
}
fn temporal_tick_nanos(dtype: &DataType) -> Result<i64, RelationError> {
    use arrow::datatypes::TimeUnit;
    Ok(match dtype {
        DataType::Date32 => 86_400_000_000_000,
        DataType::Date64 => 1_000_000,
        DataType::Timestamp(unit, None) | DataType::Time32(unit) | DataType::Time64(unit) => {
            match unit {
                TimeUnit::Second => 1_000_000_000,
                TimeUnit::Millisecond => 1_000_000,
                TimeUnit::Microsecond => 1_000,
                TimeUnit::Nanosecond => 1,
            }
        }
        _ => return Err(RelationError::InvalidInput),
    })
}
fn temporal_nanos(value: Expr, dtype: &DataType) -> Result<Expr, RelationError> {
    Ok(
        cast(cast(value, DataType::Int64), DataType::Decimal128(38, 0))
            * cast(
                lit(temporal_tick_nanos(dtype)?),
                DataType::Decimal128(38, 0),
            ),
    )
}
fn interval(unit: &str, amount: i64) -> Result<ScalarValue, RelationError> {
    let (months, days, nanos) = match unit {
        "year" => (
            i32::try_from(amount.checked_mul(12).ok_or(RelationError::InvalidInput)?)
                .map_err(|_| RelationError::InvalidInput)?,
            0,
            0,
        ),
        "month" => (
            i32::try_from(amount).map_err(|_| RelationError::InvalidInput)?,
            0,
            0,
        ),
        "day" | "week" => (
            0,
            i32::try_from(
                amount
                    .checked_mul(if unit == "week" { 7 } else { 1 })
                    .ok_or(RelationError::InvalidInput)?,
            )
            .map_err(|_| RelationError::InvalidInput)?,
            0,
        ),
        _ => (
            0,
            0,
            amount
                .checked_mul(duration_nanos(unit)?)
                .ok_or(RelationError::InvalidInput)?,
        ),
    };
    Ok(ScalarValue::IntervalMonthDayNano(Some(
        arrow::datatypes::IntervalMonthDayNano::new(months, days, nanos),
    )))
}
