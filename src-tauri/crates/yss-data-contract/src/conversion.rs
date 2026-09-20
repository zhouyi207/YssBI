use serde::{Deserialize, Serialize};

use crate::{ColumnSemantic, SemanticType, SemanticValue};

/// A fixed representation policy, resolved from schema rather than sampled rows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumericRepresentation {
    #[default]
    Auto,
    Integer,
    Real,
}

impl NumericRepresentation {
    pub fn from_parameter(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "integer" => Some(Self::Integer),
            "real" => Some(Self::Real),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum DatetimeRepresentation {
    #[default]
    Auto,
    Date,
    Time,
    Datetime,
}

impl DatetimeRepresentation {
    pub fn from_parameter(value: &str) -> Option<Self> {
        match value {
            "auto" => Some(Self::Auto),
            "date" => Some(Self::Date),
            "time" => Some(Self::Time),
            "datetime" => Some(Self::Datetime),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum TemporalPrecision {
    Seconds,
    Milliseconds,
    #[default]
    Microseconds,
    Nanoseconds,
}

impl TemporalPrecision {
    pub fn from_parameter(value: &str) -> Option<Self> {
        match value {
            "seconds" => Some(Self::Seconds),
            "milliseconds" => Some(Self::Milliseconds),
            "microseconds" => Some(Self::Microseconds),
            "nanoseconds" => Some(Self::Nanoseconds),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConversionDomain {
    #[serde(default)]
    pub values: Vec<SemanticValue>,
    #[serde(default)]
    pub positive_value: Option<String>,
}

impl ConversionDomain {
    pub fn is_valid(&self) -> bool {
        let mut seen = std::collections::BTreeSet::new();
        self.values.len() <= 65_536
            && self
                .values
                .iter()
                .all(|value| seen.insert(value.value.as_str()))
            && self
                .values
                .iter()
                .map(|value| value.value.len().saturating_add(value.label.len()))
                .sum::<usize>()
                <= 1024 * 1024
            && self
                .positive_value
                .as_ref()
                .is_none_or(|positive| seen.contains(positive.as_str()))
    }
}

/// Resolved calendar representation; unlike configuration, it cannot be automatic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TemporalType {
    Date,
    Time(TemporalPrecision),
    Datetime(TemporalPrecision),
}

/// Meaning carried by materialized values, without importing Arrow into the kernel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionMetadata {
    pub semantic: ColumnSemantic,
    pub temporal: Option<TemporalType>,
    /// Reference category for downstream dummy encoding; values remain unchanged.
    pub dummy_base_level: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SemanticConversion {
    pub target: SemanticType,
    pub numeric: NumericRepresentation,
    pub domain: ConversionDomain,
    pub datetime: DatetimeRepresentation,
    pub precision: TemporalPrecision,
    pub format: String,
}

impl SemanticConversion {
    pub fn new(target: SemanticType, numeric: NumericRepresentation) -> Self {
        Self {
            target,
            numeric,
            domain: ConversionDomain::default(),
            datetime: DatetimeRepresentation::Auto,
            precision: TemporalPrecision::Microseconds,
            format: String::new(),
        }
    }

    pub fn from_parameters(target: &str, numeric: &str) -> Option<Self> {
        let target = SemanticType::ALL
            .into_iter()
            .find(|kind| kind.type_id() == target)?;
        Some(Self::new(
            target,
            NumericRepresentation::from_parameter(numeric)?,
        ))
    }
}
