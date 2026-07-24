use super::support::{KernelFragment, expect_arity, expect_min_arity};
use super::value::canonical_float;
use crate::node_system::protocol::{CanonicalDecimal, Value};
use crate::node_system::runtime::{Artifact, Kernel, KernelContext, KernelError, RuntimeValue};

#[derive(Clone, Copy)]
enum BinaryOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
}

#[derive(Clone, Copy)]
enum UnaryOperation {
    Ln,
    Log2,
    Log10,
    Exp,
    Sqrt,
    Square,
}

pub(super) fn register(fragment: &mut KernelFragment) {
    for (handle, operation) in [
        ("yssbi.numeric.series.add", BinaryOperation::Add),
        ("yssbi.numeric.series.subtract", BinaryOperation::Subtract),
        ("yssbi.numeric.series.multiply", BinaryOperation::Multiply),
        ("yssbi.numeric.series.divide", BinaryOperation::Divide),
    ] {
        fragment.register(handle, SeriesMathKernel { operation });
    }
    for (handle, operation) in [
        ("yssbi.numeric.ln", UnaryOperation::Ln),
        ("yssbi.numeric.log2", UnaryOperation::Log2),
        ("yssbi.numeric.log10", UnaryOperation::Log10),
        ("yssbi.numeric.exp", UnaryOperation::Exp),
        ("yssbi.numeric.sqrt", UnaryOperation::Sqrt),
        ("yssbi.numeric.square", UnaryOperation::Square),
    ] {
        fragment.register(handle, UnaryMathKernel { operation });
    }
}

struct SeriesMathKernel {
    operation: BinaryOperation,
}

impl Kernel for SeriesMathKernel {
    fn execute(
        &self,
        _context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        if matches!(self.operation, BinaryOperation::Add) {
            expect_min_arity(inputs, 2)?;
        } else {
            expect_arity(inputs, 2)?;
        }
        let artifact = inputs
            .iter()
            .find_map(|input| match input {
                RuntimeValue::Artifact(artifact) => Some(artifact),
                _ => None,
            })
            .ok_or_else(|| {
                KernelError::new("DataSeries arithmetic requires at least one artifact operand")
            })?;
        let length = artifact.values().len();
        for input in inputs {
            match input {
                RuntimeValue::Artifact(candidate) if candidate.values().len() != length => {
                    return Err(KernelError::new(
                        "DataSeries operands must have equal lengths",
                    ));
                }
                RuntimeValue::Artifact(_) | RuntimeValue::Scalar(_) => {}
                RuntimeValue::Stream(_) => {
                    return Err(KernelError::new(
                        "DataSeries arithmetic requires fully materialized inputs",
                    ));
                }
            }
        }

        let mut output = Vec::with_capacity(length);
        for index in 0..length {
            let mut values = inputs
                .iter()
                .map(|input| value_at(input, index))
                .collect::<Result<Vec<_>, _>>()?;
            if values.iter().any(|value| value.is_none()) {
                output.push(Value::Null);
                continue;
            }
            let first = values.remove(0).expect("nulls returned above");
            let result = values.into_iter().try_fold(first, |left, right| {
                apply_binary(self.operation, left, right.expect("nulls returned above"))
            })?;
            output.push(Value::Decimal(canonical_float(result)?));
        }
        Ok(vec![RuntimeValue::Artifact(Artifact::new(
            artifact.kind(),
            output,
        ))])
    }
}

struct UnaryMathKernel {
    operation: UnaryOperation,
}

impl Kernel for UnaryMathKernel {
    fn execute(
        &self,
        _context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        expect_arity(inputs, 1)?;
        match &inputs[0] {
            RuntimeValue::Scalar(Value::Null) => Ok(vec![Value::Null.into()]),
            RuntimeValue::Scalar(value) => Ok(vec![
                Value::Decimal(apply_unary(self.operation, numeric(value)?)?).into(),
            ]),
            RuntimeValue::Artifact(artifact) => {
                let values = artifact
                    .values()
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        if matches!(value, Value::Null) {
                            Ok(Value::Null)
                        } else {
                            apply_unary(self.operation, numeric(value)?).map(Value::Decimal)
                        }
                        .map_err(|error| {
                            KernelError::new(format!("DataSeries element {index}: {error}"))
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(vec![RuntimeValue::Artifact(Artifact::new(
                    artifact.kind(),
                    values,
                ))])
            }
            RuntimeValue::Stream(_) => Err(KernelError::new(
                "unary math requires a scalar or fully materialized DataSeries",
            )),
        }
    }
}

fn value_at(input: &RuntimeValue, index: usize) -> Result<Option<f64>, KernelError> {
    let value = match input {
        RuntimeValue::Scalar(value) => value,
        RuntimeValue::Artifact(artifact) => &artifact.values()[index],
        RuntimeValue::Stream(_) => unreachable!("streams rejected before evaluation"),
    };
    if matches!(value, Value::Null) {
        Ok(None)
    } else {
        numeric(value).map(Some)
    }
}

fn numeric(value: &Value) -> Result<f64, KernelError> {
    let value = match value {
        Value::Integer(value) => *value as f64,
        Value::Decimal(value) => parse_decimal(value)?,
        _ => {
            return Err(KernelError::new(
                "numeric operation expects Int64 or Float64 values",
            ));
        }
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(KernelError::new("numeric value must be finite"))
    }
}

fn parse_decimal(value: &CanonicalDecimal) -> Result<f64, KernelError> {
    value
        .as_str()
        .parse::<f64>()
        .map_err(|_| KernelError::new("invalid Float64 decimal"))
}

fn apply_binary(operation: BinaryOperation, left: f64, right: f64) -> Result<f64, KernelError> {
    if matches!(operation, BinaryOperation::Divide) && right == 0.0 {
        return Err(KernelError::new("Float64 division by zero"));
    }
    let result = match operation {
        BinaryOperation::Add => left + right,
        BinaryOperation::Subtract => left - right,
        BinaryOperation::Multiply => left * right,
        BinaryOperation::Divide => left / right,
    };
    if result.is_finite() {
        Ok(result)
    } else {
        Err(KernelError::new(
            "Float64 arithmetic produced a non-finite result",
        ))
    }
}

fn apply_unary(operation: UnaryOperation, input: f64) -> Result<CanonicalDecimal, KernelError> {
    let result = match operation {
        UnaryOperation::Ln => input.ln(),
        UnaryOperation::Log2 => input.log2(),
        UnaryOperation::Log10 => input.log10(),
        UnaryOperation::Exp => input.exp(),
        UnaryOperation::Sqrt => input.sqrt(),
        UnaryOperation::Square => input * input,
    };
    canonical_float(result)
}
