//! Canonical conversion between data-contract types and Graph type representations.

#![forbid(unsafe_code)]
#![deny(unused_must_use)]

use thiserror::Error;
use yss_data_contract::ValueType;
use yss_node_protocol::{
    DATA_SERIES_CONSTRUCTOR_ID, InvalidSemanticId, RelationalScalarType, ResolvedType,
    TypeConstructorId, TypeExpr, TypeId,
};

#[derive(Debug, Error)]
pub enum GraphTypeMappingError {
    #[error("graph type identifier is invalid: {0}")]
    InvalidGraphTypeIdentifier(#[from] InvalidSemanticId),
}

pub fn type_expr_from_data_type(data_type: &ValueType) -> Result<TypeExpr, GraphTypeMappingError> {
    match data_type {
        ValueType::Scalar(semantic) => concrete_type(semantic.type_id()),
        ValueType::Object => concrete_type("core.object"),
        ValueType::DataFrame => concrete_type("tabular.dataframe"),
        ValueType::Struct(semantic_id) => concrete_type(semantic_id),
        ValueType::Array(element) => applied_type("core.array", element),
        ValueType::DataSeries(element) => applied_type(DATA_SERIES_CONSTRUCTOR_ID, element),
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
        ResolvedType::Nominal(id) => Some(data_type_from_nominal(id.as_str())),
        ResolvedType::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == DATA_SERIES_CONSTRUCTOR_ID && arguments.len() == 1 => {
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

/// Concrete declarations convert without inferring classes, generics or unknown types.
pub fn data_type_from_type_expr(value: &TypeExpr) -> Option<ValueType> {
    match value {
        TypeExpr::Concrete(id) => Some(data_type_from_nominal(id.as_str())),
        TypeExpr::Applied {
            constructor,
            arguments,
        } if arguments.len() == 1 => match constructor.as_str() {
            DATA_SERIES_CONSTRUCTOR_ID => data_type_from_type_expr(&arguments[0])
                .map(|element| ValueType::DataSeries(Box::new(element))),
            "core.array" => data_type_from_type_expr(&arguments[0])
                .map(|element| ValueType::Array(Box::new(element))),
            _ => None,
        },
        TypeExpr::Union(values) if !values.is_empty() => values
            .iter()
            .map(data_type_from_type_expr)
            .collect::<Option<Vec<_>>>()
            .map(ValueType::one_of),
        TypeExpr::Class(_)
        | TypeExpr::Generic(_)
        | TypeExpr::Unknown
        | TypeExpr::Applied { .. }
        | TypeExpr::Union(_) => None,
    }
}

fn data_type_from_nominal(id: &str) -> ValueType {
    yss_data_contract::SemanticType::ALL
        .into_iter()
        .find(|semantic| semantic.type_id() == id)
        .map(ValueType::Scalar)
        .unwrap_or_else(|| match id {
            "core.object" => ValueType::Object,
            "tabular.dataframe" => ValueType::DataFrame,
            id => ValueType::Struct(id.to_owned()),
        })
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
        GraphTypeMappingError, data_type_from_type_expr, relational_scalar_type_from_data_type,
        type_expr_from_data_type,
    };
    use yss_data_contract::ValueType;
    use yss_node_protocol::{DATA_SERIES_CONSTRUCTOR_ID, TypeExpr};

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
                ValueType::Scalar(yss_data_contract::SemanticType::Text),
                "core.text",
            ),
            (
                ValueType::Scalar(yss_data_contract::SemanticType::Ordinal),
                "core.ordinal",
            ),
            (
                ValueType::Scalar(yss_data_contract::SemanticType::Identifier),
                "core.identifier",
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
    fn reports_invalid_graph_type_identifiers() {
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

    #[test]
    fn converts_declared_nominal_containers_and_normalizes_union_members() {
        for value in [
            ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            ValueType::Object,
            ValueType::DataFrame,
            ValueType::Struct("domain.record".into()),
            ValueType::Array(Box::new(ValueType::DataSeries(Box::new(
                ValueType::Scalar(yss_data_contract::SemanticType::Datetime),
            )))),
        ] {
            let declared = type_expr_from_data_type(&value).unwrap();
            assert_eq!(data_type_from_type_expr(&declared), Some(value));
        }
        let text = TypeExpr::Concrete("core.text".parse().unwrap());
        let number = TypeExpr::Concrete("core.numeric".parse().unwrap());
        let union = TypeExpr::Union(vec![
            text.clone(),
            TypeExpr::Union(vec![number, text]),
            TypeExpr::Concrete("core.object".parse().unwrap()),
        ]);
        assert_eq!(
            data_type_from_type_expr(&union),
            Some(ValueType::OneOf(vec![
                ValueType::Scalar(yss_data_contract::SemanticType::Text),
                ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
                ValueType::Object,
            ]))
        );
    }

    #[test]
    fn unresolved_and_unsupported_declarations_do_not_become_value_types() {
        let number = TypeExpr::Concrete("core.numeric".parse().unwrap());
        for declared in [
            TypeExpr::Class("core.numeric".parse().unwrap()),
            TypeExpr::Generic("element".parse().unwrap()),
            TypeExpr::Unknown,
            TypeExpr::Union(vec![]),
            TypeExpr::Union(vec![number.clone(), TypeExpr::Unknown]),
            TypeExpr::Applied {
                constructor: "core.array".parse().unwrap(),
                arguments: vec![],
            },
            TypeExpr::Applied {
                constructor: DATA_SERIES_CONSTRUCTOR_ID.parse().unwrap(),
                arguments: vec![number.clone(), number.clone()],
            },
            TypeExpr::Applied {
                constructor: "domain.container".parse().unwrap(),
                arguments: vec![number],
            },
        ] {
            assert_eq!(data_type_from_type_expr(&declared), None);
        }
    }
}
