use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A format-neutral value tree. Object ordering and decimal spelling are stable
/// so serialized protocol defaults do not depend on hash order or host floats.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataValue {
    #[default]
    Null,
    Bool(bool),
    Integer(#[serde(with = "signed_integer")] i64),
    Unsigned(#[serde(with = "unsigned_integer")] u64),
    Decimal(DecimalLiteral),
    String(Box<str>),
    Bytes(Vec<u8>),
    List(Vec<DataValue>),
    Object(BTreeMap<Box<str>, DataValue>),
}

/// Lossless decimal spelling for persisted literals and exact Arrow decimal filters.
/// The execution boundary chooses the numeric representation used for arithmetic.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct DecimalLiteral(Box<str>);

impl TryFrom<f64> for DecimalLiteral {
    type Error = InvalidDecimal;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if !value.is_finite() {
            return Err(InvalidDecimal(value.to_string()));
        }
        Self::new(if value == 0.0 {
            "0".into()
        } else {
            value.to_string()
        })
    }
}

impl DecimalLiteral {
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

impl<'de> Deserialize<'de> for DecimalLiteral {
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

// JSON transports must not round i64/u64 through JavaScript's binary64 numbers.
macro_rules! integer_wire {
    ($module:ident, $type:ty) => {
        mod $module {
            use serde::{Deserialize, Deserializer, Serializer};

            pub fn serialize<S: Serializer>(
                value: &$type,
                serializer: S,
            ) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&value.to_string())
            }

            pub fn deserialize<'de, D: Deserializer<'de>>(
                deserializer: D,
            ) -> Result<$type, D::Error> {
                let text = String::deserialize(deserializer)?;
                let value = text.parse::<$type>().map_err(serde::de::Error::custom)?;
                if value.to_string() != text {
                    return Err(serde::de::Error::custom("non-canonical integer"));
                }
                Ok(value)
            }
        }
    };
}

integer_wire!(signed_integer, i64);
integer_wire!(unsigned_integer, u64);

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
        || (!integer.is_empty()
            && !integer.starts_with('0')
            && integer.bytes().all(|byte| byte.is_ascii_digit()));
    let fraction_valid = fraction.is_empty()
        || (fraction.bytes().all(|byte| byte.is_ascii_digit()) && !fraction.ends_with('0'));
    integer_valid && fraction_valid && !value.ends_with('.')
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "value",
    rename_all = "camelCase",
    deny_unknown_fields
)]
pub enum FilterLiteral {
    Boolean(bool),
    Integer(#[serde(with = "signed_integer")] i64),
    Decimal(DecimalLiteral),
    String(Box<str>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimals_require_canonical_non_exponent_spelling() {
        assert!(DecimalLiteral::new("-12.34").is_ok());
        assert!(DecimalLiteral::new("01").is_err());
        assert!(DecimalLiteral::new(".1").is_err());
        assert!(DecimalLiteral::new("-.1").is_err());
        assert!(DecimalLiteral::new("1.0").is_err());
        assert!(DecimalLiteral::new("1e3").is_err());
    }
}
