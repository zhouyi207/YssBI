use super::*;

fn iv_summary_parameters(kind: &str, first_stage: bool) -> Vec<(&'static str, RuntimeValue)> {
    let mut parameters = vec![
        ("model_summary", flag(true)),
        ("coefficient_table", flag(false)),
        ("first_stage", flag(first_stage)),
        ("overidentification", flag(false)),
        ("hypothesis_test", flag(false)),
        ("hypothesis", string("x1 = 0")),
    ];
    if kind == "2sls" {
        parameters.push(("endogeneity", flag(false)));
    }
    parameters
}

#[test]
fn iv_first_stage_nodes_preserve_unavailable_inference_and_generalized_eigenvalues() {
    let n = 6;
    let mut state = 1_u64;
    let mut random = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        (state >> 32) as u32 as f64 / u32::MAX as f64 - 0.5
    };
    let instruments = (0..n - 1)
        .map(|_| (0..n).map(|_| random()).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let endogenous = (0..2)
        .map(|_| (0..n).map(|_| random()).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let response = (0..n)
        .map(|i| 1.0 + endogenous[0][i] + 0.7 * endogenous[1][i] + random())
        .collect::<Vec<_>>();
    let mut inputs = vec![("y", series(&response))];
    inputs.extend(endogenous.iter().map(|v| ("endogenous", series(v))));
    inputs.extend(instruments.iter().map(|v| ("instruments", series(v))));
    let fit = run(
        "yssbi.statistics.iv.2sls.fit",
        &inputs,
        &[
            ("constant", flag(true)),
            ("covariance", string("nonrobust")),
            ("small", flag(false)),
        ],
        3,
    )
    .unwrap();
    let model = [("model", fit[0].clone())];
    assert!(
        run(
            "yssbi.statistics.iv.2sls.summary",
            &model,
            &iv_summary_parameters("2sls", false),
            1,
        )
        .is_ok()
    );
    assert!(matches!(
        run(
            "yssbi.statistics.iv.2sls.summary",
            &model,
            &iv_summary_parameters("2sls", true),
            1,
        ),
        Err(KernelError::ScientificFailure)
    ));

    let signal = |row: usize, bit: usize| if row & (1 << bit) == 0 { -1.0 } else { 1.0 };
    let n = 64;
    let z = (0..2)
        .map(|bit| (0..n).map(|row| signal(row, bit)).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let x1 = (0..n)
        .map(|row| 2.0 * signal(row, 0) + signal(row, 2))
        .collect::<Vec<_>>();
    let x2 = (0..n)
        .map(|row| signal(row, 1) + signal(row, 2) + signal(row, 3))
        .collect::<Vec<_>>();
    let y = (0..n)
        .map(|row| 1.0 + x1[row] + 0.7 * x2[row] + signal(row, 4))
        .collect::<Vec<_>>();
    for kind in ["2sls", "liml"] {
        let fit = run(
            &format!("yssbi.statistics.iv.{kind}.fit"),
            &[
                ("y", series(&y)),
                ("endogenous", series(&x1)),
                ("endogenous", series(&x2)),
                ("instruments", series(&z[0])),
                ("instruments", series(&z[1])),
            ],
            &[
                ("constant", flag(true)),
                ("covariance", string("nonrobust")),
                ("small", flag(false)),
            ],
            3,
        )
        .unwrap();
        let report = run(
            &format!("yssbi.statistics.iv.{kind}.summary"),
            &[("model", fit[0].clone())],
            &iv_summary_parameters(kind, true),
            1,
        )
        .unwrap();
        let statistics = field(field(&report[0], "firstStage").unwrap(), "statistics").unwrap();
        let actual =
            super::super::super::numeric_input(Some(field(statistics, "min_eigenvalue").unwrap()))
                .unwrap();
        assert!((actual - 14.300569338447126).abs() < 1e-9, "{actual}");
    }
}

#[test]
fn iv_nodes_admit_linear_workspaces_and_enforce_small_memory_budgets() {
    let n = 1024;
    let signal = |row: usize, bit: usize| if row & (1 << bit) == 0 { -1.0 } else { 1.0 };
    let endogenous = (0..n)
        .map(|row| 2.0 * signal(row, 0) + 0.5 * signal(row, 1) + signal(row, 2))
        .collect::<Vec<_>>();
    let response = (0..n)
        .map(|row| 1.0 + endogenous[row] + signal(row, 3))
        .collect::<Vec<_>>();
    let instruments = (0..2)
        .map(|bit| (0..n).map(|row| signal(row, bit)).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let relations = crate::tests::relations();
    for kind in ["2sls", "liml"] {
        let input = [
            series(&response),
            series(&endogenous),
            series(&instruments[0]),
            series(&instruments[1]),
        ];
        let parameters = [
            ("constant", flag(true)),
            ("covariance", string("nonrobust")),
            ("small", flag(false)),
        ];
        let outputs = [
            KernelOutputSpec {
                data_type: ValueType::Struct("statistics.report".into()),
                fields: None,
            },
            KernelOutputSpec {
                data_type: ValueType::DataSeries(Box::new(ValueType::number())),
                fields: None,
            },
            KernelOutputSpec {
                data_type: ValueType::DataSeries(Box::new(ValueType::number())),
                fields: None,
            },
        ];
        let mut control = KernelControl::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(30),
        );
        control.max_input_bytes = 8 * 1024 * 1024;
        let registry = KernelRegistry::default();
        let fit = registry
            .execute(
                &KernelId::new(format!("yssbi.statistics.iv.{kind}.fit").into()).unwrap(),
                &KernelInvocation {
                    relations: &relations,
                    inputs: &input,
                    input_keys: &["y", "endogenous", "instruments", "instruments"],
                    parameters: parameters
                        .iter()
                        .map(|(key, value)| {
                            (
                                KernelParameterKey::new((*key).into()).unwrap(),
                                Cow::Borrowed(value),
                            )
                        })
                        .collect(),
                    outputs: &outputs,
                    control: &control,
                },
            )
            .unwrap();
        let parameters = iv_summary_parameters(kind, true);
        let outputs = [outputs[0].clone()];
        let input = [fit[0].clone()];
        let id = KernelId::new(format!("yssbi.statistics.iv.{kind}.summary").into()).unwrap();
        let invoke = |control: &KernelControl| {
            registry.execute(
                &id,
                &KernelInvocation {
                    relations: &relations,
                    inputs: &input,
                    input_keys: &["model"],
                    parameters: parameters
                        .iter()
                        .map(|(key, value)| {
                            (
                                KernelParameterKey::new((*key).into()).unwrap(),
                                Cow::Borrowed(value),
                            )
                        })
                        .collect(),
                    outputs: &outputs,
                    control,
                },
            )
        };
        assert!(invoke(&control).is_ok());
        control.max_input_bytes = 1;
        assert!(matches!(invoke(&control), Err(KernelError::BudgetExceeded)));
    }
}

fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../../../yss-sci/tests/fixtures/causal_category_reference.json"
    ))
    .unwrap()
}
fn column(v: &serde_json::Value) -> RuntimeValue {
    series(&serde_json::from_value::<Vec<f64>>(v.clone()).unwrap())
}

#[test]
fn causal_effect_projections_preserve_estimator_and_inference() {
    let f = fixture();
    let d = &f["treatment"];
    let inputs = [
        ("y", column(&d["response"])),
        ("treatment", column(&d["treatment"])),
        ("x", column(&d["predictors"][0])),
        ("x", column(&d["predictors"][1])),
    ];
    let parameters = [
        ("ps_overlap", number(1e-6)),
        ("bootstrap_replications", int(12)),
        ("seed", int(42)),
        ("max_iterations", int(500)),
        ("tolerance", number(1e-7)),
    ];
    let r = run("yssbi.statistics.causal.aipw", &inputs, &parameters, 1)
        .unwrap()
        .remove(0);
    for effect in ["ate", "att"] {
        let projected = run(
            &format!("yssbi.statistics.causal.{effect}"),
            &[("effects", r.clone())],
            &[],
            1,
        )
        .unwrap()
        .remove(0);
        assert_eq!(
            field(&projected, "effect").unwrap(),
            field(&r, effect).unwrap()
        );
        assert_eq!(field(&projected, "method").unwrap(), &string("aipw"));
        assert_eq!(
            field(&projected, "bootstrap_replications").unwrap(),
            &int(12)
        );
    }
    assert!(matches!(
        run(
            "yssbi.statistics.causal.ate",
            &[("effects", inputs[0].1.clone())],
            &[],
            1
        ),
        Err(KernelError::InvalidNumericInput)
    ));
}

#[test]
fn causal_adapters_handle_selection_nulls_equation_maps_and_exact_group_labels() {
    let f = fixture();
    let d = &f["heckman"];
    let response = RuntimeValue::List(
        d["response"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| {
                v.as_f64()
                    .map_or_else(|| TabularScalar::Null.into(), number)
            })
            .collect(),
    );
    let selected = RuntimeValue::List(
        d["selected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| flag(v.as_i64().unwrap() == 1))
            .collect(),
    );
    let mut inputs = vec![
        ("y", response),
        ("selected", selected),
        ("x", column(&d["predictors"][0])),
        (
            "selection_predictors",
            column(&d["selection_predictors"][0]),
        ),
        (
            "selection_predictors",
            column(&d["selection_predictors"][1]),
        ),
    ];
    let parameters = [
        ("bootstrap_replications", int(0)),
        ("seed", int(42)),
        ("max_iterations", int(500)),
        ("tolerance", number(1e-7)),
    ];
    let r = run(
        "yssbi.statistics.econometrics.heckman_two_step",
        &inputs,
        &parameters,
        1,
    )
    .unwrap()
    .remove(0);
    assert_eq!(
        field(&r, "outcome_covariance").unwrap(),
        &RuntimeValue::Scalar(TabularScalar::Null)
    );
    let selected_index = d["selected"]
        .as_array()
        .unwrap()
        .iter()
        .position(|v| v == 1)
        .unwrap();
    if let RuntimeValue::List(v) = &mut inputs[0].1 {
        let mut values = v.to_vec();
        values[selected_index] = TabularScalar::Null.into();
        *v = values.into();
    }
    assert!(matches!(
        run(
            "yssbi.statistics.econometrics.heckman_two_step",
            &inputs,
            &parameters,
            1
        ),
        Err(KernelError::InvalidNumericInput)
    ));
    let d = &f["sur"];
    let inputs = [
        ("y", column(&d["responses"][0])),
        ("y", column(&d["responses"][1])),
        ("x", column(&d["predictors"][0])),
        ("x", column(&d["predictors"][1])),
        ("x", column(&d["predictors"][2])),
    ];
    let r = run(
        "yssbi.statistics.econometrics.sur",
        &inputs,
        &[
            ("constant", flag(true)),
            ("equation_predictors", string("1,2;1,3")),
        ],
        1,
    )
    .unwrap()
    .remove(0);
    let RuntimeValue::List(equations) = field(&r, "equations").unwrap() else {
        panic!("equations")
    };
    assert_eq!(
        field(&equations[1], "predictors").unwrap(),
        &RuntimeValue::List(vec![int(1), int(3)].into())
    );
    assert!(matches!(
        run(
            "yssbi.statistics.econometrics.sur",
            &inputs,
            &[
                ("constant", flag(true)),
                ("equation_predictors", string("1,1;3"))
            ],
            1
        ),
        Err(KernelError::InvalidParameter)
    ));
    let d = &f["treatment"];
    let labels = RuntimeValue::List(
        f["heterogeneity"]["groups"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| int(9_007_199_254_740_992 + v.as_i64().unwrap()))
            .collect(),
    );
    let inputs = [
        ("y", column(&d["response"])),
        ("treatment", column(&d["treatment"])),
        ("groups", labels),
        ("x", column(&d["predictors"][0])),
        ("x", column(&d["predictors"][1])),
    ];
    let r = run("yssbi.statistics.test.heterogeneity", &inputs, &[], 1)
        .unwrap()
        .remove(0);
    let RuntimeValue::List(groups) = field(&r, "groups").unwrap() else {
        panic!("groups")
    };
    assert_eq!(groups.len(), 2);
    assert_ne!(groups[0], groups[1]);
}

#[test]
fn causal_adapter_validates_alignment_budget_deadline_and_cancellation() {
    let f = fixture();
    let d = &f["synthetic"];
    let id = "yssbi.statistics.causal.synthetic_control";
    let inputs = [
        ("y", column(&d["response"])),
        ("donors", column(&d["donors"][0])),
        ("donors", column(&d["donors"][1])),
        ("donors", column(&d["donors"][2])),
    ];
    let parameters = [
        ("pre_periods", int(20)),
        ("max_iterations", int(5000)),
        ("tolerance", number(1e-7)),
    ];
    run(id, &inputs, &parameters, 1).unwrap();
    let mut unequal = inputs.clone();
    unequal[1].1 = series(&[1., 2.]);
    assert!(matches!(
        run(id, &unequal, &parameters, 1),
        Err(KernelError::ShapeMismatch)
    ));
    let values = inputs.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>();
    let keys = inputs.iter().map(|(k, _)| *k).collect::<Vec<_>>();
    let relations = crate::tests::relations();
    let outputs = [KernelOutputSpec {
        data_type: ValueType::Struct("statistics.report".into()),
        fields: None,
    }];
    for (budget, cancelled, expired, expected) in [
        (1, false, false, KernelError::BudgetExceeded),
        (128 * 1024 * 1024, true, false, KernelError::Cancelled),
        (
            128 * 1024 * 1024,
            false,
            true,
            KernelError::DeadlineExceeded,
        ),
    ] {
        let mut control = KernelControl::new(
            Arc::new(AtomicBool::new(cancelled)),
            if expired {
                Instant::now() - Duration::from_secs(1)
            } else {
                Instant::now() + Duration::from_secs(30)
            },
        );
        control.max_input_bytes = budget;
        let invocation = KernelInvocation {
            relations: &relations,
            inputs: &values,
            input_keys: &keys,
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
        let error = KernelRegistry::default()
            .execute(&KernelId::new(id.into()).unwrap(), &invocation)
            .unwrap_err();
        assert_eq!(
            std::mem::discriminant(&error),
            std::mem::discriminant(&expected)
        );
    }
}
