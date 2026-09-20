use super::numeric_input;
use crate::{KernelError, KernelInvocation, RuntimeValue};
use std::collections::BTreeMap;
use std::sync::Arc;
use yss_data_contract::TabularScalar;
use yss_sci_contract::regression::{OlsCovariance, OlsOptions};
use yss_sci_contract::scientific::{
    LinearRegressionMethod, LinearRegressionRequest, ScientificComputationError,
    ScientificExecutionControl, ScientificInputViolation,
};

#[derive(Clone, Copy)]
pub(crate) enum StatisticalKernel {
    LinearFit,
    LinearRegressionSummary,
    LinearPredict,
}

pub(crate) fn execute(
    kind: StatisticalKernel,
    invocation: &KernelInvocation<'_>,
) -> Result<Vec<RuntimeValue>, KernelError> {
    invocation.check_control()?;
    let values = match kind {
        StatisticalKernel::LinearRegressionSummary => {
            let Some(RuntimeValue::LinearRegression(model)) = invocation.inputs.first() else {
                return Err(KernelError::InvalidNumericInput);
            };
            vec![
                RuntimeValue::LinearRegression(model.clone()),
                RuntimeValue::LinearRegression(model.clone()),
            ]
        }
        StatisticalKernel::LinearPredict => {
            let Some(RuntimeValue::LinearRegression(model)) = invocation.inputs.first() else {
                return Err(KernelError::InvalidNumericInput);
            };
            let predictors = columns(&group(invocation, "predictors"), invocation, 0)?;
            if predictors.is_empty()
                || predictors.len() + usize::from(model.constant) != model.coefficients.len()
            {
                return Err(KernelError::InvalidNumericInput);
            }
            let n = predictors[0].len();
            invocation.control.check_bytes(
                n.checked_mul(predictors.len())
                    .and_then(|v| v.checked_mul(size_of::<f64>()))
                    .and_then(|v| v.checked_add(n.checked_mul(size_of::<RuntimeValue>())?)),
            )?;
            let mut prediction = invocation.control.reserve(n)?;
            for row in 0..n {
                if row % 1024 == 0 {
                    invocation.check_control()?;
                }
                let mut value = if model.constant {
                    model.coefficients[0]
                } else {
                    0.0
                };
                for (index, (column, coefficient)) in predictors
                    .iter()
                    .zip(&model.coefficients[usize::from(model.constant)..])
                    .enumerate()
                {
                    if index % 1024 == 0 {
                        invocation.check_control()?;
                    }
                    value += coefficient * column[row];
                }
                if !value.is_finite() {
                    return Err(KernelError::NonFiniteResult);
                }
                prediction
                    .push(RuntimeValue::float64(value).map_err(|_| KernelError::NonFiniteResult)?);
            }
            vec![RuntimeValue::List(prediction.into())]
        }
        StatisticalKernel::LinearFit => {
            let Some(RuntimeValue::Record(configuration)) = invocation.parameter("configuration")
            else {
                return Err(KernelError::InvalidParameter);
            };
            let constant = match configuration.get("constant") {
                Some(RuntimeValue::Scalar(TabularScalar::Bool(value))) => *value,
                _ => return Err(KernelError::InvalidParameter),
            };
            let method = match configuration.get("method") {
                Some(RuntimeValue::Scalar(TabularScalar::String(value))) => value.as_ref(),
                _ => return Err(KernelError::InvalidParameter),
            };
            let weights = group(invocation, "weights");
            let sigma = group(invocation, "sigma");
            if (method == "WLS" && weights.len() != 1)
                || (method != "WLS" && !weights.is_empty())
                || (method == "GLS" && sigma.is_empty())
                || (method != "GLS" && !sigma.is_empty())
            {
                return Err(KernelError::InvalidParameter);
            }
            let response = invocation
                .inputs
                .first()
                .ok_or(KernelError::InvalidNumericInput)?;
            let mut inputs = vec![response];
            inputs.extend(group(invocation, "predictors"));
            inputs.extend(weights);
            let mut prepared = columns(&inputs, invocation, 0)?;
            let n = prepared[0].len();
            check_fit_workspace(
                n,
                group(invocation, "predictors").len(),
                constant,
                method,
                invocation,
            )?;
            let method = match method {
                "OLS" => LinearRegressionMethod::Ols,
                "WLS" => LinearRegressionMethod::Wls {
                    weights: prepared.pop().ok_or(KernelError::InvalidNumericInput)?,
                },
                "GLS" => {
                    if sigma.len() != n {
                        return Err(KernelError::ShapeMismatch);
                    }
                    let retained = n
                        .checked_mul(prepared.len())
                        .and_then(|v| v.checked_mul(size_of::<f64>()))
                        .ok_or(KernelError::BudgetExceeded)?;
                    let matrix = columns(&sigma, invocation, retained)?;
                    if matrix.iter().any(|column| column.len() != n) {
                        return Err(KernelError::ShapeMismatch);
                    }
                    // A valid covariance matrix is symmetric. Its columns already equal its
                    // rows; SCI validates symmetry, so no second n-by-n transpose is needed.
                    LinearRegressionMethod::Gls { sigma: matrix }
                }
                _ => return Err(KernelError::InvalidParameter),
            };
            let response = prepared.remove(0);
            let result = yss_sci_runtime::linear_regression(
                LinearRegressionRequest {
                    response,
                    predictors: prepared,
                    options: OlsOptions {
                        constant,
                        covariance: covariance(configuration)?,
                    },
                    method,
                },
                &ScientificExecutionControl::from_shared(
                    invocation.control.cancellation.clone(),
                    invocation.control.deadline,
                ),
            )
            .map_err(|error| match error {
                ScientificComputationError::Cancelled => KernelError::Cancelled,
                ScientificComputationError::DeadlineExceeded => KernelError::DeadlineExceeded,
                ScientificComputationError::InvalidInput { violation } => match violation {
                    ScientificInputViolation::ShapeMismatch => KernelError::ShapeMismatch,
                    ScientificInputViolation::ParameterOutOfRange => KernelError::InvalidParameter,
                    _ => KernelError::InvalidNumericInput,
                },
                ScientificComputationError::ComputationFailed => KernelError::ScientificFailure,
            })?;
            let fitted = numeric_list(&result.fitted, invocation)?;
            let residuals = numeric_list(&result.residuals, invocation)?;
            vec![
                RuntimeValue::LinearRegression(Arc::new(result)),
                fitted,
                residuals,
            ]
        }
    };
    invocation.check_control()?;
    Ok(values)
}

fn group<'a>(invocation: &'a KernelInvocation<'_>, name: &str) -> Vec<&'a RuntimeValue> {
    invocation
        .input_keys
        .iter()
        .zip(invocation.inputs)
        .filter_map(|(group, value)| (*group == name).then_some(value))
        .collect()
}

fn columns(
    values: &[&RuntimeValue],
    invocation: &KernelInvocation<'_>,
    retained_bytes: usize,
) -> Result<Vec<Vec<f64>>, KernelError> {
    invocation.control.check_bytes(Some(retained_bytes))?;
    if values.is_empty() {
        return Err(KernelError::InvalidNumericInput);
    }
    if matches!(values[0], RuntimeValue::Series(_)) {
        let series = values
            .iter()
            .map(|value| match value {
                RuntimeValue::Series(series) => Ok(series.clone()),
                _ => Err(KernelError::UnalignedSeries),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut control = invocation.relation_control();
        control.max_input_bytes -= retained_bytes;
        return series[0]
            .relation()
            .numeric_columns(&series, &control)
            .map_err(super::relational::kernel_error);
    }
    let mut rows = None;
    for value in values {
        let RuntimeValue::List(column) = value else {
            return Err(KernelError::InvalidNumericInput);
        };
        if rows.is_some_and(|rows| rows != column.len()) {
            return Err(KernelError::ShapeMismatch);
        }
        rows = Some(column.len());
    }
    let rows = rows.unwrap_or(0);
    invocation.control.check_bytes(
        rows.checked_mul(values.len())
            .and_then(|v| v.checked_mul(size_of::<f64>()))
            .and_then(|v| v.checked_add(retained_bytes)),
    )?;
    let mut columns = Vec::with_capacity(values.len());
    for value in values {
        let RuntimeValue::List(values) = value else {
            unreachable!()
        };
        let mut column = invocation.control.reserve(rows)?;
        for (index, value) in values.iter().enumerate() {
            if index % 1024 == 0 {
                invocation.check_control()?;
            }
            column.push(numeric_input(Some(value))?);
        }
        columns.push(column);
    }
    Ok(columns)
}

/// Conservative admission estimate for the dense fit, retained model and both materialized
/// outputs. The linear algebra library has its own opaque workspace, so this is not an RSS cap.
fn check_fit_workspace(
    rows: usize,
    predictors: usize,
    constant: bool,
    method: &str,
    invocation: &KernelInvocation<'_>,
) -> Result<(), KernelError> {
    let bytes = (|| {
        let parameters = predictors.checked_add(usize::from(constant))?;
        let design = rows.checked_mul(parameters)?.checked_mul(6)?;
        let covariance = parameters.checked_mul(parameters)?.checked_mul(8)?;
        let vectors = rows.checked_mul(8)?;
        let gls = if method == "GLS" {
            rows.checked_mul(rows)?.checked_mul(4)?
        } else {
            0
        };
        let numeric = design
            .checked_add(covariance)?
            .checked_add(vectors)?
            .checked_add(gls)?
            .checked_mul(size_of::<f64>())?;
        numeric.checked_add(rows.checked_mul(2 * size_of::<RuntimeValue>())?)
    })();
    invocation.control.check_bytes(bytes).map(|_| ())
}

fn covariance(values: &BTreeMap<Box<str>, RuntimeValue>) -> Result<OlsCovariance, KernelError> {
    let integer = |key: &str| match values.get(key) {
        Some(RuntimeValue::Scalar(TabularScalar::Integer(value))) => {
            usize::try_from(*value).map_err(|_| KernelError::InvalidParameter)
        }
        _ => Err(KernelError::InvalidParameter),
    };
    let string = |key: &str| match values.get(key) {
        Some(RuntimeValue::Scalar(TabularScalar::String(value))) => Ok(value.clone()),
        _ => Err(KernelError::InvalidParameter),
    };
    Ok(match string("covariance")?.as_ref() {
        "nonrobust" => OlsCovariance::NonRobust,
        "HC0" => OlsCovariance::Hc0,
        "HC1" => OlsCovariance::Hc1,
        "HC2" => OlsCovariance::Hc2,
        "HC3" => OlsCovariance::Hc3,
        "fixed scale" => OlsCovariance::FixedScale {
            scale: match values.get("scale") {
                // Configuration fields also accept the protocol's decimal JSON spelling.
                Some(RuntimeValue::Scalar(TabularScalar::String(value))) => {
                    value.parse().map_err(|_| KernelError::InvalidParameter)?
                }
                value => numeric_input(value)?,
            },
        },
        "HAC" => OlsCovariance::Hac {
            kernel: string("kernel")?.into_string(),
            bandwidth: Some(
                i64::try_from(integer("bandwidth")?).map_err(|_| KernelError::InvalidParameter)?,
            ),
        },
        "newey" => OlsCovariance::Newey {
            lag: Some(i64::try_from(integer("lag")?).map_err(|_| KernelError::InvalidParameter)?),
        },
        _ => return Err(KernelError::InvalidParameter),
    })
}

fn numeric_list(
    values: &[f64],
    invocation: &KernelInvocation<'_>,
) -> Result<RuntimeValue, KernelError> {
    let mut output = invocation.control.reserve(values.len())?;
    for (index, value) in values.iter().copied().enumerate() {
        if index % 1024 == 0 {
            invocation.check_control()?;
        }
        if !value.is_finite() {
            return Err(KernelError::NonFiniteResult);
        }
        output.push(RuntimeValue::float64(value).map_err(|_| KernelError::NonFiniteResult)?);
    }
    Ok(RuntimeValue::List(output.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KernelControl, KernelId, KernelOutputSpec, KernelRegistry};
    use std::{
        borrow::Cow,
        sync::atomic::AtomicBool,
        time::{Duration, Instant},
    };

    fn run(
        kind: &str,
        inputs: &[RuntimeValue],
        groups: &[Option<&str>],
        method: &str,
        count: usize,
        constant: bool,
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        let config = RuntimeValue::Record(std::sync::Arc::new(BTreeMap::from([
            (
                "constant".into(),
                RuntimeValue::Scalar(TabularScalar::Bool(constant)),
            ),
            (
                "method".into(),
                RuntimeValue::Scalar(TabularScalar::String(method.into())),
            ),
            (
                "covariance".into(),
                RuntimeValue::Scalar(TabularScalar::String("nonrobust".into())),
            ),
        ])));
        let control = KernelControl::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(10),
        );
        let numeric = yss_data_contract::ValueType::DataSeries(Box::new(
            yss_data_contract::ValueType::number(),
        ));
        let model = yss_data_contract::ValueType::Struct("statistics.model.linear".into());
        let outputs = (0..count)
            .map(|index| KernelOutputSpec {
                data_type: if kind == "summary" || (kind == "fit" && index == 0) {
                    model.clone()
                } else {
                    numeric.clone()
                },
                fields: None,
            })
            .collect::<Vec<_>>();
        KernelRegistry::default().execute(
            &KernelId::new(format!("yssbi.statistics.linear.{kind}").into()).unwrap(),
            &KernelInvocation {
                relations: &crate::tests::relations(),
                inputs,
                input_keys: &groups
                    .iter()
                    .map(|key| key.unwrap_or(if kind == "fit" { "response" } else { "model" }))
                    .collect::<Vec<_>>(),
                parameters: if kind == "fit" {
                    BTreeMap::from([(
                        crate::KernelParameterKey::new("configuration".into()).unwrap(),
                        Cow::Borrowed(&config),
                    )])
                } else {
                    BTreeMap::new()
                },
                outputs: &outputs,
                control: &control,
            },
        )
    }

    fn series(values: &[f64]) -> RuntimeValue {
        RuntimeValue::List(
            values
                .iter()
                .copied()
                .map(|value| RuntimeValue::float64(value).unwrap())
                .collect(),
        )
    }

    #[test]
    fn statistical_materialization_checks_budget_shape_and_control_before_conversion() {
        let invalid = RuntimeValue::List(Arc::from([RuntimeValue::Scalar(TabularScalar::String(
            "invalid number".into(),
        ))]));
        let valid = series(&[1.0, 2.0]);
        let relations = crate::tests::relations();
        let mut control = KernelControl::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(10),
        );
        control.max_input_bytes = 0;
        let mut invocation = KernelInvocation {
            relations: &relations,
            inputs: &[],
            input_keys: &[],
            parameters: BTreeMap::new(),
            outputs: &[],
            control: &control,
        };
        // Invalid data would fail numeric conversion: the budget must be rejected first.
        assert!(matches!(
            columns(&[&invalid], &invocation, 0),
            Err(KernelError::BudgetExceeded)
        ));
        assert!(matches!(
            control.reserve::<RuntimeValue>(usize::MAX),
            Err(KernelError::BudgetExceeded)
        ));
        let admitted = KernelControl {
            max_input_bytes: 32,
            cancellation: control.cancellation.clone(),
            deadline: control.deadline,
        };
        invocation.control = &admitted;
        assert!(matches!(
            columns(&[&invalid, &valid], &invocation, 0),
            Err(KernelError::ShapeMismatch)
        ));
        assert!(matches!(
            columns(&[&valid], &invocation, 24),
            Err(KernelError::BudgetExceeded)
        ));
        assert!(matches!(
            check_fit_workspace(2, 1, true, "GLS", &invocation),
            Err(KernelError::BudgetExceeded)
        ));
        admitted
            .cancellation
            .store(true, std::sync::atomic::Ordering::Release);
        assert!(matches!(
            columns(&[&valid], &invocation, 0),
            Err(KernelError::Cancelled)
        ));
        admitted
            .cancellation
            .store(false, std::sync::atomic::Ordering::Release);
        let expired = KernelControl {
            deadline: Instant::now(),
            cancellation: admitted.cancellation.clone(),
            max_input_bytes: admitted.max_input_bytes,
        };
        invocation.control = &expired;
        assert!(matches!(
            columns(&[&valid], &invocation, 0),
            Err(KernelError::DeadlineExceeded)
        ));
    }

    #[test]
    fn linear_methods_share_models_with_summary_and_predict_in_original_units() {
        for constant in [false, true] {
            for (method, auxiliary, without_intercept, with_intercept) in [
                ("OLS", vec![], 20.0 / 14.0, (-4.0 / 3.0, 2.0)),
                (
                    "WLS",
                    vec![series(&[1.0, 2.0, 4.0])],
                    69.0 / 45.0,
                    (-24.0 / 13.0, 29.0 / 13.0),
                ),
                (
                    "GLS",
                    vec![
                        series(&[1.0, 0.5, 0.0]),
                        series(&[0.5, 1.0, 0.0]),
                        series(&[0.0, 0.0, 1.0]),
                    ],
                    19.0 / 13.0,
                    (-3.0 / 4.0, 7.0 / 4.0),
                ),
            ] {
                let (intercept, expected) = if constant {
                    with_intercept
                } else {
                    (0.0, without_intercept)
                };
                let mut inputs = vec![series(&[1.0, 2.0, 5.0]), series(&[1.0, 2.0, 3.0])];
                let mut groups = vec![None, Some("predictors")];
                groups.extend(vec![
                    Some(if method == "WLS" { "weights" } else { "sigma" });
                    auxiliary.len()
                ]);
                inputs.extend(auxiliary);
                let outputs = run("fit", &inputs, &groups, method, 3, constant).unwrap();
                let RuntimeValue::LinearRegression(model) = &outputs[0] else {
                    panic!("native model");
                };
                assert_eq!(model.constant, constant);
                assert!((model.coefficients[usize::from(constant)] - expected).abs() < 1e-10);
                assert_eq!(model.report.model_basic_info.model_type, method);
                assert!((model.fitted[2] - intercept - 3.0 * expected).abs() < 1e-10);
                assert!((model.residuals[2] - (5.0 - intercept - 3.0 * expected)).abs() < 1e-10);
                let summary = run("summary", &outputs[..1], &[None], method, 2, constant).unwrap();
                for value in summary {
                    let RuntimeValue::LinearRegression(result) = value else {
                        panic!("native result");
                    };
                    assert!(Arc::ptr_eq(model, &result));
                }
                let prediction = run(
                    "predict",
                    &[outputs[0].clone(), series(&[4.0, 5.0])],
                    &[None, Some("predictors")],
                    method,
                    1,
                    constant,
                )
                .unwrap();
                let RuntimeValue::List(prediction) = &prediction[0] else {
                    panic!("prediction");
                };
                let RuntimeValue::Scalar(TabularScalar::Float64(value)) = prediction[0] else {
                    panic!("number");
                };
                assert!((value.as_f64() - intercept - 4.0 * expected).abs() < 1e-10);
            }
        }
    }

    #[test]
    fn linear_fit_rejects_invalid_method_inputs_and_covariance_matrices() {
        for (method, auxiliary, group) in [
            ("WLS", vec![], "weights"),
            ("WLS", vec![series(&[1.0, 0.0, 2.0])], "weights"),
            ("WLS", vec![series(&[1.0, 2.0])], "weights"),
            ("OLS", vec![series(&[1.0, 1.0, 1.0])], "weights"),
            ("GLS", vec![], "sigma"),
            ("GLS", vec![series(&[1.0, 0.0, 0.0])], "sigma"),
            (
                "GLS",
                vec![
                    series(&[1.0, 0.0, 0.0]),
                    series(&[0.5, 1.0, 0.0]),
                    series(&[0.0, 0.0, 1.0]),
                ],
                "sigma",
            ),
            (
                "GLS",
                vec![
                    series(&[1.0, 2.0, 0.0]),
                    series(&[2.0, 1.0, 0.0]),
                    series(&[0.0, 0.0, 1.0]),
                ],
                "sigma",
            ),
        ] {
            let mut inputs = vec![series(&[1.0, 2.0, 5.0]), series(&[1.0, 2.0, 3.0])];
            let mut groups = vec![None, Some("predictors")];
            groups.extend(vec![Some(group); auxiliary.len()]);
            inputs.extend(auxiliary);
            assert!(
                run("fit", &inputs, &groups, method, 3, false).is_err(),
                "{method} must reject invalid auxiliary inputs"
            );
        }
    }
}
