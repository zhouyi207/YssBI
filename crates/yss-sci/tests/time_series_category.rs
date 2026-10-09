use serde_json::Value;
use std::time::{Duration, Instant};
use yss_sci::time_series::forecast::*;
use yss_sci_contract::{
    execution::*, regression::models::IterationOptions, time_series::forecast::*,
};

fn fixture() -> Value {
    serde_json::from_str(include_str!("fixtures/time_series_reference.json")).unwrap()
}
fn vector(v: &Value) -> Vec<f64> {
    serde_json::from_value(v.clone()).unwrap()
}
fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(60),
    }
}
fn close(a: f64, b: f64, tol: f64) {
    assert!(
        (a - b).abs() <= tol * (1.0 + b.abs()),
        "{a} != {b} (tolerance {tol})"
    );
}
fn compare(a: &[f64], b: &Value, tol: f64) {
    let b = vector(b);
    assert_eq!(a.len(), b.len());
    for (a, b) in a.iter().zip(b) {
        close(*a, b, tol);
    }
}
fn iteration() -> IterationOptions {
    IterationOptions {
        max_iterations: 1000,
        tolerance: 1e-7,
    }
}
fn arima_options() -> ArimaOptions {
    ArimaOptions {
        p: 1,
        d: 1,
        q: 1,
        seasonal_p: 0,
        seasonal_d: 0,
        seasonal_q: 0,
        period: 1,
        constant: true,
        horizon: 6,
        confidence: 0.95,
        iteration: iteration(),
    }
}

#[test]
fn time_series_arima_css_and_integrated_intervals_match_scipy_reference() {
    let f = fixture();
    let y = vector(&f["series"]);
    assert!(y.len() > 512);
    for case in f["arima"].as_array().unwrap() {
        let read = |key: &str, i: usize| case[key][i].as_u64().unwrap() as usize;
        let o = ArimaOptions {
            p: read("order", 0),
            d: read("order", 1),
            q: read("order", 2),
            seasonal_p: read("seasonal", 0),
            seasonal_d: read("seasonal", 1),
            seasonal_q: read("seasonal", 2),
            period: read("seasonal", 3),
            ..arima_options()
        };
        let r = arima(&y, o, &control()).unwrap();
        let expected_parameters = vector(&case["coefficients"]);
        assert_eq!(r.parameters.len(), expected_parameters.len() + 1);
        for (actual, expected) in r.parameters.iter().zip(expected_parameters) {
            close(actual.estimate, expected, 2e-5);
        }
        compare(&r.forecasts, &case["forecasts"], 2e-5);
        compare(r.lower.as_ref().unwrap(), &case["lower"], 2e-5);
        close(
            r.innovation_variance,
            case["variance"].as_f64().unwrap(),
            1e-6,
        );
        let first = case["first_row"].as_u64().unwrap() as usize;
        assert!(r.fitted[..first].iter().all(Option::is_none));
        assert_eq!(r.effective_observations, y.len() - first);
        for (i, value) in y.iter().enumerate().skip(first) {
            close(
                r.fitted[i].unwrap() + r.residuals[i].unwrap(),
                *value,
                1e-12,
            );
        }
    }
}

#[test]
fn forecast_moment_units_keep_large_variance_likelihood_and_intervals_finite() {
    let units: f64 = 1e154;
    for difference in [0, 1] {
        let y = (0..640)
            .map(|i| {
                let value = if difference == 0 {
                    if i % 2 == 0 { -1.0 } else { 1.0 }
                } else {
                    (i % 2) as f64
                };
                value * units
            })
            .collect::<Vec<_>>();
        let result = arima(
            &y,
            ArimaOptions {
                p: 0,
                d: difference,
                q: 0,
                constant: false,
                ..arima_options()
            },
            &control(),
        )
        .expect("white-noise and random-walk reports are finite in these units");
        let n = y.len() - difference;
        assert_eq!(result.effective_observations, n);
        assert_eq!(result.iterations, 0);
        close(result.innovation_variance / (units * units), 1.0, 1e-12);
        let ll = -0.5 * n as f64 * ((2.0 * std::f64::consts::PI).ln() + 2.0 * units.ln() + 1.0);
        close(result.log_likelihood.unwrap(), ll, 1e-12);
        close(result.aic.unwrap(), -2.0 * ll + 2.0, 1e-12);
        close(result.bic.unwrap(), -2.0 * ll + (n as f64).ln(), 1e-12);
        assert_eq!(result.parameters[0].term, "innovation_variance");
        assert_eq!(result.parameters[0].estimate, result.innovation_variance);
        for i in 0..y.len() {
            if i < difference {
                assert!(result.fitted[i].is_none() && result.residuals[i].is_none());
            } else {
                let fitted = if difference == 0 { 0.0 } else { y[i - 1] };
                close(result.fitted[i].unwrap() / units, fitted / units, 1e-12);
                close(
                    result.residuals[i].unwrap() / units,
                    (y[i] - fitted) / units,
                    1e-12,
                );
            }
        }
        let point = if difference == 0 {
            0.0
        } else {
            *y.last().unwrap() / units
        };
        for (h, forecast) in result.forecasts.iter().enumerate() {
            close(forecast / units, point, 1e-12);
            let gain = if difference == 0 { 1.0 } else { (h + 1) as f64 };
            let margin = 1.959963984540054 * gain.sqrt();
            close(
                result.lower.as_ref().unwrap()[h] / units,
                point - margin,
                1e-12,
            );
            close(
                result.upper.as_ref().unwrap()[h] / units,
                point + margin,
                1e-12,
            );
        }
    }
}

#[test]
fn forecast_moment_units_keep_smoothing_optimization_with_subnormal_variance() {
    let y = (0..640)
        .map(|i| if i % 2 == 0 { -1.0 } else { 1.0 })
        .collect::<Vec<_>>();
    let options = SmoothingOptions {
        trend: false,
        damped: false,
        seasonality: Seasonality::None,
        period: 1,
        alpha: 0.3,
        beta: 0.1,
        gamma: 0.1,
        phi: 0.98,
        optimize: true,
        horizon: 6,
        iteration: iteration(),
    };
    let baseline = exponential_smoothing(&y, options, &control()).unwrap();
    let units = 1e-161;
    let scaled = y.iter().map(|value| value * units).collect::<Vec<_>>();
    let result = exponential_smoothing(&scaled, options, &control()).unwrap();
    assert_eq!(result.parameters, baseline.parameters);
    assert_eq!(result.iterations, baseline.iterations);
    assert_eq!(
        result.effective_observations,
        baseline.effective_observations
    );
    for (actual, expected) in result.fitted.iter().zip(&baseline.fitted) {
        assert_eq!(actual.is_some(), expected.is_some());
        if let (Some(actual), Some(expected)) = (actual, expected) {
            close(actual / units, *expected, 1e-10);
        }
    }
    for (actual, expected) in result.residuals.iter().zip(&baseline.residuals) {
        assert_eq!(actual.is_some(), expected.is_some());
        if let (Some(actual), Some(expected)) = (actual, expected) {
            close(actual / units, *expected, 1e-10);
        }
    }
    for (actual, expected) in result.forecasts.iter().zip(&baseline.forecasts) {
        close(actual / units, *expected, 1e-10);
    }
    let expected = (baseline.innovation_variance * units) * units;
    assert!(result.innovation_variance > 0.0 && result.innovation_variance < f64::MIN_POSITIVE);
    assert!((result.innovation_variance - expected).abs() <= 2.0 * f64::from_bits(1));
    let ll = -0.5
        * result.effective_observations as f64
        * ((2.0 * std::f64::consts::PI).ln() + result.innovation_variance.ln() + 1.0);
    close(result.log_likelihood.unwrap(), ll, 1e-12);
    close(result.aic.unwrap(), -2.0 * ll + 4.0, 1e-12);
    close(
        result.bic.unwrap(),
        -2.0 * ll + 2.0 * (result.effective_observations as f64).ln(),
        1e-12,
    );
}

#[test]
fn time_series_smoothing_states_match_statsmodels_known_initialization() {
    let f = fixture();
    let y = vector(&f["smoothing_series"]);
    for case in f["smoothing"].as_array().unwrap() {
        let seasonality = match case["seasonal"].as_str() {
            Some("add") => Seasonality::Additive,
            Some("mul") => Seasonality::Multiplicative,
            _ => Seasonality::None,
        };
        let o = SmoothingOptions {
            trend: case["trend"].as_bool().unwrap(),
            damped: case["damped"].as_bool().unwrap(),
            seasonality,
            period: 4,
            alpha: 0.3,
            beta: 0.1,
            gamma: 0.15,
            phi: 0.95,
            optimize: false,
            horizon: 6,
            iteration: iteration(),
        };
        let r = exponential_smoothing(&y, o, &control()).unwrap();
        compare(
            &r.fitted.iter().flatten().copied().collect::<Vec<_>>(),
            &case["fitted"],
            1e-10,
        );
        compare(&r.forecasts, &case["forecasts"], 1e-10);
        let optimized = exponential_smoothing(
            &y,
            SmoothingOptions {
                optimize: true,
                ..o
            },
            &control(),
        )
        .unwrap();
        assert!(optimized.innovation_variance <= r.innovation_variance * (1.0 + 1e-7));
    }
}

#[test]
fn time_series_stationarity_matches_arch_and_statsmodels_with_reported_tail_bounds() {
    let f = fixture();
    let y = vector(&f["series"]);
    for case in f["stationarity"].as_array().unwrap() {
        let trend = match case["trend"].as_str().unwrap() {
            "n" => Deterministic::None,
            "c" => Deterministic::Constant,
            _ => Deterministic::Trend,
        };
        let r = if case["method"] == "pp" {
            phillips_perron(&y, 4, trend, &control())
        } else {
            kpss(&y, 4, trend, &control())
        }
        .unwrap();
        close(r.statistic, case["statistic"].as_f64().unwrap(), 1e-8);
        close(r.p_value, case["p_value"].as_f64().unwrap(), 1e-8);
        if case["method"] == "kpss" {
            assert_eq!(r.p_value_kind, "less_than");
        }
    }
    let stationary: Vec<_> = (0..640)
        .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let r = kpss(&stationary, 0, Deterministic::Constant, &control()).unwrap();
    assert_eq!(r.p_value_kind, "greater_than");
    close(r.p_value, 0.1, 1e-12);
}

fn check_stationarity_response_units(scale: f64) {
    let f = fixture();
    let y = vector(&f["series"]);
    let scaled = y.iter().map(|value| value * scale).collect::<Vec<_>>();
    for case in f["stationarity"].as_array().unwrap() {
        let trend = match case["trend"].as_str().unwrap() {
            "n" => Deterministic::None,
            "c" => Deterministic::Constant,
            _ => Deterministic::Trend,
        };
        let fit = |series: &[f64]| {
            if case["method"] == "pp" {
                phillips_perron(series, 4, trend, &control())
            } else {
                kpss(series, 4, trend, &control())
            }
        };
        let baseline = fit(&y).unwrap();
        let result = fit(&scaled).unwrap_or_else(|error| {
            panic!(
                "{} {} in units {scale}: {error:?}",
                case["method"], case["trend"]
            )
        });
        close(result.statistic, case["statistic"].as_f64().unwrap(), 1e-8);
        close(result.p_value, case["p_value"].as_f64().unwrap(), 1e-8);
        assert_eq!(result.method, baseline.method);
        assert_eq!(result.observations, baseline.observations);
        assert_eq!(
            result.effective_observations,
            baseline.effective_observations
        );
        assert_eq!(result.deterministic, baseline.deterministic);
        assert_eq!(result.bandwidth, baseline.bandwidth);
        assert_eq!(result.p_value_kind, baseline.p_value_kind);
        assert_eq!(result.critical_values, baseline.critical_values);
        let expected_variance = (baseline.long_run_variance * scale) * scale;
        assert!(expected_variance.is_finite() && expected_variance > 0.0);
        assert!(result.long_run_variance.is_finite() && result.long_run_variance > 0.0);
        let tolerance = (1e-10 * expected_variance).max(2.0 * f64::from_bits(1));
        assert!(
            (result.long_run_variance - expected_variance).abs() <= tolerance,
            "long-run variance {} != {expected_variance}",
            result.long_run_variance
        );
        if scale < 1.0 {
            assert!(result.long_run_variance < f64::MIN_POSITIVE);
        }
    }
}

#[test]
fn stationarity_response_units_preserve_subnormal_long_run_variance() {
    check_stationarity_response_units(1e-161);
}

#[test]
fn stationarity_response_units_avoid_large_intermediate_products() {
    check_stationarity_response_units(1e152);
}

#[test]
fn time_series_ecm_and_grey_forecast_match_independent_least_squares() {
    let f = fixture();
    let data = &f["ecm"];
    let r = ecm(
        &vector(&data["y"]),
        &[vector(&data["x"])],
        EcmOptions {
            lags: 1,
            constant: true,
        },
        &control(),
    )
    .unwrap();
    assert_eq!(r.first_short_run_row, 2);
    compare(
        &r.long_run
            .coefficients
            .iter()
            .map(|c| c.estimate)
            .collect::<Vec<_>>(),
        &data["long"],
        1e-9,
    );
    compare(
        &r.short_run
            .coefficients
            .iter()
            .map(|c| c.estimate)
            .collect::<Vec<_>>(),
        &data["short"],
        1e-8,
    );
    compare(
        &r.short_run
            .coefficients
            .iter()
            .map(|c| c.standard_error.unwrap())
            .collect::<Vec<_>>(),
        &data["short_se"],
        1e-8,
    );
    let data = &f["grey"];
    let r = grey_prediction(&vector(&data["y"]), 6, &control()).unwrap();
    close(r.parameters[0].estimate, data["a"].as_f64().unwrap(), 1e-10);
    close(r.parameters[1].estimate, data["b"].as_f64().unwrap(), 1e-10);
    compare(&r.forecasts, &data["forecasts"], 1e-9);
    let r = grey_prediction(&[4., 4., 4., 4.], 3, &control()).unwrap();
    for v in r.forecasts {
        close(v, 4., 1e-9);
    }
}

#[test]
fn volatility_response_units_restore_variance_when_empirical_scale_square_underflows() {
    let data = fixture();
    let cases = data["volatility"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["method"] != "egarch")
        .collect::<Vec<_>>();
    // The offset makes conditional variances larger than the centered empirical scale squared.
    check_volatility_response_units(&cases, true, 2.0f64.powi(-536));
}

#[test]
fn volatility_response_units_keep_egarch_log_intercept_unrounded() {
    let data = fixture();
    let case = data["volatility"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["method"] == "egarch")
        .unwrap();
    check_volatility_response_units(&[case], false, 2.0f64.powi(-532));
}

fn check_volatility_response_units(cases: &[&Value], shifted: bool, units: f64) {
    for case in cases {
        let method = match case["method"].as_str().unwrap() {
            "arch" => VolatilityMethod::Arch,
            "garch" => VolatilityMethod::Garch,
            "egarch" => VolatilityMethod::Egarch,
            _ => VolatilityMethod::GjrGarch,
        };
        let options = VolatilityOptions {
            method,
            p: 1,
            q: usize::from(method != VolatilityMethod::Arch),
            constant: false,
            horizon: 6,
            simulations: 1000,
            seed: 42,
            iteration: iteration(),
        };
        let y = vector(&case["y"])
            .into_iter()
            .map(|value| if shifted { 2.0 + 0.1 * value } else { value })
            .collect::<Vec<_>>();
        let baseline = volatility(&y, options, &control())
            .unwrap_or_else(|error| panic!("baseline {method:?}: {error:?}"));
        // Power-of-two units preserve the normalized fit exactly.
        let scaled = y.iter().map(|value| value * units).collect::<Vec<_>>();
        let result = volatility(&scaled, options, &control())
            .unwrap_or_else(|error| panic!("scaled {:?}: {error:?}", options.method));
        let beta_sum = baseline
            .parameters
            .iter()
            .filter(|parameter| parameter.term.starts_with("beta"))
            .map(|parameter| parameter.estimate)
            .sum::<f64>();
        for (actual, expected) in result.parameters.iter().zip(&baseline.parameters) {
            assert_eq!(actual.term, expected.term);
            if actual.term == "omega" && options.method == VolatilityMethod::Egarch {
                close(
                    actual.estimate,
                    expected.estimate + (1.0 - beta_sum) * 2.0 * units.ln(),
                    1e-12,
                );
            } else if actual.term == "omega" {
                let expected = (expected.estimate * units) * units;
                assert!(actual.estimate.is_finite());
                assert!((actual.estimate - expected).abs() <= 2.0 * f64::from_bits(1));
            } else {
                close(actual.estimate, expected.estimate, 1e-12);
            }
        }
        for (actual, expected) in result
            .conditional_variances
            .iter()
            .zip(&baseline.conditional_variances)
            .chain(
                result
                    .forecast_variances
                    .iter()
                    .zip(&baseline.forecast_variances),
            )
        {
            let expected = (expected * units) * units;
            assert!(expected > 0.0);
            assert!(actual.is_finite() && *actual > 0.0);
            assert!((actual - expected).abs() <= 2.0 * f64::from_bits(1));
        }
        for (actual, expected) in result.residuals.iter().zip(&baseline.residuals) {
            close(actual / units, *expected, 1e-12);
        }
        for (actual, expected) in result
            .standardized_residuals
            .iter()
            .zip(&baseline.standardized_residuals)
        {
            close(*actual, *expected, 1e-12);
        }
        close(
            result.log_likelihood,
            baseline.log_likelihood - y.len() as f64 * units.ln(),
            1e-12,
        );
        let shift = 2.0 * y.len() as f64 * units.ln();
        close(result.aic, baseline.aic + shift, 1e-12);
        close(result.bic, baseline.bic + shift, 1e-12);
        assert_eq!(result.iterations, baseline.iterations);
        assert_eq!(result.simulations, baseline.simulations);
        assert_eq!(result.seed, baseline.seed);
    }
}

#[test]
fn time_series_volatility_likelihood_and_recursions_match_arch() {
    let f = fixture();
    for case in f["volatility"].as_array().unwrap() {
        let method = match case["method"].as_str().unwrap() {
            "arch" => VolatilityMethod::Arch,
            "garch" => VolatilityMethod::Garch,
            "egarch" => VolatilityMethod::Egarch,
            _ => VolatilityMethod::GjrGarch,
        };
        let o = VolatilityOptions {
            method,
            p: 1,
            q: usize::from(method != VolatilityMethod::Arch),
            constant: false,
            horizon: 6,
            simulations: 1000,
            seed: 42,
            iteration: iteration(),
        };
        let y = vector(&case["y"]);
        let r = volatility(&y, o, &control()).unwrap_or_else(|e| panic!("{method:?}: {e:?}"));
        compare(
            &r.parameters.iter().map(|p| p.estimate).collect::<Vec<_>>(),
            &case["parameters"],
            3e-4,
        );
        close(
            r.log_likelihood,
            case["log_likelihood"].as_f64().unwrap(),
            1e-8,
        );
        compare(&r.conditional_variances, &case["variances"], 5e-4);
        if method == VolatilityMethod::Egarch {
            close(
                r.forecast_variances[0],
                case["forecasts"][0].as_f64().unwrap(),
                2e-4,
            );
            assert!(
                r.forecast_variances
                    .iter()
                    .all(|v| v.is_finite() && *v > 0.0)
            );
            assert_eq!(
                r.forecast_variances,
                volatility(&y, o, &control()).unwrap().forecast_variances
            );
        } else {
            compare(&r.forecast_variances, &case["forecasts"], 5e-4);
        }
    }
}

#[test]
fn time_series_markov_preserves_transition_counts_and_handles_unobserved_outgoing_states() {
    let r = markov_prediction(&[0, 1, 0, 1, 1, 0], 2, 3, 0.0, &control()).unwrap();
    assert_eq!(r.counts, vec![vec![0, 2], vec![2, 1]]);
    close(r.forecast_probabilities[0][1], 1.0, 1e-12);
    close(r.forecast_probabilities[1][0], 2.0 / 3.0, 1e-12);
    assert_eq!(r.forecast_states, vec![1, 0, 1]);
    assert!(markov_prediction(&[0, 0, 1], 2, 2, 0.0, &control()).is_err());
    let smoothed = markov_prediction(&[0, 0, 1], 2, 2, 1.0, &control()).unwrap();
    assert_eq!(smoothed.transition_probabilities[1], vec![0.5, 0.5]);
    assert_eq!(smoothed.forecast_states[0], 0);
}

#[test]
fn time_series_invalid_domains_and_execution_control_do_not_return_partial_success() {
    let f = fixture();
    let y = vector(&f["series"]);
    let minimal = arima(
        &[2.0, 3.0],
        ArimaOptions {
            p: 0,
            d: 0,
            q: 0,
            ..arima_options()
        },
        &control(),
    )
    .unwrap();
    close(minimal.innovation_variance, 0.25, 1e-12);
    assert!(
        arima(
            &y,
            ArimaOptions {
                d: usize::MAX,
                ..arima_options()
            },
            &control()
        )
        .is_err()
    );
    assert!(
        arima(
            &y,
            ArimaOptions {
                iteration: IterationOptions {
                    max_iterations: 1,
                    tolerance: 1e-12
                },
                ..arima_options()
            },
            &control()
        )
        .is_err()
    );
    assert!(kpss(&y, y.len(), Deterministic::Constant, &control()).is_err());
    assert!(kpss(&y, 0, Deterministic::None, &control()).is_err());
    assert!(phillips_perron(&[2.; 20], 0, Deterministic::Constant, &control()).is_err());
    assert!(grey_prediction(&[-1., 2., 3., 4.], 2, &control()).is_err());
    assert!(
        ecm(
            &y,
            &[vec![1.; 3]],
            EcmOptions {
                lags: 0,
                constant: true
            },
            &control()
        )
        .is_err()
    );
    let cancelled = control();
    cancelled.cancellation.cancel();
    assert!(matches!(
        arima(&y, arima_options(), &cancelled),
        Err(ScientificComputationError::Cancelled)
    ));
    assert!(matches!(
        markov_prediction(&[0, 1, 0], 2, 1, 0., &cancelled),
        Err(ScientificComputationError::Cancelled)
    ));
    let expired = ScientificExecutionControl {
        deadline: Instant::now() - Duration::from_secs(1),
        ..control()
    };
    assert!(matches!(
        kpss(&y, 0, Deterministic::Constant, &expired),
        Err(ScientificComputationError::DeadlineExceeded)
    ));
}
