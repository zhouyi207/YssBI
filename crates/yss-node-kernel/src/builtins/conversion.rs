use yss_data_contract::{
    ConversionDomain, DatetimeRepresentation, NumericRepresentation, SemanticConversion,
    SemanticType, SemanticValue, TemporalPrecision, ValueType,
};

use yss_data_contract::TabularScalar;
use yss_database_arrow::InferredCategoricalDomain;
use yss_relational_contract::{RelationError, SeriesHandle};

use super::relational::kernel_error;
use crate::{KernelControl, KernelError, KernelInvocation, RuntimeValue};

pub(super) fn execute(
    target: SemanticType,
    invocation: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    let mut conversion = SemanticConversion::new(target, NumericRepresentation::Auto);
    let text = |name| match invocation.parameter(name) {
        Some(RuntimeValue::Scalar(TabularScalar::String(value))) => Ok(value.as_ref()),
        _ => Err(KernelError::InvalidParameter),
    };
    match target {
        SemanticType::Numeric => {
            conversion.numeric = NumericRepresentation::from_parameter(text("numeric_mode")?)
                .ok_or(KernelError::InvalidParameter)?;
        }
        SemanticType::Categorical | SemanticType::Ordinal | SemanticType::Binary => {
            conversion.domain = conversion_domain(
                invocation
                    .parameter("semantic_domain")
                    .ok_or(KernelError::InvalidParameter)?,
            )?;
        }
        SemanticType::Datetime => {
            conversion.datetime = DatetimeRepresentation::from_parameter(text("datetime_kind")?)
                .ok_or(KernelError::InvalidParameter)?;
            conversion.precision = TemporalPrecision::from_parameter(text("datetime_precision")?)
                .ok_or(KernelError::InvalidParameter)?;
            conversion.format = text("datetime_format")?.into();
        }
        SemanticType::Text | SemanticType::Identifier => {}
    }
    convert(invocation, conversion)
}

pub(super) fn labels(invocation: &KernelInvocation<'_>) -> Result<RuntimeValue, KernelError> {
    let Some(RuntimeValue::Scalar(TabularScalar::String(target))) =
        invocation.parameter("target_type")
    else {
        return Err(KernelError::InvalidParameter);
    };
    if !matches!(target.as_ref(), "core.categorical" | "core.ordinal") {
        return Err(KernelError::InvalidParameter);
    }
    let mut conversion =
        SemanticConversion::from_parameters(target, "auto").ok_or(KernelError::InvalidParameter)?;
    conversion.domain = conversion_domain(
        invocation
            .parameter("semantic_domain")
            .ok_or(KernelError::InvalidParameter)?,
    )?;
    convert(invocation, conversion)
}

fn convert(
    invocation: &KernelInvocation<'_>,
    mut conversion: SemanticConversion,
) -> Result<RuntimeValue, KernelError> {
    let [original] = invocation.inputs else {
        return Err(KernelError::Failed);
    };
    let input = original.unannotated();
    let expected = ValueType::Scalar(conversion.target);
    let expected = if matches!(input, RuntimeValue::Series(_) | RuntimeValue::List(_)) {
        ValueType::DataSeries(Box::new(expected))
    } else {
        expected
    };
    if invocation.outputs.len() != 1 || invocation.outputs[0].data_type != expected {
        return Err(KernelError::Failed);
    }
    invocation.check_control()?;
    if let RuntimeValue::Series(series) = input {
        if let Some(domain) =
            InferredCategoricalDomain::for_conversion(series.plan().field(), &conversion)
                .map_err(|_| KernelError::InvalidParameter)?
        {
            conversion.domain = infer_domain(series, domain, invocation)?;
        }
        return series
            .relation()
            .convert_series(series, conversion)
            .map(RuntimeValue::Series)
            .map_err(kernel_error);
    }
    materialized(original, &conversion, invocation.control)
}

fn infer_domain(
    series: &SeriesHandle,
    mut domain: InferredCategoricalDomain,
    invocation: &KernelInvocation<'_>,
) -> Result<ConversionDomain, KernelError> {
    let control = invocation.relation_control();
    series
        .relation()
        .project_series(std::slice::from_ref(series))
        .map_err(kernel_error)?
        .visit_batches(&control, &mut |batch| {
            control.check()?;
            let batch_bytes = batch
                .num_rows()
                .checked_mul(size_of::<RuntimeValue>() * 4)
                .and_then(|bytes| {
                    bytes.checked_add(batch.get_array_memory_size().saturating_mul(4))
                })
                .ok_or(RelationError::MemoryLimitExceeded)?;
            let check_budget = |domain: &InferredCategoricalDomain| {
                batch_bytes
                    .checked_add(domain.retained_bytes())
                    .filter(|bytes| *bytes <= control.max_input_bytes)
                    .ok_or(RelationError::MemoryLimitExceeded)
            };
            check_budget(&domain)?;
            domain
                .extend(batch.column(0).as_ref())
                .map_err(|_| RelationError::InvalidConversion)?;
            check_budget(&domain)?;
            control.check()
        })
        .map_err(kernel_error)?;
    Ok(domain.finish())
}

pub(super) fn materialized(
    original: &RuntimeValue,
    conversion: &SemanticConversion,
    control: &KernelControl,
) -> Result<RuntimeValue, KernelError> {
    let metadata = original.metadata();
    let input = original.unannotated();
    let list = matches!(input, RuntimeValue::List(_));
    let inputs = match input {
        RuntimeValue::List(values) => values.as_ref(),
        value => std::slice::from_ref(value),
    };
    let mut budget =
        control.check_bytes(inputs.len().checked_mul(size_of::<RuntimeValue>() * 4))?;
    for (index, value) in inputs.iter().enumerate() {
        if index % 1024 == 0 {
            control.check()?;
        }
        if let RuntimeValue::Scalar(TabularScalar::String(value)) = value {
            budget = control.check_bytes(
                value
                    .len()
                    .checked_mul(4)
                    .and_then(|size| budget.checked_add(size)),
            )?;
        }
    }
    let mut values = control.reserve(inputs.len())?;
    for (index, input) in inputs.iter().enumerate() {
        if index % 1024 == 0 {
            control.check()?;
        }
        values.push(
            input
                .tabular_scalar()
                .map_err(|_| KernelError::InvalidParameter)?,
        );
    }
    let result = yss_database_arrow::convert_semantic_values(&values, metadata, conversion)
        .map_err(|_| KernelError::InvalidParameter)?;
    let metadata = result.metadata;
    let mut output_bytes =
        control.check_bytes(result.values.len().checked_mul(size_of::<RuntimeValue>()))?;
    for (index, value) in result.values.iter().enumerate() {
        if index % 1024 == 0 {
            control.check()?;
        }
        if let TabularScalar::String(value) = value {
            output_bytes = control.check_bytes(output_bytes.checked_add(value.len()))?;
        }
    }
    let mut values = control.reserve(result.values.len())?;
    for (index, value) in result.values.into_iter().enumerate() {
        if index % 1024 == 0 {
            control.check()?;
        }
        values.push(RuntimeValue::from(value));
    }
    let value = if list {
        RuntimeValue::List(values.into())
    } else {
        values.into_iter().next().ok_or(KernelError::Failed)?
    };
    value
        .with_metadata(metadata)
        .map_err(|_| KernelError::Failed)
}

fn conversion_domain(value: &RuntimeValue) -> Result<ConversionDomain, KernelError> {
    let RuntimeValue::Record(fields) = value else {
        return Err(KernelError::Failed);
    };
    if fields
        .keys()
        .any(|key| !matches!(key.as_ref(), "values" | "positiveValue"))
    {
        return Err(KernelError::Failed);
    }
    let values = match fields.get("values") {
        None => Vec::new(),
        Some(RuntimeValue::List(values)) => values
            .iter()
            .map(|value| {
                let RuntimeValue::Record(value) = value else {
                    return Err(KernelError::Failed);
                };
                let (
                    Some(RuntimeValue::Scalar(TabularScalar::String(code))),
                    Some(RuntimeValue::Scalar(TabularScalar::String(label))),
                ) = (value.get("value"), value.get("label"))
                else {
                    return Err(KernelError::Failed);
                };
                if value.len() != 2 {
                    return Err(KernelError::Failed);
                }
                Ok(SemanticValue {
                    value: code.to_string(),
                    label: label.to_string(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err(KernelError::Failed),
    };
    let positive_value = match fields.get("positiveValue") {
        None | Some(RuntimeValue::Scalar(TabularScalar::Null)) => None,
        Some(RuntimeValue::Scalar(TabularScalar::String(value))) => Some(value.to_string()),
        _ => return Err(KernelError::Failed),
    };
    let domain = ConversionDomain {
        values,
        positive_value,
    };
    if !domain.is_valid() {
        return Err(KernelError::Failed);
    }
    Ok(domain)
}
