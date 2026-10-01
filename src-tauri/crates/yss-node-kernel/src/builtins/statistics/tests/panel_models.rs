use super::*;

#[test]
fn panel_category_adapters_validate_parameters_alignment_and_resource_control() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../yss-sci/tests/fixtures/panel_category_reference.json"
    ))
    .unwrap();
    let data = &fixture["dynamic"];
    let column =
        |key: &str| series(&serde_json::from_value::<Vec<f64>>(data[key].clone()).unwrap());
    let x = series(&serde_json::from_value::<Vec<f64>>(data["predictors"][0].clone()).unwrap());
    let inputs = [
        ("response", column("response")),
        ("predictors", x),
        ("entity", column("entity")),
        ("time", column("time")),
    ];
    let id = "yssbi.statistics.econometrics.panel.dynamic";
    let parameters = [
        ("max_instrument_lag", int(3)),
        ("covariance", string("robust")),
    ];
    let result = run(id, &inputs, &parameters, 1).unwrap();
    assert_eq!(
        field(&result[0], "method").unwrap(),
        &string("arellano_bond_one_step_collapsed")
    );
    let RuntimeValue::List(coefficients) = field(&result[0], "coefficients").unwrap() else {
        panic!("coefficients");
    };
    assert_eq!(coefficients.len(), 2);
    assert!(matches!(
        run(
            id,
            &inputs,
            &[("max_instrument_lag", int(1)), parameters[1].clone()],
            1
        ),
        Err(KernelError::InvalidParameter)
    ));
    let mut unaligned = inputs.clone();
    unaligned[3].1 = series(&[0., 1., 2.]);
    assert!(matches!(
        run(id, &unaligned, &parameters, 1),
        Err(KernelError::ShapeMismatch)
    ));
    let mut missing = inputs.clone();
    if let RuntimeValue::List(values) = &mut missing[0].1 {
        let mut copied = values.to_vec();
        copied[0] = TabularScalar::Null.into();
        *values = copied.into();
    }
    assert!(matches!(
        run(id, &missing, &parameters, 1),
        Err(KernelError::InvalidNumericInput)
    ));
    let values = inputs.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>();
    let keys = inputs.iter().map(|(k, _)| *k).collect::<Vec<_>>();
    let relations = crate::tests::relations();
    let outputs = [KernelOutputSpec {
        data_type: ValueType::Struct("statistics.report".into()),
        fields: None,
    }];
    let registry = KernelRegistry::default();
    for (budget, cancelled, expected) in [
        (1, false, KernelError::BudgetExceeded),
        (128 * 1024 * 1024, true, KernelError::Cancelled),
    ] {
        let mut control = KernelControl::new(
            Arc::new(AtomicBool::new(cancelled)),
            Instant::now() + Duration::from_secs(10),
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
        let error = registry
            .execute(&KernelId::new(id.into()).unwrap(), &invocation)
            .unwrap_err();
        assert_eq!(
            std::mem::discriminant(&error),
            std::mem::discriminant(&expected)
        );
    }
}
