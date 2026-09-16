use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Structure of an analysis value. Scalar meaning belongs exclusively to SemanticType;
/// physical representation is carried by the value or field metadata, never by this type.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "inner")]
pub enum ValueType {
    Scalar(crate::SemanticType),
    Array(Box<ValueType>),
    Object,
    DataFrame,
    DataSeries(Box<ValueType>),
    Struct(std::string::String),
    OneOf(Vec<ValueType>),
    Any,
}

impl ValueType {
    /// Constructs a flattened union while preserving the first occurrence order.
    pub fn one_of(types: Vec<ValueType>) -> ValueType {
        let mut flat = Vec::new();
        for data_type in types {
            match data_type {
                ValueType::Any => return ValueType::Any,
                ValueType::OneOf(inner) => {
                    for item in inner {
                        if item == ValueType::Any {
                            return ValueType::Any;
                        }
                        if !flat.contains(&item) {
                            flat.push(item);
                        }
                    }
                }
                other => {
                    if !flat.contains(&other) {
                        flat.push(other);
                    }
                }
            }
        }

        match flat.len() {
            0 => ValueType::Any,
            1 => flat.pop().unwrap_or(ValueType::Any),
            _ => ValueType::OneOf(flat),
        }
    }

    /// Constructs the Numeric scalar meaning without selecting a physical representation.
    pub fn number() -> ValueType {
        ValueType::Scalar(crate::SemanticType::Numeric)
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueType::Scalar(semantic) => write!(formatter, "{semantic}"),
            ValueType::Array(inner) => write!(formatter, "Array<{inner}>"),
            ValueType::Object => write!(formatter, "Object"),
            ValueType::DataFrame => write!(formatter, "DataFrame"),
            ValueType::DataSeries(inner) => write!(formatter, "DataSeries<{inner}>"),
            ValueType::Struct(key) => write!(formatter, "Struct<{key}>"),
            ValueType::OneOf(types) => {
                for (index, data_type) in types.iter().enumerate() {
                    if index > 0 {
                        write!(formatter, " | ")?;
                    }
                    write!(formatter, "{data_type}")?;
                }
                Ok(())
            }
            ValueType::Any => write!(formatter, "Any"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ValueTypeParseError {
    #[error("data type is empty")]
    Empty,
    #[error("unknown data type")]
    UnknownKind,
    #[error("malformed composite data type")]
    MalformedComposite,
}

impl FromStr for ValueType {
    type Err = ValueTypeParseError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let trimmed = source.trim();
        if trimmed.is_empty() {
            return Err(ValueTypeParseError::Empty);
        }

        let parts = split_top_level(trimmed, '|')?;
        if parts.len() > 1 {
            let types = parts
                .into_iter()
                .map(str::parse)
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(ValueType::one_of(types));
        }

        if let Ok(semantic) = trimmed.parse::<crate::SemanticType>() {
            return Ok(ValueType::Scalar(semantic));
        }
        match trimmed {
            "Object" => Ok(ValueType::Object),
            "DataFrame" => Ok(ValueType::DataFrame),
            "DataSeries" => Ok(ValueType::DataSeries(Box::new(ValueType::Any))),
            "Any" => Ok(ValueType::Any),
            _ => parse_composite(trimmed),
        }
    }
}

fn parse_composite(source: &str) -> Result<ValueType, ValueTypeParseError> {
    if let Some(inner) = delimited_inner(source, "Array")? {
        let data_type = inner
            .parse()
            .map_err(|_| ValueTypeParseError::MalformedComposite)?;
        return Ok(ValueType::Array(Box::new(data_type)));
    }
    if let Some(inner) = delimited_inner(source, "DataSeries")? {
        let data_type = inner
            .parse()
            .map_err(|_| ValueTypeParseError::MalformedComposite)?;
        return Ok(ValueType::DataSeries(Box::new(data_type)));
    }
    if let Some(key) = delimited_inner(source, "Struct")? {
        return Ok(ValueType::Struct(key.to_owned()));
    }
    if source.contains(['<', '>']) {
        return Err(ValueTypeParseError::MalformedComposite);
    }
    Err(ValueTypeParseError::UnknownKind)
}

fn delimited_inner<'a>(
    source: &'a str,
    kind: &str,
) -> Result<Option<&'a str>, ValueTypeParseError> {
    let Some(rest) = source.strip_prefix(kind) else {
        return Ok(None);
    };
    if !rest.starts_with('<') || !rest.ends_with('>') {
        return Err(ValueTypeParseError::MalformedComposite);
    }
    let inner = &rest[1..rest.len() - 1];
    if !angles_are_balanced(inner) {
        return Err(ValueTypeParseError::MalformedComposite);
    }
    Ok(Some(inner))
}

fn split_top_level(source: &str, separator: char) -> Result<Vec<&str>, ValueTypeParseError> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (index, character) in source.char_indices() {
        match character {
            '<' => depth += 1,
            '>' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or(ValueTypeParseError::MalformedComposite)?;
            }
            character if character == separator && depth == 0 => {
                let part = source[start..index].trim();
                if part.is_empty() {
                    return Err(ValueTypeParseError::MalformedComposite);
                }
                parts.push(part);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(ValueTypeParseError::MalformedComposite);
    }

    let tail = source[start..].trim();
    if tail.is_empty() {
        return Err(ValueTypeParseError::MalformedComposite);
    }
    parts.push(tail);
    Ok(parts)
}

fn angles_are_balanced(source: &str) -> bool {
    let mut depth = 0usize;
    for character in source.chars() {
        match character {
            '<' => depth += 1,
            '>' => {
                let Some(next) = depth.checked_sub(1) else {
                    return false;
                };
                depth = next;
            }
            _ => {}
        }
    }
    depth == 0
}
