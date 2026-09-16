mod numeric;
mod relational;
mod statistics;

use crate::{KernelError, KernelInvocation, RuntimeValue};
use yss_relational_contract::NumericOperation;

#[derive(Clone, Copy)]
enum BuiltinKernel {
    Statistical(statistics::StatisticalKernel),
    Relational(relational::RelationalKernel),
    Decompose,
    Constant,
    Numeric(NumericOperation),
    And,
    Or,
    Not,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Convert,
    Identity,
}

pub(crate) fn register_builtin_kernels(builder: &mut crate::KernelRegistryBuilder) {
    use BuiltinKernel::*;
    use NumericOperation::{Add, Divide, Multiply, Subtract};
    use statistics::StatisticalKernel::{OlsFit, OlsSummary};
    let entries: &[(
        &str,
        BuiltinKernel,
        &[&str],
        std::ops::RangeInclusive<usize>,
    )] = &[
        (
            "yssbi.statistics.ols.fit",
            Statistical(OlsFit),
            &["configuration"],
            3..=3,
        ),
        (
            "yssbi.statistics.ols.summary",
            Statistical(OlsSummary),
            &["configuration"],
            2..=2,
        ),
        (
            "yssbi.dataframe.source.get",
            Relational(relational::RelationalKernel::Source),
            &["dataframe"],
            1..=1,
        ),
        (
            "yssbi.dataframe.project",
            Relational(relational::RelationalKernel::Project),
            &["columns"],
            1..=1,
        ),
        (
            "yssbi.dataframe.filter.rows",
            Relational(relational::RelationalKernel::Filter),
            &["predicate"],
            1..=1,
        ),
        (
            "yssbi.dataframe.series.select",
            Relational(relational::RelationalKernel::Series),
            &["column"],
            1..=1,
        ),
        ("yssbi.dataframe.decompose", Decompose, &[], 0..=usize::MAX),
        (
            "yssbi.dataframe.limit",
            Relational(relational::RelationalKernel::Limit),
            &["rows"],
            1..=1,
        ),
        (
            "yssbi.dataframe.rename",
            Relational(relational::RelationalKernel::Rename),
            &["from", "to"],
            1..=1,
        ),
        ("yssbi.constant.get", Constant, &["value"], 1..=1),
        ("yssbi.numeric.add", Numeric(Add), &[], 1..=1),
        ("yssbi.numeric.subtract", Numeric(Subtract), &[], 1..=1),
        ("yssbi.numeric.multiply", Numeric(Multiply), &[], 1..=1),
        ("yssbi.numeric.divide", Numeric(Divide), &[], 1..=1),
        ("yssbi.logic.and", And, &[], 1..=1),
        ("yssbi.logic.or", Or, &[], 1..=1),
        ("yssbi.logic.not", Not, &[], 1..=1),
        ("yssbi.compare.equal", Equal, &[], 1..=1),
        ("yssbi.compare.not_equal", NotEqual, &[], 1..=1),
        ("yssbi.compare.less", Less, &[], 1..=1),
        ("yssbi.compare.less_equal", LessEqual, &[], 1..=1),
        ("yssbi.compare.greater", Greater, &[], 1..=1),
        ("yssbi.compare.greater_equal", GreaterEqual, &[], 1..=1),
        ("yssbi.value.convert", Convert, &["target_type"], 1..=1),
        ("yssbi.debug.view", Identity, &[], 0..=0),
        ("yssbi.core.reroute", Identity, &[], 1..=1),
    ];
    for (id, kind, parameters, outputs) in entries {
        let kind = *kind;
        let contract = crate::KernelContract::new(
            parameters.iter().map(|field| {
                crate::KernelParameterKey::new((*field).into())
                    .expect("built-in parameter identity")
            }),
            outputs.clone(),
        )
        .expect("built-in kernel contract");
        builder
            .register(
                crate::KernelId::new((*id).into()).expect("built-in kernel identity"),
                std::num::NonZeroU32::new(match kind {
                    Equal | NotEqual | Less | LessEqual | Greater | GreaterEqual => 3,
                    Statistical(OlsFit | OlsSummary) => 2,
                    Numeric(_) | Convert | Relational(relational::RelationalKernel::Filter) => 2,
                    _ => 1,
                })
                .expect("built-in implementation revision"),
                contract,
                move |invocation| execute_kernel(kind, invocation),
            )
            .expect("built-in kernels have distinct identities");
    }
}

fn execute_kernel(
    kind: BuiltinKernel,
    invocation: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let inputs = invocation.inputs;
    let outputs = invocation.outputs;
    let value = match kind {
        BuiltinKernel::Statistical(kind) => {
            return statistics::execute(kind, invocation);
        }
        BuiltinKernel::Relational(kind) => relational::execute(kind, invocation),
        BuiltinKernel::Decompose => return relational::decompose(invocation),
        BuiltinKernel::Constant => invocation
            .parameter("value")
            .cloned()
            .ok_or(KernelError::Failed),
        BuiltinKernel::Numeric(operation) => numeric::execute(operation, invocation),
        BuiltinKernel::And => binary_bool(inputs, |left, right| left && right),
        BuiltinKernel::Or => binary_bool(inputs, |left, right| left || right),
        BuiltinKernel::Not => unary_bool(inputs, |value| !value),
        BuiltinKernel::Equal | BuiltinKernel::NotEqual => {
            let [left, right] = inputs else {
                return Err(KernelError::Failed);
            };
            let equal = left.semantic_eq(right);
            Ok(RuntimeValue::Bool(
                if matches!(kind, BuiltinKernel::Equal) {
                    equal
                } else {
                    !equal
                },
            ))
        }
        BuiltinKernel::Less
        | BuiltinKernel::LessEqual
        | BuiltinKernel::Greater
        | BuiltinKernel::GreaterEqual => compare_numeric(kind, inputs),
        BuiltinKernel::Convert => {
            let target = outputs
                .first()
                .map(|output| &output.data_type)
                .ok_or(KernelError::Failed)?;
            inputs
                .first()
                .cloned()
                .ok_or(KernelError::Failed)?
                .coerce_to(target)
                .map_err(|_| KernelError::Failed)
        }
        BuiltinKernel::Identity if outputs.is_empty() => return Ok(Vec::new()),
        BuiltinKernel::Identity => inputs.first().cloned().ok_or(KernelError::Failed),
    }?;
    let [_output] = outputs else {
        return Err(KernelError::Failed);
    };
    Ok(vec![value])
}

pub(crate) fn numeric_input(value: Option<&RuntimeValue>) -> Result<f64, KernelError> {
    match value {
        Some(RuntimeValue::Integer(value)) if (*value as f64) as i128 == i128::from(*value) => {
            Ok(*value as f64)
        }
        Some(RuntimeValue::Unsigned(value)) if (*value as f64) as u128 == u128::from(*value) => {
            Ok(*value as f64)
        }
        Some(RuntimeValue::Decimal(value)) if value.is_finite() => Ok(*value),
        _ => Err(KernelError::InvalidNumericInput),
    }
}

fn binary_bool(
    inputs: &[RuntimeValue],
    operation: impl FnOnce(bool, bool) -> bool,
) -> Result<RuntimeValue, KernelError> {
    let Some(RuntimeValue::Bool(left)) = inputs.first() else {
        return Err(KernelError::Failed);
    };
    let Some(RuntimeValue::Bool(right)) = inputs.get(1) else {
        return Err(KernelError::Failed);
    };
    Ok(RuntimeValue::Bool(operation(*left, *right)))
}

fn unary_bool(
    inputs: &[RuntimeValue],
    operation: impl FnOnce(bool) -> bool,
) -> Result<RuntimeValue, KernelError> {
    let Some(RuntimeValue::Bool(value)) = inputs.first() else {
        return Err(KernelError::Failed);
    };
    Ok(RuntimeValue::Bool(operation(*value)))
}

fn compare_numeric(
    kind: BuiltinKernel,
    inputs: &[RuntimeValue],
) -> Result<RuntimeValue, KernelError> {
    let [left, right] = inputs else {
        return Err(KernelError::InvalidNumericInput);
    };
    let ordering = left
        .numeric_cmp(right)
        .ok_or(KernelError::InvalidNumericInput)?;
    let value = match kind {
        BuiltinKernel::Less => ordering.is_lt(),
        BuiltinKernel::LessEqual => ordering.is_le(),
        BuiltinKernel::Greater => ordering.is_gt(),
        BuiltinKernel::GreaterEqual => ordering.is_ge(),
        _ => return Err(KernelError::Failed),
    };
    Ok(RuntimeValue::Bool(value))
}
