use std::collections::BTreeMap;

use yss_data_contract::TabularScalar;
use yss_node_kernel::RuntimeValue;

pub(super) type Record = BTreeMap<Box<str>, RuntimeValue>;

/// Shape mismatch while reading a bounded result projection; each query maps it to its own error.
#[derive(Debug)]
pub(super) struct Unrepresentable;

impl From<Unrepresentable> for super::ResultQueryApplicationError {
    fn from(_: Unrepresentable) -> Self {
        Self::UnrepresentableValue
    }
}

impl From<Unrepresentable> for super::report::ReportQueryError {
    fn from(_: Unrepresentable) -> Self {
        Self::UnrepresentableValue
    }
}

pub(super) fn record(value: &RuntimeValue) -> Result<&Record, Unrepresentable> {
    match value.unannotated() {
        RuntimeValue::Record(fields) => Ok(fields),
        _ => Err(Unrepresentable),
    }
}

pub(super) fn list(value: &RuntimeValue) -> Result<&[RuntimeValue], Unrepresentable> {
    match value.unannotated() {
        RuntimeValue::List(values) => Ok(values),
        _ => Err(Unrepresentable),
    }
}

pub(super) fn field<'a>(
    fields: &'a Record,
    key: &str,
) -> Result<&'a RuntimeValue, Unrepresentable> {
    fields.get(key).ok_or(Unrepresentable)
}

pub(super) fn text(value: &RuntimeValue) -> Result<&str, Unrepresentable> {
    match value.unannotated() {
        RuntimeValue::Scalar(TabularScalar::String(text)) => Ok(text),
        _ => Err(Unrepresentable),
    }
}

/// Integers are accepted only where `f64` represents them exactly.
pub(super) fn number(value: &RuntimeValue) -> Result<f64, Unrepresentable> {
    const EXACT: u64 = 1 << 53;
    match value.unannotated() {
        RuntimeValue::Scalar(TabularScalar::Float64(value)) => Ok(value.as_f64()),
        RuntimeValue::Scalar(TabularScalar::Integer(value)) if value.unsigned_abs() <= EXACT => {
            Ok(*value as f64)
        }
        RuntimeValue::Scalar(TabularScalar::Unsigned(value)) if *value <= EXACT => {
            Ok(*value as f64)
        }
        _ => Err(Unrepresentable),
    }
}

pub(super) fn count<T: TryFrom<u64>>(value: &RuntimeValue) -> Result<T, Unrepresentable> {
    let value = match value.unannotated() {
        RuntimeValue::Scalar(TabularScalar::Unsigned(value)) => *value,
        RuntimeValue::Scalar(TabularScalar::Integer(value)) => {
            u64::try_from(*value).map_err(|_| Unrepresentable)?
        }
        _ => return Err(Unrepresentable),
    };
    T::try_from(value).map_err(|_| Unrepresentable)
}
