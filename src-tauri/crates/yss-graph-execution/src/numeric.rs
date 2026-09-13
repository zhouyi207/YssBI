use yss_data_contract::DataType;
use yss_relational_contract::{NumericOperation, NumericType, RelationLiteral, SeriesOperand};

use crate::state::{
    KernelExecutionError, PreparedKernelInvocation, check_kernel_control, numeric_input,
};
use crate::value::RuntimeValue;

pub(crate) fn execute(
    operation: NumericOperation,
    invocation: &PreparedKernelInvocation<'_>,
) -> Result<RuntimeValue, KernelExecutionError> {
    let inputs = invocation.inputs;
    if inputs.len() < 2 || (operation != NumericOperation::Add && inputs.len() != 2) {
        return Err(KernelExecutionError::InvalidNumericInput);
    }
    let output_type = invocation
        .specialization
        .output_types()
        .first()
        .map(crate::plan::PlanTypeBinding::data_type)
        .ok_or(KernelExecutionError::Failed)?;
    let DataType::DataSeries(element) = output_type else {
        return scalar(operation, inputs.iter(), output_type);
    };
    let numeric_type = match element.as_ref() {
        DataType::Int64 => NumericType::Int64,
        DataType::Float64 => NumericType::Float64,
        _ => return Err(KernelExecutionError::InvalidNumericInput),
    };
    if operation == NumericOperation::Divide
        && !matches!(inputs[1], RuntimeValue::Series(_) | RuntimeValue::List(_))
        && numeric_input(inputs.get(1))? == 0.0
    {
        return Err(KernelExecutionError::DivisionByZero);
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
                        i64::try_from(*value)
                            .map_err(|_| KernelExecutionError::InvalidNumericInput)?,
                    )))
                }
                RuntimeValue::Decimal(_) | RuntimeValue::Unsigned(_) => Ok(SeriesOperand::Scalar(
                    RelationLiteral::Decimal(numeric_input(Some(value))?.to_string().into()),
                )),
                // A materialized list carries no proof of alignment with a live row domain.
                _ => Err(KernelExecutionError::InvalidNumericInput),
            })
            .collect::<Result<Vec<_>, _>>()?;
        return first
            .relation()
            .numeric_series(operation, &operands, numeric_type)
            .map(RuntimeValue::Series)
            .map_err(crate::relational::kernel_error);
    }

    let rows = inputs
        .iter()
        .find_map(|value| match value {
            RuntimeValue::List(values) => Some(values.len()),
            _ => None,
        })
        .ok_or(KernelExecutionError::InvalidNumericInput)?;
    for input in inputs {
        match input {
            RuntimeValue::List(values) if values.len() == rows => {}
            RuntimeValue::List(_) => return Err(KernelExecutionError::InvalidNumericInput),
            value => {
                numeric_input(Some(value))?;
            }
        }
    }
    let bytes = rows
        .checked_mul(std::mem::size_of::<RuntimeValue>())
        .ok_or(KernelExecutionError::Failed)?;
    if bytes > crate::relational::MAX_NUMERIC_INPUT_BYTES {
        return Err(KernelExecutionError::Failed);
    }
    let mut result = Vec::new();
    result
        .try_reserve_exact(rows)
        .map_err(|_| KernelExecutionError::Failed)?;
    for row in 0..rows {
        if row % 1024 == 0 {
            check_kernel_control(invocation.control)?;
        }
        let values = inputs.iter().map(|input| match input {
            RuntimeValue::List(values) => &values[row],
            value => value,
        });
        result.push(scalar(operation, values, element)?);
    }
    check_kernel_control(invocation.control)?;
    Ok(RuntimeValue::List(result.into_boxed_slice()))
}

fn integer(value: Option<&RuntimeValue>) -> Result<i64, KernelExecutionError> {
    match value {
        Some(RuntimeValue::Integer(value)) => Ok(*value),
        Some(RuntimeValue::Unsigned(value)) => {
            i64::try_from(*value).map_err(|_| KernelExecutionError::InvalidNumericInput)
        }
        _ => Err(KernelExecutionError::InvalidNumericInput),
    }
}

fn scalar<'a>(
    operation: NumericOperation,
    mut values: impl Iterator<Item = &'a RuntimeValue>,
    output_type: &DataType,
) -> Result<RuntimeValue, KernelExecutionError> {
    match output_type {
        DataType::Int64 => {
            let mut result = integer(values.next())?;
            for right in values {
                let right = integer(Some(right))?;
                result = match operation {
                    NumericOperation::Add => result.checked_add(right),
                    NumericOperation::Subtract => result.checked_sub(right),
                    NumericOperation::Multiply => result.checked_mul(right),
                    NumericOperation::Divide => {
                        return Err(KernelExecutionError::InvalidNumericInput);
                    }
                }
                .ok_or(KernelExecutionError::NonFiniteResult)?;
            }
            Ok(RuntimeValue::Integer(result))
        }
        DataType::Float64 => {
            let mut result = numeric_input(values.next())?;
            for right in values {
                let right = numeric_input(Some(right))?;
                result = match operation {
                    NumericOperation::Add => result + right,
                    NumericOperation::Subtract => result - right,
                    NumericOperation::Multiply => result * right,
                    NumericOperation::Divide if right == 0.0 => {
                        return Err(KernelExecutionError::DivisionByZero);
                    }
                    NumericOperation::Divide => result / right,
                };
                if !result.is_finite() {
                    return Err(KernelExecutionError::NonFiniteResult);
                }
            }
            Ok(RuntimeValue::Decimal(result))
        }
        _ => Err(KernelExecutionError::InvalidNumericInput),
    }
}
