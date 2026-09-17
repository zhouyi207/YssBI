use yss_data_contract::{
    ConversionDomain, DatetimeRepresentation, SemanticConversion, SemanticValue, TemporalPrecision,
    ValueType,
};

use yss_tabular_contract::TabularScalar;

use crate::{KernelError, KernelInvocation, RuntimeValue};

pub(super) fn execute(invocation: &KernelInvocation<'_>) -> Result<RuntimeValue, KernelError> {
    let [original] = invocation.inputs else {
        return Err(KernelError::Failed);
    };
    let metadata = original.metadata();
    let input = original.unannotated();
    let Some(RuntimeValue::String(target)) = invocation.parameter("target_type") else {
        return Err(KernelError::Failed);
    };
    let Some(RuntimeValue::String(numeric)) = invocation.parameter("numeric_mode") else {
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
        Some(RuntimeValue::String(value)) => Ok(value.as_ref()),
        _ => Err(KernelError::Failed),
    };
    conversion.datetime = DatetimeRepresentation::from_parameter(text("datetime_kind")?)
        .ok_or(KernelError::Failed)?;
    conversion.precision = TemporalPrecision::from_parameter(text("datetime_precision")?)
        .ok_or(KernelError::Failed)?;
    conversion.format = text("datetime_format")?.into();
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
    let list = matches!(input, RuntimeValue::List(_));
    let inputs = match input {
        RuntimeValue::List(values) => values.as_ref(),
        value => std::slice::from_ref(value),
    };
    let mut budget = inputs
        .len()
        .checked_mul(std::mem::size_of::<RuntimeValue>() * 4)
        .ok_or(KernelError::Failed)?;
    for (index, value) in inputs.iter().enumerate() {
        if index % 1024 == 0 {
            invocation.check_control()?;
        }
        if let RuntimeValue::String(value) = value {
            budget = budget
                .checked_add(value.len().checked_mul(4).ok_or(KernelError::Failed)?)
                .ok_or(KernelError::Failed)?;
        }
    }
    if budget > invocation.control.max_input_bytes {
        return Err(KernelError::Failed);
    }
    let values = inputs
        .iter()
        .map(|input| {
            Ok(match input {
                RuntimeValue::Null => TabularScalar::Null,
                RuntimeValue::Bool(value) => TabularScalar::Bool(*value),
                RuntimeValue::Integer(value) => TabularScalar::Integer(*value),
                RuntimeValue::Unsigned(value) => TabularScalar::Unsigned(*value),
                RuntimeValue::Decimal(value) => {
                    TabularScalar::Decimal((*value).try_into().map_err(|_| KernelError::Failed)?)
                }
                RuntimeValue::String(value) => TabularScalar::String(value.clone()),
                _ => return Err(KernelError::Failed),
            })
        })
        .collect::<Result<Vec<_>, KernelError>>()?;
    let result = yss_tabular_arrow::convert_semantic_values(&values, metadata, &conversion)
        .map_err(|_| KernelError::Failed)?;
    let metadata = result.metadata;
    let values = result.values;
    invocation.check_control()?;
    let mut output_bytes = values
        .len()
        .checked_mul(std::mem::size_of::<RuntimeValue>())
        .ok_or(KernelError::Failed)?;
    let values = values
        .into_iter()
        .map(|value| {
            Ok(match value {
                TabularScalar::Null => RuntimeValue::Null,
                TabularScalar::Bool(value) => RuntimeValue::Bool(value),
                TabularScalar::Integer(value) => RuntimeValue::Integer(value),
                TabularScalar::Unsigned(value) => RuntimeValue::Unsigned(value),
                TabularScalar::Decimal(value) => RuntimeValue::Decimal(value.as_f64()),
                TabularScalar::String(value) => {
                    output_bytes = output_bytes
                        .checked_add(value.len())
                        .ok_or(KernelError::Failed)?;
                    RuntimeValue::String(value)
                }
            })
        })
        .collect::<Result<Vec<_>, KernelError>>()?;
    if output_bytes > invocation.control.max_input_bytes {
        return Err(KernelError::Failed);
    }
    let value = if list {
        RuntimeValue::List(values.into_boxed_slice())
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
                let (Some(RuntimeValue::String(code)), Some(RuntimeValue::String(label))) =
                    (value.get("value"), value.get("label"))
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
        None | Some(RuntimeValue::Null) => None,
        Some(RuntimeValue::String(value)) => Some(value.to_string()),
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
