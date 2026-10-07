use super::*;

#[test]
fn time_series_adapters_preserve_state_labels_and_enforce_alignment_and_budgets() {
    let id = "yssbi.statistics.timeseries.markov_prediction";
    let states = RuntimeValue::List(
        vec![
            int(9_007_199_254_740_993),
            int(9_007_199_254_740_992),
            int(9_007_199_254_740_993),
        ]
        .into(),
    );
    let inputs = [("series", states.clone())];
    let params = [("ts_horizon", int(2)), ("ts_pseudocount", number(0.))];
    let result = run(id, &inputs, &params, 1).unwrap();
    let RuntimeValue::List(labels) = field(&result[0], "state_labels").unwrap() else {
        panic!("labels")
    };
    assert_eq!(
        labels.as_ref(),
        &[int(9_007_199_254_740_993), int(9_007_199_254_740_992)]
    );
    assert_eq!(
        field(&result[0], "forecast_labels").unwrap(),
        &RuntimeValue::List(vec![labels[1].clone(), labels[0].clone()].into())
    );
    assert!(matches!(
        run(
            id,
            &[(
                "series",
                RuntimeValue::List(vec![int(0), TabularScalar::Null.into(), int(1)].into())
            )],
            &params,
            1
        ),
        Err(KernelError::InvalidNumericInput)
    ));
    assert!(matches!(
        run(
            "yssbi.statistics.plot.time_series",
            &[
                ("series", series(&[1., 2., 3.])),
                ("time", series(&[1., 2.]))
            ],
            &[],
            1
        ),
        Err(KernelError::ShapeMismatch)
    ));
    assert!(matches!(
        run(
            "yssbi.statistics.plot.time_series",
            &[
                ("series", series(&[1., 2., 3.])),
                ("time", series(&[2., 1., 3.]))
            ],
            &[],
            1
        ),
        Err(KernelError::InvalidParameter)
    ));
    let output = run(
        "yssbi.statistics.plot.time_series",
        &[("series", series(&[2., 4., 3.]))],
        &[],
        1,
    )
    .unwrap();
    assert_eq!(field(&output[0], "xLabel").unwrap(), &string("Observation"));
    let relations = crate::tests::relations();
    let values = [states];
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
        let inv = KernelInvocation {
            relations: &relations,
            inputs: &values,
            input_keys: &["series"],
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
            .execute(&KernelId::new(id.into()).unwrap(), &inv)
            .unwrap_err();
        assert!(matches!(
            (error, cancelled, expired),
            (KernelError::Cancelled, true, _)
                | (KernelError::DeadlineExceeded, false, true)
                | (KernelError::BudgetExceeded, false, false)
        ));
    }
    let mut huge = params.clone();
    huge[0].1 = int(i64::MAX);
    assert!(matches!(
        run(id, &inputs, &huge, 1),
        Err(KernelError::BudgetExceeded)
    ));
}
