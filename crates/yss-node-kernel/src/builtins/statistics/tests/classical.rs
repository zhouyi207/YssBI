use super::*;

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
