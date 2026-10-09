//! Encode scientific time-series fits for runtime callers.
use crate::error::computation_failed;
use yss_sci_contract::{SciError, SciOperationCode};

pub fn augmented_dickey_fuller(
    series: &[f64],
    lags: usize,
    regression: &str,
) -> Result<serde_json::Value, SciError> {
    let result = yss_sci::time_series::models::augmented_dickey_fuller(series, lags, regression)?;
    let mut report = serde_json::json!({
        "operation": "adf",
        "regression": regression,
        "useTDistribution": result.use_t_distribution,
        "laggedLevelCoefficient": result.coef_lagged,
        "laggedLevelStandardError": result.std_err_lagged,
        "regressionTable": result.regression_table.iter().map(|r|serde_json::json!({"variable":r.variable,"coefficient":r.coef,"standardError":r.std_err,"t":r.t,"pValue":r.p_value,"ciLower":r.ci_lower,"ciUpper":r.ci_upper})).collect::<Vec<_>>(),
        "statistic": result.test_statistic,
        "pValue": result.p_value,
        "observations": result.num_obs,
        "lags": result.lags,
        "criticalValues": {
            "1%": result.critical_value_1pct,
            "5%": result.critical_value_5pct,
            "10%": result.critical_value_10pct,
        }
    });
    crate::report_display::section(
        &mut report,
        "auxiliary_regression",
        "ADF auxiliary regression",
        "table",
        "/regressionTable",
        &[
            ("variable", "Variable"),
            ("coefficient", "Coefficient"),
            ("standardError", "Std. error"),
            ("t", "t"),
            ("pValue", "Auxiliary-regression p-value"),
            ("ciLower", "95% CI lower"),
            ("ciUpper", "95% CI upper"),
        ],
    );
    Ok(report)
}

pub fn var_fit(series: Vec<Vec<f64>>, lags: usize) -> Result<serde_json::Value, SciError> {
    let result = yss_sci::time_series::models::var_fit(series, lags)?;
    serde_json::to_value(result).map_err(|_| computation_failed(SciOperationCode::VarFit))
}

pub fn var_lag_order(
    series: Vec<Vec<f64>>,
    max_lags: usize,
) -> Result<serde_json::Value, SciError> {
    let result = yss_sci::time_series::models::var_lag_order(series, max_lags)?;
    serde_json::to_value(result).map_err(|_| computation_failed(SciOperationCode::VarLagOrder))
}

pub fn vec_fit(
    series: Vec<Vec<f64>>,
    rank: usize,
    lags: usize,
    trend: &str,
) -> Result<serde_json::Value, SciError> {
    let result = yss_sci::time_series::models::vec_fit(series, rank, lags, trend)?;
    serde_json::to_value(result).map_err(|_| computation_failed(SciOperationCode::VecFit))
}

pub fn vec_rank_test(
    series: Vec<Vec<f64>>,
    lags: usize,
    trend: &str,
) -> Result<serde_json::Value, SciError> {
    let result = yss_sci::time_series::models::vec_rank_test(series, lags, trend)?;
    serde_json::to_value(result).map_err(|_| computation_failed(SciOperationCode::VecRank))
}

pub fn var_fit_configured(
    series: Vec<Vec<f64>>,
    exogenous: Vec<Vec<f64>>,
    options: yss_sci_contract::time_series::var::VarOptions,
) -> Result<serde_json::Value, SciError> {
    serde_json::to_value(yss_sci::time_series::models::var_fit_configured(
        series, exogenous, options,
    )?)
    .map_err(|_| computation_failed(SciOperationCode::VarFit))
}
pub fn vec_fit_named(
    series: Vec<Vec<f64>>,
    rank: usize,
    lags: usize,
    trend: &str,
    names: Vec<String>,
) -> Result<serde_json::Value, SciError> {
    serde_json::to_value(yss_sci::time_series::models::vec_fit_named(
        series, rank, lags, trend, names,
    )?)
    .map_err(|_| computation_failed(SciOperationCode::VecFit))
}

#[cfg(test)]
mod tests {
    use super::augmented_dickey_fuller;
    use yss_sci_contract::{SciError, SciOperationCode, execution::ScientificInputViolation};

    #[test]
    fn augmented_dickey_fuller_rejects_unknown_regression() {
        let series = [1.0, 1.4, 1.1, 1.8, 1.5, 2.2, 1.9, 2.6, 2.3, 3.0, 2.7, 3.4];

        let error = augmented_dickey_fuller(&series, 1, "unexpected").unwrap_err();

        assert_eq!(
            error,
            SciError::InvalidInput {
                operation: SciOperationCode::Adf,
                violation: ScientificInputViolation::ParameterOutOfRange,
            }
        );

        for lags in [series.len() - 1, usize::MAX] {
            assert_eq!(
                augmented_dickey_fuller(&series, lags, "constant").unwrap_err(),
                SciError::ComputationFailed {
                    operation: SciOperationCode::Adf,
                }
            );
        }
        for (values, lags, regression) in
            [(&series[..4], 0, "trend"), (&series[..5], 1, "constant")]
        {
            assert_eq!(
                augmented_dickey_fuller(values, lags, regression).unwrap_err(),
                SciError::ComputationFailed {
                    operation: SciOperationCode::Adf,
                }
            );
        }
    }
}
