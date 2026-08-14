//! DataFrame kernels over run-scoped project resources and protocol values.

use super::KernelFragment;
use crate::node_system::plan::ResourceId;
use crate::node_system::protocol::{CanonicalDecimal, Value};
use crate::node_system::runtime::{
    Artifact, ArtifactKind, ArtifactValueKind, DataSeriesBuilder, DataSeriesElementType, Kernel,
    KernelContext, KernelError, NumericValue, ProjectResourceLease, RuntimeValue, numeric_equal,
    numeric_ordering, require_data_series,
};
use std::cmp::Ordering;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataframeApi {
    /// `crate::tabular` snapshots/catalog and the relational backend.
    Tabular,
    /// `crate::sci` time-series transforms.
    ScientificTimeSeries,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataframeOperation {
    SourceGet,
    Decompose,
    Combine,
    Filter,
    SeriesSelect,
    IntegerRange,
    SeriesLength,
    SeriesCount,
    SeriesSum,
    SeriesMean,
    SeriesGreater,
    SeriesLess,
    SeriesGreaterEqual,
    SeriesLessEqual,
    SeriesEqual,
    SeriesNotEqual,
    StringSeriesEqual,
    StringSeriesNotEqual,
    SeriesStandardize,
    SeriesInverseStandardize,
    SeriesAnnotateDummy,
    TimeSeriesAlign,
    TimeSeriesDifference,
    TimeSeriesPercentChange,
    TimeSeriesRollingMean,
    TimeSeriesLag,
    PanelAlign,
    PanelDifference,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DataframeKernelParameters {
    pub resource: Option<ResourceId>,
    pub column: Option<Box<str>>,
    pub columns: Option<Box<[Box<str>]>>,
    pub order: Option<usize>,
}

#[derive(Debug, Clone, Copy)]
struct DataframeKernel {
    operation: DataframeOperation,
    api: DataframeApi,
}

impl Kernel for DataframeKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        context
            .cancellation
            .check()
            .map_err(|error| KernelError::cancelled(error.to_string()))?;
        let parameters = context.parameters::<DataframeKernelParameters>()?;
        use DataframeOperation::*;
        let outputs = match self.operation {
            SourceGet => vec![RuntimeValue::Scalar(source_dataframe(context, parameters)?)],
            Decompose => {
                decompose_dataframe(&dataframe_value(inputs, 0)?, parameters.columns.as_deref())?
            }
            Combine => vec![RuntimeValue::Scalar(combine_series(inputs)?)],
            Filter => vec![RuntimeValue::Scalar(filter_dataframe(
                &dataframe_value(inputs, 0)?,
                input(inputs, 1)?,
            )?)],
            SeriesSelect => vec![RuntimeValue::Artifact(select_series(
                &dataframe_value(inputs, 0)?,
                parameters.column.as_deref(),
            )?)],
            IntegerRange => vec![RuntimeValue::Artifact(integer_range(inputs)?)],
            SeriesLength => vec![RuntimeValue::Scalar(Value::Integer(
                series_metadata(input(inputs, 0)?)?.length as i64,
            ))],
            SeriesCount => vec![RuntimeValue::Scalar(Value::Integer(
                (series_metadata(input(inputs, 0)?)?.length
                    - series_metadata(input(inputs, 0)?)?.null_count) as i64,
            ))],
            SeriesSum => vec![RuntimeValue::Scalar(sum_series(input(inputs, 0)?)?)],
            SeriesMean => vec![RuntimeValue::Scalar(mean_series(input(inputs, 0)?)?)],
            SeriesGreater | SeriesLess | SeriesGreaterEqual | SeriesLessEqual | SeriesEqual
            | SeriesNotEqual => vec![RuntimeValue::Artifact(compare_numeric_series(
                context,
                input(inputs, 0)?,
                input(inputs, 1)?,
                self.operation,
            )?)],
            StringSeriesEqual | StringSeriesNotEqual => {
                vec![RuntimeValue::Artifact(compare_string_series(
                    input(inputs, 0)?,
                    input(inputs, 1)?,
                    self.operation,
                )?)]
            }
            SeriesStandardize => standardize(input(inputs, 0)?)?,
            SeriesInverseStandardize => {
                vec![RuntimeValue::Artifact(inverse_standardize(inputs)?)]
            }
            SeriesAnnotateDummy => vec![RuntimeValue::Artifact(
                series_artifact(input(inputs, 0)?)?.clone(),
            )],
            TimeSeriesAlign => {
                series_artifact(input(inputs, 1)?)?;
                vec![RuntimeValue::Scalar(dataframe_value(inputs, 0)?)]
            }
            PanelAlign => {
                series_artifact(input(inputs, 1)?)?;
                series_artifact(input(inputs, 2)?)?;
                vec![RuntimeValue::Scalar(dataframe_value(inputs, 0)?)]
            }
            TimeSeriesDifference | PanelDifference => vec![RuntimeValue::Artifact(difference(
                input(inputs, inputs.len().saturating_sub(1))?,
                parameters.order.unwrap_or(1),
            )?)],
            TimeSeriesPercentChange => vec![RuntimeValue::Artifact(percent_change(
                input(inputs, 0)?,
                parameters.order.unwrap_or(1),
            )?)],
            TimeSeriesRollingMean => vec![RuntimeValue::Artifact(rolling_mean(
                input(inputs, 0)?,
                parameters.order.unwrap_or(1),
            )?)],
            TimeSeriesLag => vec![RuntimeValue::Artifact(lag(
                input(inputs, 0)?,
                parameters.order.unwrap_or(1),
            )?)],
        };
        let _ = self.api;
        Ok(outputs)
    }
}

fn input(inputs: &[RuntimeValue], index: usize) -> Result<&RuntimeValue, KernelError> {
    inputs
        .get(index)
        .ok_or_else(|| KernelError::new(format!("missing dataframe input {index}")))
}

fn dataframe_value(inputs: &[RuntimeValue], index: usize) -> Result<Value, KernelError> {
    match inputs.get(index) {
        Some(RuntimeValue::Scalar(value)) => Ok(value.clone()),
        Some(RuntimeValue::Artifact(artifact))
            if artifact.value_kind() == ArtifactValueKind::Sequence =>
        {
            let mut values = artifact
                .cursor()
                .map_err(|error| KernelError::new(error.to_string()))?;
            let value = values
                .next()
                .transpose()
                .map_err(|error| KernelError::new(error.to_string()))?
                .ok_or_else(|| {
                    KernelError::new("dataframe artifact must contain exactly one value")
                })?;
            if values
                .next()
                .transpose()
                .map_err(|error| KernelError::new(error.to_string()))?
                .is_some()
            {
                return Err(KernelError::new(
                    "dataframe artifact must contain exactly one value",
                ));
            }
            Ok(value)
        }
        Some(RuntimeValue::Artifact(_)) => {
            Err(KernelError::new("expected DataFrame, received DataSeries"))
        }
        Some(RuntimeValue::Stream(_)) => Err(KernelError::new(
            "dataframe input stream must be collected before execution",
        )),
        None => Err(KernelError::new(format!("missing dataframe input {index}"))),
    }
}

fn scalar(inputs: &[RuntimeValue], index: usize) -> Result<&Value, KernelError> {
    match inputs.get(index) {
        Some(RuntimeValue::Scalar(value)) => Ok(value),
        Some(_) => Err(KernelError::new(
            "dataframe kernels require materialized values",
        )),
        None => Err(KernelError::new(format!("missing dataframe input {index}"))),
    }
}

fn source_dataframe(
    context: &KernelContext<'_>,
    parameters: &DataframeKernelParameters,
) -> Result<Value, KernelError> {
    let resource = parameters
        .resource
        .as_ref()
        .ok_or_else(|| KernelError::new("dataframe source has no bound resource"))?;
    let lease = context
        .resources
        .get(resource)
        .and_then(|lease| lease.as_any().downcast_ref::<ProjectResourceLease>())
        .ok_or_else(|| KernelError::new("bound dataframe resource is unavailable"))?;
    let dataframe = lease
        .load_dataframe()
        .map_err(KernelError::new)?
        .ok_or_else(|| KernelError::new("bound dataframe resource is unavailable"))?;
    dataframe_to_protocol_value(dataframe.as_ref())
}

pub fn dataframe_to_protocol_value(
    dataframe: &polars::prelude::DataFrame,
) -> Result<Value, KernelError> {
    dataframe_to_protocol_value_with_checkpoint(dataframe, || Ok(()))
}

pub(crate) fn dataframe_to_protocol_value_with_checkpoint(
    dataframe: &polars::prelude::DataFrame,
    mut checkpoint: impl FnMut() -> Result<(), KernelError>,
) -> Result<Value, KernelError> {
    let mut columns = BTreeMap::new();
    for column in dataframe.columns() {
        let values = (0..dataframe.height())
            .map(|row| {
                checkpoint()?;
                column
                    .get(row)
                    .map_err(|error| KernelError::new(error.to_string()))
                    .and_then(any_value)
            })
            .collect::<Result<Vec<_>, _>>()?;
        columns.insert(
            column.name().to_string().into_boxed_str(),
            Value::List(values),
        );
    }
    Ok(Value::Object(columns))
}

fn any_value(value: polars::prelude::AnyValue<'_>) -> Result<Value, KernelError> {
    use polars::prelude::AnyValue;

    match value {
        AnyValue::Null => Ok(Value::Null),
        AnyValue::Boolean(value) => Ok(Value::Bool(value)),
        AnyValue::Int8(value) => Ok(Value::Integer(value.into())),
        AnyValue::Int16(value) => Ok(Value::Integer(value.into())),
        AnyValue::Int32(value) => Ok(Value::Integer(value.into())),
        AnyValue::Int64(value) => Ok(Value::Integer(value)),
        AnyValue::UInt8(value) => Ok(Value::Unsigned(value.into())),
        AnyValue::UInt16(value) => Ok(Value::Unsigned(value.into())),
        AnyValue::UInt32(value) => Ok(Value::Unsigned(value.into())),
        AnyValue::UInt64(value) => Ok(Value::Unsigned(value)),
        AnyValue::Float32(value) => protocol_decimal_value(value as f64),
        AnyValue::Float64(value) => protocol_decimal_value(value),
        AnyValue::String(value) => Ok(Value::String(value.into())),
        AnyValue::StringOwned(value) => Ok(Value::String(value.as_str().into())),
        value => Ok(Value::String(value.to_string().into())),
    }
}

fn protocol_decimal_value(value: f64) -> Result<Value, KernelError> {
    if !value.is_finite() {
        return Err(KernelError::new(
            "dataframe float value is not finite and cannot cross the runtime boundary",
        ));
    }
    CanonicalDecimal::new(value.to_string())
        .map(Value::Decimal)
        .map_err(|error| KernelError::new(error.to_string()))
}

fn dataframe_columns(value: &Value) -> Result<BTreeMap<Box<str>, Value>, KernelError> {
    match value {
        Value::Object(columns) => Ok(columns.clone()),
        _ => Err(KernelError::new("expected dataframe column object")),
    }
}

fn series_artifact(value: &RuntimeValue) -> Result<&Artifact, KernelError> {
    require_data_series(value)
}

fn series_metadata(
    value: &RuntimeValue,
) -> Result<&crate::node_system::runtime::DataSeriesMetadata, KernelError> {
    series_artifact(value)?
        .data_series_metadata()
        .ok_or_else(|| KernelError::new("expected DataSeries Artifact, received sequence Artifact"))
}

fn artifact_values(artifact: &Artifact) -> Result<Vec<Value>, KernelError> {
    artifact
        .cursor()
        .map_err(|error| KernelError::new(error.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| KernelError::new(error.to_string()))
}

fn build_series(
    element_type: DataSeriesElementType,
    values: Vec<Value>,
) -> Result<Artifact, KernelError> {
    build_series_with_metadata(element_type, values, None, None)
}

fn build_series_with_metadata(
    element_type: DataSeriesElementType,
    values: Vec<Value>,
    name: Option<&str>,
    format: Option<&str>,
) -> Result<Artifact, KernelError> {
    let mut builder = DataSeriesBuilder::new(element_type).values(values);
    if let Some(name) = name {
        builder = builder.name(name);
    }
    if let Some(format) = format {
        builder = builder.format(format);
    }
    builder
        .build(ArtifactKind::Collected)
        .map_err(|error| KernelError::new(error.to_string()))
}

fn infer_element_type(values: &[Value]) -> Result<DataSeriesElementType, KernelError> {
    values
        .iter()
        .find_map(|value| match value {
            Value::Null => None,
            Value::Bool(_) => Some(DataSeriesElementType::Boolean),
            Value::Integer(_) => Some(DataSeriesElementType::Int64),
            Value::Decimal(_) => Some(DataSeriesElementType::Float64),
            Value::String(_) => Some(DataSeriesElementType::String),
            _ => None,
        })
        .ok_or_else(|| {
            KernelError::new("cannot infer DataSeries element type from empty/null column")
        })
}

fn decompose_dataframe(
    value: &Value,
    selected_columns: Option<&[Box<str>]>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let columns = dataframe_columns(value)?;
    let columns = match selected_columns {
        Some(selected) => selected
            .iter()
            .map(|name| {
                columns
                    .get(name.as_ref())
                    .cloned()
                    .map(|value| (name.clone(), value))
                    .ok_or_else(|| {
                        KernelError::new(format!("dataframe column '{}' was not found", name))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?,
        None => columns.into_iter().collect(),
    };
    columns
        .into_iter()
        .map(|(name, value)| {
            let Value::List(values) = value else {
                return Err(KernelError::new("expected dataframe column list"));
            };
            let element_type = infer_element_type(&values)?;
            DataSeriesBuilder::new(element_type)
                .values(values)
                .name(name)
                .build(ArtifactKind::Collected)
                .map(RuntimeValue::Artifact)
                .map_err(|error| KernelError::new(error.to_string()))
        })
        .collect()
}

fn combine_series(inputs: &[RuntimeValue]) -> Result<Value, KernelError> {
    let mut columns = BTreeMap::new();
    for (index, input) in inputs.iter().enumerate() {
        let artifact = series_artifact(input)?;
        let metadata = series_metadata(input)?;
        let name = metadata
            .name
            .clone()
            .unwrap_or_else(|| format!("column_{index}").into_boxed_str());
        columns.insert(name, Value::List(artifact_values(artifact)?));
    }
    Ok(Value::Object(columns))
}

fn filter_dataframe(dataframe: &Value, condition: &RuntimeValue) -> Result<Value, KernelError> {
    let columns = dataframe_columns(dataframe)?;
    let artifact = series_artifact(condition)?;
    let metadata = series_metadata(condition)?;
    if metadata.element_type != DataSeriesElementType::Boolean {
        return Err(KernelError::new(format!(
            "expected Boolean DataSeries, received {}",
            metadata.element_type
        )));
    }
    let mask = artifact_values(artifact)?;
    let mut filtered = BTreeMap::new();
    for (name, values) in columns {
        let Value::List(values) = values else {
            return Err(KernelError::new("expected dataframe column list"));
        };
        let values = values
            .iter()
            .zip(&mask)
            .filter_map(|(value, keep)| matches!(keep, Value::Bool(true)).then_some(value.clone()))
            .collect();
        filtered.insert(name, Value::List(values));
    }
    Ok(Value::Object(filtered))
}

fn select_series(dataframe: &Value, column: Option<&str>) -> Result<Artifact, KernelError> {
    let columns = dataframe_columns(dataframe)?;
    let column = column.ok_or_else(|| KernelError::new("series selection has no column"))?;
    let value = columns
        .get(column)
        .cloned()
        .ok_or_else(|| KernelError::new(format!("dataframe column '{column}' was not found")))?;
    let Value::List(values) = value else {
        return Err(KernelError::new("expected dataframe column list"));
    };
    DataSeriesBuilder::new(infer_element_type(&values)?)
        .values(values)
        .name(column)
        .build(ArtifactKind::Collected)
        .map_err(|error| KernelError::new(error.to_string()))
}

fn integer_range(inputs: &[RuntimeValue]) -> Result<Artifact, KernelError> {
    let integer = |index| match scalar(inputs, index)? {
        Value::Integer(value) => Ok(*value),
        _ => Err(KernelError::new("integer range bounds must be int64")),
    };
    let (start, end, step) = (integer(0)?, integer(1)?, integer(2)?);
    if step == 0 {
        return Err(KernelError::new("integer range step cannot be zero"));
    }
    let mut values = Vec::new();
    let mut current = start;
    while (step > 0 && current < end) || (step < 0 && current > end) {
        values.push(Value::Integer(current));
        current = current
            .checked_add(step)
            .ok_or_else(|| KernelError::new("integer range overflow"))?;
    }
    build_series(DataSeriesElementType::Int64, values)
}

fn number(value: &Value) -> Result<f64, KernelError> {
    match value {
        Value::Integer(value) => Ok(*value as f64),
        Value::Decimal(value) => value
            .as_str()
            .parse()
            .map_err(|_| KernelError::new("invalid Float64 DataSeries value")),
        _ => Err(KernelError::new("expected numeric scalar")),
    }
}

fn numeric_value(value: &Value) -> Result<NumericValue, KernelError> {
    match value {
        Value::Integer(value) => Ok(NumericValue::Int64(*value)),
        Value::Decimal(value) => value
            .as_str()
            .parse()
            .map(NumericValue::Float64)
            .map_err(|_| KernelError::new("invalid Float64 DataSeries value")),
        _ => Err(KernelError::new("expected numeric scalar")),
    }
}

fn numeric_values(value: &RuntimeValue) -> Result<Vec<Option<NumericValue>>, KernelError> {
    let artifact = series_artifact(value)?;
    let metadata = series_metadata(value)?;
    if !matches!(
        metadata.element_type,
        DataSeriesElementType::Int64 | DataSeriesElementType::Float64
    ) {
        return Err(KernelError::new(format!(
            "expected numeric DataSeries, received {}",
            metadata.element_type
        )));
    }
    artifact_values(artifact)?
        .iter()
        .map(|value| match value {
            Value::Null => Ok(None),
            value => numeric_value(value).map(Some),
        })
        .collect()
}

fn numeric_f64_values(value: &RuntimeValue) -> Result<Vec<Option<f64>>, KernelError> {
    numeric_values(value)?
        .into_iter()
        .map(|value| {
            value
                .map(|value| match value {
                    NumericValue::Int64(value) => Ok(value as f64),
                    NumericValue::Float64(value) => Ok(value),
                })
                .transpose()
        })
        .collect()
}

fn decimal_value(value: f64) -> Result<Value, KernelError> {
    if !value.is_finite() {
        return Ok(Value::Null);
    }
    CanonicalDecimal::new(value.to_string())
        .map(Value::Decimal)
        .map_err(|error| KernelError::new(error.to_string()))
}

fn sum_series(value: &RuntimeValue) -> Result<Value, KernelError> {
    let metadata = series_metadata(value)?;
    match metadata.element_type {
        DataSeriesElementType::Int64 => artifact_values(series_artifact(value)?)?
            .into_iter()
            .try_fold(0_i64, |sum, value| match value {
                Value::Null => Ok(sum),
                Value::Integer(value) => sum
                    .checked_add(value)
                    .ok_or_else(|| KernelError::new("Int64 DataSeries sum overflow")),
                _ => Err(KernelError::new("invalid Int64 DataSeries storage")),
            })
            .map(Value::Integer),
        DataSeriesElementType::Float64 => {
            let sum = numeric_f64_values(value)?
                .into_iter()
                .flatten()
                .sum::<f64>();
            decimal_value(sum)
        }
        actual => Err(KernelError::new(format!(
            "expected numeric DataSeries, received {actual}"
        ))),
    }
}

fn mean_series(value: &RuntimeValue) -> Result<Value, KernelError> {
    let values = numeric_f64_values(value)?;
    let present = values.into_iter().flatten().collect::<Vec<_>>();
    if present.is_empty() {
        return Ok(Value::Null);
    }
    decimal_value(present.iter().sum::<f64>() / present.len() as f64)
}

fn comparison_right(
    right: &RuntimeValue,
    length: usize,
) -> Result<Vec<Option<NumericValue>>, KernelError> {
    match right {
        RuntimeValue::Artifact(_) => numeric_values(right),
        RuntimeValue::Scalar(Value::Null) => Ok(vec![None; length]),
        RuntimeValue::Scalar(value) => Ok(vec![Some(numeric_value(value)?); length]),
        RuntimeValue::Stream(_) => Err(KernelError::new("expected numeric DataSeries or scalar")),
    }
}

fn compare_numeric_series(
    context: &KernelContext<'_>,
    left: &RuntimeValue,
    right: &RuntimeValue,
    operation: DataframeOperation,
) -> Result<Artifact, KernelError> {
    let left = numeric_values(left)?;
    let right = comparison_right(right, left.len())?;
    if left.len() != right.len() {
        return Err(KernelError::new("series comparison lengths differ"));
    }
    let tolerance = context.computation_settings().numeric_tolerance;
    let values = left
        .into_iter()
        .zip(right)
        .map(|(left, right)| match (left, right) {
            (Some(left), Some(right)) => {
                let result = match operation {
                    DataframeOperation::SeriesEqual => numeric_equal(left, right, tolerance),
                    DataframeOperation::SeriesNotEqual => {
                        numeric_equal(left, right, tolerance).map(|equal| !equal)
                    }
                    DataframeOperation::SeriesGreater => {
                        numeric_ordering(left, right).map(|value| value == Ordering::Greater)
                    }
                    DataframeOperation::SeriesLess => {
                        numeric_ordering(left, right).map(|value| value == Ordering::Less)
                    }
                    DataframeOperation::SeriesGreaterEqual => {
                        numeric_ordering(left, right).map(|value| value != Ordering::Less)
                    }
                    DataframeOperation::SeriesLessEqual => {
                        numeric_ordering(left, right).map(|value| value != Ordering::Greater)
                    }
                    _ => unreachable!(),
                }
                .map_err(|error| KernelError::new(error.to_string()))?;
                Ok(Value::Bool(result))
            }
            _ => Ok(Value::Null),
        })
        .collect::<Result<Vec<_>, KernelError>>()?;
    build_series(DataSeriesElementType::Boolean, values)
}

fn compare_string_series(
    left: &RuntimeValue,
    right: &RuntimeValue,
    operation: DataframeOperation,
) -> Result<Artifact, KernelError> {
    let left_artifact = series_artifact(left)?;
    if series_metadata(left)?.element_type != DataSeriesElementType::String {
        return Err(KernelError::new("expected String DataSeries"));
    }
    let left = artifact_values(left_artifact)?;
    let right = match right {
        RuntimeValue::Artifact(artifact) => {
            if series_metadata(right)?.element_type != DataSeriesElementType::String {
                return Err(KernelError::new("expected String DataSeries"));
            }
            artifact_values(artifact)?
        }
        RuntimeValue::Scalar(value @ (Value::String(_) | Value::Null)) => {
            vec![value.clone(); left.len()]
        }
        RuntimeValue::Scalar(_) => return Err(KernelError::new("expected String scalar")),
        RuntimeValue::Stream(_) => {
            return Err(KernelError::new("expected String DataSeries or scalar"));
        }
    };
    if left.len() != right.len() {
        return Err(KernelError::new("series comparison lengths differ"));
    }
    let values = left
        .into_iter()
        .zip(right)
        .map(|(left, right)| match (left, right) {
            (Value::String(left), Value::String(right)) => Value::Bool(match operation {
                DataframeOperation::StringSeriesEqual => left == right,
                DataframeOperation::StringSeriesNotEqual => left != right,
                _ => unreachable!(),
            }),
            _ => Value::Null,
        })
        .collect();
    build_series(DataSeriesElementType::Boolean, values)
}

fn standardize(value: &RuntimeValue) -> Result<Vec<RuntimeValue>, KernelError> {
    let values = numeric_f64_values(value)?;
    let present = values.iter().flatten().copied().collect::<Vec<_>>();
    let mean = present.iter().sum::<f64>() / present.len().max(1) as f64;
    let variance = present
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / present.len().max(1) as f64;
    let deviation = variance.sqrt();
    let standardized = values
        .into_iter()
        .map(|value| match value {
            None => Ok(Value::Null),
            Some(value) => decimal_value(if deviation == 0.0 {
                0.0
            } else {
                (value - mean) / deviation
            }),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(vec![
        RuntimeValue::Artifact(build_series(DataSeriesElementType::Float64, standardized)?),
        RuntimeValue::Scalar(decimal_value(mean)?),
        RuntimeValue::Scalar(decimal_value(deviation)?),
    ])
}

fn inverse_standardize(inputs: &[RuntimeValue]) -> Result<Artifact, KernelError> {
    let values = numeric_f64_values(input(inputs, 0)?)?;
    let mean = number(scalar(inputs, 1)?)?;
    let deviation = number(scalar(inputs, 2)?)?;
    let output = values
        .into_iter()
        .map(|value| match value {
            Some(value) => decimal_value(value * deviation + mean),
            None => Ok(Value::Null),
        })
        .collect::<Result<Vec<_>, _>>()?;
    build_series(DataSeriesElementType::Float64, output)
}

fn difference(value: &RuntimeValue, order: usize) -> Result<Artifact, KernelError> {
    let mut values = numeric_f64_values(value)?;
    for _ in 0..order {
        values = values
            .windows(2)
            .map(|pair| pair[0].zip(pair[1]).map(|(left, right)| right - left))
            .collect();
    }
    let output = values
        .into_iter()
        .map(|value| match value {
            Some(value) => decimal_value(value),
            None => Ok(Value::Null),
        })
        .collect::<Result<Vec<_>, _>>()?;
    build_series(DataSeriesElementType::Float64, output)
}

fn percent_change(value: &RuntimeValue, order: usize) -> Result<Artifact, KernelError> {
    let values = numeric_f64_values(value)?;
    let output = (order..values.len())
        .map(|index| match (values[index - order], values[index]) {
            (Some(base), Some(current)) => decimal_value(if base == 0.0 {
                0.0
            } else {
                (current - base) / base
            }),
            _ => Ok(Value::Null),
        })
        .collect::<Result<Vec<_>, _>>()?;
    build_series(DataSeriesElementType::Float64, output)
}

fn rolling_mean(value: &RuntimeValue, window: usize) -> Result<Artifact, KernelError> {
    let values = numeric_f64_values(value)?;
    let window = window.max(1);
    let output = values
        .windows(window)
        .map(|values| {
            values
                .iter()
                .copied()
                .collect::<Option<Vec<_>>>()
                .map(|values| decimal_value(values.iter().sum::<f64>() / values.len() as f64))
                .unwrap_or(Ok(Value::Null))
        })
        .collect::<Result<Vec<_>, _>>()?;
    build_series(DataSeriesElementType::Float64, output)
}

fn lag(value: &RuntimeValue, order: usize) -> Result<Artifact, KernelError> {
    let artifact = series_artifact(value)?;
    let metadata = series_metadata(value)?;
    let values = artifact_values(artifact)?;
    let mut output = vec![Value::Null; order.min(values.len())];
    output.extend(
        values
            .iter()
            .take(values.len().saturating_sub(order))
            .cloned(),
    );
    build_series_with_metadata(
        metadata.element_type,
        output,
        metadata.name.as_deref(),
        metadata.format.as_deref(),
    )
}

/// Builds the production DataFrame kernel fragment.
pub(crate) fn build_kernel_fragment() -> KernelFragment {
    use DataframeApi::{ScientificTimeSeries as Sci, Tabular};
    use DataframeOperation::*;
    let registrations = [
        registration("yssbi.dataframe.source.get", SourceGet, Tabular),
        registration("yssbi.dataframe.decompose", Decompose, Tabular),
        registration("yssbi.dataframe.combine", Combine, Tabular),
        registration("yssbi.dataframe.filter", Filter, Tabular),
        registration("yssbi.dataframe.series.select", SeriesSelect, Tabular),
        registration("yssbi.dataframe.series.int_range", IntegerRange, Tabular),
        registration("yssbi.dataframe.series.length", SeriesLength, Tabular),
        registration("yssbi.dataframe.series.count", SeriesCount, Tabular),
        registration("yssbi.dataframe.series.sum", SeriesSum, Tabular),
        registration("yssbi.dataframe.series.mean", SeriesMean, Tabular),
        registration(
            "yssbi.dataframe.series.compare.greater",
            SeriesGreater,
            Tabular,
        ),
        registration("yssbi.dataframe.series.compare.less", SeriesLess, Tabular),
        registration(
            "yssbi.dataframe.series.compare.greater_equal",
            SeriesGreaterEqual,
            Tabular,
        ),
        registration(
            "yssbi.dataframe.series.compare.less_equal",
            SeriesLessEqual,
            Tabular,
        ),
        registration("yssbi.dataframe.series.compare.equal", SeriesEqual, Tabular),
        registration(
            "yssbi.dataframe.series.compare.not_equal",
            SeriesNotEqual,
            Tabular,
        ),
        registration(
            "yssbi.dataframe.series.compare.string.equal",
            StringSeriesEqual,
            Tabular,
        ),
        registration(
            "yssbi.dataframe.series.compare.string.not_equal",
            StringSeriesNotEqual,
            Tabular,
        ),
        registration("yssbi.dataframe.series.standardize", SeriesStandardize, Sci),
        registration(
            "yssbi.dataframe.series.inverse_standardize",
            SeriesInverseStandardize,
            Sci,
        ),
        registration(
            "yssbi.dataframe.series.annotate_dummy",
            SeriesAnnotateDummy,
            Tabular,
        ),
        registration("yssbi.dataframe.timeseries.align", TimeSeriesAlign, Sci),
        registration(
            "yssbi.dataframe.timeseries.difference",
            TimeSeriesDifference,
            Sci,
        ),
        registration(
            "yssbi.dataframe.timeseries.percent_change",
            TimeSeriesPercentChange,
            Sci,
        ),
        registration(
            "yssbi.dataframe.timeseries.rolling_mean",
            TimeSeriesRollingMean,
            Sci,
        ),
        registration("yssbi.dataframe.timeseries.lag", TimeSeriesLag, Sci),
        registration("yssbi.dataframe.panel.align", PanelAlign, Sci),
        registration("yssbi.dataframe.panel.difference", PanelDifference, Sci),
    ];
    let mut fragment = KernelFragment::default();
    for (handle, operation, api) in registrations {
        fragment.register(handle, DataframeKernel { operation, api });
    }
    fragment
}

const fn registration(
    handle: &'static str,
    operation: DataframeOperation,
    api: DataframeApi,
) -> (&'static str, DataframeOperation, DataframeApi) {
    (handle, operation, api)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::analysis::RunId;
    use crate::node_system::plan::{CompiledParameterHandle, CompiledResourceRequirement};
    use crate::node_system::runtime::{
        ActivationId, CancellationToken, EffectiveComputationSettings, FrameId, KernelErrorKind,
        ResourceError, ResourceLease, ResourceProvider, RunResourceBudgets, RunResourceOwner,
        RunResourceSet,
    };

    struct NoResources;

    impl ResourceProvider for NoResources {
        fn acquire(
            &self,
            _: &CompiledResourceRequirement,
        ) -> Result<Box<dyn ResourceLease>, ResourceError> {
            unreachable!("cancelled dataframe kernel does not acquire resources")
        }
    }

    #[test]
    fn cancelled_dataframe_kernel_returns_structured_cancellation() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let resources = RunResourceSet::acquire(&[], &NoResources).unwrap();
        let params = CompiledParameterHandle::new("cancelled.dataframe").unwrap();
        let resource_owner = RunResourceOwner::new(
            RunId::new(1),
            RunResourceBudgets::default(),
            cancellation.clone(),
        )
        .unwrap();
        let context = KernelContext {
            run_id: RunId::new(1),
            frame_id: FrameId::next(),
            activation_id: ActivationId::next().unwrap(),
            computation_settings: EffectiveComputationSettings::default(),
            params: &params,
            compiled_parameters: None,
            resources: &resources,
            resource_owner: &resource_owner,
            cancellation: &cancellation,
            deadline: None,
        };
        let kernel = DataframeKernel {
            operation: DataframeOperation::IntegerRange,
            api: DataframeApi::Tabular,
        };

        let error = kernel.execute(&context, &[]).unwrap_err();

        assert_eq!(error.kind(), KernelErrorKind::Cancelled);
    }
}
