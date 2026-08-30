use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Persisted data type metadata.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "inner")]
pub enum DataType {
    Boolean,
    Int64,
    Float64,
    String,
    Date,
    Datetime,
    Time,
    Categorical,
    Array(Box<DataType>),
    Object,
    DataFrame,
    DataSeries(Box<DataType>),
    Struct(std::string::String),
    OneOf(Vec<DataType>),
    Any,
}

impl DataType {
    /// Constructs a flattened union while preserving the first occurrence order.
    pub fn one_of(types: Vec<DataType>) -> DataType {
        let mut flat = Vec::new();
        for data_type in types {
            match data_type {
                DataType::Any => return DataType::Any,
                DataType::OneOf(inner) => {
                    for item in inner {
                        if item == DataType::Any {
                            return DataType::Any;
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
            0 => DataType::Any,
            1 => flat.pop().unwrap_or(DataType::Any),
            _ => DataType::OneOf(flat),
        }
    }

    /// Canonical persisted representation of a numeric union.
    pub fn number() -> DataType {
        DataType::OneOf(vec![DataType::Int64, DataType::Float64])
    }
}

impl fmt::Display for DataType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataType::Boolean => write!(formatter, "Boolean"),
            DataType::Int64 => write!(formatter, "Int64"),
            DataType::Float64 => write!(formatter, "Float64"),
            DataType::String => write!(formatter, "String"),
            DataType::Date => write!(formatter, "Date"),
            DataType::Datetime => write!(formatter, "Datetime"),
            DataType::Time => write!(formatter, "Time"),
            DataType::Categorical => write!(formatter, "Categorical"),
            DataType::Array(inner) => write!(formatter, "Array<{inner}>"),
            DataType::Object => write!(formatter, "Object"),
            DataType::DataFrame => write!(formatter, "DataFrame"),
            DataType::DataSeries(inner) => write!(formatter, "DataSeries<{inner}>"),
            DataType::Struct(key) => write!(formatter, "Struct<{key}>"),
            DataType::OneOf(types) => {
                for (index, data_type) in types.iter().enumerate() {
                    if index > 0 {
                        write!(formatter, " | ")?;
                    }
                    write!(formatter, "{data_type}")?;
                }
                Ok(())
            }
            DataType::Any => write!(formatter, "Any"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum DataTypeParseError {
    #[error("data type is empty")]
    Empty,
    #[error("unknown data type")]
    UnknownKind,
    #[error("malformed composite data type")]
    MalformedComposite,
}

impl FromStr for DataType {
    type Err = DataTypeParseError;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        let trimmed = source.trim();
        if trimmed.is_empty() {
            return Err(DataTypeParseError::Empty);
        }

        let parts = split_top_level(trimmed, '|')?;
        if parts.len() > 1 {
            let types = parts
                .into_iter()
                .map(str::parse)
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(DataType::one_of(types));
        }

        match trimmed {
            "Boolean" => Ok(DataType::Boolean),
            "Int8" | "Int16" | "Int32" | "Int64" | "UInt8" | "UInt16" | "UInt32" | "UInt64" => {
                Ok(DataType::Int64)
            }
            "Float32" | "Float64" => Ok(DataType::Float64),
            "Number" => Ok(DataType::number()),
            "String" => Ok(DataType::String),
            "Date" => Ok(DataType::Date),
            "Datetime" | "DateTime" => Ok(DataType::Datetime),
            "Time" => Ok(DataType::Time),
            "Categorical" => Ok(DataType::Categorical),
            "Object" => Ok(DataType::Object),
            "DataFrame" => Ok(DataType::DataFrame),
            "DataSeries" => Ok(DataType::DataSeries(Box::new(DataType::Any))),
            "Any" => Ok(DataType::Any),
            _ => parse_composite(trimmed),
        }
    }
}

fn parse_composite(source: &str) -> Result<DataType, DataTypeParseError> {
    if let Some(inner) = delimited_inner(source, "Array")? {
        let data_type = inner
            .parse()
            .map_err(|_| DataTypeParseError::MalformedComposite)?;
        return Ok(DataType::Array(Box::new(data_type)));
    }
    if let Some(inner) = delimited_inner(source, "DataSeries")? {
        let data_type = inner
            .parse()
            .map_err(|_| DataTypeParseError::MalformedComposite)?;
        return Ok(DataType::DataSeries(Box::new(data_type)));
    }
    if let Some(key) = delimited_inner(source, "Struct")? {
        return Ok(DataType::Struct(key.to_owned()));
    }
    if source.contains(['<', '>']) {
        return Err(DataTypeParseError::MalformedComposite);
    }
    Err(DataTypeParseError::UnknownKind)
}

fn delimited_inner<'a>(source: &'a str, kind: &str) -> Result<Option<&'a str>, DataTypeParseError> {
    let Some(rest) = source.strip_prefix(kind) else {
        return Ok(None);
    };
    if !rest.starts_with('<') || !rest.ends_with('>') {
        return Err(DataTypeParseError::MalformedComposite);
    }
    let inner = &rest[1..rest.len() - 1];
    if !angles_are_balanced(inner) {
        return Err(DataTypeParseError::MalformedComposite);
    }
    Ok(Some(inner))
}

fn split_top_level(source: &str, separator: char) -> Result<Vec<&str>, DataTypeParseError> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (index, character) in source.char_indices() {
        match character {
            '<' => depth += 1,
            '>' => {
                depth = depth
                    .checked_sub(1)
                    .ok_or(DataTypeParseError::MalformedComposite)?;
            }
            character if character == separator && depth == 0 => {
                let part = source[start..index].trim();
                if part.is_empty() {
                    return Err(DataTypeParseError::MalformedComposite);
                }
                parts.push(part);
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(DataTypeParseError::MalformedComposite);
    }

    let tail = source[start..].trim();
    if tail.is_empty() {
        return Err(DataTypeParseError::MalformedComposite);
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
