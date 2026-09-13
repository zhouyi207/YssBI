use std::collections::BTreeMap;

use yss_relational_contract::{
    RelationComparison, RelationControl, RelationError, RelationLiteral, RelationPredicate,
    SeriesHandle,
};

use crate::state::{KernelExecutionError, PreparedKernelInvocation, parameter_value};
use crate::value::RuntimeValue;

pub(crate) fn decompose(
    invocation: &PreparedKernelInvocation<'_>,
) -> Result<BTreeMap<crate::plan::PlanOutputRef, RuntimeValue>, KernelExecutionError> {
    let [input @ (RuntimeValue::Relation(_) | RuntimeValue::Record(_))] = invocation.inputs else {
        return Err(KernelExecutionError::Failed);
    };
    invocation
        .outputs
        .iter()
        .map(|output| {
            let [field] = output
                .contract()
                .schema
                .as_deref()
                .ok_or(KernelExecutionError::Failed)?
            else {
                return Err(KernelExecutionError::Failed);
            };
            let value = match input {
                RuntimeValue::Relation(relation) => {
                    RuntimeValue::Series(relation.select_series(&field.name).map_err(kernel_error)?)
                }
                RuntimeValue::Record(columns) => match columns.get(&field.name) {
                    Some(column @ RuntimeValue::List(_)) => column.clone(),
                    _ => return Err(KernelExecutionError::Failed),
                },
                _ => return Err(KernelExecutionError::Failed),
            };
            Ok((output.output().clone(), value))
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
    invocation: &PreparedKernelInvocation<'_>,
) -> Result<RuntimeValue, KernelExecutionError> {
    let parameter = |key| {
        parameter_value(
            invocation
                .parameter(key)
                .ok_or(KernelExecutionError::Failed)?,
            invocation.resources,
        )
    };
    if matches!(kind, RelationalKernel::Source) {
        let value = parameter("dataframe")?;
        return match value {
            RuntimeValue::Relation(_) => Ok(value),
            _ => Err(KernelExecutionError::Failed),
        };
    }
    let Some(RuntimeValue::Relation(relation)) = invocation.inputs.first() else {
        return Err(KernelExecutionError::Failed);
    };
    let relation = match kind {
        RelationalKernel::Project => {
            let RuntimeValue::List(columns) = parameter("columns")? else {
                return Err(KernelExecutionError::Failed);
            };
            let columns = columns.iter().map(text).collect::<Result<Vec<_>, _>>()?;
            relation.project(&columns)
        }
        RelationalKernel::Filter => {
            let RuntimeValue::Record(fields) = parameter("predicate")? else {
                return Err(KernelExecutionError::Failed);
            };
            relation.filter(&predicate(&fields)?)
        }
        RelationalKernel::Series => {
            return relation
                .select_series(&text(&parameter("column")?)?)
                .map(RuntimeValue::Series)
                .map_err(kernel_error);
        }
        RelationalKernel::Limit => {
            let RuntimeValue::Integer(rows) = parameter("rows")? else {
                return Err(KernelExecutionError::Failed);
            };
            relation.limit(
                0,
                usize::try_from(rows).map_err(|_| KernelExecutionError::Failed)?,
            )
        }
        RelationalKernel::Rename => {
            relation.rename(&text(&parameter("from")?)?, &text(&parameter("to")?)?)
        }
        RelationalKernel::Source => return Err(KernelExecutionError::Failed),
    };
    relation.map(RuntimeValue::Relation).map_err(kernel_error)
}

fn text(value: &RuntimeValue) -> Result<Box<str>, KernelExecutionError> {
    match value {
        RuntimeValue::String(value) => Ok(value.clone()),
        _ => Err(KernelExecutionError::Failed),
    }
}

fn predicate(
    fields: &BTreeMap<Box<str>, RuntimeValue>,
) -> Result<RelationPredicate, KernelExecutionError> {
    let field = |key: &str| fields.get(key).ok_or(KernelExecutionError::Failed);
    let comparison = match text(field("operator")?)?.as_ref() {
        "equal" => RelationComparison::Equal,
        "notEqual" => RelationComparison::NotEqual,
        "lessThan" => RelationComparison::Less,
        "lessThanOrEqual" => RelationComparison::LessEqual,
        "greaterThan" => RelationComparison::Greater,
        "greaterThanOrEqual" => RelationComparison::GreaterEqual,
        "isNull" => RelationComparison::IsNull,
        "isNotNull" => RelationComparison::IsNotNull,
        _ => return Err(KernelExecutionError::Failed),
    };
    let value = match fields.get("value") {
        None | Some(RuntimeValue::Null) => None,
        Some(RuntimeValue::Record(literal)) => {
            let value = literal.get("value").ok_or(KernelExecutionError::Failed)?;
            Some(
                match literal.get("type").map(text).transpose()?.as_deref() {
                    Some("boolean") => match value {
                        RuntimeValue::Bool(value) => RelationLiteral::Boolean(*value),
                        _ => return Err(KernelExecutionError::Failed),
                    },
                    Some("integer") => RelationLiteral::Integer(
                        text(value)?
                            .parse()
                            .map_err(|_| KernelExecutionError::Failed)?,
                    ),
                    Some("decimal") => RelationLiteral::Decimal(text(value)?),
                    Some("string") => RelationLiteral::String(text(value)?),
                    _ => return Err(KernelExecutionError::Failed),
                },
            )
        }
        _ => return Err(KernelExecutionError::Failed),
    };
    Ok(RelationPredicate {
        column: text(field("column")?)?,
        comparison,
        value,
    })
}

pub(crate) const MAX_NUMERIC_INPUT_BYTES: usize = 128 * 1024 * 1024;

pub(crate) fn numeric_columns(
    series: &[SeriesHandle],
    invocation: &PreparedKernelInvocation<'_>,
) -> Result<Vec<Vec<f64>>, KernelExecutionError> {
    let relation = series
        .first()
        .ok_or(KernelExecutionError::InvalidNumericInput)?
        .relation();
    relation
        .numeric_columns(
            series,
            &RelationControl {
                cancellation: invocation.control.cancellation.clone(),
                deadline: invocation.control.deadline,
                max_input_bytes: MAX_NUMERIC_INPUT_BYTES,
            },
        )
        .map_err(kernel_error)
}

pub(crate) fn kernel_error(error: RelationError) -> KernelExecutionError {
    match error {
        RelationError::Cancelled => KernelExecutionError::Cancelled,
        RelationError::DeadlineExceeded => KernelExecutionError::DeadlineExceeded,
        RelationError::DivisionByZero => KernelExecutionError::DivisionByZero,
        RelationError::NonFiniteResult => KernelExecutionError::NonFiniteResult,
        RelationError::InvalidInput | RelationError::UnalignedSeries => {
            KernelExecutionError::InvalidNumericInput
        }
        _ => KernelExecutionError::Failed,
    }
}
