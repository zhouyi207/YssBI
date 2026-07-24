//! Statistical kernels over materialized protocol series and model values.

use super::KernelFragment;
use crate::node_system::protocol::Value;
use crate::node_system::runtime::{Kernel, KernelContext, KernelError, RuntimeValue};
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

#[derive(Debug, Clone, Default)]
pub struct StatisticsKernelParameters {
    pub lags: Option<usize>,
    pub max_lags: Option<usize>,
    pub rank: Option<usize>,
    pub trend: Option<Box<str>>,
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
            .map_err(|error| KernelError::new(error.to_string()))?;
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

fn numeric_series(value: &Value) -> Result<Vec<f64>, KernelError> {
    let Value::List(values) = value else {
        return Err(KernelError::new("expected numeric series"));
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

fn regression_fit(
    operation: StatisticsOperation,
    inputs: &[RuntimeValue],
) -> Result<crate::sci::api::node_statistics::RegressionFit, KernelError> {
    let response = inputs
        .first()
        .ok_or_else(|| KernelError::new("regression requires a response series"))
        .and_then(scalar)
        .and_then(numeric_series)?;
    let mut series = inputs[1..]
        .iter()
        .map(scalar)
        .filter_map(|value| match value {
            Ok(Value::List(_)) => Some(value.and_then(numeric_series)),
            Ok(Value::Null | Value::Object(_)) => None,
            Ok(_) => Some(Err(KernelError::new("regression input is not a series"))),
            Err(error) => Some(Err(error)),
        })
        .collect::<Result<Vec<_>, _>>()?;
    let weights = if matches!(
        operation,
        StatisticsOperation::WlsFit | StatisticsOperation::WlsSummary
    ) {
        Some(
            series
                .pop()
                .ok_or_else(|| KernelError::new("WLS requires weights"))?,
        )
    } else {
        None
    };
    crate::sci::api::node_statistics::fit_regression(
        regression_kind(operation).expect("regression operation"),
        response,
        series,
        weights,
    )
    .map_err(KernelError::new)
}

fn fit_values(
    fit: crate::sci::api::node_statistics::RegressionFit,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let model = protocol_value(serde_json::json!({
        "family": fit.family,
        "coefficients": fit.coefficients,
        "statistics": fit.statistics,
    }))?;
    Ok(vec![
        RuntimeValue::Scalar(model),
        RuntimeValue::Scalar(protocol_value(serde_json::json!(fit.fitted))?),
        RuntimeValue::Scalar(protocol_value(serde_json::json!(fit.residuals))?),
    ])
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
    inputs: &[RuntimeValue],
) -> Result<Value, KernelError> {
    use statrs::distribution::{ContinuousCDF, Normal};
    let Value::Object(model) = scalar(
        inputs
            .first()
            .ok_or_else(|| KernelError::new("prediction requires a model"))?,
    )?
    else {
        return Err(KernelError::new("prediction model is invalid"));
    };
    let coefficients = model
        .get("coefficients")
        .ok_or_else(|| KernelError::new("prediction model has no coefficients"))
        .and_then(numeric_series)?;
    let predictors = inputs[1..]
        .iter()
        .map(|input| scalar(input).and_then(numeric_series))
        .collect::<Result<Vec<_>, _>>()?;
    let observations = predictors.first().map(Vec::len).unwrap_or(0);
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
    protocol_value(serde_json::json!(values))
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
            fit_values(regression_fit(operation, inputs)?)
        }
        LinearPredict | LogitPredict | ProbitPredict => {
            Ok(vec![RuntimeValue::Scalar(prediction(operation, inputs)?)])
        }
        AdfTest => {
            let series = numeric_series(scalar(
                inputs
                    .first()
                    .ok_or_else(|| KernelError::new("ADF requires a series"))?,
            )?)?;
            let result = crate::sci::api::node_statistics::augmented_dickey_fuller(
                &series,
                parameters.lags.unwrap_or(1),
                parameters.trend.as_deref().unwrap_or("constant"),
            )
            .map_err(KernelError::new)?;
            Ok(vec![RuntimeValue::Scalar(protocol_value(result)?)])
        }
        VarLagOrder => {
            let series = inputs
                .iter()
                .map(|input| scalar(input).and_then(numeric_series))
                .collect::<Result<Vec<_>, _>>()?;
            require_constant_trend("VAR lag-order selection", parameters)?;
            let result = crate::sci::api::node_statistics::var_lag_order(
                series,
                parameters.max_lags.unwrap_or(4),
            )
            .map_err(KernelError::new)?;
            Ok(vec![RuntimeValue::Scalar(protocol_value(result)?)])
        }
        VecFit => {
            let series = inputs
                .iter()
                .filter_map(|input| match scalar(input) {
                    Ok(Value::List(_)) => Some(scalar(input).and_then(numeric_series)),
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let result = crate::sci::api::node_statistics::vec_fit(
                series,
                parameters.rank.unwrap_or(1),
                parameters.lags.unwrap_or(1),
                parameters.trend.as_deref().unwrap_or("constant"),
            )
            .map_err(KernelError::new)?;
            let model = protocol_value(result)?;
            Ok(vec![
                RuntimeValue::Scalar(model),
                RuntimeValue::Scalar(Value::Null),
                RuntimeValue::Scalar(Value::Null),
            ])
        }
        VecRankTest => {
            let series = inputs
                .iter()
                .map(|input| scalar(input).and_then(numeric_series))
                .collect::<Result<Vec<_>, _>>()?;
            let result = crate::sci::api::node_statistics::vec_rank_test(
                series,
                parameters.max_lags.unwrap_or(4),
                parameters.trend.as_deref().unwrap_or("constant"),
            )
            .map_err(KernelError::new)?;
            Ok(vec![RuntimeValue::Scalar(protocol_value(result)?)])
        }
        OlsSummary | GlsSummary | LogitSummary | PraisSummary | ProbitSummary | WlsSummary => {
            let fit = regression_fit(operation, inputs)?;
            let result = protocol_value(
                serde_json::to_value(&fit).map_err(|error| KernelError::new(error.to_string()))?,
            )?;
            let report = Value::String(format!("{} fit from yss_sci", fit.family).into());
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
            let series = inputs
                .iter()
                .filter_map(|input| match scalar(input) {
                    Ok(Value::List(_)) => Some(scalar(input).and_then(numeric_series)),
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                })
                .collect::<Result<Vec<_>, _>>()?;
            require_constant_trend("VAR summary", parameters)?;
            let result = crate::sci::api::node_statistics::var_lag_order(
                series,
                parameters.max_lags.unwrap_or(4),
            )
            .map_err(KernelError::new)?;
            Ok(vec![
                RuntimeValue::Scalar(protocol_value(result)?),
                RuntimeValue::Scalar(Value::String("VAR lag-order selection from yss_sci".into())),
            ])
        }
        Iv2slsSummary | IvLimlSummary => {
            let series = inputs
                .iter()
                .filter_map(|input| match scalar(input) {
                    Ok(Value::List(_)) => Some(scalar(input).and_then(numeric_series)),
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                })
                .collect::<Result<Vec<_>, _>>()?;
            if series.len() != 4 {
                return Err(KernelError::new(
                    "IV summary requires response, one exogenous predictor, one endogenous predictor, and one instrument",
                ));
            }
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
            .map_err(KernelError::new)?;
            Ok(vec![
                RuntimeValue::Scalar(protocol_value(result)?),
                RuntimeValue::Scalar(Value::String(
                    format!("{operation:?} result from yss_sci").into(),
                )),
            ])
        }
        PanelSummary | PanelDidTwfe => {
            let mut series = inputs
                .iter()
                .filter_map(|input| match scalar(input) {
                    Ok(Value::List(_)) => Some(scalar(input).and_then(numeric_series)),
                    Ok(_) => None,
                    Err(error) => Some(Err(error)),
                })
                .collect::<Result<Vec<_>, _>>()?;
            let required = if operation == PanelDidTwfe { 5 } else { 4 };
            if series.len() < required {
                return Err(KernelError::new(
                    "panel summary requires response, predictors, entity, time, and optional treatment series",
                ));
            }
            let treatment = (operation == PanelDidTwfe).then(|| series.pop().unwrap());
            let time = series.pop().unwrap();
            let entity = series.pop().unwrap();
            let response = series.remove(0);
            let result = crate::sci::api::node_statistics::fit_panel(
                response, series, entity, time, treatment,
            )
            .map_err(KernelError::new)?;
            Ok(vec![
                RuntimeValue::Scalar(protocol_value(result)?),
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
        RuntimeValue::Scalar(protocol_value(serde_json::json!(values)).unwrap())
    }

    fn object(value: &RuntimeValue) -> &BTreeMap<Box<str>, Value> {
        let RuntimeValue::Scalar(Value::Object(value)) = value else {
            panic!("expected object output");
        };
        value
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
                "r2",
            ),
            (
                StatisticsOperation::GlsFit,
                ScientificApi::Regression,
                vec![linear_y.clone(), x.clone()],
                "gls",
                "standardErrors",
            ),
            (
                StatisticsOperation::WlsFit,
                ScientificApi::Regression,
                vec![linear_y.clone(), x.clone(), weights],
                "wls",
                "standardErrors",
            ),
            (
                StatisticsOperation::PraisFit,
                ScientificApi::Regression,
                vec![correlated_y, x.clone()],
                "prais",
                "rho",
            ),
            (
                StatisticsOperation::LogitFit,
                ScientificApi::DiscreteRegression,
                vec![binary_y.clone(), x.clone()],
                "logit",
                "logLikelihood",
            ),
            (
                StatisticsOperation::ProbitFit,
                ScientificApi::DiscreteRegression,
                vec![binary_y, x],
                "probit",
                "logLikelihood",
            ),
        ];
        let mut coefficients = BTreeMap::new();
        for (operation, api, inputs, family, statistic) in cases {
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
            trend: Some("constant".into()),
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
