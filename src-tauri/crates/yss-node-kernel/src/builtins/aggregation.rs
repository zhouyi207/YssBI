use super::relational::{kernel_error, materialize};
use crate::{
    KernelContract, KernelError, KernelField, KernelId, KernelInputSpec, KernelInvocation,
    KernelParameterKey, KernelRegistryBuilder, RuntimeValue,
};
use yss_data_contract::{
    SemanticType, TabularColumnName, TabularScalar, ValueType,
    aggregation::{AggregateOperation, ColumnAggregate},
};
use yss_relational_contract::RelationHandle;

mod description;

#[derive(Clone, Copy)]
enum Operation {
    Frequency,
    Describe,
    GroupBy,
    Groups,
}

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    for (id, operation, input, parameters) in [
        (
            "yssbi.dataframe.series.frequency",
            Operation::Frequency,
            "series",
            vec!["include_null"],
        ),
        (
            "yssbi.statistics.describe",
            Operation::Describe,
            "source",
            vec![],
        ),
        (
            "yssbi.dataframe.groupby",
            Operation::GroupBy,
            "source",
            std::iter::once("keys")
                .chain(AggregateOperation::ALL.map(|op| op.key()))
                .collect(),
        ),
        (
            "yssbi.dataframe.groupby.groups",
            Operation::Groups,
            "source",
            vec!["keys"],
        ),
    ] {
        builder
            .register(
                KernelId::new(id.into()).expect("kernel id"),
                std::num::NonZeroU32::new(match operation {
                    Operation::Describe => 5,
                    Operation::GroupBy => 2,
                    _ => 1,
                })
                .unwrap(),
                KernelContract::new(
                    [KernelInputSpec::fixed(input)],
                    parameters
                        .into_iter()
                        .map(|p| KernelParameterKey::new(p.into()).unwrap()),
                    1..=1,
                )
                .unwrap(),
                move |inv| execute(operation, inv),
            )
            .expect("distinct aggregate kernel");
    }
}

fn names(inv: &KernelInvocation<'_>, key: &str) -> Result<Vec<Box<str>>, KernelError> {
    let Some(RuntimeValue::List(values)) = inv.parameter(key) else {
        return Err(KernelError::InvalidParameter);
    };
    let mut seen = std::collections::BTreeSet::new();
    values
        .iter()
        .map(|v| match v {
            RuntimeValue::Scalar(TabularScalar::String(s))
                if TabularColumnName::is_valid(s) && seen.insert(s.as_ref()) =>
            {
                Ok(s.clone())
            }
            _ => Err(KernelError::InvalidParameter),
        })
        .collect()
}

fn series_relation(
    value: &RuntimeValue,
    inv: &KernelInvocation<'_>,
) -> Result<RelationHandle, KernelError> {
    if let RuntimeValue::Series(series) = value {
        return series
            .relation()
            .project_series(std::slice::from_ref(series))
            .map_err(kernel_error);
    }
    let RuntimeValue::List(values) = value.unannotated() else {
        return Err(KernelError::InvalidParameter);
    };
    let kind = value
        .metadata()
        .map(|m| m.semantic.kind)
        .unwrap_or_else(|| {
            values
                .iter()
                .find_map(|v| match v {
                    RuntimeValue::Scalar(TabularScalar::Null) => None,
                    RuntimeValue::Scalar(TabularScalar::Bool(_)) => Some(SemanticType::Binary),
                    RuntimeValue::Scalar(TabularScalar::String(_)) => Some(SemanticType::Text),
                    _ => Some(SemanticType::Numeric),
                })
                .unwrap_or(SemanticType::Numeric)
        });
    let RuntimeValue::Relation(relation) = materialize(
        &[KernelField {
            name: "value".into(),
            data_type: ValueType::Scalar(kind),
        }],
        &[value],
        inv,
    )?
    else {
        unreachable!()
    };
    Ok(relation)
}

fn execute(
    operation: Operation,
    inv: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    inv.check_control()?;
    let [input] = inv.inputs else {
        return Err(KernelError::InputLayoutMismatch);
    };
    let result = match operation {
        Operation::Frequency => {
            let Some(RuntimeValue::Scalar(TabularScalar::Bool(include_null))) =
                inv.parameter("include_null")
            else {
                return Err(KernelError::InvalidParameter);
            };
            let relation = series_relation(input, inv)?;
            RuntimeValue::Relation(
                relation
                    .frequency(relation.schema().field(0).name(), *include_null)
                    .map_err(kernel_error)?,
            )
        }
        Operation::Describe => {
            let relation = match input {
                RuntimeValue::Relation(relation) => relation.clone(),
                _ => series_relation(input, inv)?,
            };
            description::describe(&relation, inv)?
        }
        Operation::Groups => {
            let RuntimeValue::Relation(relation) = input else {
                return Err(KernelError::InputLayoutMismatch);
            };
            RuntimeValue::Grouped(std::sync::Arc::new(
                yss_relational_contract::GroupedRelationHandle::new(
                    relation.clone(),
                    names(inv, "keys")?.into(),
                )
                .map_err(kernel_error)?,
            ))
        }
        Operation::GroupBy => {
            let RuntimeValue::Relation(relation) = input else {
                return Err(KernelError::InvalidParameter);
            };
            let mut columns = Vec::new();
            for operation in AggregateOperation::ALL {
                columns.extend(
                    names(inv, operation.key())?
                        .into_iter()
                        .map(|column| ColumnAggregate { column, operation }),
                );
            }
            RuntimeValue::Relation(
                relation
                    .aggregate(&names(inv, "keys")?, &columns)
                    .map_err(kernel_error)?,
            )
        }
    };
    inv.check_control()?;
    Ok(vec![result])
}
