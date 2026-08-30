use super::*;

#[test]
fn call_missing_caller_value_does_not_acquire_callee_resources() {
    let mut callee = plan(vec![], 1, StructuredControlRegion::Sequence(Box::new([])));
    callee.value_sources = Box::new([PlanValueSource::ExternalInput(
        ValueRef::new(0),
        OutputProduction::FullyMaterialized,
    )]);
    callee.resources = Box::new([CompiledResourceRequirement {
        resource: id("external/callee", ResourceId::new),
        kind: ResourceKind::ExternalArtifact,
        access: ResourceAccess::Shared,
        optional: false,
    }]);
    let published = published_function(callee, "functions/callee.yssbi-function", &[0], &[]);
    let caller = plan(
        vec![operation("source_not_in_region", &[], &[0])],
        1,
        StructuredControlRegion::Call {
            target: id("functions/callee.yssbi-function", FunctionPlanHandle::new),
            arguments: Box::new([CallArgumentBinding {
                caller_source: ValueRef::new(0),
                callee_destination: ValueRef::new(0),
            }]),
            results: Box::new([]),
            mandatory: true,
        },
    );
    let resources = no_resources();

    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &resources,
        &OneFunction(published),
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&caller, CancellationToken::new())
    .expect_err("the caller value is unavailable");

    assert!(matches!(error, RunError::InvalidPlan(_)));
    assert_eq!(resources.acquired.load(Ordering::SeqCst), 0);
}

#[test]
fn call_copies_values_across_different_caller_and_callee_layouts() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("source", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(41).into()])),
        )
        .unwrap();
    kernels
        .register(
            id("increment", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| {
                let RuntimeValue::Scalar(Value::Integer(value)) = &inputs[0] else {
                    return Err(KernelError::new("expected integer"));
                };
                Ok(vec![Value::Integer(value + 1).into()])
            }),
        )
        .unwrap();

    let mut callee = plan(
        vec![operation("increment", &[1], &[3])],
        4,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    callee.value_sources = Box::new([PlanValueSource::ExternalInput(
        ValueRef::new(1),
        OutputProduction::FullyMaterialized,
    )]);
    let mut caller = plan(
        vec![operation("source", &[], &[4])],
        5,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Region(Box::new(StructuredControlRegion::Call {
                target: id("functions/callee.yssbi-function", FunctionPlanHandle::new),
                arguments: Box::new([CallArgumentBinding {
                    caller_source: ValueRef::new(4),
                    callee_destination: ValueRef::new(1),
                }]),
                results: Box::new([CallResultBinding {
                    callee_source: ValueRef::new(3),
                    caller_destination: ValueRef::new(0),
                    production: Some(OutputProduction::FullyMaterialized),
                }]),
                mandatory: true,
            })),
        ])),
    );
    caller.value_sources = Box::new([PlanValueSource::ControlProduced(
        ValueRef::new(0),
        OutputProduction::FullyMaterialized,
    )]);
    caller.results = Box::new([PlanResult {
        name: "answer".into(),
        output: stable_output("answer"),
        value: ValueRef::new(0),
    }]);
    publish_graph_results(&mut caller);

    let function = published_function(callee, "functions/callee.yssbi-function", &[1], &[3]);
    let result = RunExecutor::new(
        &kernels,
        &no_resources(),
        &OneFunction(function),
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&caller, CancellationToken::new())
    .unwrap();

    assert_eq!(
        result.value_for_test("answer").unwrap(),
        RuntimeValue::from(Value::Integer(42))
    );
}

#[test]
fn function_call_preserves_data_series_artifact() {
    let series_contract = PlannedValueContract {
        kind: PlannedValueKind::DataSeries,
        type_expr: yss_graph_protocol::data_series_type(TypeExpr::Concrete(
            TypeId::new("core.float64").unwrap(),
        )),
    };
    let mut kernels = build_builtin_kernel_registry();
    kernels
        .register(
            id("function_series_source", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| {
                Ok(vec![RuntimeValue::Artifact(
                    DataSeriesBuilder::new(DataSeriesElementType::Float64)
                        .values([decimal("1"), Value::Null, decimal("3")])
                        .name("function payload")
                        .format("number")
                        .build(ArtifactKind::Collected)
                        .unwrap(),
                )])
            }),
        )
        .unwrap();
    kernels
        .register(
            id("function_scalar_source", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(2).into()])),
        )
        .unwrap();
    kernels
        .register(
            id("function_series_identity", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| Ok(vec![inputs[0].clone()])),
        )
        .unwrap();

    let mut identity = operation("function_series_identity", &[1], &[3]);
    identity.inputs[0].contract = series_contract.clone();
    identity.outputs[0].contract = series_contract.clone();
    identity.outputs[0].production = OutputProduction::FullyMaterialized;
    let mut callee = plan(
        vec![identity],
        4,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    callee.value_sources = Box::new([PlanValueSource::ExternalInput(
        ValueRef::new(1),
        OutputProduction::FullyMaterialized,
    )]);
    callee
        .value_contracts
        .insert(ValueRef::new(1), series_contract.clone());
    callee
        .value_contracts
        .insert(ValueRef::new(3), series_contract.clone());

    let mut source = operation("function_series_source", &[], &[4]);
    source.outputs[0].contract = series_contract.clone();
    source.outputs[0].production = OutputProduction::FullyMaterialized;
    let scalar = operation("function_scalar_source", &[], &[5]);
    let mut math = operation("yssbi.numeric.series.add", &[0, 5], &[6]);
    math.inputs[0].contract = series_contract.clone();
    math.outputs[0].contract = series_contract.clone();
    math.outputs[0].production = OutputProduction::FullyMaterialized;
    let mut caller = plan(
        vec![source, scalar, math],
        7,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(1)),
            ControlStep::Region(Box::new(StructuredControlRegion::Call {
                target: id(
                    "functions/data-series.yssbi-function",
                    FunctionPlanHandle::new,
                ),
                arguments: Box::new([CallArgumentBinding {
                    caller_source: ValueRef::new(4),
                    callee_destination: ValueRef::new(1),
                }]),
                results: Box::new([CallResultBinding {
                    callee_source: ValueRef::new(3),
                    caller_destination: ValueRef::new(0),
                    production: Some(OutputProduction::FullyMaterialized),
                }]),
                mandatory: true,
            })),
            ControlStep::Operation(OperationIndex::new(2)),
        ])),
    );
    caller.value_sources = Box::new([PlanValueSource::ControlProduced(
        ValueRef::new(0),
        OutputProduction::FullyMaterialized,
    )]);
    for value in [0, 4, 6] {
        caller
            .value_contracts
            .insert(ValueRef::new(value), series_contract.clone());
    }
    caller.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(6),
    }]);
    publish_graph_results(&mut caller);

    let mut function =
        published_function(callee, "functions/data-series.yssbi-function", &[1], &[3]);
    let published = Arc::make_mut(&mut function);
    Arc::make_mut(&mut published.abi)
        .parameter_contracts
        .values_mut()
        .for_each(|contract| *contract = series_contract.clone());
    Arc::make_mut(&mut published.abi)
        .result_contracts
        .values_mut()
        .for_each(|contract| *contract = series_contract.clone());
    let result = RunExecutor::new(
        &kernels,
        &no_resources(),
        &OneFunction(function),
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&caller, CancellationToken::new())
    .unwrap();
    let result_value = result.value_for_test("result").unwrap();
    let artifact = require_data_series(&result_value).unwrap();
    let metadata = artifact.data_series_metadata().unwrap();

    assert_eq!(metadata.element_type, DataSeriesElementType::Float64);
    assert_eq!(metadata.length, 3);
    assert_eq!(metadata.null_count, 1);
    assert_eq!(metadata.name.as_deref(), Some("function payload"));
    assert_eq!(metadata.format.as_deref(), Some("number"));
    assert_eq!(
        artifact
            .cursor()
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        [decimal("3"), Value::Null, decimal("5")]
    );
}

#[test]
fn call_rejects_stale_published_abi_before_entering_the_callee_frame() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("callee", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(vec![])
            }),
        )
        .unwrap();
    let mut callee = plan(
        vec![operation("callee", &[], &[])],
        0,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    callee.provenance.graph_path =
        GraphResourcePath::new("functions/callee.yssbi-function").unwrap();
    let mut stale_provenance = callee.provenance.clone();
    stale_provenance.compile_id = CompileId::new(999);
    let published = Arc::new(PublishedFunctionPlan {
        plan: Arc::new(callee),
        abi: Arc::new(FunctionPlanAbi {
            provenance: stale_provenance,
            parameters: BTreeMap::new(),
            parameter_contracts: BTreeMap::new(),
            results: BTreeMap::new(),
            result_productions: BTreeMap::new(),
            result_contracts: BTreeMap::new(),
        }),
    });
    let caller = plan(
        vec![],
        0,
        StructuredControlRegion::Call {
            target: id("functions/callee.yssbi-function", FunctionPlanHandle::new),
            arguments: Box::new([]),
            results: Box::new([]),
            mandatory: true,
        },
    );

    let error = RunExecutor::new(
        &kernels,
        &no_resources(),
        &OneFunction(published),
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&caller, CancellationToken::new())
    .unwrap_err();

    assert!(matches!(error, RunError::FunctionPlanFailed(_)));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn call_preflight_rejects_invalid_public_bindings_before_callee_side_effects() {
    #[derive(Clone, Copy)]
    enum InvalidCall {
        MissingArgument,
        MissingResult,
        DuplicateCalleeArgument,
        DuplicateCalleeResult,
        DuplicateCallerResult,
        OutOfBoundsParameter,
        UnsourcedResult,
        StaleResultProduction,
    }

    for case in [
        InvalidCall::MissingArgument,
        InvalidCall::MissingResult,
        InvalidCall::DuplicateCalleeArgument,
        InvalidCall::DuplicateCalleeResult,
        InvalidCall::DuplicateCallerResult,
        InvalidCall::OutOfBoundsParameter,
        InvalidCall::UnsourcedResult,
        InvalidCall::StaleResultProduction,
    ] {
        let calls = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&calls);
        let mut kernels = KernelRegistry::new();
        kernels
            .register(
                id("source", KernelHandle::new),
                FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(7).into()])),
            )
            .unwrap();
        kernels
            .register(
                id("callee_preflight", KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| {
                    observed.fetch_add(1, Ordering::SeqCst);
                    Ok(vec![Value::Integer(8).into()])
                }),
            )
            .unwrap();

        let mut callee = plan(
            vec![operation("callee_preflight", &[], &[2])],
            4,
            StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(
                OperationIndex::new(0),
            )])),
        );
        callee.value_sources = Box::new([PlanValueSource::ExternalInput(
            ValueRef::new(0),
            OutputProduction::FullyMaterialized,
        )]);
        callee.resources = Box::new([CompiledResourceRequirement {
            resource: id("external/callee", ResourceId::new),
            kind: ResourceKind::ExternalArtifact,
            access: ResourceAccess::Shared,
            optional: false,
        }]);
        if matches!(case, InvalidCall::StaleResultProduction) {
            callee.operations[0].outputs[0].production = OutputProduction::Streaming;
        }
        let mut published =
            published_function(callee, "functions/callee.yssbi-function", &[0], &[2]);

        let standard_argument = CallArgumentBinding {
            caller_source: ValueRef::new(0),
            callee_destination: ValueRef::new(0),
        };
        let standard_result = CallResultBinding {
            callee_source: ValueRef::new(2),
            caller_destination: ValueRef::new(1),
            production: Some(OutputProduction::FullyMaterialized),
        };
        let (arguments, results) = match case {
            InvalidCall::MissingArgument => (vec![], vec![standard_result]),
            InvalidCall::MissingResult => (vec![standard_argument], vec![]),
            InvalidCall::DuplicateCalleeArgument => (
                vec![standard_argument, standard_argument],
                vec![standard_result],
            ),
            InvalidCall::DuplicateCalleeResult => (
                vec![standard_argument],
                vec![
                    standard_result,
                    CallResultBinding {
                        callee_source: ValueRef::new(2),
                        caller_destination: ValueRef::new(3),
                        production: Some(OutputProduction::FullyMaterialized),
                    },
                ],
            ),
            InvalidCall::DuplicateCallerResult => (
                vec![standard_argument],
                vec![standard_result, standard_result],
            ),
            InvalidCall::OutOfBoundsParameter => {
                let published_mut = Arc::make_mut(&mut published);
                Arc::make_mut(&mut published_mut.abi).parameters =
                    BTreeMap::from([(FunctionParameterId::new("parameter-0"), ValueRef::new(9))]);
                (
                    vec![CallArgumentBinding {
                        caller_source: ValueRef::new(0),
                        callee_destination: ValueRef::new(9),
                    }],
                    vec![standard_result],
                )
            }
            InvalidCall::UnsourcedResult => {
                let published_mut = Arc::make_mut(&mut published);
                Arc::make_mut(&mut published_mut.abi).results =
                    BTreeMap::from([(FunctionParameterId::new("result-0"), ValueRef::new(3))]);
                (
                    vec![standard_argument],
                    vec![CallResultBinding {
                        callee_source: ValueRef::new(3),
                        caller_destination: ValueRef::new(1),
                        production: Some(OutputProduction::FullyMaterialized),
                    }],
                )
            }
            InvalidCall::StaleResultProduction => {
                let published_mut = Arc::make_mut(&mut published);
                Arc::make_mut(&mut published_mut.abi).result_productions = BTreeMap::from([(
                    FunctionParameterId::new("result-0"),
                    OutputProduction::Streaming,
                )]);
                (vec![standard_argument], vec![standard_result])
            }
        };
        let mut caller = plan(
            vec![operation("source", &[], &[0])],
            4,
            StructuredControlRegion::Sequence(Box::new([
                ControlStep::Operation(OperationIndex::new(0)),
                ControlStep::Region(Box::new(StructuredControlRegion::Call {
                    target: id("functions/callee.yssbi-function", FunctionPlanHandle::new),
                    arguments: arguments.into_boxed_slice(),
                    results: results.into_boxed_slice(),
                    mandatory: true,
                })),
            ])),
        );
        let mut destinations = BTreeMap::new();
        if !matches!(case, InvalidCall::MissingResult) {
            destinations.insert(
                ValueRef::new(1),
                PlanValueSource::ControlProduced(
                    ValueRef::new(1),
                    OutputProduction::FullyMaterialized,
                ),
            );
        }
        if matches!(case, InvalidCall::DuplicateCalleeResult) {
            destinations.insert(
                ValueRef::new(3),
                PlanValueSource::ControlProduced(
                    ValueRef::new(3),
                    OutputProduction::FullyMaterialized,
                ),
            );
        }
        caller.value_sources = destinations
            .into_values()
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let resources = no_resources();

        let error = RunExecutor::new(
            &kernels,
            &resources,
            &OneFunction(published),
            crate::node_system::runtime::ResultStore::new(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
        )
        .run(&caller, CancellationToken::new())
        .expect_err("invalid public Call bindings must fail preflight");

        assert!(matches!(
            error,
            RunError::FunctionPlanFailed(_) | RunError::InvalidPlan(_)
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(resources.acquired.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn call_preflight_allows_reusing_one_caller_source_for_distinct_parameters() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("source", KernelHandle::new),
            FnKernel(|_: &[RuntimeValue]| Ok(vec![Value::Integer(7).into()])),
        )
        .unwrap();
    kernels
        .register(
            id("callee_fan_in", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(vec![])
            }),
        )
        .unwrap();
    let mut callee = plan(
        vec![operation("callee_fan_in", &[], &[])],
        2,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    callee.value_sources = Box::new([
        PlanValueSource::ExternalInput(ValueRef::new(0), OutputProduction::FullyMaterialized),
        PlanValueSource::ExternalInput(ValueRef::new(1), OutputProduction::FullyMaterialized),
    ]);
    let published = published_function(callee, "functions/callee.yssbi-function", &[0, 1], &[]);
    let caller = plan(
        vec![operation("source", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Region(Box::new(StructuredControlRegion::Call {
                target: id("functions/callee.yssbi-function", FunctionPlanHandle::new),
                arguments: Box::new([
                    CallArgumentBinding {
                        caller_source: ValueRef::new(0),
                        callee_destination: ValueRef::new(0),
                    },
                    CallArgumentBinding {
                        caller_source: ValueRef::new(0),
                        callee_destination: ValueRef::new(1),
                    },
                ]),
                results: Box::new([]),
                mandatory: true,
            })),
        ])),
    );

    RunExecutor::new(
        &kernels,
        &no_resources(),
        &OneFunction(published),
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&caller, CancellationToken::new())
    .unwrap();

    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn call_uses_an_independent_frame() {
    struct ContextKernel(Arc<Mutex<Vec<FrameId>>>);
    impl Kernel for ContextKernel {
        fn execute(
            &self,
            context: &KernelContext<'_>,
            _: &[RuntimeValue],
        ) -> Result<Vec<RuntimeValue>, KernelError> {
            self.0.lock().unwrap().push(context.frame_id);
            Ok(vec![Value::Null.into()])
        }
    }

    let frames = Arc::new(Mutex::new(Vec::new()));
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("context", KernelHandle::new),
            ContextKernel(frames.clone()),
        )
        .unwrap();
    let callee = Arc::new(plan(
        vec![operation("context", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    ));
    let caller = plan(
        vec![operation("context", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Region(Box::new(StructuredControlRegion::Call {
                target: id("functions/callee.yssbi-function", FunctionPlanHandle::new),
                arguments: Box::new([]),
                results: Box::new([]),
                mandatory: true,
            })),
        ])),
    );

    RunExecutor::new(
        &kernels,
        &no_resources(),
        &OneFunction(published_function(
            Arc::unwrap_or_clone(callee),
            "functions/callee.yssbi-function",
            &[],
            &[],
        )),
        ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&caller, CancellationToken::new())
    .unwrap();

    let frames = frames.lock().unwrap();
    assert_eq!(frames.len(), 2);
    assert_ne!(frames[0], frames[1]);
}

#[test]
fn reversed_two_function_publication_is_equivalent_and_callable() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("function_a", KernelHandle::new),
            FnKernel(move |_: &[RuntimeValue]| {
                observed.fetch_add(1, Ordering::SeqCst);
                Ok(vec![])
            }),
        )
        .unwrap();

    let versions = BTreeMap::from([
        (
            ResourceKey::new("functions/a.yssbi-function"),
            ResourceVersion::new("a-v1"),
        ),
        (
            ResourceKey::new("functions/b.yssbi-function"),
            ResourceVersion::new("b-v1"),
        ),
    ]);
    let make_function = |path: &str, root_region: StructuredControlRegion, operations| {
        let mut function = plan(operations, 0, root_region);
        function.provenance.graph_path = GraphResourcePath::new(path).unwrap();
        function.provenance.basis.resource_versions = versions.clone();
        let abi = FunctionPlanAbi {
            provenance: function.provenance.clone(),
            parameters: BTreeMap::new(),
            parameter_contracts: BTreeMap::new(),
            results: BTreeMap::new(),
            result_productions: BTreeMap::new(),
            result_contracts: BTreeMap::new(),
        };
        (Arc::new(function), Arc::new(abi))
    };
    let (plan_a, abi_a) = make_function(
        "functions/a.yssbi-function",
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
        vec![operation("function_a", &[], &[])],
    );
    let (plan_b, abi_b) = make_function(
        "functions/b.yssbi-function",
        StructuredControlRegion::Call {
            target: id("functions/a.yssbi-function", FunctionPlanHandle::new),
            arguments: Box::new([]),
            results: Box::new([]),
            mandatory: true,
        },
        vec![],
    );
    let entries = vec![
        (
            GraphResourcePath::new("functions/a.yssbi-function").unwrap(),
            ResourceVersion::new("a-v1"),
            plan_a,
            abi_a,
        ),
        (
            GraphResourcePath::new("functions/b.yssbi-function").unwrap(),
            ResourceVersion::new("b-v1"),
            plan_b,
            abi_b,
        ),
    ];
    let store = FunctionPlanStore::new(ProjectSessionId::new("test-session"), 64);
    let forward = store
        .generation(
            RegistryFingerprint::from_bytes([1; 32]),
            versions.clone(),
            entries.clone(),
        )
        .unwrap();
    let reverse = store
        .generation(
            RegistryFingerprint::from_bytes([1; 32]),
            versions.clone(),
            entries.into_iter().rev().collect(),
        )
        .unwrap();
    let mut caller = plan(
        vec![],
        0,
        StructuredControlRegion::Call {
            target: id("functions/b.yssbi-function", FunctionPlanHandle::new),
            arguments: Box::new([]),
            results: Box::new([]),
            mandatory: true,
        },
    );
    caller.provenance.basis.resource_versions = versions;

    RunExecutor::new(
        &kernels,
        &no_resources(),
        &forward,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&caller, CancellationToken::new())
    .unwrap();
    RunExecutor::new(
        &kernels,
        &no_resources(),
        &reverse,
        crate::node_system::runtime::ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&caller, CancellationToken::new())
    .unwrap();

    assert_eq!(forward.plan_count(), reverse.plan_count());
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[test]
fn recursive_calls_stop_at_the_configured_limit() {
    let recursive = plan(
        vec![],
        0,
        StructuredControlRegion::Call {
            target: id(
                "functions/recursive.yssbi-function",
                FunctionPlanHandle::new,
            ),
            arguments: Box::new([]),
            results: Box::new([]),
            mandatory: true,
        },
    );
    let recursive = published_function(recursive, "functions/recursive.yssbi-function", &[], &[]);
    let resources = no_resources();

    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &resources,
        &OneFunction(Arc::clone(&recursive)),
        ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .with_recursion_limit(3)
    .run(recursive.plan.as_ref(), CancellationToken::new())
    .unwrap_err();

    assert_eq!(
        error,
        RunError::RecursionLimitExceeded { recursion_limit: 3 }
    );
}

#[test]
fn call_failure_releases_caller_and_callee_resources() {
    let mut callee = plan(
        vec![operation("missing", &[], &[0])],
        1,
        StructuredControlRegion::Sequence(Box::new([ControlStep::Operation(OperationIndex::new(
            0,
        ))])),
    );
    callee.resources = Box::new([requirement("callee")]);
    let mut caller = plan(
        vec![],
        0,
        StructuredControlRegion::Call {
            target: id("functions/callee.yssbi-function", FunctionPlanHandle::new),
            arguments: Box::new([]),
            results: Box::new([]),
            mandatory: true,
        },
    );
    caller.resources = Box::new([requirement("caller")]);
    let resources = no_resources();
    let released = resources.released.clone();

    let error = RunExecutor::new(
        &KernelRegistry::new(),
        &resources,
        &OneFunction(published_function(
            callee,
            "functions/callee.yssbi-function",
            &[],
            &[],
        )),
        ResultStore::new(),
        std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
    )
    .run(&caller, CancellationToken::new())
    .unwrap_err();

    assert!(matches!(error, RunError::KernelNotFound(_)));
    assert_eq!(released.load(Ordering::SeqCst), 2);
}

#[test]
fn external_stream_fanout_before_branch_executes_once_and_delivers_complete_data() {
    for selected in [true, false] {
        let source_executions = Arc::new(AtomicUsize::new(0));
        let observed = Arc::clone(&source_executions);
        let mut kernels = KernelRegistry::new();
        kernels
            .register(
                id("external_stream_source", KernelHandle::new),
                OwnedStreamKernel {
                    values: vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)]
                        .into_boxed_slice(),
                    executions: Some(observed),
                },
            )
            .unwrap();
        kernels
            .register(
                id("branch_condition", KernelHandle::new),
                FnKernel(move |_: &[RuntimeValue]| Ok(vec![Value::Bool(selected).into()])),
            )
            .unwrap();
        for name in ["then_stream_sink", "else_stream_sink"] {
            kernels
                .register(
                    id(name, KernelHandle::new),
                    FnKernel(|inputs: &[RuntimeValue]| {
                        let RuntimeValue::Artifact(artifact) = &inputs[0] else {
                            return Err(KernelError::new("expected materialized artifact"));
                        };
                        Ok(vec![
                            Value::Integer(artifact.cursor().unwrap().count() as i64).into(),
                        ])
                    }),
                )
                .unwrap();
        }

        let condition = operation("branch_condition", &[], &[1]);
        let shared = adapter_operation(
            "external.shared.collect",
            2,
            3,
            OutputProduction::Streaming,
            InputConsumption::FullyMaterialized,
        );
        let mut callee = plan(
            vec![
                condition,
                shared,
                operation("then_stream_sink", &[4], &[5]),
                operation("else_stream_sink", &[6], &[7]),
            ],
            9,
            StructuredControlRegion::Sequence(Box::new([
                ControlStep::Operation(OperationIndex::new(1)),
                ControlStep::Operation(OperationIndex::new(0)),
                ControlStep::Region(Box::new(StructuredControlRegion::If {
                    condition: ValueRef::new(1),
                    then_region: Box::new(StructuredControlRegion::Sequence(Box::new([
                        ControlStep::Operation(OperationIndex::new(2)),
                    ]))),
                    else_region: Box::new(StructuredControlRegion::Sequence(Box::new([
                        ControlStep::Operation(OperationIndex::new(3)),
                    ]))),
                    results: Box::new([BranchResultBinding {
                        destination: ValueRef::new(8),
                        then_source: ValueRef::new(5),
                        else_source: ValueRef::new(7),
                        production: Some(OutputProduction::FullyMaterialized),
                    }]),
                })),
            ])),
        );
        callee.value_sources = Box::new([
            PlanValueSource::ExternalInput(ValueRef::new(0), OutputProduction::Streaming),
            PlanValueSource::ControlProduced(ValueRef::new(8), OutputProduction::FullyMaterialized),
        ]);
        callee.value_dependencies = Box::new([
            ValueDependency {
                source: ValueRef::new(0),
                destination: ValueRef::new(2),
            },
            ValueDependency {
                source: ValueRef::new(3),
                destination: ValueRef::new(4),
            },
            ValueDependency {
                source: ValueRef::new(3),
                destination: ValueRef::new(6),
            },
        ]);

        let mut source = operation("external_stream_source", &[], &[0]);
        source.outputs[0].production = OutputProduction::Streaming;
        let mut caller = plan(
            vec![source],
            2,
            StructuredControlRegion::Sequence(Box::new([
                ControlStep::Operation(OperationIndex::new(0)),
                ControlStep::Region(Box::new(StructuredControlRegion::Call {
                    target: id(
                        "functions/external-branch.yssbi-function",
                        FunctionPlanHandle::new,
                    ),
                    arguments: Box::new([CallArgumentBinding {
                        caller_source: ValueRef::new(0),
                        callee_destination: ValueRef::new(0),
                    }]),
                    results: Box::new([CallResultBinding {
                        callee_source: ValueRef::new(8),
                        caller_destination: ValueRef::new(1),
                        production: Some(OutputProduction::FullyMaterialized),
                    }]),
                    mandatory: true,
                })),
            ])),
        );
        caller.value_sources = Box::new([PlanValueSource::ControlProduced(
            ValueRef::new(1),
            OutputProduction::FullyMaterialized,
        )]);
        caller.results = Box::new([PlanResult {
            name: "count".into(),
            output: stable_output("count"),
            value: ValueRef::new(1),
        }]);
        publish_graph_results(&mut caller);

        let function = published_function(
            callee,
            "functions/external-branch.yssbi-function",
            &[0],
            &[8],
        );
        let result = RunExecutor::new(
            &kernels,
            &no_resources(),
            &OneFunction(function),
            crate::node_system::runtime::ResultStore::new(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
        )
        .run(&caller, CancellationToken::new())
        .unwrap();

        assert_eq!(
            result.value_for_test("count").unwrap(),
            Value::Integer(3).into()
        );
        assert_eq!(source_executions.load(Ordering::SeqCst), 1);
    }
}
