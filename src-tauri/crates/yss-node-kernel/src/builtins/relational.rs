use crate::KernelInvocation;
use std::collections::BTreeMap;

use yss_relational_contract::{
    RelationComparison, RelationControl, RelationError, RelationLiteral, RelationPredicate,
    SeriesHandle,
};

use crate::KernelError;
use crate::RuntimeValue;

pub(crate) fn decompose(
    invocation: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let [input @ (RuntimeValue::Relation(_) | RuntimeValue::Record(_))] = invocation.inputs else {
        return Err(KernelError::Failed);
    };
    invocation
        .outputs
        .iter()
        .map(|output| {
            let [field] = output.fields.as_deref().ok_or(KernelError::Failed)? else {
                return Err(KernelError::Failed);
            };
            let value = match input {
                RuntimeValue::Relation(relation) => {
                    RuntimeValue::Series(relation.select_series(&field.name).map_err(kernel_error)?)
                }
                RuntimeValue::Record(columns) => match columns.get(&field.name) {
                    Some(column) if matches!(column.unannotated(), RuntimeValue::List(_)) => {
                        column.clone()
                    }
                    _ => return Err(KernelError::Failed),
                },
                _ => return Err(KernelError::Failed),
            };
            Ok(value)
        })
        .collect()
}

#[derive(Clone, Copy)]
pub(crate) enum RelationalKernel {
    Source,
    Project,
    Filter,
    DropColumns,
    DropRows,
    Series,
    Limit,
    Rename,
    ConcatRows,
    ConcatColumns,
    Join,
    Assemble,
}

pub(crate) fn execute(
    kind: RelationalKernel,
    invocation: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    let parameter = |key| invocation.parameter(key).ok_or(KernelError::Failed);
    invocation.check_control()?;
    if matches!(kind, RelationalKernel::Assemble) {
        let fields = invocation
            .outputs
            .first()
            .and_then(|output| output.fields.as_deref())
            .ok_or(KernelError::Failed)?;
        if fields.len() != invocation.inputs.len()
            || fields.is_empty()
            || fields.iter().any(|field| field.name.trim().is_empty())
            || fields
                .iter()
                .map(|field| &field.name)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != fields.len()
        {
            return Err(KernelError::Failed);
        }
        if invocation
            .inputs
            .iter()
            .all(|v| matches!(v, RuntimeValue::Series(_)))
        {
            let series = invocation
                .inputs
                .iter()
                .map(|v| match v {
                    RuntimeValue::Series(s) => Ok(s.clone()),
                    _ => Err(KernelError::Failed),
                })
                .collect::<Result<Vec<_>, _>>()?;
            return series[0]
                .relation()
                .assemble_series(
                    &series,
                    &fields
                        .iter()
                        .map(|field| field.name.clone())
                        .collect::<Vec<_>>(),
                )
                .map(RuntimeValue::Relation)
                .map_err(kernel_error);
        }
        let mut rows = None;
        let mut bytes = 0usize;
        for input in invocation.inputs {
            let RuntimeValue::List(values) = input.unannotated() else {
                return Err(KernelError::Failed);
            };
            if rows.is_some_and(|rows| rows != values.len()) {
                return Err(KernelError::Failed);
            }
            rows = Some(values.len());
            for (index, value) in values.iter().enumerate() {
                if index % 1024 == 0 {
                    invocation.check_control()?;
                }
                match value {
                    RuntimeValue::Null
                    | RuntimeValue::Bool(_)
                    | RuntimeValue::Integer(_)
                    | RuntimeValue::Unsigned(_)
                    | RuntimeValue::String(_) => {}
                    RuntimeValue::Decimal(value) if value.is_finite() => {}
                    _ => return Err(KernelError::Failed),
                }
                bytes = bytes
                    .checked_add(
                        std::mem::size_of::<RuntimeValue>()
                            + match value {
                                RuntimeValue::String(value) => value.len(),
                                _ => 0,
                            },
                    )
                    .ok_or(KernelError::Failed)?;
                if bytes > invocation.control.max_input_bytes {
                    return Err(KernelError::Failed);
                }
            }
        }
        return Ok(RuntimeValue::Record(
            fields
                .iter()
                .zip(invocation.inputs)
                .map(|(field, value)| (field.name.clone(), value.clone()))
                .collect(),
        ));
    }
    if matches!(kind, RelationalKernel::Source) {
        let value = parameter("dataframe")?;
        return match value {
            RuntimeValue::Relation(_) => Ok(value.clone()),
            _ => Err(KernelError::Failed),
        };
    }
    let Some(RuntimeValue::Relation(relation)) = invocation.inputs.first() else {
        return Err(KernelError::Failed);
    };
    let relation = match kind {
        RelationalKernel::ConcatRows | RelationalKernel::ConcatColumns => {
            if invocation.inputs.len() < 2 {
                return Err(KernelError::Failed);
            }
            let others = invocation.inputs[1..]
                .iter()
                .map(|v| match v {
                    RuntimeValue::Relation(v) => Ok(v.clone()),
                    _ => Err(KernelError::Failed),
                })
                .collect::<Result<Vec<_>, _>>()?;
            if matches!(kind, RelationalKernel::ConcatRows) {
                let mode = match text(parameter("column_match")?)?.as_ref() {
                    "by_name" => yss_data_contract::table::RowConcatMode::ByName,
                    "by_position" => yss_data_contract::table::RowConcatMode::ByPosition,
                    _ => return Err(KernelError::Failed),
                };
                relation.concat_rows(&others, mode)
            } else {
                relation.concat_columns(&others)
            }
        }
        RelationalKernel::Join => {
            let [_, RuntimeValue::Relation(right)] = invocation.inputs else {
                return Err(KernelError::Failed);
            };
            let keys = |name| {
                let RuntimeValue::List(values) = parameter(name)? else {
                    return Err(KernelError::Failed);
                };
                values
                    .iter()
                    .map(|v| text(v).map(String::from))
                    .collect::<Result<Vec<_>, _>>()
            };
            let kind = match text(parameter("join_type")?)?.as_ref() {
                "inner" => yss_data_contract::table::TableJoinKind::Inner,
                "left" => yss_data_contract::table::TableJoinKind::Left,
                "right" => yss_data_contract::table::TableJoinKind::Right,
                "full" => yss_data_contract::table::TableJoinKind::Full,
                _ => return Err(KernelError::Failed),
            };
            relation.join(
                right,
                &yss_data_contract::table::TableJoin {
                    kind,
                    left_keys: keys("left_keys")?,
                    right_keys: keys("right_keys")?,
                    right_suffix: text(parameter("right_suffix")?)?.into(),
                },
            )
        }
        RelationalKernel::Project | RelationalKernel::DropColumns => {
            let RuntimeValue::List(columns) = parameter("columns")? else {
                return Err(KernelError::Failed);
            };
            let columns = columns.iter().map(text).collect::<Result<Vec<_>, _>>()?;
            if matches!(kind, RelationalKernel::DropColumns) {
                relation.drop_columns(&columns)
            } else {
                relation.project(&columns)
            }
        }
        RelationalKernel::Filter | RelationalKernel::DropRows => {
            let RuntimeValue::Record(fields) = parameter("predicate")? else {
                return Err(KernelError::Failed);
            };
            if matches!(kind, RelationalKernel::DropRows) {
                relation.drop_rows(&predicate(fields)?)
            } else {
                relation.filter(&predicate(fields)?)
            }
        }
        RelationalKernel::Series => {
            return relation
                .select_series(&text(parameter("column")?)?)
                .map(RuntimeValue::Series)
                .map_err(kernel_error);
        }
        RelationalKernel::Limit => {
            let RuntimeValue::Integer(rows) = parameter("rows")? else {
                return Err(KernelError::Failed);
            };
            relation.limit(0, usize::try_from(*rows).map_err(|_| KernelError::Failed)?)
        }
        RelationalKernel::Rename => {
            relation.rename(&text(parameter("from")?)?, &text(parameter("to")?)?)
        }
        RelationalKernel::Source | RelationalKernel::Assemble => return Err(KernelError::Failed),
    };
    relation.map(RuntimeValue::Relation).map_err(kernel_error)
}

fn text(value: &RuntimeValue) -> Result<Box<str>, KernelError> {
    match value {
        RuntimeValue::String(value) => Ok(value.clone()),
        _ => Err(KernelError::Failed),
    }
}

fn predicate(fields: &BTreeMap<Box<str>, RuntimeValue>) -> Result<RelationPredicate, KernelError> {
    let field = |key: &str| fields.get(key).ok_or(KernelError::Failed);
    let comparison = match text(field("operator")?)?.as_ref() {
        "equal" => RelationComparison::Equal,
        "notEqual" => RelationComparison::NotEqual,
        "lessThan" => RelationComparison::Less,
        "lessThanOrEqual" => RelationComparison::LessEqual,
        "greaterThan" => RelationComparison::Greater,
        "greaterThanOrEqual" => RelationComparison::GreaterEqual,
        "isNull" => RelationComparison::IsNull,
        "isNotNull" => RelationComparison::IsNotNull,
        _ => return Err(KernelError::Failed),
    };
    let value = match fields.get("value") {
        None | Some(RuntimeValue::Null) => None,
        Some(RuntimeValue::Record(literal)) => {
            let value = literal.get("value").ok_or(KernelError::Failed)?;
            Some(
                match literal.get("type").map(text).transpose()?.as_deref() {
                    Some("boolean") => match value {
                        RuntimeValue::Bool(value) => RelationLiteral::Boolean(*value),
                        _ => return Err(KernelError::Failed),
                    },
                    Some("integer") => RelationLiteral::Integer(
                        text(value)?.parse().map_err(|_| KernelError::Failed)?,
                    ),
                    Some("decimal") => RelationLiteral::Decimal(text(value)?),
                    Some("string") => RelationLiteral::String(text(value)?),
                    _ => return Err(KernelError::Failed),
                },
            )
        }
        _ => return Err(KernelError::Failed),
    };
    Ok(RelationPredicate {
        column: text(field("column")?)?,
        comparison,
        value,
    })
}

pub(crate) fn numeric_columns(
    series: &[SeriesHandle],
    invocation: &KernelInvocation<'_>,
) -> Result<Vec<Vec<f64>>, KernelError> {
    let relation = series
        .first()
        .ok_or(KernelError::InvalidNumericInput)?
        .relation();
    relation
        .numeric_columns(
            series,
            &RelationControl {
                cancellation: invocation.control.cancellation.clone(),
                deadline: invocation.control.deadline,
                max_input_bytes: invocation.control.max_input_bytes,
            },
        )
        .map_err(kernel_error)
}

pub(crate) fn kernel_error(error: RelationError) -> KernelError {
    match error {
        RelationError::Cancelled => KernelError::Cancelled,
        RelationError::DeadlineExceeded => KernelError::DeadlineExceeded,
        RelationError::DivisionByZero => KernelError::DivisionByZero,
        RelationError::NonFiniteResult => KernelError::NonFiniteResult,
        RelationError::InvalidInput | RelationError::UnalignedSeries => {
            KernelError::InvalidNumericInput
        }
        _ => KernelError::Failed,
    }
}
