//! Selected report projections and independently requested postestimation.
use crate::error::computation_failed;
use serde_json::{Map, Value, json};
use yss_sci_contract::time_series::{
    fit::MultivariateStatistics,
    var::{VarFit, VarSummaryOptions},
    vec::{VecFit, VecSummaryOptions},
};
use yss_sci_contract::{SciError, SciOperationCode};

fn model_summary(names: &[String], statistics: &MultivariateStatistics) -> Value {
    json!({
        "variables": names, "observations": statistics.observations,
        "logLikelihood": statistics.log_likelihood, "aic": statistics.aic,
        "hqic": statistics.hqic, "sbic": statistics.sbic,
        "detSigmaMl": statistics.det_sigma_ml,
    })
}

fn coefficients(estimates: &[Vec<f64>], statistics: &MultivariateStatistics) -> Value {
    json!({"estimates": estimates, "inference": statistics.coefficients,
        "labels": statistics.coefficient_labels, "equations": statistics.equations})
}

pub fn var_summary(fit: &VarFit, options: VarSummaryOptions) -> Result<Value, SciError> {
    use yss_sci::time_series::var;
    let failure = |_| computation_failed(SciOperationCode::VarFit);
    let mut report = Map::new();
    if options.model_summary {
        let mut model = model_summary(&fit.var_names, &fit.statistics);
        model["fpe"] = json!(fit.fpe);
        model["lags"] = json!(fit.lags);
        report.insert("model".into(), model);
    }
    if options.coefficient_table {
        report.insert(
            "coefficients".into(),
            coefficients(&fit.coefficients, &fit.statistics),
        );
    }
    if options.lag_exclusion {
        report.insert(
            "lagExclusion".into(),
            json!(var::lag_exclusion(fit).map_err(failure)?),
        );
    }
    if options.serial_tests {
        report.insert(
            "serialTests".into(),
            json!(var::serial_correlation(fit, options.serial_lags).map_err(failure)?),
        );
    }
    if options.stability {
        report.insert(
            "stability".into(),
            json!(var::stability(fit).map_err(failure)?),
        );
    }
    Ok(report.into())
}

pub fn vec_summary(fit: &VecFit, options: VecSummaryOptions) -> Result<Value, SciError> {
    use yss_sci::time_series::vec;
    let failure = |_| computation_failed(SciOperationCode::VecFit);
    let mut report = Map::new();
    if options.model_summary {
        let mut model = model_summary(&fit.var_names, &fit.statistics);
        model["rank"] = json!(fit.rank);
        model["lags"] = json!(fit.lags);
        model["trend"] = json!(fit.trend_spec);
        report.insert("model".into(), model);
    }
    if options.coefficient_table {
        report.insert(
            "coefficients".into(),
            coefficients(&fit.coefficients, &fit.statistics),
        );
    }
    if options.cointegration {
        report.insert("cointegration".into(), json!(fit.cointegration));
    }
    if options.serial_tests {
        report.insert(
            "serialTests".into(),
            json!(vec::serial_correlation(fit, options.serial_lags).map_err(failure)?),
        );
    }
    if options.stability {
        report.insert(
            "stability".into(),
            json!(vec::stability(fit).map_err(failure)?),
        );
    }
    Ok(report.into())
}

pub fn var_granger(fit: &VarFit) -> Result<Value, SciError> {
    Ok(json!({"vargranger": yss_sci::time_series::var::granger(fit)
        .map_err(|_| computation_failed(SciOperationCode::VarFit))?}))
}

pub fn var_impulse_responses(fit: &VarFit, steps: usize) -> Result<Value, SciError> {
    Ok(
        json!({"oirf": yss_sci::time_series::var::impulse_responses(fit, steps)
        .map_err(|_| computation_failed(SciOperationCode::VarFit))?}),
    )
}

pub fn var_variance_decomposition(fit: &VarFit, steps: usize) -> Result<Value, SciError> {
    Ok(
        json!({"fevd": yss_sci::time_series::var::variance_decomposition(fit, steps)
        .map_err(|_| computation_failed(SciOperationCode::VarFit))?}),
    )
}
