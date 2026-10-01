use super::*;

#[test]
fn diagnostic_adapters_validate_paired_inputs_and_propagate_execution_limits() {
    let id = "yssbi.statistics.diagnostic.nri_idi";
    let params = [
        ("nri_mode", string("categorical")),
        ("risk_thresholds", series(&[0.5])),
    ];
    let mut inputs = [
        (
            "outcome",
            RuntimeValue::List(vec![flag(true), flag(true), flag(false), flag(false)].into()),
        ),
        ("reference", series(&[0.3, 0.6, 0.6, 0.4])),
        ("new", series(&[0.5, 0.6, 0.3, 0.4])),
    ];
    let result = run(id, &inputs, &params, 1).unwrap();
    assert_eq!(field(&result[0], "nri").unwrap(), &number(1.0));
    inputs[2].1 = series(&[0.5]);
    assert!(matches!(
        run(id, &inputs, &params, 1),
        Err(KernelError::ShapeMismatch)
    ));
    inputs[2].1 = RuntimeValue::List(
        vec![
            number(0.5),
            TabularScalar::Null.into(),
            number(0.3),
            number(0.4),
        ]
        .into(),
    );
    assert!(matches!(
        run(id, &inputs, &params, 1),
        Err(KernelError::InvalidNumericInput)
    ));
    inputs[2].1 = series(&[0.5, 0.6, 0.3, 0.4]);
    let invalid = [
        ("nri_mode", string("categorical")),
        ("risk_thresholds", series(&[0.5, 0.2])),
    ];
    assert!(matches!(
        run(id, &inputs, &invalid, 1),
        Err(KernelError::InvalidParameter)
    ));

    let values = inputs.iter().map(|(_, v)| v.clone()).collect::<Vec<_>>();
    let relations = crate::tests::relations();
    let outputs = [KernelOutputSpec {
        data_type: ValueType::Struct("statistics.report".into()),
        fields: None,
    }];
    for (budget, cancelled, expired) in [
        (1, false, false),
        (128 * 1024 * 1024, true, false),
        (128 * 1024 * 1024, false, true),
    ] {
        let mut control = KernelControl::new(
            Arc::new(AtomicBool::new(cancelled)),
            if expired {
                Instant::now() - Duration::from_secs(1)
            } else {
                Instant::now() + Duration::from_secs(10)
            },
        );
        control.max_input_bytes = budget;
        let invocation = KernelInvocation {
            relations: &relations,
            inputs: &values,
            input_keys: &["outcome", "reference", "new"],
            parameters: params
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
        assert!(matches!(
            (error, cancelled, expired),
            (KernelError::Cancelled, true, _)
                | (KernelError::DeadlineExceeded, false, true)
                | (KernelError::BudgetExceeded, false, false)
        ));
    }
    // A wide design needs thin factors and a small term report, not a p-by-p workspace.
    let values = vec![series(&[1., 2., 3., 4., 5., 6., 7., 8.]); 256];
    let keys = vec!["variables"; values.len()];
    let mut control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(10),
    );
    control.max_input_bytes = 2 * 1024 * 1024;
    let invocation = KernelInvocation {
        relations: &relations,
        inputs: &values,
        input_keys: &keys,
        parameters: [(
            KernelParameterKey::new("constant".into()).unwrap(),
            Cow::Owned(flag(true)),
        )]
        .into(),
        outputs: &outputs,
        control: &control,
    };
    let result = KernelRegistry::default()
        .execute(
            &KernelId::new("yssbi.statistics.diagnostic.collinearity".into()).unwrap(),
            &invocation,
        )
        .unwrap();
    assert_eq!(field(&result[0], "rank").unwrap(), &int(2));
}
