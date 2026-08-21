use super::*;

#[test]
fn bound_input_operation_executes_downstream_and_publishes_result_without_fallback() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("bound_source", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| Ok(vec![inputs[0].clone()])),
        )
        .unwrap();
    kernels
        .register(
            id("increment", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| {
                let RuntimeValue::Scalar(Value::Integer(value)) = inputs[0] else {
                    return Err(KernelError::new("expected integer"));
                };
                Ok(vec![Value::Integer(value + 1).into()])
            }),
        )
        .unwrap();
    let mut source = operation("bound_source", &[0], &[1]);
    source.inputs[0].bound_value = Some(Value::Integer(7));
    let mut execution_plan = plan(
        vec![source, operation("increment", &[1], &[2])],
        3,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(2),
    }]);
    publish_graph_results(&mut execution_plan);

    let result = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    assert_eq!(
        result.value_for_test("result").unwrap(),
        Value::Integer(8).into()
    );
}

#[test]
fn bound_input_blocked_by_effect_dependency_reports_effect_error() {
    let mut blocked = operation("blocked", &[0], &[]);
    blocked.inputs[0].bound_value = Some(Value::Integer(7));
    let mut execution_plan = plan(
        vec![operation("required", &[], &[]), blocked],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            1,
        ))])),
    );
    execution_plan.effect_dependencies = Box::new([EffectDependency {
        before: OperationIndex::new(0),
        after: OperationIndex::new(1),
    }]);

    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert!(matches!(
        error,
        RunError::UnsatisfiedEffectDependency {
            operation,
            required,
        } if operation == OperationIndex::new(1) && required == OperationIndex::new(0)
    ));
}

#[test]
fn bound_input_blocked_by_value_dependency_reports_dependency_source() {
    let mut blocked = operation("blocked", &[0], &[1]);
    blocked.inputs[0].bound_value = Some(Value::Integer(7));
    let mut execution_plan = plan(
        vec![blocked],
        3,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.value_sources = Box::new([PlanValueSource::ExternalInput(
        ValueRef::new(2),
        OutputProduction::FullyMaterialized,
    )]);
    execution_plan.value_dependencies = Box::new([ValueDependency {
        source: ValueRef::new(2),
        destination: ValueRef::new(1),
    }]);

    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert!(matches!(error, RunError::MissingValue(value) if value == ValueRef::new(2)));
}

#[test]
fn truly_missing_operation_input_still_reports_missing_value() {
    let mut execution_plan = plan(
        vec![operation("blocked", &[0], &[])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    execution_plan.value_sources = Box::new([PlanValueSource::ExternalInput(
        ValueRef::new(0),
        OutputProduction::FullyMaterialized,
    )]);

    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert!(matches!(error, RunError::MissingValue(value) if value == ValueRef::new(0)));
}

#[test]
fn runtime_admission_rejects_sequence_artifact_for_data_series_contract() {
    let series_contract = PlannedValueContract {
        kind: PlannedValueKind::DataSeries,
        type_expr: crate::node_system::protocol::data_series_type(TypeExpr::Concrete(
            TypeId::new("core.int64").unwrap(),
        )),
    };
    let downstream_executed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("sequence_artifact_source", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| {
                Ok(vec![RuntimeValue::Artifact(Artifact::new(
                    ArtifactKind::Collected,
                    [Value::Integer(1)],
                ))])
            }),
        )
        .unwrap();
    let observed = Arc::clone(&downstream_executed);
    kernels
        .register(
            id("data_series_sink", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.store(true, Ordering::SeqCst);
                Ok(Vec::new())
            }),
        )
        .unwrap();
    let mut source = operation("sequence_artifact_source", &[], &[0]);
    source.outputs[0].contract = series_contract.clone();
    source.outputs[0].production = OutputProduction::Streaming;
    let mut adapter = adapter_operation(
        "data_series_collect",
        1,
        2,
        OutputProduction::Streaming,
        InputConsumption::FullyMaterialized,
    );
    adapter.inputs[0].contract = series_contract.clone();
    adapter.outputs[0].contract = series_contract.clone();
    let mut sink = operation("data_series_sink", &[3], &[]);
    sink.inputs[0].contract = series_contract.clone();
    let mut execution_plan = plan(
        vec![source, adapter, sink],
        4,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    execution_plan.value_contracts = BTreeMap::from([
        (ValueRef::new(0), series_contract.clone()),
        (ValueRef::new(1), series_contract.clone()),
        (ValueRef::new(2), series_contract.clone()),
        (ValueRef::new(3), series_contract),
    ]);
    execution_plan.value_dependencies = Box::new([
        ValueDependency {
            source: ValueRef::new(0),
            destination: ValueRef::new(1),
        },
        ValueDependency {
            source: ValueRef::new(2),
            destination: ValueRef::new(3),
        },
    ]);

    let error = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert!(
        matches!(&error, RunError::InvalidPlan(message) if message.contains("DataSeries Artifact")),
        "unexpected admission error: {error:?}"
    );
    assert!(!downstream_executed.load(Ordering::SeqCst));
}

#[test]
fn runtime_admission_rejects_data_series_element_metadata_mismatch() {
    let int_series_contract = PlannedValueContract {
        kind: PlannedValueKind::DataSeries,
        type_expr: crate::node_system::protocol::data_series_type(TypeExpr::Concrete(
            TypeId::new("core.int64").unwrap(),
        )),
    };
    let downstream_executed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let observed = Arc::clone(&downstream_executed);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("float_series_source", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| {
                Ok(vec![RuntimeValue::Artifact(
                    DataSeriesBuilder::new(DataSeriesElementType::Float64)
                        .values([decimal("1.5")])
                        .name("float input")
                        .build(ArtifactKind::Collected)
                        .unwrap(),
                )])
            }),
        )
        .unwrap();
    kernels
        .register(
            id("int_series_sink", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.store(true, Ordering::SeqCst);
                Ok(Vec::new())
            }),
        )
        .unwrap();
    let mut source = operation("float_series_source", &[], &[0]);
    source.outputs[0].contract = int_series_contract.clone();
    source.outputs[0].production = OutputProduction::FullyMaterialized;
    let mut sink = operation("int_series_sink", &[0], &[]);
    sink.inputs[0].contract = int_series_contract.clone();
    let mut execution_plan = plan(
        vec![source, sink],
        1,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    execution_plan.value_contracts = BTreeMap::from([(ValueRef::new(0), int_series_contract)]);

    let error = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new())
    .unwrap_err();

    assert!(
        matches!(&error, RunError::InvalidPlan(message) if message.contains("expects Int64") && message.contains("Float64")),
        "unexpected metadata admission error: {error:?}"
    );
    assert!(!downstream_executed.load(Ordering::SeqCst));
}

#[test]
fn executes_sequence_deterministically() {
    let events = Arc::new(Mutex::new(Vec::new()));
    let mut kernels = KernelRegistry::new();
    for (name, number) in [("first", 1_i64), ("second", 2)] {
        let events = events.clone();
        kernels
            .register(
                id(name, KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| {
                    events.lock().unwrap().push(number);
                    Ok(vec![Value::Integer(number).into()])
                }),
            )
            .unwrap();
    }
    let mut execution_plan = plan(
        vec![
            operation("first", &[], &[0]),
            operation("second", &[0], &[1]),
        ],
        2,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
    execution_plan.value_dependencies = Box::new([ValueDependency {
        source: ValueRef::new(0),
        destination: ValueRef::new(1),
    }]);
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(1),
    }]);
    publish_graph_results(&mut execution_plan);

    let result = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    assert_eq!(*events.lock().unwrap(), vec![1, 2]);
    assert_eq!(
        result.value_for_test("result").unwrap(),
        RuntimeValue::from(Value::Integer(2))
    );
}

#[test]
fn if_uses_a_plan_bound_condition_value() {
    let counts = Arc::new(Mutex::new(BTreeMap::<&'static str, usize>::new()));
    let mut kernels = KernelRegistry::new();
    for name in ["then", "else"] {
        let counts = counts.clone();
        kernels
            .register(
                id(name, KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| {
                    *counts.lock().unwrap().entry(name).or_default() += 1;
                    Ok(Vec::new())
                }),
            )
            .unwrap();
    }
    let mut execution_plan = plan(
        vec![operation("then", &[], &[]), operation("else", &[], &[])],
        1,
        StructuredControlRegion::If {
            condition: ValueRef::new(0),
            then_region: Box::new(StructuredControlRegion::Sequence(Box::new([
                ControlStep::Operation(OperationIndex::new(0)),
            ]))),
            else_region: Box::new(StructuredControlRegion::Sequence(Box::new([
                ControlStep::Operation(OperationIndex::new(1)),
            ]))),
            results: Box::new([]),
        },
    );
    execution_plan
        .bound_values
        .insert(ValueRef::new(0), Value::Bool(true));

    RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    assert_eq!(counts.lock().unwrap().get("then"), Some(&1));
    assert_eq!(counts.lock().unwrap().get("else"), None);
}

#[test]
fn if_executes_only_selected_branch() {
    let counts = Arc::new(Mutex::new(BTreeMap::<&'static str, usize>::new()));
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("condition", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Bool(true).into()])),
        )
        .unwrap();
    for name in ["then", "else"] {
        let counts = counts.clone();
        kernels
            .register(
                id(name, KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| {
                    *counts.lock().unwrap().entry(name).or_default() += 1;
                    Ok(vec![Value::String(name.into()).into()])
                }),
            )
            .unwrap();
    }
    let mut execution_plan = plan(
        vec![
            operation("condition", &[], &[0]),
            operation("then", &[], &[1]),
            operation("else", &[], &[2]),
        ],
        4,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Region(Box::new(StructuredControlRegion::If {
                condition: ValueRef::new(0),
                then_region: Box::new(StructuredControlRegion::Sequence(Box::new([
                    ControlStep::Operation(OperationIndex::new(1)),
                ]))),
                else_region: Box::new(StructuredControlRegion::Sequence(Box::new([
                    ControlStep::Operation(OperationIndex::new(2)),
                ]))),
                results: Box::new([BranchResultBinding {
                    destination: ValueRef::new(3),
                    then_source: ValueRef::new(1),
                    else_source: ValueRef::new(2),
                    production: Some(OutputProduction::FullyMaterialized),
                }]),
            })),
        ])),
    );
    execution_plan.value_sources = Box::new([PlanValueSource::ControlProduced(
        ValueRef::new(3),
        OutputProduction::FullyMaterialized,
    )]);

    RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    assert_eq!(counts.lock().unwrap().get("then"), Some(&1));
    assert_eq!(counts.lock().unwrap().get("else"), None);
}

fn execute_nested_branch_sequence_switch(
    first_matches: bool,
    second_matches: bool,
) -> (RunResult, Vec<&'static str>) {
    let observed = Arc::new(Mutex::new(Vec::new()));
    let mut kernels = KernelRegistry::new();
    for (name, selected) in [
        ("first_condition", first_matches),
        ("second_condition", second_matches),
    ] {
        kernels
            .register(
                id(name, KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| Ok(vec![Value::Bool(selected).into()])),
            )
            .unwrap();
    }
    for (name, value) in [("first_case", 10_i64), ("second_case", 20), ("default", 30)] {
        let observed = Arc::clone(&observed);
        kernels
            .register(
                id(name, KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| {
                    observed.lock().unwrap().push(name);
                    Ok(vec![Value::Integer(value).into()])
                }),
            )
            .unwrap();
    }

    let inner_switch = StructuredControlRegion::If {
        condition: ValueRef::new(2),
        then_region: Box::new(StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(3)),
        ]))),
        else_region: Box::new(StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(4)),
        ]))),
        results: Box::new([BranchResultBinding {
            destination: ValueRef::new(5),
            then_source: ValueRef::new(3),
            else_source: ValueRef::new(4),
            production: Some(OutputProduction::FullyMaterialized),
        }]),
    };
    let outer_switch = StructuredControlRegion::If {
        condition: ValueRef::new(0),
        then_region: Box::new(StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(1)),
        ]))),
        else_region: Box::new(StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(2)),
            ControlStep::Region(Box::new(inner_switch)),
        ]))),
        results: Box::new([BranchResultBinding {
            destination: ValueRef::new(6),
            then_source: ValueRef::new(1),
            else_source: ValueRef::new(5),
            production: Some(OutputProduction::FullyMaterialized),
        }]),
    };
    let mut execution_plan = plan(
        vec![
            operation("first_condition", &[], &[0]),
            operation("first_case", &[], &[1]),
            operation("second_condition", &[], &[2]),
            operation("second_case", &[], &[3]),
            operation("default", &[], &[4]),
        ],
        7,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Region(Box::new(outer_switch)),
        ])),
    );
    execution_plan.value_sources = Box::new([
        PlanValueSource::ControlProduced(ValueRef::new(5), OutputProduction::FullyMaterialized),
        PlanValueSource::ControlProduced(ValueRef::new(6), OutputProduction::FullyMaterialized),
    ]);
    execution_plan.results = Box::new([PlanResult {
        name: "selected".into(),
        output: stable_output("selected"),
        value: ValueRef::new(6),
    }]);
    publish_graph_results(&mut execution_plan);

    let result = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new())
    .unwrap();
    let observed = observed.lock().unwrap().clone();
    (result, observed)
}

#[test]
fn nested_sibling_regions_produce_complete_data_exactly_once() {
    let (result, observed) = execute_nested_branch_sequence_switch(true, true);

    assert_eq!(observed, vec!["first_case"]);
    assert_eq!(
        result.value_for_test("selected").unwrap(),
        Value::Integer(10).into()
    );
}

#[test]
fn nested_branch_sequence_switch_executes_only_n_way_match() {
    let (result, observed) = execute_nested_branch_sequence_switch(false, true);

    assert_eq!(observed, vec!["second_case"]);
    assert_eq!(
        result.value_for_test("selected").unwrap(),
        Value::Integer(20).into()
    );
}

#[test]
fn nested_branch_sequence_switch_executes_default_when_no_case_matches() {
    let (result, observed) = execute_nested_branch_sequence_switch(false, false);

    assert_eq!(observed, vec!["default"]);
    assert_eq!(
        result.value_for_test("selected").unwrap(),
        Value::Integer(30).into()
    );
}

#[test]
fn loop_carries_values_through_fresh_activations() {
    let activations = Arc::new(Mutex::new(Vec::new()));
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("initial", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(0).into()])),
        )
        .unwrap();
    let seen = activations.clone();
    struct LoopKernel(Arc<Mutex<Vec<ActivationId>>>);
    impl Kernel for LoopKernel {
        fn execute(
            &self,
            context: &KernelContext<'_>,
            inputs: &[RuntimeValue],
        ) -> Result<Vec<RuntimeValue>, KernelError> {
            self.0.lock().unwrap().push(context.activation_id);
            let Some(RuntimeValue::Scalar(Value::Integer(value))) = inputs.first() else {
                return Err(KernelError::new("expected integer"));
            };
            let next = *value + 1;
            Ok(vec![
                Value::Integer(next).into(),
                Value::Bool(next < 3).into(),
            ])
        }
    }
    kernels
        .register(id("loop", KernelHandle::new), LoopKernel(seen))
        .unwrap();
    kernels
        .register(
            id("loop_continuation", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| {
                let RuntimeValue::Scalar(Value::Integer(value)) = &inputs[0] else {
                    return Err(KernelError::new("expected loop result"));
                };
                Ok(vec![Value::Integer(value + 10).into()])
            }),
        )
        .unwrap();
    let mut execution_plan = plan(
        vec![
            operation("initial", &[], &[0]),
            operation("loop", &[1], &[2, 3]),
            operation("loop_continuation", &[4], &[5]),
        ],
        6,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Region(Box::new(StructuredControlRegion::Loop {
                body: Box::new(StructuredControlRegion::Sequence(Box::new([
                    ControlStep::Operation(OperationIndex::new(1)),
                ]))),
                carried: Box::new([LoopCarriedBinding {
                    body_input: ValueRef::new(1),
                    initial_source: ValueRef::new(0),
                    next_source: ValueRef::new(2),
                    result: ValueRef::new(4),
                    production: Some(OutputProduction::FullyMaterialized),
                }]),
                continue_condition: ValueRef::new(3),
                max_iterations: 4,
            })),
            ControlStep::Operation(OperationIndex::new(2)),
        ])),
    );
    execution_plan.value_sources = Box::new([
        PlanValueSource::ControlProduced(ValueRef::new(1), OutputProduction::FullyMaterialized),
        PlanValueSource::ControlProduced(ValueRef::new(4), OutputProduction::FullyMaterialized),
    ]);
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(5),
    }]);
    publish_graph_results(&mut execution_plan);

    let result = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new())
    .unwrap();

    assert_eq!(
        result.value_for_test("result").unwrap(),
        RuntimeValue::from(Value::Integer(13))
    );
    let activations = activations.lock().unwrap();
    assert_eq!(activations.len(), 3);
    assert!(activations.windows(2).all(|pair| pair[0] != pair[1]));
}

#[test]
fn loop_does_not_reuse_an_unselected_branch_value_from_a_prior_iteration() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("initial", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(0).into()])),
        )
        .unwrap();
    kernels
        .register(
            id("selector", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| {
                let RuntimeValue::Scalar(Value::Integer(value)) = &inputs[0] else {
                    return Err(KernelError::new("expected integer"));
                };
                let next = value + 1;
                Ok(vec![
                    Value::Integer(next).into(),
                    Value::Bool(*value == 0).into(),
                    Value::Bool(next < 2).into(),
                ])
            }),
        )
        .unwrap();
    kernels
        .register(
            id("branch_value", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(41).into()])),
        )
        .unwrap();
    kernels
        .register(
            id("consume", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| Ok(vec![inputs[0].clone()])),
        )
        .unwrap();

    let mut execution_plan = plan(
        vec![
            operation("initial", &[], &[1]),
            operation("selector", &[0], &[2, 3, 4]),
            operation("branch_value", &[], &[5]),
            operation("consume", &[5], &[6]),
        ],
        8,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Region(Box::new(StructuredControlRegion::Loop {
                body: Box::new(StructuredControlRegion::Sequence(Box::new([
                    ControlStep::Operation(OperationIndex::new(1)),
                    ControlStep::Region(Box::new(StructuredControlRegion::If {
                        condition: ValueRef::new(3),
                        then_region: Box::new(StructuredControlRegion::Sequence(Box::new([
                            ControlStep::Operation(OperationIndex::new(2)),
                        ]))),
                        else_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
                        results: Box::new([]),
                    })),
                    ControlStep::Operation(OperationIndex::new(3)),
                ]))),
                carried: Box::new([LoopCarriedBinding {
                    body_input: ValueRef::new(0),
                    initial_source: ValueRef::new(1),
                    next_source: ValueRef::new(2),
                    result: ValueRef::new(7),
                    production: Some(OutputProduction::FullyMaterialized),
                }]),
                continue_condition: ValueRef::new(4),
                max_iterations: 3,
            })),
        ])),
    );
    execution_plan.value_sources = Box::new([
        PlanValueSource::ControlProduced(ValueRef::new(0), OutputProduction::FullyMaterialized),
        PlanValueSource::ControlProduced(ValueRef::new(7), OutputProduction::FullyMaterialized),
    ]);

    let error = RunExecutor::new(
        &kernels,
        &no_resources(),
        &NoFunctions,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&execution_plan, CancellationToken::new())
    .expect_err("an unselected branch must not leak a prior activation value");

    assert!(matches!(error, RunError::InvalidPlan(_)));
}
