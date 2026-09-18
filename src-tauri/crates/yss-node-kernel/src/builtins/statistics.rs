use super::numeric_input;
use crate::{KernelError, KernelInvocation, RuntimeValue};
use std::collections::BTreeMap;
use std::sync::Arc;
use yss_sci_contract::regression::{OlsCovariance, OlsOptions};
use yss_sci_contract::scientific::{
    LinearRegressionMethod, LinearRegressionRequest, ScientificComputationError,
    ScientificExecutionControl,
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
            let predictors = columns(&group(invocation, "predictors"), invocation)?;
            if predictors.is_empty()
                || predictors.len() + usize::from(model.constant) != model.coefficients.len()
            {
                return Err(KernelError::InvalidNumericInput);
            }
            let n = predictors[0].len();
            let mut prediction = vec![
                if model.constant {
                    model.coefficients[0]
                } else {
                    0.0
                };
                n
            ];
            for (column, coefficient) in predictors
                .iter()
                .zip(&model.coefficients[usize::from(model.constant)..])
            {
                for (value, x) in prediction.iter_mut().zip(column) {
                    *value += coefficient * x;
                }
            }
            vec![numeric_list(prediction)?]
        }
        StatisticalKernel::LinearFit => {
            let Some(RuntimeValue::Record(configuration)) = invocation.parameter("configuration")
            else {
                return Err(KernelError::InvalidNumericInput);
            };
            let constant = match configuration.get("constant") {
                Some(RuntimeValue::Bool(value)) => *value,
                _ => return Err(KernelError::InvalidNumericInput),
            };
            let method = match configuration.get("method") {
                Some(RuntimeValue::String(value)) => value.as_ref(),
                _ => return Err(KernelError::InvalidNumericInput),
            };
            let weights = group(invocation, "weights");
            let sigma = group(invocation, "sigma");
            if (method == "WLS" && weights.len() != 1)
                || (method != "WLS" && !weights.is_empty())
                || (method == "GLS" && sigma.is_empty())
                || (method != "GLS" && !sigma.is_empty())
            {
                return Err(KernelError::InvalidNumericInput);
            }
            let response = invocation
                .inputs
                .first()
                .ok_or(KernelError::InvalidNumericInput)?;
            let mut inputs = vec![response];
            inputs.extend(group(invocation, "predictors"));
            inputs.extend(weights);
            let mut prepared = columns(&inputs, invocation)?;
            let method = match method {
                "OLS" => LinearRegressionMethod::Ols,
                "WLS" => LinearRegressionMethod::Wls {
                    weights: prepared.pop().ok_or(KernelError::InvalidNumericInput)?,
                },
                "GLS" => {
                    let n = prepared[0].len();
                    if sigma.len() != n
                        || n.checked_mul(n)
                            .and_then(|v| v.checked_mul(8))
                            .is_none_or(|bytes| bytes > invocation.control.max_input_bytes)
                    {
                        return Err(KernelError::InvalidNumericInput);
                    }
                    let matrix = columns(&sigma, invocation)?;
                    if matrix.iter().any(|column| column.len() != n) {
                        return Err(KernelError::InvalidNumericInput);
                    }
                    LinearRegressionMethod::Gls {
                        sigma: (0..n)
                            .map(|i| matrix.iter().map(|column| column[i]).collect())
                            .collect(),
                    }
                }
                _ => return Err(KernelError::InvalidNumericInput),
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
                ScientificComputationError::InvalidInput { .. } => KernelError::InvalidNumericInput,
                _ => KernelError::Failed,
            })?;
            let fitted = numeric_list(result.fitted.clone())?;
            let residuals = numeric_list(result.residuals.clone())?;
            vec![
                RuntimeValue::LinearRegression(Arc::new(result)),
                fitted,
                residuals,
            ]
        }
    };
    invocation.check_control()?;
    if values.len() != invocation.outputs.len() {
        return Err(KernelError::Failed);
    }
    Ok(values)
}

fn group<'a>(invocation: &'a KernelInvocation<'_>, name: &str) -> Vec<&'a RuntimeValue> {
    invocation
        .input_templates
        .iter()
        .zip(invocation.inputs)
        .filter_map(|(group, value)| (*group == Some(name)).then_some(value))
        .collect()
}

fn columns(
    values: &[&RuntimeValue],
    invocation: &KernelInvocation<'_>,
) -> Result<Vec<Vec<f64>>, KernelError> {
    let columns = if values
        .first()
        .is_some_and(|value| matches!(value, RuntimeValue::Series(_)))
    {
        let series = values
            .iter()
            .map(|value| match value {
                RuntimeValue::Series(series) => Ok(series.clone()),
                _ => Err(KernelError::InvalidNumericInput),
            })
            .collect::<Result<Vec<_>, _>>()?;
        super::relational::numeric_columns(&series, invocation)?
    } else {
        values
            .iter()
            .map(|value| numeric_series(value))
            .collect::<Result<Vec<_>, _>>()?
    };
    let n = columns.first().map_or(0, Vec::len);
    if columns
        .iter()
        .any(|column| column.len() != n || column.iter().any(|value| !value.is_finite()))
        || n.checked_mul(columns.len())
            .and_then(|v| v.checked_mul(8))
            .is_none_or(|bytes| bytes > invocation.control.max_input_bytes)
    {
        return Err(KernelError::InvalidNumericInput);
    }
    Ok(columns)
}

fn covariance(values: &BTreeMap<Box<str>, RuntimeValue>) -> Result<OlsCovariance, KernelError> {
    let integer = |key: &str| match values.get(key) {
        Some(RuntimeValue::Integer(value)) => {
            usize::try_from(*value).map_err(|_| KernelError::InvalidNumericInput)
        }
        _ => Err(KernelError::InvalidNumericInput),
    };
    let string = |key: &str| match values.get(key) {
        Some(RuntimeValue::String(value)) => Ok(value.clone()),
        _ => Err(KernelError::InvalidNumericInput),
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
                Some(RuntimeValue::String(value)) => value
                    .parse()
                    .map_err(|_| KernelError::InvalidNumericInput)?,
                value => numeric_input(value)?,
            },
        },
        "HAC" => OlsCovariance::Hac {
            kernel: string("kernel")?.into_string(),
            bandwidth: Some(
                i64::try_from(integer("bandwidth")?)
                    .map_err(|_| KernelError::InvalidNumericInput)?,
            ),
        },
        "newey" => OlsCovariance::Newey {
            lag: Some(
                i64::try_from(integer("lag")?).map_err(|_| KernelError::InvalidNumericInput)?,
            ),
        },
        _ => return Err(KernelError::InvalidNumericInput),
    })
}

fn numeric_series(value: &RuntimeValue) -> Result<Vec<f64>, KernelError> {
    let RuntimeValue::List(values) = value else {
        return Err(KernelError::InvalidNumericInput);
    };
    values
        .iter()
        .map(|value| numeric_input(Some(value)))
        .collect()
}

fn numeric_list(values: Vec<f64>) -> Result<RuntimeValue, KernelError> {
    if values.iter().any(|value| !value.is_finite()) {
        return Err(KernelError::NonFiniteResult);
    }
    Ok(RuntimeValue::List(
        values.into_iter().map(RuntimeValue::Decimal).collect(),
    ))
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
        let config = RuntimeValue::Record(BTreeMap::from([
            ("constant".into(), RuntimeValue::Bool(constant)),
            ("method".into(), RuntimeValue::String(method.into())),
            (
                "covariance".into(),
                RuntimeValue::String("nonrobust".into()),
            ),
        ]));
        let control = KernelControl::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(10),
        );
        let outputs = vec![
            KernelOutputSpec {
                data_type: yss_data_contract::ValueType::Scalar(
                    yss_data_contract::SemanticType::Numeric
                ),
                fields: None
            };
            count
        ];
        KernelRegistry::default().execute(
            &KernelId::new(format!("yssbi.statistics.linear.{kind}").into()).unwrap(),
            &KernelInvocation {
                inputs,
                input_templates: groups,
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
        numeric_list(values.to_vec()).unwrap()
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
                let RuntimeValue::Decimal(value) = prediction[0] else {
                    panic!("number");
                };
                assert!((value - intercept - 4.0 * expected).abs() < 1e-10);
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
