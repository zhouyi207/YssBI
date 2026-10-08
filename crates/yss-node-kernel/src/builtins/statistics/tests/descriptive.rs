use super::*;

#[test]
fn inequality_domain_failures_remain_numeric_input_errors() {
    for (id, inputs, parameters) in [
        (
            "yssbi.statistics.inequality.gini",
            vec![("series", series(&[-1., 2.]))],
            vec![],
        ),
        (
            "yssbi.statistics.inequality.dagum_gini",
            vec![
                ("series", series(&[-1., 2.])),
                ("groups", series(&[0., 1.])),
            ],
            vec![],
        ),
        (
            "yssbi.statistics.inequality.theil",
            vec![("series", series(&[-1., 2.]))],
            vec![("theil_form", string("individual"))],
        ),
    ] {
        let error = run(id, &inputs, &parameters, 1).unwrap_err();
        assert!(
            matches!(error, KernelError::InvalidNumericInput),
            "{id}: {error:?}",
        );
    }
}

#[test]
fn dagum_adapter_preserves_exact_labels_and_enforces_input_admission() {
    let id = "yssbi.statistics.inequality.dagum_gini";
    let values = series(&[1., 2., 3., 4., 2., 6.]);
    let labels = [u64::MAX, u64::MAX - 1];
    let groups = RuntimeValue::List(
        (0..6)
            .map(|row| TabularScalar::Unsigned(labels[row % 2]).into())
            .collect(),
    );
    let result = run(
        id,
        &[("series", values.clone()), ("groups", groups.clone())],
        &[],
        1,
    )
    .unwrap();
    let RuntimeValue::Scalar(TabularScalar::Float64(gini)) = field(&result[0], "gini").unwrap()
    else {
        panic!("Gini must be numeric");
    };
    assert!((gini.as_f64() - 8. / 27.).abs() < 1e-12);
    let RuntimeValue::List(actual) = field(&result[0], "groups").unwrap() else {
        panic!("Dagum must retain its group details");
    };
    assert_eq!(actual.len(), 2);
    for (index, group) in actual.iter().enumerate() {
        assert_eq!(
            field(group, "group").unwrap(),
            &RuntimeValue::from(TabularScalar::Unsigned(labels[index])),
        );
        assert_eq!(field(group, "observations").unwrap(), &int(3));
        assert_eq!(
            field(group, "mean").unwrap(),
            &number(2. + index as f64 * 2.)
        );
    }
    let missing = RuntimeValue::List(vec![TabularScalar::Null.into(); 6].into());
    assert!(matches!(
        run(
            id,
            &[("series", values.clone()), ("groups", missing)],
            &[],
            1
        ),
        Err(KernelError::InvalidNumericInput)
    ));
    assert!(matches!(
        run(
            id,
            &[("series", values.clone()), ("groups", series(&[0., 1.]))],
            &[],
            1,
        ),
        Err(KernelError::ShapeMismatch)
    ));
    let relations = crate::tests::relations();
    let inputs = [values, groups];
    let outputs = [KernelOutputSpec {
        data_type: ValueType::Struct("statistics.report".into()),
        fields: None,
    }];
    let mut control = KernelControl::new(
        Arc::new(AtomicBool::new(false)),
        Instant::now() + Duration::from_secs(30),
    );
    control.max_input_bytes = 16 * 1024;
    let inv = KernelInvocation {
        relations: &relations,
        inputs: &inputs,
        input_keys: &["series", "groups"],
        parameters: Default::default(),
        outputs: &outputs,
        control: &control,
    };
    super::super::common::materialize(&inv).unwrap();
    assert!(matches!(
        KernelRegistry::default().execute(&KernelId::new(id.into()).unwrap(), &inv),
        Err(KernelError::BudgetExceeded)
    ));
}
