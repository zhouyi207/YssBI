use super::*;
use serde_json::Value;
use std::time::{Duration, Instant};
use yss_sci_contract::execution::{
    ScientificCancellationToken, ScientificComputationError as Error, ScientificExecutionControl,
};
use yss_sci_contract::regression::models::*;

fn control() -> ScientificExecutionControl {
    ScientificExecutionControl {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(30),
    }
}

#[test]
fn coefficient_intervals_keep_large_degree_accuracy() {
    let covariance = yss_sci_linalg::Mat::identity(1, 1);
    let coefficients = common::coefficient_table(
        &[0.0],
        vec!["intercept".into()],
        Some(&covariance),
        Some(1_000_000),
        0.95,
    )
    .unwrap();
    let coefficient = &coefficients[0];
    assert_eq!(coefficient.standard_error, Some(1.0));
    // Independent high-precision Student density/Beta reference for df=1e6.
    let expected = 1.9599663568141066;
    let [lower, upper] = coefficient.confidence_interval.unwrap();
    assert!((lower + expected).abs() < 1e-12, "{lower}");
    assert!((upper - expected).abs() < 1e-12, "{upper}");
}

#[test]
fn shared_design_failures_distinguish_data_calculation_and_tuning() {
    use yss_sci_contract::execution::ScientificInputViolation as Violation;
    let options = RobustOptions {
        constant: true,
        loss: RobustLoss::Huber,
        tuning: 1.345,
        iteration: IterationOptions::default(),
    };
    for (response, predictors, options, expected) in [
        (
            vec![1., 3.],
            vec![vec![0., 1.]],
            options,
            Error::InvalidInput {
                violation: Violation::EmptyInput,
            },
        ),
        (
            vec![1., 3., 2.],
            vec![vec![1., 1., 1.]],
            options,
            Error::InvalidInput {
                violation: Violation::DataOutOfRange,
            },
        ),
        (
            vec![1., 3., 2.],
            vec![],
            RobustOptions {
                constant: false,
                ..options
            },
            Error::InvalidInput {
                violation: Violation::ShapeMismatch,
            },
        ),
        (
            vec![1., 3., 2.],
            vec![vec![-f64::MAX, f64::MAX, f64::MAX]],
            options,
            Error::ComputationFailed,
        ),
        (
            vec![1., 3., 2.],
            vec![vec![0., 1., 2.]],
            RobustOptions {
                tuning: 0.0,
                ..options
            },
            Error::InvalidInput {
                violation: Violation::ParameterOutOfRange,
            },
        ),
    ] {
        assert_eq!(
            robust(&response, &predictors, options, &control()).unwrap_err(),
            expected
        );
    }
}

fn reference() -> Value {
    serde_json::from_str(include_str!("fixtures/reference.json")).unwrap()
}
fn vector(v: &Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect()
}
fn inputs(v: &Value) -> (Vec<f64>, Vec<Vec<f64>>) {
    (
        vector(&v["y"]),
        v["x"].as_array().unwrap().iter().map(vector).collect(),
    )
}
fn close(a: f64, b: f64, tol: f64) {
    assert!(
        (a - b).abs() <= tol * (1.0 + b.abs()),
        "actual {a}, reference {b}, tolerance {tol}"
    );
}
fn coefficients(r: &RegressionModelResult, v: &Value, tol: f64, se: bool) {
    let expected = vector(&v["coefficients"]);
    assert_eq!(r.coefficients.len(), expected.len());
    for (c, &e) in r.coefficients.iter().zip(&expected) {
        close(c.estimate, e, tol);
    }
    if se {
        for (c, &e) in r
            .coefficients
            .iter()
            .zip(vector(&v["standard_errors"]).iter())
        {
            close(c.standard_error.unwrap(), e, 0.002);
        }
    }
    assert!(r.converged);
}
fn likelihood_options(method: LikelihoodMethod) -> LikelihoodOptions {
    LikelihoodOptions {
        method,
        constant: true,
        lower: 0.0,
        upper: None,
        iteration: IterationOptions::default(),
    }
}

#[test]
fn penalties_and_pls_preserve_objective_normalization_and_original_units() {
    let data = reference();
    let (y, x) = inputs(&data["linear"]);
    let c = control();
    for (name, penalty) in [("ridge", Penalty::Ridge), ("lasso", Penalty::Lasso)] {
        let r = penalized(
            &y,
            &x,
            PenalizedOptions {
                constant: true,
                standardize: true,
                penalty,
                lambda: 0.17,
                iteration: IterationOptions {
                    max_iterations: 5000,
                    ..Default::default()
                },
            },
            &c,
        )
        .unwrap();
        coefficients(&r, &data["cases"][name], 1e-6, false);
        assert!(r.coefficients.iter().all(|c| c.p_value.is_none()));
    }
    coefficients(
        &pls(&y, &x, 1, true, &c).unwrap(),
        &data["cases"]["pls"],
        1e-10,
        false,
    );
    let duplicate = vec![x[0].clone(), x[0].clone()];
    let r = penalized(
        &y,
        &duplicate,
        PenalizedOptions {
            constant: true,
            standardize: false,
            penalty: Penalty::Ridge,
            lambda: 1.0,
            iteration: IterationOptions::default(),
        },
        &c,
    )
    .unwrap();
    close(
        r.coefficients[1].estimate,
        r.coefficients[2].estimate,
        1e-10,
    );
}
#[test]
fn robust_estimators_and_quantile_match_independent_fits() {
    let data = reference();
    let c = control();
    for (name, loss, tuning) in [
        ("huber", RobustLoss::Huber, 1.345),
        ("tukey", RobustLoss::Tukey, 4.685),
    ] {
        let (y, x) = inputs(&data["cases"][name]);
        let r = robust(
            &y,
            &x,
            RobustOptions {
                constant: true,
                loss,
                tuning,
                iteration: IterationOptions::default(),
            },
            &c,
        )
        .unwrap();
        coefficients(&r, &data["cases"][name], 1e-5, true);
    }
    let v = &data["cases"]["quantile"];
    let (y, x) = inputs(v);
    let r = quantile(
        &y,
        &x,
        true,
        0.3,
        IterationOptions {
            max_iterations: 5000,
            ..Default::default()
        },
        &c,
    )
    .unwrap();
    coefficients(&r, v, 1e-4, false);
    assert!(
        r.coefficients
            .iter()
            .all(|c| c.standard_error.is_some_and(|v| v > 0.0))
    );
}
#[test]
fn glm_families_links_and_fractional_sandwich_match_statsmodels() {
    let data = reference();
    let c = control();
    for (name, family, link, fractional) in [
        ("gaussian", GlmFamily::Gaussian, GlmLink::Identity, false),
        ("gaussian_log", GlmFamily::Gaussian, GlmLink::Log, false),
        ("poisson", GlmFamily::Poisson, GlmLink::Log, false),
        ("gamma", GlmFamily::Gamma, GlmLink::Log, false),
        (
            "inverse_gaussian",
            GlmFamily::InverseGaussian,
            GlmLink::Log,
            false,
        ),
        ("logit", GlmFamily::Binomial, GlmLink::Logit, false),
        ("probit", GlmFamily::Binomial, GlmLink::Probit, false),
        ("cloglog", GlmFamily::Binomial, GlmLink::Cloglog, false),
        ("fractional", GlmFamily::Binomial, GlmLink::Logit, true),
    ] {
        let v = &data["cases"][name];
        let (y, x) = inputs(v);
        let r = glm(
            &y,
            &x,
            GlmOptions {
                constant: true,
                family,
                link,
                fractional,
                iteration: IterationOptions::default(),
            },
            &c,
        )
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        coefficients(&r, v, 1e-5, true);
        for (a, b) in r.fitted.iter().zip(vector(&v["fitted"])) {
            close(*a, b, 1e-5);
        }
        if fractional {
            assert!(r.statistics.aic.is_none());
        }
    }
}
#[test]
fn raw_restoration_keeps_gaussian_inference_with_subnormal_predictors() {
    let data = reference();
    let reference = &data["cases"]["gaussian"];
    let (y, x) = inputs(reference);
    let options = GlmOptions {
        constant: true,
        family: GlmFamily::Gaussian,
        link: GlmLink::Identity,
        fractional: false,
        iteration: IterationOptions::default(),
    };
    let baseline = glm(&y, &x, options, &control()).unwrap();
    let response_scale = 1e-156;
    let predictor_scale = 1e-310;
    let scaled_y = y
        .iter()
        .map(|value| value * response_scale)
        .collect::<Vec<_>>();
    let scaled_x = x
        .iter()
        .map(|column| column.iter().map(|value| value * predictor_scale).collect())
        .collect::<Vec<Vec<_>>>();
    let result = glm(&scaled_y, &scaled_x, options, &control())
        .expect("raw coefficients and covariance are finite in these joint units");
    let units = [
        response_scale,
        response_scale / predictor_scale,
        response_scale / predictor_scale,
    ];
    for (i, (actual, expected)) in result
        .coefficients
        .iter()
        .zip(&baseline.coefficients)
        .enumerate()
    {
        assert_eq!(actual.term, expected.term);
        close(
            actual.estimate / units[i],
            reference["coefficients"][i].as_f64().unwrap(),
            1e-7,
        );
        close(
            actual.standard_error.unwrap() / units[i],
            reference["standard_errors"][i].as_f64().unwrap(),
            1e-7,
        );
        close(actual.statistic.unwrap(), expected.statistic.unwrap(), 1e-6);
        close(actual.p_value.unwrap(), expected.p_value.unwrap(), 1e-7);
        for (actual, expected) in actual
            .confidence_interval
            .unwrap()
            .into_iter()
            .zip(expected.confidence_interval.unwrap())
        {
            assert!(actual.is_finite());
            close(actual / units[i], expected, 1e-7);
        }
    }
    let covariance = result.covariance.as_ref().unwrap();
    for (i, row) in covariance.iter().enumerate() {
        for (j, value) in row.iter().enumerate() {
            assert!(value.is_finite());
            let restored = value / units[i].max(units[j]) / units[i].min(units[j]);
            let expected = baseline.covariance.as_ref().unwrap()[i][j];
            assert!(
                (restored - expected).abs() <= 2e-6 * expected.abs(),
                "covariance[{i}, {j}]: {restored} != {expected}"
            );
        }
    }
    for (actual, expected) in result.fitted.iter().zip(vector(&reference["fitted"])) {
        close(actual / response_scale, expected, 1e-7);
    }
    for (actual, expected) in result.residuals.iter().zip(&baseline.residuals) {
        close(actual / response_scale, *expected, 1e-7);
    }
    assert!(result.converged);
    assert_eq!(result.observations, baseline.observations);
    assert_eq!(
        result.statistics.df_residual,
        baseline.statistics.df_residual
    );
}

#[test]
fn likelihood_count_censoring_and_proportion_models_match_references() {
    let data = reference();
    let c = control();
    for (name, method, upper) in [
        (
            "negative_binomial",
            LikelihoodMethod::NegativeBinomial,
            None,
        ),
        ("zip", LikelihoodMethod::ZeroInflatedPoisson, None),
        ("zinb", LikelihoodMethod::ZeroInflatedNegativeBinomial, None),
        ("beta", LikelihoodMethod::Beta, None),
        ("tobit", LikelihoodMethod::Tobit, None),
        ("tobit_both", LikelihoodMethod::Tobit, Some(1.4)),
    ] {
        let v = &data["cases"][name];
        let (y, x) = inputs(v);
        let options = LikelihoodOptions {
            upper,
            ..likelihood_options(method)
        };
        let r =
            likelihood(&y, &x, &[], None, options, &c).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        coefficients(&r, v, 0.0003, name != "tobit" && name != "tobit_both");
        if method != LikelihoodMethod::ZeroInflatedPoisson {
            let nuisance = r.coefficients.last().unwrap();
            assert!(nuisance.p_value.is_none());
            assert!(
                nuisance
                    .confidence_interval
                    .is_some_and(|interval| interval[0] > 0.0 && interval[1] > interval[0])
            );
        }
        close(
            r.statistics.log_likelihood.unwrap(),
            v["log_likelihood"].as_f64().unwrap(),
            1e-7,
        );
    }
}
#[test]
fn categorical_and_conditional_likelihoods_preserve_parameter_order_and_probabilities() {
    let data = reference();
    let c = control();
    for (name, method) in [
        ("multinomial", LikelihoodMethod::MultinomialLogit),
        ("ordinal", LikelihoodMethod::OrdinalLogit),
        ("conditional", LikelihoodMethod::ConditionalLogit),
    ] {
        let v = &data["cases"][name];
        let (y, x) = inputs(v);
        let groups = v.get("groups").map(|v| {
            vector(v)
                .into_iter()
                .map(|v| v as usize)
                .collect::<Vec<_>>()
        });
        let options = LikelihoodOptions {
            constant: method == LikelihoodMethod::MultinomialLogit,
            ..likelihood_options(method)
        };
        let r = likelihood(&y, &x, &[], groups.as_deref(), options, &c)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        coefficients(&r, v, 0.0003, true);
        close(
            r.statistics.log_likelihood.unwrap(),
            v["log_likelihood"].as_f64().unwrap(),
            1e-7,
        );
        assert!(r.fitted.is_empty());
        assert!(r.statistics.rss.is_none());
        for row in &r.probabilities {
            close(row.iter().sum(), 1.0, 1e-12);
            assert!(row.iter().all(|v| *v >= 0.0 && *v <= 1.0));
        }
    }
    let y = [0., 0., 0., 1., 1., 1.];
    let x = vec![vec![0., 0., 0., 1., 1., 1.]];
    let r = firth_logit(&y, &x, true, IterationOptions::default(), &c).unwrap();
    close(r.coefficients[0].estimate, -7.0_f64.ln(), 1e-6);
    close(r.coefficients[1].estimate, 2.0 * 7.0_f64.ln(), 1e-6);
    assert!(r.fitted.iter().all(|v| *v > 0.0 && *v < 1.0));
}
#[test]
fn nonlinear_formulas_curves_deming_and_splines_fit_actual_models() {
    let data = reference();
    let c = control();
    let v = &data["cases"]["nonlinear"];
    let (y, x) = inputs(v);
    let r = nonlinear_formula(
        &y,
        &x,
        "b1*exp(b2*x1)",
        &[2., 0.2],
        &[],
        &[],
        IterationOptions::default(),
        &c,
    )
    .unwrap();
    coefficients(&r, v, 1e-5, false);
    coefficients(
        &nonlinear(
            &y,
            &x[0],
            NonlinearFamily::Exponential,
            &[],
            IterationOptions::default(),
            &c,
        )
        .unwrap(),
        v,
        1e-5,
        false,
    );
    let bounded = nonlinear_formula(
        &y,
        &x,
        "b1*exp(b2*x1)",
        &[2., 0.1],
        &[0., 0.],
        &[10., 0.2],
        IterationOptions::default(),
        &c,
    )
    .unwrap();
    close(bounded.coefficients[1].estimate, 0.2, 1e-7);
    assert!(bounded.covariance.is_none());
    for family in [
        NonlinearFamily::Logistic,
        NonlinearFamily::MichaelisMenten,
        NonlinearFamily::Gompertz,
    ] {
        let expected = if family == NonlinearFamily::MichaelisMenten {
            vec![4.0, 1.2]
        } else {
            vec![4.0, 1.2, 1.8]
        };
        let response = x[0]
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let mean = match family {
                    NonlinearFamily::Logistic => 4.0 / (1.0 + (-1.2 * (v - 1.8)).exp()),
                    NonlinearFamily::MichaelisMenten => 4.0 * v / (1.2 + v),
                    NonlinearFamily::Gompertz => 4.0 * (-(-1.2 * (v - 1.8)).exp()).exp(),
                    _ => unreachable!(),
                };
                mean + 0.003 * (i as f64).sin()
            })
            .collect::<Vec<_>>();
        let fitted = nonlinear(
            &response,
            &x[0],
            family,
            &[],
            IterationOptions::default(),
            &c,
        )
        .unwrap();
        for (coefficient, reference) in fitted.coefficients.iter().zip(expected) {
            close(coefficient.estimate, reference, 0.005);
        }
    }
    for family in [
        CurveFamily::Polynomial,
        CurveFamily::Logarithmic,
        CurveFamily::Inverse,
        CurveFamily::Exponential,
        CurveFamily::Power,
    ] {
        assert!(
            curve(&y, &x[0], family, 2, &c)
                .unwrap()
                .statistics
                .rss
                .is_some()
        );
    }
    let xs = [1., 2., 3., 4., 5., 6.];
    let ys = [2.2, 3.9, 6.2, 7.8, 10.1, 12.2];
    let d = deming(&ys, &xs, 1.0, &c).unwrap();
    close(d.coefficients[1].estimate, 2.0, 0.02);
    assert!(d.covariance.is_some());
    let knots = automatic_spline_knots(&x[0], 4, &c).unwrap();
    let spline = restricted_cubic_spline(&y, &x[0], &knots, &c).unwrap();
    assert_eq!(spline.coefficients.len(), 4);
    if let RegressionDetails::RestrictedCubicSpline { basis, .. } = spline.details {
        assert_eq!(basis.len(), 3);
        assert_eq!(basis[0], x[0]);
    } else {
        panic!("spline details");
    }
}
#[test]
fn workflows_retain_nested_changes_selection_history_and_original_rows() {
    let data = reference();
    let (y, x) = inputs(&data["linear"]);
    let c = control();
    let h = hierarchical(&y, &x, &[1, 1], true, &c).unwrap();
    assert_eq!(h.stages.len(), 2);
    assert!(h.stages[1].delta_r_squared.unwrap() > 0.1);
    assert!(h.stages[1].change_p_value.unwrap() < 0.001);
    let multi = univariate_multivariable(&y, &x, true, &c).unwrap();
    assert_eq!(multi.stages.len(), 3);
    assert_eq!(multi.stages[1].model.coefficients[1].term, "x2");
    for direction in [
        SelectionDirection::Forward,
        SelectionDirection::Backward,
        SelectionDirection::Both,
    ] {
        let s = stepwise(&y, &x, true, direction, SelectionCriterion::Bic, &c).unwrap();
        assert_eq!(s.selected_predictors, vec![1, 2]);
        assert!(
            s.selection_history
                .windows(2)
                .all(|w| w[1].criterion_value < w[0].criterion_value)
        );
    }
    let groups = (0..y.len()).map(|i| i % 4).collect::<Vec<_>>();
    let g = grouped(&y, &x, &groups, true, &c).unwrap();
    assert_eq!(g.stages.len(), 4);
    assert_eq!(g.stages[0].observation_indices[1], 5);
    let nonlinear_y = x[0]
        .iter()
        .zip(&y)
        .map(|(t, y)| if *t <= 2.0 { *y } else { *y + 8.0 + 2.0 * t })
        .collect::<Vec<_>>();
    let t = threshold(&nonlinear_y, &x, &x[0], true, 0.15, 200, &c).unwrap();
    if let RegressionDetails::Threshold {
        threshold,
        regime_counts,
        ..
    } = t.details
    {
        assert!(threshold <= 2.0 && threshold > 1.8);
        assert_eq!(regime_counts.iter().sum::<usize>(), y.len());
    } else {
        panic!("threshold");
    }
}
#[test]
fn invalid_domains_identifiability_nonconvergence_and_control_remain_failures() {
    let c = control();
    let x = vec![vec![0., 1., 2., 3., 4., 5.]];
    let y = [0., 1., 0., 1., 0., 1.];
    assert!(
        likelihood(
            &[1e300; 6],
            &x,
            &[],
            None,
            likelihood_options(LikelihoodMethod::MultinomialLogit),
            &c
        )
        .is_err()
    );
    assert!(grouped(&y, &x, &[usize::MAX; 6], true, &c).is_err());
    assert!(
        likelihood(
            &y,
            &x,
            &[],
            Some(&[usize::MAX; 6]),
            likelihood_options(LikelihoodMethod::ConditionalLogit),
            &c
        )
        .is_err()
    );
    assert!(
        nonlinear_formula(
            &y,
            &x,
            "λ + b1*x1",
            &[1.0],
            &[],
            &[],
            IterationOptions::default(),
            &c
        )
        .is_err()
    );
    assert!(
        likelihood(
            &y,
            &x,
            &[],
            None,
            likelihood_options(LikelihoodMethod::Beta),
            &c
        )
        .is_err()
    );
    assert!(
        glm(
            &y,
            &x,
            GlmOptions {
                constant: true,
                family: GlmFamily::Gamma,
                link: GlmLink::Log,
                fractional: false,
                iteration: IterationOptions::default()
            },
            &c
        )
        .is_err()
    );
    assert!(
        nonlinear_formula(
            &y,
            &x,
            "b1+b2*x2",
            &[0., 1.],
            &[],
            &[],
            IterationOptions::default(),
            &c
        )
        .is_err()
    );
    assert!(baseline(&y, &[x[0].clone(), x[0].clone()], true, &c).is_err());
    assert!(
        firth_logit(
            &y,
            &x,
            true,
            IterationOptions {
                max_iterations: 1,
                ..Default::default()
            },
            &c
        )
        .is_err()
    );
    let cancelled = control();
    cancelled.cancellation.cancel();
    assert_eq!(
        penalized(
            &y,
            &x,
            PenalizedOptions {
                constant: true,
                standardize: true,
                penalty: Penalty::Ridge,
                lambda: 1.0,
                iteration: IterationOptions::default()
            },
            &cancelled
        ),
        Err(Error::Cancelled)
    );
    let expired = ScientificExecutionControl {
        deadline: Instant::now() - Duration::from_secs(1),
        ..control()
    };
    assert_eq!(
        deming(&y, &x[0], 1.0, &expired),
        Err(Error::DeadlineExceeded)
    );
}

#[test]
fn scale_limits_multinomial_keeps_all_observed_response_categories() {
    let y = (0..200).map(|i| (i % 10) as f64).collect::<Vec<_>>();
    let fit = likelihood(
        &y,
        &[],
        &[],
        None,
        LikelihoodOptions {
            method: LikelihoodMethod::MultinomialLogit,
            constant: true,
            lower: 0.0,
            upper: None,
            iteration: IterationOptions {
                max_iterations: 20000,
                ..Default::default()
            },
        },
        &control(),
    )
    .unwrap();
    assert_eq!(fit.categories.len(), 10);
    assert_eq!(fit.coefficients.len(), 9);
    for row in fit.probabilities {
        for probability in row {
            close(probability, 0.1, 1e-8);
        }
    }
}
