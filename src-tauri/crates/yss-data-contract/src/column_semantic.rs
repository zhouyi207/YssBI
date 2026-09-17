use serde::{Deserialize, Serialize};

/// Internal interpretation of stored values, independent of their physical representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SemanticType {
    Numeric,
    Categorical,
    Ordinal,
    Binary,
    Datetime,
    Text,
    Identifier,
}

impl SemanticType {
    pub const ALL: [Self; 7] = [
        Self::Numeric,
        Self::Categorical,
        Self::Ordinal,
        Self::Binary,
        Self::Datetime,
        Self::Text,
        Self::Identifier,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Numeric => "Numeric",
            Self::Categorical => "Categorical",
            Self::Ordinal => "Ordinal",
            Self::Binary => "Binary",
            Self::Datetime => "Datetime",
            Self::Text => "Text",
            Self::Identifier => "Identifier",
        }
    }

    pub const fn type_id(self) -> &'static str {
        match self {
            Self::Numeric => "core.numeric",
            Self::Categorical => "core.categorical",
            Self::Ordinal => "core.ordinal",
            Self::Binary => "core.binary",
            Self::Datetime => "core.datetime",
            Self::Text => "core.text",
            Self::Identifier => "core.identifier",
        }
    }
}

impl std::fmt::Display for SemanticType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl std::str::FromStr for SemanticType {
    type Err = crate::ValueTypeParseError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|semantic| semantic.as_str() == value)
            .ok_or(crate::ValueTypeParseError::UnknownKind)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticValue {
    /// Exact text representation of the stored value; never a JavaScript number.
    pub value: String,
    /// Internal meaning of the code. Data views continue to display the original value.
    pub label: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NumericConstraints {
    #[serde(default)]
    pub integer: bool,
    #[serde(default)]
    pub minimum: Option<String>,
    #[serde(default)]
    pub maximum: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ColumnSemantic {
    pub kind: SemanticType,
    /// For Ordinal, this list is explicitly ordered from lowest to highest.
    #[serde(default)]
    pub values: Vec<SemanticValue>,
    #[serde(default)]
    pub positive_value: Option<String>,
    #[serde(default)]
    pub numeric: Option<NumericConstraints>,
}

impl ColumnSemantic {
    pub fn new(kind: SemanticType) -> Self {
        Self {
            kind,
            values: Vec::new(),
            positive_value: None,
            numeric: None,
        }
    }
}
