use super::{
    InterfaceResolverId, ParameterKey, PortKey, SchemaResolverId, TypeClassId, TypeConstructorId,
    TypeId, TypeParameterId,
};
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeExpr {
    Concrete(TypeId),
    Generic(TypeParameterId),
    Applied {
        constructor: TypeConstructorId,
        arguments: Vec<TypeExpr>,
    },
    Union(Vec<TypeExpr>),
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeNormalizationError {
    EmptyUnion,
}

impl fmt::Display for TypeNormalizationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyUnion => formatter.write_str("type unions must contain at least one member"),
        }
    }
}

impl std::error::Error for TypeNormalizationError {}

pub fn normalize_type_expr(value: TypeExpr) -> Result<TypeExpr, TypeNormalizationError> {
    match value {
        TypeExpr::Applied {
            constructor,
            arguments,
        } => Ok(TypeExpr::Applied {
            constructor,
            arguments: arguments
                .into_iter()
                .map(normalize_type_expr)
                .collect::<Result<_, _>>()?,
        }),
        TypeExpr::Union(members) => normalize_union(members),
        value => Ok(value),
    }
}

fn normalize_union(members: Vec<TypeExpr>) -> Result<TypeExpr, TypeNormalizationError> {
    if members.is_empty() {
        return Err(TypeNormalizationError::EmptyUnion);
    }

    let mut normalized = Vec::new();
    for member in members {
        match normalize_type_expr(member)? {
            TypeExpr::Union(nested) => normalized.extend(nested),
            member => normalized.push(member),
        }
    }
    normalized.sort_by_key(type_expr_sort_key);
    normalized.dedup();

    match normalized.len() {
        0 => Err(TypeNormalizationError::EmptyUnion),
        1 => Ok(normalized.pop().expect("single normalized union member")),
        _ => Ok(TypeExpr::Union(normalized)),
    }
}

fn type_expr_sort_key(value: &TypeExpr) -> String {
    match value {
        TypeExpr::Concrete(id) => format!("0:{}", id.as_str()),
        TypeExpr::Generic(id) => format!("1:{}", id.as_str()),
        TypeExpr::Applied {
            constructor,
            arguments,
        } => format!(
            "2:{}<{}>",
            constructor.as_str(),
            arguments
                .iter()
                .map(type_expr_sort_key)
                .collect::<Vec<_>>()
                .join(",")
        ),
        TypeExpr::Union(members) => format!(
            "3:{}",
            members
                .iter()
                .map(type_expr_sort_key)
                .collect::<Vec<_>>()
                .join("|")
        ),
        TypeExpr::Unknown => "4".to_string(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeTerm {
    Expr(TypeExpr),
    Port(PortKey),
    Parameter(ParameterKey),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeConstraint {
    Equal(TypeTerm, TypeTerm),
    Assignable(TypeTerm, TypeTerm),
    Implements(TypeTerm, TypeClassId),
    ElementOf(TypeTerm, TypeTerm),
    OneOf(TypeTerm, Vec<TypeTerm>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchemaExpr {
    Input(PortKey),
    Project {
        input: Box<SchemaExpr>,
        columns: ColumnSelectionExpr,
    },
    Append {
        inputs: Vec<SchemaExpr>,
    },
    Rename {
        input: Box<SchemaExpr>,
        mapping: RenameExpr,
    },
    Filter {
        input: Box<SchemaExpr>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        predicate: Option<ParameterKey>,
    },
    Derived {
        resolver: SchemaResolverId,
        dependencies: Vec<SchemaDependency>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnSelectionExpr {
    All,
    Explicit(Vec<SchemaColumnRef>),
    FromParameter(ParameterKey),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaColumnRef(pub Box<str>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelationalScalarType {
    Boolean,
    Int64,
    Float64,
    String,
    Date,
    DateTime,
    Unknown,
}

impl RelationalScalarType {
    pub fn from_database_dtype(value: &str) -> Self {
        match value.to_ascii_uppercase().as_str() {
            "BOOLEAN" => Self::Boolean,
            "TINYINT" | "SMALLINT" | "INTEGER" | "BIGINT" | "INT64" => Self::Int64,
            "FLOAT" | "DOUBLE" | "REAL" | "FLOAT64" => Self::Float64,
            "VARCHAR" | "TEXT" | "STRING" => Self::String,
            "DATE" => Self::Date,
            "TIMESTAMP" | "DATETIME" => Self::DateTime,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaFieldLineage {
    pub source: Box<str>,
    pub field: Box<str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaField {
    pub name: SchemaColumnRef,
    pub scalar_type: RelationalScalarType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lineage: Option<SchemaFieldLineage>,
}

impl From<SchemaColumnRef> for SchemaField {
    fn from(name: SchemaColumnRef) -> Self {
        Self {
            name,
            scalar_type: RelationalScalarType::Unknown,
            lineage: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedSchemaFact {
    pub expression: SchemaExpr,
    pub fields: Vec<SchemaField>,
}

impl ResolvedSchemaFact {
    pub fn new(
        expression: SchemaExpr,
        fields: impl IntoIterator<Item = impl Into<SchemaField>>,
    ) -> Self {
        Self {
            expression,
            fields: fields.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenameExpr {
    Explicit(Vec<ColumnRename>),
    FromParameter(ParameterKey),
    FromParameters {
        from: ParameterKey,
        to: ParameterKey,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnRename {
    pub from: SchemaColumnRef,
    pub to: SchemaColumnRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SchemaDependency {
    Port(PortKey),
    Parameter(ParameterKey),
    Interface(InterfaceResolverId),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_dtype_aliases_normalize_to_relational_scalar_types() {
        for (aliases, expected) in [
            (&["BOOLEAN"][..], RelationalScalarType::Boolean),
            (
                &["TINYINT", "SMALLINT", "INTEGER", "BIGINT", "INT64"][..],
                RelationalScalarType::Int64,
            ),
            (
                &["FLOAT", "DOUBLE", "REAL", "FLOAT64"][..],
                RelationalScalarType::Float64,
            ),
            (
                &["VARCHAR", "TEXT", "STRING"][..],
                RelationalScalarType::String,
            ),
            (&["DATE"][..], RelationalScalarType::Date),
            (
                &["TIMESTAMP", "DATETIME"][..],
                RelationalScalarType::DateTime,
            ),
        ] {
            for alias in aliases {
                assert_eq!(RelationalScalarType::from_database_dtype(alias), expected);
                assert_eq!(
                    RelationalScalarType::from_database_dtype(&alias.to_ascii_lowercase()),
                    expected
                );
            }
        }
        assert_eq!(
            RelationalScalarType::from_database_dtype("DECIMAL(18,2)"),
            RelationalScalarType::Unknown
        );
    }

    #[test]
    fn schema_field_lineage_is_optional_and_round_trips() {
        let legacy = SchemaField {
            name: SchemaColumnRef("amount".into()),
            scalar_type: RelationalScalarType::Float64,
            lineage: None,
        };
        assert_eq!(
            serde_json::to_value(&legacy).unwrap(),
            serde_json::json!({"name": "amount", "scalar_type": "Float64"})
        );

        let stable = SchemaField {
            name: SchemaColumnRef("amount".into()),
            scalar_type: RelationalScalarType::Float64,
            lineage: Some(SchemaFieldLineage {
                source: "databases/main".into(),
                field: "amount".into(),
            }),
        };
        assert_eq!(
            serde_json::from_value::<SchemaField>(serde_json::to_value(&stable).unwrap()).unwrap(),
            stable
        );
    }

    #[test]
    fn filter_schema_expression_preserves_legacy_wire_without_predicate() {
        let expression = SchemaExpr::Filter {
            input: Box::new(SchemaExpr::Input(PortKey::new("source").unwrap())),
            predicate: None,
        };
        let expected = serde_json::json!({
            "Filter": {
                "input": { "Input": "source" }
            }
        });

        assert_eq!(serde_json::to_value(&expression).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<SchemaExpr>(expected).unwrap(),
            expression
        );
    }

    #[test]
    fn filter_schema_expression_serializes_exact_predicate_parameter() {
        let expression = SchemaExpr::Filter {
            input: Box::new(SchemaExpr::Input(PortKey::new("source").unwrap())),
            predicate: Some(ParameterKey::new("predicate").unwrap()),
        };
        let expected = serde_json::json!({
            "Filter": {
                "input": { "Input": "source" },
                "predicate": "predicate"
            }
        });

        assert_eq!(serde_json::to_value(&expression).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<SchemaExpr>(expected).unwrap(),
            expression
        );
    }
}
