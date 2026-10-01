use super::*;

fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../../../../yss-sci/tests/fixtures/spatial_category_reference.json"
    ))
    .unwrap()
}
fn vector(v: &serde_json::Value) -> Vec<f64> {
    serde_json::from_value(v.clone()).unwrap()
}
fn ids(n: usize) -> RuntimeValue {
    RuntimeValue::List(
        (0..n)
            .map(|i| int(9_007_199_254_740_992 + i as i64))
            .collect(),
    )
}
fn weight_inputs(d: &serde_json::Value) -> Vec<(&'static str, RuntimeValue)> {
    vec![
        ("units", ids(d["x"].as_array().unwrap().len())),
        ("x", series(&vector(&d["x"]))),
        ("y", series(&vector(&d["y_coordinate"]))),
    ]
}
fn weight_parameters() -> Vec<(&'static str, RuntimeValue)> {
    vec![
        ("spatial_weight_rule", string("knn")),
        ("spatial_neighbors", int(4)),
        ("spatial_radius", number(1.)),
        ("spatial_power", number(1.)),
        ("spatial_symmetrize", flag(false)),
        ("spatial_row_standardize", flag(true)),
    ]
}
fn reverse(v: &RuntimeValue) -> RuntimeValue {
    let RuntimeValue::List(v) = v else {
        panic!("list")
    };
    RuntimeValue::List(v.iter().rev().cloned().collect())
}

#[test]
fn spatial_adapter_weights_roundtrip_preserves_exact_identifiers_and_reorders_observations() {
    let f = fixture();
    let d = &f["data"];
    let wi = weight_inputs(d);
    let w = run(
        "yssbi.statistics.spatial.weights",
        &wi,
        &weight_parameters(),
        1,
    )
    .unwrap()
    .remove(0);
    assert_eq!(field(&w, "units").unwrap(), &wi[0].1);
    let mut inputs = vec![
        ("weights", w.clone()),
        ("units", wi[0].1.clone()),
        ("response", series(&vector(&d["response"]))),
    ];
    for x in d["predictors"].as_array().unwrap() {
        inputs.push(("predictors", series(&vector(x))));
    }
    let parameters = [("constant", flag(true))];
    let first = run("yssbi.statistics.spatial.ols", &inputs, &parameters, 1)
        .unwrap()
        .remove(0);
    for (_, column) in inputs.iter_mut().skip(1) {
        *column = reverse(column);
    }
    let reordered = run("yssbi.statistics.spatial.ols", &inputs, &parameters, 1)
        .unwrap()
        .remove(0);
    assert_eq!(
        field(&first, "coefficients").unwrap(),
        field(&reordered, "coefficients").unwrap()
    );
    assert_eq!(
        reverse(field(&first, "fitted").unwrap()),
        *field(&reordered, "fitted").unwrap()
    );
    assert_eq!(field(&reordered, "unit_labels").unwrap(), &wi[0].1);
    // Duplicate, unknown, null and stringified numeric identifiers must not be guessed.
    for bad in [
        int(9_007_199_254_740_992),
        int(17),
        TabularScalar::Null.into(),
        string("9007199254740992"),
    ] {
        let RuntimeValue::List(original) = &inputs[1].1 else {
            panic!("ids")
        };
        let mut bad_ids = original.to_vec();
        bad_ids[0] = bad;
        let mut bad_inputs = inputs.clone();
        bad_inputs[1].1 = RuntimeValue::List(bad_ids.into());
        assert!(run("yssbi.statistics.spatial.ols", &bad_inputs, &parameters, 1).is_err());
    }
    let mut duplicate = wi.clone();
    duplicate[0].1 = RuntimeValue::List(vec![int(1); 48].into());
    assert!(matches!(
        run(
            "yssbi.statistics.spatial.weights",
            &duplicate,
            &weight_parameters(),
            1
        ),
        Err(KernelError::InvalidParameter)
    ));
}

#[test]
fn spatial_adapter_panel_balance_and_execution_budgets_are_enforced() {
    let f = fixture();
    let d = &f["data"];
    let panel = &f["panel"];
    let wi = weight_inputs(d);
    let units = d["x"].as_array().unwrap().len();
    let w = run(
        "yssbi.statistics.spatial.weights",
        &wi,
        &weight_parameters(),
        1,
    )
    .unwrap()
    .remove(0);
    let rows = panel["response"].as_array().unwrap().len();
    let mut inputs = vec![
        ("weights", w),
        (
            "units",
            RuntimeValue::List(
                (0..rows)
                    .map(|i| int(9_007_199_254_740_992 + (i % units) as i64))
                    .collect(),
            ),
        ),
        (
            "periods",
            RuntimeValue::List(
                (0..rows)
                    .map(|i| string(&format!("period-{}", i / units)))
                    .collect(),
            ),
        ),
        ("response", series(&vector(&panel["response"]))),
    ];
    for x in panel["predictors"].as_array().unwrap() {
        inputs.push(("predictors", series(&vector(x))));
    }
    let parameters = [
        ("spatial_panel_model", string("sem")),
        ("max_iterations", int(500)),
        ("tolerance", number(1e-7)),
    ];
    for (_, col) in inputs.iter_mut().skip(1) {
        *col = reverse(col);
    }
    let result = run("yssbi.statistics.spatial.panel", &inputs, &parameters, 1).unwrap();
    assert_eq!(field(&result[0], "periods").unwrap(), &int(5));
    assert_eq!(
        field(&result[0], "estimation_observations").unwrap(),
        &int(192)
    );
    let mut duplicate = inputs.clone();
    let RuntimeValue::List(v) = &duplicate[1].1 else {
        panic!("list")
    };
    let mut v = v.to_vec();
    v[0] = v[1].clone();
    duplicate[1].1 = RuntimeValue::List(v.into());
    assert!(matches!(
        run("yssbi.statistics.spatial.panel", &duplicate, &parameters, 1),
        Err(KernelError::InvalidParameter)
    ));
    let mut unbalanced = inputs.clone();
    for (_, col) in unbalanced.iter_mut().skip(1) {
        let RuntimeValue::List(v) = col else {
            panic!("list")
        };
        *col = RuntimeValue::List(v[1..].to_vec().into());
    }
    assert!(matches!(
        run(
            "yssbi.statistics.spatial.panel",
            &unbalanced,
            &parameters,
            1
        ),
        Err(KernelError::ShapeMismatch)
    ));
    let values = wi.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>();
    let keys = wi.iter().map(|(k, _)| *k).collect::<Vec<_>>();
    let wp = weight_parameters();
    let relations = crate::tests::relations();
    let outputs = [KernelOutputSpec {
        data_type: ValueType::Struct("statistics.design.spatial_weights".into()),
        fields: None,
    }];
    for (budget, cancelled, expired, expected) in [
        (64_000, false, false, KernelError::BudgetExceeded),
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
        let inv = KernelInvocation {
            relations: &relations,
            inputs: &values,
            input_keys: &keys,
            parameters: wp
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
            .execute(
                &KernelId::new("yssbi.statistics.spatial.weights".into()).unwrap(),
                &inv,
            )
            .unwrap_err();
        assert_eq!(
            std::mem::discriminant(&error),
            std::mem::discriminant(&expected)
        );
    }
}
