use super::{RelationalError, RuntimeValue};
use crate::node_system::plan::{RelationalExpression, RelationalLiteral};
use crate::node_system::protocol::Value;
use std::collections::BTreeMap;

pub(super) fn runtime_scalar(value: &RuntimeValue) -> Result<&Value, RelationalError> {
    match value {
        RuntimeValue::Scalar(value) => Ok(value),
        _ => Err(RelationalError::new(
            "relational operator input is not materialized",
        )),
    }
}

pub(super) fn limit_protocol_value(value: &Value, rows: u64) -> Result<Value, RelationalError> {
    let source = relational_object(value)?;
    Ok(Value::Object(
        source
            .into_iter()
            .map(|(name, value)| {
                let value = match value {
                    Value::List(mut values) => {
                        values.truncate(rows.min(usize::MAX as u64) as usize);
                        Value::List(values)
                    }
                    value => value,
                };
                (name, value)
            })
            .collect(),
    ))
}

pub(super) fn relational_object(
    value: &Value,
) -> Result<BTreeMap<Box<str>, Value>, RelationalError> {
    match value {
        Value::Object(value) => Ok(value.clone()),
        _ => Err(RelationalError::new("relational value is not a dataframe")),
    }
}

pub(super) fn relational_expression(
    expression: &RelationalExpression,
    dataframe: &Value,
) -> Result<Value, RelationalError> {
    match expression {
        RelationalExpression::Column(name) => relational_object(dataframe)?
            .remove(name.as_ref())
            .ok_or_else(|| RelationalError::new(format!("column '{name}' was not found"))),
        RelationalExpression::Literal(value) => Ok(match value {
            RelationalLiteral::Null => Value::Null,
            RelationalLiteral::Boolean(value) => Value::Bool(*value),
            RelationalLiteral::Integer(value) => Value::Integer(*value),
            RelationalLiteral::String(value) => Value::String(value.clone()),
        }),
        RelationalExpression::Equal(left, right) => {
            relational_compare(left, right, dataframe, |a, b| a == b)
        }
        RelationalExpression::NotEqual(left, right) => {
            relational_compare(left, right, dataframe, |a, b| a != b)
        }
        RelationalExpression::LessThan(left, right) => {
            relational_numeric_compare(left, right, dataframe, |a, b| a < b)
        }
        RelationalExpression::LessThanOrEqual(left, right) => {
            relational_numeric_compare(left, right, dataframe, |a, b| a <= b)
        }
        RelationalExpression::GreaterThan(left, right) => {
            relational_numeric_compare(left, right, dataframe, |a, b| a > b)
        }
        RelationalExpression::GreaterThanOrEqual(left, right) => {
            relational_numeric_compare(left, right, dataframe, |a, b| a >= b)
        }
        RelationalExpression::And(expressions) | RelationalExpression::Or(expressions) => {
            let is_and = matches!(expression, RelationalExpression::And(_));
            let mut masks = expressions
                .iter()
                .map(|expression| relational_expression(expression, dataframe));
            let first = masks.next().transpose()?.unwrap_or(Value::Bool(is_and));
            masks.try_fold(first, |left, right| {
                relational_bool_combine(&left, &right?, is_and)
            })
        }
        RelationalExpression::Not(expression) => {
            match relational_expression(expression, dataframe)? {
                Value::Bool(value) => Ok(Value::Bool(!value)),
                Value::List(values) => Ok(Value::List(
                    values
                        .into_iter()
                        .map(|value| match value {
                            Value::Bool(value) => Value::Bool(!value),
                            _ => Value::Bool(false),
                        })
                        .collect(),
                )),
                _ => Err(RelationalError::new("not expects boolean values")),
            }
        }
        RelationalExpression::IsNull(expression) => {
            match relational_expression(expression, dataframe)? {
                Value::List(values) => Ok(Value::List(
                    values
                        .into_iter()
                        .map(|value| Value::Bool(matches!(value, Value::Null)))
                        .collect(),
                )),
                value => Ok(Value::Bool(matches!(value, Value::Null))),
            }
        }
    }
}

fn relational_expand(value: Value, len: usize) -> Vec<Value> {
    match value {
        Value::List(values) => values,
        value => vec![value; len],
    }
}

fn relational_compare(
    left: &RelationalExpression,
    right: &RelationalExpression,
    dataframe: &Value,
    compare: impl Fn(&Value, &Value) -> bool,
) -> Result<Value, RelationalError> {
    let left = relational_expression(left, dataframe)?;
    let right = relational_expression(right, dataframe)?;
    let len = match (&left, &right) {
        (Value::List(values), _) | (_, Value::List(values)) => values.len(),
        _ => 1,
    };
    Ok(Value::List(
        relational_expand(left, len)
            .iter()
            .zip(relational_expand(right, len).iter())
            .map(|(left, right)| Value::Bool(compare(left, right)))
            .collect(),
    ))
}

fn relational_number(value: &Value) -> Option<f64> {
    match value {
        Value::Integer(value) => Some(*value as f64),
        Value::Unsigned(value) => Some(*value as f64),
        Value::Decimal(value) => value.as_str().parse().ok(),
        _ => None,
    }
}

fn relational_numeric_compare(
    left: &RelationalExpression,
    right: &RelationalExpression,
    dataframe: &Value,
    compare: impl Fn(f64, f64) -> bool,
) -> Result<Value, RelationalError> {
    relational_compare(left, right, dataframe, |left, right| {
        relational_number(left)
            .zip(relational_number(right))
            .is_some_and(|(left, right)| compare(left, right))
    })
}

fn relational_bool_combine(
    left: &Value,
    right: &Value,
    and: bool,
) -> Result<Value, RelationalError> {
    let values = |value: &Value| match value {
        Value::List(values) => values.clone(),
        value => vec![value.clone()],
    };
    let left = values(left);
    let right = values(right);
    if left.is_empty() || right.is_empty() {
        return Ok(Value::List(Vec::new()));
    }
    let len = left.len().max(right.len());
    Ok(Value::List(
        (0..len)
            .map(|index| {
                let left = matches!(left.get(index % left.len()), Some(Value::Bool(true)));
                let right = matches!(right.get(index % right.len()), Some(Value::Bool(true)));
                Value::Bool(if and { left && right } else { left || right })
            })
            .collect(),
    ))
}

pub(super) fn relational_filter(dataframe: &Value, mask: &Value) -> Result<Value, RelationalError> {
    let Value::List(mask) = mask else {
        return Err(RelationalError::new(
            "filter predicate is not a boolean series",
        ));
    };
    Ok(Value::Object(
        relational_object(dataframe)?
            .into_iter()
            .map(|(name, value)| {
                let values = match value {
                    Value::List(values) => values,
                    value => vec![value],
                };
                let filtered = values
                    .into_iter()
                    .zip(mask)
                    .filter_map(|(value, keep)| matches!(keep, Value::Bool(true)).then_some(value))
                    .collect();
                (name, Value::List(filtered))
            })
            .collect(),
    ))
}
