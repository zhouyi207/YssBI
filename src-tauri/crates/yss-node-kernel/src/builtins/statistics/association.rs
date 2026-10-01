use super::{
    Input,
    common::{boolean, columns, integer, number, text, value},
};
use crate::{
    KernelContract, KernelError, KernelId, KernelInvocation, KernelParameterKey,
    KernelRegistryBuilder, RuntimeValue,
};
use std::collections::BTreeMap;
use yss_data_contract::{SemanticType, TabularScalar};
use yss_sci_contract::association::*;
use yss_sci_contract::execution::{
    ScientificComputationError, ScientificExecutionControl, ScientificInputViolation,
};
use yss_sci_contract::hypothesis::Alternative;
use yss_sci_runtime::association as sci;

#[derive(Clone, Copy)]
enum Method {
    Pearson,
    Partial,
    Spearman,
    Kendall,
    Kappa,
    Icc,
    BlandAltman,
    KendallW,
    Ridit,
    Rwg,
}

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    use Method::*;
    for (id, method, inputs, parameters, optional) in [
        (
            "association.pearson",
            Pearson,
            vec![Input::fixed("x"), Input::fixed("y")],
            &["alternative", "confidence_level"][..],
            &[][..],
        ),
        (
            "association.partial",
            Partial,
            vec![
                Input::fixed("x"),
                Input::fixed("y"),
                Input::repeated("controls", 1..=usize::MAX),
            ],
            &["alternative", "confidence_level"][..],
            &[][..],
        ),
        (
            "association.spearman",
            Spearman,
            vec![Input::fixed("x"), Input::fixed("y")],
            &["alternative", "p_value_method"][..],
            &[][..],
        ),
        (
            "association.kendall",
            Kendall,
            vec![Input::fixed("x"), Input::fixed("y")],
            &["alternative", "p_value_method"][..],
            &[][..],
        ),
        (
            "test.kappa",
            Kappa,
            vec![Input::repeated("ratings", 2..=usize::MAX)],
            &["kappa_method", "kappa_weighting", "confidence_level"][..],
            &["kappa_weighting"][..],
        ),
        (
            "association.icc",
            Icc,
            vec![Input::repeated("ratings", 2..=usize::MAX)],
            &["icc_type", "confidence_level"][..],
            &[][..],
        ),
        (
            "association.bland_altman",
            BlandAltman,
            vec![Input::fixed("x"), Input::fixed("y")],
            &["coverage", "confidence_level"][..],
            &[][..],
        ),
        (
            "test.kendall_w",
            KendallW,
            vec![Input::repeated("ratings", 2..=usize::MAX)],
            &[][..],
            &[][..],
        ),
        (
            "association.ridit",
            Ridit,
            vec![Input::fixed("sample"), Input::fixed("reference")],
            &["alternative", "continuity_correction"][..],
            &[][..],
        ),
        (
            "association.rwg",
            Rwg,
            vec![Input::repeated("items", 1..=usize::MAX)],
            &["null_distribution", "scale_points", "expected_variance"][..],
            &["scale_points", "expected_variance"][..],
        ),
    ] {
        let contract = KernelContract::new(
            inputs,
            parameters
                .iter()
                .map(|key| KernelParameterKey::new((*key).into()).expect("parameter")),
            1..=1,
        )
        .expect("association contract")
        .with_optional_parameters(
            optional
                .iter()
                .map(|key| KernelParameterKey::new((*key).into()).expect("optional parameter")),
        )
        .expect("association optional parameters");
        builder
            .register(
                KernelId::new(format!("yssbi.statistics.{id}").into()).expect("association ID"),
                std::num::NonZeroU32::new(1).unwrap(),
                contract,
                move |inv| execute(method, inv),
            )
            .expect("unique association kernel");
    }
}

fn error(error: ScientificComputationError) -> KernelError {
    match error {
        ScientificComputationError::Cancelled => KernelError::Cancelled,
        ScientificComputationError::DeadlineExceeded => KernelError::DeadlineExceeded,
        ScientificComputationError::InvalidInput {
            violation: ScientificInputViolation::ShapeMismatch,
        } => KernelError::ShapeMismatch,
        ScientificComputationError::InvalidInput { .. } => KernelError::InvalidNumericInput,
        ScientificComputationError::ComputationFailed => KernelError::ScientificFailure,
    }
}
fn alternative(inv: &KernelInvocation<'_>) -> Result<Alternative, KernelError> {
    match text(inv, "alternative")? {
        "two_sided" => Ok(Alternative::TwoSided),
        "greater" => Ok(Alternative::Greater),
        "less" => Ok(Alternative::Less),
        _ => Err(KernelError::InvalidParameter),
    }
}
fn rank_method(inv: &KernelInvocation<'_>) -> Result<RankInference, KernelError> {
    match text(inv, "p_value_method")? {
        "auto" => Ok(RankInference::Auto),
        "permutation_exact" => Ok(RankInference::PermutationExact),
        "asymptotic" => Ok(RankInference::Asymptotic),
        _ => Err(KernelError::InvalidParameter),
    }
}
fn check_workspace(
    inv: &KernelInvocation<'_>,
    rows: usize,
    count: usize,
) -> Result<(), KernelError> {
    inv.control
        .check_bytes(
            rows.checked_mul(count.max(2))
                .and_then(|n| n.checked_mul(16 * size_of::<f64>())),
        )
        .map(|_| ())
}
fn numeric_columns(inv: &KernelInvocation<'_>) -> Result<Vec<Vec<f64>>, KernelError> {
    let columns = columns(&inv.inputs.iter().collect::<Vec<_>>(), inv, 0)?;
    check_workspace(inv, columns.first().map_or(0, Vec::len), columns.len())?;
    Ok(columns)
}
fn typed_columns(
    inv: &KernelInvocation<'_>,
    aligned: bool,
) -> Result<Vec<super::super::series::Column>, KernelError> {
    use super::super::series;
    if aligned {
        if inv
            .inputs
            .iter()
            .all(|value| matches!(value.unannotated(), RuntimeValue::Series(_)))
        {
            let handles = inv
                .inputs
                .iter()
                .map(|value| match value.unannotated() {
                    RuntimeValue::Series(handle) => handle.clone(),
                    _ => unreachable!(),
                })
                .collect::<Vec<_>>();
            let output = series::load(&handles, inv)?;
            check_typed_budget(inv, &output)?;
            return Ok(output);
        }
        let Some(RuntimeValue::List(first)) = inv.inputs.first().map(RuntimeValue::unannotated)
        else {
            return Err(KernelError::UnalignedSeries);
        };
        for input in inv.inputs {
            let RuntimeValue::List(values) = input.unannotated() else {
                return Err(KernelError::UnalignedSeries);
            };
            if values.len() != first.len() {
                return Err(KernelError::ShapeMismatch);
            }
        }
        inv.control.check_bytes(
            first
                .len()
                .checked_mul(inv.inputs.len())
                .and_then(|n| n.checked_mul(size_of::<RuntimeValue>() * 8)),
        )?;
    }
    let output = inv
        .inputs
        .iter()
        .map(|input| series::column(input, inv))
        .collect::<Result<Vec<_>, _>>()?;
    check_typed_budget(inv, &output)?;
    Ok(output)
}
fn check_typed_budget(
    inv: &KernelInvocation<'_>,
    columns: &[super::super::series::Column],
) -> Result<(), KernelError> {
    let mut bytes = 0usize;
    for column in columns {
        bytes = inv.control.check_bytes(
            column
                .values
                .len()
                .checked_mul(size_of::<RuntimeValue>() * 8)
                .and_then(|n| bytes.checked_add(n)),
        )?;
        for (i, value) in column.values.iter().enumerate() {
            if i.is_multiple_of(1024) {
                inv.check_control()?;
            }
            if let TabularScalar::String(value) = value {
                bytes = inv.control.check_bytes(
                    value
                        .len()
                        .checked_mul(4)
                        .and_then(|n| bytes.checked_add(n)),
                )?;
            }
        }
    }
    Ok(())
}
fn scalar_text(value: &TabularScalar) -> Result<String, KernelError> {
    Ok(match value {
        TabularScalar::Null => return Err(KernelError::InvalidNumericInput),
        TabularScalar::String(s) => s.to_string(),
        TabularScalar::Bool(v) => v.to_string(),
        TabularScalar::Integer(v) => v.to_string(),
        TabularScalar::Unsigned(v) => v.to_string(),
        TabularScalar::Float64(v) => v.as_f64().to_string(),
    })
}
fn declared_order(column: &super::super::series::Column) -> Option<Vec<String>> {
    column
        .metadata
        .as_ref()
        .filter(|metadata| metadata.semantic.kind == SemanticType::Ordinal)
        .map(|metadata| {
            metadata
                .semantic
                .values
                .iter()
                .map(|value| value.value.clone())
                .collect()
        })
}
fn rank_columns(inv: &KernelInvocation<'_>) -> Result<Vec<Vec<f64>>, KernelError> {
    let input = typed_columns(inv, true)?;
    let mut output = Vec::with_capacity(input.len());
    for column in input {
        let order = declared_order(&column);
        let mapping = order.as_ref().map(|order| {
            order
                .iter()
                .enumerate()
                .map(|(i, value)| (value.as_str(), i))
                .collect::<BTreeMap<_, _>>()
        });
        let mut values = inv.control.reserve(column.values.len())?;
        for (i, value) in column.values.into_iter().enumerate() {
            if i.is_multiple_of(1024) {
                inv.check_control()?;
            }
            values.push(if let Some(mapping) = &mapping {
                *mapping
                    .get(scalar_text(&value)?.as_str())
                    .ok_or(KernelError::InvalidNumericInput)? as f64
            } else {
                super::super::numeric_input(Some(&RuntimeValue::Scalar(value)))?
            });
        }
        output.push(values);
    }
    check_workspace(inv, output.first().map_or(0, Vec::len), output.len())?;
    Ok(output)
}
fn parse_like(code: &str, example: &TabularScalar) -> Result<TabularScalar, KernelError> {
    Ok(match example {
        TabularScalar::String(_) => TabularScalar::String(code.into()),
        TabularScalar::Bool(_) => {
            TabularScalar::Bool(code.parse().map_err(|_| KernelError::InvalidNumericInput)?)
        }
        TabularScalar::Integer(_) => {
            TabularScalar::Integer(code.parse().map_err(|_| KernelError::InvalidNumericInput)?)
        }
        TabularScalar::Unsigned(_) => {
            TabularScalar::Unsigned(code.parse().map_err(|_| KernelError::InvalidNumericInput)?)
        }
        TabularScalar::Float64(_) => TabularScalar::Float64(
            code.parse::<f64>()
                .map_err(|_| KernelError::InvalidNumericInput)?
                .try_into()
                .map_err(|_| KernelError::InvalidNumericInput)?,
        ),
        TabularScalar::Null => return Err(KernelError::InvalidNumericInput),
    })
}
fn encode_categories(
    inv: &KernelInvocation<'_>,
    columns: &[super::super::series::Column],
    ordered: bool,
) -> Result<(Vec<Vec<usize>>, Vec<TabularScalar>), KernelError> {
    let orders = columns.iter().map(declared_order).collect::<Vec<_>>();
    let declared = if ordered && orders.iter().any(Option::is_some) {
        let Some(first) = &orders[0] else {
            return Err(KernelError::InvalidParameter);
        };
        if first.is_empty() || orders.iter().any(|order| order.as_ref() != Some(first)) {
            return Err(KernelError::InvalidParameter);
        }
        Some(first)
    } else {
        None
    };
    let mut labels = Vec::<TabularScalar>::new();
    if let Some(order) = declared {
        let example = columns[0]
            .values
            .iter()
            .find(|value| !matches!(value, TabularScalar::Null))
            .ok_or(KernelError::InvalidNumericInput)?;
        labels = order
            .iter()
            .map(|code| parse_like(code, example))
            .collect::<Result<_, _>>()?;
    } else {
        for column in columns {
            for (i, scalar) in column.values.iter().enumerate() {
                if i.is_multiple_of(1024) {
                    inv.check_control()?;
                }
                if matches!(scalar, TabularScalar::Null) {
                    return Err(KernelError::InvalidNumericInput);
                }
                if !labels
                    .iter()
                    .any(|label| label.compare(scalar) == Some(std::cmp::Ordering::Equal))
                {
                    labels.push(scalar.clone());
                }
            }
        }
        if ordered {
            if labels
                .iter()
                .any(|label| matches!(label, TabularScalar::String(_)))
            {
                return Err(KernelError::InvalidParameter);
            }
            if labels
                .first()
                .is_some_and(|first| labels.iter().any(|label| label.compare(first).is_none()))
            {
                return Err(KernelError::InvalidParameter);
            }
            labels.sort_by(|a, b| a.compare(b).expect("comparable ordered categories"));
        }
    }
    let mapping = declared.map(|order| {
        order
            .iter()
            .enumerate()
            .map(|(i, code)| (code.as_str(), i))
            .collect::<BTreeMap<_, _>>()
    });
    let mut output = Vec::with_capacity(columns.len());
    for column in columns {
        let mut codes = inv.control.reserve(column.values.len())?;
        for (i, scalar) in column.values.iter().enumerate() {
            if i.is_multiple_of(1024) {
                inv.check_control()?;
            }
            let index = if let Some(mapping) = &mapping {
                *mapping
                    .get(scalar_text(scalar)?.as_str())
                    .ok_or(KernelError::InvalidNumericInput)?
            } else {
                labels
                    .iter()
                    .position(|label| label.compare(scalar) == Some(std::cmp::Ordering::Equal))
                    .ok_or(KernelError::InvalidNumericInput)?
            };
            codes.push(index);
        }
        output.push(codes);
    }
    let label_bytes = labels.iter().try_fold(0usize, |n, label| {
        n.checked_add(scalar_text(label).ok()?.len())
    });
    inv.control.check_bytes(
        labels
            .len()
            .checked_mul(labels.len())
            .and_then(|n| n.checked_mul(1024))
            .and_then(|n| n.checked_add(label_bytes?.checked_mul(4)?)),
    )?;
    Ok((output, labels))
}

fn execute(method: Method, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    let control = ScientificExecutionControl::from_shared(
        inv.control.cancellation.clone(),
        inv.control.deadline,
    );
    use Method::*;
    let result = match method {
        Pearson | Partial => {
            let data = numeric_columns(inv)?;
            let options = CorrelationOptions {
                alternative: alternative(inv)?,
                confidence_level: number(inv, "confidence_level")?,
            };
            value(
                if matches!(method, Pearson) {
                    sci::pearson(&data[0], &data[1], options, &control)
                } else {
                    sci::partial(&data[0], &data[1], &data[2..], options, &control)
                }
                .map_err(error)?,
                inv,
            )?
        }
        Spearman | Kendall => {
            let data = rank_columns(inv)?;
            let inference = rank_method(inv)?;
            if inference == RankInference::PermutationExact
                && data[0].len() > MAX_EXACT_RANK_OBSERVATIONS
            {
                return Err(KernelError::InvalidParameter);
            }
            let options = RankCorrelationOptions {
                alternative: alternative(inv)?,
                inference,
            };
            value(
                if matches!(method, Spearman) {
                    sci::spearman(&data[0], &data[1], options, &control)
                } else {
                    sci::kendall(&data[0], &data[1], options, &control)
                }
                .map_err(error)?,
                inv,
            )?
        }
        Kappa => {
            let kind = match text(inv, "kappa_method")? {
                "cohen" => KappaMethod::Cohen,
                "fleiss" => KappaMethod::Fleiss,
                _ => return Err(KernelError::InvalidParameter),
            };
            if kind == KappaMethod::Fleiss && inv.parameter("kappa_weighting").is_some() {
                return Err(KernelError::InvalidParameter);
            }
            let weighting = if kind == KappaMethod::Fleiss {
                KappaWeighting::None
            } else {
                match text(inv, "kappa_weighting")? {
                    "none" => KappaWeighting::None,
                    "linear" => KappaWeighting::Linear,
                    "quadratic" => KappaWeighting::Quadratic,
                    _ => return Err(KernelError::InvalidParameter),
                }
            };
            let columns = typed_columns(inv, true)?;
            let (ratings, labels) =
                encode_categories(inv, &columns, weighting != KappaWeighting::None)?;
            // Cohen's contingency table and weights grow with categories squared;
            // Fleiss keeps rater/category counts instead of a dense category table.
            inv.control.check_bytes((|| {
                let categories = labels.len();
                let rows = ratings.first()?.len();
                let input = rows
                    .checked_mul(ratings.len())?
                    .checked_mul(size_of::<RuntimeValue>() * 8)?;
                let counts = categories.checked_mul(ratings.len())?.checked_mul(8)?;
                let table = if kind == KappaMethod::Cohen {
                    categories.checked_mul(categories)?.checked_mul(
                        super::common::STRUCTURED_VALUE_BYTES
                            * super::common::STRUCTURED_VALUE_COPIES
                            + 32,
                    )?
                } else {
                    0
                };
                input
                    .checked_add(counts)?
                    .checked_add(table)?
                    .checked_add(categories.checked_mul(1024)?)
            })())?;
            let options = KappaOptions {
                method: kind,
                weighting,
                confidence_level: number(inv, "confidence_level")?,
            };
            value(
                sci::kappa(&ratings, labels.len(), options, &control)
                    .map_err(error)?
                    .map_categories(|category| labels[category].clone()),
                inv,
            )?
        }
        Icc => {
            let data = numeric_columns(inv)?;
            let kind = match text(inv, "icc_type")? {
                "ICC1" => IccType::Icc1,
                "ICC2" => IccType::Icc2,
                "ICC3" => IccType::Icc3,
                "ICC1k" => IccType::Icc1k,
                "ICC2k" => IccType::Icc2k,
                "ICC3k" => IccType::Icc3k,
                _ => return Err(KernelError::InvalidParameter),
            };
            value(
                sci::icc(&data, kind, number(inv, "confidence_level")?, &control).map_err(error)?,
                inv,
            )?
        }
        BlandAltman => {
            let data = numeric_columns(inv)?;
            value(
                sci::bland_altman(
                    &data[0],
                    &data[1],
                    number(inv, "coverage")?,
                    number(inv, "confidence_level")?,
                    &control,
                )
                .map_err(error)?,
                inv,
            )?
        }
        KendallW => value(
            sci::kendall_w(&rank_columns(inv)?, &control).map_err(error)?,
            inv,
        )?,
        Ridit => {
            let columns = typed_columns(inv, false)?;
            let (codes, labels) = encode_categories(inv, &columns, true)?;
            inv.control.check_bytes(labels.len().checked_mul(8192))?;
            value(
                sci::ridit(
                    &codes[0],
                    &codes[1],
                    labels.len(),
                    alternative(inv)?,
                    boolean(inv, "continuity_correction")?,
                    &control,
                )
                .map_err(error)?
                .map_categories(|category| labels[category].clone()),
                inv,
            )?
        }
        Rwg => {
            let data = numeric_columns(inv)?;
            if (text(inv, "null_distribution")? == "uniform"
                && inv.parameter("expected_variance").is_some())
                || (text(inv, "null_distribution")? == "specified_variance"
                    && inv.parameter("scale_points").is_some())
            {
                return Err(KernelError::InvalidParameter);
            }
            let null = match text(inv, "null_distribution")? {
                "uniform" => AgreementNull::Uniform {
                    scale_points: integer(inv, "scale_points")?,
                },
                "specified_variance" => AgreementNull::SpecifiedVariance {
                    variance: number(inv, "expected_variance")?,
                },
                _ => return Err(KernelError::InvalidParameter),
            };
            value(sci::rwg(&data, null, &control).map_err(error)?, inv)?
        }
    };
    Ok(vec![result])
}
