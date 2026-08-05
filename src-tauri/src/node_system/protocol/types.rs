use super::{
    InterfaceResolverId, ParameterKey, PortKey, SchemaResolverId, TypeClassId, TypeConstructorId,
    TypeId, TypeParameterId,
};
use serde::{Deserialize, Serialize};

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
pub struct SchemaField {
    pub name: SchemaColumnRef,
    pub scalar_type: RelationalScalarType,
}

impl From<SchemaColumnRef> for SchemaField {
    fn from(name: SchemaColumnRef) -> Self {
        Self {
            name,
            scalar_type: RelationalScalarType::Unknown,
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
