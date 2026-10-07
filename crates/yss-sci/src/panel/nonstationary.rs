//! Fisher aggregation of individual ADF / residual Engle–Granger tests.
use super::data::{Result, failed, groups, parameter};
use crate::time_series::unit_root::adf_test;
use statrs::distribution::{ChiSquared, ContinuousCDF};
use yss_sci_contract::{execution::ScientificExecutionControl, panel::*};
use yss_sci_linalg::{Col, Mat, MatrixExt, Solve, matrix_rank};

use crate::time_series::mackinnon;

fn adf(
    values: &[f64],
    options: PanelTestOptions,
    control: &ScientificExecutionControl,
) -> Result<(f64, usize)> {
    let constant = options.deterministic != PanelDeterministic::None;
    let trend = options.deterministic == PanelDeterministic::Trend;
    if options.lags >= values.len() {
        return Err(parameter());
    }
    let columns = 1 + options.lags + usize::from(constant) + usize::from(trend);
    if values.len() < 4 || values.len() <= options.lags + 1 + columns {
        return Err(parameter());
    }
    control.check()?;
    // Reuse the ADF regression, but calibrate panel tests with the unit-root response surface,
    // including an estimated constant; a Student-t tail is not a unit-root null distribution.
    let result = adf_test(values, options.lags, constant, trend).map_err(|_| failed())?;
    control.check()?;
    if !result.test_statistic.is_finite() {
        return Err(failed());
    }
    Ok((result.test_statistic, result.num_obs))
}

fn combine(
    method: &str,
    options: PanelTestOptions,
    entity_tests: Vec<PanelEntityTest>,
) -> Result<PanelFisherTest> {
    let df = 2 * entity_tests.len();
    let statistic = if entity_tests.iter().any(|test| test.p_value == 0.0) {
        None
    } else {
        Some(
            -2.0 * entity_tests
                .iter()
                .map(|test| test.p_value.ln())
                .sum::<f64>(),
        )
    };
    let p_value = if let Some(statistic) = statistic {
        if !statistic.is_finite() {
            return Err(failed());
        }
        ChiSquared::new(df as f64)
            .map_err(|_| failed())?
            .sf(statistic)
    } else {
        0.0
    };
    Ok(PanelFisherTest {
        method: method.into(),
        deterministic: options.deterministic,
        lags: options.lags,
        observations: entity_tests.iter().map(|test| test.observations).sum(),
        statistic,
        degrees_of_freedom: df,
        p_value,
        entity_tests,
    })
}

pub fn fisher_unit_root(
    data: PanelData<'_>,
    options: PanelTestOptions,
    control: &ScientificExecutionControl,
) -> Result<PanelFisherTest> {
    if !data.predictors.is_empty() {
        return Err(parameter());
    }
    let groups = groups(&data, control)?;
    let mut tests = Vec::with_capacity(groups.len());
    for rows in groups {
        control.check()?;
        let values = rows.iter().map(|&i| data.response[i]).collect::<Vec<_>>();
        let (statistic, observations) = adf(&values, options, control)?;
        tests.push(PanelEntityTest {
            entity: data.entity[rows[0]],
            observations,
            statistic,
            p_value: mackinnon::p_value(statistic, tau_deterministic(options.deterministic), 1),
            cointegrating_coefficients: Vec::new(),
        });
    }
    control.check()?;
    combine("fisher_adf", options, tests)
}

pub fn fisher_cointegration(
    data: PanelData<'_>,
    options: PanelTestOptions,
    control: &ScientificExecutionControl,
) -> Result<PanelFisherTest> {
    if !(1..=MAX_COINTEGRATION_PREDICTORS).contains(&data.predictors.len())
        || options.deterministic == PanelDeterministic::None
    {
        return Err(parameter());
    }
    let groups = groups(&data, control)?;
    let deterministic = if options.deterministic == PanelDeterministic::Trend {
        2
    } else {
        1
    };
    let width = data.predictors.len() + deterministic;
    let mut tests = Vec::with_capacity(groups.len());
    for rows in groups {
        control.check()?;
        let n = rows.len();
        if n <= width {
            return Err(parameter());
        }
        let x = Mat::from_fn(n, width, |i, j| {
            if j == 0 {
                1.0
            } else if j < deterministic {
                (i + 1) as f64
            } else {
                data.predictors[j - deterministic][rows[i]]
            }
        });
        if matrix_rank(x.as_ref()).map_err(|_| failed())?.0 != width {
            return Err(parameter());
        }
        let y = Col::from_iter(rows.iter().map(|&i| data.response[i]));
        let cross = x.transpose() * x.as_ref();
        let beta = cross
            .checked_cholesky()
            .map_err(|_| failed())?
            .solve(&(x.transpose() * y.as_ref()));
        let residual = y.as_ref() - (x.as_ref() * beta.as_ref()).as_ref();
        let residual = residual.iter().copied().collect::<Vec<_>>();
        let (statistic, observations) = adf(
            &residual,
            PanelTestOptions {
                lags: options.lags,
                deterministic: PanelDeterministic::None,
            },
            control,
        )?;
        let coefficients = beta.iter().copied().collect::<Vec<_>>();
        if coefficients.iter().any(|v| !v.is_finite()) {
            return Err(failed());
        }
        tests.push(PanelEntityTest {
            entity: data.entity[rows[0]],
            observations,
            statistic,
            p_value: mackinnon::p_value(
                statistic,
                tau_deterministic(options.deterministic),
                data.predictors.len() + 1,
            ),
            cointegrating_coefficients: coefficients,
        });
    }
    control.check()?;
    combine("fisher_engle_granger", options, tests)
}

fn tau_deterministic(
    value: PanelDeterministic,
) -> yss_sci_contract::time_series::forecast::Deterministic {
    use yss_sci_contract::time_series::forecast::Deterministic;
    match value {
        PanelDeterministic::None => Deterministic::None,
        PanelDeterministic::Constant => Deterministic::Constant,
        PanelDeterministic::Trend => Deterministic::Trend,
    }
}
