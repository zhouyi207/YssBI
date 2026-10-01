use crate::error::{computation_failed, invalid_input};
use crate::time_series::unit_root::{AdfResult, adf_test};
use crate::time_series::var::{VAR, VARConfig, VARSocResult, VarFit, var_varsoc};
use crate::time_series::vec::{
    VECConfig, VecFit, VecRankResult, VecTrendSpec, vec_estimate, vec_vecrank_stats,
};
use yss_sci_contract::{SciError, SciInputViolation, SciOperationCode};
use yss_sci_linalg::Mat;
pub fn augmented_dickey_fuller(
    series: &[f64],
    lags: usize,
    regression: &str,
) -> Result<AdfResult, SciError> {
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
    Ok(result)
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

pub fn var_fit(series: Vec<Vec<f64>>, lags: usize) -> Result<VarFit, SciError> {
    let names = (0..series.len()).map(|i| format!("y{i}")).collect();
    var_fit_configured(
        series,
        vec![],
        yss_sci_contract::time_series::var::VarOptions {
            lags: (1..=lags).collect(),
            constant: true,
            dfk: false,
            variable_names: names,
            exogenous_names: vec![],
        },
    )
}

pub fn var_fit_configured(
    series: Vec<Vec<f64>>,
    exogenous: Vec<Vec<f64>>,
    options: yss_sci_contract::time_series::var::VarOptions,
) -> Result<VarFit, SciError> {
    let op = SciOperationCode::VarFit;
    let n = series.first().map_or(0, Vec::len);
    if options.lags.is_empty()
        || options.lags.contains(&0)
        || options.lags.windows(2).any(|w| w[0] >= w[1])
        || options.variable_names.len() != series.len()
        || options.exogenous_names.len() != exogenous.len()
        || exogenous
            .iter()
            .any(|x| x.len() != n || x.iter().any(|v| !v.is_finite()))
    {
        return Err(invalid_input(op, SciInputViolation::ParameterOutOfRange));
    }
    let exog =
        (!exogenous.is_empty()).then(|| Mat::from_fn(n, exogenous.len(), |i, j| exogenous[j][i]));
    VAR {
        y: multivariate_series(series, op)?,
        exog,
        config: VARConfig {
            constant: options.constant,
            lags: options.lags,
            dfk: options.dfk,
            sample_start_offset: None,
        },
        var_names: Some(options.variable_names),
        exog_names: Some(options.exogenous_names),
        regression_times: None,
    }
    .fit()
    .map_err(|_| computation_failed(op))
}

pub fn var_lag_order(series: Vec<Vec<f64>>, max_lags: usize) -> Result<VARSocResult, SciError> {
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
    Ok(result)
}

pub fn vec_fit(
    series: Vec<Vec<f64>>,
    rank: usize,
    lags: usize,
    trend: &str,
) -> Result<VecFit, SciError> {
    let names = (0..series.len()).map(|i| format!("y{i}")).collect();
    vec_fit_named(series, rank, lags, trend, names)
}

pub fn vec_fit_named(
    series: Vec<Vec<f64>>,
    rank: usize,
    lags: usize,
    trend: &str,
    names: Vec<String>,
) -> Result<VecFit, SciError> {
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
        },
        Some(names),
        None,
    )
    .map_err(|_| computation_failed(SciOperationCode::VecFit))?;
    Ok(result)
}

pub fn vec_rank_test(
    series: Vec<Vec<f64>>,
    lags: usize,
    trend: &str,
) -> Result<VecRankResult, SciError> {
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
    Ok(result)
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
