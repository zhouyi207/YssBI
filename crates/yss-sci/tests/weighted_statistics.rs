use yss_sci::regression::linear::{GLS, GLSConfig, WLS, WLSConfig};
use yss_sci_linalg::{Col, Mat};

fn near(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-8, "{actual} != {expected}");
}

#[test]
fn automatic_hac_preserves_response_units_and_reference_bandwidths() {
    use yss_sci::regression::linear::fit::fit_linear_regression;
    use yss_sci_contract::regression::fit::RegressionStatistics;
    use yss_sci_contract::regression::linear::LinearRegressionMethod;
    use yss_sci_contract::regression::{OlsCovariance, OlsOptions};
    use yss_sci_contract::{MissingValuePolicy, StatisticalObservationMetadata};

    let near_statistic = |actual: f64, expected: f64| {
        assert!(
            (actual - expected).abs() < 1e-9 * expected.abs().max(1.0),
            "{actual} != {expected}"
        );
    };
    let predictors = vec![
        (0..64)
            .map(|i| if i & 1 == 0 { 1.0 } else { -1.0 })
            .collect::<Vec<_>>(),
    ];
    for (mask, ols_bandwidths, wls_bandwidths) in [
        (4_usize, [28, 60, 28], [28, 61, 29]),
        (48, [7, 22, 14], [6, 23, 15]),
    ] {
        // Exact rational score products and NW(1994) Table I constants give
        // these bandwidths. The pilot has 18 (Bartlett/Parzen) or 19 (QS) lags;
        // it estimates the moments and is not a cap on the selected bandwidth.
        let response = predictors[0]
            .iter()
            .enumerate()
            .map(|(i, &x)| {
                let score = if (i & mask).count_ones().is_multiple_of(2) {
                    1.0
                } else {
                    -1.0
                };
                1.0 + 0.5 * x + x * score
            })
            .collect::<Vec<_>>();
        for (method, bandwidths) in [
            (LinearRegressionMethod::Ols, ols_bandwidths),
            (
                LinearRegressionMethod::Wls {
                    weights: (0..64).map(|i| (1 + i % 3) as f64).collect(),
                },
                wls_bandwidths,
            ),
        ] {
            for (kernel, bandwidth) in ["bartlett", "parzen", "quadratic spectral"]
                .into_iter()
                .zip(bandwidths)
            {
                let fit = |unit: f64, bandwidth| {
                    fit_linear_regression(
                        response.iter().map(|y| y * unit).collect(),
                        &predictors,
                        OlsOptions {
                            constant: true,
                            covariance: OlsCovariance::Hac {
                                kernel: kernel.into(),
                                bandwidth,
                            },
                        },
                        method.clone(),
                        StatisticalObservationMetadata {
                            original_observation_count: 64,
                            used_observation_count: 64,
                            dropped_null_count: 0,
                            dropped_nan_count: 0,
                            missing_value_policy: MissingValuePolicy::Reject,
                        },
                    )
                    .unwrap_or_else(|error| panic!("{method:?} mask={mask} kernel={kernel} unit={unit} bandwidth={bandwidth:?}: {error:?}"))
                };
                let reference = fit(1.0, Some(bandwidth));
                let RegressionStatistics::Linear {
                    coefficients: expected,
                    model: expected_model,
                } = &reference.statistics
                else {
                    panic!("Expected linear statistics");
                };
                if mask == 4
                    && matches!(method, LinearRegressionMethod::Wls { .. })
                    && kernel == "quadratic spectral"
                {
                    // Independent full-lag QS covariance. Cutting off at bandwidth
                    // made this positive-weight fit fail its coefficient inference.
                    near(expected.covariance[0][0], 0.0003147865539433194);
                    near(expected.covariance[0][1], 0.0002744964033211535);
                    near(expected.covariance[1][1], 0.0015114840365360392);
                }
                for unit in [1.0, 1e-12, 1e-151, 1e100] {
                    let actual = fit(unit, None);
                    let RegressionStatistics::Linear {
                        coefficients,
                        model,
                    } = actual.statistics
                    else {
                        panic!("Expected linear statistics");
                    };
                    for j in 0..2 {
                        near(actual.coefficients[j] / unit, reference.coefficients[j]);
                        near(
                            coefficients.standard_errors[j] / unit,
                            expected.standard_errors[j],
                        );
                        near_statistic(
                            coefficients.statistic_values[j],
                            expected.statistic_values[j],
                        );
                        near(coefficients.p_values[j], expected.p_values[j]);
                        for k in 0..2 {
                            near(
                                coefficients.covariance[j][k] / unit / unit,
                                expected.covariance[j][k],
                            );
                        }
                    }
                    near_statistic(model.f_statistic, expected_model.f_statistic);
                    near(model.f_p_value, expected_model.f_p_value);
                }
            }
        }
    }
}

#[test]
fn automatic_hac_rejects_undefined_pilot_covariance() {
    use yss_sci::regression::covariance::compute_cov_beta;
    use yss_sci_contract::regression::OlsCovariance;

    let design = Mat::from_fn(2, 1, |_, _| 1.0);
    let inverse = Mat::from_fn(1, 1, |_, _| 0.5);
    for unit in [1.0, 1e-151, 1e100] {
        // gamma(0) + 2*gamma(1) = 0 with nonzero score variation.
        let residuals = Col::from_iter([unit, -unit]);
        let covariance = |bandwidth| {
            compute_cov_beta(
                &design,
                &inverse,
                residuals.as_ref(),
                1,
                Some(0),
                &OlsCovariance::Hac {
                    kernel: "bartlett".into(),
                    bandwidth,
                },
            )
        };
        assert!(
            covariance(None)
                .unwrap_err()
                .contains("undefined pilot covariance")
        );
        near(covariance(Some(1)).unwrap()[(0, 0)] / unit / unit, 0.5);
    }
}

#[test]
fn gls_identity_estimates_scale_like_ols() {
    use yss_sci::regression::linear::fit::fit_linear_regression;
    use yss_sci_contract::regression::fit::RegressionStatistics;
    use yss_sci_contract::regression::linear::LinearRegressionMethod;
    use yss_sci_contract::{MissingValuePolicy, StatisticalObservationMetadata};

    let fit = |method| {
        fit_linear_regression(
            vec![1., 2., 1., 4., 3.],
            &[vec![0., 1., 2., 3., 4.]],
            Default::default(),
            method,
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
    let ols = fit(LinearRegressionMethod::Ols);
    let gls = fit(LinearRegressionMethod::Gls {
        sigma: (0..5)
            .map(|i| (0..5).map(|j| f64::from(i == j)).collect())
            .collect(),
    });
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

    for method in [
        LinearRegressionMethod::Ols,
        LinearRegressionMethod::Wls {
            weights: vec![1.0; 5],
        },
        LinearRegressionMethod::Gls {
            sigma: (0..5)
                .map(|i| (0..5).map(|j| f64::from(i == j)).collect())
                .collect(),
        },
    ] {
        let fit = fit_linear_regression(
            vec![1., 2., 1., 4., 3.],
            &[vec![1.0; 5]],
            yss_sci_contract::regression::OlsOptions {
                constant: false,
                ..Default::default()
            },
            method,
            StatisticalObservationMetadata {
                original_observation_count: 5,
                used_observation_count: 5,
                dropped_null_count: 0,
                dropped_nan_count: 0,
                missing_value_policy: MissingValuePolicy::Reject,
            },
        )
        .unwrap();
        assert!(!fit.constant);
        assert_eq!(fit.parameter_names, ["x1"]);
        assert_eq!(fit.design, [vec![1.0; 5]]);
    }
}

#[test]
fn weighted_statistics_use_the_transformed_intercept() {
    use yss_sci::regression::covariance::compute_cov_beta;
    use yss_sci_contract::regression::OlsCovariance;
    use yss_sci_linalg::{MatrixExt, Solve};
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

    let data: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/diagnostics_reference.json")).unwrap();
    let response: Vec<f64> = serde_json::from_value(data["y"].clone()).unwrap();
    let predictors: Vec<Vec<f64>> = serde_json::from_value(data["x"].clone()).unwrap();
    let weights: Vec<f64> = serde_json::from_value(data["weights"].clone()).unwrap();
    let n = response.len();
    let design = Mat::from_fn(n, 3, |i, j| if j == 0 { 1.0 } else { predictors[j - 1][i] });
    let fit = |order: [usize; 3]| {
        WLS {
            endog: Col::from_iter(response.iter().copied()),
            exog: Mat::from_fn(n, 3, |i, j| design[(i, order[j])]),
            weights: Col::from_iter(weights.iter().copied()),
            config: WLSConfig {
                constant: true,
                covariance: OlsCovariance::Hac {
                    kernel: "bartlett".into(),
                    bandwidth: None,
                },
            },
        }
        .fit()
        .unwrap()
    };
    let expected = fit([0, 1, 2]);
    let order = [0, 2, 1];
    let reordered = fit(order);
    for i in 0..3 {
        for j in 0..3 {
            near(
                reordered.cov_beta[(i, j)],
                expected.cov_beta[(order[i], order[j])],
            );
        }
    }

    // The configured intercept remains column zero after whitening; its values
    // are no longer constant. Check forwarding to the covariance owner directly.
    let transformed = Mat::from_fn(n, 3, |i, j| design[(i, j)] * weights[i].sqrt());
    let residuals = Col::from_fn(n, |i| {
        (response[i]
            - (0..3)
                .map(|j| design[(i, j)] * expected.betas[j])
                .sum::<f64>())
            * weights[i].sqrt()
    });
    let inverse = (transformed.transpose() * transformed.as_ref())
        .checked_cholesky()
        .unwrap()
        .solve(&Mat::identity(3, 3));
    let covariance = |intercept_col| {
        compute_cov_beta(
            &transformed,
            &inverse,
            residuals.as_ref(),
            expected.df_residual,
            intercept_col,
            &OlsCovariance::Hac {
                kernel: "bartlett".into(),
                bandwidth: None,
            },
        )
        .unwrap()
    };
    let configured = covariance(Some(0));
    let omitted = covariance(None);
    for i in 0..3 {
        for j in 0..3 {
            near(expected.cov_beta[(i, j)], configured[(i, j)]);
        }
    }
    assert!((configured[(0, 0)] - omitted[(0, 0)]).abs() > 1e-8);
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
