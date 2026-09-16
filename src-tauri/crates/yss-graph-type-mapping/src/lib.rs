//! Canonical conversion between data-contract types and Graph type representations.

#![forbid(unsafe_code)]
#![deny(unused_must_use)]

use thiserror::Error;
use yss_data_contract::{ValueType, ValueTypeParseError};
use yss_node_protocol::{
    InvalidSemanticId, RelationalScalarType, ResolvedType, TypeConstructorId, TypeExpr, TypeId,
};

#[derive(Debug, Error)]
pub enum GraphTypeMappingError {
    #[error("data type name is invalid: {0}")]
    InvalidDataTypeName(#[from] ValueTypeParseError),
    #[error("graph type identifier is invalid: {0}")]
    InvalidGraphTypeIdentifier(#[from] InvalidSemanticId),
}

pub fn type_expr_from_data_type_name(type_name: &str) -> Result<TypeExpr, GraphTypeMappingError> {
    type_expr_from_data_type(&type_name.parse::<ValueType>()?)
}

pub fn type_expr_from_data_type(data_type: &ValueType) -> Result<TypeExpr, GraphTypeMappingError> {
    match data_type {
        ValueType::Scalar(semantic) => concrete_type(semantic.type_id()),
        ValueType::Object => concrete_type("core.object"),
        ValueType::DataFrame => concrete_type("tabular.dataframe"),
        ValueType::Struct(semantic_id) => concrete_type(semantic_id),
        ValueType::Array(element) => applied_type("core.array", element),
        ValueType::DataSeries(element) => applied_type("core.data_series", element),
        ValueType::OneOf(values) => values
            .iter()
            .map(type_expr_from_data_type)
            .collect::<Result<Vec<_>, _>>()
            .map(TypeExpr::Union),
        ValueType::Any => Ok(TypeExpr::Unknown),
    }
}

pub fn relational_scalar_type_from_data_type(data_type: &ValueType) -> RelationalScalarType {
    match data_type {
        ValueType::Scalar(semantic) => RelationalScalarType::Known(*semantic),
        _ => RelationalScalarType::Unknown,
    }
}

pub fn data_type_from_resolved_type(value: &ResolvedType) -> Option<ValueType> {
    match value {
        ResolvedType::Nominal(id) => Some(
            yss_data_contract::SemanticType::ALL
                .into_iter()
                .find(|semantic| semantic.type_id() == id.as_str())
                .map(ValueType::Scalar)
                .unwrap_or_else(|| match id.as_str() {
                    "core.object" => ValueType::Object,
                    "tabular.dataframe" => ValueType::DataFrame,
                    id => ValueType::Struct(id.to_owned()),
                }),
        ),
        ResolvedType::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == "core.data_series" && arguments.len() == 1 => {
            data_type_from_resolved_type(&arguments[0])
                .map(|element| ValueType::DataSeries(Box::new(element)))
        }
        ResolvedType::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == "core.array" && arguments.len() == 1 => {
            data_type_from_resolved_type(&arguments[0])
                .map(|element| ValueType::Array(Box::new(element)))
        }
        ResolvedType::Applied { .. } => None,
    }
}

fn concrete_type(semantic_id: &str) -> Result<TypeExpr, GraphTypeMappingError> {
    TypeId::new(semantic_id)
        .map(TypeExpr::Concrete)
        .map_err(Into::into)
}

fn applied_type(constructor: &str, element: &ValueType) -> Result<TypeExpr, GraphTypeMappingError> {
    Ok(TypeExpr::Applied {
        constructor: TypeConstructorId::new(constructor)?,
        arguments: vec![type_expr_from_data_type(element)?],
    })
}

#[cfg(test)]
mod tests {
    use super::{
        GraphTypeMappingError, relational_scalar_type_from_data_type, type_expr_from_data_type,
        type_expr_from_data_type_name,
    };
    use yss_data_contract::ValueType;
    use yss_node_protocol::TypeExpr;

    #[test]
    fn maps_scalar_composite_union_and_unknown_types() {
        for (data_type, semantic_id) in [
            (
                ValueType::Scalar(yss_data_contract::SemanticType::Binary),
                "core.binary",
            ),
            (
                ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
                "core.numeric",
            ),
            (
                ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
                "core.numeric",
            ),
            (
                ValueType::Scalar(yss_data_contract::SemanticType::Text),
                "core.text",
            ),
            (
                ValueType::Scalar(yss_data_contract::SemanticType::Datetime),
                "core.datetime",
            ),
            (
                ValueType::Scalar(yss_data_contract::SemanticType::Datetime),
                "core.datetime",
            ),
            (
                ValueType::Scalar(yss_data_contract::SemanticType::Datetime),
                "core.datetime",
            ),
            (
                ValueType::Scalar(yss_data_contract::SemanticType::Categorical),
                "core.categorical",
            ),
            (ValueType::Object, "core.object"),
            (ValueType::DataFrame, "tabular.dataframe"),
            (ValueType::Struct("domain.record".into()), "domain.record"),
        ] {
            assert_eq!(
                type_expr_from_data_type(&data_type).unwrap(),
                TypeExpr::Concrete(semantic_id.parse().unwrap()),
                "unexpected mapping for {data_type}"
            );
        }
        assert_eq!(
            type_expr_from_data_type(&ValueType::Array(Box::new(ValueType::Scalar(
                yss_data_contract::SemanticType::Numeric
            ))))
            .unwrap(),
            TypeExpr::Applied {
                constructor: "core.array".parse().unwrap(),
                arguments: vec![TypeExpr::Concrete("core.numeric".parse().unwrap())],
            }
        );
        assert_eq!(
            type_expr_from_data_type(&ValueType::DataSeries(Box::new(ValueType::Scalar(
                yss_data_contract::SemanticType::Numeric
            ))))
            .unwrap(),
            TypeExpr::Applied {
                constructor: "core.data_series".parse().unwrap(),
                arguments: vec![TypeExpr::Concrete("core.numeric".parse().unwrap())],
            }
        );
        assert_eq!(
            type_expr_from_data_type(&ValueType::OneOf(vec![
                ValueType::Scalar(yss_data_contract::SemanticType::Text),
                ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            ]))
            .unwrap(),
            TypeExpr::Union(vec![
                TypeExpr::Concrete("core.text".parse().unwrap()),
                TypeExpr::Concrete("core.numeric".parse().unwrap()),
            ])
        );
        assert_eq!(
            type_expr_from_data_type(&ValueType::Any).unwrap(),
            TypeExpr::Unknown
        );
    }

    #[test]
    fn parses_names_and_reports_typed_failures() {
        assert_eq!(
            type_expr_from_data_type_name("DataSeries<Numeric>").unwrap(),
            TypeExpr::Applied {
                constructor: "core.data_series".parse().unwrap(),
                arguments: vec![TypeExpr::Concrete("core.numeric".parse().unwrap())],
            }
        );
        assert!(matches!(
            type_expr_from_data_type_name("not-a-data-type"),
            Err(GraphTypeMappingError::InvalidDataTypeName(_))
        ));
        assert_eq!(
            type_expr_from_data_type_name("not-a-data-type")
                .unwrap_err()
                .to_string(),
            "data type name is invalid: unknown data type"
        );
        assert!(matches!(
            type_expr_from_data_type(&ValueType::Struct("Invalid Type".into())),
            Err(GraphTypeMappingError::InvalidGraphTypeIdentifier(_))
        ));
    }

    #[test]
    fn maps_persisted_types_to_relational_schema_scalars() {
        assert_eq!(
            relational_scalar_type_from_data_type(&ValueType::Scalar(
                yss_data_contract::SemanticType::Datetime
            )),
            yss_node_protocol::RelationalScalarType::Known(
                yss_node_protocol::SemanticType::Datetime
            )
        );
        assert_eq!(
            relational_scalar_type_from_data_type(&ValueType::DataFrame),
            yss_node_protocol::RelationalScalarType::Unknown
        );
    }
}
