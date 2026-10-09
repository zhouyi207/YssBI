use super::common::field;
mod association;
mod causal_models;
mod classical;
mod decision;
mod descriptive;
mod diagnostics;
mod inference;
mod meta;
mod panel_models;
mod path;
mod preparation;
mod regression_models;
mod spatial;
mod survival;
mod time_series;
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
    KernelRegistry::default()
        .execute(
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
        .inspect(|values| {
            for value in values {
                validate_display(value);
            }
        })
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
fn longitudinal_adapter_preserves_wide_labels_and_rejects_nulls_shapes_and_excess_budget() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../yss-sci/src/longitudinal/fixtures/reference.json"
    ))
    .unwrap();
    let y: Vec<f64> = serde_json::from_value(fixture["cases"]["lmm_reml"]["y"].clone()).unwrap();
    let x: Vec<f64> = serde_json::from_value(fixture["x"][0].clone()).unwrap();
    let labels = RuntimeValue::List(
        fixture["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|g| int(9_007_199_254_740_992 + g.as_i64().unwrap()))
            .collect(),
    );
    let mut inputs = vec![
        ("y", series(&y)),
        ("x", series(&x)),
        ("groups", labels.clone()),
    ];
    let parameters = [
        ("constant", flag(true)),
        ("max_iterations", int(500)),
        ("tolerance", number(1e-7)),
        ("mixed_estimation", string("reml")),
    ];
    let id = "yssbi.statistics.mixed.random_intercept";
    let result = run(id, &inputs, &parameters, 1).unwrap();
    let RuntimeValue::List(groups) = field(&result[0], "group_labels").unwrap() else {
        panic!("groups")
    };
    let RuntimeValue::List(levels) = &groups[0] else {
        panic!("levels")
    };
    assert_eq!(levels.len(), 18);
    assert_eq!(levels[0], int(9_007_199_254_740_992));
    assert_eq!(levels[1], int(9_007_199_254_740_993));
    inputs[2].1 = RuntimeValue::List(vec![TabularScalar::Null.into(); y.len()].into());
    assert!(matches!(
        run(id, &inputs, &parameters, 1),
        Err(KernelError::InvalidNumericInput)
    ));
    inputs[2].1 = series(&[0., 1.]);
    assert!(matches!(
        run(id, &inputs, &parameters, 1),
        Err(KernelError::ShapeMismatch)
    ));
    inputs[2].1 = labels;
    let mut control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    );
    // Input columns fit, but the admitted dense mixed-model workspace does not.
    control.max_input_bytes = 512 * 1024;
    let inputs = inputs.into_iter().map(|(_, v)| v).collect::<Vec<_>>();
    let outputs = [KernelOutputSpec {
        data_type: ValueType::Struct("statistics.report".into()),
        fields: None,
    }];
    let inv = KernelInvocation {
        relations: &crate::tests::relations(),
        inputs: &inputs,
        input_keys: &["y", "x", "groups"],
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
    };
    assert!(matches!(
        KernelRegistry::default().execute(&KernelId::new(id.into()).unwrap(), &inv),
        Err(KernelError::BudgetExceeded)
    ));
}

#[test]
fn anova_adapters_preserve_exact_labels_and_enforce_shapes_and_parameters() {
    let id = "yssbi.statistics.anova.one_way";
    let wide = [9_007_199_254_740_992i64, 9_007_199_254_740_993];
    let labels = RuntimeValue::List(
        [wide[0], wide[0], wide[0], wide[1], wide[1], wide[1]]
            .into_iter()
            .map(int)
            .collect(),
    );
    let result = run(
        id,
        &[
            ("y", series(&[1., 2., 3., 2., 4., 6.])),
            ("factors", labels.clone()),
        ],
        &[],
        1,
    )
    .unwrap();
    let RuntimeValue::List(factors) = field(&result[0], "factors").unwrap() else {
        panic!("factor metadata");
    };
    assert_eq!(
        field(&factors[0], "levels").unwrap(),
        &RuntimeValue::List(wide.into_iter().map(int).collect())
    );
    assert!(matches!(
        run(id, &[("y", series(&[1., 2.])), ("factors", labels)], &[], 1),
        Err(KernelError::ShapeMismatch)
    ));
    let nulls = RuntimeValue::List(vec![int(0), TabularScalar::Null.into(), int(1), int(1)].into());
    assert!(matches!(
        run(
            id,
            &[("y", series(&[1., 2., 3., 4.])), ("factors", nulls)],
            &[],
            1
        ),
        Err(KernelError::InvalidNumericInput)
    ));
    let repeated = "yssbi.statistics.anova.repeated_measures";
    let inputs = [
        ("y", series(&[1., 3., 2., 5., 4., 5.])),
        (
            "subjects",
            RuntimeValue::List(
                ["乙", "乙", "甲", "甲", "丙", "丙"]
                    .into_iter()
                    .map(string)
                    .collect(),
            ),
        ),
        (
            "factors",
            RuntimeValue::List(
                ["before", "after", "before", "after", "before", "after"]
                    .into_iter()
                    .map(string)
                    .collect(),
            ),
        ),
    ];
    let result = run(
        repeated,
        &inputs,
        &[("sphericity_correction", string("greenhouse_geisser"))],
        1,
    )
    .unwrap();
    assert_eq!(field(&result[0], "subjects").unwrap(), &int(3));
    assert!(matches!(
        run(
            repeated,
            &inputs,
            &[("sphericity_correction", string("invalid"))],
            1
        ),
        Err(KernelError::InvalidParameter)
    ));
}

#[test]
fn classical_count_tables_admit_their_actual_workspace_before_computation() {
    let relations = crate::tests::relations();
    let large_inputs = [series(&vec![10.0; 2002])];
    let large_outputs = [KernelOutputSpec {
        data_type: ValueType::Struct("statistics.report".into()),
        fields: None,
    }];
    let large_control = KernelControl {
        cancellation: Arc::new(AtomicBool::new(false)),
        deadline: Instant::now() + Duration::from_secs(30),
        max_input_bytes: 256 * 1024,
    };
    let kernels = KernelRegistry::default();
    let pearson = KernelId::new("yssbi.statistics.test.chisquare.general".into()).unwrap();
    for (rows, columns) in [(2, 1001), (1001, 2)] {
        let large_invocation = KernelInvocation {
            relations: &relations,
            inputs: &large_inputs,
            input_keys: &["counts"],
            parameters: [
                (
                    KernelParameterKey::new("rows".into()).unwrap(),
                    std::borrow::Cow::Owned(int(rows)),
                ),
                (
                    KernelParameterKey::new("columns".into()).unwrap(),
                    std::borrow::Cow::Owned(int(columns)),
                ),
            ]
            .into(),
            outputs: &large_outputs,
            control: &large_control,
        };
        let result = kernels.execute(&pearson, &large_invocation).unwrap();
        assert_eq!(
            super::super::numeric_input(Some(field(&result[0], "statistic").unwrap())).unwrap(),
            0.0
        );
        assert_eq!(
            super::super::numeric_input(Some(field(&result[0], "p_value").unwrap())).unwrap(),
            1.0
        );
        let limited = KernelControl {
            cancellation: large_control.cancellation.clone(),
            deadline: large_control.deadline,
            max_input_bytes: 4096,
        };
        assert!(matches!(
            kernels.execute(
                &pearson,
                &KernelInvocation {
                    control: &limited,
                    ..large_invocation
                }
            ),
            Err(KernelError::BudgetExceeded)
        ));
    }
    let distinct = RuntimeValue::List((0..96).map(int).collect());
    let inputs = [distinct.clone(), distinct];
    let mut control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    );
    control.max_input_bytes = 32 * 1024;
    let outputs = [KernelOutputSpec {
        data_type: ValueType::Struct("statistics.report".into()),
        fields: None,
    }];
    let mut inv = KernelInvocation {
        relations: &relations,
        inputs: &inputs,
        input_keys: &["row", "column"],
        parameters: Default::default(),
        outputs: &outputs,
        control: &control,
    };
    let id = KernelId::new("yssbi.statistics.test.chisquare.crosstab".into()).unwrap();
    // Encoded columns fit, but the actual 96 by 96 count matrix alone exceeds the budget.
    assert!(matches!(
        kernels.execute(&id, &inv),
        Err(KernelError::BudgetExceeded)
    ));
    let admitted = KernelControl {
        cancellation: control.cancellation.clone(),
        deadline: control.deadline,
        max_input_bytes: 256 * 1024,
    };
    inv.control = &admitted;
    assert!(kernels.execute(&id, &inv).is_ok());

    // Equal row counts with only two levels must not be charged as a 96 by 96 table.
    let repeated = RuntimeValue::List((0..96).map(|i| int(i % 2)).collect());
    let repeated_inputs = [repeated.clone(), repeated];
    inv.inputs = &repeated_inputs;
    inv.control = &control;
    assert!(kernels.execute(&id, &inv).is_ok());

    // A small physical dictionary can expand to much larger logical string data.
    let labels = Arc::new(arrow_array::StringArray::from(vec![
        "a".repeat(512),
        "b".repeat(512),
    ]));
    let dictionary = arrow_array::DictionaryArray::<arrow_array::types::Int8Type>::try_new(
        arrow_array::Int8Array::from((0..256).map(|i| (i % 2) as i8).collect::<Vec<_>>()),
        labels,
    )
    .unwrap();
    let mut categories = Vec::new();
    let mut bytes = 0;
    assert!(matches!(
        super::classical::append_category_batch(&dictionary, &mut categories, &mut bytes, 0, &inv),
        Err(KernelError::BudgetExceeded)
    ));
    assert!(categories.len() < 256);
    categories.clear();
    bytes = 0;
    inv.control = &admitted;
    super::classical::append_category_batch(&dictionary, &mut categories, &mut bytes, 0, &inv)
        .unwrap();
    assert_eq!(categories.len(), 256);
    assert_eq!(categories[0].as_ref(), format!("s:{}", "a".repeat(512)));
}

#[test]
fn association_ordinal_order_and_wide_category_labels_survive_kernel_mapping() {
    use yss_data_contract::{ColumnSemantic, ConversionMetadata, SemanticType, SemanticValue};
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../yss-sci/src/association/fixtures/reference.json"
    ))
    .unwrap();
    let codes: Vec<Vec<usize>> =
        serde_json::from_value(fixture["ordinal_weighted"]["ratings"].clone()).unwrap();
    let labels = ["Z", "M", "A", "B"];
    let ordinal = |values: &[usize]| {
        RuntimeValue::List(values.iter().map(|&i| string(labels[i])).collect())
            .with_metadata(ConversionMetadata {
                semantic: ColumnSemantic {
                    values: labels
                        .iter()
                        .map(|label| SemanticValue {
                            value: (*label).into(),
                            label: (*label).into(),
                        })
                        .collect(),
                    ..ColumnSemantic::new(SemanticType::Ordinal)
                },
                temporal: None,
                dummy_base_level: None,
            })
            .unwrap()
    };
    let outputs = run(
        "yssbi.statistics.test.kappa",
        &[
            ("ratings", ordinal(&codes[0])),
            ("ratings", ordinal(&codes[1])),
        ],
        &[
            ("kappa_method", string("cohen")),
            ("kappa_weighting", string("linear")),
            ("confidence_level", number(0.95)),
        ],
        1,
    )
    .unwrap();
    let coefficient =
        super::super::numeric_input(Some(field(&outputs[0], "coefficient").unwrap())).unwrap();
    assert!(
        (coefficient - fixture["ordinal_weighted"]["coefficient"].as_f64().unwrap()).abs() < 1e-12
    );
    let RuntimeValue::List(categories) = field(&outputs[0], "categories").unwrap() else {
        panic!("category labels")
    };
    assert_eq!(categories.len(), 4);
    assert_eq!(categories[0], string("Z"));
    let wide = |values: &[u64]| {
        RuntimeValue::List(
            values
                .iter()
                .map(|&value| TabularScalar::Unsigned(value).into())
                .collect(),
        )
    };
    let outputs = run(
        "yssbi.statistics.test.kappa",
        &[
            (
                "ratings",
                wide(&[u64::MAX, u64::MAX, u64::MAX - 1, u64::MAX - 1]),
            ),
            (
                "ratings",
                wide(&[u64::MAX, u64::MAX - 1, u64::MAX - 1, u64::MAX - 1]),
            ),
        ],
        &[
            ("kappa_method", string("cohen")),
            ("kappa_weighting", string("none")),
            ("confidence_level", number(0.95)),
        ],
        1,
    )
    .unwrap();
    assert_eq!(field(&outputs[0], "coefficient").unwrap(), &number(0.5));
    let RuntimeValue::List(categories) = field(&outputs[0], "categories").unwrap() else {
        panic!("wide labels")
    };
    assert_eq!(
        categories[0],
        RuntimeValue::Scalar(TabularScalar::Unsigned(u64::MAX))
    );
    assert_eq!(
        categories[1],
        RuntimeValue::Scalar(TabularScalar::Unsigned(u64::MAX - 1))
    );
    assert!(matches!(
        run(
            "yssbi.statistics.test.kappa",
            &[
                (
                    "ratings",
                    RuntimeValue::List(vec![string("A"), string("B")].into())
                ),
                (
                    "ratings",
                    RuntimeValue::List(vec![string("B"), string("A")].into())
                )
            ],
            &[
                ("kappa_method", string("cohen")),
                ("kappa_weighting", string("linear")),
                ("confidence_level", number(0.95))
            ],
            1
        ),
        Err(KernelError::InvalidParameter)
    ));
    let independent = run(
        "yssbi.statistics.association.ridit",
        &[
            ("sample", series(&[1., 2., 3.])),
            ("reference", series(&[1., 1., 2., 2., 3., 3., 3., 3.])),
        ],
        &[
            ("alternative", string("two_sided")),
            ("continuity_correction", flag(false)),
        ],
        1,
    )
    .unwrap();
    let mean =
        super::super::numeric_input(Some(field(&independent[0], "mean_ridit").unwrap())).unwrap();
    assert!((mean - 5.0 / 12.0).abs() < 1e-12);
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
            ("max_iterations", int(20000)),
            ("tolerance", number(1e-8)),
        ];
        let fit = run(
            &format!("yssbi.statistics.{method}.fit"),
            &[("y", y.clone()), ("x", x.clone())],
            &parameters,
            3,
        )
        .unwrap();
        let prediction = run(
            &format!("yssbi.statistics.{method}.predict"),
            &[("model", fit[0].clone()), ("x", x.clone())],
            &[],
            1,
        )
        .unwrap();
        assert_eq!(prediction[0], fit[1]);
        assert_eq!(field(&fit[0], "constant").unwrap(), &flag(false));
        let summary = run(
            &format!("yssbi.statistics.{method}.summary"),
            &[("model", fit[0].clone())],
            &{
                let mut p = vec![
                    ("marginal_effects", flag(true)),
                    ("marginal_evaluation", string("average")),
                    ("marginal_method", string("dydx")),
                    ("marginal_at", string("")),
                    ("classification", flag(true)),
                    ("cutoff", number(0.5)),
                    ("hypothesis_test", flag(true)),
                    ("hypothesis", string("x1 = 0")),
                ];
                if method == "logit" {
                    p.push(("odds_ratios", flag(true)));
                }
                p
            },
            1,
        )
        .unwrap();
        assert!(field(&summary[0], "marginal_effects").is_ok());
        assert!(field(&summary[0], "classification").is_ok());
        assert_eq!(
            field(field(&summary[0], "hypothesis_test").unwrap(), "test_type").unwrap(),
            &string("z")
        );
        assert_eq!(field(&summary[0], "odds_ratios").is_ok(), method == "logit");
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
                &[("y", y.clone()), ("x", x.clone())],
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
            &[("y", series(&py)), ("x", series(&px))],
            &[
                ("constant", flag(true)),
                ("max_iterations", int(20000)),
                ("tolerance", number(1e-6)),
                ("transform", string(transform)),
            ],
            3,
        )
        .unwrap();
        let summary = run(
            "yssbi.statistics.prais.summary",
            &[("model", fit[0].clone())],
            &[
                ("hypothesis_test", flag(true)),
                ("hypothesis", string("x1 = 0")),
            ],
            1,
        )
        .unwrap();
        assert!(field(field(&summary[0], "diagnostic_info").unwrap(), "prais_info").is_ok());
        assert_eq!(
            field(field(&summary[0], "hypothesis_test").unwrap(), "test_type").unwrap(),
            &string("t")
        );
    }
}

#[test]
fn iv_nodes_accept_multiple_instruments_and_preserve_identification_results() {
    let assert_first_stage_display = |summary: &RuntimeValue| {
        let first_stage = field(summary, "firstStage").unwrap();
        let RuntimeValue::List(equations) = field(first_stage, "equations").unwrap() else {
            panic!("first-stage equations")
        };
        let RuntimeValue::List(rows) = field(summary, "first_stage_rows").unwrap() else {
            panic!("first-stage display rows")
        };
        let RuntimeValue::Scalar(TabularScalar::String(text)) =
            field(summary, "first_stage_equations").unwrap()
        else {
            panic!("first-stage equation text")
        };
        let mut row_index = 0;
        for equation in equations.iter() {
            let RuntimeValue::List(names) = field(equation, "var_names").unwrap() else {
                panic!("first-stage labels")
            };
            for (index, name) in names.iter().enumerate() {
                let row = &rows[row_index];
                assert_eq!(
                    field(row, "equation").unwrap(),
                    field(equation, "endog_name").unwrap()
                );
                assert_eq!(field(row, "variable").unwrap(), name);
                for (display, source) in [
                    ("estimate", "betas"),
                    ("standard_error", "stds"),
                    ("statistic", "tvalues"),
                    ("p_value", "pvalues"),
                    ("ci_lower", "conf_int_left"),
                    ("ci_upper", "conf_int_right"),
                ] {
                    let RuntimeValue::List(values) = field(equation, source).unwrap() else {
                        panic!("first-stage coefficient values")
                    };
                    assert_eq!(field(row, display).unwrap(), &values[index]);
                }
                row_index += 1;
            }
            let RuntimeValue::Scalar(TabularScalar::String(response)) =
                field(equation, "endog_name").unwrap()
            else {
                panic!("first-stage response label")
            };
            assert!(text.contains(&format!("{response} = ")));
        }
        assert_eq!(rows.len(), row_index);
        let RuntimeValue::List(weak_rows) = field(summary, "weak_instrument_rows").unwrap() else {
            panic!("weak-instrument rows")
        };
        let statistics = field(first_stage, "statistics").unwrap();
        for name in ["min_eigenvalue", "partial_r2", "f_stat", "f_p_value"] {
            assert_eq!(
                field(&weak_rows[0], name).unwrap(),
                field(statistics, name).unwrap()
            );
        }
    };
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
                ("y", series(&y)),
                ("x", series(&x)),
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
        assert_eq!(field(&fit[0], "responseName").unwrap(), &string("response"));
        assert_eq!(
            field(&fit[0], "parameterNames").unwrap(),
            &RuntimeValue::List(vec![string("_cons"), string("x1"), string("x2")].into())
        );
        assert_eq!(
            field(&fit[0], "instrumentNames").unwrap(),
            &RuntimeValue::List(vec![string("z1"), string("z2")].into())
        );
        assert_eq!(field(&fit[0], "fitted").unwrap(), &fit[1]);
        assert_eq!(field(&fit[0], "residuals").unwrap(), &fit[2]);
        let mut options = vec![
            ("hypothesis_test", flag(true)),
            ("hypothesis", string("x1 = 0")),
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
        assert_first_stage_display(&summary[0]);
        assert_eq!(
            field(field(&summary[0], "hypothesisTest").unwrap(), "test_type").unwrap(),
            &string("z")
        );
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
        let mut malformed = fit[0].clone();
        if let RuntimeValue::Record(fields) = &mut malformed {
            Arc::make_mut(fields).insert(
                "parameterNames".into(),
                RuntimeValue::List(Vec::new().into()),
            );
        }
        assert!(
            run(
                &format!("yssbi.statistics.iv.{method}.summary"),
                &[("model", malformed)],
                &options,
                1
            )
            .is_err()
        );
        for (key, val) in &mut options {
            if !matches!(*key, "model_summary" | "coefficient_table" | "hypothesis") {
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

    // Orthogonal signal/noise frequencies give each endogenous variable its own
    // analytic Shea R², including when the dynamic input order changes.
    let n = 128;
    let angle = |row: usize| 2.0 * std::f64::consts::PI * row as f64 / n as f64;
    let signal = [2.0_f64, 1.5, 0.8];
    let noise = [1.0_f64, 0.7, 0.5];
    let x = (0..n).map(|i| (4.0 * angle(i)).cos()).collect::<Vec<_>>();
    let instruments = (1..=3)
        .map(|j| {
            (0..n)
                .map(|i| (j as f64 * angle(i)).sin())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let endogenous = (0..3)
        .map(|j| {
            (0..n)
                .map(|i| {
                    signal[j] * instruments[j][i]
                        + noise[j] * ((j + 1) as f64 * angle(i)).cos()
                        + 0.3 * (j + 1) as f64 * x[i]
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let y = (0..n)
        .map(|i| {
            1.0 + 0.7 * x[i] + 2.0 * endogenous[0][i] - 0.5 * endogenous[1][i]
                + 0.4 * endogenous[2][i]
                + (5.0 * angle(i)).cos()
        })
        .collect::<Vec<_>>();
    for method in ["2sls", "liml"] {
        for order in [[0, 1, 2], [2, 0, 1]] {
            let mut inputs = vec![("y", series(&y)), ("x", series(&x))];
            inputs.extend(
                order
                    .iter()
                    .map(|&j| ("endogenous", series(&endogenous[j]))),
            );
            inputs.extend(
                instruments
                    .iter()
                    .map(|values| ("instruments", series(values))),
            );
            let fit = run(
                &format!("yssbi.statistics.iv.{method}.fit"),
                &inputs,
                &[
                    ("constant", flag(true)),
                    ("covariance", string("nonrobust")),
                    ("small", flag(false)),
                ],
                3,
            )
            .unwrap();
            let mut options = vec![
                ("model_summary", flag(false)),
                ("coefficient_table", flag(false)),
                ("first_stage", flag(true)),
                ("overidentification", flag(false)),
                ("hypothesis_test", flag(false)),
                ("hypothesis", string("x1 = 0")),
            ];
            if method == "2sls" {
                options.push(("endogeneity", flag(false)));
            }
            let summary = run(
                &format!("yssbi.statistics.iv.{method}.summary"),
                &[("model", fit[0].clone())],
                &options,
                1,
            )
            .unwrap();
            assert_first_stage_display(&summary[0]);
            let first_stage = field(&summary[0], "firstStage").unwrap();
            let RuntimeValue::List(shea) =
                field(field(first_stage, "statistics").unwrap(), "shea_partial_r2").unwrap()
            else {
                panic!("Shea values")
            };
            let RuntimeValue::List(equations) = field(first_stage, "equations").unwrap() else {
                panic!("equations")
            };
            assert_eq!(shea.len(), order.len());
            for (position, &j) in order.iter().enumerate() {
                let expected = signal[j].powi(2) / (signal[j].powi(2) + noise[j].powi(2));
                let actual = super::super::numeric_input(Some(&shea[position])).unwrap();
                assert!(
                    (actual - expected).abs() < 1e-10,
                    "{method} endogenous {j}: {actual} != {expected}"
                );
                assert_eq!(
                    field(&equations[position], "endog_name").unwrap(),
                    &string(&format!("x{}", position + 2))
                );
            }
        }
    }
    let response = (1..=16).map(f64::from).collect::<Vec<_>>();
    let constant_column = vec![1.0; response.len()];
    let fit = run(
        "yssbi.statistics.iv.2sls.fit",
        &[
            ("y", series(&response)),
            ("endogenous", series(&constant_column)),
            ("instruments", series(&constant_column)),
        ],
        &[
            ("constant", flag(false)),
            ("covariance", string("nonrobust")),
            ("small", flag(false)),
        ],
        3,
    )
    .unwrap();
    let summary = run(
        "yssbi.statistics.iv.2sls.summary",
        &[("model", fit[0].clone())],
        &[
            ("model_summary", flag(false)),
            ("coefficient_table", flag(false)),
            ("first_stage", flag(true)),
            ("overidentification", flag(false)),
            ("endogeneity", flag(true)),
            ("hypothesis_test", flag(false)),
            ("hypothesis", string("x1 = 0")),
        ],
        1,
    )
    .unwrap();
    let statistics = field(field(&summary[0], "firstStage").unwrap(), "statistics").unwrap();
    assert_eq!(
        field(statistics, "r2_adjusted").unwrap(),
        &RuntimeValue::Scalar(TabularScalar::Null),
        "undefined centered adjusted R² is unavailable"
    );
    assert_eq!(
        field(field(&summary[0], "endogeneity").unwrap(), "hausman").unwrap(),
        &RuntimeValue::Scalar(TabularScalar::Null),
        "zero-rank Hausman is unavailable"
    );
    assert!(
        !matches!(
            field(field(&summary[0], "endogeneity").unwrap(), "endogenous").unwrap(),
            RuntimeValue::Scalar(TabularScalar::Null)
        ),
        "the independent endogenous test remains available"
    );
    assert!(matches!(
        run(
            "yssbi.statistics.diagnostic.hausman",
            &[("model", fit[0].clone())],
            &[],
            1,
        ),
        Err(KernelError::ScientificFailure)
    ));
    for (design, has_hausman) in [([1.0, 1.0], true), ([1.0, 0.0], false)] {
        let fit = run(
            "yssbi.statistics.iv.2sls.fit",
            &[
                ("y", series(&[1.0, 2.0])),
                ("endogenous", series(&design)),
                ("instruments", series(&design)),
            ],
            &[
                ("constant", flag(false)),
                ("covariance", string("nonrobust")),
                ("small", flag(false)),
            ],
            3,
        )
        .unwrap();
        let summary = run(
            "yssbi.statistics.iv.2sls.summary",
            &[("model", fit[0].clone())],
            &[
                ("model_summary", flag(false)),
                ("coefficient_table", flag(false)),
                ("first_stage", flag(false)),
                ("overidentification", flag(false)),
                ("endogeneity", flag(true)),
                ("hypothesis_test", flag(false)),
                ("hypothesis", string("x1 = 0")),
            ],
            1,
        )
        .unwrap();
        let endogeneity = field(&summary[0], "endogeneity").unwrap();
        assert_eq!(
            field(endogeneity, "endogenous").unwrap(),
            &RuntimeValue::Scalar(TabularScalar::Null),
            "zero denominator degrees of freedom makes the combined test unavailable"
        );
        assert_eq!(
            !matches!(
                field(endogeneity, "hausman").unwrap(),
                RuntimeValue::Scalar(TabularScalar::Null)
            ),
            has_hausman
        );
        if !has_hausman {
            assert_eq!(
                field(&summary[0], "endogeneityUnavailable").unwrap(),
                &string("insufficient_residual_variation_or_degrees_of_freedom")
            );
        }
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
        ("y", series(&y)),
        ("x", series(&x)),
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
                3,
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
    let fit = run(
        "yssbi.statistics.var.fit",
        &inputs,
        &[
            ("lags", int(1)),
            ("selected_lags", string("")),
            ("constant", flag(true)),
            ("dfk", flag(false)),
        ],
        1,
    )
    .unwrap();
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
    for extra_row in [false, true] {
        let mut malformed = fit[0].clone();
        let RuntimeValue::Record(fields) = &mut malformed else {
            panic!("expected fitted model");
        };
        let RuntimeValue::List(rows) = Arc::make_mut(fields).get_mut("sigma").unwrap() else {
            panic!("expected covariance matrix");
        };
        if extra_row {
            *rows = rows.iter().cloned().chain([rows[0].clone()]).collect();
        } else {
            let RuntimeValue::List(columns) = &mut Arc::make_mut(rows)[0] else {
                panic!("expected covariance row");
            };
            *columns = columns.iter().cloned().chain([number(0.0)]).collect();
        }
        assert!(matches!(
            run(
                "yssbi.statistics.var.summary",
                &[("model", malformed)],
                &[
                    ("model_summary", flag(true)),
                    ("coefficient_table", flag(false)),
                    ("lag_exclusion", flag(false)),
                    ("serial_tests", flag(false)),
                    ("stability", flag(false)),
                    ("serial_lags", int(2)),
                ],
                1,
            ),
            Err(KernelError::ScientificFailure)
        ));
    }
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
        let mut inputs = vec![("y", series(&y)), ("x", series(&x))];
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
            ("y", series(&y)),
            ("x", series(&x)),
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
            ("y", series(&y)),
            ("x", series(&x)),
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
    let density = run(
        "yssbi.plot.kde.view",
        &[("values", fit[2].clone())],
        &[("grid_points", int(256))],
        1,
    )
    .unwrap();
    let RuntimeValue::List(points) = field(&density[0], "data").unwrap() else {
        panic!()
    };
    assert_eq!(points.len(), 256);
    assert!(matches!(
        run(
            "yssbi.plot.kde.view",
            &[("values", series(&[-f64::MAX, f64::MAX]))],
            &[("grid_points", int(256))],
            1
        ),
        Err(KernelError::ScientificFailure)
    ));

    // Two predictors must not be mistaken for an intercept and one predictor.
    let other = x.iter().map(|v| v.sin()).collect::<Vec<_>>();
    let mut intercept_checks = Vec::new();
    for method in ["OLS", "WLS"] {
        let mut inputs = vec![("y", series(&y)), ("x", series(&x)), ("x", series(&other))];
        if method == "WLS" {
            inputs.push(("weights", series(&weights)));
        }
        let no_intercept = run(
            "yssbi.statistics.linear.fit",
            &inputs,
            &[
                ("method", string(method)),
                ("constant", flag(false)),
                ("covariance", string("nonrobust")),
            ],
            3,
        )
        .unwrap();
        for test in ["white", "information_matrix"] {
            let result = run(
                &format!("yssbi.statistics.diagnostic.{test}"),
                &[("model", no_intercept[0].clone())],
                &[],
                1,
            );
            intercept_checks.push((
                method,
                test,
                matches!(result, Err(KernelError::ScientificFailure)),
            ));
        }
        let summary = run(
            "yssbi.statistics.linear.summary",
            &[("model", no_intercept[0].clone())],
            &[
                ("equation", flag(false)),
                ("model_summary", flag(false)),
                ("anova", flag(false)),
                ("coefficient_table", flag(false)),
                ("coefficient_chart", flag(false)),
                ("diagnostics", flag(true)),
                ("residual_plot", flag(false)),
                ("observations", flag(false)),
                ("acf_pacf", flag(false)),
                ("serial_tests", flag(false)),
                ("hypothesis_test", flag(false)),
            ],
            1,
        )
        .unwrap();
        let RuntimeValue::LinearRegression(model) = &summary[0] else {
            panic!("summary must retain the fitted model")
        };
        let diagnostics = model
            .summary
            .as_ref()
            .unwrap()
            .diagnostics
            .as_ref()
            .unwrap();
        for name in ["White", "Information matrix"] {
            let entry = diagnostics
                .tests
                .iter()
                .find(|entry| entry.name == name)
                .unwrap();
            intercept_checks.push((
                method,
                name,
                entry.value.is_none() && entry.unavailable_reason.is_some(),
            ));
        }
    }
    assert!(
        intercept_checks.iter().all(|(_, _, rejected)| *rejected),
        "White/IM require the fitted intercept: {intercept_checks:?}"
    );
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
        ("y", series(&y)),
        ("entity", series(&entity)),
        ("time", series(&time)),
        ("treat", series(&treat)),
        ("post", series(&post)),
    ];
    let parameters = [
        ("repetitions", int(20)),
        ("seed", int(42)),
        ("constant", flag(true)),
        ("covariance", string("cluster")),
        ("use_observed_coefficient", flag(false)),
        ("observed_coefficient", number(0.)),
    ];
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
    let mut no_constant = parameters.clone();
    for (name, value) in &mut no_constant {
        if *name == "constant" {
            *value = flag(false);
        }
    }
    let no_constant = run(
        "yssbi.statistics.panel.did.randomization",
        &inputs,
        &no_constant,
        1,
    )
    .unwrap();
    assert_eq!(field(&no_constant[0], "available").unwrap(), &flag(true));
    assert_eq!(
        field(&no_constant[0], "observed_coef").unwrap(),
        field(&first[0], "observed_coef").unwrap()
    );
    let did = run(
        "yssbi.statistics.panel.did.twfe",
        &[
            ("y", series(&y)),
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

    let zero_response = vec![0.0; n];
    let error = yss_sci_runtime::causal::did::randomization_test(
        yss_sci_contract::causal::did::DidRandomizationInput {
            constant: true,
            covariance: "nonrobust".into(),
            observed_coefficient: None,
            response: zero_response.clone(),
            predictors: vec![],
            entity: entity.clone(),
            time: time.clone(),
            treat: treat.clone(),
            post: post.clone(),
            repetitions: 20,
            seed: 42,
        },
        &yss_sci_contract::execution::ScientificExecutionControl {
            cancellation: Default::default(),
            deadline: Instant::now() + Duration::from_secs(5),
        },
    )
    .unwrap_err();
    assert!(matches!(
        error,
        yss_sci_contract::causal::did::DidFakeGroupError::FitFailed { diagnostic }
            if diagnostic == "OLS coefficient t-statistic is undefined"
    ));
    let mut undefined_inputs = inputs.clone();
    undefined_inputs[0].1 = series(&zero_response);
    let mut nonrobust_parameters = parameters.clone();
    nonrobust_parameters[3].1 = string("nonrobust");
    assert!(matches!(
        run(
            "yssbi.statistics.panel.did.randomization",
            &undefined_inputs,
            &nonrobust_parameters,
            1,
        ),
        Err(KernelError::ScientificFailure)
    ));
}

// Two separate boundary regressions: estimator-scale panel prediction and partial
// comparison failure; selected-lag/exogenous VAR and multi-series ADF projection.
#[test]
fn panel_estimation_predictions_and_comparison_failures_are_explicit() {
    let n = 60;
    let e = noise(120);
    let entity = (0..n).map(|i| (i / 6) as f64).collect::<Vec<_>>();
    let time = (0..n).map(|i| (i % 6) as f64).collect::<Vec<_>>();
    let x = e[..n].to_vec();
    let y = (0..n)
        .map(|i| 1.0 + entity[i] + 0.4 * time[i] + 2.0 * x[i] + e[n + i] * 0.2)
        .collect::<Vec<_>>();
    let inputs = [
        ("y", series(&y)),
        ("x", series(&x)),
        ("entity", series(&entity)),
        ("time", series(&time)),
    ];
    let fit = run(
        "yssbi.statistics.panel.fit",
        &inputs,
        &[
            ("constant", flag(true)),
            ("estimator", string("fixed_effects")),
            ("effects", string("two_way")),
            ("covariance", string("nonrobust")),
        ],
        3,
    )
    .unwrap();
    let estimation = field(&fit[0], "estimation").unwrap();
    assert_eq!(field(estimation, "space").unwrap(), &string("within"));
    let RuntimeValue::List(design) = field(estimation, "design").unwrap() else {
        panic!("design");
    };
    let prediction = run(
        "yssbi.statistics.panel.predict",
        &[("model", fit[0].clone()), ("x", design[0].clone())],
        &[],
        1,
    )
    .unwrap();
    let RuntimeValue::List(actual) = &prediction[0] else {
        panic!()
    };
    let RuntimeValue::List(expected) = &fit[1] else {
        panic!()
    };
    for (a, b) in actual.iter().zip(expected.iter()) {
        assert!(
            (crate::builtins::numeric_input(Some(a)).unwrap()
                - crate::builtins::numeric_input(Some(b)).unwrap())
            .abs()
                < 1e-12
        );
    }
    let comparison = run(
        "yssbi.statistics.panel.compare",
        &inputs,
        &[
            ("constant", flag(true)),
            ("estimators", string("fixed_effects,between")),
            ("effects", string("two_way")),
            ("covariance", string("nonrobust")),
        ],
        1,
    )
    .unwrap();
    let RuntimeValue::List(statuses) = field(&comparison[0], "model_statuses").unwrap() else {
        panic!()
    };
    assert_eq!(field(&statuses[0], "status").unwrap(), &string("success"));
    assert_eq!(field(&statuses[1], "status").unwrap(), &string("failed"));
    assert!(field(&statuses[1], "failure").is_ok());

    // Finite observations can still produce undefined model statistics. Keep the
    // typed result visible to validation before JSON would turn them into null.
    let predictor = vec![-1.0, -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0];
    let response = predictor.iter().map(|x| 1.0 + 2.0 * x).collect::<Vec<_>>();
    let entity = (0..8).map(|i| (i / 2) as f64).collect::<Vec<_>>();
    let time = (0..8).map(|i| (i % 2) as f64).collect::<Vec<_>>();
    let undefined = yss_sci_runtime::panel::fit_model(
        response.clone(),
        vec![predictor.clone()],
        entity.clone(),
        time.clone(),
        yss_sci_contract::panel::PanelOptions {
            estimator: yss_sci_contract::panel::PanelEstimator::Between,
            effects: yss_sci_contract::panel::PanelEffects::Entity,
            constant: true,
            covariance: "nonrobust".into(),
        },
    )
    .unwrap();
    assert!(undefined.statistics.linear.f_statistic.is_infinite());
    let inputs = [
        ("y", series(&response)),
        ("x", series(&predictor)),
        ("entity", series(&entity)),
        ("time", series(&time)),
    ];
    for (id, estimator_key, output_count) in [
        ("yssbi.statistics.panel.fit", "estimator", 3),
        ("yssbi.statistics.panel.compare", "estimators", 1),
    ] {
        let result = run(
            id,
            &inputs,
            &[
                ("constant", flag(true)),
                (estimator_key, string("between")),
                ("effects", string("entity")),
                ("covariance", string("nonrobust")),
            ],
            output_count,
        );
        assert!(
            matches!(result, Err(KernelError::NonFiniteResult)),
            "{id}: {result:?}"
        );
    }

    // Undefined 0/0 coefficient inference must fail through the scientific
    // boundary rather than pass NaN to the distribution implementation.
    let zero_response = vec![0.0; 8];
    let predictor = (0..8).map(|i| i as f64).collect::<Vec<_>>();
    let result = run(
        "yssbi.statistics.panel.fit",
        &[
            ("y", series(&zero_response)),
            ("x", series(&predictor)),
            ("entity", series(&entity)),
            ("time", series(&time)),
        ],
        &[
            ("constant", flag(true)),
            ("estimator", string("between")),
            ("effects", string("entity")),
            ("covariance", string("nonrobust")),
        ],
        3,
    );
    assert!(
        matches!(result, Err(KernelError::ScientificFailure)),
        "{result:?}"
    );
}
#[test]
fn selected_lag_var_and_adf_collections_keep_real_sample_and_failure_information() {
    let n = 100;
    let e = noise(300);
    let x = e[..n].to_vec();
    let y = e[n..2 * n].to_vec();
    let z = e[2 * n..].to_vec();
    let fit = run(
        "yssbi.statistics.var.fit",
        &[
            ("variables", series(&x)),
            ("variables", series(&y)),
            ("exogenous", series(&z)),
        ],
        &[
            ("lags", int(1)),
            ("selected_lags", string("1,3")),
            ("constant", flag(false)),
            ("dfk", flag(true)),
        ],
        1,
    )
    .unwrap();
    assert_eq!(
        field(&fit[0], "lags").unwrap(),
        &RuntimeValue::List(vec![int(1), int(3)].into())
    );
    assert_eq!(field(&fit[0], "constant").unwrap(), &flag(false));
    assert_eq!(field(&fit[0], "dfk").unwrap(), &flag(true));
    let RuntimeValue::List(rows) = field(&fit[0], "sample_rows").unwrap() else {
        panic!()
    };
    assert_eq!(rows.len(), 97);
    assert_eq!(rows[0], int(3));
    let adf = run(
        "yssbi.statistics.adf.test",
        &[("series", series(&x)), ("series", series(&vec![1.; n]))],
        &[("lags", int(1)), ("regression", string("constant"))],
        1,
    )
    .unwrap();
    let RuntimeValue::List(rows) = field(&adf[0], "test_rows").unwrap() else {
        panic!()
    };
    assert_eq!(rows.len(), 2);
    assert_eq!(field(&rows[0], "status").unwrap(), &string("success"));
    assert_eq!(field(&rows[1], "status").unwrap(), &string("failed"));
    let RuntimeValue::List(tests) = field(&adf[0], "tests").unwrap() else {
        panic!()
    };
    assert_eq!(field(&tests[0], "series").unwrap(), &string("series1"));
    assert_eq!(field(&tests[0], "seriesIndex").unwrap(), &int(0));
    assert!(field(&adf[0], "report_display").is_ok());

    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../yss-sci/tests/fixtures/panel_category_reference.json"
    ))
    .unwrap();
    let data = &fixture["nonstationary"];
    let response: Vec<f64> = serde_json::from_value(data["response"].clone()).unwrap();
    let entity: Vec<f64> = serde_json::from_value(data["entity"].clone()).unwrap();
    let input = response
        .iter()
        .zip(&entity)
        .filter_map(|(&value, &group)| (group == 0.0).then_some(value))
        .collect::<Vec<_>>();
    for case in data["cases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|case| case["method"] == "unit_root" && case["regression"] != "constant")
    {
        let regression = case["regression"].as_str().unwrap();
        let result = run(
            "yssbi.statistics.adf.test",
            &[("series", series(&input))],
            &[
                ("lags", int(case["lags"].as_i64().unwrap())),
                ("regression", string(regression)),
            ],
            1,
        )
        .unwrap();
        for (actual_key, reference_key) in [("statistic", "statistic"), ("pValue", "p_value")] {
            let actual =
                super::super::numeric_input(Some(field(&result[0], actual_key).unwrap())).unwrap();
            let reference = case["entity_tests"][0][reference_key].as_f64().unwrap();
            assert!(
                (actual - reference).abs() < 1e-7 * (1.0 + reference.abs()),
                "{regression} {actual_key}: {actual} != {reference}"
            );
        }
        assert_eq!(field(&result[0], "useTDistribution").unwrap(), &flag(false));
        assert_eq!(
            field(&result[0], "regression").unwrap(),
            &string(regression)
        );
    }
}

// Validate real emitted descriptor data, including serde field renames, instead
// of accepting a template whose table columns do not exist in its payload.
fn validate_display(root: &RuntimeValue) {
    let Ok(display) = field(root, "report_display") else {
        return;
    };
    let RuntimeValue::Record(sections) = field(display, "sections").unwrap() else {
        panic!("sections")
    };
    for (id, section) in sections.iter() {
        let RuntimeValue::Scalar(TabularScalar::String(path)) = field(section, "path").unwrap()
        else {
            panic!("path")
        };
        let target = path.split('/').skip(1).fold(root, |value, key| {
            field(value, &key.replace("~1", "/").replace("~0", "~"))
                .unwrap_or_else(|_| panic!("missing {id}: {path}"))
        });
        let RuntimeValue::Scalar(TabularScalar::String(kind)) = field(section, "kind").unwrap()
        else {
            panic!("kind")
        };
        if kind.as_ref() == "equation" {
            assert!(matches!(
                target,
                RuntimeValue::Scalar(TabularScalar::String(_))
            ));
            continue;
        }
        let RuntimeValue::List(rows) = target else {
            panic!("{id} is not a table")
        };
        let RuntimeValue::Record(columns) = field(section, "columns").unwrap() else {
            panic!("columns")
        };
        for row in rows.iter() {
            for name in columns.keys() {
                assert!(
                    matches!(field(row, name), Ok(RuntimeValue::Scalar(_))),
                    "{id}: missing/scalar column {name}"
                );
            }
        }
    }
}

#[test]
fn scale_limits_gee_accepts_large_full_rank_design_and_singleton_clusters() {
    let n = 1024usize;
    let x = (1..=70)
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
            3.0 + 0.25 * x[0][i]
                + 0.1 * x[1][i]
                + if (i & 127).count_ones().is_multiple_of(2) {
                    0.2
                } else {
                    -0.2
                }
        })
        .collect::<Vec<_>>();
    let mut inputs = vec![("y", series(&y))];
    inputs.extend(x.iter().map(|v| ("x", series(v))));
    inputs.push((
        "groups",
        RuntimeValue::List((0..n).map(|i| int(i as i64)).collect()),
    ));
    let result = run(
        "yssbi.statistics.longitudinal.gee",
        &inputs,
        &[
            ("constant", flag(true)),
            ("max_iterations", int(20000)),
            ("tolerance", number(1e-7)),
            ("longitudinal_family", string("gaussian")),
            ("working_correlation", string("independence")),
        ],
        1,
    )
    .unwrap();
    let report = &result[0];
    assert_eq!(field(report, "observations").unwrap(), &int(n as i64));
    let RuntimeValue::List(coefficients) = field(report, "coefficients").unwrap() else {
        panic!("coefficients")
    };
    assert_eq!(coefficients.len(), 71);
    for (row, expected) in coefficients.iter().zip([3.0, 0.25, 0.1]) {
        let actual = super::super::numeric_input(Some(field(row, "estimate").unwrap())).unwrap();
        assert!((actual - expected).abs() < 1e-8);
    }
}

#[test]
fn scale_limits_mixed_estimators_preserve_replicated_reference_fits() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../yss-sci/src/longitudinal/fixtures/reference.json"
    ))
    .unwrap();
    let x: Vec<f64> = serde_json::from_value(fixture["x"][0].clone()).unwrap();
    let groups: Vec<i64> = serde_json::from_value(fixture["groups"].clone()).unwrap();
    for (id, case, copies) in [
        ("mixed.poisson", "glmm_poisson", 8),
        ("mixed.random_intercept", "lmm_ml", 4),
    ] {
        let y: Vec<f64> = serde_json::from_value(fixture["cases"][case]["y"].clone()).unwrap();
        let labels = RuntimeValue::List(
            (0..copies)
                .flat_map(|copy| groups.iter().map(move |g| int(g + 18 * copy as i64)))
                .collect(),
        );
        let mut parameters = vec![
            ("constant", flag(true)),
            ("max_iterations", int(20000)),
            ("tolerance", number(1e-7)),
        ];
        if case == "lmm_ml" {
            parameters.push(("mixed_estimation", string("ml")));
        }
        let result = run(
            &format!("yssbi.statistics.{id}"),
            &[
                ("y", series(&y.repeat(copies))),
                ("x", series(&x.repeat(copies))),
                ("groups", labels),
            ],
            &parameters,
            1,
        )
        .unwrap();
        let report = &result[0];
        assert_eq!(
            field(report, "observations").unwrap(),
            &int((y.len() * copies) as i64)
        );
        let RuntimeValue::List(coefficients) = field(report, "coefficients").unwrap() else {
            panic!("coefficients")
        };
        for (i, row) in coefficients.iter().enumerate() {
            let actual =
                super::super::numeric_input(Some(field(row, "estimate").unwrap())).unwrap();
            let expected = fixture["cases"][case]["coefficients"][i].as_f64().unwrap();
            assert!(
                (actual - expected).abs() < 5e-4,
                "{id}: {actual} != {expected}"
            );
        }
    }
}

#[test]
fn scale_limits_classification_and_classical_groups_keep_valid_results() {
    for (id, groups, expected_p) in [
        (
            "yssbi.statistics.test.levene",
            [
                [-1.125, -0.875, 0.875, 1.125],
                [
                    -1_000_000_000_000.125,
                    -999_999_999_999.875,
                    999_999_999_999.875,
                    1_000_000_000_000.125,
                ],
            ],
            7.629_394_531_295_777e-77,
        ),
        (
            "yssbi.statistics.test.brown_forsythe",
            [
                [-1.125, -0.875, 0.875, 1.125],
                [
                    -1_000_000_000_000.125,
                    -999_999_999_999.875,
                    999_999_999_999.875,
                    1_000_000_000_000.125,
                ],
            ],
            7.629_394_531_295_777e-77,
        ),
        (
            "yssbi.statistics.test.bartlett",
            [[1.0, 2.0, 3.0, 4.0], [1e8, 2e8, 3e8, 4e8]],
            1.318_325_745_654_562_2e-21,
        ),
    ] {
        let inputs = groups
            .iter()
            .map(|group| ("groups", series(group)))
            .collect::<Vec<_>>();
        let report = run(id, &inputs, &[], 1).unwrap();
        let p = super::super::numeric_input(Some(field(&report[0], "p_value").unwrap())).unwrap();
        assert!(
            (p / expected_p - 1.0).abs() < 1e-12,
            "{id} lost its upper tail: p={p}"
        );
    }
    let median_groups = [
        ("groups", series(&[1.0, 2.0, 3.0])),
        ("groups", series(&[4.0, 5.0, 6.0])),
        ("groups", series(&[7.0, 8.0, 9.0])),
    ];
    for (id, parameters) in [
        ("yssbi.statistics.test.mood_median", vec![]),
        (
            "yssbi.statistics.test.nonparametric.family",
            vec![("method", string("mood_median"))],
        ),
    ] {
        let report = run(id, &median_groups, &parameters, 1).unwrap();
        let statistic =
            super::super::numeric_input(Some(field(&report[0], "statistic").unwrap())).unwrap();
        let p = super::super::numeric_input(Some(field(&report[0], "p_value").unwrap())).unwrap();
        assert!(
            (statistic - 6.0).abs() < 1e-12,
            "{id} lost group margins: statistic={statistic}, p={p}"
        );
        assert!((p - 0.049_787_068_367_863_944).abs() < 1e-12);
        let report = run(
            id,
            &[
                ("groups", series(&[f64::MAX / 2.0; 4])),
                ("groups", series(&[f64::MAX; 4])),
            ],
            &parameters,
            1,
        )
        .expect("finite observations must not overflow the pooled median");
        let statistic =
            super::super::numeric_input(Some(field(&report[0], "statistic").unwrap())).unwrap();
        let p = super::super::numeric_input(Some(field(&report[0], "p_value").unwrap())).unwrap();
        assert_eq!(statistic, 8.0);
        assert!((p - 0.004_677_734_981_047_265).abs() < 1e-14, "{id}");
    }
    let result = run(
        "yssbi.statistics.test.proportion.multiple",
        &[(
            "successes_and_trials",
            series(&[20.0, 100.0, 40.0, 100.0, 60.0, 100.0]),
        )],
        &[],
        1,
    )
    .unwrap();
    let statistic =
        super::super::numeric_input(Some(field(&result[0], "statistic").unwrap())).unwrap();
    let p = super::super::numeric_input(Some(field(&result[0], "p_value").unwrap())).unwrap();
    assert!(
        (statistic - 100.0 / 3.0).abs() < 1e-12,
        "multiple-proportion adapter returned {statistic}, p={p}"
    );
    assert!((p - 5.777_748_519_419_133e-8).abs() < 1e-15);

    for (id, parameter, null, expected_statistic, expected_p) in [
        (
            "yssbi.statistics.test.t.one_sample",
            "null_mean",
            2.0,
            0.0,
            1.0,
        ),
        (
            "yssbi.statistics.test.wilcoxon.one_sample",
            "null_median",
            0.0,
            6.0 / 14.0_f64.sqrt(),
            0.25,
        ),
    ] {
        let result = run(
            id,
            &[("series", series(&[1.0, 2.0, 3.0]))],
            &[
                (parameter, number(null)),
                ("alternative", string("two_sided")),
            ],
            1,
        )
        .unwrap();
        let statistic =
            super::super::numeric_input(Some(field(&result[0], "statistic").unwrap())).unwrap();
        let p = super::super::numeric_input(Some(field(&result[0], "p_value").unwrap())).unwrap();
        assert!((statistic - expected_statistic).abs() < 1e-12, "{id}");
        assert!((p - expected_p).abs() < 1e-12, "{id}");
    }

    let first = (0..600).map(|i| (i % 300) as f64).collect::<Vec<_>>();
    let second = (0..600)
        .map(|i| {
            if i % 10 == 0 {
                ((i + 1) % 300) as f64
            } else {
                first[i]
            }
        })
        .collect::<Vec<_>>();
    let result = run(
        "yssbi.statistics.test.kappa",
        &[("ratings", series(&first)), ("ratings", series(&second))],
        &[
            ("kappa_method", string("cohen")),
            ("kappa_weighting", string("none")),
            ("confidence_level", number(0.95)),
        ],
        1,
    )
    .unwrap();
    let RuntimeValue::List(categories) = field(&result[0], "categories").unwrap() else {
        panic!("categories")
    };
    assert_eq!(categories.len(), 300);
    let observed =
        super::super::numeric_input(Some(field(&result[0], "observed_agreement").unwrap()))
            .unwrap();
    assert!((observed - 0.9).abs() < 1e-12);
    let groups = (0..17)
        .map(|i| {
            (
                "groups",
                series(&[i as f64, i as f64 + 1.0, i as f64 + 2.0]),
            )
        })
        .collect::<Vec<_>>();
    assert!(run("yssbi.statistics.test.bartlett", &groups, &[], 1).is_ok());
}
