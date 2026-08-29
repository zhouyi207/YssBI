//! Statistical kernels over materialized protocol series and model values.

use super::KernelFragment;
use crate::graph::protocol::Value;
use crate::node_system::runtime::{
    ArtifactKind, DataSeriesBuilder, DataSeriesElementType, Kernel, KernelContext, KernelError,
    NullPolicy, NumericSeriesView, RuntimeValue, numeric_series as read_numeric_series,
    prepare_numeric_rows, require_data_series,
};
use crate::project::{NumericTolerance, StatisticalMissingValuePolicy};
use crate::sci::api::computation::{
    MissingValuePolicy, StatisticalObservationMetadata, StatisticalSettingSource,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScientificApi {
    Regression,
    DiscreteRegression,
    PanelRegression,
    TimeSeries,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatisticsOperation {
    AdfTest,
    AdfSummary,
    OlsVceNonRobust,
    OlsVceHc0,
    OlsVceHc1,
    OlsVceHc2,
    OlsVceHc3,
    OlsVceFixedScale,
    OlsVceCluster,
    OlsVceHac,
    OlsVceNeweyWest,
    OlsConfigure,
    OlsFit,
    OlsSummary,
    GlsConfigure,
    GlsFit,
    GlsSummary,
    Iv2slsConfigure,
    Iv2slsSummary,
    IvLimlSummary,
    LogitConfigure,
    LogitFit,
    LogitSummary,
    PanelConfigure,
    PanelVceClusterEntity,
    PanelSummary,
    PanelDidTwfe,
    PraisConfigure,
    PraisFit,
    PraisSummary,
    LinearPredict,
    LogitPredict,
    ProbitPredict,
    ProbitConfigure,
    ProbitFit,
    ProbitSummary,
    VarLagOrder,
    VarSummary,
    VecFit,
    VecRankTest,
    WlsFit,
    WlsSummary,
}

#[derive(Debug, Clone)]
pub struct StatisticsKernelParameters {
    pub data_series_input_indices: Option<Box<[usize]>>,
    pub lags: Option<usize>,
    pub max_lags: Option<usize>,
    pub rank: Option<usize>,
    pub regression: Option<Box<str>>,
    pub trend: Option<Box<str>>,
    pub convergence_tolerance: f64,
    pub convergence_tolerance_source: StatisticalSettingSource,
    pub missing_value_policy: StatisticalMissingValuePolicy,
    pub missing_value_policy_source: StatisticalSettingSource,
}

impl Default for StatisticsKernelParameters {
    fn default() -> Self {
        Self {
            data_series_input_indices: None,
            lags: None,
            max_lags: None,
            rank: None,
            regression: None,
            trend: None,
            convergence_tolerance: NumericTolerance::default().absolute,
            convergence_tolerance_source: StatisticalSettingSource::ProjectDefault,
            missing_value_policy: StatisticalMissingValuePolicy::default(),
            missing_value_policy_source: StatisticalSettingSource::ProjectDefault,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct StatisticsKernel {
    operation: StatisticsOperation,
    api: ScientificApi,
}

impl Kernel for StatisticsKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        context
            .cancellation
            .check()
            .map_err(|error| KernelError::cancelled(error.to_string()))?;
        let parameters = context.parameters::<StatisticsKernelParameters>()?;
        execute_operation(self.operation, self.api, parameters, inputs)
    }
}

fn scalar(input: &RuntimeValue) -> Result<&Value, KernelError> {
    match input {
        RuntimeValue::Scalar(value) => Ok(value),
        _ => Err(KernelError::new(
            "statistics kernels require materialized values",
        )),
    }
}

/// Decodes coefficient vectors stored inside an opaque scalar model object.
/// Graph DataSeries inputs must always pass through `require_data_series`.
fn opaque_model_coefficients(value: &Value) -> Result<Vec<f64>, KernelError> {
    let Value::List(values) = value else {
        return Err(KernelError::new(
            "prediction model coefficients must be a numeric list",
        ));
    };
    values
        .iter()
        .map(|value| match value {
            Value::Integer(value) => Ok(*value as f64),
            Value::Unsigned(value) => Ok(*value as f64),
            Value::Decimal(value) => value
                .as_str()
                .parse()
                .map_err(|_| KernelError::new("invalid decimal statistic input")),
            _ => Err(KernelError::new(
                "statistics series contains a non-numeric value",
            )),
        })
        .collect()
}

fn protocol_value(value: serde_json::Value) -> Result<Value, KernelError> {
    Ok(match value {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(value) => Value::Bool(value),
        serde_json::Value::Number(value) if value.is_i64() => {
            Value::Integer(value.as_i64().unwrap())
        }
        serde_json::Value::Number(value) if value.is_u64() => {
            Value::Unsigned(value.as_u64().unwrap())
        }
        serde_json::Value::Number(value) => Value::Decimal(super::core_nodes::canonical_float(
            value
                .as_f64()
                .ok_or_else(|| KernelError::new("invalid floating-point statistic output"))?,
        )?),
        serde_json::Value::String(value) => Value::String(value.into()),
        serde_json::Value::Array(values) => Value::List(
            values
                .into_iter()
                .map(protocol_value)
                .collect::<Result<_, _>>()?,
        ),
        serde_json::Value::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| Ok((key.into(), protocol_value(value)?)))
                .collect::<Result<_, KernelError>>()?,
        ),
    })
}

fn regression_kind(
    operation: StatisticsOperation,
) -> Option<crate::sci::api::node_statistics::RegressionKind> {
    use crate::sci::api::node_statistics::RegressionKind;
    use StatisticsOperation::*;
    match operation {
        OlsFit | OlsSummary | LinearPredict => Some(RegressionKind::Ols),
        GlsFit | GlsSummary => Some(RegressionKind::Gls),
        LogitFit | LogitSummary | LogitPredict => Some(RegressionKind::Logit),
        ProbitFit | ProbitSummary | ProbitPredict => Some(RegressionKind::Probit),
        PraisFit | PraisSummary => Some(RegressionKind::Prais),
        WlsFit | WlsSummary => Some(RegressionKind::Wls),
        _ => None,
    }
}

fn numeric_view(input: &RuntimeValue) -> Result<NumericSeriesView, KernelError> {
    read_numeric_series(require_data_series(input)?, NullPolicy::Propagate)
}

struct PreparedStatisticsRows {
    columns: Box<[Box<[f64]>]>,
    metadata: StatisticalObservationMetadata,
}

fn prepare_statistics_rows(
    inputs: &[RuntimeValue],
    labels: &[Box<str>],
    parameters: &StatisticsKernelParameters,
    convergence_tolerance_consumed: bool,
) -> Result<PreparedStatisticsRows, KernelError> {
    let views = inputs
        .iter()
        .map(numeric_view)
        .collect::<Result<Vec<_>, _>>()?;
    let rows = prepare_numeric_rows(&views, parameters.missing_value_policy).map_err(|error| {
        let message = error.message();
        for (index, label) in labels.iter().enumerate() {
            let prefix = format!("numeric input {index}");
            if let Some(detail) = message.strip_prefix(&prefix) {
                return KernelError::new(format!("statistics input '{label}'{detail}"));
            }
        }
        KernelError::new(message.to_owned())
    })?;
    if rows.used_row_count() == 0 {
        return Err(KernelError::new(
            "statistics input has no usable observations",
        ));
    }
    Ok(PreparedStatisticsRows {
        columns: rows.columns().to_vec().into_boxed_slice(),
        metadata: StatisticalObservationMetadata {
            original_observation_count: rows.original_row_count(),
            used_observation_count: rows.used_row_count(),
            dropped_null_count: rows.dropped_null_count(),
            dropped_nan_count: rows.dropped_nan_count(),
            missing_value_policy: match parameters.missing_value_policy {
                StatisticalMissingValuePolicy::Listwise => MissingValuePolicy::Listwise,
                StatisticalMissingValuePolicy::Reject => MissingValuePolicy::Reject,
            },
            missing_value_policy_source: parameters.missing_value_policy_source,
            effective_convergence_tolerance: parameters.convergence_tolerance,
            convergence_tolerance_source: parameters.convergence_tolerance_source,
            convergence_tolerance_consumed,
        },
    })
}

fn regression_fit(
    operation: StatisticsOperation,
    parameters: &StatisticsKernelParameters,
    inputs: &[RuntimeValue],
) -> Result<crate::sci::api::node_statistics::RegressionFit, KernelError> {
    let selected_inputs;
    let inputs = match parameters.data_series_input_indices.as_deref() {
        Some(indices) => {
            selected_inputs = indices
                .iter()
                .map(|index| {
                    inputs.get(*index).cloned().ok_or_else(|| {
                        KernelError::new(format!(
                            "compiled statistics input index {index} is out of bounds"
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            selected_inputs.as_slice()
        }
        None => inputs,
    };
    if inputs.len() < 2 {
        return Err(KernelError::new(
            "regression requires a response and at least one predictor",
        ));
    }
    let labels = (0..inputs.len())
        .map(|index| match index {
            0 => "response".into(),
            index
                if matches!(
                    operation,
                    StatisticsOperation::WlsFit | StatisticsOperation::WlsSummary
                ) && index + 1 == inputs.len() =>
            {
                "weights".into()
            }
            index => format!("predictors[{}]", index - 1).into(),
        })
        .collect::<Vec<Box<str>>>();
    let prepared = prepare_statistics_rows(inputs, &labels, parameters, false)?;
    let mut columns = prepared.columns.into_vec();
    let response = columns.remove(0).into_vec();
    let weights = if matches!(
        operation,
        StatisticsOperation::WlsFit | StatisticsOperation::WlsSummary
    ) {
        Some(
            columns
                .pop()
                .ok_or_else(|| KernelError::new("WLS requires weights"))?
                .into_vec(),
        )
    } else {
        None
    };
    if prepared.metadata.used_observation_count < columns.len() + 1 {
        return Err(KernelError::new(format!(
            "regression has {} usable observations but requires at least {} fitted parameters",
            prepared.metadata.used_observation_count,
            columns.len() + 1
        )));
    }
    crate::sci::api::node_statistics::fit_regression(
        regression_kind(operation).expect("regression operation"),
        response,
        columns.into_iter().map(Vec::from).collect(),
        weights,
        prepared.metadata,
    )
    .map_err(|error| KernelError::new(error.to_string()))
}

fn float_series(values: Vec<f64>, name: &'static str) -> Result<RuntimeValue, KernelError> {
    let values = values
        .into_iter()
        .map(|value| super::core_nodes::canonical_float(value).map(Value::Decimal))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(RuntimeValue::Artifact(
        DataSeriesBuilder::new(DataSeriesElementType::Float64)
            .name(name)
            .values(values)
            .build(ArtifactKind::Collected)
            .map_err(|error| KernelError::new(error.to_string()))?,
    ))
}

fn fit_values(
    fit: crate::sci::api::node_statistics::RegressionFit,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let model = protocol_value(serde_json::json!({
        "family": fit.family,
        "coefficients": fit.coefficients,
        "statistics": fit.statistics,
        "metadata": fit.metadata,
    }))?;
    emit_fit_log(fit.family, &fit.metadata);
    Ok(vec![
        RuntimeValue::Scalar(model),
        float_series(fit.fitted, "fitted")?,
        float_series(fit.residuals, "residuals")?,
    ])
}

fn participating_series<'a>(
    inputs: &'a [RuntimeValue],
) -> Result<Vec<&'a RuntimeValue>, KernelError> {
    inputs
        .iter()
        .filter_map(|input| match input {
            RuntimeValue::Artifact(_) => Some(Ok(input)),
            RuntimeValue::Scalar(Value::Null | Value::Object(_)) => None,
            RuntimeValue::Scalar(Value::List(_)) => Some(Err(KernelError::new(
                "expected DataSeries Artifact, received scalar",
            ))),
            RuntimeValue::Scalar(_) => None,
            RuntimeValue::Stream(_) => Some(Err(KernelError::new(
                "expected DataSeries Artifact, received stream",
            ))),
        })
        .collect()
}

fn prepare_participating_rows(
    inputs: &[RuntimeValue],
    labels: Vec<Box<str>>,
    parameters: &StatisticsKernelParameters,
) -> Result<PreparedStatisticsRows, KernelError> {
    let series = participating_series(inputs)?;
    let owned = series.into_iter().cloned().collect::<Vec<_>>();
    prepare_statistics_rows(&owned, &labels, parameters, false)
}

fn emit_fit_log(family: &str, metadata: &StatisticalObservationMetadata) {
    let source = format!("yssbi.statistics.{family}.fit");
    tracing::info!(
        target: "yssbi::node_system::runtime::statistics",
        diagnostic_domain = "execution",
        diagnostic_event = "fitCompleted",
        diagnostic_source = source.as_str(),
        family,
        used_observation_count = metadata.used_observation_count,
        original_observation_count = metadata.original_observation_count,
        dropped_null_count = metadata.dropped_null_count,
        dropped_nan_count = metadata.dropped_nan_count,
        "Statistical fit completed"
    );
}

fn result_with_metadata(
    result: serde_json::Value,
    metadata: &StatisticalObservationMetadata,
    family: &str,
) -> Result<Value, KernelError> {
    let serde_json::Value::Object(mut result) = result else {
        return Err(KernelError::new(
            "statistical model result must be an object",
        ));
    };
    result.insert(
        "metadata".to_owned(),
        serde_json::to_value(metadata).map_err(|error| KernelError::new(error.to_string()))?,
    );
    emit_fit_log(family, metadata);
    protocol_value(serde_json::Value::Object(result))
}

fn configuration(operation: StatisticsOperation) -> Value {
    let mut value = BTreeMap::new();
    value.insert(
        "operation".into(),
        Value::String(format!("{operation:?}").into()),
    );
    value.insert("implementation".into(), Value::String("yss_sci".into()));
    Value::Object(value)
}

fn prediction(
    operation: StatisticsOperation,
    parameters: &StatisticsKernelParameters,
    inputs: &[RuntimeValue],
) -> Result<RuntimeValue, KernelError> {
    use statrs::distribution::{ContinuousCDF, Normal};
    let Value::Object(model) = scalar(
        inputs
            .first()
            .ok_or_else(|| KernelError::new("prediction requires a model"))?,
    )?
    else {
        return Err(KernelError::new("prediction model is invalid"));
    };
    let expected_family = match operation {
        StatisticsOperation::LinearPredict => "ols",
        StatisticsOperation::LogitPredict => "logit",
        StatisticsOperation::ProbitPredict => "probit",
        _ => unreachable!("prediction operation"),
    };
    let Value::String(family) = model
        .get("family")
        .ok_or_else(|| KernelError::new("prediction model has no family"))?
    else {
        return Err(KernelError::new("prediction model family is invalid"));
    };
    if family.as_ref() != expected_family {
        return Err(KernelError::new(format!(
            "statistics prediction {operation:?} requires model family '{expected_family}', got '{family}'"
        )));
    }
    let coefficients = model
        .get("coefficients")
        .ok_or_else(|| KernelError::new("prediction model has no coefficients"))
        .and_then(opaque_model_coefficients)?;
    let labels = (0..inputs.len().saturating_sub(1))
        .map(|index| format!("predictors[{index}]").into())
        .collect::<Vec<Box<str>>>();
    let prepared = prepare_statistics_rows(&inputs[1..], &labels, parameters, false)?;
    let predictors = prepared
        .columns
        .into_vec()
        .into_iter()
        .map(Vec::from)
        .collect::<Vec<_>>();
    let observations = prepared.metadata.used_observation_count;
    if coefficients.len() != predictors.len() + 1
        || predictors.iter().any(|series| series.len() != observations)
    {
        return Err(KernelError::new(
            "prediction inputs do not match the fitted model",
        ));
    }
    let normal = Normal::new(0.0, 1.0).map_err(|error| KernelError::new(error.to_string()))?;
    let values = (0..observations)
        .map(|row| {
            let linear = coefficients[0]
                + predictors
                    .iter()
                    .zip(&coefficients[1..])
                    .map(|(series, coefficient)| series[row] * coefficient)
                    .sum::<f64>();
            match operation {
                StatisticsOperation::LogitPredict => 1.0 / (1.0 + (-linear).exp()),
                StatisticsOperation::ProbitPredict => normal.cdf(linear),
                StatisticsOperation::LinearPredict => linear,
                _ => unreachable!("prediction operation"),
            }
        })
        .collect::<Vec<_>>();
    float_series(values, "prediction")
}

fn execute_operation(
    operation: StatisticsOperation,
    api: ScientificApi,
    parameters: &StatisticsKernelParameters,
    inputs: &[RuntimeValue],
) -> Result<Vec<RuntimeValue>, KernelError> {
    use StatisticsOperation::*;
    validate_api(operation, api)?;
    match operation {
        OlsVceNonRobust
        | OlsVceHc0
        | OlsVceHc1
        | OlsVceHc2
        | OlsVceHc3
        | OlsVceFixedScale
        | OlsVceCluster
        | OlsVceHac
        | OlsVceNeweyWest
        | OlsConfigure
        | GlsConfigure
        | Iv2slsConfigure
        | LogitConfigure
        | PanelConfigure
        | PanelVceClusterEntity
        | PraisConfigure
        | ProbitConfigure => Ok(vec![RuntimeValue::Scalar(configuration(operation))]),
        OlsFit | GlsFit | LogitFit | PraisFit | ProbitFit | WlsFit => {
            fit_values(regression_fit(operation, parameters, inputs)?)
        }
        LinearPredict | LogitPredict | ProbitPredict => {
            Ok(vec![prediction(operation, parameters, inputs)?])
        }
        AdfTest => {
            let prepared = prepare_statistics_rows(inputs, &["series".into()], parameters, false)?;
            let series = &prepared.columns[0];
            let result = crate::sci::api::node_statistics::augmented_dickey_fuller(
                series,
                parameters.lags.unwrap_or(1),
                parameters.regression.as_deref().unwrap_or("constant"),
            )
            .map_err(|error| KernelError::new(error.to_string()))?;
            Ok(vec![RuntimeValue::Scalar(protocol_value(result)?)])
        }
        VarLagOrder => {
            let count = participating_series(inputs)?.len();
            let prepared = prepare_participating_rows(
                inputs,
                (0..count)
                    .map(|index| format!("variables[{index}]").into())
                    .collect(),
                parameters,
            )?;
            require_constant_trend("VAR lag-order selection", parameters)?;
            let result = crate::sci::api::node_statistics::var_lag_order(
                prepared
                    .columns
                    .into_vec()
                    .into_iter()
                    .map(Vec::from)
                    .collect(),
                parameters.max_lags.unwrap_or(4),
            )
            .map_err(|error| KernelError::new(error.to_string()))?;
            Ok(vec![RuntimeValue::Scalar(protocol_value(result)?)])
        }
        VecFit => {
            let count = participating_series(inputs)?.len();
            let prepared = prepare_participating_rows(
                inputs,
                (0..count)
                    .map(|index| format!("variables[{index}]").into())
                    .collect(),
                parameters,
            )?;
            let result = crate::sci::api::node_statistics::vec_fit(
                prepared
                    .columns
                    .into_vec()
                    .into_iter()
                    .map(Vec::from)
                    .collect(),
                parameters.rank.unwrap_or(1),
                parameters.lags.unwrap_or(1),
                parameters.trend.as_deref().unwrap_or("constant"),
            )
            .map_err(|error| KernelError::new(error.to_string()))?;
            let model = result_with_metadata(result, &prepared.metadata, "vec")?;
            Ok(vec![
                RuntimeValue::Scalar(model),
                float_series(Vec::new(), "fitted")?,
                float_series(Vec::new(), "residuals")?,
            ])
        }
        VecRankTest => {
            let count = participating_series(inputs)?.len();
            let prepared = prepare_participating_rows(
                inputs,
                (0..count)
                    .map(|index| format!("variables[{index}]").into())
                    .collect(),
                parameters,
            )?;
            let result = crate::sci::api::node_statistics::vec_rank_test(
                prepared
                    .columns
                    .into_vec()
                    .into_iter()
                    .map(Vec::from)
                    .collect(),
                parameters.max_lags.unwrap_or(4),
                parameters.trend.as_deref().unwrap_or("constant"),
            )
            .map_err(|error| KernelError::new(error.to_string()))?;
            Ok(vec![RuntimeValue::Scalar(protocol_value(result)?)])
        }
        OlsSummary | GlsSummary | LogitSummary | PraisSummary | ProbitSummary | WlsSummary => {
            let fit = regression_fit(operation, parameters, inputs)?;
            let result = protocol_value(
                serde_json::to_value(&fit).map_err(|error| KernelError::new(error.to_string()))?,
            )?;
            let report = protocol_value(crate::sci::api::node_statistics::regression_report(&fit))?;
            emit_fit_log(fit.family, &fit.metadata);
            Ok(vec![
                RuntimeValue::Scalar(result),
                RuntimeValue::Scalar(report),
            ])
        }
        AdfSummary => {
            let result = scalar(
                inputs
                    .first()
                    .ok_or_else(|| KernelError::new("ADF summary requires a test result"))?,
            )?
            .clone();
            Ok(vec![
                RuntimeValue::Scalar(result),
                RuntimeValue::Scalar(Value::String("ADF result from yss_sci".into())),
            ])
        }
        VarSummary => {
            let count = participating_series(inputs)?.len();
            let prepared = prepare_participating_rows(
                inputs,
                (0..count)
                    .map(|index| format!("variables[{index}]").into())
                    .collect(),
                parameters,
            )?;
            require_constant_trend("VAR summary", parameters)?;
            let result = crate::sci::api::node_statistics::var_fit(
                prepared
                    .columns
                    .into_vec()
                    .into_iter()
                    .map(Vec::from)
                    .collect(),
                parameters.lags.unwrap_or(1),
            )
            .map_err(|error| KernelError::new(error.to_string()))?;
            Ok(vec![
                RuntimeValue::Scalar(result_with_metadata(result, &prepared.metadata, "var")?),
                RuntimeValue::Scalar(Value::String("VAR estimation from yss_sci".into())),
            ])
        }
        Iv2slsSummary | IvLimlSummary => {
            let series_count = participating_series(inputs)?.len();
            if series_count != 4 {
                return Err(KernelError::new(
                    "IV summary requires exactly response, one exogenous predictor, one endogenous predictor, and one instrument",
                ));
            }
            let prepared = prepare_participating_rows(
                inputs,
                [
                    "response",
                    "predictors[0]",
                    "endogenous[0]",
                    "instruments[0]",
                ]
                .into_iter()
                .map(Into::into)
                .collect(),
                parameters,
            )?;
            let series = prepared
                .columns
                .into_vec()
                .into_iter()
                .map(Vec::from)
                .collect::<Vec<_>>();
            let kind = if operation == Iv2slsSummary {
                crate::sci::api::node_statistics::InstrumentalVariableKind::TwoStageLeastSquares
            } else {
                crate::sci::api::node_statistics::InstrumentalVariableKind::LimitedInformationMaximumLikelihood
            };
            let result = crate::sci::api::node_statistics::fit_instrumental_variables(
                kind,
                series[0].clone(),
                series[1].clone(),
                series[2].clone(),
                series[3].clone(),
            )
            .map_err(|error| KernelError::new(error.to_string()))?;
            Ok(vec![
                RuntimeValue::Scalar(result_with_metadata(
                    result,
                    &prepared.metadata,
                    if operation == Iv2slsSummary {
                        "iv_2sls"
                    } else {
                        "iv_liml"
                    },
                )?),
                RuntimeValue::Scalar(Value::String(
                    format!("{operation:?} result from yss_sci").into(),
                )),
            ])
        }
        PanelSummary | PanelDidTwfe => {
            let required = if operation == PanelDidTwfe { 5 } else { 4 };
            let series_count = participating_series(inputs)?.len();
            if series_count < required {
                return Err(KernelError::new(
                    "panel summary requires response, predictors, entity, time, and optional treatment series",
                ));
            }
            let mut labels = vec!["response".into()];
            let predictor_count = series_count - if operation == PanelDidTwfe { 4 } else { 3 };
            labels.extend((0..predictor_count).map(|index| format!("predictors[{index}]").into()));
            labels.push("entity".into());
            labels.push("time".into());
            if operation == PanelDidTwfe {
                labels.push("treatment".into());
            }
            let prepared = prepare_participating_rows(inputs, labels, parameters)?;
            let mut series = prepared
                .columns
                .into_vec()
                .into_iter()
                .map(Vec::from)
                .collect::<Vec<_>>();
            let treatment = (operation == PanelDidTwfe).then(|| series.pop().unwrap());
            let time = series.pop().unwrap();
            let entity = series.pop().unwrap();
            let response = series.remove(0);
            let result = crate::sci::api::node_statistics::fit_panel(
                response, series, entity, time, treatment,
            )
            .map_err(|error| KernelError::new(error.to_string()))?;
            Ok(vec![
                RuntimeValue::Scalar(result_with_metadata(
                    result,
                    &prepared.metadata,
                    if operation == PanelDidTwfe {
                        "panel_did_twfe"
                    } else {
                        "panel"
                    },
                )?),
                RuntimeValue::Scalar(Value::String(
                    format!("{operation:?} result from yss_sci").into(),
                )),
            ])
        }
    }
}

fn require_constant_trend(
    operation: &str,
    parameters: &StatisticsKernelParameters,
) -> Result<(), KernelError> {
    let trend = parameters.trend.as_deref().unwrap_or("constant");
    if trend == "constant" {
        Ok(())
    } else {
        Err(KernelError::new(format!(
            "{operation} does not support trend '{trend}' in the scientific backend"
        )))
    }
}

fn validate_api(operation: StatisticsOperation, actual: ScientificApi) -> Result<(), KernelError> {
    use StatisticsOperation::*;
    let expected = match operation {
        LogitConfigure | LogitFit | LogitSummary | LogitPredict | ProbitConfigure | ProbitFit
        | ProbitSummary | ProbitPredict => ScientificApi::DiscreteRegression,
        PanelConfigure | PanelVceClusterEntity | PanelSummary | PanelDidTwfe => {
            ScientificApi::PanelRegression
        }
        AdfTest | AdfSummary | VarLagOrder | VarSummary | VecFit | VecRankTest => {
            ScientificApi::TimeSeries
        }
        _ => ScientificApi::Regression,
    };
    if actual == expected {
        Ok(())
    } else {
        Err(KernelError::new(format!(
            "statistics operation {operation:?} requires {expected:?}, got {actual:?}"
        )))
    }
}

pub(crate) fn build_kernel_fragment() -> KernelFragment {
    use ScientificApi::{
        DiscreteRegression as Discrete, PanelRegression as Panel, Regression, TimeSeries,
    };
    use StatisticsOperation::*;
    let registrations = [
        registration("yssbi.statistics.adf.test", AdfTest, TimeSeries),
        registration("yssbi.statistics.adf.summary", AdfSummary, TimeSeries),
        registration(
            "yssbi.statistics.ols.vce.non_robust",
            OlsVceNonRobust,
            Regression,
        ),
        registration("yssbi.statistics.ols.vce.hc0", OlsVceHc0, Regression),
        registration("yssbi.statistics.ols.vce.hc1", OlsVceHc1, Regression),
        registration("yssbi.statistics.ols.vce.hc2", OlsVceHc2, Regression),
        registration("yssbi.statistics.ols.vce.hc3", OlsVceHc3, Regression),
        registration(
            "yssbi.statistics.ols.vce.fixed_scale",
            OlsVceFixedScale,
            Regression,
        ),
        registration(
            "yssbi.statistics.ols.vce.cluster",
            OlsVceCluster,
            Regression,
        ),
        registration("yssbi.statistics.ols.vce.hac", OlsVceHac, Regression),
        registration(
            "yssbi.statistics.ols.vce.newey_west",
            OlsVceNeweyWest,
            Regression,
        ),
        registration("yssbi.statistics.ols.configure", OlsConfigure, Regression),
        registration("yssbi.statistics.ols.fit", OlsFit, Regression),
        registration("yssbi.statistics.ols.summary", OlsSummary, Regression),
        registration("yssbi.statistics.gls.configure", GlsConfigure, Regression),
        registration("yssbi.statistics.gls.fit", GlsFit, Regression),
        registration("yssbi.statistics.gls.summary", GlsSummary, Regression),
        registration(
            "yssbi.statistics.iv.2sls.configure",
            Iv2slsConfigure,
            Regression,
        ),
        registration(
            "yssbi.statistics.iv.2sls.summary",
            Iv2slsSummary,
            Regression,
        ),
        registration(
            "yssbi.statistics.iv.liml.summary",
            IvLimlSummary,
            Regression,
        ),
        registration("yssbi.statistics.logit.configure", LogitConfigure, Discrete),
        registration("yssbi.statistics.logit.fit", LogitFit, Discrete),
        registration("yssbi.statistics.logit.summary", LogitSummary, Discrete),
        registration("yssbi.statistics.panel.configure", PanelConfigure, Panel),
        registration(
            "yssbi.statistics.panel.vce.cluster_entity",
            PanelVceClusterEntity,
            Panel,
        ),
        registration("yssbi.statistics.panel.summary", PanelSummary, Panel),
        registration("yssbi.statistics.panel.did.twfe", PanelDidTwfe, Panel),
        registration(
            "yssbi.statistics.prais.configure",
            PraisConfigure,
            Regression,
        ),
        registration("yssbi.statistics.prais.fit", PraisFit, Regression),
        registration("yssbi.statistics.prais.summary", PraisSummary, Regression),
        registration("yssbi.statistics.linear.predict", LinearPredict, Regression),
        registration("yssbi.statistics.logit.predict", LogitPredict, Discrete),
        registration("yssbi.statistics.probit.predict", ProbitPredict, Discrete),
        registration(
            "yssbi.statistics.probit.configure",
            ProbitConfigure,
            Discrete,
        ),
        registration("yssbi.statistics.probit.fit", ProbitFit, Discrete),
        registration("yssbi.statistics.probit.summary", ProbitSummary, Discrete),
        registration("yssbi.statistics.var.lag_order", VarLagOrder, TimeSeries),
        registration("yssbi.statistics.var.summary", VarSummary, TimeSeries),
        registration("yssbi.statistics.vec.fit", VecFit, TimeSeries),
        registration("yssbi.statistics.vec.rank_test", VecRankTest, TimeSeries),
        registration("yssbi.statistics.wls.fit", WlsFit, Regression),
        registration("yssbi.statistics.wls.summary", WlsSummary, Regression),
    ];
    let mut fragment = KernelFragment::default();
    for (handle, operation, api) in registrations {
        fragment.register(handle, StatisticsKernel { operation, api });
    }
    fragment
}

const fn registration(
    handle: &'static str,
    operation: StatisticsOperation,
    api: ScientificApi,
) -> (&'static str, StatisticsOperation, ScientificApi) {
    (handle, operation, api)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn series(values: &[f64]) -> RuntimeValue {
        float_series(values.to_vec(), "test").unwrap()
    }

    fn object(value: &RuntimeValue) -> &BTreeMap<Box<str>, Value> {
        let RuntimeValue::Scalar(Value::Object(value)) = value else {
            panic!("expected object output");
        };
        value
    }

    #[test]
    fn adf_uses_only_the_regression_parameter() {
        let input = series(&[1.0, 1.4, 1.1, 1.8, 1.5, 2.2, 1.9, 2.6, 2.3, 3.0, 2.7, 3.4]);
        for regression in ["none", "trend"] {
            let parameters = StatisticsKernelParameters {
                lags: Some(1),
                max_lags: None,
                rank: None,
                regression: Some(regression.into()),
                trend: Some(if regression == "none" {
                    "trend".into()
                } else {
                    "none".into()
                }),
                ..StatisticsKernelParameters::default()
            };

            let outputs = execute_operation(
                StatisticsOperation::AdfTest,
                ScientificApi::TimeSeries,
                &parameters,
                std::slice::from_ref(&input),
            )
            .unwrap();
            let expected = crate::sci::api::node_statistics::augmented_dickey_fuller(
                &[1.0, 1.4, 1.1, 1.8, 1.5, 2.2, 1.9, 2.6, 2.3, 3.0, 2.7, 3.4],
                1,
                regression,
            )
            .unwrap();

            assert_eq!(
                outputs[0],
                RuntimeValue::Scalar(protocol_value(expected).unwrap())
            );
        }

        let parameters = StatisticsKernelParameters {
            lags: Some(1),
            max_lags: None,
            rank: None,
            regression: Some("unexpected".into()),
            trend: Some("constant".into()),
            ..StatisticsKernelParameters::default()
        };
        let error = execute_operation(
            StatisticsOperation::AdfTest,
            ScientificApi::TimeSeries,
            &parameters,
            &[input],
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "scientific input is invalid");
    }

    #[test]
    fn prediction_rejects_incompatible_model_family() {
        let cases = [
            (
                StatisticsOperation::LinearPredict,
                "logit",
                "statistics prediction LinearPredict requires model family 'ols', got 'logit'",
            ),
            (
                StatisticsOperation::LogitPredict,
                "ols",
                "statistics prediction LogitPredict requires model family 'logit', got 'ols'",
            ),
            (
                StatisticsOperation::ProbitPredict,
                "logit",
                "statistics prediction ProbitPredict requires model family 'probit', got 'logit'",
            ),
        ];

        for (operation, family, expected_error) in cases {
            let model = RuntimeValue::Scalar(
                protocol_value(serde_json::json!({
                    "family": family,
                    "coefficients": [0.0, 1.0],
                }))
                .unwrap(),
            );
            let error = prediction(
                operation,
                &StatisticsKernelParameters::default(),
                &[model, series(&[1.0])],
            )
            .unwrap_err();

            assert_eq!(error.to_string(), expected_error);
        }
    }

    #[test]
    fn prediction_rejects_missing_and_non_string_model_family() {
        let cases = [
            (
                serde_json::json!({"coefficients": [0.0, 1.0]}),
                "prediction model has no family",
            ),
            (
                serde_json::json!({"family": 1, "coefficients": [0.0, 1.0]}),
                "prediction model family is invalid",
            ),
        ];

        for (model, expected_error) in cases {
            let model = RuntimeValue::Scalar(protocol_value(model).unwrap());
            let error = prediction(
                StatisticsOperation::LinearPredict,
                &StatisticsKernelParameters::default(),
                &[model, series(&[1.0])],
            )
            .unwrap_err();

            assert_eq!(error.to_string(), expected_error);
        }
    }

    #[test]
    fn var_summary_runs_estimation_instead_of_lag_selection() {
        let ts_a = series(&[
            1.0, 1.2, 0.9, 1.1, 1.4, 1.0, 0.8, 1.3, 1.1, 0.9, 1.2, 1.5, 1.1, 0.7, 1.0, 1.3,
        ]);
        let ts_b = series(&[
            0.5, 0.7, 0.6, 0.9, 0.8, 1.0, 0.7, 0.6, 0.9, 1.1, 0.8, 0.7, 1.0, 0.9, 0.6, 0.8,
        ]);
        let parameters = StatisticsKernelParameters {
            lags: Some(1),
            max_lags: Some(2),
            rank: None,
            regression: None,
            trend: Some("constant".into()),
            ..StatisticsKernelParameters::default()
        };

        let outputs = execute_operation(
            StatisticsOperation::VarSummary,
            ScientificApi::TimeSeries,
            &parameters,
            &[ts_a, ts_b],
        )
        .unwrap();

        let result = object(&outputs[0]);
        assert!(result.contains_key("equations"));
        assert!(result.contains_key("coefficients"));
        assert!(!result.contains_key("rows"));
        assert_eq!(
            outputs[1],
            RuntimeValue::Scalar(Value::String("VAR estimation from yss_sci".into()))
        );
    }

    #[test]
    fn operation_specific_statistics_match_sci_golden_fixtures() {
        let x = series(&[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0]);
        let linear_y = series(&[1.0, 2.2, 2.8, 4.1, 5.2, 5.8, 7.1, 8.2]);
        let binary_y = series(&[0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0]);
        let weights = series(&[1.0, 1.0, 1.0, 1.0, 2.0, 3.0, 5.0, 8.0]);
        let correlated_y = series(&[1.0, 1.8, 2.7, 3.9, 5.4, 6.8, 8.5, 10.1]);

        let cases = [
            (
                StatisticsOperation::OlsFit,
                ScientificApi::Regression,
                vec![linear_y.clone(), x.clone()],
                "ols",
                "linear",
                None,
                "r2",
            ),
            (
                StatisticsOperation::GlsFit,
                ScientificApi::Regression,
                vec![linear_y.clone(), x.clone()],
                "gls",
                "linear",
                None,
                "standardErrors",
            ),
            (
                StatisticsOperation::WlsFit,
                ScientificApi::Regression,
                vec![linear_y.clone(), x.clone(), weights],
                "wls",
                "linear",
                None,
                "standardErrors",
            ),
            (
                StatisticsOperation::PraisFit,
                ScientificApi::Regression,
                vec![correlated_y, x.clone()],
                "prais",
                "prais",
                None,
                "rho",
            ),
            (
                StatisticsOperation::LogitFit,
                ScientificApi::DiscreteRegression,
                vec![binary_y.clone(), x.clone()],
                "logit",
                "binary",
                Some("logit"),
                "logLikelihood",
            ),
            (
                StatisticsOperation::ProbitFit,
                ScientificApi::DiscreteRegression,
                vec![binary_y, x],
                "probit",
                "binary",
                Some("probit"),
                "logLikelihood",
            ),
        ];
        let mut coefficients = BTreeMap::new();
        for (operation, api, inputs, family, statistics_kind, link, statistic) in cases {
            let outputs = execute_operation(
                operation,
                api,
                &StatisticsKernelParameters::default(),
                &inputs,
            )
            .unwrap();
            let model = object(&outputs[0]);
            assert_eq!(model.get("family"), Some(&Value::String(family.into())));
            let Value::Object(statistics) = model.get("statistics").unwrap() else {
                panic!("statistics must be an object")
            };
            assert_eq!(
                statistics.get("kind"),
                Some(&Value::String(statistics_kind.into()))
            );
            let expected_link = link.map(|link| Value::String(link.into()));
            assert_eq!(statistics.get("link"), expected_link.as_ref());
            assert!(
                statistics.contains_key(statistic),
                "{family} missing {statistic}"
            );
            coefficients.insert(family, model.get("coefficients").unwrap().clone());
        }
        assert_ne!(coefficients["logit"], coefficients["probit"]);
        assert_ne!(coefficients["ols"], coefficients["wls"]);

        let ts_a = series(&[1.0, 1.4, 1.1, 1.8, 1.5, 2.2, 1.9, 2.6, 2.3, 3.0, 2.7, 3.4]);
        let ts_b = series(&[0.7, 0.9, 1.0, 1.2, 1.1, 1.5, 1.4, 1.8, 1.7, 2.1, 2.0, 2.4]);
        let parameters = StatisticsKernelParameters {
            lags: Some(2),
            max_lags: Some(2),
            rank: Some(1),
            regression: Some("constant".into()),
            trend: Some("constant".into()),
            ..StatisticsKernelParameters::default()
        };
        let adf = execute_operation(
            StatisticsOperation::AdfTest,
            ScientificApi::TimeSeries,
            &parameters,
            &[ts_a.clone()],
        )
        .unwrap();
        assert_eq!(object(&adf[0]).get("lags"), Some(&Value::Integer(2)));
        let var = execute_operation(
            StatisticsOperation::VarLagOrder,
            ScientificApi::TimeSeries,
            &parameters,
            &[ts_a.clone(), ts_b.clone()],
        )
        .unwrap();
        assert!(object(&var[0]).len() > 1);
        let vec = execute_operation(
            StatisticsOperation::VecFit,
            ScientificApi::TimeSeries,
            &parameters,
            &[ts_a.clone(), ts_b.clone()],
        )
        .unwrap();
        assert_ne!(vec[0], var[0]);
        let rank = execute_operation(
            StatisticsOperation::VecRankTest,
            ScientificApi::TimeSeries,
            &parameters,
            &[ts_a, ts_b],
        )
        .unwrap();
        assert_ne!(rank[0], vec[0]);
    }
}
