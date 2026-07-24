use super::TypeExpr;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A format-neutral value tree. Object ordering and decimal spelling are stable
/// so serialized protocol defaults do not depend on hash order or host floats.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
    Null,
    Bool(bool),
    Integer(i64),
    Unsigned(u64),
    Decimal(CanonicalDecimal),
    String(Box<str>),
    Bytes(Vec<u8>),
    List(Vec<Value>),
    Object(BTreeMap<Box<str>, Value>),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct CanonicalDecimal(Box<str>);

impl CanonicalDecimal {
    pub fn new(value: impl Into<Box<str>>) -> Result<Self, InvalidDecimal> {
        let value = value.into();
        if is_canonical_decimal(&value) {
            Ok(Self(value))
        } else {
            Err(InvalidDecimal(value.into_string()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for CanonicalDecimal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Box::<str>::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidDecimal(String);

impl std::fmt::Display for InvalidDecimal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "'{}' is not a canonical decimal", self.0)
    }
}

impl std::error::Error for InvalidDecimal {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypedValue {
    pub value_type: TypeExpr,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParameterValue {
    pub value_type: TypeExpr,
    pub value: Value,
}

impl From<TypedValue> for ParameterValue {
    fn from(value: TypedValue) -> Self {
        Self {
            value_type: value.value_type,
            value: value.value,
        }
    }
}

fn is_canonical_decimal(value: &str) -> bool {
    if value.is_empty() || value.starts_with('+') || value.contains(['e', 'E']) {
        return false;
    }
    let unsigned = value.strip_prefix('-').unwrap_or(value);
    if unsigned.is_empty() || value == "-0" {
        return false;
    }
    let (integer, fraction) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let integer_valid = integer == "0"
        || (!integer.starts_with('0') && integer.bytes().all(|byte| byte.is_ascii_digit()));
    let fraction_valid = fraction.is_empty()
        || (fraction.bytes().all(|byte| byte.is_ascii_digit()) && !fraction.ends_with('0'));
    integer_valid && fraction_valid && !value.ends_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn objects_serialize_in_key_order() {
        let value = Value::Object(BTreeMap::from([
            (Box::from("z"), Value::Integer(1)),
            (Box::from("a"), Value::Integer(2)),
        ]));
        let json = serde_json::to_string(&value).unwrap();
        assert!(json.find("a").unwrap() < json.find("z").unwrap());
    }

    #[test]
    fn decimals_require_canonical_non_exponent_spelling() {
        assert!(CanonicalDecimal::new("-12.34").is_ok());
        assert!(CanonicalDecimal::new("01").is_err());
        assert!(CanonicalDecimal::new("1.0").is_err());
        assert!(CanonicalDecimal::new("1e3").is_err());
    }
}
