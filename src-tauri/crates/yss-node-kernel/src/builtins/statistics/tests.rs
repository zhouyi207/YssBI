use super::common::field;
use crate::*;
use std::{
    borrow::Cow,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use yss_data_contract::{TabularScalar, ValueType};

fn string(v: &str) -> RuntimeValue {
    TabularScalar::String(v.into()).into()
}
fn int(v: i64) -> RuntimeValue {
    TabularScalar::Integer(v).into()
}
fn flag(v: bool) -> RuntimeValue {
    TabularScalar::Bool(v).into()
}
fn number(v: f64) -> RuntimeValue {
    RuntimeValue::float64(v).unwrap()
}
fn series(v: &[f64]) -> RuntimeValue {
    RuntimeValue::List(v.iter().map(|v| number(*v)).collect())
}
fn run(
    id: &str,
    inputs: &[(&str, RuntimeValue)],
    parameters: &[(&str, RuntimeValue)],
    count: usize,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    );
    let outputs = (0..count)
        .map(|i| KernelOutputSpec {
            data_type: if (count == 3 && i > 0) || id.ends_with("predict") {
                ValueType::DataSeries(Box::new(ValueType::number()))
            } else {
                ValueType::Struct("statistics.report".into())
            },
            fields: None,
        })
        .collect::<Vec<_>>();
    KernelRegistry::default().execute(
        &KernelId::new(id.into()).unwrap(),
        &KernelInvocation {
            relations: &crate::tests::relations(),
            inputs: &inputs.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>(),
            input_keys: &inputs.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
            parameters: parameters
                .iter()
                .map(|(k, v)| {
                    (
                        KernelParameterKey::new((*k).into()).unwrap(),
                        Cow::Borrowed(v),
                    )
                })
                .collect(),
            outputs: &outputs,
            control: &control,
        },
    )
}
fn noise(n: usize) -> Vec<f64> {
    let mut state = 177u64;
    (0..n)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((state >> 32) as u32) as f64 / u32::MAX as f64 - 0.5
        })
        .collect()
}

#[test]
fn theil_node_enforces_form_weights_and_returns_structured_result() {
    let id = "yssbi.statistics.inequality.theil";
    let grouped = run(
        id,
        &[
            ("series", series(&[1., 3.])),
            ("weights", series(&[3., 1.])),
        ],
        &[("theil_form", string("grouped"))],
        1,
    )
    .unwrap();
    let individual = run(
        id,
        &[("series", series(&[1., 1., 1., 3.]))],
        &[("theil_form", string("individual"))],
        1,
    )
    .unwrap();
    let scalar = |v: &RuntimeValue| super::super::numeric_input(Some(v)).unwrap();
    assert!(
        (scalar(field(&grouped[0], "theil_t").unwrap())
            - scalar(field(&individual[0], "theil_t").unwrap()))
        .abs()
            < 1e-12
    );
    assert_eq!(grouped.len(), 1);
    assert_eq!(field(&grouped[0], "form").unwrap(), &string("grouped"));
    assert_eq!(field(&grouped[0], "observations").unwrap(), &int(2));
    for (form, weights) in [
        ("individual", Some(series(&[1., 1.]))),
        ("grouped", None),
        ("invalid", None),
    ] {
        let mut inputs = vec![("series", series(&[1., 3.]))];
        if let Some(weights) = weights {
            inputs.push(("weights", weights));
        }
        assert!(matches!(
            run(id, &inputs, &[("theil_form", string(form))], 1),
            Err(KernelError::InvalidParameter)
        ));
    }
    assert!(matches!(
        run(
            id,
            &[("series", series(&[1., 3.])), ("weights", series(&[1.]))],
            &[("theil_form", string("grouped"))],
            1,
        ),
        Err(KernelError::ShapeMismatch)
    ));
    assert!(matches!(
        run(
            id,
            &[(
                "series",
                RuntimeValue::List(vec![number(1.), TabularScalar::Null.into()].into())
            )],
            &[("theil_form", string("individual"))],
            1,
        ),
        Err(KernelError::InvalidNumericInput)
    ));
}

#[test]
fn acf_pacf_nodes_return_finite_correlations_for_large_finite_inputs() {
    for (method, expected) in [
        ("acf", vec![number(1.0), number(-0.75)]),
        ("pacf", vec![number(-0.75)]),
    ] {
        let result = run(
            &format!("yssbi.statistics.timeseries.{method}"),
            &[("series", series(&[1e308, -1e308, 1e308, -1e308]))],
            &[("lags", int(1))],
            1,
        )
        .unwrap();
        assert_eq!(
            field(&result[0], "values").unwrap(),
            &RuntimeValue::List(expected.into())
        );
        assert_eq!(field(&result[0], "observations").unwrap(), &int(4));
        assert_eq!(result.len(), 1);
    }
}

#[test]
fn nonfinite_diagnostic_results_fail_instead_of_returning_null_reports() {
    assert!(matches!(
        run(
            "yssbi.statistics.diagnostic.durbin_watson",
            &[("series", series(&[1e308, -1e308, 1e308, -1e308]))],
            &[],
            1,
        ),
        Err(KernelError::NonFiniteResult)
    ));
}

#[test]
fn statistical_value_checks_nested_numbers_while_preserving_optional_nulls() {
    #[derive(serde::Serialize)]
    struct Report {
        statistic: f64,
        optional: Option<f64>,
        samples: Vec<Option<f64>>,
    }
    let control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(5),
    );
    let inv = KernelInvocation {
        relations: &crate::tests::relations(),
        inputs: &[],
        input_keys: &[],
        parameters: Default::default(),
        outputs: &[],
        control: &control,
    };
    let report = Report {
        statistic: 0.25,
        optional: None,
        samples: vec![Some(0.5), None],
    };
    let result = super::common::value(&report, &inv).unwrap();
    assert_eq!(
        field(&result, "optional").unwrap(),
        &RuntimeValue::from(TabularScalar::Null)
    );
    assert_eq!(
        field(&result, "samples").unwrap(),
        &RuntimeValue::List(vec![number(0.5), TabularScalar::Null.into()].into())
    );
    for report in [
        Report {
            statistic: f64::NAN,
            ..report
        },
        Report {
            statistic: 0.25,
            optional: Some(f64::INFINITY),
            samples: vec![],
        },
        Report {
            statistic: 0.25,
            optional: None,
            samples: vec![Some(f64::NEG_INFINITY)],
        },
    ] {
        assert!(matches!(
            super::common::value(report, &inv),
            Err(KernelError::NonFiniteResult)
        ));
    }
}

#[test]
fn binary_and_prais_nodes_honor_options_and_predict_without_refitting() {
    let y = series(&[0., 0., 1., 0., 1., 0., 1., 1.]);
    let x = series(&[0., 1., 2., 3., 4., 5., 6., 7.]);
    for method in ["logit", "probit"] {
        let parameters = [
            ("constant", flag(false)),
            ("max_iterations", int(100)),
            ("tolerance", number(1e-8)),
        ];
        let fit = run(
            &format!("yssbi.statistics.{method}.fit"),
            &[("response", y.clone()), ("predictors", x.clone())],
            &parameters,
            3,
        )
        .unwrap();
        let prediction = run(
            &format!("yssbi.statistics.{method}.predict"),
            &[("model", fit[0].clone()), ("predictors", x.clone())],
            &[],
            1,
        )
        .unwrap();
        assert_eq!(prediction[0], fit[1]);
        assert_eq!(field(&fit[0], "constant").unwrap(), &flag(false));
        let summary = run(
            &format!("yssbi.statistics.{method}.summary"),
            &[("model", fit[0].clone())],
            &[],
            1,
        )
        .unwrap();
        assert!(field(&fit[0], "report").is_err());
        assert_eq!(
            field(&summary[0], "betas").unwrap(),
            field(&fit[0], "coefficients").unwrap()
        );
        assert_eq!(
            field(field(&summary[0], "model_basic_info").unwrap(), "df_model").unwrap(),
            &int(1)
        );
        assert!(
            run(
                &format!("yssbi.statistics.{method}.fit"),
                &[("response", y.clone()), ("predictors", x.clone())],
                &[
                    ("constant", flag(true)),
                    ("max_iterations", int(1)),
                    ("tolerance", number(1e-14))
                ],
                3
            )
            .is_err()
        );
    }
    let eps = noise(80);
    let px = (0..80).map(|i| i as f64 / 10.).collect::<Vec<_>>();
    let py = (0..80)
        .map(|i| 2. + 0.4 * px[i] + eps[i])
        .collect::<Vec<_>>();
    for transform in ["prais_winsten", "cochrane_orcutt"] {
        let fit = run(
            "yssbi.statistics.prais.fit",
            &[("response", series(&py)), ("predictors", series(&px))],
            &[
                ("constant", flag(true)),
                ("max_iterations", int(100)),
                ("tolerance", number(1e-6)),
                ("transform", string(transform)),
            ],
            3,
        )
        .unwrap();
        let summary = run(
            "yssbi.statistics.prais.summary",
            &[("model", fit[0].clone())],
            &[],
            1,
        )
        .unwrap();
        assert!(field(field(&summary[0], "diagnostic_info").unwrap(), "prais_info").is_ok());
    }
}

#[test]
fn iv_nodes_accept_multiple_instruments_and_preserve_identification_results() {
    let n = 120;
    let eps = noise(n * 4);
    let x = eps[..n].to_vec();
    let z1 = eps[n..n * 2].to_vec();
    let z2 = eps[n * 2..n * 3].to_vec();
    let endog = (0..n)
        .map(|i| 0.4 * x[i] + 0.9 * z1[i] + 0.7 * z2[i] + 0.3 * eps[n * 3 + i])
        .collect::<Vec<_>>();
    let y = (0..n)
        .map(|i| 2. + 0.3 * x[i] + 1.5 * endog[i] + 0.4 * eps[n * 3 + i])
        .collect::<Vec<_>>();
    for method in ["2sls", "liml"] {
        let fit = run(
            &format!("yssbi.statistics.iv.{method}.fit"),
            &[
                ("response", series(&y)),
                ("predictors", series(&x)),
                ("endogenous", series(&endog)),
                ("instruments", series(&z1)),
                ("instruments", series(&z2)),
            ],
            &[
                ("constant", flag(true)),
                ("covariance", string("nonrobust")),
                ("small", flag(false)),
            ],
            3,
        )
        .unwrap();
        assert!(field(&fit[0], "hausman").is_err());
        assert!(field(&fit[0], "firstStage").is_err());
        let mut options = vec![
            ("model_summary", flag(true)),
            ("coefficient_table", flag(true)),
            ("first_stage", flag(true)),
            ("overidentification", flag(true)),
        ];
        if method == "2sls" {
            options.push(("endogeneity", flag(true)));
        }
        let summary = run(
            &format!("yssbi.statistics.iv.{method}.summary"),
            &[("model", fit[0].clone())],
            &options,
            1,
        )
        .unwrap();
        assert_eq!(
            field(
                field(field(&summary[0], "firstStage").unwrap(), "statistics").unwrap(),
                "k_excluded_instruments"
            )
            .unwrap(),
            &int(2)
        );
        assert!(!matches!(
            field(&summary[0], "overidentification").unwrap(),
            RuntimeValue::Scalar(TabularScalar::Null)
        ));
        if method == "2sls" {
            let hausman = run(
                "yssbi.statistics.diagnostic.hausman",
                &[("model", fit[0].clone())],
                &[],
                1,
            )
            .unwrap();
            assert_eq!(
                field(&hausman[0], "hausman").unwrap(),
                field(field(&summary[0], "endogeneity").unwrap(), "hausman").unwrap()
            );
        }
        for (key, val) in &mut options {
            if !matches!(*key, "model_summary" | "coefficient_table") {
                *val = flag(false);
            }
        }
        let basic = run(
            &format!("yssbi.statistics.iv.{method}.summary"),
            &[("model", fit[0].clone())],
            &options,
            1,
        )
        .unwrap();
        assert!(field(&basic[0], "firstStage").is_err());
        assert!(field(&basic[0], "overidentification").is_err());
        assert_eq!(
            field(&basic[0], "coefficients").unwrap(),
            field(&summary[0], "coefficients").unwrap()
        );
    }
}

#[test]
fn panel_nodes_select_each_implemented_estimator_and_reject_unsupported_combinations() {
    let n = 72;
    let eps = noise(n * 2);
    // Mixed signed zeros must identify the same entity/time level, without lookup panics.
    let entities = (0..n)
        .map(|i| if i == 0 { -0.0 } else { (i / 6) as f64 })
        .collect::<Vec<_>>();
    let times = (0..n)
        .map(|i| if i == 6 { -0.0 } else { (i % 6) as f64 })
        .collect::<Vec<_>>();
    let x = eps[..n].to_vec();
    let y = (0..n)
        .map(|i| 1. + 0.5 * entities[i] + 1.2 * times[i] + 1.5 * x[i] + 0.3 * eps[n + i])
        .collect::<Vec<_>>();
    let inputs = [
        ("response", series(&y)),
        ("predictors", series(&x)),
        ("entity", series(&entities)),
        ("time", series(&times)),
    ];
    for estimator in [
        "fixed_effects",
        "lsdv",
        "random_effects",
        "maximum_likelihood",
        "between",
        "first_difference",
    ] {
        for effects in ["entity", "time", "two_way"] {
            let result = run(
                "yssbi.statistics.panel.fit",
                &inputs,
                &[
                    ("constant", flag(true)),
                    ("estimator", string(estimator)),
                    ("effects", string(effects)),
                    ("covariance", string("nonrobust")),
                ],
                1,
            );
            if (estimator == "between" && effects == "two_way")
                || (estimator == "first_difference" && effects != "entity")
            {
                assert!(matches!(result, Err(KernelError::InvalidParameter)));
                continue;
            }
            let fit = result.unwrap_or_else(|error| panic!("{estimator}/{effects}: {error:?}"));
            let report = run(
                "yssbi.statistics.panel.summary",
                &[("model", fit[0].clone())],
                &[
                    ("model_summary", flag(true)),
                    ("coefficient_table", flag(true)),
                    ("effects_statistics", flag(true)),
                    ("estimator_statistics", flag(true)),
                ],
                1,
            )
            .unwrap();
            assert!(field(&report[0], "coefficients").is_ok());
            assert!(
                field(
                    field(field(&report[0], "coefficients").unwrap(), "inference").unwrap(),
                    "covariance"
                )
                .is_ok()
            );
        }
    }
}

#[test]
fn time_series_nodes_preserve_multivariate_postestimation_results() {
    let n = 160;
    let eps = noise(n * 3);
    let mut x = vec![0.; n];
    let mut y = vec![0.; n];
    for i in 1..n {
        x[i] = 0.4 * x[i - 1] + eps[i];
        y[i] = 0.3 * y[i - 1] + 0.2 * x[i - 1] + eps[n + i];
    }
    let inputs = [("variables", series(&x)), ("variables", series(&y))];
    let fit = run("yssbi.statistics.var.fit", &inputs, &[("lags", int(1))], 1).unwrap();
    for (id, field_name) in [("granger", "vargranger"), ("irf", "oirf"), ("fevd", "fevd")] {
        let report = run(
            &format!("yssbi.statistics.timeseries.{id}"),
            &[("model", fit[0].clone())],
            &if id == "granger" {
                vec![]
            } else {
                vec![("steps", int(4))]
            },
            1,
        )
        .unwrap();
        assert!(field(&fit[0], field_name).is_err());
        let RuntimeValue::List(rows) = field(&report[0], field_name).unwrap() else {
            panic!("expected postestimation rows");
        };
        if id == "granger" {
            assert!(!rows.is_empty());
        } else {
            assert_eq!(rows.len(), 5);
        }
        if id == "fevd" {
            for horizon in rows.iter() {
                let RuntimeValue::List(responses) = horizon else {
                    panic!();
                };
                for response in responses.iter() {
                    let RuntimeValue::List(shares) = response else {
                        panic!();
                    };
                    let total: f64 = shares
                        .iter()
                        .map(|v| crate::builtins::numeric_input(Some(v)).unwrap())
                        .sum();
                    assert!((total - 1.0).abs() < 1e-10);
                }
            }
        }
    }
    let summary = run(
        "yssbi.statistics.var.summary",
        &[("model", fit[0].clone())],
        &[
            ("model_summary", flag(true)),
            ("coefficient_table", flag(true)),
            ("lag_exclusion", flag(true)),
            ("serial_tests", flag(true)),
            ("stability", flag(true)),
            ("serial_lags", int(2)),
        ],
        1,
    )
    .unwrap();
    assert!(field(&summary[0], "lagExclusion").is_ok());
    assert!(field(&summary[0], "serialTests").is_ok());
    assert!(field(&summary[0], "design").is_err());
    let basic = run(
        "yssbi.statistics.var.summary",
        &[("model", fit[0].clone())],
        &[
            ("model_summary", flag(true)),
            ("coefficient_table", flag(true)),
            ("lag_exclusion", flag(false)),
            ("serial_tests", flag(false)),
            ("stability", flag(false)),
            ("serial_lags", int(2)),
        ],
        1,
    )
    .unwrap();
    assert!(field(&basic[0], "serialTests").is_err());
    assert!(field(&basic[0], "stability").is_err());
    run(
        "yssbi.statistics.var.lag_order",
        &inputs,
        &[("max_lags", int(2))],
        1,
    )
    .unwrap();
    run(
        "yssbi.statistics.adf.test",
        &[("series", series(&x))],
        &[("lags", int(0)), ("regression", string("constant"))],
        1,
    )
    .unwrap();
    for i in 1..n {
        x[i] = x[i - 1] + eps[n * 2 + i];
        y[i] = x[i] + 0.4 * eps[i];
    }
    let inputs = [("variables", series(&x)), ("variables", series(&y))];
    let fit = run(
        "yssbi.statistics.vec.fit",
        &inputs,
        &[
            ("rank", int(1)),
            ("lags", int(2)),
            ("trend", string("constant")),
        ],
        1,
    )
    .unwrap();
    assert!(field(&fit[0], "veclmar").is_err());
    assert!(field(&fit[0], "vecstable").is_err());
    let report = run(
        "yssbi.statistics.vec.summary",
        &[("model", fit[0].clone())],
        &[
            ("model_summary", flag(true)),
            ("coefficient_table", flag(true)),
            ("cointegration", flag(true)),
            ("serial_tests", flag(true)),
            ("stability", flag(true)),
            ("serial_lags", int(2)),
        ],
        1,
    )
    .unwrap();
    assert!(field(&report[0], "cointegration").is_ok());
    assert!(field(&report[0], "serialTests").is_ok());
    assert!(field(&report[0], "stability").is_ok());
    run(
        "yssbi.statistics.vec.rank_test",
        &inputs,
        &[("max_lags", int(2)), ("trend", string("constant"))],
        1,
    )
    .unwrap();
}

#[test]
fn breusch_pagan_fitted_and_rhs_agree_for_one_predictor() {
    let n = 64;
    let eps = noise(n);
    let x = (0..n).map(|i| i as f64 / 10.).collect::<Vec<_>>();
    let y = (0..n)
        .map(|i| 1. + 0.6 * x[i] + eps[i] * (1. + x[i]))
        .collect::<Vec<_>>();
    let weights = x.iter().map(|x| 1. / (1. + x).powi(2)).collect::<Vec<_>>();
    for method in ["OLS", "WLS"] {
        let mut inputs = vec![("response", series(&y)), ("predictors", series(&x))];
        if method == "WLS" {
            inputs.push(("weights", series(&weights)));
        }
        let fit = run(
            "yssbi.statistics.linear.fit",
            &inputs,
            &[
                ("method", string(method)),
                ("constant", flag(true)),
                ("covariance", string("nonrobust")),
            ],
            3,
        )
        .unwrap();
        // A nonconstant fitted line and its sole predictor span the same
        // auxiliary design with an intercept, for either weighting scheme.
        for koenker in [false, true] {
            let reports = [false, true].map(|rhs| {
                run(
                    "yssbi.statistics.diagnostic.breusch_pagan",
                    &[("model", fit[0].clone())],
                    &[("rhs", flag(rhs)), ("koenker", flag(koenker))],
                    1,
                )
                .unwrap()
            });
            for key in ["lm_stat", "p_value"] {
                let values = reports.each_ref().map(|report| {
                    let result = field(&report[0], "result").unwrap();
                    super::super::numeric_input(Some(field(result, key).unwrap())).unwrap()
                });
                assert!(
                    (values[0] - values[1]).abs() < 1e-8,
                    "{method}, koenker={koenker}, {key}: {values:?}"
                );
            }
        }
    }
}

#[test]
fn weighted_diagnostics_and_cluster_covariance_use_fitted_observations() {
    let n = 64;
    let eps = noise(n);
    let x = (0..n).map(|i| i as f64 / 10.).collect::<Vec<_>>();
    let y = (0..n)
        .map(|i| 1. + 0.6 * x[i] + eps[i] * (1. + x[i]))
        .collect::<Vec<_>>();
    let weights = (0..n).map(|i| 1. / (1. + x[i]).powi(2)).collect::<Vec<_>>();
    let fit = run(
        "yssbi.statistics.linear.fit",
        &[
            ("response", series(&y)),
            ("predictors", series(&x)),
            ("weights", series(&weights)),
        ],
        &[
            ("method", string("WLS")),
            ("constant", flag(true)),
            ("covariance", string("nonrobust")),
        ],
        3,
    )
    .unwrap();
    let RuntimeValue::LinearRegression(model) = &fit[0] else {
        panic!()
    };
    assert_eq!(model.weights.as_ref().unwrap(), &weights);
    for (test, parameters) in [
        (
            "breusch_pagan",
            vec![("rhs", flag(true)), ("koenker", flag(true))],
        ),
        ("white", vec![]),
        ("information_matrix", vec![]),
        ("reset", vec![("rhs", flag(false))]),
        ("vif", vec![]),
        ("leverage", vec![]),
        (
            "breusch_godfrey",
            vec![("lags", int(1)), ("bg_nomiss0", flag(true))],
        ),
        ("wald", vec![("hypothesis", string("x1 = 0"))]),
    ] {
        let report = run(
            &format!("yssbi.statistics.diagnostic.{test}"),
            &[("model", fit[0].clone())],
            &parameters,
            1,
        )
        .unwrap_or_else(|e| panic!("{test}: {e:?}"));
        if test == "vif" {
            let RuntimeValue::List(entries) = field(&report[0], "result").unwrap() else {
                panic!()
            };
            assert_eq!(
                field(&entries[0], "vif").unwrap(),
                &TabularScalar::Null.into()
            );
            assert_eq!(
                field(&entries[0], "tolerance").unwrap(),
                &TabularScalar::Null.into()
            );
        }
    }
    for (id, params) in [
        ("test.normality", vec![]),
        ("diagnostic.durbin_watson", vec![]),
        ("diagnostic.ljung_box", vec![("lags", int(2))]),
        ("timeseries.acf", vec![("lags", int(4))]),
        ("timeseries.pacf", vec![("lags", int(4))]),
    ] {
        run(
            &format!("yssbi.statistics.{id}"),
            &[("series", fit[2].clone())],
            &params,
            1,
        )
        .unwrap();
    }
    let clustered = run(
        "yssbi.statistics.linear.fit",
        &[
            ("response", series(&y)),
            ("predictors", series(&x)),
            (
                "clusters",
                series(&(0..n).map(|i| (i / 8) as f64).collect::<Vec<_>>()),
            ),
        ],
        &[
            ("method", string("OLS")),
            ("constant", flag(true)),
            ("covariance", string("cluster")),
        ],
        3,
    )
    .unwrap();
    let RuntimeValue::LinearRegression(model) = &clustered[0] else {
        panic!()
    };
    assert_eq!(model.report.model_basic_info.covariance_type, "cluster");
    let density = run("yssbi.plot.kde.view", &[("values", fit[2].clone())], &[], 1).unwrap();
    let RuntimeValue::List(points) = field(&density[0], "points").unwrap() else {
        panic!()
    };
    assert_eq!(points.len(), 256);
    assert!(matches!(
        run(
            "yssbi.plot.kde.view",
            &[("values", series(&[-f64::MAX, f64::MAX]))],
            &[],
            1
        ),
        Err(KernelError::NonFiniteResult)
    ));
}

#[test]
fn did_randomization_node_is_reproducible_and_reports_valid_permutations() {
    let n = 48;
    let eps = noise(n);
    let entity = (0..n).map(|i| (i / 6) as f64).collect::<Vec<_>>();
    let time = (0..n).map(|i| (i % 6) as f64).collect::<Vec<_>>();
    let treat = entity
        .iter()
        .map(|v| f64::from(*v < 3.))
        .collect::<Vec<_>>();
    let post = time.iter().map(|v| f64::from(*v >= 3.)).collect::<Vec<_>>();
    let y = (0..n)
        .map(|i| entity[i] * 0.4 + time[i] * 0.3 + treat[i] * post[i] * 1.75 + eps[i] * 0.01)
        .collect::<Vec<_>>();
    let inputs = [
        ("response", series(&y)),
        ("entity", series(&entity)),
        ("time", series(&time)),
        ("treat", series(&treat)),
        ("post", series(&post)),
    ];
    let parameters = [("repetitions", int(20)), ("seed", int(42))];
    let first = run(
        "yssbi.statistics.panel.did.randomization",
        &inputs,
        &parameters,
        1,
    )
    .unwrap();
    let second = run(
        "yssbi.statistics.panel.did.randomization",
        &inputs,
        &parameters,
        1,
    )
    .unwrap();
    assert_eq!(first, second);
    assert_eq!(field(&first[0], "available").unwrap(), &flag(true));
    assert_eq!(field(&first[0], "n_perm_valid").unwrap(), &int(20));
    let did = run(
        "yssbi.statistics.panel.did.twfe",
        &[
            ("response", series(&y)),
            ("entity", series(&entity)),
            ("time", series(&time)),
            (
                "treatment",
                series(&(0..n).map(|i| treat[i] * post[i]).collect::<Vec<_>>()),
            ),
        ],
        &[],
        1,
    )
    .unwrap();
    assert_eq!(did.len(), 1);
    assert!(field(&did[0], "model").is_ok());
    assert!(field(&did[0], "summary").is_ok());
}
