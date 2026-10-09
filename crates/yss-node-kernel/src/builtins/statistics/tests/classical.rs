use super::*;

#[test]
fn student_t_node_preserves_extreme_directed_tails() {
    for (null_mean, alternative, multiplier) in [
        (1e308, "two_sided", 2.0),
        (1e308, "less", 1.0),
        (-1e308, "greater", 1.0),
    ] {
        let outputs = run(
            "yssbi.statistics.test.t.one_sample",
            &[("series", series(&[0.0, 2.0]))],
            &[
                ("null_mean", number(null_mean)),
                ("alternative", string(alternative)),
            ],
            1,
        )
        .unwrap();
        let p =
            crate::builtins::numeric_input(Some(field(&outputs[0], "p_value").unwrap())).unwrap();
        let expected = multiplier * (1e-308 / std::f64::consts::PI);
        assert!(
            (p / expected - 1.0).abs() < 2e-12,
            "{null_mean}, {alternative}: actual {p}, expected {expected}"
        );
    }
}

#[test]
fn classical_admission_preserves_option_observation_and_computation_roles() {
    for (id, values, parameters) in [
        (
            "yssbi.statistics.test.poisson",
            vec![1.0, 2.0, 3.0],
            vec![
                ("null_rate", number(-1.0)),
                ("alternative", string("two_sided")),
            ],
        ),
        (
            "yssbi.statistics.test.z.mean",
            vec![1.0, 2.0, 3.0],
            vec![
                ("null_mean", number(0.0)),
                ("population_sd", number(0.0)),
                ("alternative", string("two_sided")),
            ],
        ),
        (
            "yssbi.statistics.test.binomial",
            vec![0.0, 1.0, 0.0],
            vec![
                ("null_probability", number(1.2)),
                ("alternative", string("two_sided")),
            ],
        ),
    ] {
        let error = run(id, &[("series", series(&values))], &parameters, 1).unwrap_err();
        assert!(
            matches!(error, KernelError::InvalidParameter),
            "{id}: {error:?}"
        );
    }
    let error = run(
        "yssbi.statistics.test.poisson",
        &[("series", series(&[1.0, -1.0, 2.0]))],
        &[
            ("null_rate", number(1.0)),
            ("alternative", string("two_sided")),
        ],
        1,
    )
    .unwrap_err();
    assert!(
        matches!(error, KernelError::InvalidNumericInput),
        "{error:?}"
    );
    let error = run(
        "yssbi.statistics.test.t.one_sample",
        &[("series", series(&[f64::MAX, f64::MAX, f64::MAX]))],
        &[
            ("null_mean", number(0.0)),
            ("alternative", string("two_sided")),
        ],
        1,
    )
    .unwrap_err();
    assert!(matches!(error, KernelError::ScientificFailure), "{error:?}");

    let result = run(
        "yssbi.statistics.test.chisquare.goodness_of_fit",
        &[
            ("observed", series(&[10.0, 20.0])),
            ("expected", series(&[10.0, 10.0])),
        ],
        &[],
        1,
    );
    assert!(
        matches!(result, Err(KernelError::InvalidNumericInput)),
        "invalid frequency totals crossed the kernel boundary: {result:?}"
    );
    let result = run(
        "yssbi.statistics.test.chisquare.general",
        &[("counts", series(&[0.0; 4]))],
        &[("rows", int(2)), ("columns", int(2))],
        1,
    );
    assert!(
        matches!(result, Err(KernelError::InvalidNumericInput)),
        "zero-total counts crossed the wrong error boundary: {result:?}"
    );
}

#[test]
fn poisson_adapter_admits_counts_and_rates_without_fixed_scientific_caps() {
    for (counts, rate, alternative) in [
        ([0.0], 500_001.0, "greater"),
        ([1_000_001.0], 0.0, "less"),
        ([1_000_001.0], 1.0, "less"),
    ] {
        let result = run(
            "yssbi.statistics.test.poisson",
            &[("series", series(&counts))],
            &[
                ("null_rate", number(rate)),
                ("alternative", string(alternative)),
            ],
            1,
        )
        .unwrap();
        let p_value =
            crate::builtins::numeric_input(Some(field(&result[0], "p_value").unwrap())).unwrap();
        assert_eq!(p_value, 1.0);
    }
}

#[test]
fn discrete_adapters_preserve_inclusive_and_degenerate_exact_tails() {
    for (id, values, parameter, null, alternative, expected) in [
        (
            "binomial",
            [0.0, 0.0],
            "null_probability",
            0.5,
            "greater",
            1.0,
        ),
        ("binomial", [1.0, 0.0], "null_probability", 0.0, "less", 1.0),
        (
            "binomial",
            [1.0, 0.0],
            "null_probability",
            1.0,
            "greater",
            1.0,
        ),
        ("poisson", [0.0, 0.0], "null_rate", 1.0, "greater", 1.0),
        ("poisson", [1.0, 0.0], "null_rate", 0.0, "less", 1.0),
    ] {
        let result = run(
            &format!("yssbi.statistics.test.{id}"),
            &[("series", series(&values))],
            &[
                (parameter, number(null)),
                ("alternative", string(alternative)),
            ],
            1,
        )
        .unwrap();
        let p_value =
            crate::builtins::numeric_input(Some(field(&result[0], "p_value").unwrap())).unwrap();
        assert!(
            (p_value - expected).abs() < 1e-12,
            "{id}, {alternative}, null={null}: {p_value} != {expected}"
        );
    }
}

#[test]
fn fisher_adapter_delivers_exact_result_with_unbounded_odds_ratio() {
    let result = run(
        "yssbi.statistics.test.fisher_exact",
        &[
            ("row", series(&[0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0])),
            ("column", series(&[0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0])),
        ],
        &[],
        1,
    )
    .unwrap();
    assert_eq!(
        field(&result[0], "statistic").unwrap(),
        &RuntimeValue::Scalar(TabularScalar::Null)
    );
    let p_value =
        crate::builtins::numeric_input(Some(field(&result[0], "p_value").unwrap())).unwrap();
    assert!((p_value - 1.0 / 35.0).abs() < 1e-12);
}

#[test]
fn cmh_binary_inputs_preserve_mixed_source_statistics_and_numeric_admission() {
    use arrow_array::{ArrayRef, BooleanArray, RecordBatch};
    use yss_database_engine::DataFusionRuntime;
    use yss_relational_contract::{RelationControl, RelationFactory};

    let factory: Arc<dyn RelationFactory> = DataFusionRuntime::unbounded(32).unwrap();
    let mut control = KernelControl::new(
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
            RecordBatch::try_from_iter([(
                "outcome",
                Arc::new(BooleanArray::from(vec![
                    false, false, true, false, true, true, false, true, false, true, true, true,
                ])) as ArrayRef,
            )])
            .unwrap(),
            &relation_control,
        )
        .unwrap();
    let exposed = vec![
        number(-0.0),
        flag(false),
        TabularScalar::Unsigned(0).into(),
        int(1),
        number(1.0),
        flag(true),
        int(0),
        number(-0.0),
        flag(false),
        TabularScalar::Unsigned(1).into(),
        flag(true),
        number(1.0),
    ];
    let mut values = [
        RuntimeValue::List(exposed.into()),
        RuntimeValue::Series(source.select_series("outcome").unwrap()),
        RuntimeValue::List(
            (0..12)
                .map(|i| int(9_007_199_254_740_992 + i64::from(i >= 6)))
                .collect(),
        ),
    ];
    let outputs = [KernelOutputSpec {
        data_type: ValueType::Struct("statistics.report".into()),
        fields: None,
    }];
    let registry = KernelRegistry::default();
    let id = KernelId::new("yssbi.statistics.test.cmh".into()).unwrap();
    let execute = |values: &[RuntimeValue], control: &KernelControl| {
        registry.execute(
            &id,
            &KernelInvocation {
                relations: &factory,
                inputs: values,
                input_keys: &["exposed", "outcome", "strata"],
                parameters: Default::default(),
                outputs: &outputs,
                control,
            },
        )
    };
    let result = execute(&values, &control).unwrap();
    // The two 2x2 tables give sum(a-E[a])=1.5 and summed conditional variance=0.85.
    let statistic =
        crate::builtins::numeric_input(Some(field(&result[0], "statistic").unwrap())).unwrap();
    assert!((statistic - 45.0 / 17.0).abs() < 1e-12);
    control.max_input_bytes = 256;
    assert!(matches!(
        execute(&values, &control),
        Err(KernelError::BudgetExceeded)
    ));
    control.max_input_bytes = usize::MAX;
    values[0] = RuntimeValue::List(vec![string("0"); 12].into());
    assert!(matches!(
        execute(&values, &control),
        Err(KernelError::InvalidNumericInput)
    ));
}
