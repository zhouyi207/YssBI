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
                    Some(column @ RuntimeValue::List(_)) => column.clone(),
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
    Series,
    Limit,
    Rename,
}

pub(crate) fn execute(
    kind: RelationalKernel,
    invocation: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    let parameter = |key| invocation.parameter(key).ok_or(KernelError::Failed);
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
        RelationalKernel::Project => {
            let RuntimeValue::List(columns) = parameter("columns")? else {
                return Err(KernelError::Failed);
            };
            let columns = columns.iter().map(text).collect::<Result<Vec<_>, _>>()?;
            relation.project(&columns)
        }
        RelationalKernel::Filter => {
            let RuntimeValue::Record(fields) = parameter("predicate")? else {
                return Err(KernelError::Failed);
            };
            relation.filter(&predicate(fields)?)
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
        RelationalKernel::Source => return Err(KernelError::Failed),
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
