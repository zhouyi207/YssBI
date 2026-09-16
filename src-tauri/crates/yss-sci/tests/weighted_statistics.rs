use yss_sci::regression::linear_model::{GLS, GLSConfig, WLS, WLSConfig};
use yss_sci_linalg::{Col, Mat};

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-8, "{actual} != {expected}");
}

#[test]
fn unified_gls_identity_estimates_scale_like_ols() {
    use yss_sci::regression::fit::fit_regression;
    use yss_sci_contract::regression::fit::{RegressionKind, RegressionStatistics};
    use yss_sci_contract::{MissingValuePolicy, StatisticalObservationMetadata};

    let fit = |kind| {
        fit_regression(
            kind,
            vec![1., 2., 1., 4., 3.],
            vec![vec![0., 1., 2., 3., 4.]],
            None,
            StatisticalObservationMetadata {
                original_observation_count: 5,
                used_observation_count: 5,
                dropped_null_count: 0,
                dropped_nan_count: 0,
                missing_value_policy: MissingValuePolicy::Reject,
            },
        )
        .unwrap()
    };
    let ols = fit(RegressionKind::Ols);
    let gls = fit(RegressionKind::Gls);
    assert_eq!(gls.family, "gls");
    let (
        RegressionStatistics::Linear {
            coefficients: expected,
            model: expected_model,
        },
        RegressionStatistics::Linear {
            coefficients: actual,
            model: actual_model,
        },
    ) = (ols.statistics, gls.statistics)
    else {
        panic!("Expected linear results")
    };
    assert_eq!(actual_model.covariance_type, "GLS (estimated scale)");
    for i in 0..2 {
        near(actual.standard_errors[i], expected.standard_errors[i]);
        near(actual.statistic_values[i], expected.statistic_values[i]);
        near(actual.p_values[i], expected.p_values[i]);
        near(
            actual.confidence_interval_lower[i],
            expected.confidence_interval_lower[i],
        );
        near(
            actual.confidence_interval_upper[i],
            expected.confidence_interval_upper[i],
        );
        for j in 0..2 {
            near(actual.covariance[i][j], expected.covariance[i][j]);
        }
    }
    near(actual_model.f_statistic, expected_model.f_statistic);
    near(actual_model.f_p_value, expected_model.f_p_value);
}

#[test]
fn weighted_statistics_use_the_transformed_intercept() {
    let weights = Col::from_fn(5, |i| (1 << i) as f64);
    for constant in [true, false] {
        let exog = Mat::from_fn(5, if constant { 2 } else { 1 }, |i, j| {
            if constant && j == 0 { 1.0 } else { i as f64 }
        });
        for offset in [0.0, 100.0] {
            let endog = Col::from_fn(5, |i| [1., 2., 1., 4., 3.][i] + offset);
            let wls = WLS {
                endog: endog.clone(),
                exog: exog.clone(),
                weights: weights.clone(),
                config: WLSConfig {
                    constant,
                    covariance: Default::default(),
                },
            }
            .fit()
            .unwrap();
            let gls = GLS {
                endog: endog.clone(),
                exog: exog.clone(),
                sigma: Mat::from_fn(5, 5, |i, j| if i == j { 1.0 / weights[i] } else { 0.0 }),
                config: GLSConfig { constant },
            }
            .fit()
            .unwrap();
            near(wls.ss_total, gls.ss_total);
            near(wls.r2, gls.r2);
            near(wls.fvalue, gls.fvalue);
            near(wls.f_p_value, gls.f_p_value);
            for j in 0..exog.ncols() {
                near(wls.stds[j], gls.stds[j]);
                near(wls.tvalues[j], gls.tvalues[j]);
                near(wls.pvalues[j], gls.pvalues[j]);
                near(wls.conf_int_left[j], gls.conf_int_left[j]);
                near(wls.conf_int_right[j], gls.conf_int_right[j]);
            }
            if constant {
                // Exact weighted least-squares values derived using rational arithmetic.
                near(wls.ss_total, 914.0 / 31.0);
                near(wls.r2, 4913.0 / 15081.0);
                near(wls.fvalue, 14739.0 / 10168.0);
            } else {
                near(
                    wls.ss_total,
                    (0..5).map(|i| weights[i] * endog[i].powi(2)).sum(),
                );
            }
        }
    }
}

#[test]
fn correlated_gls_centers_against_the_whitened_constant() {
    // Sigma = L L', with L[0,0]=2, L[1,0]=1 and other diagonal entries 1.
    let sigma = Mat::from_fn(5, 5, |i, j| match (i, j) {
        (0, 0) => 4.0,
        (1, 1) | (0, 1) | (1, 0) => 2.0,
        _ if i == j => 1.0,
        _ => 0.0,
    });
    // L^-1 * 1 = [.5,.5,1,1,1]; L^-1 * y = [.5,1.5,1,4,3].
    let c = [0.5, 0.5, 1.0, 1.0, 1.0];
    let z = [0.5, 1.5, 1.0, 4.0, 3.0];
    let mean: f64 = 9.0 / 3.5;
    let expected = (0..5).map(|i| (z[i] - mean * c[i]).powi(2)).sum();
    for offset in [0.0, 100.0] {
        let result = GLS {
            endog: Col::from_fn(5, |i| [1., 2., 1., 4., 3.][i] + offset),
            exog: Mat::from_fn(5, 2, |i, j| if j == 0 { 1.0 } else { i as f64 }),
            sigma: sigma.clone(),
            config: GLSConfig { constant: true },
        }
        .fit()
        .unwrap();
        near(result.ss_total, expected);
        let scaled = GLS {
            endog: Col::from_fn(5, |i| [1., 2., 1., 4., 3.][i] + offset),
            exog: Mat::from_fn(5, 2, |i, j| if j == 0 { 1.0 } else { i as f64 }),
            sigma: yss_sci_linalg::Scale(7.0) * &sigma,
            config: GLSConfig { constant: true },
        }
        .fit()
        .unwrap();
        for j in 0..2 {
            near(result.betas[j], scaled.betas[j]);
            near(result.stds[j], scaled.stds[j]);
            near(result.pvalues[j], scaled.pvalues[j]);
            near(result.conf_int_left[j], scaled.conf_int_left[j]);
            near(result.conf_int_right[j], scaled.conf_int_right[j]);
        }
        near(result.fvalue, scaled.fvalue);
        near(result.f_p_value, scaled.f_p_value);
    }
}
