use super::super::{numeric_input, series};
use super::{
    Input,
    common::{
        boolean, columns, computation_error, group, integer, matrix_table, number, text, value,
    },
};
use crate::{
    KernelContract, KernelError, KernelId, KernelInvocation, KernelParameterKey,
    KernelRegistryBuilder, RuntimeValue,
};
use std::sync::Arc;
use yss_data_contract::TabularScalar;
use yss_sci_contract::{execution::*, multivariate::*};
use yss_sci_runtime::multivariate as sci;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Method {
    Pca,
    Factor,
    Canonical,
    Correspondence,
    Discriminant,
    Rda,
    Mds,
}

pub(super) fn register(builder: &mut KernelRegistryBuilder) {
    use Method::*;
    for (id, method, inputs, parameters, outputs) in [
        (
            "yssbi.statistics.multivariate.pca",
            Pca,
            vec![Input::repeated("variables", 2..=usize::MAX)],
            &["components", "standardize"][..],
            2,
        ),
        (
            "yssbi.statistics.multivariate.exploratory_factor",
            Factor,
            vec![Input::repeated("variables", 3..=usize::MAX)],
            &["components", "rotation", "max_iterations", "tolerance"][..],
            2,
        ),
        (
            "yssbi.statistics.association.canonical",
            Canonical,
            vec![
                Input::repeated("x", 1..=usize::MAX),
                Input::repeated("y", 1..=usize::MAX),
            ],
            &["components"][..],
            2,
        ),
        (
            "yssbi.statistics.multivariate.correspondence",
            Correspondence,
            vec![Input::repeated("columns", 2..=usize::MAX)],
            &["components"][..],
            3,
        ),
        (
            "yssbi.statistics.multivariate.discriminant",
            Discriminant,
            vec![
                Input::fixed("groups"),
                Input::repeated("variables", 1..=usize::MAX),
                Input::repeated("new_variables", 0..=usize::MAX),
            ],
            &["discriminant_method", "class_priors", "shrinkage"][..],
            2,
        ),
        (
            "yssbi.statistics.multivariate.rda",
            Rda,
            vec![
                Input::repeated("responses", 1..=usize::MAX),
                Input::repeated("constraints", 1..=usize::MAX),
            ],
            &["components", "standardize", "permutations", "seed"][..],
            2,
        ),
        (
            "yssbi.statistics.multivariate.mds",
            Mds,
            vec![Input::repeated("variables", 1..=usize::MAX)],
            &["components", "input_kind", "standardize"][..],
            2,
        ),
    ] {
        let contract = KernelContract::new(
            inputs,
            parameters
                .iter()
                .map(|key| KernelParameterKey::new((*key).into()).expect("parameter key")),
            outputs..=outputs,
        )
        .expect("multivariate contract");
        let contract = if method == Mds {
            contract
                .with_optional_parameters([KernelParameterKey::new("standardize".into()).unwrap()])
                .expect("conditional MDS parameter")
        } else {
            contract
        };
        builder
            .register(
                KernelId::new(id.into()).unwrap(),
                std::num::NonZeroU32::new(2).unwrap(),
                contract,
                move |inv| execute(method, inv),
            )
            .expect("distinct multivariate kernel");
    }
}

fn workspace(
    inv: &KernelInvocation<'_>,
    n: usize,
    p: usize,
    k: usize,
    dense: bool,
    retained: usize,
) -> Result<(), KernelError> {
    let bytes = (|| {
        let matrices = n
            .checked_mul(p)?
            .checked_mul(16)?
            .checked_add(p.checked_mul(p)?.checked_mul(16)?)?
            .checked_add(if dense {
                n.checked_mul(n)?.checked_mul(16)?
            } else {
                0
            })?
            .checked_mul(size_of::<f64>())?;
        let outputs = n
            .checked_mul(k)?
            .checked_mul(size_of::<RuntimeValue>() * 8)?;
        matrices
            .checked_add(outputs)?
            .checked_add(retained)?
            .checked_add(256 * 1024)
    })();
    inv.control.check_bytes(bytes).map(|_| ())
}
fn selected<'a>(inv: &'a KernelInvocation<'_>, keys: &[&str]) -> Vec<&'a RuntimeValue> {
    inv.input_keys
        .iter()
        .zip(inv.inputs)
        .filter_map(|(key, value)| keys.contains(key).then_some(value))
        .collect()
}
fn numeric(
    inputs: &[&RuntimeValue],
    inv: &KernelInvocation<'_>,
    k: usize,
    dense: bool,
) -> Result<Vec<Vec<f64>>, KernelError> {
    let inputs = inputs
        .iter()
        .map(|value| value.unannotated())
        .collect::<Vec<_>>();
    let data = columns(&inputs, inv, 0)?;
    workspace(
        inv,
        data.first().map_or(0, Vec::len),
        data.len(),
        k,
        dense,
        0,
    )?;
    Ok(data)
}

fn classification(
    inv: &KernelInvocation<'_>,
    control: &ScientificExecutionControl,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let input = selected(inv, &["groups", "variables"]);
    let typed = series::columns(&input, inv, true)?;
    let mut retained = inv.control.check_bytes(
        typed[0]
            .values
            .len()
            .checked_mul(typed.len())
            .and_then(|n| n.checked_mul(size_of::<TabularScalar>())),
    )?;
    for (i, scalar) in typed[0].values.iter().enumerate() {
        if i.is_multiple_of(1024) {
            inv.check_control()?;
        }
        if let TabularScalar::String(label) = scalar {
            retained = inv.control.check_bytes(
                label
                    .len()
                    .checked_mul(8)
                    .and_then(|bytes| retained.checked_add(bytes)),
            )?;
        }
    }
    let (groups, labels) = super::common::categories(&typed[0], false, inv)?;
    // Class-specific covariance factors and the confusion report have independent
    // dimensions; admitting only the observation matrix misses many-class inputs.
    let retained = inv.control.check_bytes((|| {
        let classes = labels.len();
        let p = typed.len().checked_sub(1)?;
        let factors = classes.checked_mul(p)?.checked_mul(p)?.checked_mul(32)?;
        let report = classes
            .checked_mul(classes)?
            .checked_add(classes.checked_mul(p)?)?
            .checked_mul(
                super::common::STRUCTURED_VALUE_BYTES * super::common::STRUCTURED_VALUE_COPIES,
            )?;
        retained.checked_add(factors)?.checked_add(report)
    })())?;
    workspace(inv, groups.len(), typed.len(), 1, false, retained)?;
    let mut variables = Vec::new();
    for column in &typed[1..] {
        let mut values = inv.control.reserve(column.values.len())?;
        for (i, scalar) in column.values.iter().enumerate() {
            if i.is_multiple_of(1024) {
                inv.check_control()?;
            }
            values.push(numeric_input(Some(&RuntimeValue::Scalar(scalar.clone())))?);
        }
        variables.push(values);
    }
    let new_inputs = group(inv, "new_variables");
    let new_variables = if new_inputs.is_empty() {
        None
    } else {
        Some(numeric(&new_inputs, inv, 1, false)?)
    };
    if let Some(data) = &new_variables {
        workspace(
            inv,
            groups.len().max(data[0].len()),
            typed.len(),
            1,
            false,
            retained,
        )?;
    }
    let options = DiscriminantOptions {
        method: match text(inv, "discriminant_method")? {
            "linear" => DiscriminantMethod::Linear,
            "quadratic" => DiscriminantMethod::Quadratic,
            _ => return Err(KernelError::InvalidParameter),
        },
        priors: match text(inv, "class_priors")? {
            "empirical" => ClassPriors::Empirical,
            "equal" => ClassPriors::Equal,
            _ => return Err(KernelError::InvalidParameter),
        },
        shrinkage: number(inv, "shrinkage")?,
    };
    let result = sci::discriminant(
        &variables,
        &groups,
        labels.len(),
        new_variables.as_deref(),
        options,
        control,
    )
    .map_err(computation_error)?;
    let mut predicted = inv.control.reserve(result.predictions.len())?;
    for (i, index) in result.predictions.into_iter().enumerate() {
        if i.is_multiple_of(1024) {
            inv.check_control()?;
        }
        predicted.push(RuntimeValue::Scalar(labels[index].clone()));
    }
    let mut predicted = RuntimeValue::List(Arc::from(predicted));
    if let Some(metadata) = &typed[0].metadata {
        predicted = predicted
            .with_metadata(metadata.clone())
            .map_err(|_| KernelError::InvalidParameter)?;
    }
    Ok(vec![
        value(
            result.report.map_classes(|class| labels[class].clone()),
            inv,
        )?,
        predicted,
    ])
}

fn execute(method: Method, inv: &KernelInvocation<'_>) -> Result<Vec<RuntimeValue>, KernelError> {
    inv.check_control()?;
    let control = ScientificExecutionControl::from_shared(
        inv.control.cancellation.clone(),
        inv.control.deadline,
    );
    if method == Method::Discriminant {
        return classification(inv, &control);
    }
    let k = integer(inv, "components")?;
    if k == 0 {
        return Err(KernelError::InvalidParameter);
    }
    let data = numeric(
        &inv.inputs.iter().collect::<Vec<_>>(),
        inv,
        if method == Method::Canonical {
            k * 2
        } else {
            k
        },
        method == Method::Mds,
    )?;
    match method {
        Method::Pca => {
            let result = sci::pca(
                &data,
                PcaOptions {
                    components: k,
                    standardize: boolean(inv, "standardize")?,
                },
                &control,
            )
            .map_err(computation_error)?;
            Ok(vec![
                value(result.report, inv)?,
                matrix_table(result.coordinates, 1, inv)?,
            ])
        }
        Method::Factor => {
            let options = FactorOptions {
                factors: k,
                rotation: match text(inv, "rotation")? {
                    "none" => FactorRotation::None,
                    "varimax" => FactorRotation::Varimax,
                    _ => return Err(KernelError::InvalidParameter),
                },
                max_iterations: integer(inv, "max_iterations")?,
                tolerance: number(inv, "tolerance")?,
            };
            let result =
                sci::exploratory_factor(&data, options, &control).map_err(computation_error)?;
            Ok(vec![
                value(result.report, inv)?,
                matrix_table(result.coordinates, 1, inv)?,
            ])
        }
        Method::Canonical => {
            let count = group(inv, "x").len();
            let result = sci::canonical_correlation(&data[..count], &data[count..], k, &control)
                .map_err(computation_error)?;
            let scores = result
                .x_scores
                .into_iter()
                .zip(result.y_scores)
                .map(|(mut x, y)| {
                    x.extend(y);
                    x
                })
                .collect();
            Ok(vec![
                value(result.report, inv)?,
                matrix_table(scores, 1, inv)?,
            ])
        }
        Method::Correspondence => {
            let result = sci::correspondence(&data, k, &control).map_err(computation_error)?;
            Ok(vec![
                value(result.report, inv)?,
                matrix_table(result.row_coordinates, 1, inv)?,
                matrix_table(result.column_coordinates, 2, inv)?,
            ])
        }
        Method::Rda => {
            let count = group(inv, "responses").len();
            let options = RdaOptions {
                components: k,
                standardize: boolean(inv, "standardize")?,
                permutations: integer(inv, "permutations")?,
                seed: integer(inv, "seed")? as u64,
            };
            let result = sci::rda(&data[..count], &data[count..], options, &control)
                .map_err(computation_error)?;
            Ok(vec![
                value(result.report, inv)?,
                matrix_table(result.coordinates, 1, inv)?,
            ])
        }
        Method::Mds => {
            let input = match text(inv, "input_kind")? {
                "observations" => MdsInput::Observations,
                "dissimilarity_matrix" => MdsInput::DissimilarityMatrix,
                _ => return Err(KernelError::InvalidParameter),
            };
            if input == MdsInput::DissimilarityMatrix && inv.parameter("standardize").is_some() {
                return Err(KernelError::InvalidParameter);
            }
            let result = sci::mds(
                &data,
                MdsOptions {
                    components: k,
                    input,
                    standardize: if input == MdsInput::Observations {
                        boolean(inv, "standardize")?
                    } else {
                        false
                    },
                },
                &control,
            )
            .map_err(computation_error)?;
            Ok(vec![
                value(result.report, inv)?,
                matrix_table(result.coordinates, 1, inv)?,
            ])
        }
        Method::Discriminant => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KernelControl, KernelOutputSpec};
    use std::{
        borrow::Cow,
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };
    use yss_data_contract::ValueType;
    #[test]
    fn multivariate_kernels_admit_dense_workspace_before_computation_and_preserve_exact_classes() {
        let relations = crate::tests::relations();
        let series = |data: &[f64]| {
            RuntimeValue::List(
                data.iter()
                    .map(|&v| RuntimeValue::float64(v).unwrap())
                    .collect(),
            )
        };
        let inputs = [series(&[1., 2., 3., 4.]), series(&[4., 1., 3., 2.])];
        let parameters = [
            ("components", TabularScalar::Integer(2).into()),
            ("standardize", TabularScalar::Bool(true).into()),
        ];
        let outputs = [
            KernelOutputSpec {
                data_type: ValueType::Struct("statistics.report".into()),
                fields: None,
            },
            KernelOutputSpec {
                data_type: ValueType::DataFrame,
                fields: None,
            },
        ];
        let control = KernelControl {
            cancellation: Arc::new(AtomicBool::new(false)),
            deadline: Instant::now() + Duration::from_secs(30),
            max_input_bytes: 1024,
        };
        let inv = KernelInvocation {
            relations: &relations,
            inputs: &inputs,
            input_keys: &["variables", "variables"],
            parameters: parameters
                .iter()
                .map(|(key, value)| {
                    (
                        KernelParameterKey::new((*key).into()).unwrap(),
                        Cow::Borrowed(value),
                    )
                })
                .collect(),
            outputs: &outputs,
            control: &control,
        };
        assert!(matches!(
            execute(Method::Pca, &inv),
            Err(KernelError::BudgetExceeded)
        ));
        let wide = [9_007_199_254_740_992i64, 9_007_199_254_740_993];
        let inputs = [
            RuntimeValue::List(
                [
                    wide[0], wide[0], wide[0], wide[0], wide[1], wide[1], wide[1], wide[1],
                ]
                .into_iter()
                .map(|v| TabularScalar::Integer(v).into())
                .collect(),
            ),
            series(&[0., 0.8, 1.4, -0.3, 4., 4.6, 5.1, 3.7]),
            series(&[1., 1.3, 0.2, -0.7, 4., 5.3, 3.8, 4.7]),
        ];
        let parameters = [
            (
                "discriminant_method",
                TabularScalar::String("linear".into()).into(),
            ),
            (
                "class_priors",
                TabularScalar::String("empirical".into()).into(),
            ),
            ("shrinkage", RuntimeValue::float64(0.0).unwrap()),
        ];
        let control = KernelControl::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(30),
        );
        let class_outputs = [
            outputs[0].clone(),
            KernelOutputSpec {
                data_type: ValueType::DataSeries(Box::new(ValueType::number())),
                fields: None,
            },
        ];
        let inv = KernelInvocation {
            relations: &relations,
            inputs: &inputs,
            input_keys: &["groups", "variables", "variables"],
            parameters: parameters
                .iter()
                .map(|(key, value)| {
                    (
                        KernelParameterKey::new((*key).into()).unwrap(),
                        Cow::Borrowed(value),
                    )
                })
                .collect(),
            outputs: &class_outputs,
            control: &control,
        };
        let result = execute(Method::Discriminant, &inv).unwrap();
        assert_eq!(result[1], inputs[0]);
        control
            .cancellation
            .store(true, std::sync::atomic::Ordering::Release);
        assert!(matches!(
            execute(Method::Discriminant, &inv),
            Err(KernelError::Cancelled)
        ));
    }
}
