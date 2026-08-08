use super::support::{KernelFragment, expect_arity, expect_min_arity};
use super::value::canonical_float;
use crate::node_system::protocol::{CanonicalDecimal, Value};
use crate::node_system::runtime::{
    ArtifactCursor, Kernel, KernelContext, KernelError, RunError, RuntimeValue,
};

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
        context: &KernelContext<'_>,
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
        let length = artifact.materialized().len();
        let mut cursors = inputs
            .iter()
            .map(|input| match input {
                RuntimeValue::Scalar(value) => Ok(SeriesInput::Scalar(value)),
                RuntimeValue::Artifact(candidate) if candidate.materialized().len() == length => {
                    candidate
                        .cursor()
                        .map(SeriesInput::Artifact)
                        .map_err(kernel_error)
                }
                RuntimeValue::Artifact(_) => Err(KernelError::new(
                    "DataSeries operands must have equal lengths",
                )),
                RuntimeValue::Stream(_) => Err(KernelError::new(
                    "DataSeries arithmetic requires fully materialized inputs",
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let operation = self.operation;
        let mut index = 0_usize;
        let output = std::iter::from_fn(move || {
            if index == length {
                return None;
            }
            index += 1;
            Some(series_binary_value(operation, &mut cursors).map_err(run_error))
        });
        let output = context
            .resource_owner
            .materialize_artifact(artifact.kind(), output)
            .map_err(kernel_error)?;
        Ok(vec![RuntimeValue::Artifact(output)])
    }
}

struct UnaryMathKernel {
    operation: UnaryOperation,
}

impl Kernel for UnaryMathKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        expect_arity(inputs, 1)?;
        match &inputs[0] {
            RuntimeValue::Scalar(Value::Null) => Ok(vec![Value::Null.into()]),
            RuntimeValue::Scalar(value) => Ok(vec![
                Value::Decimal(apply_unary(self.operation, numeric(value)?)?).into(),
            ]),
            RuntimeValue::Artifact(artifact) => {
                let operation = self.operation;
                let values = artifact.cursor().map_err(kernel_error)?.enumerate().map(
                    move |(index, value)| {
                        let value = value?;
                        let converted = if matches!(value, Value::Null) {
                            Ok(Value::Null)
                        } else {
                            numeric(&value)
                                .and_then(|value| apply_unary(operation, value))
                                .map(Value::Decimal)
                        };
                        converted.map_err(|error| {
                            RunError::Stream(format!("DataSeries element {index}: {error}").into())
                        })
                    },
                );
                let output = context
                    .resource_owner
                    .materialize_artifact(artifact.kind(), values)
                    .map_err(kernel_error)?;
                Ok(vec![RuntimeValue::Artifact(output)])
            }
            RuntimeValue::Stream(_) => Err(KernelError::new(
                "unary math requires a scalar or fully materialized DataSeries",
            )),
        }
    }
}

enum SeriesInput<'a> {
    Scalar(&'a Value),
    Artifact(ArtifactCursor),
}

impl SeriesInput<'_> {
    fn next_value(&mut self) -> Result<Value, KernelError> {
        match self {
            Self::Scalar(value) => Ok((*value).clone()),
            Self::Artifact(values) => values
                .next()
                .ok_or_else(|| KernelError::new("DataSeries ended before its declared length"))?
                .map_err(kernel_error),
        }
    }
}

fn series_binary_value(
    operation: BinaryOperation,
    inputs: &mut [SeriesInput<'_>],
) -> Result<Value, KernelError> {
    let values = inputs
        .iter_mut()
        .map(SeriesInput::next_value)
        .collect::<Result<Vec<_>, _>>()?;
    if values.iter().any(|value| matches!(value, Value::Null)) {
        return Ok(Value::Null);
    }
    let mut values = values.iter().map(numeric);
    let first = values
        .next()
        .expect("series arithmetic has at least two inputs")?;
    let result = values.try_fold(first, |left, right| apply_binary(operation, left, right?))?;
    Ok(Value::Decimal(canonical_float(result)?))
}

fn kernel_error(error: RunError) -> KernelError {
    KernelError::new(error.to_string())
}

fn run_error(error: KernelError) -> RunError {
    RunError::Stream(error.to_string().into())
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
