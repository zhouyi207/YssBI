use crate::{KernelError, KernelInvocation, RuntimeValue};
use yss_data_contract::TabularScalar;
use yss_data_contract::{SemanticType, ValueType};
use yss_relational_contract::{ComparisonOperand, ComparisonOperation};

pub(super) fn execute(
    operation: ComparisonOperation,
    invocation: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    let tolerance = {
        match invocation.parameter("mode").map(RuntimeValue::unannotated) {
            Some(RuntimeValue::Scalar(TabularScalar::String(mode))) if mode.as_ref() == "exact" => {
                None
            }
            Some(RuntimeValue::Scalar(TabularScalar::String(mode)))
                if mode.as_ref() == "tolerance" =>
            {
                let read = |key: &str| -> Result<f64, KernelError> {
                    super::numeric_input(invocation.parameter(key))
                        .map_err(|_| KernelError::InvalidParameter)
                };
                Some(
                    yss_relational_contract::NumericTolerance::new(
                        read("absolute_tolerance")?,
                        read("relative_tolerance")?,
                    )
                    .map_err(|_| KernelError::InvalidParameter)?,
                )
            }
            _ => return Err(KernelError::InvalidParameter),
        }
    };
    execute_with_tolerance(operation, tolerance, invocation)
}

fn execute_with_tolerance(
    operation: ComparisonOperation,
    tolerance: Option<yss_relational_contract::NumericTolerance>,
    invocation: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    let compare = |left: &RuntimeValue, right: &RuntimeValue| {
        if let Some(tolerance) = tolerance {
            if [left, right]
                .iter()
                .any(|v| matches!(v.unannotated(), RuntimeValue::Scalar(TabularScalar::Null)))
            {
                return Ok(RuntimeValue::Scalar(TabularScalar::Null));
            }
            let equal = tolerance
                .evaluate(
                    operation,
                    super::numeric_input(Some(left))?,
                    super::numeric_input(Some(right))?,
                )
                .map_err(super::relational::kernel_error)?;
            Ok(RuntimeValue::Scalar(TabularScalar::Bool(equal)))
        } else {
            scalar(operation, left, right)
        }
    };
    let [left, right] = invocation.inputs else {
        return Err(KernelError::Failed);
    };
    if !matches!(
        operation,
        ComparisonOperation::Equal | ComparisonOperation::NotEqual
    ) && [left, right].iter().any(|value| {
        value.metadata().is_some_and(|metadata| {
            !matches!(
                metadata.semantic.kind,
                SemanticType::Numeric | SemanticType::Text
            )
        })
    }) {
        return Err(KernelError::Failed);
    }
    if tolerance.is_some()
        && [left, right].iter().any(|value| {
            value
                .metadata()
                .is_some_and(|metadata| metadata.semantic.kind != SemanticType::Numeric)
        })
    {
        return Err(KernelError::Failed);
    }
    let inputs = [left.unannotated(), right.unannotated()];
    let series = inputs
        .iter()
        .any(|v| matches!(v, RuntimeValue::List(_) | RuntimeValue::Series(_)));
    let scalar_type = ValueType::Scalar(SemanticType::Binary);
    let expected = if series {
        ValueType::DataSeries(Box::new(scalar_type))
    } else {
        scalar_type
    };
    if invocation.outputs.len() != 1 || invocation.outputs[0].data_type != expected {
        return Err(KernelError::Failed);
    }
    invocation.check_control()?;
    if let Some(RuntimeValue::Series(first)) = inputs
        .iter()
        .copied()
        .find(|v| matches!(v, RuntimeValue::Series(_)))
    {
        let operands = inputs
            .iter()
            .map(|v| match v {
                RuntimeValue::Series(s) => Ok(ComparisonOperand::Series(s.clone())),
                v => v
                    .tabular_scalar()
                    .map(ComparisonOperand::Scalar)
                    .map_err(|_| KernelError::Failed),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(tolerance) = tolerance {
            return first
                .relation()
                .compare_series_with_tolerance(operation, &operands, tolerance)
                .map(RuntimeValue::Series)
                .map_err(super::relational::kernel_error);
        }
        return first
            .relation()
            .compare_series(operation, &operands)
            .map(RuntimeValue::Series)
            .map_err(super::relational::kernel_error);
    }
    let rows = inputs.iter().find_map(|v| match v {
        RuntimeValue::List(v) => Some(v.len()),
        _ => None,
    });
    let Some(rows) = rows else {
        return compare(inputs[0], inputs[1]);
    };
    for input in inputs {
        match input {
            RuntimeValue::List(v) if v.len() == rows => {}
            RuntimeValue::List(_) => return Err(KernelError::ShapeMismatch),
            v => {
                v.tabular_scalar().map_err(|_| KernelError::Failed)?;
            }
        }
    }
    let mut result = invocation.control.reserve(rows)?;
    for row in 0..rows {
        if row % 1024 == 0 {
            invocation.check_control()?;
        }
        let [left, right] = inputs.map(|v| match v {
            RuntimeValue::List(v) => &v[row],
            v => v,
        });
        result.push(compare(left, right)?);
    }
    invocation.check_control()?;
    Ok(RuntimeValue::List(result.into()))
}

fn scalar(
    operation: ComparisonOperation,
    left: &RuntimeValue,
    right: &RuntimeValue,
) -> Result<RuntimeValue, KernelError> {
    if let (
        RuntimeValue::Scalar(TabularScalar::String(left)),
        RuntimeValue::Scalar(TabularScalar::String(right)),
    ) = (left, right)
    {
        return Ok(RuntimeValue::Scalar(TabularScalar::Bool(
            operation.evaluate(left.cmp(right)),
        )));
    }
    let left = left.tabular_scalar().map_err(|_| KernelError::Failed)?;
    let right = right.tabular_scalar().map_err(|_| KernelError::Failed)?;
    if !matches!(
        operation,
        ComparisonOperation::Equal | ComparisonOperation::NotEqual
    ) && (matches!(left, TabularScalar::Bool(_)) || matches!(right, TabularScalar::Bool(_)))
    {
        return Err(KernelError::Failed);
    }
    if matches!(left, TabularScalar::Null) || matches!(right, TabularScalar::Null) {
        return Ok(RuntimeValue::Scalar(TabularScalar::Null));
    }
    left.compare(&right)
        .map(|order| RuntimeValue::Scalar(TabularScalar::Bool(operation.evaluate(order))))
        .ok_or(KernelError::Failed)
}
