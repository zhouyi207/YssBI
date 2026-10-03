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
        model["constant"] = json!(fit.constant);
        model["dfk"] = json!(fit.dfk);
        model["exogenousNames"] = json!(fit.exogenous_names);
        model["sampleRows"] = json!(fit.sample_rows);
        model["sigma"] = json!(fit.sigma);
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
    let mut report = Value::Object(report);
    decorate(
        &mut report,
        &fit.var_names,
        &fit.coefficients,
        &fit.statistics,
    )?;
    Ok(report)
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
    let mut report = Value::Object(report);
    decorate(
        &mut report,
        &fit.var_names,
        &fit.coefficients,
        &fit.statistics,
    )?;
    Ok(report)
}

pub fn var_granger(fit: &VarFit) -> Result<Value, SciError> {
    Ok(json!({"vargranger": yss_sci::time_series::var::granger(fit)
        .map_err(|_| computation_failed(SciOperationCode::VarFit))?}))
}

pub fn var_impulse_responses(fit: &VarFit, steps: usize) -> Result<Value, SciError> {
    let data = yss_sci::time_series::var::impulse_responses(fit, steps)
        .map_err(|_| computation_failed(SciOperationCode::VarFit))?;
    let mut report = json!({"responseNames":fit.var_names,"impulseNames":fit.var_names,"horizons":(0..=steps).collect::<Vec<_>>(),"oirf":data});
    report["response_rows"] = json!(named_response_rows(&fit.var_names, &data));
    crate::report_display::section(
        &mut report,
        "irf",
        "Orthogonalized impulse responses",
        "table",
        "/response_rows",
        &[
            ("horizon", "Horizon"),
            ("response", "Response"),
            ("impulse", "Impulse"),
            ("value", "Response"),
        ],
    );
    Ok(report)
}
pub fn var_variance_decomposition(fit: &VarFit, steps: usize) -> Result<Value, SciError> {
    let data = yss_sci::time_series::var::variance_decomposition(fit, steps)
        .map_err(|_| computation_failed(SciOperationCode::VarFit))?;
    let mut report = json!({"responseNames":fit.var_names,"impulseNames":fit.var_names,"horizons":(0..=steps).collect::<Vec<_>>(),"fevd":data});
    report["response_rows"] = json!(named_response_rows(&fit.var_names, &data));
    crate::report_display::section(
        &mut report,
        "fevd",
        "Forecast-error variance decomposition",
        "table",
        "/response_rows",
        &[
            ("horizon", "Horizon"),
            ("response", "Response"),
            ("impulse", "Impulse"),
            ("value", "Variance share"),
        ],
    );
    Ok(report)
}
fn named_response_rows(names: &[String], values: &[Vec<Vec<f64>>]) -> Vec<Value> {
    values.iter().enumerate().flat_map(|(h,m)|m.iter().enumerate().flat_map(move |(r,row)|row.iter().enumerate().map(move |(i,v)|json!({"horizon":h,"response":names[r],"impulse":names[i],"value":v})))).collect()
}
fn decorate(
    report: &mut Value,
    names: &[String],
    estimates: &[Vec<f64>],
    statistics: &MultivariateStatistics,
) -> Result<(), SciError> {
    use crate::report_display::{coefficient_rows, equation, section};
    if estimates.len() != statistics.coefficients.len()
        || estimates.len() != statistics.coefficient_labels.len()
        || estimates.len() != statistics.equations.len()
        || estimates.len() != names.len()
    {
        return Err(computation_failed(SciOperationCode::VarFit));
    }
    if report.get("coefficients").is_some() {
        let mut rows = Vec::new();
        let mut equations = Vec::new();
        for (eq, (b, stats)) in estimates.iter().zip(&statistics.coefficients).enumerate() {
            let name = &statistics.equations[eq].eq_name;
            let labels = &statistics.coefficient_labels[eq];
            equations.push(equation(name, labels, b));
            for mut row in coefficient_rows(labels, b, stats)? {
                row["equation"] = json!(name);
                rows.push(row);
            }
        }
        report["coefficient_rows"] = json!(rows);
        report["model_equations"] = json!(equations.join("\n"));
        section(
            report,
            "coefficients",
            "Equation coefficients",
            "table",
            "/coefficient_rows",
            &[
                ("equation", "Equation"),
                ("variable", "Variable"),
                ("estimate", "Coefficient"),
                ("standard_error", "Std. error"),
                ("statistic", "z"),
                ("p_value", "p-value"),
                ("ci_lower", "95% CI lower"),
                ("ci_upper", "95% CI upper"),
            ],
        );
        section(
            report,
            "equations",
            "Model equations",
            "equation",
            "/model_equations",
            &[],
        );
    }
    if report.get("stability").is_some() {
        section(
            report,
            "stability",
            "Companion eigenvalues and unit circle",
            "stability",
            "/stability",
            &[("re", "Real"), ("im", "Imaginary"), ("modulus", "Modulus")],
        );
    }
    if let Some(matrix) = report["model"]["sigma"].as_array() {
        if matrix.len() != names.len()
            || matrix
                .iter()
                .any(|row| row.as_array().is_none_or(|row| row.len() != names.len()))
        {
            return Err(computation_failed(SciOperationCode::VarFit));
        }
        report["sigma_rows"] =
            json!(
                matrix
                    .iter()
                    .enumerate()
                    .flat_map(|(i, r)| r.as_array().into_iter().flatten().enumerate().map(
                        move |(j, v)| json!({"row":names[i],"column":names[j],"covariance":v})
                    ))
                    .collect::<Vec<_>>()
            );
        section(
            report,
            "sigma",
            "Innovation covariance matrix",
            "table",
            "/sigma_rows",
            &[
                ("row", "Response"),
                ("column", "Response"),
                ("covariance", "Covariance"),
            ],
        );
    }
    Ok(())
}
