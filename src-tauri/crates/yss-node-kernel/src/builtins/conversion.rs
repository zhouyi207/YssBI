use yss_data_contract::{
    ConversionDomain, DatetimeRepresentation, SemanticConversion, SemanticValue, TemporalPrecision,
    ValueType,
};

use yss_data_contract::TabularScalar;

use crate::{KernelControl, KernelError, KernelInvocation, RuntimeValue};

pub(super) fn execute(invocation: &KernelInvocation<'_>) -> Result<RuntimeValue, KernelError> {
    let Some(RuntimeValue::Scalar(TabularScalar::String(target))) =
        invocation.parameter("target_type")
    else {
        return Err(KernelError::Failed);
    };
    let Some(RuntimeValue::Scalar(TabularScalar::String(numeric))) =
        invocation.parameter("numeric_mode")
    else {
        return Err(KernelError::Failed);
    };
    let target = if target.as_ref() == "auto" {
        let [output] = invocation.outputs else {
            return Err(KernelError::Failed);
        };
        match &output.data_type {
            ValueType::Scalar(semantic) => semantic.type_id(),
            ValueType::DataSeries(element) => match element.as_ref() {
                ValueType::Scalar(semantic) => semantic.type_id(),
                _ => return Err(KernelError::Failed),
            },
            _ => return Err(KernelError::Failed),
        }
    } else {
        target.as_ref()
    };
    let mut conversion =
        SemanticConversion::from_parameters(target, numeric).ok_or(KernelError::Failed)?;
    conversion.domain = conversion_domain(
        invocation
            .parameter("semantic_domain")
            .ok_or(KernelError::Failed)?,
    )?;
    let text = |name| match invocation.parameter(name) {
        Some(RuntimeValue::Scalar(TabularScalar::String(value))) => Ok(value.as_ref()),
        _ => Err(KernelError::Failed),
    };
    conversion.datetime = DatetimeRepresentation::from_parameter(text("datetime_kind")?)
        .ok_or(KernelError::Failed)?;
    conversion.precision = TemporalPrecision::from_parameter(text("datetime_precision")?)
        .ok_or(KernelError::Failed)?;
    conversion.format = text("datetime_format")?.into();
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
    conversion: SemanticConversion,
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
        return series
            .relation()
            .convert_series(series, conversion)
            .map(RuntimeValue::Series)
            .map_err(super::relational::kernel_error);
    }
    materialized(original, &conversion, invocation.control)
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
