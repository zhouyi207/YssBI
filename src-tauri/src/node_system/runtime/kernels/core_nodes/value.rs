use super::support::{KernelFragment, expect_arity};
use crate::node_system::protocol::{CanonicalDecimal, Value};
use crate::node_system::runtime::{Kernel, KernelContext, KernelError, RunError, RuntimeValue};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConvertTarget {
    Bool,
    Int64,
    Float64,
    String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConvertParameters {
    pub target: ConvertTarget,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ValueKind {
    Bool,
    Int64,
    Float64,
    String,
    Categorical,
}

const SERIES_CONVERSIONS: &[(&str, ValueKind, ValueKind)] = &[
    (
        "yssbi.data_series.convert.string_to_categorical",
        ValueKind::String,
        ValueKind::Categorical,
    ),
    (
        "yssbi.data_series.convert.string_to_float64",
        ValueKind::String,
        ValueKind::Float64,
    ),
    (
        "yssbi.data_series.convert.string_to_int64",
        ValueKind::String,
        ValueKind::Int64,
    ),
    (
        "yssbi.data_series.convert.int64_to_string",
        ValueKind::Int64,
        ValueKind::String,
    ),
    (
        "yssbi.data_series.convert.float64_to_string",
        ValueKind::Float64,
        ValueKind::String,
    ),
    (
        "yssbi.data_series.convert.int64_to_float64",
        ValueKind::Int64,
        ValueKind::Float64,
    ),
    (
        "yssbi.data_series.convert.float64_to_int64",
        ValueKind::Float64,
        ValueKind::Int64,
    ),
    (
        "yssbi.data_series.convert.int64_to_bool",
        ValueKind::Int64,
        ValueKind::Bool,
    ),
    (
        "yssbi.data_series.convert.float64_to_bool",
        ValueKind::Float64,
        ValueKind::Bool,
    ),
    (
        "yssbi.data_series.convert.categorical_to_string",
        ValueKind::Categorical,
        ValueKind::String,
    ),
    (
        "yssbi.data_series.convert.int64_to_categorical",
        ValueKind::Int64,
        ValueKind::Categorical,
    ),
    (
        "yssbi.data_series.convert.categorical_to_int64",
        ValueKind::Categorical,
        ValueKind::Int64,
    ),
    (
        "yssbi.data_series.convert.float64_to_categorical",
        ValueKind::Float64,
        ValueKind::Categorical,
    ),
    (
        "yssbi.data_series.convert.categorical_to_float64",
        ValueKind::Categorical,
        ValueKind::Float64,
    ),
];

pub(super) fn register(fragment: &mut KernelFragment) {
    fragment.register("yssbi.value.convert", ScalarConvertKernel);
    for &(handle, source, target) in SERIES_CONVERSIONS {
        fragment.register(handle, SeriesConvertKernel { source, target });
    }
}

struct ScalarConvertKernel;

impl Kernel for ScalarConvertKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        expect_arity(inputs, 1)?;
        let RuntimeValue::Scalar(value) = &inputs[0] else {
            return Err(KernelError::new("value conversion expects a scalar input"));
        };
        let parameters = context.parameters::<ConvertParameters>()?;
        let target = match parameters.target {
            ConvertTarget::Bool => ValueKind::Bool,
            ConvertTarget::Int64 => ValueKind::Int64,
            ConvertTarget::Float64 => ValueKind::Float64,
            ConvertTarget::String => ValueKind::String,
        };
        Ok(vec![convert(value, target)?.into()])
    }
}

struct SeriesConvertKernel {
    source: ValueKind,
    target: ValueKind,
}

impl Kernel for SeriesConvertKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        expect_arity(inputs, 1)?;
        let RuntimeValue::Artifact(artifact) = &inputs[0] else {
            return Err(KernelError::new(
                "DataSeries conversion expects a fully materialized artifact",
            ));
        };
        let values = artifact
            .cursor()
            .map_err(|error| KernelError::new(error.to_string()))?
            .enumerate()
            .map(|(index, value)| {
                let value = value?;
                if matches!(value, Value::Null) {
                    return Ok(Value::Null);
                }
                require_kind(&value, self.source)
                    .and_then(|()| convert(&value, self.target))
                    .map_err(|error| {
                        RunError::Stream(format!("DataSeries element {index}: {error}").into())
                    })
            });
        let output = context
            .resource_owner
            .materialize_artifact(artifact.kind(), values)
            .map_err(kernel_error_from_run)?;
        Ok(vec![RuntimeValue::Artifact(output)])
    }
}

fn kernel_error_from_run(error: RunError) -> KernelError {
    match error {
        RunError::Stream(message) => KernelError::new(message),
        error => KernelError::new(error.to_string()),
    }
}

fn require_kind(value: &Value, expected: ValueKind) -> Result<(), KernelError> {
    let accepted = match expected {
        ValueKind::Bool => matches!(value, Value::Bool(_)),
        ValueKind::Int64 => matches!(value, Value::Integer(_)),
        ValueKind::Float64 => matches!(value, Value::Decimal(_)),
        ValueKind::String | ValueKind::Categorical => matches!(value, Value::String(_)),
    };
    if accepted {
        Ok(())
    } else {
        Err(KernelError::new(format!(
            "expected {}, got {}",
            kind_name(expected),
            value_name(value)
        )))
    }
}

fn convert(value: &Value, target: ValueKind) -> Result<Value, KernelError> {
    if matches!(value, Value::Null) {
        return Ok(Value::Null);
    }
    match target {
        ValueKind::Bool => to_bool(value).map(Value::Bool),
        ValueKind::Int64 => to_int64(value).map(Value::Integer),
        ValueKind::Float64 => to_float64(value).map(Value::Decimal),
        ValueKind::String | ValueKind::Categorical => to_string(value).map(Value::String),
    }
}

fn to_bool(value: &Value) -> Result<bool, KernelError> {
    match value {
        Value::Bool(value) => Ok(*value),
        Value::Integer(value) => Ok(*value != 0),
        Value::Decimal(value) => Ok(parse_decimal(value)? != 0.0),
        Value::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "1" | "yes" | "on" => Ok(true),
            "false" | "0" | "no" | "off" | "" => Ok(false),
            _ => Err(KernelError::new(format!(
                "cannot parse '{value}' as Boolean"
            ))),
        },
        _ => Err(unsupported(value, "Boolean")),
    }
}

fn to_int64(value: &Value) -> Result<i64, KernelError> {
    match value {
        Value::Bool(value) => Ok(if *value { 1 } else { 0 }),
        Value::Integer(value) => Ok(*value),
        Value::Decimal(value) => {
            let value = parse_decimal(value)?;
            if value < i64::MIN as f64 || value > i64::MAX as f64 {
                return Err(KernelError::new("Float64 value is outside the Int64 range"));
            }
            Ok(value.trunc() as i64)
        }
        Value::String(value) => value
            .parse::<i64>()
            .map_err(|_| KernelError::new(format!("cannot parse '{value}' as Int64"))),
        _ => Err(unsupported(value, "Int64")),
    }
}

fn to_float64(value: &Value) -> Result<CanonicalDecimal, KernelError> {
    let value = match value {
        Value::Bool(value) => f64::from(u8::from(*value)),
        Value::Integer(value) => *value as f64,
        Value::Decimal(value) => return Ok(value.clone()),
        Value::String(value) => value
            .parse::<f64>()
            .map_err(|_| KernelError::new(format!("cannot parse '{value}' as Float64")))?,
        _ => return Err(unsupported(value, "Float64")),
    };
    canonical_float(value)
}

fn to_string(value: &Value) -> Result<Box<str>, KernelError> {
    match value {
        Value::Bool(value) => Ok(value.to_string().into()),
        Value::Integer(value) => Ok(value.to_string().into()),
        Value::Unsigned(value) => Ok(value.to_string().into()),
        Value::Decimal(value) => Ok(value.as_str().into()),
        Value::String(value) => Ok(value.clone()),
        _ => Err(unsupported(value, "String")),
    }
}

fn parse_decimal(value: &CanonicalDecimal) -> Result<f64, KernelError> {
    let parsed = value
        .as_str()
        .parse::<f64>()
        .map_err(|_| KernelError::new("invalid Float64 decimal"))?;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        Err(KernelError::new("Float64 value must be finite"))
    }
}

pub(crate) fn canonical_float(value: f64) -> Result<CanonicalDecimal, KernelError> {
    if !value.is_finite() {
        return Err(KernelError::new("Float64 result must be finite"));
    }
    if value == 0.0 {
        return CanonicalDecimal::new("0").map_err(|error| KernelError::new(error.to_string()));
    }
    let displayed = value.to_string();
    let canonical = expand_exponent(&displayed)?;
    CanonicalDecimal::new(canonical).map_err(|error| KernelError::new(error.to_string()))
}

fn expand_exponent(value: &str) -> Result<String, KernelError> {
    let Some((mantissa, exponent)) = value.split_once(['e', 'E']) else {
        return Ok(value.to_owned());
    };
    let exponent = exponent
        .parse::<i32>()
        .map_err(|_| KernelError::new("Float64 result has an invalid exponent"))?;
    let (sign, unsigned) = mantissa
        .strip_prefix('-')
        .map_or(("", mantissa), |unsigned| ("-", unsigned));
    let integer_digits = unsigned.find('.').unwrap_or(unsigned.len()) as i32;
    let digits = unsigned.replace('.', "");
    let decimal_position = integer_digits + exponent;
    if decimal_position <= 0 {
        Ok(format!(
            "{sign}0.{}{}",
            "0".repeat((-decimal_position) as usize),
            digits
        ))
    } else if decimal_position as usize >= digits.len() {
        Ok(format!(
            "{sign}{digits}{}",
            "0".repeat(decimal_position as usize - digits.len())
        ))
    } else {
        let position = decimal_position as usize;
        Ok(format!(
            "{sign}{}.{}",
            &digits[..position],
            &digits[position..]
        ))
    }
}

fn unsupported(value: &Value, target: &str) -> KernelError {
    KernelError::new(format!("cannot convert {} to {target}", value_name(value)))
}

fn kind_name(kind: ValueKind) -> &'static str {
    match kind {
        ValueKind::Bool => "Boolean",
        ValueKind::Int64 => "Int64",
        ValueKind::Float64 => "Float64",
        ValueKind::String => "String",
        ValueKind::Categorical => "Categorical",
    }
}

fn value_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "Null",
        Value::Bool(_) => "Boolean",
        Value::Integer(_) => "Int64",
        Value::Unsigned(_) => "UInt64",
        Value::Decimal(_) => "Float64",
        Value::String(_) => "String",
        Value::Bytes(_) => "Bytes",
        Value::List(_) => "List",
        Value::Object(_) => "Object",
    }
}
