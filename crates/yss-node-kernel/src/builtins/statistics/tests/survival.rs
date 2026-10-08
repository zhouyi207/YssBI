use super::*;

#[test]
fn survival_predictions_from_mixed_inputs_retain_pairing_for_evaluation() {
    use arrow_array::{ArrayRef, Float64Array, RecordBatch};
    use yss_database_engine::DataFusionRuntime;
    use yss_relational_contract::{RelationControl, RelationFactory};

    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../yss-sci/tests/fixtures/survival_category_reference.json"
    ))
    .unwrap();
    let data = &fixture["data"];
    let time: Vec<f64> = serde_json::from_value(data["time"].clone()).unwrap();
    let event: Vec<f64> = serde_json::from_value(data["event"].clone()).unwrap();
    let predictors: Vec<Vec<f64>> = serde_json::from_value(data["predictors"].clone()).unwrap();
    let factory: Arc<dyn RelationFactory> = DataFusionRuntime::unbounded(32).unwrap();
    let control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    );
    let relation_control = RelationControl {
        cancellation: control.cancellation.clone(),
        deadline: control.deadline,
        max_input_bytes: control.max_input_bytes,
    };
    let source = factory
        .clone()
        .materialize(
            RecordBatch::try_from_iter([
                (
                    "age",
                    Arc::new(Float64Array::from(predictors[0].clone())) as ArrayRef,
                ),
                (
                    "marker",
                    Arc::new(Float64Array::from(predictors[1].clone())) as ArrayRef,
                ),
            ])
            .unwrap(),
            &relation_control,
        )
        .unwrap();
    let values = [
        series(&time),
        RuntimeValue::List(event.iter().map(|&v| flag(v == 1.0)).collect()),
        RuntimeValue::Series(source.select_series("age").unwrap()),
        RuntimeValue::Series(source.select_series("marker").unwrap()),
    ];
    let outputs = [
        KernelOutputSpec {
            data_type: ValueType::Struct("statistics.report".into()),
            fields: None,
        },
        KernelOutputSpec {
            data_type: ValueType::DataFrame,
            fields: Some(
                ["time", "event", "risk"]
                    .iter()
                    .map(|name| KernelField {
                        name: (*name).into(),
                        data_type: ValueType::number(),
                    })
                    .collect(),
            ),
        },
    ];
    let registry = KernelRegistry::default();
    for (method, expected) in [
        ("survival.cox", &fixture["cox"]["efron"]["risk"]),
        ("survival.weibull", &fixture["aft"]["weibull"]["risk"]),
    ] {
        let mut parameters = vec![
            ("max_iterations", int(1000)),
            ("tolerance", number(1e-8)),
            ("survival_horizon", number(2.0)),
        ];
        if method == "survival.cox" {
            parameters.push(("survival_ties", string("efron")));
        }
        let result = registry
            .execute(
                &KernelId::new(format!("yssbi.statistics.{method}").into()).unwrap(),
                &KernelInvocation {
                    relations: &factory,
                    inputs: &values,
                    input_keys: &["time", "event", "x", "x"],
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
            .unwrap();
        let RuntimeValue::Relation(predictions) = &result[1] else {
            panic!("predictions must retain a relation")
        };
        let page = predictions.page(0, time.len(), &relation_control).unwrap();
        for (row, &time) in time.iter().enumerate() {
            for (column, expected) in [
                (0, time),
                (1, event[row]),
                (2, expected[row].as_f64().unwrap()),
            ] {
                let value = &page.data.columns()[column].values()[row];
                let actual =
                    crate::builtins::numeric_input(Some(&RuntimeValue::Scalar(value.clone())))
                        .unwrap();
                assert!(
                    (actual - expected).abs() < 1e-6,
                    "{method}: row {row}, column {column}: {actual} != {expected}"
                );
            }
        }
        let inputs = ["time", "event", "risk"]
            .iter()
            .map(|name| RuntimeValue::Series(predictions.select_series(name).unwrap()))
            .collect::<Vec<_>>();
        let parameters = [
            ("survival_horizon", number(2.0)),
            ("calibration_bins", int(4)),
        ];
        let calibrated = registry
            .execute(
                &KernelId::new("yssbi.statistics.plot.calibration".into()).unwrap(),
                &KernelInvocation {
                    relations: &factory,
                    inputs: &inputs,
                    input_keys: &["time", "event", "predicted_risk"],
                    parameters: parameters
                        .iter()
                        .map(|(k, v)| {
                            (
                                KernelParameterKey::new((*k).into()).unwrap(),
                                Cow::Borrowed(v),
                            )
                        })
                        .collect(),
                    outputs: &outputs[..1],
                    control: &control,
                },
            )
            .unwrap();
        let RuntimeValue::List(bins) = field(&calibrated[0], "bins").unwrap() else {
            panic!("calibration bins must be a list")
        };
        assert_eq!(bins.len(), 4);
    }
}

#[test]
fn survival_workspace_distinguishes_group_metadata_from_matrix_dimensions() {
    let relations = crate::tests::relations();
    let registry = KernelRegistry::default();
    let outputs = [KernelOutputSpec {
        data_type: ValueType::Struct("statistics.report".into()),
        fields: None,
    }];
    let mut control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    );
    let n = 300;
    let mut values = vec![
        series(&vec![1.0; n]),
        series(&vec![1.0; n]),
        RuntimeValue::List((0..n).map(|i| int(i as i64)).collect()),
    ];
    for budget in [8 * 1024 * 1024, 1024 * 1024] {
        control.max_input_bytes = budget;
        let inv = KernelInvocation {
            relations: &relations,
            inputs: &values,
            input_keys: &["time", "event", "groups"],
            parameters: Default::default(),
            outputs: &outputs,
            control: &control,
        };
        super::super::common::materialize(&inv).unwrap();
        let result = registry.execute(
            &KernelId::new("yssbi.statistics.survival.kaplan_meier".into()).unwrap(),
            &inv,
        );
        if budget == 8 * 1024 * 1024 {
            let values = result.unwrap();
            let RuntimeValue::List(curves) = field(&values[0], "curves").unwrap() else {
                panic!("curves must be a list")
            };
            assert_eq!(curves.len(), n);
            // Log-rank retains a real group covariance; its quadratic workspace still exceeds this budget.
            assert!(matches!(
                registry.execute(
                    &KernelId::new("yssbi.statistics.survival.logrank".into()).unwrap(),
                    &inv
                ),
                Err(KernelError::BudgetExceeded)
            ));
        } else {
            assert!(
                matches!(result, Err(KernelError::BudgetExceeded)),
                "{result:?}"
            );
        }
    }
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../yss-sci/tests/fixtures/survival_category_reference.json"
    ))
    .unwrap();
    let data = &fixture["time_dependent"];
    values = ["start", "stop", "event", "subjects"]
        .iter()
        .map(|key| series(&serde_json::from_value::<Vec<f64>>(data[key].clone()).unwrap()))
        .collect();
    for x in data["predictors"].as_array().unwrap() {
        values.push(series(
            &serde_json::from_value::<Vec<f64>>(x.clone()).unwrap(),
        ));
    }
    control.max_input_bytes = 8 * 1024 * 1024;
    let parameters = [
        ("survival_ties", string("efron")),
        ("max_iterations", int(1000)),
        ("tolerance", number(1e-8)),
    ];
    let inv = KernelInvocation {
        relations: &relations,
        inputs: &values,
        input_keys: &["start", "stop", "event", "subjects", "x", "x"],
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
    let result = registry
        .execute(
            &KernelId::new("yssbi.statistics.survival.time_dependent_cox".into()).unwrap(),
            &inv,
        )
        .unwrap();
    assert_eq!(field(&result[0], "observations").unwrap(), &int(240));
    let RuntimeValue::List(coefficients) = field(&result[0], "coefficients").unwrap() else {
        panic!("coefficients must be a list")
    };
    assert_eq!(coefficients.len(), 2);
}

#[test]
fn survival_admission_distinguishes_observations_from_options() {
    let error = run(
        "yssbi.statistics.survival.kaplan_meier",
        &[("time", series(&[0., 2.])), ("event", series(&[1., 0.]))],
        &[],
        1,
    )
    .unwrap_err();
    assert!(
        matches!(error, KernelError::InvalidNumericInput),
        "{error:?}"
    );
    for (predicted, bins, parameter_error) in
        [(vec![0.2, 1.2], 2, false), (vec![0.2, 0.8], 1, true)]
    {
        let error = run(
            "yssbi.statistics.plot.calibration",
            &[
                ("time", series(&[1., 2.])),
                ("event", series(&[1., 0.])),
                ("predicted_risk", series(&predicted)),
            ],
            &[
                ("survival_horizon", number(1.)),
                ("calibration_bins", int(bins)),
            ],
            1,
        )
        .unwrap_err();
        if parameter_error {
            assert!(matches!(error, KernelError::InvalidParameter), "{error:?}");
        } else {
            assert!(
                matches!(error, KernelError::InvalidNumericInput),
                "{error:?}"
            );
        }
    }
}

#[test]
fn survival_adapters_preserve_binary_events_exact_groups_and_cause_codes() {
    let inputs = [
        ("time", series(&[1.0, 2.0, 3.0, 4.0])),
        (
            "event",
            RuntimeValue::List(vec![flag(true), flag(false), flag(true), flag(false)].into()),
        ),
        (
            "groups",
            RuntimeValue::List(
                vec![
                    int(9_007_199_254_740_992),
                    int(9_007_199_254_740_993),
                    int(9_007_199_254_740_992),
                    int(9_007_199_254_740_993),
                ]
                .into(),
            ),
        ),
    ];
    let r = run("yssbi.statistics.survival.kaplan_meier", &inputs, &[], 1)
        .unwrap()
        .remove(0);
    let RuntimeValue::List(curves) = field(&r, "curves").unwrap() else {
        panic!("curves")
    };
    assert_eq!(curves.len(), 2);
    assert_eq!(
        field(&curves[0], "group").unwrap(),
        &int(9_007_199_254_740_992)
    );
    assert_eq!(
        field(&curves[1], "group").unwrap(),
        &int(9_007_199_254_740_993)
    );
    let result = run(
        "yssbi.statistics.survival.competing_risks",
        &[
            inputs[0].clone(),
            (
                "status",
                RuntimeValue::List(
                    vec![
                        int(9_007_199_254_740_992),
                        int(0),
                        int(9_007_199_254_740_993),
                        int(0),
                    ]
                    .into(),
                ),
            ),
        ],
        &[],
        1,
    )
    .unwrap()
    .remove(0);
    assert_eq!(
        field(&result, "causes").unwrap(),
        &RuntimeValue::List(vec![int(9_007_199_254_740_992), int(9_007_199_254_740_993)].into())
    );
    assert!(matches!(
        run(
            "yssbi.statistics.survival.logrank",
            &[
                ("time", series(&[1.0, 2.0])),
                ("event", series(&[1.0])),
                ("groups", series(&[0.0, 1.0]))
            ],
            &[],
            1
        ),
        Err(KernelError::ShapeMismatch)
    ));
}

#[test]
fn survival_plot_workspace_and_missing_observations_fail_before_computation() {
    let mut inputs = vec![
        ("time", series(&[1.0, 2.0, 3.0])),
        ("event", series(&[1.0, 1.0, 0.0])),
        ("predicted_risk", series(&[0.2, 0.4, 0.6])),
    ];
    let mut params = vec![
        ("survival_horizon", number(1.0)),
        ("decision_threshold_min", number(0.01)),
        ("decision_threshold_max", number(0.99)),
        ("decision_points", int(i64::MAX)),
    ];
    assert!(matches!(
        run("yssbi.statistics.plot.decision_curve", &inputs, &params, 1),
        Err(KernelError::BudgetExceeded)
    ));
    params[3].1 = int(99);
    inputs[1].1 =
        RuntimeValue::List(vec![number(1.0), TabularScalar::Null.into(), number(0.0)].into());
    assert!(run("yssbi.statistics.plot.decision_curve", &inputs, &params, 1).is_err());
    assert!(matches!(
        run(
            "yssbi.statistics.plot.nomogram",
            &[("model", series(&[1.0]))],
            &[
                ("survival_horizon", number(1.0)),
                ("nomogram_ticks", int(5))
            ],
            1
        ),
        Err(KernelError::InvalidNumericInput)
    ));
}
