use super::relational::{kernel_error, materialize};
use crate::{
    KernelContract, KernelError, KernelField, KernelId, KernelInputSpec, KernelInvocation,
    KernelParameterKey, KernelRegistryBuilder, RuntimeValue,
};
use yss_data_contract::{
    SemanticType, TabularScalar, ValueType,
    aggregation::{AggregateOperation, ColumnAggregate},
};
use yss_relational_contract::RelationHandle;

#[derive(Clone, Copy)]
enum Operation {
    Frequency,
    SeriesDescribe,
    Describe,
    GroupBy,
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
            "yssbi.dataframe.series.describe",
            Operation::SeriesDescribe,
            "series",
            vec![],
        ),
        (
            "yssbi.dataframe.describe",
            Operation::Describe,
            "source",
            vec!["describe_columns"],
        ),
        (
            "yssbi.dataframe.groupby",
            Operation::GroupBy,
            "source",
            std::iter::once("keys")
                .chain(AggregateOperation::ALL.map(|op| op.key()))
                .collect(),
        ),
    ] {
        builder
            .register(
                KernelId::new(id.into()).expect("kernel id"),
                std::num::NonZeroU32::new(1).unwrap(),
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
                if !s.is_empty() && s.trim() == s.as_ref() && seen.insert(s.as_ref()) =>
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
            relation.frequency(relation.schema().field(0).name(), *include_null)
        }
        Operation::SeriesDescribe => series_relation(input, inv)?.describe(&[]),
        Operation::Describe => {
            let RuntimeValue::Relation(relation) = input else {
                return Err(KernelError::InvalidParameter);
            };
            relation.describe(&names(inv, "describe_columns")?)
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
            relation.aggregate(&names(inv, "keys")?, &columns)
        }
    }
    .map_err(kernel_error)?;
    inv.check_control()?;
    Ok(vec![RuntimeValue::Relation(result)])
}
