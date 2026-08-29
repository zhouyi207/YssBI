use super::support::{KernelFragment, expect_arity, expect_min_arity};
use super::value::canonical_float;
use crate::graph::protocol::{CanonicalDecimal, Value};
use crate::node_system::runtime::{
    Artifact, ArtifactKind, DataSeriesBuilder, DataSeriesElementType, DataSeriesMetadata, Kernel,
    KernelContext, KernelError, NullPolicy, NumericSeriesView, RuntimeValue, checked_int64_to_f64,
    numeric_series, require_data_series,
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
        _context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        if matches!(self.operation, BinaryOperation::Add) {
            expect_min_arity(inputs, 2)?;
        } else {
            expect_arity(inputs, 2)?;
        }
        let (operands, metadata, kind) = read_operands(inputs)?;
        let float_output = matches!(self.operation, BinaryOperation::Divide)
            || operands.iter().any(NumericOperand::is_float);
        let values = (0..metadata.length)
            .map(|index| evaluate_row(self.operation, &operands, index, float_output))
            .collect::<Result<Vec<_>, _>>()?;
        let element_type = if float_output {
            DataSeriesElementType::Float64
        } else {
            DataSeriesElementType::Int64
        };
        Ok(vec![RuntimeValue::Artifact(build_series(
            element_type,
            &metadata,
            kind,
            values,
        )?)])
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
                Value::Decimal(apply_unary(self.operation, scalar_float(value)?)?).into(),
            ]),
            RuntimeValue::Artifact(_) => {
                let artifact = require_data_series(&inputs[0])?;
                let series = numeric_series(artifact, NullPolicy::Propagate)?;
                let metadata = series.metadata().clone();
                let values = series
                    .float_values()?
                    .iter()
                    .map(|value| {
                        value
                            .map(|value| apply_unary(self.operation, value).map(Value::Decimal))
                            .unwrap_or(Ok(Value::Null))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(vec![RuntimeValue::Artifact(build_series(
                    DataSeriesElementType::Float64,
                    &metadata,
                    artifact.kind(),
                    values,
                )?)])
            }
            RuntimeValue::Stream(_) => Err(KernelError::new(
                "unary math requires a scalar or fully materialized DataSeries",
            )),
        }
    }
}

#[derive(Clone)]
enum NumericOperand {
    ScalarInt(i64),
    ScalarFloat(f64),
    SeriesInt(Box<[Option<i64>]>),
    SeriesFloat(Box<[Option<f64>]>),
}

impl NumericOperand {
    fn is_float(&self) -> bool {
        match self {
            Self::ScalarFloat(_) | Self::SeriesFloat(_) => true,
            Self::ScalarInt(_) | Self::SeriesInt(_) => false,
        }
    }

    fn int_at(&self, index: usize) -> Result<Option<i64>, KernelError> {
        match self {
            Self::ScalarInt(value) => Ok(Some(*value)),
            Self::SeriesInt(values) => Ok(values[index]),
            Self::ScalarFloat(_) | Self::SeriesFloat(_) => {
                Err(KernelError::new("internal numeric promotion mismatch"))
            }
        }
    }

    fn float_at(&self, index: usize) -> Result<Option<f64>, KernelError> {
        match self {
            Self::ScalarInt(value) => checked_int64_to_f64(*value).map(Some),
            Self::ScalarFloat(value) => Ok(Some(*value)),
            Self::SeriesInt(values) => values[index].map(checked_int64_to_f64).transpose(),
            Self::SeriesFloat(values) => Ok(values[index]),
        }
    }
}

trait NumericSeriesMetadata {
    fn metadata(&self) -> &DataSeriesMetadata;
    fn float_values(&self) -> Result<Box<[Option<f64>]>, KernelError>;
}

impl NumericSeriesMetadata for NumericSeriesView {
    fn metadata(&self) -> &DataSeriesMetadata {
        match self {
            Self::Int64(series) => series.metadata(),
            Self::Float64(series) => series.metadata(),
        }
    }

    fn float_values(&self) -> Result<Box<[Option<f64>]>, KernelError> {
        match self {
            Self::Int64(series) => series
                .values()
                .iter()
                .map(|value| value.map(checked_int64_to_f64).transpose())
                .collect::<Result<Vec<_>, _>>()
                .map(Vec::into_boxed_slice),
            Self::Float64(series) => Ok(series.values().into()),
        }
    }
}

fn read_operands(
    inputs: &[RuntimeValue],
) -> Result<(Vec<NumericOperand>, DataSeriesMetadata, ArtifactKind), KernelError> {
    let first_series = inputs
        .iter()
        .find_map(|input| match input {
            RuntimeValue::Artifact(_) => Some(require_data_series(input)),
            _ => None,
        })
        .transpose()?
        .ok_or_else(|| {
            KernelError::new("series arithmetic requires at least one DataSeries operand")
        })?;
    let metadata = first_series
        .data_series_metadata()
        .expect("required DataSeries has metadata")
        .clone();
    let kind = first_series.kind();
    let operands = inputs
        .iter()
        .map(|input| read_operand(input, metadata.length))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((operands, metadata, kind))
}

fn read_operand(input: &RuntimeValue, length: usize) -> Result<NumericOperand, KernelError> {
    match input {
        RuntimeValue::Scalar(Value::Integer(value)) => Ok(NumericOperand::ScalarInt(*value)),
        RuntimeValue::Scalar(Value::Decimal(value)) => {
            parse_decimal(value).map(NumericOperand::ScalarFloat)
        }
        RuntimeValue::Scalar(_) => Err(KernelError::new(
            "numeric operation expects Int64 or Float64 values",
        )),
        RuntimeValue::Artifact(_) => {
            let artifact = require_data_series(input)?;
            if artifact.data_series_metadata().unwrap().length != length {
                return Err(KernelError::new(
                    "DataSeries operands must have equal lengths",
                ));
            }
            match numeric_series(artifact, NullPolicy::Propagate)? {
                NumericSeriesView::Int64(series) => {
                    Ok(NumericOperand::SeriesInt(series.values().into()))
                }
                NumericSeriesView::Float64(series) => {
                    Ok(NumericOperand::SeriesFloat(series.values().into()))
                }
            }
        }
        RuntimeValue::Stream(_) => Err(KernelError::new(
            "DataSeries arithmetic requires fully materialized inputs",
        )),
    }
}

fn evaluate_row(
    operation: BinaryOperation,
    operands: &[NumericOperand],
    index: usize,
    float_output: bool,
) -> Result<Value, KernelError> {
    if float_output {
        let values = operands
            .iter()
            .map(|operand| operand.float_at(index))
            .collect::<Result<Vec<_>, _>>()?;
        if values.iter().any(Option::is_none) {
            return Ok(Value::Null);
        }
        let mut values = values.into_iter().flatten();
        let first = values
            .next()
            .expect("series math has at least two operands");
        let result = values.try_fold(first, |left, right| apply_float(operation, left, right))?;
        Ok(Value::Decimal(canonical_float(result)?))
    } else {
        let values = operands
            .iter()
            .map(|operand| operand.int_at(index))
            .collect::<Result<Vec<_>, _>>()?;
        if values.iter().any(Option::is_none) {
            return Ok(Value::Null);
        }
        let mut values = values.into_iter().flatten();
        let first = values
            .next()
            .expect("series math has at least two operands");
        values
            .try_fold(first, |left, right| apply_int(operation, left, right))
            .map(Value::Integer)
    }
}

fn apply_int(operation: BinaryOperation, left: i64, right: i64) -> Result<i64, KernelError> {
    let result = match operation {
        BinaryOperation::Add => left.checked_add(right),
        BinaryOperation::Subtract => left.checked_sub(right),
        BinaryOperation::Multiply => left.checked_mul(right),
        BinaryOperation::Divide => unreachable!("division always promotes to Float64"),
    };
    result.ok_or_else(|| KernelError::new("Int64 arithmetic overflow"))
}

fn apply_float(operation: BinaryOperation, left: f64, right: f64) -> Result<f64, KernelError> {
    if matches!(operation, BinaryOperation::Divide) && right == 0.0 {
        return Err(KernelError::new("Float64 division by zero"));
    }
    let result = match operation {
        BinaryOperation::Add => left + right,
        BinaryOperation::Subtract => left - right,
        BinaryOperation::Multiply => left * right,
        BinaryOperation::Divide => left / right,
    };
    result
        .is_finite()
        .then_some(result)
        .ok_or_else(|| KernelError::new("Float64 arithmetic produced a non-finite result"))
}

fn build_series(
    element_type: DataSeriesElementType,
    metadata: &DataSeriesMetadata,
    kind: ArtifactKind,
    values: Vec<Value>,
) -> Result<Artifact, KernelError> {
    let mut builder = DataSeriesBuilder::new(element_type).values(values);
    if let Some(name) = &metadata.name {
        builder = builder.name(name.clone());
    }
    if let Some(format) = &metadata.format {
        builder = builder.format(format.clone());
    }
    builder
        .build(kind)
        .map_err(|error| KernelError::new(error.to_string()))
}

fn scalar_float(value: &Value) -> Result<f64, KernelError> {
    match value {
        Value::Integer(value) => checked_int64_to_f64(*value),
        Value::Decimal(value) => parse_decimal(value),
        _ => Err(KernelError::new(
            "numeric operation expects Int64 or Float64 values",
        )),
    }
}

fn parse_decimal(value: &CanonicalDecimal) -> Result<f64, KernelError> {
    let value = value
        .as_str()
        .parse::<f64>()
        .map_err(|_| KernelError::new("invalid Float64 decimal"))?;
    value
        .is_finite()
        .then_some(value)
        .ok_or_else(|| KernelError::new("numeric value must be finite"))
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
