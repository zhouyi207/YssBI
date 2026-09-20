use crate::KernelInvocation;
use std::collections::BTreeMap;
use yss_data_contract::FilterLiteral;
use yss_data_contract::TabularScalar;

use yss_relational_contract::{RelationComparison, RelationError, RelationPredicate};

use crate::KernelError;
use crate::RuntimeValue;

pub(crate) fn decompose(
    invocation: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let [RuntimeValue::Relation(relation)] = invocation.inputs else {
        return Err(KernelError::InputLayoutMismatch);
    };
    invocation
        .outputs
        .iter()
        .map(|output| {
            let [field] = output
                .fields
                .as_deref()
                .ok_or(KernelError::OutputContractMismatch)?
            else {
                return Err(KernelError::OutputContractMismatch);
            };
            relation
                .select_series(&field.name)
                .map(RuntimeValue::Series)
                .map_err(kernel_error)
        })
        .collect()
}

pub(crate) fn constant(invocation: &KernelInvocation<'_>) -> Result<RuntimeValue, KernelError> {
    let value = invocation
        .parameter("value")
        .ok_or(KernelError::InvalidParameter)?;
    if invocation
        .outputs
        .first()
        .is_some_and(|output| output.data_type == yss_data_contract::ValueType::DataFrame)
    {
        let RuntimeValue::Record(columns) = value else {
            return Err(KernelError::InvalidParameter);
        };
        let fields = invocation.outputs[0]
            .fields
            .as_deref()
            .ok_or(KernelError::OutputContractMismatch)?;
        if fields.len() != columns.len() {
            return Err(KernelError::ShapeMismatch);
        }
        let values = fields
            .iter()
            .map(|field| columns.get(&field.name).ok_or(KernelError::ShapeMismatch))
            .collect::<Result<Vec<_>, _>>()?;
        materialize(fields, &values, invocation)
    } else {
        Ok(value.clone())
    }
}

/// A single boundary for literal tables and assembled memory columns. Downstream table kernels
/// always consume relation handles; records remain available for ordinary structured values.
fn materialize(
    fields: &[crate::KernelField],
    inputs: &[&RuntimeValue],
    invocation: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    use arrow_array::RecordBatch;
    use arrow_schema::Schema;
    use std::sync::Arc;
    use yss_data_contract::TabularScalar;
    let mut rows = None;
    let mut bytes = 0usize;
    // Validate all lengths and the complete conversion budget before copying any columns.
    for input in inputs {
        let RuntimeValue::List(values) = input.unannotated() else {
            return Err(KernelError::InvalidParameter);
        };
        if rows.is_some_and(|rows| rows != values.len()) {
            return Err(KernelError::ShapeMismatch);
        }
        rows = Some(values.len());
        for (index, value) in values.iter().enumerate() {
            if index % 1024 == 0 {
                invocation.check_control()?;
            }
            let size = size_of::<TabularScalar>()
                + match value.unannotated() {
                    RuntimeValue::Scalar(TabularScalar::String(value)) => value.len(),
                    _ => 0,
                };
            bytes = invocation.control.check_bytes(bytes.checked_add(size))?;
        }
    }
    let mut columns = Vec::with_capacity(fields.len());
    let mut arrow_fields = Vec::with_capacity(fields.len());
    for (field, input) in fields.iter().zip(inputs) {
        let RuntimeValue::List(values) = input.unannotated() else {
            unreachable!()
        };
        let mut scalars = invocation.control.reserve(values.len())?;
        for (index, value) in values.iter().enumerate() {
            if index % 1024 == 0 {
                invocation.check_control()?;
            }
            scalars.push(
                value
                    .tabular_scalar()
                    .map_err(|_| KernelError::InvalidParameter)?,
            );
        }
        let metadata = input
            .metadata()
            .cloned()
            .or_else(|| match &field.data_type {
                yss_data_contract::ValueType::Scalar(semantic) => {
                    Some(yss_data_contract::ConversionMetadata {
                        semantic: yss_data_contract::ColumnSemantic::new(*semantic),
                        temporal: None,
                    })
                }
                _ => None,
            });
        let (field, array) =
            yss_database_arrow::materialized_column(&field.name, &scalars, metadata.as_ref())
                .map_err(|_| KernelError::InvalidParameter)?;
        arrow_fields.push(field);
        columns.push(array);
    }
    let schema = Arc::new(Schema::new(arrow_fields));
    let data = if columns.is_empty() {
        RecordBatch::new_empty(schema)
    } else {
        RecordBatch::try_new(schema, columns).map_err(|_| KernelError::ShapeMismatch)?
    };
    invocation
        .relations
        .clone()
        .materialize(data, &invocation.relation_control())
        .map(RuntimeValue::Relation)
        .map_err(kernel_error)
}

#[derive(Clone, Copy)]
pub(crate) enum RelationalKernel {
    Source,
    Project,
    Filter,
    DropColumns,
    DropRows,
    DropNaRows,
    DropNaColumns,
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
        return materialize(
            fields,
            &invocation.inputs.iter().collect::<Vec<_>>(),
            invocation,
        );
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
        RelationalKernel::DropNaRows | RelationalKernel::DropNaColumns => {
            let RuntimeValue::List(columns) = parameter("subset")? else {
                return Err(KernelError::Failed);
            };
            let columns = columns.iter().map(text).collect::<Result<Vec<_>, _>>()?;
            let mode = match text(parameter("how")?)?.as_ref() {
                "any" => yss_relational_contract::DropNaMode::Any,
                "all" => yss_relational_contract::DropNaMode::All,
                _ => return Err(KernelError::Failed),
            };
            if matches!(kind, RelationalKernel::DropNaRows) {
                relation.drop_na_rows(&columns, mode)
            } else {
                relation.drop_na_columns(&columns, mode)
            }
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
            let RuntimeValue::Scalar(TabularScalar::Integer(rows)) = parameter("rows")? else {
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
        RuntimeValue::Scalar(TabularScalar::String(value)) => Ok(value.clone()),
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
        None | Some(RuntimeValue::Scalar(TabularScalar::Null)) => None,
        Some(RuntimeValue::Record(literal)) => {
            let value = literal.get("value").ok_or(KernelError::Failed)?;
            Some(
                match literal.get("type").map(text).transpose()?.as_deref() {
                    Some("boolean") => match value {
                        RuntimeValue::Scalar(TabularScalar::Bool(value)) => {
                            FilterLiteral::Boolean(*value)
                        }
                        _ => return Err(KernelError::Failed),
                    },
                    Some("integer") => FilterLiteral::Integer(
                        text(value)?.parse().map_err(|_| KernelError::Failed)?,
                    ),
                    Some("decimal") => FilterLiteral::Decimal(
                        yss_data_contract::DecimalLiteral::new(text(value)?)
                            .map_err(|_| KernelError::InvalidParameter)?,
                    ),
                    Some("string") => FilterLiteral::String(text(value)?),
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

pub(crate) fn kernel_error(error: RelationError) -> KernelError {
    match error {
        RelationError::Cancelled => KernelError::Cancelled,
        RelationError::DeadlineExceeded => KernelError::DeadlineExceeded,
        RelationError::DivisionByZero => KernelError::DivisionByZero,
        RelationError::NonFiniteResult => KernelError::NonFiniteResult,
        RelationError::InvalidInput => KernelError::InvalidNumericInput,
        RelationError::UnalignedSeries => KernelError::UnalignedSeries,
        RelationError::MemoryLimitExceeded => KernelError::BudgetExceeded,
        RelationError::InvalidConversion => KernelError::InvalidParameter,
        _ => KernelError::Failed,
    }
}
