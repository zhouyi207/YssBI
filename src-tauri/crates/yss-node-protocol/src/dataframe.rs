use crate::RelationalScalarType;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeSet;
use yss_data_contract::{FilterLiteral, TabularColumnName};

mod schema;

pub const PROJECT_COLUMNS_TYPE_ID: &str = "yssbi.dataframe.project_columns";
pub const FILTER_PREDICATE_TYPE_ID: &str = "yssbi.dataframe.filter_predicate";
pub const PROJECT_COLUMNS_VALIDATOR_ID: &str = "yssbi.dataframe.project_columns.codec";
pub const FILTER_PREDICATE_VALIDATOR_ID: &str = "yssbi.dataframe.filter_predicate.codec";
pub const DATAFRAME_NOMINAL_CODEC_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(transparent)]
#[schemars(extend("uniqueItems" = true))]
pub struct ProjectColumns(
    #[schemars(with = "Vec<TabularColumnName>", length(min = 1))] Box<[Box<str>]>,
);

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
        validate_column_names(columns.iter().map(AsRef::as_ref))
            .map_err(serde::de::Error::custom)?;
        Ok(Self(columns))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
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
        if !TabularColumnName::is_valid(&wire.column) {
            return Err(serde::de::Error::custom("filter column must not be blank"));
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
    use crate::SemanticType;
    let equality = matches!(operator, FilterOperator::Equal | FilterOperator::NotEqual);
    match (scalar_type, literal) {
        (RelationalScalarType::Known(SemanticType::Numeric), FilterLiteral::Integer(_)) => true,
        (RelationalScalarType::Known(SemanticType::Numeric), FilterLiteral::Decimal(value)) => {
            value.as_str().parse::<f64>().is_ok_and(f64::is_finite)
        }
        (
            RelationalScalarType::Known(SemanticType::Text | SemanticType::Datetime),
            FilterLiteral::String(_),
        ) => true,
        (RelationalScalarType::Known(SemanticType::Ordinal), _) => true,
        (
            RelationalScalarType::Known(
                SemanticType::Categorical | SemanticType::Binary | SemanticType::Identifier,
            ),
            _,
        ) => equality,
        _ => false,
    }
}

pub fn prepare_project_columns_json(value: &serde_json::Value) -> Result<ProjectColumns, String> {
    serde_json::from_value::<ProjectColumns>(value.clone()).map_err(|error| error.to_string())
}

fn validate_column_names<'a>(
    columns: impl IntoIterator<Item = &'a str>,
) -> Result<(), &'static str> {
    let mut seen = BTreeSet::new();
    for column in columns {
        if !TabularColumnName::is_valid(column) {
            return Err("column names must not be blank");
        }
        if !seen.insert(column) {
            return Err("column names must be unique");
        }
    }
    Ok(())
}

/// Ordered, distinct column references; an empty list can mean all columns or no selection.
pub fn prepare_column_names_json(value: &serde_json::Value) -> Result<Vec<Box<str>>, String> {
    let columns = value
        .as_array()
        .ok_or("column names must be an array")?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(Box::from)
                .ok_or("column names must be strings")
        })
        .collect::<Result<Vec<Box<str>>, _>>()?;
    validate_column_names(columns.iter().map(AsRef::as_ref))?;
    Ok(columns)
}

pub fn prepare_filter_predicate_json(value: &serde_json::Value) -> Result<FilterPredicate, String> {
    serde_json::from_value::<FilterPredicate>(value.clone()).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use yss_data_contract::DecimalLiteral;

    #[test]
    fn project_columns_roundtrip_exact_persisted_array() {
        let columns: ProjectColumns = serde_json::from_value(json!([" b\t", "a"])).unwrap();

        assert_eq!(
            columns.as_slice(),
            [Box::<str>::from(" b\t"), Box::<str>::from("a")]
        );
        assert_eq!(serde_json::to_value(columns).unwrap(), json!([" b\t", "a"]));
    }

    #[test]
    fn project_columns_reject_invalid_shapes_names_and_duplicates() {
        for invalid in [
            json!([]),
            json!(["a", "a"]),
            json!([""]),
            json!([" "]),
            json!(["\t\n"]),
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
                json!({"column":" active\t","operator":"equal","value":{"type":"boolean","value":true}}),
                FilterLiteral::Boolean(true),
            ),
            (
                json!({"column":"count","operator":"greaterThan","value":{"type":"integer","value":"9007199254740993"}}),
                FilterLiteral::Integer(9_007_199_254_740_993),
            ),
            (
                json!({"column":"amount","operator":"lessThanOrEqual","value":{"type":"decimal","value":"10.5"}}),
                FilterLiteral::Decimal(DecimalLiteral::new("10.5").unwrap()),
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
        let decimal = FilterLiteral::Decimal(DecimalLiteral::new("10.5").unwrap());
        let huge_decimal =
            FilterLiteral::Decimal(DecimalLiteral::new(format!("1{}", "0".repeat(400))).unwrap());

        assert!(filter_comparison_is_compatible(
            RelationalScalarType::Known(crate::SemanticType::Numeric),
            FilterOperator::GreaterThan,
            Some(&integer),
        ));
        assert!(filter_comparison_is_compatible(
            RelationalScalarType::Known(crate::SemanticType::Numeric),
            FilterOperator::Equal,
            Some(&integer),
        ));
        assert!(filter_comparison_is_compatible(
            RelationalScalarType::Known(crate::SemanticType::Numeric),
            FilterOperator::Equal,
            Some(&inexact_integer),
        ));
        assert!(filter_comparison_is_compatible(
            RelationalScalarType::Known(crate::SemanticType::Numeric),
            FilterOperator::LessThan,
            Some(&decimal),
        ));
        assert!(!filter_comparison_is_compatible(
            RelationalScalarType::Known(crate::SemanticType::Numeric),
            FilterOperator::LessThan,
            Some(&huge_decimal),
        ));
        assert!(filter_comparison_is_compatible(
            RelationalScalarType::Known(crate::SemanticType::Datetime),
            FilterOperator::IsNull,
            None,
        ));
        assert!(filter_comparison_is_compatible(
            RelationalScalarType::Known(crate::SemanticType::Datetime),
            FilterOperator::Equal,
            Some(&FilterLiteral::String("2026-08-03".into())),
        ));
        assert!(!filter_comparison_is_compatible(
            RelationalScalarType::Unknown,
            FilterOperator::IsNull,
            None,
        ));
        assert!(!filter_comparison_is_compatible(
            RelationalScalarType::Known(crate::SemanticType::Binary),
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
            json!({"column":" \t","operator":"isNull"}),
        ] {
            assert!(serde_json::from_value::<FilterPredicate>(invalid).is_err());
        }
    }
}
