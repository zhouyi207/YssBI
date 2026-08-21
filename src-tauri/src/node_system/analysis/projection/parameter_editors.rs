use super::relational_scalar_type_dto;
use super::types::{
    DataframeColumnOptionDto, FilterColumnOptionDto, FilterLiteralTypeDto, ParameterEditorKindDto,
    SchemaAwareParameterEditorDto,
};
use crate::node_system::protocol::{ParameterEditorSpec, RelationalScalarType, ResolvedSchemaFact};

pub(super) fn inherited_statistics_parameter_value(
    node_type_id: &str,
    key: &str,
    settings: &crate::project::ProjectComputationSettings,
) -> Option<serde_json::Value> {
    if !node_type_id.starts_with("yssbi.statistics.") {
        return None;
    }
    match key {
        "convergence_tolerance" => {
            serde_json::Number::from_f64(settings.numeric.tolerance.absolute)
                .map(serde_json::Value::Number)
        }
        "missing_value_policy" => Some(serde_json::Value::String(
            match settings.missing_values.statistics {
                crate::project::StatisticalMissingValuePolicy::Listwise => "Listwise",
                crate::project::StatisticalMissingValuePolicy::Reject => "Reject",
            }
            .to_owned(),
        )),
        _ => None,
    }
}

pub(super) fn statistics_parameter_options(node_type_id: &str, key: &str) -> Option<Vec<Box<str>>> {
    (node_type_id.starts_with("yssbi.statistics.") && key == "missing_value_policy")
        .then(|| vec!["Listwise".into(), "Reject".into()])
}
pub(super) fn project_schema_aware_editor(
    node_type_id: &str,
    value: Option<&serde_json::Value>,
    source_schema: Option<&ResolvedSchemaFact>,
    unavailable_reason: Box<str>,
) -> Option<SchemaAwareParameterEditorDto> {
    use crate::node_system::protocol::dataframe::{FilterPredicate, ProjectColumns};

    let available = source_schema.is_some();
    let unavailable_reason = (!available).then_some(unavailable_reason);
    match node_type_id {
        "yssbi.dataframe.project" => Some(SchemaAwareParameterEditorDto::ProjectColumns {
            available,
            unavailable_reason,
            options: source_schema
                .into_iter()
                .flat_map(|fact| fact.fields.iter())
                .map(project_dataframe_column_option)
                .collect(),
            value: value
                .and_then(|value| serde_json::from_value::<ProjectColumns>(value.clone()).ok())
                .map(|columns| columns.as_slice().to_vec())
                .unwrap_or_default(),
        }),
        "yssbi.dataframe.filter.rows" => Some(SchemaAwareParameterEditorDto::FilterPredicate {
            available,
            unavailable_reason,
            columns: source_schema
                .into_iter()
                .flat_map(|fact| fact.fields.iter())
                .map(|field| FilterColumnOptionDto {
                    name: field.name.0.clone(),
                    data_type: relational_scalar_type_dto(field.scalar_type),
                    operators: filter_operators(field.scalar_type),
                    literal_types: filter_literal_types(field.scalar_type),
                })
                .collect(),
            value: value
                .and_then(|value| serde_json::from_value::<FilterPredicate>(value.clone()).ok())
                .and_then(|predicate| serde_json::to_value(predicate).ok()),
        }),
        _ => return None,
    }
}

fn project_dataframe_column_option(
    field: &crate::node_system::protocol::SchemaField,
) -> DataframeColumnOptionDto {
    DataframeColumnOptionDto {
        name: field.name.0.clone(),
        data_type: relational_scalar_type_dto(field.scalar_type),
    }
}
fn filter_literal_types(scalar_type: RelationalScalarType) -> Vec<FilterLiteralTypeDto> {
    match scalar_type {
        RelationalScalarType::Boolean => vec![FilterLiteralTypeDto::Boolean],
        RelationalScalarType::Int64 => vec![FilterLiteralTypeDto::Integer],
        RelationalScalarType::Float64 => {
            vec![FilterLiteralTypeDto::Integer, FilterLiteralTypeDto::Decimal]
        }
        RelationalScalarType::String => vec![FilterLiteralTypeDto::String],
        RelationalScalarType::Date
        | RelationalScalarType::DateTime
        | RelationalScalarType::Unknown => vec![],
    }
}

fn filter_operators(
    scalar_type: RelationalScalarType,
) -> Vec<crate::node_system::protocol::dataframe::FilterOperator> {
    use crate::node_system::protocol::dataframe::FilterOperator::*;
    match scalar_type {
        RelationalScalarType::Boolean => vec![Equal, NotEqual, IsNull, IsNotNull],
        RelationalScalarType::Int64
        | RelationalScalarType::Float64
        | RelationalScalarType::String => vec![
            Equal,
            NotEqual,
            LessThan,
            LessThanOrEqual,
            GreaterThan,
            GreaterThanOrEqual,
            IsNull,
            IsNotNull,
        ],
        RelationalScalarType::Date | RelationalScalarType::DateTime => {
            vec![IsNull, IsNotNull]
        }
        RelationalScalarType::Unknown => vec![],
    }
}

pub(super) fn project_parameter_editor(
    editor: &ParameterEditorSpec,
) -> Option<(ParameterEditorKindDto, bool)> {
    Some(match editor {
        ParameterEditorSpec::Auto => (ParameterEditorKindDto::Auto, false),
        ParameterEditorSpec::Hidden => return None,
        ParameterEditorSpec::Text { multiline } => (ParameterEditorKindDto::Text, *multiline),
        ParameterEditorSpec::Number => (ParameterEditorKindDto::Number, false),
        ParameterEditorSpec::Toggle => (ParameterEditorKindDto::Toggle, false),
        ParameterEditorSpec::Select => (ParameterEditorKindDto::Select, false),
        ParameterEditorSpec::Resource { .. } => (ParameterEditorKindDto::Resource, false),
    })
}
