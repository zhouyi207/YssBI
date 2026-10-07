use super::*;
use std::time::{Duration, Instant};
use yss_sci_contract::execution::{
    ScientificCancellationToken, ScientificComputationError as Error,
};
use yss_sci_contract::regression::models::IterationOptions;

fn control() -> Control {
    Control {
        cancellation: ScientificCancellationToken::new(),
        deadline: Instant::now() + Duration::from_secs(120),
    }
}
fn close(a: f64, b: f64, tol: f64) {
    assert!(
        (a - b).abs() <= tol * (1.0 + b.abs()),
        "actual {a}, expected {b}, tolerance {tol}"
    );
}

#[test]
fn longitudinal_estimators_match_independent_coefficients_inference_variance_and_predictions() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/reference.json")).unwrap();
    let x: Vec<Vec<f64>> = serde_json::from_value(fixture["x"].clone()).unwrap();
    let group = Grouping {
        codes: serde_json::from_value(fixture["groups"].clone()).unwrap(),
        levels: 18,
    };
    for (name, case) in fixture["cases"].as_object().unwrap() {
        let y: Vec<f64> = serde_json::from_value(case["y"].clone()).unwrap();
        let fit = match name.as_str() {
            "nested" | "crossed" => {
                let codes: Vec<usize> =
                    serde_json::from_value(case["second_groups"].clone()).unwrap();
                let levels = codes.iter().max().unwrap() + 1;
                linear_mixed(
                    &y,
                    &x,
                    &[group.clone(), Grouping { codes, levels }],
                    &[],
                    MixedOptions {
                        nested: name == "nested",
                        ..Default::default()
                    },
                    &control(),
                )
            }
            "lmm_ml" | "lmm_reml" | "random_slope" => linear_mixed(
                &y,
                &x,
                std::slice::from_ref(&group),
                if name == "random_slope" { &x } else { &[] },
                MixedOptions {
                    estimation: if name == "lmm_ml" {
                        MixedEstimation::Ml
                    } else {
                        MixedEstimation::Reml
                    },
                    ..Default::default()
                },
                &control(),
            ),
            "gee_gaussian" | "gee_binomial" | "gee_poisson" => gee(
                &y,
                &x,
                &group,
                GeeOptions {
                    family: match name.as_str() {
                        "gee_gaussian" => ResponseFamily::Gaussian,
                        "gee_binomial" => ResponseFamily::Binomial,
                        _ => ResponseFamily::Poisson,
                    },
                    correlation: if name == "gee_binomial" {
                        WorkingCorrelation::Independence
                    } else {
                        WorkingCorrelation::Exchangeable
                    },
                    ..Default::default()
                },
                &control(),
            ),
            _ => generalized_mixed(
                &y,
                &x,
                &group,
                match name.as_str() {
                    "glmm_binomial" => ResponseFamily::Binomial,
                    "glmm_poisson" => ResponseFamily::Poisson,
                    _ => ResponseFamily::NegativeBinomial,
                },
                true,
                IterationOptions::default(),
                &control(),
            ),
        }
        .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        for (i, c) in fit.coefficients.iter().enumerate() {
            close(c.estimate, case["coefficients"][i].as_f64().unwrap(), 2e-4);
            close(
                c.standard_error.unwrap(),
                case["standard_errors"][i].as_f64().unwrap(),
                3e-4,
            );
            assert!(c.p_value.unwrap().is_finite());
        }
        for (i, m) in fit.fitted_values.iter().enumerate() {
            close(*m, case["fitted"][i].as_f64().unwrap(), 3e-4);
            close(fit.residuals[i], y[i] - m, 1e-12);
        }
        if let Some(ll) = case["log_likelihood"].as_f64() {
            close(fit.log_likelihood.unwrap(), ll, 1e-6);
        }
        if let Some(v) = case["variance"].as_f64() {
            close(fit.variance_components[0].variance, v, 5e-4);
        }
        if let Some(v) = case["scale"].as_f64() {
            close(fit.scale, v, 2e-4);
        }
        if let Some(v) = case["correlation"].as_f64() {
            close(fit.correlation.unwrap(), v, 2e-5);
        }
        if let Some(v) = case["alpha"].as_f64() {
            close(fit.negative_binomial_alpha.unwrap(), v, 5e-4);
        }
        if let Some(variances) = case["variances"].as_array() {
            for (c, v) in fit.variance_components.iter().zip(variances) {
                close(c.variance, v.as_f64().unwrap(), 5e-4);
            }
        }
    }
}

#[test]
fn longitudinal_preserves_unsorted_rows_and_zero_variance_boundaries() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/reference.json")).unwrap();
    let x: Vec<Vec<f64>> = serde_json::from_value(fixture["x"].clone()).unwrap();
    let y: Vec<f64> = serde_json::from_value(fixture["cases"]["lmm_reml"]["y"].clone()).unwrap();
    let codes: Vec<usize> = serde_json::from_value(fixture["groups"].clone()).unwrap();
    let permutation = (0..y.len()).map(|i| i * 37 % y.len()).collect::<Vec<_>>();
    let permuted_y = permutation.iter().map(|&i| y[i]).collect::<Vec<_>>();
    let permuted_x = vec![permutation.iter().map(|&i| x[0][i]).collect()];
    let group = Grouping {
        codes: permutation.iter().map(|&i| codes[i]).collect(),
        levels: 18,
    };
    let fit = linear_mixed(
        &permuted_y,
        &permuted_x,
        &[group],
        &[],
        MixedOptions::default(),
        &control(),
    )
    .unwrap();
    for (i, &row) in permutation.iter().enumerate() {
        close(
            fit.fitted_values[i],
            fixture["cases"]["lmm_reml"]["fitted"][row]
                .as_f64()
                .unwrap(),
            3e-4,
        );
    }
    let x = vec![(0..48).map(|i| (i % 4) as f64 - 1.0).collect::<Vec<_>>()];
    let y = (0..48)
        .map(|i| 2.0 + 1.5 * x[0][i] + [0.2, -0.3, 0.0, 0.1][i % 4])
        .collect::<Vec<_>>();
    let group = Grouping {
        codes: (0..48).map(|i| i / 4).collect(),
        levels: 12,
    };
    let fit = linear_mixed(
        &y,
        &x,
        std::slice::from_ref(&group),
        &[],
        MixedOptions::default(),
        &control(),
    )
    .unwrap();
    assert!(fit.variance_components[0].boundary);
    let binary = (0..48).map(|i| (i % 2) as f64).collect::<Vec<_>>();
    let fit = generalized_mixed(
        &binary,
        &x,
        &group,
        ResponseFamily::Binomial,
        true,
        IterationOptions::default(),
        &control(),
    )
    .unwrap();
    assert!(fit.variance_components[0].boundary);
    assert!(
        fit.coefficients
            .iter()
            .all(|c| c.standard_error.unwrap().is_finite())
    );
    let constant_counts = gee(
        &[2.0; 48],
        &[],
        &group,
        GeeOptions {
            family: ResponseFamily::Poisson,
            correlation: WorkingCorrelation::Independence,
            ..Default::default()
        },
        &control(),
    )
    .unwrap();
    close(
        constant_counts.coefficients[0].estimate,
        2.0_f64.ln(),
        1e-10,
    );
    assert_eq!(constant_counts.coefficients[0].p_value, None);
}

#[test]
fn longitudinal_rejects_invalid_designs_responses_and_nonconvergence_and_honors_control() {
    let y = vec![0.1, 0.6, 1.5, 1.1, 2.3, 3.2, 3.0, 4.3];
    let x = vec![vec![0., 1., 0., 1., 0., 1., 0., 1.]];
    let g = Grouping {
        codes: vec![0, 0, 1, 1, 2, 2, 3, 3],
        levels: 4,
    };
    let c = control();
    let invalid =
        |r: Result<LongitudinalResult>| assert!(matches!(r, Err(Error::InvalidInput { .. })));
    invalid(gee(
        &y,
        &x,
        &g,
        GeeOptions {
            family: ResponseFamily::Binomial,
            ..Default::default()
        },
        &c,
    ));
    invalid(linear_mixed(
        &y,
        &[vec![1.; 8]],
        std::slice::from_ref(&g),
        &[],
        MixedOptions::default(),
        &c,
    ));
    invalid(linear_mixed(
        &y,
        &x,
        &[g.clone(), g.clone()],
        &[],
        MixedOptions::default(),
        &c,
    ));
    let crossed = Grouping {
        codes: vec![0, 1, 0, 1, 0, 1, 0, 1],
        levels: 2,
    };
    invalid(linear_mixed(
        &y,
        &x,
        &[g.clone(), crossed],
        &[],
        MixedOptions {
            nested: true,
            ..Default::default()
        },
        &c,
    ));
    invalid(gee(
        &y,
        &x,
        &Grouping {
            codes: vec![0; 7],
            levels: 2,
        },
        GeeOptions::default(),
        &c,
    ));
    assert_eq!(
        linear_mixed(
            &y,
            &x,
            std::slice::from_ref(&g),
            &[],
            MixedOptions {
                iteration: IterationOptions {
                    max_iterations: 1,
                    tolerance: 1e-12
                },
                ..Default::default()
            },
            &c
        )
        .unwrap_err(),
        Error::ComputationFailed
    );
    c.cancellation.cancel();
    assert_eq!(
        gee(&y, &x, &g, GeeOptions::default(), &c).unwrap_err(),
        Error::Cancelled
    );
    let mut c = control();
    c.deadline = Instant::now();
    assert_eq!(
        generalized_mixed(
            &y,
            &x,
            &g,
            ResponseFamily::Poisson,
            true,
            IterationOptions::default(),
            &c
        )
        .unwrap_err(),
        Error::DeadlineExceeded
    );
}

#[test]
fn scale_limits_mixed_supports_additional_slopes_and_nested_levels() {
    let n = 128usize;
    let x = (1..=3)
        .map(|j| {
            (0..n)
                .map(|i| {
                    if (i & j).count_ones().is_multiple_of(2) {
                        1.0
                    } else {
                        -1.0
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let y = (0..n)
        .map(|i| {
            5.0 + 0.2 * x[0][i]
                + 0.3 * x[1][i]
                + 0.4 * x[2][i]
                + if (i & 7).count_ones().is_multiple_of(2) {
                    0.2
                } else {
                    -0.2
                }
        })
        .collect::<Vec<_>>();
    let groups = (3..=6)
        .map(|shift| Grouping {
            codes: (0..n).map(|i| i >> shift).collect(),
            levels: n >> shift,
        })
        .collect::<Vec<_>>();
    for (grouping, slopes, nested, count) in [
        (&groups[..1], &x[..], false, 4),
        (&groups[..], &[][..], true, 4),
    ] {
        let result = linear_mixed(
            &y,
            &x,
            grouping,
            slopes,
            MixedOptions {
                nested,
                estimation: MixedEstimation::Ml,
                ..Default::default()
            },
            &control(),
        )
        .unwrap();
        assert_eq!(result.variance_components.len(), count);
        for (coefficient, expected) in result.coefficients.iter().zip([5.0, 0.2, 0.3, 0.4]) {
            close(coefficient.estimate, expected, 1e-7);
        }
        assert!(result.variance_components.iter().all(|v| v.variance < 1e-5));
    }
}
