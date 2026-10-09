use super::*;

#[test]
fn asymptotic_nodes_preserve_small_and_subnormal_tail_values() {
    let residuals: Vec<_> = (0..20).map(|i| f64::from(i == 0)).collect();
    let outputs = run(
        "yssbi.statistics.test.normality",
        &[("series", series(&residuals))],
        &[],
        1,
    )
    .unwrap();
    let numeric = |v: &RuntimeValue| crate::builtins::numeric_input(Some(v)).unwrap();
    let statistic = numeric(field(&outputs[0], "jarque_bera_stat").unwrap());
    let mut reports = vec![(
        "Jarque-Bera",
        numeric(field(&outputs[0], "jarque_bera_p_value").unwrap()),
        (-statistic / 2.0).exp(),
    )];
    for (method, observations, successes, expected) in [
        ("logit", 200, 180, 1.141_306_038_338_677_2e-20),
        ("probit", 200, 180, 2.905_331_538_655_039e-26),
        ("logit", 3408, 3067, f64::from_bits(1)),
    ] {
        let response: Vec<_> = (0..observations)
            .map(|i| f64::from(i < successes))
            .collect();
        let outputs = run(
            &format!("yssbi.statistics.{method}.fit"),
            &[
                ("y", series(&response)),
                ("x", series(&vec![1.0; observations])),
            ],
            &[
                ("constant", flag(false)),
                ("max_iterations", int(100)),
                ("tolerance", number(1e-12)),
            ],
            3,
        )
        .unwrap();
        let stats = field(&outputs[0], "statistics").unwrap();
        let RuntimeValue::List(probabilities) = field(stats, "pValues").unwrap() else {
            panic!("coefficient probabilities")
        };
        reports.push((method, numeric(&probabilities[0]), expected));
    }
    let lost: Vec<_> = reports
        .into_iter()
        .filter(|(_, actual, expected)| {
            (actual / expected - 1.0).abs() >= 1e-8 || !actual.is_finite()
        })
        .collect();
    assert!(
        lost.is_empty(),
        "Tail values lost at the node boundary: {lost:?}"
    );
}

#[test]
fn f_inference_keeps_small_tails_in_anova_and_linear_nodes() {
    let response = [
        1.,
        2.,
        3.,
        1e10 + 1.,
        1e10 + 2.,
        1e10 + 3.,
        2e10 + 1.,
        2e10 + 2.,
        2e10 + 3.,
    ];
    let factor = RuntimeValue::List((0..9).map(|row| int(row / 3)).collect());
    let anova = run(
        "yssbi.statistics.anova.one_way",
        &[("y", series(&response)), ("factors", factor)],
        &[],
        1,
    )
    .unwrap();
    let RuntimeValue::List(terms) = field(&anova[0], "table").unwrap() else {
        panic!("ANOVA table");
    };
    let statistic =
        super::super::super::numeric_input(Some(field(&terms[0], "f_statistic").unwrap())).unwrap();
    let p = super::super::super::numeric_input(Some(field(&terms[0], "p_value").unwrap())).unwrap();
    let first = (0..9)
        .map(|row| f64::from(row / 3 == 1))
        .collect::<Vec<_>>();
    let second = (0..9)
        .map(|row| f64::from(row / 3 == 2))
        .collect::<Vec<_>>();
    let fit = run(
        "yssbi.statistics.linear.fit",
        &[
            ("y", series(&response)),
            ("x", series(&first)),
            ("x", series(&second)),
        ],
        &[
            ("method", string("OLS")),
            ("constant", flag(true)),
            ("covariance", string("nonrobust")),
        ],
        3,
    )
    .unwrap();
    let RuntimeValue::LinearRegression(model) = &fit[0] else {
        panic!("linear model");
    };
    let linear = &model.report.model_basic_info;
    let reports = [
        ("anova", statistic, p),
        ("linear", linear.f_statistic, linear.prob_f_statistic),
    ];
    for (method, statistic, p) in reports {
        let expected = (3.0 / (3.0 + statistic)).powi(3);
        assert!(
            (p / expected - 1.0).abs() < 1e-12,
            "{method} lost its representable F tail: {reports:?}"
        );
    }
}

#[test]
fn model_diagnostics_have_no_default_workspace_budget() {
    let n = 512;
    let p = 40;
    let data = noise(n * (p + 1));
    let predictors = data[..n * p].chunks_exact(n).collect::<Vec<_>>();
    let y = (0..n)
        .map(|i| {
            2. + predictors
                .iter()
                .enumerate()
                .map(|(j, x)| x[i] / (j + 1) as f64)
                .sum::<f64>()
                + data[n * p + i] * (1. + predictors[0][i].abs())
        })
        .collect::<Vec<_>>();
    let weights = predictors[0]
        .iter()
        .map(|x| 1. / (1. + x.abs()).powi(2))
        .collect::<Vec<_>>();
    for method in ["OLS", "WLS"] {
        let mut inputs = vec![("y", series(&y))];
        inputs.extend(predictors.iter().map(|x| ("x", series(x))));
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
        let model = [("model", fit[0].clone())];
        for rhs in [false, true] {
            for koenker in [false, true] {
                let report = run(
                    "yssbi.statistics.diagnostic.breusch_pagan",
                    &model,
                    &[("rhs", flag(rhs)), ("koenker", flag(koenker))],
                    1,
                )
                .unwrap_or_else(|error| {
                    panic!("{method}, rhs={rhs}, koenker={koenker}: {error:?}")
                });
                let result = field(&report[0], "result").unwrap();
                assert_eq!(
                    field(result, "df").unwrap(),
                    &int(if rhs { p as i64 } else { 1 })
                );
                let probability =
                    super::super::super::numeric_input(Some(field(result, "p_value").unwrap()))
                        .unwrap();
                assert!((0.0..=1.0).contains(&probability));
            }
        }
        for (name, parameters) in [
            ("vif", vec![]),
            ("leverage", vec![]),
            ("reset", vec![("rhs", flag(false))]),
            ("reset", vec![("rhs", flag(true))]),
            ("wald", vec![("hypothesis", string("x1 = 0"))]),
            (
                "breusch_godfrey",
                vec![("lags", int(2)), ("bg_nomiss0", flag(true))],
            ),
        ] {
            run(
                &format!("yssbi.statistics.diagnostic.{name}"),
                &model,
                &parameters,
                1,
            )
            .unwrap_or_else(|error| panic!("{method}, {name}: {error:?}"));
        }
        let control = KernelControl::new(
            Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(30),
        );
        let invocation = KernelInvocation {
            relations: &crate::tests::relations(),
            inputs: &fit[..1],
            input_keys: &["model"],
            parameters: [("rhs", flag(false)), ("koenker", flag(false))]
                .into_iter()
                .map(|(key, value)| {
                    (
                        KernelParameterKey::new(key.into()).unwrap(),
                        Cow::Owned(value),
                    )
                })
                .collect(),
            outputs: &[KernelOutputSpec {
                data_type: ValueType::Struct("statistics.report".into()),
                fields: None,
            }],
            control: &control,
        };
        let registry = KernelRegistry::default();
        let id = KernelId::new("yssbi.statistics.diagnostic.breusch_pagan".into()).unwrap();
        let result = registry.execute(&id, &invocation).unwrap();
        validate_display(&result[0]);
        control
            .cancellation
            .store(true, std::sync::atomic::Ordering::Release);
        assert!(matches!(
            registry.execute(&id, &invocation),
            Err(KernelError::Cancelled)
        ));
    }
}

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
