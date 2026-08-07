use crate::node_system::protocol::{CanonicalDecimal, RelationalScalarType};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeSet;

pub const PROJECT_COLUMNS_TYPE_ID: &str = "yssbi.dataframe.project_columns";
pub const FILTER_PREDICATE_TYPE_ID: &str = "yssbi.dataframe.filter_predicate";
pub const PROJECT_COLUMNS_VALIDATOR_ID: &str = "yssbi.dataframe.project_columns.codec";
pub const FILTER_PREDICATE_VALIDATOR_ID: &str = "yssbi.dataframe.filter_predicate.codec";
pub const DATAFRAME_NOMINAL_CODEC_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ProjectColumns(Box<[Box<str>]>);

impl ProjectColumns {
    pub fn as_slice(&self) -> &[Box<str>] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for ProjectColumns {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let columns = Box::<[Box<str>]>::deserialize(deserializer)?;
        if columns.is_empty() {
            return Err(serde::de::Error::custom(
                "project columns must not be empty",
            ));
        }
        let mut seen = BTreeSet::new();
        for column in &columns {
            if column.is_empty() || column.trim() != column.as_ref() {
                return Err(serde::de::Error::custom(
                    "project column names must be non-empty and unpadded",
                ));
            }
            if !seen.insert(column.as_ref()) {
                return Err(serde::de::Error::custom("project columns must be unique"));
            }
        }
        Ok(Self(columns))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FilterOperator {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
    IsNull,
    IsNotNull,
}

impl FilterOperator {
    pub fn requires_value(self) -> bool {
        !matches!(self, Self::IsNull | Self::IsNotNull)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterLiteral {
    Boolean(bool),
    Integer(i64),
    Decimal(CanonicalDecimal),
    String(Box<str>),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum FilterLiteralWire {
    Boolean { value: bool },
    Integer { value: Box<str> },
    Decimal { value: CanonicalDecimal },
    String { value: Box<str> },
}

impl Serialize for FilterLiteral {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let wire = match self {
            Self::Boolean(value) => FilterLiteralWire::Boolean { value: *value },
            Self::Integer(value) => FilterLiteralWire::Integer {
                value: value.to_string().into(),
            },
            Self::Decimal(value) => FilterLiteralWire::Decimal {
                value: value.clone(),
            },
            Self::String(value) => FilterLiteralWire::String {
                value: value.clone(),
            },
        };
        wire.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FilterLiteral {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match FilterLiteralWire::deserialize(deserializer)? {
            FilterLiteralWire::Boolean { value } => Ok(Self::Boolean(value)),
            FilterLiteralWire::Integer { value } => {
                let parsed = value.parse::<i64>().map_err(serde::de::Error::custom)?;
                if parsed.to_string() != value.as_ref() {
                    return Err(serde::de::Error::custom(
                        "integer must use canonical spelling",
                    ));
                }
                Ok(Self::Integer(parsed))
            }
            FilterLiteralWire::Decimal { value } => Ok(Self::Decimal(value)),
            FilterLiteralWire::String { value } => Ok(Self::String(value)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterPredicate {
    pub column: Box<str>,
    pub operator: FilterOperator,
    pub value: Option<FilterLiteral>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FilterPredicateWire {
    column: Box<str>,
    operator: FilterOperator,
    #[serde(default)]
    value: OptionalLiteral,
}

#[derive(Default)]
enum OptionalLiteral {
    #[default]
    Missing,
    Present(FilterLiteral),
}

impl<'de> Deserialize<'de> for OptionalLiteral {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        FilterLiteral::deserialize(deserializer).map(Self::Present)
    }
}

impl Serialize for FilterPredicate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(if self.value.is_some() { 3 } else { 2 }))?;
        map.serialize_entry("column", &self.column)?;
        map.serialize_entry("operator", &self.operator)?;
        if let Some(value) = &self.value {
            map.serialize_entry("value", value)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for FilterPredicate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = FilterPredicateWire::deserialize(deserializer)?;
        if wire.column.is_empty() || wire.column.trim() != wire.column.as_ref() {
            return Err(serde::de::Error::custom(
                "filter column must be non-empty and unpadded",
            ));
        }
        let value = match (wire.operator.requires_value(), wire.value) {
            (true, OptionalLiteral::Present(value)) => Some(value),
            (true, OptionalLiteral::Missing) => {
                return Err(serde::de::Error::custom(
                    "comparison operator requires a value",
                ));
            }
            (false, OptionalLiteral::Missing) => None,
            (false, OptionalLiteral::Present(_)) => {
                return Err(serde::de::Error::custom(
                    "null-check operator forbids a value",
                ));
            }
        };
        Ok(Self {
            column: wire.column,
            operator: wire.operator,
            value,
        })
    }
}

pub fn filter_comparison_is_compatible(
    scalar_type: RelationalScalarType,
    operator: FilterOperator,
    literal: Option<&FilterLiteral>,
) -> bool {
    if matches!(scalar_type, RelationalScalarType::Unknown) {
        return false;
    }
    if matches!(operator, FilterOperator::IsNull | FilterOperator::IsNotNull) {
        return literal.is_none();
    }
    let Some(literal) = literal else {
        return false;
    };
    match (scalar_type, literal) {
        (RelationalScalarType::Boolean, FilterLiteral::Boolean(_)) => {
            matches!(operator, FilterOperator::Equal | FilterOperator::NotEqual)
        }
        (RelationalScalarType::Int64, FilterLiteral::Integer(_))
        | (RelationalScalarType::String, FilterLiteral::String(_)) => true,
        (RelationalScalarType::Float64, FilterLiteral::Integer(value)) => {
            let converted = *value as f64;
            converted.is_finite() && converted as i128 == i128::from(*value)
        }
        (RelationalScalarType::Float64, FilterLiteral::Decimal(value)) => {
            value.as_str().parse::<f64>().is_ok_and(f64::is_finite)
        }
        _ => false,
    }
}

pub fn prepare_project_columns_json(value: &serde_json::Value) -> Result<ProjectColumns, String> {
    serde_json::from_value::<ProjectColumns>(value.clone()).map_err(|error| error.to_string())
}

pub fn validate_project_columns_json(value: &serde_json::Value) -> Result<(), String> {
    prepare_project_columns_json(value).map(|_| ())
}

pub fn prepare_filter_predicate_json(value: &serde_json::Value) -> Result<FilterPredicate, String> {
    serde_json::from_value::<FilterPredicate>(value.clone()).map_err(|error| error.to_string())
}

pub fn validate_filter_predicate_json(value: &serde_json::Value) -> Result<(), String> {
    prepare_filter_predicate_json(value).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::protocol::CanonicalDecimal;
    use serde_json::json;

    #[test]
    fn project_columns_roundtrip_exact_persisted_array() {
        let columns: ProjectColumns = serde_json::from_value(json!(["b", "a"])).unwrap();

        assert_eq!(
            columns.as_slice(),
            [Box::<str>::from("b"), Box::<str>::from("a")]
        );
        assert_eq!(serde_json::to_value(columns).unwrap(), json!(["b", "a"]));
        assert_eq!(PROJECT_COLUMNS_TYPE_ID, "yssbi.dataframe.project_columns");
    }

    #[test]
    fn project_columns_reject_invalid_shapes_names_and_duplicates() {
        for invalid in [
            json!([]),
            json!(["a", "a"]),
            json!([""]),
            json!([" a"]),
            json!(["a "]),
            json!([1]),
            json!({"columns": ["a"]}),
        ] {
            assert!(serde_json::from_value::<ProjectColumns>(invalid).is_err());
        }
    }

    #[test]
    fn filter_predicate_roundtrips_exact_tagged_literals() {
        let cases = [
            (
                json!({"column":"active","operator":"equal","value":{"type":"boolean","value":true}}),
                FilterLiteral::Boolean(true),
            ),
            (
                json!({"column":"count","operator":"greaterThan","value":{"type":"integer","value":"9007199254740993"}}),
                FilterLiteral::Integer(9_007_199_254_740_993),
            ),
            (
                json!({"column":"amount","operator":"lessThanOrEqual","value":{"type":"decimal","value":"10.5"}}),
                FilterLiteral::Decimal(CanonicalDecimal::new("10.5").unwrap()),
            ),
            (
                json!({"column":"status","operator":"notEqual","value":{"type":"string","value":"paid"}}),
                FilterLiteral::String("paid".into()),
            ),
        ];

        for (wire, expected_literal) in cases {
            let predicate: FilterPredicate = serde_json::from_value(wire.clone()).unwrap();
            assert_eq!(predicate.value, Some(expected_literal));
            assert_eq!(serde_json::to_value(predicate).unwrap(), wire);
        }
        assert_eq!(FILTER_PREDICATE_TYPE_ID, "yssbi.dataframe.filter_predicate");
    }

    #[test]
    fn null_operators_forbid_value_and_comparisons_require_it() {
        for operator in [FilterOperator::IsNull, FilterOperator::IsNotNull] {
            let predicate = FilterPredicate {
                column: "optional".into(),
                operator,
                value: None,
            };
            let wire = serde_json::to_value(&predicate).unwrap();
            assert!(wire.get("value").is_none());
            assert_eq!(
                serde_json::from_value::<FilterPredicate>(wire).unwrap(),
                predicate
            );
        }

        assert!(
            serde_json::from_value::<FilterPredicate>(json!({
                "column":"optional","operator":"isNull","value":{"type":"string","value":"x"}
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<FilterPredicate>(json!({
                "column":"amount","operator":"greaterThan"
            }))
            .is_err()
        );
    }

    #[test]
    fn comparison_compatibility_is_typed_and_exact() {
        let integer = FilterLiteral::Integer(42);
        let inexact_integer = FilterLiteral::Integer(9_007_199_254_740_993);
        let decimal = FilterLiteral::Decimal(CanonicalDecimal::new("10.5").unwrap());
        let huge_decimal =
            FilterLiteral::Decimal(CanonicalDecimal::new(format!("1{}", "0".repeat(400))).unwrap());

        assert!(filter_comparison_is_compatible(
            RelationalScalarType::Int64,
            FilterOperator::GreaterThan,
            Some(&integer),
        ));
        assert!(filter_comparison_is_compatible(
            RelationalScalarType::Float64,
            FilterOperator::Equal,
            Some(&integer),
        ));
        assert!(!filter_comparison_is_compatible(
            RelationalScalarType::Float64,
            FilterOperator::Equal,
            Some(&inexact_integer),
        ));
        assert!(filter_comparison_is_compatible(
            RelationalScalarType::Float64,
            FilterOperator::LessThan,
            Some(&decimal),
        ));
        assert!(!filter_comparison_is_compatible(
            RelationalScalarType::Float64,
            FilterOperator::LessThan,
            Some(&huge_decimal),
        ));
        assert!(filter_comparison_is_compatible(
            RelationalScalarType::Date,
            FilterOperator::IsNull,
            None,
        ));
        assert!(!filter_comparison_is_compatible(
            RelationalScalarType::DateTime,
            FilterOperator::Equal,
            Some(&FilterLiteral::String("2026-08-03".into())),
        ));
        assert!(!filter_comparison_is_compatible(
            RelationalScalarType::Unknown,
            FilterOperator::IsNull,
            None,
        ));
        assert!(!filter_comparison_is_compatible(
            RelationalScalarType::Boolean,
            FilterOperator::LessThan,
            Some(&FilterLiteral::Boolean(false)),
        ));
    }

    #[test]
    fn filter_predicate_rejects_unknown_fields_tags_and_noncanonical_values() {
        for invalid in [
            json!({"column":"a","operator":"equal","value":{"type":"integer","value":1}}),
            json!({"column":"a","operator":"equal","value":{"type":"integer","value":"01"}}),
            json!({"column":"a","operator":"equal","value":{"type":"integer","value":"9223372036854775808"}}),
            json!({"column":"a","operator":"equal","value":{"type":"decimal","value":10.5}}),
            json!({"column":"a","operator":"equal","value":{"type":"decimal","value":"1.0"}}),
            json!({"column":"a","operator":"unknown","value":{"type":"string","value":"x"}}),
            json!({"column":"a","operator":"equal","value":{"type":"unknown","value":"x"}}),
            json!({"column":"a","operator":"equal","value":{"type":"string","value":"x","extra":true}}),
            json!({"column":"a","operator":"equal","value":{"type":"string","value":{"type":"string","value":"x"}}}),
            json!({"column":"a","operator":"equal","value":{"type":"string","value":"x"},"extra":true}),
            json!({"column":"","operator":"equal","value":{"type":"string","value":"x"}}),
        ] {
            assert!(serde_json::from_value::<FilterPredicate>(invalid).is_err());
        }
    }
}
