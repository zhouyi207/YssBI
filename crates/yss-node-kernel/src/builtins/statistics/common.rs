pub(super) use super::super::series::{
    independent_numeric_columns as independent_columns, numeric_columns as columns,
};
pub(super) use super::linear::{check_fit_workspace, group, numeric_list};
use crate::{KernelError, KernelInvocation, RuntimeValue};
use yss_data_contract::TabularScalar;

mod finite;
pub(super) use finite::validate as validate_finite;
mod models;
pub(super) use models::{model_dimensions, regression_outputs, with_model};
mod inputs;
mod tables;
pub(super) use inputs::{
    Category, categories, category_code, materialize, numeric, ordinal_domain, ordinal_index,
};
pub(super) use tables::{matrix_table, numeric_table, scalar_table};

// Charge the temporary JSON representation and the resulting runtime containers together.
pub(in crate::builtins) const STRUCTURED_VALUE_BYTES: usize = 128;
pub(in crate::builtins) const STRUCTURED_VALUE_COPIES: usize = 3;

pub(crate) fn text<'a>(inv: &'a KernelInvocation<'_>, key: &str) -> Result<&'a str, KernelError> {
    match inv.parameter(key) {
        Some(RuntimeValue::Scalar(TabularScalar::String(v))) => Ok(v),
        _ => Err(KernelError::InvalidParameter),
    }
}
pub(in crate::builtins) fn boolean(
    inv: &KernelInvocation<'_>,
    key: &str,
) -> Result<bool, KernelError> {
    match inv.parameter(key) {
        Some(RuntimeValue::Scalar(TabularScalar::Bool(v))) => Ok(*v),
        _ => Err(KernelError::InvalidParameter),
    }
}
pub(crate) fn integer(inv: &KernelInvocation<'_>, key: &str) -> Result<usize, KernelError> {
    match inv.parameter(key) {
        Some(RuntimeValue::Scalar(TabularScalar::Integer(v))) => {
            usize::try_from(*v).map_err(|_| KernelError::InvalidParameter)
        }
        _ => Err(KernelError::InvalidParameter),
    }
}
pub(in crate::builtins) fn number(
    inv: &KernelInvocation<'_>,
    key: &str,
) -> Result<f64, KernelError> {
    super::super::numeric_input(inv.parameter(key)).map_err(|_| KernelError::InvalidParameter)
}
pub(super) fn sci(error: yss_sci_contract::SciError) -> KernelError {
    use yss_sci_contract::{SciError, execution::ScientificInputViolation};
    match error {
        SciError::InvalidInput {
            violation: ScientificInputViolation::ShapeMismatch,
            ..
        } => KernelError::ShapeMismatch,
        SciError::InvalidInput {
            violation: ScientificInputViolation::ParameterOutOfRange,
            ..
        } => KernelError::InvalidParameter,
        SciError::InvalidInput { .. } => KernelError::InvalidNumericInput,
        SciError::ComputationFailed { .. } => KernelError::ScientificFailure,
    }
}
pub(super) fn metadata(n: usize) -> yss_sci_contract::StatisticalObservationMetadata {
    yss_sci_contract::StatisticalObservationMetadata {
        original_observation_count: n,
        used_observation_count: n,
        dropped_null_count: 0,
        dropped_nan_count: 0,
        missing_value_policy: yss_sci_contract::MissingValuePolicy::Reject,
    }
}
pub(in crate::builtins) fn value(
    data: impl serde::Serialize,
    inv: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    inv.check_control()?;
    finite::validate(&data)?;
    let data = serde_json::to_value(data).map_err(|_| KernelError::ScientificFailure)?;
    fn charge(v: &serde_json::Value) -> Option<usize> {
        let children = match v {
            serde_json::Value::Array(items) => items
                .iter()
                .try_fold(0usize, |n, v| n.checked_add(charge(v)?))?,
            serde_json::Value::Object(items) => items.iter().try_fold(0usize, |n, (k, v)| {
                n.checked_add(k.len())?.checked_add(charge(v)?)
            })?,
            serde_json::Value::String(s) => s.len(),
            _ => 0,
        };
        children.checked_add(STRUCTURED_VALUE_BYTES)
    }
    inv.control
        .check_bytes(charge(&data).and_then(|n| n.checked_mul(STRUCTURED_VALUE_COPIES)))?;
    RuntimeValue::from_json_checked(data, &mut || inv.check_control())
}
pub(super) fn field<'a>(
    model: &'a RuntimeValue,
    key: &str,
) -> Result<&'a RuntimeValue, KernelError> {
    match model {
        RuntimeValue::Record(fields) => fields.get(key).ok_or(KernelError::InvalidNumericInput),
        _ => Err(KernelError::InvalidNumericInput),
    }
}
pub(super) fn decode_model<T: serde::de::DeserializeOwned>(
    inv: &KernelInvocation<'_>,
) -> Result<T, KernelError> {
    let model = inv.inputs.first().ok_or(KernelError::InvalidNumericInput)?;
    decode_model_value(model, inv)
}
pub(in crate::builtins) fn computation_error(
    error: yss_sci_contract::execution::ScientificComputationError,
) -> KernelError {
    use yss_sci_contract::execution::{
        ScientificComputationError as Error, ScientificInputViolation as Violation,
    };
    match error {
        Error::Cancelled => KernelError::Cancelled,
        Error::DeadlineExceeded => KernelError::DeadlineExceeded,
        Error::InvalidInput {
            violation: Violation::ShapeMismatch,
        } => KernelError::ShapeMismatch,
        Error::InvalidInput {
            violation: Violation::ParameterOutOfRange,
        } => KernelError::InvalidParameter,
        Error::InvalidInput { .. } => KernelError::InvalidNumericInput,
        Error::ComputationFailed => KernelError::ScientificFailure,
    }
}
pub(super) fn decode_model_value<T: serde::de::DeserializeOwned>(
    model: &RuntimeValue,
    inv: &KernelInvocation<'_>,
) -> Result<T, KernelError> {
    if !matches!(model, RuntimeValue::Record(_)) {
        return Err(KernelError::InvalidNumericInput);
    }
    fn bytes(
        value: &RuntimeValue,
        inv: &KernelInvocation<'_>,
        depth: usize,
    ) -> Result<usize, KernelError> {
        inv.check_control()?;
        if depth > 64 {
            return Err(KernelError::InvalidNumericInput);
        }
        let mut size = 128usize;
        match value {
            RuntimeValue::List(values) => {
                for value in values.iter() {
                    size = size
                        .checked_add(bytes(value, inv, depth + 1)?)
                        .ok_or(KernelError::BudgetExceeded)?;
                }
            }
            RuntimeValue::Record(values) => {
                for (key, value) in values.iter() {
                    size = size
                        .checked_add(key.len())
                        .and_then(|n| n.checked_add(128))
                        .ok_or(KernelError::BudgetExceeded)?;
                    size = size
                        .checked_add(bytes(value, inv, depth + 1)?)
                        .ok_or(KernelError::BudgetExceeded)?;
                }
            }
            RuntimeValue::Scalar(TabularScalar::String(value)) => {
                size = size
                    .checked_add(value.len())
                    .ok_or(KernelError::BudgetExceeded)?
            }
            RuntimeValue::Scalar(_) => {}
            _ => return Err(KernelError::InvalidNumericInput),
        }
        Ok(size)
    }
    fn json(value: &RuntimeValue) -> Result<serde_json::Value, KernelError> {
        Ok(match value {
            RuntimeValue::Scalar(value) => {
                serde_json::to_value(value).map_err(|_| KernelError::InvalidNumericInput)?
            }
            RuntimeValue::List(values) => {
                serde_json::Value::Array(values.iter().map(json).collect::<Result<_, _>>()?)
            }
            RuntimeValue::Record(values) => serde_json::Value::Object(
                values
                    .iter()
                    .map(|(key, value)| Ok((key.to_string(), json(value)?)))
                    .collect::<Result<_, KernelError>>()?,
            ),
            _ => return Err(KernelError::InvalidNumericInput),
        })
    }
    inv.control
        .check_bytes(bytes(model, inv, 0)?.checked_mul(3))?;
    let result =
        serde_json::from_value(json(model)?).map_err(|_| KernelError::InvalidNumericInput)?;
    inv.check_control()?;
    Ok(result)
}

/// Preserve a relation column's actual label; constants have no column metadata.
pub(super) fn input_label(value: &RuntimeValue, fallback: String) -> String {
    match value.unannotated() {
        RuntimeValue::Series(s) => s.column().to_owned(),
        _ => fallback,
    }
}
