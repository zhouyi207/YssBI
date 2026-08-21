use super::*;

#[test]
fn bounded_channel_applies_backpressure() {
    let (sender, receiver) = bounded_stream_channel(1, CancellationToken::new()).unwrap();

    sender.try_send(Value::Integer(1)).unwrap();
    assert_eq!(
        sender.try_send(Value::Integer(2)),
        Err(StreamSendError::Full(Value::Integer(2)))
    );
    assert_eq!(receiver.recv().unwrap(), Value::Integer(1));
    sender.try_send(Value::Integer(2)).unwrap();
    assert_eq!(receiver.recv().unwrap(), Value::Integer(2));
}

#[test]
fn scheduler_executes_only_the_planned_materialization_adapter() {
    let mut kernels = KernelRegistry::new();
    kernels
        .register(
            id("adapter_source", KernelHandle::new),
            OwnedStreamKernel {
                values: vec![Value::Integer(7), Value::Integer(8)].into_boxed_slice(),
                executions: None,
            },
        )
        .unwrap();
    kernels
        .register(
            id("adapter_sink", KernelHandle::new),
            FnKernel(|inputs: &[RuntimeValue]| {
                assert!(matches!(
                    inputs,
                    [RuntimeValue::Artifact(artifact)]
                        if artifact.kind() == ArtifactKind::Collected
                            && artifact.cursor().unwrap().collect::<Result<Vec<_>, _>>().unwrap()
                                == [Value::Integer(7), Value::Integer(8)]
                ));
                Ok(vec![Value::Integer(2).into()])
            }),
        )
        .unwrap();

    let mut source = operation("adapter_source", &[], &[0]);
    source.outputs[0].production = OutputProduction::Streaming;
    let mut sink = operation("adapter_sink", &[3], &[4]);
    sink.inputs[0].consumption = InputConsumption::FullyMaterialized;
    let adapter = PlannedOperation {
        stable_id: OperationStableId::new("test.operation.adapter.collect").unwrap(),
        source_node_id: NodeId::from_uuid(uuid::Uuid::nil()),
        source_node_type_id: NodeTypeId::new("yssbi.test.adapter.collect").unwrap(),
        kernel: PlannedKernel::Adapter(PlannedAdapter::Collect {
            limits: MaterializationLimits {
                max_values: 1_000_000,
                max_bytes: 64 * 1024 * 1024,
            },
        }),
        inputs: Box::new([PlannedInput {
            value: ValueRef::new(1),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            consumption: InputConsumption::Streaming,
            bound_value: None,
        }]),
        outputs: Box::new([PlannedOutput {
            value: ValueRef::new(2),
            contract: crate::node_system::plan::PlannedValueContract::opaque(),
            production: OutputProduction::FullyMaterialized,
            public_output: None,
            presentation: crate::node_system::plan::ResultPresentation::Inspector,
        }]),
        params: id("adapter.none", CompiledParameterHandle::new),
        resource_dependencies: Box::new([]),
        cache_policy: CachePolicy::Disabled,
        semantics_version: ExecutionSemanticsVersion::from_bytes([9; 32]),
        workload: WorkloadClass::AdapterIo,
        retry: PlannedRetry::default(),
    };
    let mut execution_plan = plan(
        vec![source, sink, adapter],
        5,
        StructuredControlRegion::Sequence(Box::new([
            ControlStep::Operation(OperationIndex::new(0)),
            ControlStep::Operation(OperationIndex::new(2)),
            ControlStep::Operation(OperationIndex::new(1)),
        ])),
    );
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
    execution_plan.results = Box::new([PlanResult {
        name: "result".into(),
        output: stable_output("result"),
        value: ValueRef::new(4),
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
        RuntimeValue::from(Value::Integer(2))
    );
}

#[test]
fn shared_materialized_fanout_delivers_complete_data_to_same_and_different_consumers() {
    for different_contracts in [false, true] {
        let observed = Arc::new(Mutex::new(Vec::new()));
        let mut kernels = KernelRegistry::new();
        kernels
            .register(
                id("fanout_source", KernelHandle::new),
                OwnedStreamKernel {
                    values: vec![Value::Integer(1), Value::Integer(2), Value::Integer(3)]
                        .into_boxed_slice(),
                    executions: None,
                },
            )
            .unwrap();
        for name in ["fanout_a", "fanout_b"] {
            let observed = Arc::clone(&observed);
            kernels
                .register(
                    id(name, KernelHandle::new),
                    FnKernel(move |inputs: &[RuntimeValue]| {
                        let count = match &inputs[0] {
                            RuntimeValue::Artifact(artifact) => artifact.cursor().unwrap().count(),
                            RuntimeValue::Stream(stream) => {
                                let mut count = 0;
                                while stream.recv().is_ok() {
                                    count += 1;
                                }
                                count
                            }
                            RuntimeValue::Scalar(_) => 1,
                        };
                        observed.lock().unwrap().push(count);
                        Ok(vec![Value::Integer(count as i64).into()])
                    }),
                )
                .unwrap();
        }
        let mut source = operation("fanout_source", &[], &[0]);
        source.outputs[0].production = OutputProduction::Streaming;
        let mut sink_a = operation("fanout_a", &[7], &[8]);
        sink_a.inputs[0].consumption = if different_contracts {
            InputConsumption::Streaming
        } else {
            InputConsumption::FullyMaterialized
        };
        let sink_b = operation("fanout_b", &[9], &[10]);
        let adapter_operation =
            |stable: &str,
             adapter: PlannedAdapter,
             input: u32,
             output: u32,
             consumption: InputConsumption,
             production: OutputProduction| PlannedOperation {
                stable_id: OperationStableId::new(stable).unwrap(),
                source_node_id: NodeId::from_uuid(uuid::Uuid::nil()),
                source_node_type_id: NodeTypeId::new("yssbi.test.fanout_adapter").unwrap(),
                kernel: PlannedKernel::Adapter(adapter),
                inputs: Box::new([PlannedInput {
                    value: ValueRef::new(input),
                    contract: crate::node_system::plan::PlannedValueContract::opaque(),
                    consumption,
                    bound_value: None,
                }]),
                outputs: Box::new([PlannedOutput {
                    value: ValueRef::new(output),
                    contract: crate::node_system::plan::PlannedValueContract::opaque(),
                    production,
                    public_output: None,
                    presentation: crate::node_system::plan::ResultPresentation::Inspector,
                }]),
                params: id("adapter.fanout", CompiledParameterHandle::new),
                resource_dependencies: Box::new([]),
                cache_policy: CachePolicy::Disabled,
                semantics_version: ExecutionSemanticsVersion::from_bytes([7; 32]),
                workload: WorkloadClass::AdapterIo,
                retry: PlannedRetry::default(),
            };
        let shared = adapter_operation(
            "fanout.shared",
            PlannedAdapter::Collect {
                limits: MaterializationLimits {
                    max_values: 1_000_000,
                    max_bytes: 64 * 1024 * 1024,
                },
            },
            1,
            2,
            InputConsumption::Streaming,
            OutputProduction::FullyMaterialized,
        );
        let (operations, steps, dependencies) = if different_contracts {
            let adapter_a = adapter_operation(
                "fanout.adapter.a",
                PlannedAdapter::StreamBridge {
                    format: StreamFormat::Native,
                },
                3,
                4,
                InputConsumption::FullyMaterialized,
                OutputProduction::Streaming,
            );
            (
                vec![source, sink_a, sink_b, shared, adapter_a],
                vec![
                    ControlStep::Operation(OperationIndex::new(0)),
                    ControlStep::Operation(OperationIndex::new(3)),
                    ControlStep::Operation(OperationIndex::new(4)),
                    ControlStep::Operation(OperationIndex::new(1)),
                    ControlStep::Operation(OperationIndex::new(2)),
                ],
                vec![
                    ValueDependency {
                        source: ValueRef::new(0),
                        destination: ValueRef::new(1),
                    },
                    ValueDependency {
                        source: ValueRef::new(2),
                        destination: ValueRef::new(3),
                    },
                    ValueDependency {
                        source: ValueRef::new(4),
                        destination: ValueRef::new(7),
                    },
                    ValueDependency {
                        source: ValueRef::new(2),
                        destination: ValueRef::new(9),
                    },
                ],
            )
        } else {
            (
                vec![source, sink_a, sink_b, shared],
                vec![
                    ControlStep::Operation(OperationIndex::new(0)),
                    ControlStep::Operation(OperationIndex::new(3)),
                    ControlStep::Operation(OperationIndex::new(1)),
                    ControlStep::Operation(OperationIndex::new(2)),
                ],
                vec![
                    ValueDependency {
                        source: ValueRef::new(0),
                        destination: ValueRef::new(1),
                    },
                    ValueDependency {
                        source: ValueRef::new(2),
                        destination: ValueRef::new(7),
                    },
                    ValueDependency {
                        source: ValueRef::new(2),
                        destination: ValueRef::new(9),
                    },
                ],
            )
        };
        let mut execution_plan = plan(
            operations,
            11,
            StructuredControlRegion::Sequence(steps.into_boxed_slice()),
        );
        execution_plan.value_dependencies = dependencies.into_boxed_slice();
        execution_plan.results = Box::new([
            PlanResult {
                name: "a".into(),
                output: stable_output("a"),
                value: ValueRef::new(8),
            },
            PlanResult {
                name: "b".into(),
                output: stable_output("b"),
                value: ValueRef::new(10),
            },
        ]);
        publish_graph_results(&mut execution_plan);

        RunExecutor::new(
            &kernels,
            &no_resources(),
            &NoFunctions,
            crate::node_system::runtime::ResultStore::new(),
            std::sync::Arc::new(crate::node_system::runtime::SessionMemoization::new()),
        )
        .run(&execution_plan, CancellationToken::new())
        .unwrap();
        let mut counts = observed.lock().unwrap().clone();
        counts.sort();
        assert_eq!(counts, vec![3, 3]);
    }
}

#[test]
fn materialization_matrix_represents_and_executes_all_fifteen_contract_cells() {
    let stream_owner = materialization_test_owner();
    #[derive(Clone, Copy)]
    enum Shape {
        Stream,
        Artifact(ArtifactKind),
    }

    let stream_bridge = Some(PlannedAdapter::StreamBridge {
        format: StreamFormat::Native,
    });
    let buffer = Some(PlannedAdapter::Buffer { capacity: 64 });
    let collect = Some(PlannedAdapter::Collect {
        limits: MaterializationLimits {
            max_values: 1_000_000,
            max_bytes: 64 * 1024 * 1024,
        },
    });
    let spill = Some(PlannedAdapter::Spill {
        memory_limit_bytes: 64 * 1024 * 1024,
    });
    let cases = [
        (
            OutputProduction::Streaming,
            InputConsumption::Streaming,
            None,
            InputConsumption::Streaming,
            OutputProduction::Streaming,
            Shape::Stream,
        ),
        (
            OutputProduction::Streaming,
            InputConsumption::SinglePassBatches,
            buffer,
            InputConsumption::Streaming,
            OutputProduction::Batches,
            Shape::Artifact(ArtifactKind::Buffered),
        ),
        (
            OutputProduction::Streaming,
            InputConsumption::RewindableBatches,
            collect.clone(),
            InputConsumption::Streaming,
            OutputProduction::Batches,
            Shape::Artifact(ArtifactKind::Collected),
        ),
        (
            OutputProduction::Streaming,
            InputConsumption::RandomAccess,
            spill.clone(),
            InputConsumption::Streaming,
            OutputProduction::FullyMaterialized,
            Shape::Artifact(ArtifactKind::Spilled),
        ),
        (
            OutputProduction::Streaming,
            InputConsumption::FullyMaterialized,
            collect.clone(),
            InputConsumption::Streaming,
            OutputProduction::FullyMaterialized,
            Shape::Artifact(ArtifactKind::Collected),
        ),
        (
            OutputProduction::Batches,
            InputConsumption::Streaming,
            stream_bridge.clone(),
            InputConsumption::SinglePassBatches,
            OutputProduction::Streaming,
            Shape::Stream,
        ),
        (
            OutputProduction::Batches,
            InputConsumption::SinglePassBatches,
            None,
            InputConsumption::SinglePassBatches,
            OutputProduction::Batches,
            Shape::Artifact(ArtifactKind::Buffered),
        ),
        (
            OutputProduction::Batches,
            InputConsumption::RewindableBatches,
            None,
            InputConsumption::SinglePassBatches,
            OutputProduction::Batches,
            Shape::Artifact(ArtifactKind::Buffered),
        ),
        (
            OutputProduction::Batches,
            InputConsumption::RandomAccess,
            spill,
            InputConsumption::SinglePassBatches,
            OutputProduction::FullyMaterialized,
            Shape::Artifact(ArtifactKind::Spilled),
        ),
        (
            OutputProduction::Batches,
            InputConsumption::FullyMaterialized,
            collect,
            InputConsumption::SinglePassBatches,
            OutputProduction::FullyMaterialized,
            Shape::Artifact(ArtifactKind::Collected),
        ),
        (
            OutputProduction::FullyMaterialized,
            InputConsumption::Streaming,
            stream_bridge,
            InputConsumption::FullyMaterialized,
            OutputProduction::Streaming,
            Shape::Stream,
        ),
        (
            OutputProduction::FullyMaterialized,
            InputConsumption::SinglePassBatches,
            None,
            InputConsumption::FullyMaterialized,
            OutputProduction::FullyMaterialized,
            Shape::Artifact(ArtifactKind::Collected),
        ),
        (
            OutputProduction::FullyMaterialized,
            InputConsumption::RewindableBatches,
            None,
            InputConsumption::FullyMaterialized,
            OutputProduction::FullyMaterialized,
            Shape::Artifact(ArtifactKind::Collected),
        ),
        (
            OutputProduction::FullyMaterialized,
            InputConsumption::RandomAccess,
            None,
            InputConsumption::FullyMaterialized,
            OutputProduction::FullyMaterialized,
            Shape::Artifact(ArtifactKind::Collected),
        ),
        (
            OutputProduction::FullyMaterialized,
            InputConsumption::FullyMaterialized,
            None,
            InputConsumption::FullyMaterialized,
            OutputProduction::FullyMaterialized,
            Shape::Artifact(ArtifactKind::Collected),
        ),
    ];

    for (production, consumption, adapter, adapter_consumption, adapter_production, shape) in cases
    {
        let planned = MaterializationAdapterPlan::for_contract(production, consumption);
        assert_eq!(planned.adapter, adapter);
        assert_eq!(planned.input_consumption, adapter_consumption);
        assert_eq!(planned.output_production, adapter_production);
        let input = match production {
            OutputProduction::Streaming => RuntimeValue::Stream(
                stream_owner
                    .stream_from_values([Value::Integer(7)])
                    .unwrap(),
            ),
            OutputProduction::Batches => RuntimeValue::Artifact(Artifact::new(
                ArtifactKind::Buffered,
                vec![Value::Integer(7)],
            )),
            OutputProduction::FullyMaterialized => RuntimeValue::Artifact(Artifact::new(
                ArtifactKind::Collected,
                vec![Value::Integer(7)],
            )),
        };
        let cancellation = CancellationToken::new();
        let output = match planned.adapter.as_ref() {
            Some(adapter) => {
                execute_planned_adapter(adapter, input, stream_owner.as_ref(), &cancellation)
                    .unwrap()
            }
            None => input,
        };
        match (shape, output) {
            (Shape::Stream, RuntimeValue::Stream(_)) => {}
            (Shape::Artifact(expected), RuntimeValue::Artifact(actual)) => {
                assert_eq!(actual.kind(), expected);
                assert_eq!(
                    actual
                        .cursor()
                        .unwrap()
                        .collect::<Result<Vec<_>, _>>()
                        .unwrap(),
                    [Value::Integer(7)]
                );
            }
            _ => panic!("adapter runtime result does not match its declared production"),
        }
    }
}
