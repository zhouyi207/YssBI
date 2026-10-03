//! Shared meanings and output names for descriptive table operations.
use crate::SemanticType;

pub fn supports_description(kind: SemanticType) -> bool {
    matches!(
        kind,
        SemanticType::Numeric
            | SemanticType::Categorical
            | SemanticType::Ordinal
            | SemanticType::Binary
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AggregateOperation {
    Count,
    Sum,
    Mean,
    Min,
    Max,
    Std,
    Median,
}

impl AggregateOperation {
    pub const ALL: [Self; 7] = [
        Self::Count,
        Self::Sum,
        Self::Mean,
        Self::Min,
        Self::Max,
        Self::Std,
        Self::Median,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::Count => "count",
            Self::Sum => "sum",
            Self::Mean => "mean",
            Self::Min => "min",
            Self::Max => "max",
            Self::Std => "std",
            Self::Median => "median",
        }
    }

    pub fn accepts(self, kind: SemanticType) -> bool {
        self == Self::Count || kind == SemanticType::Numeric
    }

    pub fn output_name(self, column: &str) -> String {
        format!("{column}_{}", self.key())
    }
}

#[derive(Debug, Clone)]
pub struct ColumnAggregate {
    pub column: Box<str>,
    pub operation: AggregateOperation,
}

/// One row per described column. Inapplicable or undefined statistics remain Null.
pub const DESCRIPTION_FIELDS: &[(&str, SemanticType)] = &[
    ("column", SemanticType::Text),
    ("semantic", SemanticType::Text),
    ("count", SemanticType::Numeric),
    ("missing", SemanticType::Numeric),
    ("mean", SemanticType::Numeric),
    ("std", SemanticType::Numeric),
    ("min", SemanticType::Numeric),
    ("q25", SemanticType::Numeric),
    ("median", SemanticType::Numeric),
    ("q75", SemanticType::Numeric),
    ("max", SemanticType::Numeric),
    ("unique", SemanticType::Numeric),
];
