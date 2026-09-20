use crate::{KernelError, KernelInvocation, RuntimeValue};
use yss_data_contract::TabularScalar;
use yss_data_contract::{SemanticType, ValueType};
use yss_relational_contract::{BooleanOperand, BooleanOperation};

fn scalar(value: &RuntimeValue) -> Result<Option<bool>, KernelError> {
    match value {
        RuntimeValue::Scalar(TabularScalar::Bool(value)) => Ok(Some(*value)),
        RuntimeValue::Scalar(TabularScalar::Null) => Ok(None),
        _ => Err(KernelError::Failed),
    }
}

pub(super) fn execute(
    operation: BooleanOperation,
    invocation: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    if invocation.inputs.len() != operation.arity() {
        return Err(KernelError::Failed);
    }
    invocation.check_control()?;
    let inputs = invocation.inputs;
    let is_series = inputs
        .iter()
        .any(|v| matches!(v, RuntimeValue::List(_) | RuntimeValue::Series(_)));
    let binary = ValueType::Scalar(SemanticType::Binary);
    let expected = if is_series {
        ValueType::DataSeries(Box::new(binary))
    } else {
        binary
    };
    if invocation.outputs.len() != 1 || invocation.outputs[0].data_type != expected {
        return Err(KernelError::Failed);
    }
    if let Some(RuntimeValue::Series(first)) =
        inputs.iter().find(|v| matches!(v, RuntimeValue::Series(_)))
    {
        let operands = inputs
            .iter()
            .map(|value| match value {
                RuntimeValue::Series(series) => Ok(BooleanOperand::Series(series.clone())),
                value => scalar(value).map(BooleanOperand::Scalar),
            })
            .collect::<Result<Vec<_>, _>>()?;
        return first
            .relation()
            .boolean_series(operation, &operands)
            .map(RuntimeValue::Series)
            .map_err(super::relational::kernel_error);
    }
    let rows = inputs.iter().find_map(|v| match v {
        RuntimeValue::List(v) => Some(v.len()),
        _ => None,
    });
    let mut values = [None; 2];
    let values = &mut values[..operation.arity()];
    let evaluate = |values: &[Option<bool>]| {
        operation
            .evaluate(values)
            .map(|v| {
                v.map_or(RuntimeValue::Scalar(TabularScalar::Null), |value| {
                    RuntimeValue::Scalar(TabularScalar::Bool(value))
                })
            })
            .map_err(super::relational::kernel_error)
    };
    let Some(rows) = rows else {
        for (value, input) in values.iter_mut().zip(inputs) {
            *value = scalar(input)?;
        }
        return evaluate(values);
    };
    for input in inputs {
        match input {
            RuntimeValue::List(values) if values.len() == rows => {}
            RuntimeValue::List(_) => return Err(KernelError::ShapeMismatch),
            value => {
                scalar(value)?;
            }
        }
    }
    let mut output = invocation.control.reserve(rows)?;
    for row in 0..rows {
        if row % 1024 == 0 {
            invocation.check_control()?;
        }
        for (value, input) in values.iter_mut().zip(inputs) {
            *value = scalar(match input {
                RuntimeValue::List(values) => &values[row],
                value => value,
            })?;
        }
        output.push(evaluate(values)?);
    }
    invocation.check_control()?;
    Ok(RuntimeValue::List(output.into()))
}
