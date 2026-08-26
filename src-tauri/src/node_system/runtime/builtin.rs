use super::{
    ArtifactKind, DataSeriesBuilder, DataSeriesElementType, Kernel, KernelContext, KernelError,
    KernelRegistry, RuntimeValue,
};
use crate::node_system::plan::{KernelHandle, ResourceId};
use crate::node_system::protocol::{CanonicalDecimal, Value};
use std::cmp::Ordering;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinConstantParameters {
    value: Value,
}

impl BuiltinConstantParameters {
    pub fn new(value: Value) -> Self {
        Self { value }
    }

    pub fn value(&self) -> &Value {
        &self.value
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinVariableParameters {
    resource: ResourceId,
}

impl BuiltinVariableParameters {
    pub fn new(resource: ResourceId) -> Self {
        Self { resource }
    }
}

pub fn build_builtin_kernel_registry() -> KernelRegistry {
    let mut registry = KernelRegistry::new();
    for (handle, kind) in [
        ("yssbi.constant.bool", ValueKind::Bool),
        ("yssbi.constant.string", ValueKind::String),
        ("yssbi.constant.int64", ValueKind::Int64),
        ("yssbi.constant.float64", ValueKind::Float64),
    ] {
        register(&mut registry, handle, ConstantKernel { kind });
    }
    for (handle, kind, operation) in [
        (
            "yssbi.numeric.add.int64",
            NumericKind::Int64,
            NumericOperation::Add,
        ),
        (
            "yssbi.numeric.subtract.int64",
            NumericKind::Int64,
            NumericOperation::Subtract,
        ),
        (
            "yssbi.numeric.multiply.int64",
            NumericKind::Int64,
            NumericOperation::Multiply,
        ),
        (
            "yssbi.numeric.divide.int64",
            NumericKind::Int64,
            NumericOperation::Divide,
        ),
        (
            "yssbi.numeric.add.float64",
            NumericKind::Float64,
            NumericOperation::Add,
        ),
        (
            "yssbi.numeric.subtract.float64",
            NumericKind::Float64,
            NumericOperation::Subtract,
        ),
        (
            "yssbi.numeric.multiply.float64",
            NumericKind::Float64,
            NumericOperation::Multiply,
        ),
        (
            "yssbi.numeric.divide.float64",
            NumericKind::Float64,
            NumericOperation::Divide,
        ),
    ] {
        register(&mut registry, handle, NumericKernel { kind, operation });
    }
    for (handle, operation) in [
        ("yssbi.compare.equal", CompareOperation::Equal),
        ("yssbi.compare.not_equal", CompareOperation::NotEqual),
        ("yssbi.compare.less", CompareOperation::Less),
        ("yssbi.compare.less_equal", CompareOperation::LessEqual),
        ("yssbi.compare.greater", CompareOperation::Greater),
        (
            "yssbi.compare.greater_equal",
            CompareOperation::GreaterEqual,
        ),
    ] {
        register(&mut registry, handle, CompareKernel { operation });
    }
    register(
        &mut registry,
        "yssbi.logic.and",
        LogicKernel {
            operation: LogicOperation::And,
        },
    );
    register(
        &mut registry,
        "yssbi.logic.or",
        LogicKernel {
            operation: LogicOperation::Or,
        },
    );
    register(&mut registry, "yssbi.logic.not", NotKernel);
    register(
        &mut registry,
        "yssbi.project.variable.get",
        VariableKernel { write: false },
    );
    register(
        &mut registry,
        "yssbi.project.variable.set",
        VariableKernel { write: true },
    );
    for fragment in super::kernels::build_kernel_fragments() {
        fragment
            .install(&mut registry)
            .expect("built-in kernel handles have one owner");
    }
    registry
}

struct VariableKernel {
    write: bool,
}

impl Kernel for VariableKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        let parameters = context.parameters::<BuiltinVariableParameters>()?;
        let lease = context
            .resources
            .get(&parameters.resource)
            .and_then(|lease| lease.as_any().downcast_ref::<super::ProjectResourceLease>())
            .and_then(super::ProjectResourceLease::variable_access)
            .ok_or_else(|| KernelError::new("bound project variable resource is unavailable"))?;
        if self.write {
            expect_arity(inputs, 1)?;
            let variable = lease.read().map_err(KernelError::new)?;
            let value = runtime_to_variable_value(&variable, &inputs[0])?;
            lease.write(value).map_err(KernelError::new)?;
            Ok(Vec::new())
        } else {
            expect_arity(inputs, 0)?;
            let variable = lease.read().map_err(KernelError::new)?;
            Ok(vec![variable_runtime_value(&variable)?])
        }
    }
}

fn variable_runtime_value(
    variable: &crate::variable::VariableInstance,
) -> Result<RuntimeValue, KernelError> {
    let crate::data_contract::DataType::DataSeries(element_type) = &variable.data_type else {
        return Ok(RuntimeValue::Scalar(data_value_to_protocol(
            &variable.data_value,
        )?));
    };
    let snapshot = variable
        .tabular
        .as_ref()
        .ok_or_else(|| KernelError::new("DataSeries variable has no persisted tabular snapshot"))?;
    let (name, values) = snapshot.columns.iter().next().ok_or_else(|| {
        KernelError::new("DataSeries variable snapshot must contain exactly one column")
    })?;
    if snapshot.columns.len() != 1 {
        return Err(KernelError::new(
            "DataSeries variable snapshot must contain exactly one column",
        ));
    }
    let element_type = data_series_element_type(element_type)?;
    let values = values
        .iter()
        .enumerate()
        .map(|(index, value)| json_series_value(element_type, value, index))
        .collect::<Result<Vec<_>, _>>()?;
    let artifact = DataSeriesBuilder::new(element_type)
        .values(values)
        .name(name.as_str())
        .build(ArtifactKind::Collected)
        .map_err(|error| KernelError::new(error.to_string()))?;
    Ok(RuntimeValue::Artifact(artifact))
}

fn data_series_element_type(
    data_type: &crate::data_contract::DataType,
) -> Result<DataSeriesElementType, KernelError> {
    use crate::data_contract::DataType;
    match data_type {
        DataType::Int64 => Ok(DataSeriesElementType::Int64),
        DataType::Float64 => Ok(DataSeriesElementType::Float64),
        DataType::String => Ok(DataSeriesElementType::String),
        DataType::Boolean => Ok(DataSeriesElementType::Boolean),
        DataType::Date => Ok(DataSeriesElementType::Date),
        DataType::Datetime => Ok(DataSeriesElementType::Datetime),
        DataType::Categorical => Ok(DataSeriesElementType::Categorical),
        unsupported => Err(KernelError::new(format!(
            "DataSeries variable element type {unsupported:?} is not executable"
        ))),
    }
}

fn json_series_value(
    element_type: DataSeriesElementType,
    value: &serde_json::Value,
    index: usize,
) -> Result<Value, KernelError> {
    if value.is_null() {
        return Ok(Value::Null);
    }
    let converted = match element_type {
        DataSeriesElementType::Int64 => value.as_i64().map(Value::Integer),
        DataSeriesElementType::Float64 => value.as_f64().and_then(|value| {
            CanonicalDecimal::new(value.to_string())
                .ok()
                .map(Value::Decimal)
        }),
        DataSeriesElementType::Boolean => value.as_bool().map(Value::Bool),
        DataSeriesElementType::String
        | DataSeriesElementType::Date
        | DataSeriesElementType::Datetime
        | DataSeriesElementType::Categorical => value
            .as_str()
            .map(|value| Value::String(value.to_owned().into_boxed_str())),
    };
    converted.ok_or_else(|| {
        KernelError::new(format!(
            "DataSeries variable element {index} is incompatible with {element_type}"
        ))
    })
}

fn runtime_to_variable_value(
    variable: &crate::variable::VariableInstance,
    value: &RuntimeValue,
) -> Result<crate::data_contract::DataValue, KernelError> {
    let crate::data_contract::DataType::DataSeries(declared_element) = &variable.data_type else {
        return protocol_to_data_value(runtime_scalar(std::slice::from_ref(value), 0)?);
    };
    let artifact = super::require_data_series(value)?;
    let metadata = artifact
        .data_series_metadata()
        .ok_or_else(|| KernelError::new("DataSeries Artifact metadata is unavailable"))?;
    let declared_runtime_type = data_series_element_type(declared_element)?;
    if metadata.element_type != declared_runtime_type {
        return Err(KernelError::new(format!(
            "DataSeries variable expects {declared_runtime_type}, received {}",
            metadata.element_type
        )));
    }
    let column_name = metadata.name.as_deref().unwrap_or(variable.name.as_str());
    let values = artifact
        .cursor()
        .map_err(|error| KernelError::new(error.to_string()))?
        .map(|value| {
            value
                .map_err(|error| KernelError::new(error.to_string()))
                .and_then(protocol_series_value_to_json)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let json = serde_json::to_string(&serde_json::json!({column_name: values}))
        .map_err(|error| KernelError::new(error.to_string()))?;
    Ok(crate::data_contract::DataValue::DataSeries(
        crate::data_contract::DataSeriesValue::with_element_type(
            json,
            declared_element.as_ref().clone(),
        ),
    ))
}

fn protocol_series_value_to_json(value: Value) -> Result<serde_json::Value, KernelError> {
    match value {
        Value::Null => Ok(serde_json::Value::Null),
        Value::Bool(value) => Ok(serde_json::Value::Bool(value)),
        Value::Integer(value) => Ok(serde_json::json!(value)),
        Value::Decimal(value) => serde_json::from_str(value.as_str())
            .map_err(|_| KernelError::new("DataSeries decimal is not valid JSON numeric storage")),
        Value::String(value) => Ok(serde_json::Value::String(value.into())),
        unsupported => Err(KernelError::new(format!(
            "DataSeries Artifact contains unsupported {unsupported:?} storage"
        ))),
    }
}

fn data_value_to_protocol(value: &crate::data_contract::DataValue) -> Result<Value, KernelError> {
    use crate::data_contract::DataValue;
    Ok(match value {
        DataValue::Boolean(value) => Value::Bool(*value),
        DataValue::Int64(value) => Value::Integer(*value),
        DataValue::Float64(value) => Value::Decimal(
            CanonicalDecimal::new(value.to_string())
                .map_err(|error| KernelError::new(error.to_string()))?,
        ),
        DataValue::String(value) => Value::String(value.as_str().into()),
        DataValue::Array(values) => Value::List(
            values
                .iter()
                .map(data_value_to_protocol)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        DataValue::Object(values) => Value::Object(
            values
                .iter()
                .map(|(key, value)| Ok((key.as_str().into(), data_value_to_protocol(value)?)))
                .collect::<Result<_, KernelError>>()?,
        ),
        unsupported => {
            return Err(KernelError::new(format!(
                "variable value {unsupported:?} is not a scalar runtime value"
            )));
        }
    })
}

fn protocol_to_data_value(value: &Value) -> Result<crate::data_contract::DataValue, KernelError> {
    use crate::data_contract::DataValue;
    Ok(match value {
        Value::Bool(value) => DataValue::Boolean(*value),
        Value::Integer(value) => DataValue::Int64(*value),
        Value::Unsigned(value) => DataValue::Int64(
            i64::try_from(*value).map_err(|_| KernelError::new("unsigned value exceeds int64"))?,
        ),
        Value::Decimal(value) => DataValue::Float64(
            value
                .as_str()
                .parse()
                .map_err(|_| KernelError::new("decimal value is not representable as float64"))?,
        ),
        Value::String(value) => DataValue::String(value.to_string()),
        Value::List(values) => DataValue::Array(
            values
                .iter()
                .map(protocol_to_data_value)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Value::Bytes(values) => DataValue::Array(
            values
                .iter()
                .map(|value| DataValue::Int64(i64::from(*value)))
                .collect(),
        ),
        Value::Object(values) => DataValue::Object(
            values
                .iter()
                .map(|(key, value)| Ok((key.to_string(), protocol_to_data_value(value)?)))
                .collect::<Result<_, KernelError>>()?,
        ),
        Value::Null => {
            return Err(KernelError::new(
                "null cannot be assigned to a project variable",
            ));
        }
    })
}

fn runtime_scalar(inputs: &[RuntimeValue], index: usize) -> Result<&Value, KernelError> {
    match inputs.get(index) {
        Some(RuntimeValue::Scalar(value)) => Ok(value),
        Some(_) => Err(KernelError::new(
            "project variable assignment requires a scalar value",
        )),
        None => Err(KernelError::new(
            "project variable assignment is missing its value",
        )),
    }
}

fn register(registry: &mut KernelRegistry, handle: &'static str, kernel: impl Kernel + 'static) {
    let handle = KernelHandle::new(handle).expect("built-in kernel handles are valid");
    registry
        .register(handle, kernel)
        .expect("built-in kernel handles are unique");
}

#[derive(Clone, Copy)]
enum ValueKind {
    Bool,
    String,
    Int64,
    Float64,
}

impl ValueKind {
    fn name(self) -> &'static str {
        match self {
            Self::Bool => "bool",
            Self::String => "string",
            Self::Int64 => "int64",
            Self::Float64 => "float64",
        }
    }

    fn accepts(self, value: &Value) -> bool {
        matches!(
            (self, value),
            (Self::Bool, Value::Bool(_))
                | (Self::String, Value::String(_))
                | (Self::Int64, Value::Integer(_))
                | (Self::Float64, Value::Decimal(_))
        )
    }
}

struct ConstantKernel {
    kind: ValueKind,
}

impl Kernel for ConstantKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        expect_arity(inputs, 0)?;
        let parameters = context.parameters::<BuiltinConstantParameters>()?;
        if !self.kind.accepts(parameters.value()) {
            return Err(KernelError::new(format!(
                "constant parameter has type {}; expected {}",
                value_kind(parameters.value()),
                self.kind.name()
            )));
        }
        Ok(vec![parameters.value().clone().into()])
    }
}

#[derive(Clone, Copy)]
enum NumericKind {
    Int64,
    Float64,
}

#[derive(Clone, Copy)]
enum NumericOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
}

struct NumericKernel {
    kind: NumericKind,
    operation: NumericOperation,
}

impl Kernel for NumericKernel {
    fn execute(
        &self,
        _context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        expect_arity(inputs, 2)?;
        let output = match self.kind {
            NumericKind::Int64 => {
                let left = int64_input(inputs, 0)?;
                let right = int64_input(inputs, 1)?;
                Value::Integer(integer_operation(self.operation, left, right)?)
            }
            NumericKind::Float64 => {
                let left = float64_input(inputs, 0)?;
                let right = float64_input(inputs, 1)?;
                Value::Decimal(float_operation(self.operation, left, right)?)
            }
        };
        Ok(vec![output.into()])
    }
}

fn integer_operation(
    operation: NumericOperation,
    left: i64,
    right: i64,
) -> Result<i64, KernelError> {
    if matches!(operation, NumericOperation::Divide) && right == 0 {
        return Err(KernelError::new("int64 division by zero"));
    }
    let result = match operation {
        NumericOperation::Add => left.checked_add(right),
        NumericOperation::Subtract => left.checked_sub(right),
        NumericOperation::Multiply => left.checked_mul(right),
        NumericOperation::Divide => left.checked_div(right),
    };
    result.ok_or_else(|| KernelError::new("int64 arithmetic overflow"))
}

fn float_operation(
    operation: NumericOperation,
    left: f64,
    right: f64,
) -> Result<CanonicalDecimal, KernelError> {
    if matches!(operation, NumericOperation::Divide) && right == 0.0 {
        return Err(KernelError::new("float64 division by zero"));
    }
    let result = match operation {
        NumericOperation::Add => left + right,
        NumericOperation::Subtract => left - right,
        NumericOperation::Multiply => left * right,
        NumericOperation::Divide => left / right,
    };
    canonical_float(result)
}

#[derive(Clone, Copy)]
enum CompareOperation {
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

struct CompareKernel {
    operation: CompareOperation,
}

impl Kernel for CompareKernel {
    fn execute(
        &self,
        _context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        expect_arity(inputs, 2)?;
        let result = match self.operation {
            CompareOperation::Equal | CompareOperation::NotEqual => {
                let RuntimeValue::Scalar(left) = &inputs[0] else {
                    return Err(KernelError::new("comparison input 0 must be a scalar"));
                };
                let RuntimeValue::Scalar(right) = &inputs[1] else {
                    return Err(KernelError::new("comparison input 1 must be a scalar"));
                };
                let equal = left == right;
                if matches!(self.operation, CompareOperation::Equal) {
                    equal
                } else {
                    !equal
                }
            }
            operation => {
                let left = float64_input(inputs, 0)?;
                let right = float64_input(inputs, 1)?;
                let ordering = left
                    .partial_cmp(&right)
                    .ok_or_else(|| KernelError::new("float64 values are not comparable"))?;
                match operation {
                    CompareOperation::Less => ordering == Ordering::Less,
                    CompareOperation::LessEqual => ordering != Ordering::Greater,
                    CompareOperation::Greater => ordering == Ordering::Greater,
                    CompareOperation::GreaterEqual => ordering != Ordering::Less,
                    CompareOperation::Equal | CompareOperation::NotEqual => unreachable!(),
                }
            }
        };
        Ok(vec![Value::Bool(result).into()])
    }
}

#[derive(Clone, Copy)]
enum LogicOperation {
    And,
    Or,
}

struct LogicKernel {
    operation: LogicOperation,
}

impl Kernel for LogicKernel {
    fn execute(
        &self,
        _context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        expect_arity(inputs, 2)?;
        let left = bool_input(inputs, 0)?;
        let right = bool_input(inputs, 1)?;
        let result = match self.operation {
            LogicOperation::And => left && right,
            LogicOperation::Or => left || right,
        };
        Ok(vec![Value::Bool(result).into()])
    }
}

struct NotKernel;

impl Kernel for NotKernel {
    fn execute(
        &self,
        _context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        expect_arity(inputs, 1)?;
        Ok(vec![Value::Bool(!bool_input(inputs, 0)?).into()])
    }
}

fn expect_arity(inputs: &[RuntimeValue], expected: usize) -> Result<(), KernelError> {
    if inputs.len() == expected {
        Ok(())
    } else {
        Err(KernelError::new(format!(
            "kernel received {} inputs; expected {expected}",
            inputs.len()
        )))
    }
}

fn scalar_input(inputs: &[RuntimeValue], index: usize) -> Result<&Value, KernelError> {
    match &inputs[index] {
        RuntimeValue::Scalar(value) => Ok(value),
        RuntimeValue::Artifact(_) => Err(KernelError::new(format!(
            "input {index} is an artifact; expected a scalar"
        ))),
        RuntimeValue::Stream(_) => Err(KernelError::new(format!(
            "input {index} is a stream; expected a scalar"
        ))),
    }
}

fn int64_input(inputs: &[RuntimeValue], index: usize) -> Result<i64, KernelError> {
    match scalar_input(inputs, index)? {
        Value::Integer(value) => Ok(*value),
        value => Err(type_error(index, "int64", value)),
    }
}

fn float64_input(inputs: &[RuntimeValue], index: usize) -> Result<f64, KernelError> {
    let Value::Decimal(value) = scalar_input(inputs, index)? else {
        return Err(type_error(index, "float64", scalar_input(inputs, index)?));
    };
    let parsed = value
        .as_str()
        .parse::<f64>()
        .map_err(|_| KernelError::new(format!("input {index} is not a valid float64")))?;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        Err(KernelError::new(format!(
            "input {index} is outside the finite float64 range"
        )))
    }
}

fn bool_input(inputs: &[RuntimeValue], index: usize) -> Result<bool, KernelError> {
    match scalar_input(inputs, index)? {
        Value::Bool(value) => Ok(*value),
        value => Err(type_error(index, "bool", value)),
    }
}

fn type_error(index: usize, expected: &str, actual: &Value) -> KernelError {
    KernelError::new(format!(
        "input {index} has type {}; expected {expected}",
        value_kind(actual)
    ))
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Integer(_) => "int64",
        Value::Unsigned(_) => "uint64",
        Value::Decimal(_) => "float64",
        Value::String(_) => "string",
        Value::Bytes(_) => "bytes",
        Value::List(_) => "list",
        Value::Object(_) => "object",
    }
}

fn canonical_float(value: f64) -> Result<CanonicalDecimal, KernelError> {
    if !value.is_finite() {
        return Err(KernelError::new(
            "float64 arithmetic produced a non-finite result",
        ));
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
        .map_err(|_| KernelError::new("float64 result has an invalid exponent"))?;
    let (sign, unsigned) = mantissa
        .strip_prefix('-')
        .map_or(("", mantissa), |unsigned| ("-", unsigned));
    let integer_digits = unsigned.find('.').unwrap_or(unsigned.len()) as i32;
    let digits = unsigned.replace('.', "");
    let decimal_position = integer_digits + exponent;
    let expanded = if decimal_position <= 0 {
        format!(
            "{sign}0.{}{}",
            "0".repeat((-decimal_position) as usize),
            digits
        )
    } else if decimal_position as usize >= digits.len() {
        format!(
            "{sign}{digits}{}",
            "0".repeat(decimal_position as usize - digits.len())
        )
    } else {
        let position = decimal_position as usize;
        format!("{sign}{}.{}", &digits[..position], &digits[position..])
    };
    Ok(expanded)
}
