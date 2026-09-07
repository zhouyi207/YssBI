use crate::state::{
    KernelExecutionError, PreparedKernelInvocation, numeric_input, parameter_value,
};
use crate::value::RuntimeValue;
use std::collections::BTreeMap;
use yss_sci_contract::regression::{OlsCovariance, OlsOptions};
use yss_sci_contract::scientific::{
    BackendExecutionControl, OlsRequest, ScientificBackend, ScientificBackendError,
};

#[derive(Clone, Copy)]
pub(crate) enum StatisticalKernel {
    OlsFit,
    OlsSummary,
}

pub(crate) fn execute(
    kind: StatisticalKernel,
    invocation: &PreparedKernelInvocation<'_>,
    backend: &dyn ScientificBackend,
) -> Result<BTreeMap<crate::plan::PlanOutputRef, RuntimeValue>, KernelExecutionError> {
    let values = match kind {
        StatisticalKernel::OlsFit | StatisticalKernel::OlsSummary => {
            let configuration = parameter_value(
                invocation
                    .parameter("configuration")
                    .ok_or(KernelExecutionError::Failed)?,
                invocation.resources,
            )?;
            let RuntimeValue::Record(configuration) = configuration else {
                return Err(KernelExecutionError::InvalidNumericInput);
            };
            let constant = match configuration.get("constant") {
                Some(RuntimeValue::Bool(value)) => *value,
                _ => return Err(KernelExecutionError::InvalidNumericInput),
            };
            let covariance = covariance(&configuration)?;
            let response = numeric_series(
                invocation
                    .inputs
                    .first()
                    .ok_or(KernelExecutionError::Failed)?,
            )?;
            let predictors = invocation
                .input_slots
                .iter()
                .zip(invocation.inputs)
                .filter(|(slot, _)| slot.contract().group.is_some())
                .map(|(_, value)| numeric_series(value))
                .collect::<Result<Vec<_>, _>>()?;
            let result = backend
                .ols(
                    OlsRequest {
                        response,
                        predictors,
                        options: OlsOptions {
                            constant,
                            covariance,
                        },
                    },
                    &BackendExecutionControl::from_shared(
                        invocation.control.cancellation.clone(),
                        invocation.control.deadline,
                    ),
                )
                .map_err(|error| match error {
                    ScientificBackendError::Cancelled => KernelExecutionError::Cancelled,
                    ScientificBackendError::DeadlineExceeded => {
                        KernelExecutionError::DeadlineExceeded
                    }
                    ScientificBackendError::InvalidInput { .. } => {
                        KernelExecutionError::InvalidNumericInput
                    }
                    _ => KernelExecutionError::Failed,
                })?;
            match kind {
                StatisticalKernel::OlsFit => vec![
                    RuntimeValue::Record(BTreeMap::from([
                        ("constant".into(), RuntimeValue::Bool(constant)),
                        ("coefficients".into(), numeric_list(result.coefficients)?),
                    ])),
                    numeric_list(result.fitted)?,
                    numeric_list(result.residuals)?,
                ],
                _ => {
                    let report = json_value(
                        serde_json::to_value(result.report)
                            .map_err(|_| KernelExecutionError::Failed)?,
                    )?;
                    vec![report.clone(), report]
                }
            }
        }
    };
    if values.len() != invocation.outputs.len() {
        return Err(KernelExecutionError::Failed);
    }
    Ok(invocation
        .outputs
        .iter()
        .zip(values)
        .map(|(output, value)| (output.output().clone(), value))
        .collect())
}

fn covariance(
    values: &BTreeMap<Box<str>, RuntimeValue>,
) -> Result<OlsCovariance, KernelExecutionError> {
    let integer = |key: &str| match values.get(key) {
        Some(RuntimeValue::Integer(value)) => {
            usize::try_from(*value).map_err(|_| KernelExecutionError::InvalidNumericInput)
        }
        _ => Err(KernelExecutionError::InvalidNumericInput),
    };
    let string = |key: &str| match values.get(key) {
        Some(RuntimeValue::String(value)) => Ok(value.clone()),
        _ => Err(KernelExecutionError::InvalidNumericInput),
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
                    .map_err(|_| KernelExecutionError::InvalidNumericInput)?,
                value => numeric_input(value)?,
            },
        },
        "HAC" => OlsCovariance::Hac {
            kernel: string("kernel")?.into_string(),
            bandwidth: Some(
                i64::try_from(integer("bandwidth")?)
                    .map_err(|_| KernelExecutionError::InvalidNumericInput)?,
            ),
        },
        "newey" => OlsCovariance::Newey {
            lag: Some(
                i64::try_from(integer("lag")?)
                    .map_err(|_| KernelExecutionError::InvalidNumericInput)?,
            ),
        },
        _ => return Err(KernelExecutionError::InvalidNumericInput),
    })
}

fn numeric_series(value: &RuntimeValue) -> Result<Vec<f64>, KernelExecutionError> {
    let RuntimeValue::List(values) = value else {
        return Err(KernelExecutionError::InvalidNumericInput);
    };
    values
        .iter()
        .map(|value| numeric_input(Some(value)))
        .collect()
}

fn numeric_list(values: Vec<f64>) -> Result<RuntimeValue, KernelExecutionError> {
    if values.iter().any(|value| !value.is_finite()) {
        return Err(KernelExecutionError::NonFiniteResult);
    }
    Ok(RuntimeValue::List(
        values.into_iter().map(RuntimeValue::Decimal).collect(),
    ))
}

fn json_value(value: serde_json::Value) -> Result<RuntimeValue, KernelExecutionError> {
    Ok(match value {
        serde_json::Value::Null => RuntimeValue::Null,
        serde_json::Value::Bool(value) => RuntimeValue::Bool(value),
        serde_json::Value::String(value) => RuntimeValue::String(value.into()),
        serde_json::Value::Number(value) => match value.as_i64() {
            Some(value) => RuntimeValue::Integer(value),
            None => RuntimeValue::Decimal(
                value
                    .as_f64()
                    .filter(|value| value.is_finite())
                    .ok_or(KernelExecutionError::NonFiniteResult)?,
            ),
        },
        serde_json::Value::Array(values) => RuntimeValue::List(
            values
                .into_iter()
                .map(json_value)
                .collect::<Result<_, _>>()?,
        ),
        serde_json::Value::Object(values) => RuntimeValue::Record(
            values
                .into_iter()
                .map(|(key, value)| Ok((key.into(), json_value(value)?)))
                .collect::<Result<_, KernelExecutionError>>()?,
        ),
    })
}
