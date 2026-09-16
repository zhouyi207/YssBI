use crate::KernelInvocation;
use yss_data_contract::ValueType;
use yss_relational_contract::{NumericOperation, NumericType, RelationLiteral, SeriesOperand};

use super::numeric_input;
use crate::KernelError;
use crate::RuntimeValue;

pub(crate) fn execute(
    operation: NumericOperation,
    invocation: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    let inputs = invocation.inputs;
    if inputs.len() < 2 || (operation != NumericOperation::Add && inputs.len() != 2) {
        return Err(KernelError::InvalidNumericInput);
    }
    let output_type = invocation
        .outputs
        .first()
        .map(|output| &output.data_type)
        .ok_or(KernelError::Failed)?;
    let representation = if operation == NumericOperation::Divide || inputs.iter().any(needs_float)
    {
        NumericType::Float64
    } else {
        NumericType::Int64
    };
    let ValueType::DataSeries(element) = output_type else {
        if *output_type != ValueType::Scalar(yss_data_contract::SemanticType::Numeric) {
            return Err(KernelError::InvalidNumericInput);
        }
        return scalar(operation, inputs.iter(), representation);
    };
    if **element != ValueType::Scalar(yss_data_contract::SemanticType::Numeric) {
        return Err(KernelError::InvalidNumericInput);
    }
    let numeric_type = representation;
    if operation == NumericOperation::Divide
        && !matches!(inputs[1], RuntimeValue::Series(_) | RuntimeValue::List(_))
        && numeric_input(inputs.get(1))? == 0.0
    {
        return Err(KernelError::DivisionByZero);
    }
    if let Some(RuntimeValue::Series(first)) = inputs
        .iter()
        .find(|value| matches!(value, RuntimeValue::Series(_)))
    {
        let operands = inputs
            .iter()
            .map(|value| match value {
                RuntimeValue::Series(series) => Ok(SeriesOperand::Series(series.clone())),
                RuntimeValue::Integer(value) => {
                    Ok(SeriesOperand::Scalar(RelationLiteral::Integer(*value)))
                }
                RuntimeValue::Unsigned(value) if numeric_type == NumericType::Int64 => {
                    Ok(SeriesOperand::Scalar(RelationLiteral::Integer(
                        i64::try_from(*value).map_err(|_| KernelError::InvalidNumericInput)?,
                    )))
                }
                RuntimeValue::Decimal(_) | RuntimeValue::Unsigned(_) => Ok(SeriesOperand::Scalar(
                    RelationLiteral::Decimal(numeric_input(Some(value))?.to_string().into()),
                )),
                // A materialized list carries no proof of alignment with a live row domain.
                _ => Err(KernelError::InvalidNumericInput),
            })
            .collect::<Result<Vec<_>, _>>()?;
        return first
            .relation()
            .numeric_series(operation, &operands, numeric_type)
            .map(RuntimeValue::Series)
            .map_err(super::relational::kernel_error);
    }

    let rows = inputs
        .iter()
        .find_map(|value| match value {
            RuntimeValue::List(values) => Some(values.len()),
            _ => None,
        })
        .ok_or(KernelError::InvalidNumericInput)?;
    for input in inputs {
        match input {
            RuntimeValue::List(values) if values.len() == rows => {}
            RuntimeValue::List(_) => return Err(KernelError::InvalidNumericInput),
            value => {
                numeric_input(Some(value))?;
            }
        }
    }
    let bytes = rows
        .checked_mul(std::mem::size_of::<RuntimeValue>())
        .ok_or(KernelError::Failed)?;
    if bytes > invocation.control.max_input_bytes {
        return Err(KernelError::Failed);
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(rows)
        .map_err(|_| KernelError::Failed)?;
    for row in 0..rows {
        if row % 1024 == 0 {
            invocation.check_control()?;
        }
        let values = inputs.iter().map(|input| match input {
            RuntimeValue::List(values) => &values[row],
            value => value,
        });
        result.push(scalar(operation, values, representation)?);
    }
    invocation.check_control()?;
    Ok(RuntimeValue::List(result.into_boxed_slice()))
}

fn integer(value: Option<&RuntimeValue>) -> Result<i64, KernelError> {
    match value {
        Some(RuntimeValue::Integer(value)) => Ok(*value),
        Some(RuntimeValue::Unsigned(value)) => {
            i64::try_from(*value).map_err(|_| KernelError::InvalidNumericInput)
        }
        _ => Err(KernelError::InvalidNumericInput),
    }
}

fn scalar<'a>(
    operation: NumericOperation,
    mut values: impl Iterator<Item = &'a RuntimeValue>,
    representation: NumericType,
) -> Result<RuntimeValue, KernelError> {
    match representation {
        NumericType::Int64 => {
            let mut result = integer(values.next())?;
            for right in values {
                let right = integer(Some(right))?;
                result = match operation {
                    NumericOperation::Add => result.checked_add(right),
                    NumericOperation::Subtract => result.checked_sub(right),
                    NumericOperation::Multiply => result.checked_mul(right),
                    NumericOperation::Divide => {
                        return Err(KernelError::InvalidNumericInput);
                    }
                }
                .ok_or(KernelError::NonFiniteResult)?;
            }
            Ok(RuntimeValue::Integer(result))
        }
        NumericType::Float64 => {
            let mut result = numeric_input(values.next())?;
            for right in values {
                let right = numeric_input(Some(right))?;
                result = match operation {
                    NumericOperation::Add => result + right,
                    NumericOperation::Subtract => result - right,
                    NumericOperation::Multiply => result * right,
                    NumericOperation::Divide if right == 0.0 => {
                        return Err(KernelError::DivisionByZero);
                    }
                    NumericOperation::Divide => result / right,
                };
                if !result.is_finite() {
                    return Err(KernelError::NonFiniteResult);
                }
            }
            Ok(RuntimeValue::Decimal(result))
        }
    }
}

fn needs_float(value: &RuntimeValue) -> bool {
    match value {
        RuntimeValue::Decimal(_) => true,
        RuntimeValue::List(values) => values.iter().any(needs_float),
        RuntimeValue::Series(series) => !series.plan().field().data_type().is_integer(),
        _ => false,
    }
}
