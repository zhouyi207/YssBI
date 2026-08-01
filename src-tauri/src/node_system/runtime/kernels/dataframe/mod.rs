//! DataFrame kernels over run-scoped project resources and protocol values.

use super::KernelFragment;
use crate::node_system::plan::ResourceId;
use crate::node_system::protocol::{CanonicalDecimal, Value};
use crate::node_system::runtime::{
    Kernel, KernelContext, KernelError, ProjectResourceLease, RuntimeValue,
};
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
    SeriesSum,
    SeriesMean,
    SeriesGreater,
    SeriesLess,
    SeriesGreaterEqual,
    SeriesLessEqual,
    SeriesEqual,
    SeriesNotEqual,
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
            Decompose => dataframe_columns(scalar(inputs, 0)?)?
                .into_values()
                .map(RuntimeValue::Scalar)
                .collect(),
            Combine => vec![RuntimeValue::Scalar(combine_series(inputs)?)],
            Filter => vec![RuntimeValue::Scalar(filter_dataframe(
                scalar(inputs, 0)?,
                scalar(inputs, 1)?,
            )?)],
            SeriesSelect => vec![RuntimeValue::Scalar(select_series(
                scalar(inputs, 0)?,
                parameters.column.as_deref(),
            )?)],
            IntegerRange => vec![RuntimeValue::Scalar(integer_range(inputs)?)],
            SeriesLength => vec![RuntimeValue::Scalar(Value::Integer(
                series(scalar(inputs, 0)?)?.len() as i64,
            ))],
            SeriesSum => vec![RuntimeValue::Scalar(decimal_value(
                numeric_series(scalar(inputs, 0)?)?.iter().sum(),
            )?)],
            SeriesMean => {
                let values = numeric_series(scalar(inputs, 0)?)?;
                let mean = values.iter().sum::<f64>() / values.len().max(1) as f64;
                vec![RuntimeValue::Scalar(decimal_value(mean)?)]
            }
            SeriesGreater | SeriesLess | SeriesGreaterEqual | SeriesLessEqual | SeriesEqual
            | SeriesNotEqual => vec![RuntimeValue::Scalar(compare_series(
                scalar(inputs, 0)?,
                scalar(inputs, 1)?,
                self.operation,
            )?)],
            SeriesStandardize => standardize(scalar(inputs, 0)?)?,
            SeriesInverseStandardize => vec![RuntimeValue::Scalar(inverse_standardize(inputs)?)],
            SeriesAnnotateDummy | TimeSeriesAlign | PanelAlign => {
                vec![RuntimeValue::Scalar(scalar(inputs, 0)?.clone())]
            }
            TimeSeriesDifference | PanelDifference => vec![RuntimeValue::Scalar(difference(
                scalar(inputs, inputs.len().saturating_sub(1))?,
                parameters.order.unwrap_or(1),
            )?)],
            TimeSeriesPercentChange => vec![RuntimeValue::Scalar(percent_change(
                scalar(inputs, 0)?,
                parameters.order.unwrap_or(1),
            )?)],
            TimeSeriesRollingMean => vec![RuntimeValue::Scalar(rolling_mean(
                scalar(inputs, 0)?,
                parameters.order.unwrap_or(1),
            )?)],
            TimeSeriesLag => vec![RuntimeValue::Scalar(lag(
                scalar(inputs, 0)?,
                parameters.order.unwrap_or(1),
            )?)],
        };
        let _ = self.api;
        Ok(outputs)
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
    let mut columns = BTreeMap::new();
    for column in dataframe.columns() {
        let values = (0..dataframe.height())
            .map(|row| {
                column
                    .get(row)
                    .map(|value| any_value(value.to_string()))
                    .map_err(|error| KernelError::new(error.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        columns.insert(
            column.name().to_string().into_boxed_str(),
            Value::List(values),
        );
    }
    Ok(Value::Object(columns))
}

fn any_value(value: String) -> Value {
    if value == "null" {
        Value::Null
    } else if value == "true" || value == "false" {
        Value::Bool(value == "true")
    } else if let Ok(integer) = value.parse::<i64>() {
        Value::Integer(integer)
    } else if CanonicalDecimal::new(value.as_str()).is_ok() && value.contains('.') {
        Value::Decimal(CanonicalDecimal::new(value).expect("checked decimal"))
    } else {
        Value::String(value.into())
    }
}

fn dataframe_columns(value: &Value) -> Result<BTreeMap<Box<str>, Value>, KernelError> {
    match value {
        Value::Object(columns) => Ok(columns.clone()),
        _ => Err(KernelError::new("expected dataframe column object")),
    }
}

fn series(value: &Value) -> Result<&[Value], KernelError> {
    match value {
        Value::List(values) => Ok(values),
        _ => Err(KernelError::new("expected series list")),
    }
}

fn combine_series(inputs: &[RuntimeValue]) -> Result<Value, KernelError> {
    let mut columns = BTreeMap::new();
    for (index, input) in inputs.iter().enumerate() {
        let RuntimeValue::Scalar(value @ Value::List(_)) = input else {
            return Err(KernelError::new("combine expects series inputs"));
        };
        columns.insert(format!("column_{index}").into_boxed_str(), value.clone());
    }
    Ok(Value::Object(columns))
}

fn filter_dataframe(dataframe: &Value, condition: &Value) -> Result<Value, KernelError> {
    let columns = dataframe_columns(dataframe)?;
    let mask = series(condition)?;
    let mut filtered = BTreeMap::new();
    for (name, values) in columns {
        let values = series(&values)?
            .iter()
            .zip(mask)
            .filter_map(|(value, keep)| matches!(keep, Value::Bool(true)).then_some(value.clone()))
            .collect();
        filtered.insert(name, Value::List(values));
    }
    Ok(Value::Object(filtered))
}

fn select_series(dataframe: &Value, column: Option<&str>) -> Result<Value, KernelError> {
    let columns = dataframe_columns(dataframe)?;
    let column = column.ok_or_else(|| KernelError::new("series selection has no column"))?;
    columns
        .get(column)
        .cloned()
        .ok_or_else(|| KernelError::new(format!("dataframe column '{column}' was not found")))
}

fn integer_range(inputs: &[RuntimeValue]) -> Result<Value, KernelError> {
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
    Ok(Value::List(values))
}

fn number(value: &Value) -> Result<f64, KernelError> {
    match value {
        Value::Integer(value) => Ok(*value as f64),
        Value::Unsigned(value) => Ok(*value as f64),
        Value::Decimal(value) => value
            .as_str()
            .parse()
            .map_err(|_| KernelError::new("invalid decimal series value")),
        _ => Err(KernelError::new("series contains a non-numeric value")),
    }
}

fn numeric_series(value: &Value) -> Result<Vec<f64>, KernelError> {
    series(value)?.iter().map(number).collect()
}

fn decimal_value(value: f64) -> Result<Value, KernelError> {
    if !value.is_finite() {
        return Ok(Value::Null);
    }
    CanonicalDecimal::new(value.to_string())
        .map(Value::Decimal)
        .map_err(|error| KernelError::new(error.to_string()))
}

fn compare_series(
    left: &Value,
    right: &Value,
    operation: DataframeOperation,
) -> Result<Value, KernelError> {
    let left = series(left)?;
    let right = match right {
        Value::List(values) => values.clone(),
        scalar => vec![scalar.clone(); left.len()],
    };
    if left.len() != right.len() {
        return Err(KernelError::new("series comparison lengths differ"));
    }
    let values = left
        .iter()
        .zip(&right)
        .map(|(left, right)| {
            let result = match operation {
                DataframeOperation::SeriesEqual => left == right,
                DataframeOperation::SeriesNotEqual => left != right,
                DataframeOperation::SeriesGreater => number(left)? > number(right)?,
                DataframeOperation::SeriesLess => number(left)? < number(right)?,
                DataframeOperation::SeriesGreaterEqual => number(left)? >= number(right)?,
                DataframeOperation::SeriesLessEqual => number(left)? <= number(right)?,
                _ => unreachable!(),
            };
            Ok(Value::Bool(result))
        })
        .collect::<Result<Vec<_>, KernelError>>()?;
    Ok(Value::List(values))
}

fn standardize(value: &Value) -> Result<Vec<RuntimeValue>, KernelError> {
    let values = numeric_series(value)?;
    let mean = values.iter().sum::<f64>() / values.len().max(1) as f64;
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / values.len().max(1) as f64;
    let deviation = variance.sqrt();
    let standardized = values
        .iter()
        .map(|value| {
            decimal_value(if deviation == 0.0 {
                0.0
            } else {
                (value - mean) / deviation
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(vec![
        RuntimeValue::Scalar(Value::List(standardized)),
        RuntimeValue::Scalar(decimal_value(mean)?),
        RuntimeValue::Scalar(decimal_value(deviation)?),
    ])
}

fn inverse_standardize(inputs: &[RuntimeValue]) -> Result<Value, KernelError> {
    let values = numeric_series(scalar(inputs, 0)?)?;
    let mean = number(scalar(inputs, 1)?)?;
    let deviation = number(scalar(inputs, 2)?)?;
    Ok(Value::List(
        values
            .into_iter()
            .map(|value| decimal_value(value * deviation + mean))
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

fn difference(value: &Value, order: usize) -> Result<Value, KernelError> {
    let mut values = numeric_series(value)?;
    for _ in 0..order {
        values = values.windows(2).map(|pair| pair[1] - pair[0]).collect();
    }
    Ok(Value::List(
        values
            .into_iter()
            .map(decimal_value)
            .collect::<Result<_, _>>()?,
    ))
}

fn percent_change(value: &Value, order: usize) -> Result<Value, KernelError> {
    let values = numeric_series(value)?;
    let output = (order..values.len())
        .map(|index| {
            let base = values[index - order];
            decimal_value(if base == 0.0 {
                0.0
            } else {
                (values[index] - base) / base
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::List(output))
}

fn rolling_mean(value: &Value, window: usize) -> Result<Value, KernelError> {
    let values = numeric_series(value)?;
    let window = window.max(1);
    let output = values
        .windows(window)
        .map(|values| decimal_value(values.iter().sum::<f64>() / values.len() as f64))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Value::List(output))
}

fn lag(value: &Value, order: usize) -> Result<Value, KernelError> {
    let values = series(value)?;
    let mut output = vec![Value::Null; order.min(values.len())];
    output.extend(
        values
            .iter()
            .take(values.len().saturating_sub(order))
            .cloned(),
    );
    Ok(Value::List(output))
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
        ActivationId, CancellationToken, FrameId, KernelErrorKind, ResourceError, ResourceLease,
        ResourceProvider, RunResourceSet,
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
        let context = KernelContext {
            run_id: RunId::new(1),
            frame_id: FrameId::next(),
            activation_id: ActivationId::next(),
            params: &params,
            compiled_parameters: None,
            resources: &resources,
            cancellation: &cancellation,
        };
        let kernel = DataframeKernel {
            operation: DataframeOperation::IntegerRange,
            api: DataframeApi::Tabular,
        };

        let error = kernel.execute(&context, &[]).unwrap_err();

        assert_eq!(error.kind(), KernelErrorKind::Cancelled);
    }
}
