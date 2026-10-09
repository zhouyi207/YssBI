//! Plain JSON editing without exposing DataValue's tagged persistence format.
use anyhow::{Context as _, ensure};
use bigdecimal::BigDecimal;
use serde::{Serialize, Serializer};
use serde_json::value::RawValue;
use std::collections::BTreeMap;
use yss_data_contract::{DataValue, DecimalLiteral};

// Bound exponent expansion before allocating its canonical decimal spelling.
const MAX_NUMBER_DIGITS: u64 = 65_536;
const MAX_JSON_DEPTH: usize = 128;

pub(crate) fn number(source: &str) -> anyhow::Result<DataValue> {
    let source = source.trim();
    if let Ok(value) = source.parse::<i64>() {
        return Ok(DataValue::Integer(value));
    }
    if let Ok(value) = source.parse::<u64>() {
        return Ok(DataValue::Unsigned(value));
    }
    let value = source.parse::<BigDecimal>()?;
    ensure!(
        value
            .digits()
            .saturating_add(value.fractional_digit_count().unsigned_abs())
            <= MAX_NUMBER_DIGITS,
        "constant number is too large"
    );
    let canonical = value.normalized().to_plain_string();
    if let Ok(value) = canonical.parse::<i64>() {
        return Ok(DataValue::Integer(value));
    }
    if let Ok(value) = canonical.parse::<u64>() {
        return Ok(DataValue::Unsigned(value));
    }
    Ok(DataValue::Decimal(DecimalLiteral::new(canonical)?))
}

pub(crate) fn parse_json(source: &str) -> anyhow::Result<DataValue> {
    let raw = serde_json::from_str(source)
        .map_err(|_| InputError("detail.constantValue.errors.invalidJson"))?;
    decode(raw, 0).map_err(|_| InputError("detail.constantValue.errors.invalidValue").into())
}

fn decode(raw: &RawValue, depth: usize) -> anyhow::Result<DataValue> {
    ensure!(
        depth <= MAX_JSON_DEPTH,
        "constant JSON is too deeply nested"
    );
    let source = raw.get().trim();
    Ok(
        match source.as_bytes().first().context("empty JSON value")? {
            b'n' => DataValue::Null,
            b't' => DataValue::Bool(true),
            b'f' => DataValue::Bool(false),
            b'"' => DataValue::String(serde_json::from_str(source)?),
            b'[' => DataValue::List(
                serde_json::from_str::<Vec<&RawValue>>(source)?
                    .into_iter()
                    .map(|value| decode(value, depth + 1))
                    .collect::<anyhow::Result<_>>()?,
            ),
            b'{' => DataValue::Object(
                serde_json::from_str::<BTreeMap<Box<str>, &RawValue>>(source)?
                    .into_iter()
                    .map(|(key, value)| Ok((key, decode(value, depth + 1)?)))
                    .collect::<anyhow::Result<_>>()?,
            ),
            _ => number(source)?,
        },
    )
}

pub(crate) fn json_text(value: &DataValue) -> anyhow::Result<String> {
    Ok(serde_json::to_string_pretty(&JsonValue(value))?)
}

struct JsonValue<'a>(&'a DataValue);

impl Serialize for JsonValue<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            DataValue::Null => serializer.serialize_none(),
            DataValue::Bool(value) => serializer.serialize_bool(*value),
            DataValue::Integer(value) => serializer.serialize_i64(*value),
            DataValue::Unsigned(value) => serializer.serialize_u64(*value),
            DataValue::Decimal(value) => {
                let raw: &RawValue =
                    serde_json::from_str(value.as_str()).map_err(serde::ser::Error::custom)?;
                raw.serialize(serializer)
            }
            DataValue::String(value) => serializer.serialize_str(value),
            DataValue::List(values) => serializer.collect_seq(values.iter().map(JsonValue)),
            DataValue::Object(values) => {
                serializer.collect_map(values.iter().map(|(key, value)| (key, JsonValue(value))))
            }
            // Bytes are outside the graph constant types exposed by this editor.
            DataValue::Bytes(_) => Err(serde::ser::Error::custom("bytes are not JSON constants")),
        }
    }
}

#[derive(Debug)]
pub(crate) struct InputError(pub(crate) &'static str);
impl std::fmt::Display for InputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.0)
    }
}
impl std::error::Error for InputError {}
