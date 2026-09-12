use crate::error::{computation_failed, invalid_input};
use faer::Mat;
use yss_sci::ts::unit_root::adf_test;
use yss_sci::ts::var::{VAR, VARConfig, var_varsoc};
use yss_sci::ts::vec::{VECConfig, VecTrendSpec, vec_estimate, vec_vecrank_stats};
use yss_sci_contract::{SciError, SciInputViolation, SciOperationCode};
pub fn augmented_dickey_fuller(
    series: &[f64],
    lags: usize,
    regression: &str,
) -> Result<serde_json::Value, SciError> {
    let (constant, trend) = match regression {
        "none" | "no_constant" => (false, false),
        "constant" => (true, false),
        "trend" => (true, true),
        _ => {
            return Err(invalid_input(
                SciOperationCode::Adf,
                SciInputViolation::ParameterOutOfRange,
            ));
        }
    };
    let result = adf_test(series, lags, constant, trend)
        .map_err(|_| computation_failed(SciOperationCode::Adf))?;
    serde_json::to_value(serde_json::json!({
        "operation": "adf",
        "statistic": result.test_statistic,
        "pValue": result.p_value,
        "observations": result.num_obs,
        "lags": result.lags,
        "criticalValues": {
            "1%": result.critical_value_1pct,
            "5%": result.critical_value_5pct,
            "10%": result.critical_value_10pct,
        }
    }))
    .map_err(|_| computation_failed(SciOperationCode::Adf))
}

fn multivariate_series(
    series: Vec<Vec<f64>>,
    operation: SciOperationCode,
) -> Result<Mat<f64>, SciError> {
    let observations = series.first().map(Vec::len).unwrap_or(0);
    if series.len() < 2 || observations == 0 {
        return Err(invalid_input(operation, SciInputViolation::EmptyInput));
    }
    if series.iter().any(|item| item.len() != observations) {
        return Err(invalid_input(operation, SciInputViolation::ShapeMismatch));
    }
    Ok(Mat::from_fn(observations, series.len(), |row, col| {
        series[col][row]
    }))
}

pub fn var_fit(series: Vec<Vec<f64>>, lags: usize) -> Result<serde_json::Value, SciError> {
    if lags == 0 {
        return Err(invalid_input(
            SciOperationCode::VarFit,
            SciInputViolation::ParameterOutOfRange,
        ));
    }
    let y = multivariate_series(series, SciOperationCode::VarFit)?;
    let result = VAR {
        y,
        exog: None,
        config: VARConfig {
            constant: true,
            lags: (1..=lags).collect(),
            step: 8,
            dfk: false,
            mlag: 2,
            sample_start_offset: None,
            skip_extras: false,
        },
        var_names: None,
        exog_names: None,
        regression_times: None,
    }
    .fit()
    .map_err(|_| computation_failed(SciOperationCode::VarFit))?;
    serde_json::to_value(result).map_err(|_| computation_failed(SciOperationCode::VarFit))
}

pub fn var_lag_order(
    series: Vec<Vec<f64>>,
    max_lags: usize,
) -> Result<serde_json::Value, SciError> {
    if max_lags == 0 {
        return Err(invalid_input(
            SciOperationCode::VarLagOrder,
            SciInputViolation::ParameterOutOfRange,
        ));
    }
    let result = var_varsoc(
        multivariate_series(series, SciOperationCode::VarLagOrder)?,
        max_lags,
        None,
    )
    .map_err(|_| computation_failed(SciOperationCode::VarLagOrder))?;
    serde_json::to_value(result).map_err(|_| computation_failed(SciOperationCode::VarLagOrder))
}

pub fn vec_fit(
    series: Vec<Vec<f64>>,
    rank: usize,
    lags: usize,
    trend: &str,
) -> Result<serde_json::Value, SciError> {
    if lags == 0 {
        return Err(invalid_input(
            SciOperationCode::VecFit,
            SciInputViolation::ParameterOutOfRange,
        ));
    }
    let result = vec_estimate(
        &multivariate_series(series, SciOperationCode::VecFit)?,
        &VECConfig {
            trend_spec: vec_trend(trend, SciOperationCode::VecFit)?,
            lags,
            rank,
            mlag: 2,
        },
        None,
        None,
    )
    .map_err(|_| computation_failed(SciOperationCode::VecFit))?;
    serde_json::to_value(result).map_err(|_| computation_failed(SciOperationCode::VecFit))
}

pub fn vec_rank_test(
    series: Vec<Vec<f64>>,
    lags: usize,
    trend: &str,
) -> Result<serde_json::Value, SciError> {
    if lags == 0 {
        return Err(invalid_input(
            SciOperationCode::VecRank,
            SciInputViolation::ParameterOutOfRange,
        ));
    }
    let result = vec_vecrank_stats(
        &multivariate_series(series, SciOperationCode::VecRank)?,
        lags,
        vec_trend(trend, SciOperationCode::VecRank)?,
        None,
        true,
        None,
    )
    .map_err(|_| computation_failed(SciOperationCode::VecRank))?;
    serde_json::to_value(result).map_err(|_| computation_failed(SciOperationCode::VecRank))
}

fn vec_trend(trend: &str, operation: SciOperationCode) -> Result<VecTrendSpec, SciError> {
    match trend {
        "none" | "no_constant" => Ok(VecTrendSpec::None),
        "constant" => Ok(VecTrendSpec::Constant),
        "trend" => Ok(VecTrendSpec::Trend),
        _ => Err(invalid_input(
            operation,
            SciInputViolation::ParameterOutOfRange,
        )),
    }
}
