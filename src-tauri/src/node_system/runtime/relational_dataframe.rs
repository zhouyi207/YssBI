use super::{Artifact, RelationalError, RelationalErrorCode, RuntimeValue};
use crate::node_system::plan::{
    RelationalExpression, RelationalLiteral, RelationalProjection, RelationalRename,
};
use crate::node_system::protocol::Value;
use polars::prelude::{BooleanChunked, Column, DataFrame, DataType, NamedFrom};
use std::collections::BTreeSet;

pub(super) fn tabular_runtime_to_dataframe(
    value: RuntimeValue,
) -> Result<DataFrame, RelationalError> {
    let value = match value {
        RuntimeValue::Scalar(value) => value,
        RuntimeValue::Artifact(artifact) => single_artifact_value(artifact)?,
        RuntimeValue::Stream(_) => {
            return Err(input_shape(
                "relational input stream must be collected before ingress",
            ));
        }
    };
    let Value::Object(columns) = value else {
        return Err(input_shape("relational input must be a dataframe object"));
    };
    let mut expected_height = None;
    let mut dataframe_columns = Vec::with_capacity(columns.len());
    for (name, value) in columns {
        let Value::List(values) = value else {
            return Err(input_shape("relational dataframe columns must be lists"));
        };
        if expected_height
            .replace(values.len())
            .is_some_and(|height| height != values.len())
        {
            return Err(input_shape(
                "relational dataframe columns must have equal lengths",
            ));
        }
        dataframe_columns.push(protocol_column(name.as_ref(), &values)?);
    }
    let height = expected_height.unwrap_or(0);
    DataFrame::new(height, dataframe_columns)
        .map_err(|_| input_shape("relational dataframe shape is invalid"))
}

fn single_artifact_value(artifact: Artifact) -> Result<Value, RelationalError> {
    let mut values = artifact
        .cursor()
        .map_err(|error| input_shape(error.to_string()))?;
    let Some(value) = values
        .next()
        .transpose()
        .map_err(|error| input_shape(error.to_string()))?
    else {
        return Err(input_shape(
            "relational dataframe artifact must contain exactly one value",
        ));
    };
    if values
        .next()
        .transpose()
        .map_err(|error| input_shape(error.to_string()))?
        .is_some()
    {
        return Err(input_shape(
            "relational dataframe artifact must contain exactly one value",
        ));
    }
    Ok(value)
}

fn protocol_column(name: &str, values: &[Value]) -> Result<Column, RelationalError> {
    let has_decimal = values
        .iter()
        .any(|value| matches!(value, Value::Decimal(_)));
    let kind = values.iter().find(|value| !matches!(value, Value::Null));
    let name = name.into();
    match kind {
        None => Ok(Column::new(
            name,
            vec![Option::<String>::None; values.len()],
        )),
        Some(Value::Integer(_)) if !has_decimal => values
            .iter()
            .map(|value| match value {
                Value::Integer(value) => Ok(Some(*value)),
                Value::Null => Ok(None),
                _ => Err(type_mismatch("relational column mixes incompatible values")),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|values| Column::new(name, values)),
        Some(Value::Integer(_)) | Some(Value::Decimal(_)) => values
            .iter()
            .map(protocol_float)
            .collect::<Result<Vec<_>, _>>()
            .map(|values| Column::new(name, values)),
        Some(Value::String(_)) => values
            .iter()
            .map(|value| match value {
                Value::String(value) => Ok(Some(value.as_ref())),
                Value::Null => Ok(None),
                _ => Err(type_mismatch("relational column mixes incompatible values")),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|values| Column::new(name, values)),
        Some(Value::Bool(_)) => values
            .iter()
            .map(|value| match value {
                Value::Bool(value) => Ok(Some(*value)),
                Value::Null => Ok(None),
                _ => Err(type_mismatch("relational column mixes incompatible values")),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(|values| Column::new(name, values)),
        Some(_) => Err(type_mismatch(
            "relational column contains an unsupported value type",
        )),
    }
}

fn protocol_float(value: &Value) -> Result<Option<f64>, RelationalError> {
    let value = match value {
        Value::Null => return Ok(None),
        Value::Integer(value) => exact_i64_as_f64(*value)
            .ok_or_else(|| type_mismatch("integer cannot be represented exactly as float64"))?,
        Value::Decimal(value) => value
            .as_str()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .ok_or_else(|| type_mismatch("decimal cannot be represented as float64"))?,
        _ => return Err(type_mismatch("relational column mixes incompatible values")),
    };
    Ok(Some(value))
}

pub(super) fn project_dataframe(
    dataframe: DataFrame,
    projections: &[RelationalProjection],
) -> Result<DataFrame, RelationalError> {
    let mut names = BTreeSet::new();
    let mut sources = BTreeSet::new();
    for projection in projections {
        if !names.insert(projection.name.as_ref()) {
            return Err(operator_invalid("project output columns must be unique"));
        }
        let RelationalExpression::Column(source) = &projection.expression else {
            return Err(operator_invalid(
                "project supports only direct column expressions",
            ));
        };
        if !sources.insert(source.as_ref()) {
            return Err(operator_invalid("project source columns must be unique"));
        }
    }
    let mut columns = Vec::with_capacity(projections.len());
    for projection in projections {
        let RelationalExpression::Column(source) = &projection.expression else {
            unreachable!("project expressions were validated")
        };
        if projection.name != *source {
            return Err(operator_invalid("project aliases are not supported"));
        }
        let column = dataframe.column(source.as_ref()).map_err(|_| {
            RelationalError::new(
                RelationalErrorCode::ColumnMissing,
                format!("project column '{source}' was not found"),
            )
        })?;
        columns.push(Column::from(
            column
                .as_materialized_series()
                .clone()
                .with_name(projection.name.as_ref().into()),
        ));
    }
    DataFrame::new(dataframe.height(), columns)
        .map_err(|_| operator_invalid("project dataframe construction failed"))
}

pub(super) fn filter_dataframe(
    dataframe: DataFrame,
    predicate: &RelationalExpression,
) -> Result<DataFrame, RelationalError> {
    let mask = predicate_mask(&dataframe, predicate)?;
    let mask = BooleanChunked::new("predicate".into(), mask);
    dataframe
        .filter(&mask)
        .map_err(|_| operator_invalid("filter dataframe evaluation failed"))
}

fn predicate_mask(
    dataframe: &DataFrame,
    expression: &RelationalExpression,
) -> Result<Vec<Option<bool>>, RelationalError> {
    match expression {
        RelationalExpression::Literal(RelationalLiteral::Boolean(value)) => {
            Ok(vec![Some(*value); dataframe.height()])
        }
        RelationalExpression::Column(name) => {
            let column = dataframe
                .column(name.as_ref())
                .map_err(|_| column_missing(name))?;
            let values = column
                .bool()
                .map_err(|_| type_mismatch("filter predicate column must be boolean"))?;
            Ok(values.into_iter().collect())
        }
        RelationalExpression::Equal(left, right) => {
            compare_mask(dataframe, left, right, Comparison::Equal)
        }
        RelationalExpression::NotEqual(left, right) => {
            compare_mask(dataframe, left, right, Comparison::NotEqual)
        }
        RelationalExpression::LessThan(left, right) => {
            compare_mask(dataframe, left, right, Comparison::LessThan)
        }
        RelationalExpression::LessThanOrEqual(left, right) => {
            compare_mask(dataframe, left, right, Comparison::LessThanOrEqual)
        }
        RelationalExpression::GreaterThan(left, right) => {
            compare_mask(dataframe, left, right, Comparison::GreaterThan)
        }
        RelationalExpression::GreaterThanOrEqual(left, right) => {
            compare_mask(dataframe, left, right, Comparison::GreaterThanOrEqual)
        }
        RelationalExpression::And(expressions) | RelationalExpression::Or(expressions) => {
            let and = matches!(expression, RelationalExpression::And(_));
            let mut result = vec![Some(and); dataframe.height()];
            for expression in expressions {
                let right = predicate_mask(dataframe, expression)?;
                for (left, right) in result.iter_mut().zip(right) {
                    *left = if and {
                        sql_and(*left, right)
                    } else {
                        sql_or(*left, right)
                    };
                }
            }
            Ok(result)
        }
        RelationalExpression::Not(expression) => Ok(predicate_mask(dataframe, expression)?
            .into_iter()
            .map(|value| value.map(|value| !value))
            .collect()),
        RelationalExpression::IsNull(expression) => is_null_mask(dataframe, expression),
        RelationalExpression::Literal(_) => Err(type_mismatch(
            "filter predicate expression must evaluate to boolean",
        )),
    }
}

#[derive(Clone, Copy)]
enum Comparison {
    Equal,
    NotEqual,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

fn compare_mask(
    dataframe: &DataFrame,
    left: &RelationalExpression,
    right: &RelationalExpression,
    comparison: Comparison,
) -> Result<Vec<Option<bool>>, RelationalError> {
    match (left, right) {
        (RelationalExpression::Column(column), RelationalExpression::Literal(literal)) => {
            compare_column_literal(dataframe, column, literal, comparison)
        }
        (RelationalExpression::Literal(literal), RelationalExpression::Column(column)) => {
            compare_column_literal(dataframe, column, literal, reverse(comparison))
        }
        _ => Err(operator_invalid(
            "filter comparison requires one column and one literal",
        )),
    }
}

fn compare_column_literal(
    dataframe: &DataFrame,
    name: &str,
    literal: &RelationalLiteral,
    comparison: Comparison,
) -> Result<Vec<Option<bool>>, RelationalError> {
    let column = dataframe.column(name).map_err(|_| column_missing(name))?;
    match column.dtype() {
        DataType::Int64 => {
            let RelationalLiteral::Integer(right) = literal else {
                return null_or_type_mismatch(literal, dataframe.height());
            };
            Ok(column
                .i64()
                .expect("dtype checked")
                .into_iter()
                .map(|left| left.map(|left| compare(left, *right, comparison)))
                .collect())
        }
        DataType::Float64 => {
            let right = float_literal(literal)?;
            Ok(column
                .f64()
                .expect("dtype checked")
                .into_iter()
                .map(|left| left.map(|left| compare_float(left, right, comparison)))
                .collect())
        }
        DataType::String => {
            let RelationalLiteral::String(right) = literal else {
                return null_or_type_mismatch(literal, dataframe.height());
            };
            Ok(column
                .str()
                .expect("dtype checked")
                .into_iter()
                .map(|left| left.map(|left| compare(left, right.as_ref(), comparison)))
                .collect())
        }
        DataType::Boolean => {
            let RelationalLiteral::Boolean(right) = literal else {
                return null_or_type_mismatch(literal, dataframe.height());
            };
            if !matches!(comparison, Comparison::Equal | Comparison::NotEqual) {
                return Err(type_mismatch(
                    "boolean filter comparisons support only equality operators",
                ));
            }
            Ok(column
                .bool()
                .expect("dtype checked")
                .into_iter()
                .map(|left| left.map(|left| compare(left, *right, comparison)))
                .collect())
        }
        _ => Err(type_mismatch(
            "filter comparison does not support the column dtype",
        )),
    }
}

fn null_or_type_mismatch(
    literal: &RelationalLiteral,
    height: usize,
) -> Result<Vec<Option<bool>>, RelationalError> {
    if matches!(literal, RelationalLiteral::Null) {
        Ok(vec![None; height])
    } else {
        Err(type_mismatch(
            "filter literal does not match the column dtype",
        ))
    }
}

fn float_literal(literal: &RelationalLiteral) -> Result<f64, RelationalError> {
    match literal {
        RelationalLiteral::Integer(value) => exact_i64_as_f64(*value).ok_or_else(|| {
            type_mismatch("integer literal cannot be represented exactly as float64")
        }),
        RelationalLiteral::Decimal(value) => value
            .as_str()
            .parse::<f64>()
            .ok()
            .filter(|value| value.is_finite())
            .ok_or_else(|| type_mismatch("decimal literal cannot be represented as float64")),
        RelationalLiteral::Null => Err(type_mismatch(
            "null comparison must use an explicit null operator",
        )),
        _ => Err(type_mismatch(
            "filter literal does not match the column dtype",
        )),
    }
}

fn exact_i64_as_f64(value: i64) -> Option<f64> {
    let converted = value as f64;
    ((converted as i128) == i128::from(value)).then_some(converted)
}

fn compare<T: PartialOrd + PartialEq>(left: T, right: T, comparison: Comparison) -> bool {
    match comparison {
        Comparison::Equal => left == right,
        Comparison::NotEqual => left != right,
        Comparison::LessThan => left < right,
        Comparison::LessThanOrEqual => left <= right,
        Comparison::GreaterThan => left > right,
        Comparison::GreaterThanOrEqual => left >= right,
    }
}

fn compare_float(left: f64, right: f64, comparison: Comparison) -> bool {
    compare(left, right, comparison)
}

fn reverse(comparison: Comparison) -> Comparison {
    match comparison {
        Comparison::LessThan => Comparison::GreaterThan,
        Comparison::LessThanOrEqual => Comparison::GreaterThanOrEqual,
        Comparison::GreaterThan => Comparison::LessThan,
        Comparison::GreaterThanOrEqual => Comparison::LessThanOrEqual,
        comparison => comparison,
    }
}

fn is_null_mask(
    dataframe: &DataFrame,
    expression: &RelationalExpression,
) -> Result<Vec<Option<bool>>, RelationalError> {
    match expression {
        RelationalExpression::Column(name) => {
            let column = dataframe
                .column(name.as_ref())
                .map_err(|_| column_missing(name))?;
            Ok(column.is_null().into_iter().collect())
        }
        RelationalExpression::Literal(RelationalLiteral::Null) => {
            Ok(vec![Some(true); dataframe.height()])
        }
        RelationalExpression::Literal(_) => Ok(vec![Some(false); dataframe.height()]),
        _ => Err(operator_invalid(
            "is-null requires a column or literal expression",
        )),
    }
}

fn sql_and(left: Option<bool>, right: Option<bool>) -> Option<bool> {
    match (left, right) {
        (Some(false), _) | (_, Some(false)) => Some(false),
        (Some(true), Some(true)) => Some(true),
        _ => None,
    }
}

fn sql_or(left: Option<bool>, right: Option<bool>) -> Option<bool> {
    match (left, right) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (Some(false), Some(false)) => Some(false),
        _ => None,
    }
}

pub(super) fn rename_dataframe(
    dataframe: DataFrame,
    renames: &[RelationalRename],
) -> Result<DataFrame, RelationalError> {
    let source_names = dataframe
        .get_column_names()
        .into_iter()
        .map(|name| name.as_str())
        .collect::<BTreeSet<_>>();
    let mut renamed_sources = BTreeSet::new();
    let mut destinations = BTreeSet::new();
    for rename in renames {
        validate_name("source", &rename.from)?;
        validate_name("destination", &rename.to)?;
        if !source_names.contains(rename.from.as_ref()) {
            return Err(RelationalError::new(
                RelationalErrorCode::ColumnMissing,
                format!("rename source column '{}' does not exist", rename.from),
            ));
        }
        if !renamed_sources.insert(rename.from.as_ref()) {
            return Err(operator_invalid(format!(
                "rename source column '{}' is mapped more than once",
                rename.from
            )));
        }
        if !destinations.insert(rename.to.as_ref()) {
            return Err(operator_invalid(format!(
                "rename destination column '{}' is mapped more than once",
                rename.to
            )));
        }
    }
    if let Some(destination) = destinations.iter().find(|destination| {
        source_names.contains(**destination) && !renamed_sources.contains(**destination)
    }) {
        return Err(operator_invalid(format!(
            "rename destination column '{destination}' already exists"
        )));
    }
    let columns = dataframe
        .columns()
        .iter()
        .map(|column| {
            let destination = renames
                .iter()
                .find(|rename| rename.from.as_ref() == column.name().as_str())
                .map(|rename| rename.to.as_ref())
                .unwrap_or_else(|| column.name().as_str());
            Column::from(
                column
                    .as_materialized_series()
                    .clone()
                    .with_name(destination.into()),
            )
        })
        .collect();
    DataFrame::new(dataframe.height(), columns)
        .map_err(|_| operator_invalid("rename dataframe construction failed"))
}

fn validate_name(role: &str, name: &str) -> Result<(), RelationalError> {
    if name.is_empty() {
        return Err(operator_invalid(format!(
            "rename {role} column name must not be empty"
        )));
    }
    if name.trim() != name {
        return Err(operator_invalid(format!(
            "rename {role} column name must not have leading or trailing whitespace"
        )));
    }
    Ok(())
}

fn input_shape(message: impl Into<Box<str>>) -> RelationalError {
    RelationalError::new(RelationalErrorCode::InputShapeInvalid, message)
}

fn operator_invalid(message: impl Into<Box<str>>) -> RelationalError {
    RelationalError::new(RelationalErrorCode::OperatorInvalid, message)
}

fn type_mismatch(message: impl Into<Box<str>>) -> RelationalError {
    RelationalError::new(RelationalErrorCode::TypeMismatch, message)
}

fn column_missing(name: &str) -> RelationalError {
    RelationalError::new(
        RelationalErrorCode::ColumnMissing,
        format!("relational column '{name}' was not found"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::plan::{
        RelationalExpression, RelationalLiteral, RelationalProjection, RelationalRename,
    };
    use crate::node_system::protocol::{CanonicalDecimal, Value};
    use crate::node_system::runtime::{Artifact, ArtifactKind, RelationalErrorCode, RuntimeValue};
    use polars::prelude::DataType;
    use std::collections::BTreeMap;

    #[test]
    fn ingress_accepts_scalar_object_and_single_value_artifact() {
        let value = dataframe_value(&[
            ("id", vec![Value::Integer(1), Value::Null]),
            ("active", vec![Value::Bool(true), Value::Bool(false)]),
        ]);

        let scalar = tabular_runtime_to_dataframe(value.clone()).unwrap();
        let artifact = tabular_runtime_to_dataframe(RuntimeValue::Artifact(Artifact::new(
            ArtifactKind::Collected,
            [runtime_scalar(value)],
        )))
        .unwrap();

        assert_eq!(scalar, artifact);
        assert_eq!(scalar.shape(), (2, 2));
        assert_eq!(scalar["id"].dtype(), &DataType::Int64);
        assert_eq!(scalar["id"].null_count(), 1);
        assert_eq!(scalar["active"].dtype(), &DataType::Boolean);
    }

    #[test]
    fn ingress_rejects_every_non_normalized_runtime_shape() {
        let unequal = dataframe_value(&[
            ("a", vec![Value::Integer(1)]),
            ("b", vec![Value::Integer(1), Value::Integer(2)]),
        ]);
        let cases = [
            RuntimeValue::Scalar(Value::Integer(1)),
            RuntimeValue::Artifact(Artifact::new(ArtifactKind::Collected, [])),
            RuntimeValue::Artifact(Artifact::new(
                ArtifactKind::Collected,
                [
                    Value::Object(BTreeMap::new()),
                    Value::Object(BTreeMap::new()),
                ],
            )),
            unequal,
        ];

        for value in cases {
            assert_eq!(
                tabular_runtime_to_dataframe(value).unwrap_err().code(),
                RelationalErrorCode::InputShapeInvalid
            );
        }
    }

    #[test]
    fn ingress_rejects_stream_without_collecting_it() {
        let cancellation = crate::node_system::runtime::CancellationToken::new();
        let owner = crate::node_system::runtime::RunResourceOwner::new(
            crate::node_system::runtime::RunId::new(1),
            crate::node_system::runtime::RunResourceBudgets::default(),
            cancellation,
        )
        .unwrap();
        let stream = owner
            .stream_from_values([Value::Object(BTreeMap::new())])
            .unwrap();

        let error = tabular_runtime_to_dataframe(RuntimeValue::Stream(stream)).unwrap_err();

        assert_eq!(error.code(), RelationalErrorCode::InputShapeInvalid);
    }

    #[test]
    fn ingress_uses_i64_and_checked_float64_without_losing_nulls() {
        let dataframe = tabular_runtime_to_dataframe(dataframe_value(&[
            (
                "integer",
                vec![Value::Integer(i64::MIN), Value::Null, Value::Integer(0)],
            ),
            (
                "float",
                vec![Value::Integer(2), decimal("2.5"), Value::Null],
            ),
        ]))
        .unwrap();

        assert_eq!(dataframe["integer"].dtype(), &DataType::Int64);
        assert_eq!(dataframe["float"].dtype(), &DataType::Float64);
        assert_eq!(dataframe["float"].null_count(), 1);
    }

    #[test]
    fn checked_float64_ingress_accepts_only_exact_i64_values() {
        for value in [i64::MIN, -(1_i64 << 53), (1_i64 << 53) - 1, 1_i64 << 53] {
            let dataframe = tabular_runtime_to_dataframe(dataframe_value(&[(
                "float",
                vec![decimal("1.5"), Value::Integer(value)],
            )]))
            .unwrap();
            assert_eq!(dataframe["float"].dtype(), &DataType::Float64);
        }

        for value in [i64::MAX, -(1_i64 << 53) - 1, (1_i64 << 53) + 1] {
            let error = tabular_runtime_to_dataframe(dataframe_value(&[(
                "float",
                vec![decimal("1.5"), Value::Integer(value)],
            )]))
            .unwrap_err();
            assert_eq!(error.code(), RelationalErrorCode::TypeMismatch);
        }
    }

    #[test]
    fn project_preserves_requested_order_dtype_values_and_nulls() {
        let source = tabular_runtime_to_dataframe(dataframe_value(&[
            ("id", vec![Value::Integer(1), Value::Null]),
            (
                "name",
                vec![Value::String("A".into()), Value::String("B".into())],
            ),
            ("keep", vec![Value::Bool(true), Value::Bool(false)]),
        ]))
        .unwrap();
        let projections = [projection("name", "name"), projection("id", "id")];

        let projected = project_dataframe(source, &projections).unwrap();

        assert_eq!(column_names(&projected), ["name", "id"]);
        assert_eq!(projected["name"].dtype(), &DataType::String);
        assert_eq!(projected["id"].dtype(), &DataType::Int64);
        assert_eq!(projected["id"].null_count(), 1);
    }

    #[test]
    fn project_rejects_missing_duplicate_and_derived_columns() {
        let source = || {
            tabular_runtime_to_dataframe(dataframe_value(&[("id", vec![Value::Integer(1)])]))
                .unwrap()
        };
        let missing = project_dataframe(source(), &[projection("missing", "missing")]).unwrap_err();
        let duplicate =
            project_dataframe(source(), &[projection("id", "id"), projection("id", "id")])
                .unwrap_err();
        let derived = project_dataframe(
            source(),
            &[RelationalProjection {
                name: "derived".into(),
                expression: RelationalExpression::Literal(RelationalLiteral::Integer(1)),
            }],
        )
        .unwrap_err();

        assert_eq!(missing.code(), RelationalErrorCode::ColumnMissing);
        assert_eq!(duplicate.code(), RelationalErrorCode::OperatorInvalid);
        assert_eq!(derived.code(), RelationalErrorCode::OperatorInvalid);
    }

    #[test]
    fn project_rejects_aliases_and_duplicate_source_references() {
        let source = || {
            tabular_runtime_to_dataframe(dataframe_value(&[("id", vec![Value::Integer(1)])]))
                .unwrap()
        };

        let alias = project_dataframe(source(), &[projection("alias", "id")]).unwrap_err();
        let duplicate_source = project_dataframe(
            source(),
            &[projection("first", "id"), projection("second", "id")],
        )
        .unwrap_err();

        assert_eq!(alias.code(), RelationalErrorCode::OperatorInvalid);
        assert!(alias.message().contains("aliases"));
        assert_eq!(
            duplicate_source.code(),
            RelationalErrorCode::OperatorInvalid
        );
        assert!(duplicate_source.message().contains("source columns"));
    }

    #[test]
    fn filter_preserves_row_order_and_drops_null_comparison_rows() {
        let source = tabular_runtime_to_dataframe(dataframe_value(&[
            (
                "id",
                vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)],
            ),
            ("amount", vec![decimal("1.5"), Value::Null, decimal("3.5")]),
        ]))
        .unwrap();
        let predicate = RelationalExpression::GreaterThan(
            Box::new(RelationalExpression::Column("amount".into())),
            Box::new(RelationalExpression::Literal(RelationalLiteral::Integer(1))),
        );

        let filtered = filter_dataframe(source, &predicate).unwrap();

        assert_eq!(filtered.height(), 2);
        assert_eq!(
            filtered["id"]
                .i64()
                .unwrap()
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            vec![1, 3]
        );
        assert_eq!(filtered["amount"].dtype(), &DataType::Float64);
    }

    #[test]
    fn filter_comparison_matrix_is_type_exact() {
        let source = || {
            tabular_runtime_to_dataframe(dataframe_value(&[
                ("flag", vec![Value::Bool(false), Value::Bool(true)]),
                (
                    "name",
                    vec![Value::String("a".into()), Value::String("z".into())],
                ),
                (
                    "large",
                    vec![
                        Value::Integer(9_007_199_254_740_992),
                        Value::Integer(9_007_199_254_740_993),
                    ],
                ),
                ("amount", vec![decimal("1.5"), decimal("2.5")]),
            ]))
            .unwrap()
        };
        let comparison = |column: &str, literal, ordered| {
            let left = Box::new(RelationalExpression::Column(column.into()));
            let right = Box::new(RelationalExpression::Literal(literal));
            if ordered {
                RelationalExpression::GreaterThan(left, right)
            } else {
                RelationalExpression::Equal(left, right)
            }
        };

        assert_eq!(
            filter_dataframe(
                source(),
                &comparison("flag", RelationalLiteral::Boolean(true), false),
            )
            .unwrap()
            .height(),
            1
        );
        assert_eq!(
            filter_dataframe(
                source(),
                &RelationalExpression::NotEqual(
                    Box::new(RelationalExpression::Column("flag".into())),
                    Box::new(RelationalExpression::Literal(RelationalLiteral::Boolean(
                        true
                    ))),
                ),
            )
            .unwrap()
            .height(),
            1
        );
        assert_eq!(
            filter_dataframe(
                source(),
                &comparison("name", RelationalLiteral::String("a".into()), true),
            )
            .unwrap()
            .height(),
            1
        );
        assert_eq!(
            filter_dataframe(
                source(),
                &comparison(
                    "large",
                    RelationalLiteral::Integer(9_007_199_254_740_993),
                    false,
                ),
            )
            .unwrap()
            .height(),
            1
        );
        assert_eq!(
            filter_dataframe(
                source(),
                &comparison(
                    "amount",
                    RelationalLiteral::Decimal(CanonicalDecimal::new("1.5").unwrap(),),
                    false
                ),
            )
            .unwrap()
            .height(),
            1
        );
        assert_eq!(
            filter_dataframe(
                source(),
                &comparison("amount", RelationalLiteral::Integer(2), true),
            )
            .unwrap()
            .height(),
            1
        );

        for predicate in [
            comparison("flag", RelationalLiteral::Boolean(false), true),
            comparison("flag", RelationalLiteral::String("false".into()), false),
            comparison("name", RelationalLiteral::Integer(1), false),
            comparison(
                "large",
                RelationalLiteral::Decimal(CanonicalDecimal::new("1.5").unwrap()),
                false,
            ),
            comparison(
                "amount",
                RelationalLiteral::Integer(9_007_199_254_740_993),
                false,
            ),
        ] {
            assert_eq!(
                filter_dataframe(source(), &predicate).unwrap_err().code(),
                RelationalErrorCode::TypeMismatch
            );
        }
    }

    #[test]
    fn float_filter_integer_literals_require_exact_i64_conversion() {
        let source =
            || DataFrame::new(1, vec![Column::new("amount".into(), vec![Some(0.0_f64)])]).unwrap();
        let predicate = |value| {
            RelationalExpression::Equal(
                Box::new(RelationalExpression::Column("amount".into())),
                Box::new(RelationalExpression::Literal(RelationalLiteral::Integer(
                    value,
                ))),
            )
        };

        for value in [i64::MIN, -(1_i64 << 53), (1_i64 << 53) - 1, 1_i64 << 53] {
            assert!(filter_dataframe(source(), &predicate(value)).is_ok());
        }
        for value in [i64::MAX, -(1_i64 << 53) - 1, (1_i64 << 53) + 1] {
            assert_eq!(
                filter_dataframe(source(), &predicate(value))
                    .unwrap_err()
                    .code(),
                RelationalErrorCode::TypeMismatch
            );
        }
    }

    #[test]
    fn filter_is_null_has_explicit_null_semantics_and_requires_boolean_predicate() {
        let source = || {
            tabular_runtime_to_dataframe(dataframe_value(&[(
                "value",
                vec![Value::Integer(1), Value::Null],
            )]))
            .unwrap()
        };
        let nulls = filter_dataframe(
            source(),
            &RelationalExpression::IsNull(Box::new(RelationalExpression::Column("value".into()))),
        )
        .unwrap();
        let mismatch =
            filter_dataframe(source(), &RelationalExpression::Column("value".into())).unwrap_err();

        assert_eq!(nulls.height(), 1);
        assert_eq!(nulls["value"].null_count(), 1);
        assert_eq!(mismatch.code(), RelationalErrorCode::TypeMismatch);
    }

    #[test]
    fn rename_preserves_order_dtype_nulls_and_applies_swaps_atomically() {
        let source = tabular_runtime_to_dataframe(dataframe_value(&[
            ("left", vec![Value::Integer(1), Value::Null]),
            (
                "right",
                vec![Value::String("x".into()), Value::String("y".into())],
            ),
        ]))
        .unwrap();

        let renamed =
            rename_dataframe(source, &[rename("left", "right"), rename("right", "left")]).unwrap();

        assert_eq!(column_names(&renamed), ["right", "left"]);
        assert_eq!(renamed["right"].dtype(), &DataType::Int64);
        assert_eq!(renamed["right"].null_count(), 1);
        assert_eq!(renamed["left"].dtype(), &DataType::String);
    }

    fn column_names(dataframe: &polars::prelude::DataFrame) -> Vec<&str> {
        dataframe
            .get_column_names()
            .into_iter()
            .map(|name| name.as_str())
            .collect()
    }

    fn dataframe_value(columns: &[(&str, Vec<Value>)]) -> RuntimeValue {
        RuntimeValue::Scalar(Value::Object(
            columns
                .iter()
                .map(|(name, values)| ((*name).into(), Value::List(values.clone())))
                .collect(),
        ))
    }

    fn runtime_scalar(value: RuntimeValue) -> Value {
        let RuntimeValue::Scalar(value) = value else {
            unreachable!()
        };
        value
    }

    fn decimal(value: &str) -> Value {
        Value::Decimal(CanonicalDecimal::new(value).unwrap())
    }

    fn projection(name: &str, source: &str) -> RelationalProjection {
        RelationalProjection {
            name: name.into(),
            expression: RelationalExpression::Column(source.into()),
        }
    }

    fn rename(from: &str, to: &str) -> RelationalRename {
        RelationalRename {
            from: from.into(),
            to: to.into(),
        }
    }
}
