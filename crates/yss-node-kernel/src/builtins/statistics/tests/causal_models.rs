use super::*;

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
