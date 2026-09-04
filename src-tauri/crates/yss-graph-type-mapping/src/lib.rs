//! Canonical conversion between data-contract types and Graph type representations.

#![forbid(unsafe_code)]
#![deny(unused_must_use)]

use thiserror::Error;
use yss_data_contract::{DataType, DataTypeParseError};
use yss_graph_protocol::{
    InvalidSemanticId, RelationalScalarType, ResolvedType, TypeConstructorId, TypeExpr, TypeId,
};

#[derive(Debug, Error)]
pub enum GraphTypeMappingError {
    #[error("data type name is invalid: {0}")]
    InvalidDataTypeName(#[from] DataTypeParseError),
    #[error("graph type identifier is invalid: {0}")]
    InvalidGraphTypeIdentifier(#[from] InvalidSemanticId),
}

pub fn type_expr_from_data_type_name(type_name: &str) -> Result<TypeExpr, GraphTypeMappingError> {
    type_expr_from_data_type(&type_name.parse::<DataType>()?)
}

pub fn type_expr_from_data_type(data_type: &DataType) -> Result<TypeExpr, GraphTypeMappingError> {
    match data_type {
        DataType::Boolean => concrete_type("core.bool"),
        DataType::Int64 => concrete_type("core.int64"),
        DataType::Float64 => concrete_type("core.float64"),
        DataType::String => concrete_type("core.string"),
        DataType::Date => concrete_type("core.date"),
        DataType::Datetime => concrete_type("core.datetime"),
        DataType::Time => concrete_type("core.time"),
        DataType::Categorical => concrete_type("core.categorical"),
        DataType::Object => concrete_type("core.object"),
        DataType::DataFrame => concrete_type("tabular.dataframe"),
        DataType::Struct(semantic_id) => concrete_type(semantic_id),
        DataType::Array(element) => applied_type("core.array", element),
        DataType::DataSeries(element) => applied_type("core.data_series", element),
        DataType::OneOf(values) => values
            .iter()
            .map(type_expr_from_data_type)
            .collect::<Result<Vec<_>, _>>()
            .map(TypeExpr::Union),
        DataType::Any => Ok(TypeExpr::Unknown),
    }
}

pub fn relational_scalar_type_from_data_type(data_type: &DataType) -> RelationalScalarType {
    match data_type {
        DataType::Boolean => RelationalScalarType::Boolean,
        DataType::Int64 => RelationalScalarType::Int64,
        DataType::Float64 => RelationalScalarType::Float64,
        DataType::String | DataType::Categorical => RelationalScalarType::String,
        DataType::Date => RelationalScalarType::Date,
        DataType::Datetime => RelationalScalarType::DateTime,
        DataType::Time
        | DataType::Array(_)
        | DataType::Object
        | DataType::DataFrame
        | DataType::DataSeries(_)
        | DataType::Struct(_)
        | DataType::OneOf(_)
        | DataType::Any => RelationalScalarType::Unknown,
    }
}

pub fn data_type_from_resolved_type(value: &ResolvedType) -> Option<DataType> {
    match value {
        ResolvedType::Nominal(id) => Some(match id.as_str() {
            "core.bool" => DataType::Boolean,
            "core.int64" => DataType::Int64,
            "core.float64" => DataType::Float64,
            "core.string" => DataType::String,
            "core.date" => DataType::Date,
            "core.datetime" => DataType::Datetime,
            "core.time" => DataType::Time,
            "core.categorical" => DataType::Categorical,
            "core.object" => DataType::Object,
            "tabular.dataframe" => DataType::DataFrame,
            semantic_id => DataType::Struct(semantic_id.to_owned()),
        }),
        ResolvedType::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == "core.data_series" && arguments.len() == 1 => {
            data_type_from_resolved_type(&arguments[0])
                .map(|element| DataType::DataSeries(Box::new(element)))
        }
        ResolvedType::Applied {
            constructor,
            arguments,
        } if constructor.as_str() == "core.array" && arguments.len() == 1 => {
            data_type_from_resolved_type(&arguments[0])
                .map(|element| DataType::Array(Box::new(element)))
        }
        ResolvedType::Applied { .. } => None,
    }
}

fn concrete_type(semantic_id: &str) -> Result<TypeExpr, GraphTypeMappingError> {
    TypeId::new(semantic_id)
        .map(TypeExpr::Concrete)
        .map_err(Into::into)
}

fn applied_type(constructor: &str, element: &DataType) -> Result<TypeExpr, GraphTypeMappingError> {
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
    use yss_data_contract::DataType;
    use yss_graph_protocol::TypeExpr;

    #[test]
    fn maps_scalar_composite_union_and_unknown_types() {
        for (data_type, semantic_id) in [
            (DataType::Boolean, "core.bool"),
            (DataType::Int64, "core.int64"),
            (DataType::Float64, "core.float64"),
            (DataType::String, "core.string"),
            (DataType::Date, "core.date"),
            (DataType::Datetime, "core.datetime"),
            (DataType::Time, "core.time"),
            (DataType::Categorical, "core.categorical"),
            (DataType::Object, "core.object"),
            (DataType::DataFrame, "tabular.dataframe"),
            (DataType::Struct("domain.record".into()), "domain.record"),
        ] {
            assert_eq!(
                type_expr_from_data_type(&data_type).unwrap(),
                TypeExpr::Concrete(semantic_id.parse().unwrap()),
                "unexpected mapping for {data_type}"
            );
        }
        assert_eq!(
            type_expr_from_data_type(&DataType::Array(Box::new(DataType::Int64))).unwrap(),
            TypeExpr::Applied {
                constructor: "core.array".parse().unwrap(),
                arguments: vec![TypeExpr::Concrete("core.int64".parse().unwrap())],
            }
        );
        assert_eq!(
            type_expr_from_data_type(&DataType::DataSeries(Box::new(DataType::Float64))).unwrap(),
            TypeExpr::Applied {
                constructor: "core.data_series".parse().unwrap(),
                arguments: vec![TypeExpr::Concrete("core.float64".parse().unwrap())],
            }
        );
        assert_eq!(
            type_expr_from_data_type(&DataType::OneOf(vec![DataType::String, DataType::Float64,]))
                .unwrap(),
            TypeExpr::Union(vec![
                TypeExpr::Concrete("core.string".parse().unwrap()),
                TypeExpr::Concrete("core.float64".parse().unwrap()),
            ])
        );
        assert_eq!(
            type_expr_from_data_type(&DataType::Any).unwrap(),
            TypeExpr::Unknown
        );
    }

    #[test]
    fn parses_names_and_reports_typed_failures() {
        assert_eq!(
            type_expr_from_data_type_name("DataSeries<Float64>").unwrap(),
            TypeExpr::Applied {
                constructor: "core.data_series".parse().unwrap(),
                arguments: vec![TypeExpr::Concrete("core.float64".parse().unwrap())],
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
            type_expr_from_data_type(&DataType::Struct("Invalid Type".into())),
            Err(GraphTypeMappingError::InvalidGraphTypeIdentifier(_))
        ));
    }

    #[test]
    fn maps_persisted_types_to_relational_schema_scalars() {
        assert_eq!(
            relational_scalar_type_from_data_type(&DataType::Datetime),
            yss_graph_protocol::RelationalScalarType::DateTime
        );
        assert_eq!(
            relational_scalar_type_from_data_type(&DataType::DataFrame),
            yss_graph_protocol::RelationalScalarType::Unknown
        );
    }
}
