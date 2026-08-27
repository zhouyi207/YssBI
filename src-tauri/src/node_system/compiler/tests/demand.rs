use super::*;

#[test]
fn demand_specialization_ignores_unbound_inputs_outside_the_retained_closure() {
    let (registry, mut graph) = demand_fixture();
    graph
        .connections
        .remove(&ConnectionId::from_uuid(Uuid::from_u128(11)));
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(
        GraphResourcePath::new("events/main.yssbi-event").unwrap(),
        &graph,
    );

    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();

    assert!(
        matches!(result.outcome, CompilationOutcome::Succeeded),
        "unexpected outcome {:?} with diagnostics {:?}",
        result.outcome,
        result.analysis.diagnostics
    );
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.input.unbound"
            && diagnostic.primary
                == DiagnosticLocation::Port(PortAddress::declared(node_id(4), key("in")))
    }));
    let basis = result
        .execution_basis
        .expect("unbound orphan preserves basis");
    basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/main.yssbi-event", 2, "out")]),
            include_default_results: false,
        })
        .expect("unbound input outside the retained closure must not block execution");
}

#[test]
fn pin_preview_ignores_unbound_inputs_outside_the_retained_closure() {
    let (registry, mut graph) = demand_fixture();
    graph
        .connections
        .remove(&ConnectionId::from_uuid(Uuid::from_u128(11)));
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(
        GraphResourcePath::new("events/main.yssbi-event").unwrap(),
        &graph,
    );
    let basis = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap()
        .execution_basis
        .expect("unbound orphan preserves preview basis");

    basis
        .derive_plan(&ExecutionDemand::PinPreview {
            output: demand_output("events/main.yssbi-event", 2, "out"),
            generation: 7,
        })
        .expect("preview ignores unbound inputs outside its retained closure");
}

#[test]
fn demand_specialization_rejects_a_required_unbound_input_with_its_port() {
    let (registry, mut graph) = demand_fixture();
    graph
        .connections
        .remove(&ConnectionId::from_uuid(Uuid::from_u128(11)));
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(
        GraphResourcePath::new("events/main.yssbi-event").unwrap(),
        &graph,
    );
    let basis = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap()
        .execution_basis
        .expect("unbound input preserves demand-specializable basis");
    let expected = PortAddress::declared(node_id(4), key("in"));

    assert_eq!(
        basis
            .derive_plan(&ExecutionDemand::Outputs {
                outputs: Box::new([demand_output("events/main.yssbi-event", 4, "out")]),
                include_default_results: false,
            })
            .unwrap_err(),
        DemandPlanError::UnboundInput(expected)
    );
}

#[test]
fn demand_specialization_deletes_disconnected_if_control_sources() {
    let mut basis = compiled_demand_basis();
    let destination = declare_control_value(&mut basis);
    let condition = basis.operations[0].outputs[0].value;
    let then_source = basis.operations[0].outputs[0].value;
    let else_source = basis.operations[2].outputs[0].value;
    append_control_region(
        &mut basis,
        StructuredControlRegion::If {
            condition,
            then_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            else_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            results: Box::new([BranchResultBinding {
                destination,
                then_source,
                else_source,
                production: Some(OutputProduction::FullyMaterialized),
            }]),
        },
    );

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/main.yssbi-event", 2, "out")]),
            include_default_results: false,
        })
        .expect("disconnected If declarations are projected out");

    assert!(!contains_region(&plan.root_region, &|region| matches!(
        region,
        StructuredControlRegion::If { .. }
    )));
    assert!(
        !plan
            .value_sources
            .contains(&PlanValueSource::ControlProduced(
                destination,
                OutputProduction::FullyMaterialized,
            ))
    );
    plan.validate().unwrap();
}

#[test]
fn demand_specialization_keeps_only_requested_if_result_declaration() {
    let mut basis = compiled_demand_basis();
    let retained_destination = declare_control_value(&mut basis);
    let deleted_destination = declare_control_value(&mut basis);
    let condition = basis.operations[0].outputs[0].value;
    let then_source = basis.operations[0].outputs[0].value;
    let else_source = basis.operations[2].outputs[0].value;
    append_control_region(
        &mut basis,
        StructuredControlRegion::If {
            condition,
            then_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            else_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            results: Box::new([
                BranchResultBinding {
                    destination: retained_destination,
                    then_source,
                    else_source,
                    production: Some(OutputProduction::FullyMaterialized),
                },
                BranchResultBinding {
                    destination: deleted_destination,
                    then_source,
                    else_source,
                    production: Some(OutputProduction::FullyMaterialized),
                },
            ]),
        },
    );
    let requested = demand_output("events/main.yssbi-event", 99, "out");
    basis.nodes.insert(node_id(99));
    basis.port_facts.insert(
        requested.port.clone(),
        super::super::specialization::DemandPortFact {
            kind: PortKind::Data,
            direction: PortDirection::Output,
        },
    );
    basis.output_results.insert(
        requested.clone(),
        PlanResult {
            name: "requested.branch-result".into(),
            output: requested.clone(),
            value: retained_destination,
        },
    );

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([requested]),
            include_default_results: false,
        })
        .expect("only the requested If result declaration remains");

    let retained_results = match &plan.root_region {
        region
            if contains_region(region, &|candidate| {
                matches!(candidate, StructuredControlRegion::If { .. })
            }) =>
        {
            fn find(region: &StructuredControlRegion) -> Option<&[BranchResultBinding]> {
                match region {
                    StructuredControlRegion::Sequence(steps) => {
                        steps.iter().find_map(|step| match step {
                            ControlStep::Operation(_) => None,
                            ControlStep::Region(region) => find(region),
                        })
                    }
                    StructuredControlRegion::If { results, .. } => Some(results),
                    StructuredControlRegion::Loop { body, .. } => find(body),
                    StructuredControlRegion::Call { .. } => None,
                }
            }
            find(region).unwrap()
        }
        _ => panic!("retained If region"),
    };
    assert_eq!(retained_results.len(), 1);
    assert_eq!(retained_results[0].destination, retained_destination);
    assert!(
        plan.value_sources
            .contains(&PlanValueSource::ControlProduced(
                retained_destination,
                OutputProduction::FullyMaterialized,
            ))
    );
    assert!(
        !plan
            .value_sources
            .contains(&PlanValueSource::ControlProduced(
                deleted_destination,
                OutputProduction::FullyMaterialized,
            ))
    );
    plan.validate().unwrap();
}

#[test]
fn demand_specialization_deletes_disconnected_loop_control_sources() {
    let mut basis = compiled_demand_basis();
    let body_input = declare_control_value(&mut basis);
    let result = declare_control_value(&mut basis);
    let initial_source = basis.operations[0].outputs[0].value;
    let next_source = basis.operations[2].outputs[0].value;
    let continue_condition = basis.operations[0].outputs[0].value;
    append_control_region(
        &mut basis,
        StructuredControlRegion::Loop {
            body: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            carried: Box::new([LoopCarriedBinding {
                body_input,
                initial_source,
                next_source,
                result,
                production: Some(OutputProduction::FullyMaterialized),
            }]),
            continue_condition,
            max_iterations: 3,
        },
    );

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/main.yssbi-event", 2, "out")]),
            include_default_results: false,
        })
        .expect("disconnected Loop declarations are projected out");

    assert!(!contains_region(&plan.root_region, &|region| matches!(
        region,
        StructuredControlRegion::Loop { .. }
    )));
    for value in [body_input, result] {
        assert!(
            !plan
                .value_sources
                .contains(&PlanValueSource::ControlProduced(
                    value,
                    OutputProduction::FullyMaterialized,
                ))
        );
    }
    plan.validate().unwrap();
}

#[test]
fn demand_normalization_is_order_independent_and_default_modes_are_distinct() {
    let (registry, graph) = demand_fixture();
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(
        GraphResourcePath::new("events/main.yssbi-event").unwrap(),
        &graph,
    );
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid graph has lowering basis");
    let a = demand_output("events/main.yssbi-event", 2, "out");
    let b = demand_output("events/main.yssbi-event", 4, "out");

    let first = ExecutionDemand::Outputs {
        outputs: Box::new([a.clone(), b.clone()]),
        include_default_results: false,
    };
    let second = ExecutionDemand::Outputs {
        outputs: Box::new([b.clone(), a.clone(), a.clone()]),
        include_default_results: false,
    };
    let first_key = basis.normalize_demand(&first).unwrap();
    let second_key = basis.normalize_demand(&second).unwrap();
    assert_eq!(first_key, second_key);
    assert_eq!(first_key.digest().unwrap(), second_key.digest().unwrap());
    assert_eq!(
        basis.derive_plan(&first).unwrap(),
        basis.derive_plan(&second).unwrap()
    );
    let same_selection_different_mode = ExecutionDemand::Outputs {
        outputs: Box::new([]),
        include_default_results: true,
    };
    let default_key = basis.normalize_demand(&ExecutionDemand::Default).unwrap();
    let explicit_default_key = basis
        .normalize_demand(&same_selection_different_mode)
        .unwrap();
    assert_ne!(
        default_key, explicit_default_key,
        "normalized keys retain request mode even when selected outputs match"
    );
    assert_ne!(
        default_key.digest().unwrap(),
        explicit_default_key.digest().unwrap()
    );
    let without_defaults = basis
        .normalize_demand(&ExecutionDemand::Outputs {
            outputs: Box::new([a.clone(), b.clone()]),
            include_default_results: false,
        })
        .unwrap();
    let with_defaults = basis
        .normalize_demand(&ExecutionDemand::Outputs {
            outputs: Box::new([a.clone(), b.clone()]),
            include_default_results: true,
        })
        .unwrap();
    assert_ne!(without_defaults, with_defaults);
    assert_ne!(
        without_defaults.digest().unwrap(),
        with_defaults.digest().unwrap()
    );

    let defaults = basis.derive_plan(&ExecutionDemand::Default).unwrap();
    assert_eq!(defaults.results.len(), 2);
    let only_a = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([a.clone()]),
            include_default_results: false,
        })
        .unwrap();
    assert_eq!(only_a.results.len(), 1);
    let a_with_defaults = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([a]),
            include_default_results: true,
        })
        .unwrap();
    assert_eq!(a_with_defaults.results.len(), 2);
}

#[test]
fn demand_driven_publication_preview_has_independent_normalized_identity_and_generation() {
    let (registry, graph) = demand_fixture();
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(
        GraphResourcePath::new("events/main.yssbi-event").unwrap(),
        &graph,
    );
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid graph has lowering basis");
    let output = demand_output("events/main.yssbi-event", 2, "out");
    let ordinary = ExecutionDemand::Outputs {
        outputs: Box::new([output.clone()]),
        include_default_results: false,
    };
    let preview: ExecutionDemand = serde_json::from_value(serde_json::json!({
        "PinPreview": {
            "output": serde_json::to_value(&output).unwrap(),
            "generation": 17
        }
    }))
    .expect("pin preview demand must have a dedicated wire variant");

    let ordinary_plan = basis.derive_plan(&ordinary).unwrap();
    let preview_plan = basis.derive_plan(&preview).unwrap();
    let ordinary = basis.normalize_demand(&ordinary).unwrap();
    let preview = basis.normalize_demand(&preview).unwrap();

    assert!(matches!(
        ordinary_plan.publications.as_ref(),
        [PlannedPublication::GraphResult { output: published, .. }] if published == &output
    ));
    assert!(matches!(
        preview_plan.publications.as_ref(),
        [PlannedPublication::PinPreview {
            output: published,
            generation: 17,
            ..
        }] if published == &output
    ));
    assert_ne!(ordinary, preview);
    assert_ne!(ordinary.digest().unwrap(), preview.digest().unwrap());
    assert_eq!(
        serde_json::to_value(preview).unwrap()["PinPreview"]["generation"],
        17
    );
}

#[test]
fn invalid_requested_outputs_are_rejected_before_plan_construction() {
    let source = test_protocol(
        "demand_validation_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let mut protocol = test_protocol(
        "demand_validation",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port("out", PortDirection::Output, TypeExpr::Unknown, None),
            effect_port("effect", PortDirection::Output),
            control_port("control", PortDirection::Output),
        ],
        vec![],
        vec![],
    );
    protocol.execution.effects = EffectSemantics::Ordered;
    let registry = TestRegistry::new(vec![source, protocol]);
    let mut graph = graph_with_nodes(&[(1, "demand_validation_source"), (2, "demand_validation")]);
    connect(&mut graph, 10, 1, "out", 2, "in");
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(
        GraphResourcePath::new("events/main.yssbi-event").unwrap(),
        &graph,
    );
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid graph has lowering basis");
    let stale_instance = GraphOutputRef {
        graph_path: GraphResourcePath::new("events/main.yssbi-event").unwrap(),
        port: PortAddress::instance(
            node_id(2),
            key("out"),
            PortInstanceId::from_uuid(Uuid::from_u128(99)),
        ),
    };
    let invalid = [
        (
            demand_output("events/other.yssbi-event", 2, "out"),
            "graph_path_mismatch",
        ),
        (
            demand_output("events/main.yssbi-event", 99, "out"),
            "missing_node",
        ),
        (
            demand_output("events/main.yssbi-event", 2, "missing"),
            "missing_port",
        ),
        (
            demand_output("events/main.yssbi-event", 2, "in"),
            "input_port",
        ),
        (
            demand_output("events/main.yssbi-event", 2, "effect"),
            "effect_port",
        ),
        (
            demand_output("events/main.yssbi-event", 2, "control"),
            "control_port",
        ),
        (stale_instance, "stale_instance"),
    ];

    for (output, expected) in invalid {
        let error = basis
            .derive_plan(&ExecutionDemand::Outputs {
                outputs: Box::new([output.clone()]),
                include_default_results: false,
            })
            .unwrap_err();
        let actual = match error {
            DemandPlanError::GraphPathMismatch(_) => "graph_path_mismatch",
            DemandPlanError::MissingNode(_) => "missing_node",
            DemandPlanError::MissingPort(_) => "missing_port",
            DemandPlanError::StalePortInstance(_) => "stale_instance",
            DemandPlanError::InputPort(_) => "input_port",
            DemandPlanError::ControlPort(_) => "control_port",
            DemandPlanError::EffectPort(_) => "effect_port",
            DemandPlanError::UnboundInput(_) => "unbound_input",
            DemandPlanError::InvalidDerivedPlan(_) => "invalid_derived_plan",
            DemandPlanError::CanonicalEncoding(_) => "canonical_encoding",
        };
        assert_eq!(actual, expected, "wrong error for {output:?}");
    }
}

#[test]
fn retained_operation_keeps_external_value_dependency_and_source() {
    let mut entry = test_protocol(
        "demand_external_entry",
        vec![
            control_port("then", PortDirection::Output),
            data_port("payload", PortDirection::Output, TypeExpr::Unknown, None),
        ],
        vec![],
        vec![],
    );
    entry.managed_role = Some(ManagedNodeRole::EventBegin);
    let entry_type = entry.type_id.clone();
    let sink = chain_protocol("demand_external_sink");
    let sink_type = sink.type_id.clone();
    let registry = TestRegistry::new(vec![entry, sink])
        .structural(&entry_type, StructuralNodeRole::EventBegin)
        .with_lowerer(
            &sink_type,
            FragmentLowerer {
                fragment: kernel_fragment(
                    EffectSemantics::None,
                    FragmentMetadata {
                        results: Box::new([FragmentResult {
                            name: "external-result".into(),
                            output: PortAddress::declared(node_id(2), key("out")),
                        }]),
                        ..FragmentMetadata::default()
                    },
                ),
            },
        );
    let mut graph = graph_with_nodes(&[(1, "demand_external_entry"), (2, "demand_external_sink")]);
    connect(&mut graph, 10, 1, "payload", 2, "in");
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(
        GraphResourcePath::new("events/external.yssbi-event").unwrap(),
        &graph,
    );
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid graph has lowering basis");

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/external.yssbi-event", 2, "out")]),
            include_default_results: false,
        })
        .expect("external source dependency remains valid");

    assert_eq!(plan.operations.len(), 1);
    assert_eq!(plan.value_dependencies.len(), 1);
    assert!(
        plan.operations
            .iter()
            .all(|operation| !matches!(operation.kernel, PlannedKernel::Adapter(_)))
    );
    assert!(plan.value_sources.iter().any(|source| {
        matches!(source, PlanValueSource::ExternalInput(value, _) if plan.value_dependencies.iter().any(|dependency| dependency.source == *value))
    }));
    assert!(!matches!(
        plan.validate(),
        Err(ref errors) if errors.0.iter().any(|error| matches!(error, crate::node_system::plan::PlanValidationError::MissingInputSource { .. }))
    ));
}

#[test]
fn evaluation_policy_and_effect_predecessors_are_authoritative_roots() {
    let mut predecessor = test_protocol(
        "demand_effect_predecessor",
        vec![effect_port("effect", PortDirection::Output)],
        vec![],
        vec![],
    );
    predecessor.execution.purity = Purity::Effectful;
    predecessor.execution.effects = EffectSemantics::Ordered;
    let mut middle = test_protocol(
        "demand_effect_middle",
        vec![
            effect_port("before", PortDirection::Input),
            effect_port("after", PortDirection::Output),
        ],
        vec![],
        vec![],
    );
    middle.execution.purity = Purity::Effectful;
    middle.execution.effects = EffectSemantics::Ordered;
    let mut eager = test_protocol(
        "demand_eager_pure",
        vec![effect_port("effect", PortDirection::Input)],
        vec![],
        vec![],
    );
    eager.execution.purity = Purity::Pure;
    eager.execution.evaluation = EvaluationPolicy::EagerWhenRegionEntered;
    eager.execution.effects = EffectSemantics::Ordered;
    let mut demand_driven_effectful =
        test_protocol("demand_disconnected_effectful", vec![], vec![], vec![]);
    demand_driven_effectful.execution.purity = Purity::Effectful;
    demand_driven_effectful.execution.effects = EffectSemantics::Ordered;
    let types = [
        predecessor.type_id.clone(),
        middle.type_id.clone(),
        eager.type_id.clone(),
        demand_driven_effectful.type_id.clone(),
    ];
    let mut registry = TestRegistry::new(vec![predecessor, middle, eager, demand_driven_effectful]);
    for node_type in &types {
        registry = registry.with_lowerer(
            node_type,
            FragmentLowerer {
                fragment: kernel_fragment(EffectSemantics::Ordered, FragmentMetadata::default()),
            },
        );
    }
    let mut graph = graph_with_nodes(&[
        (1, "demand_effect_predecessor"),
        (2, "demand_effect_middle"),
        (3, "demand_eager_pure"),
        (4, "demand_disconnected_effectful"),
    ]);
    connect(&mut graph, 10, 1, "effect", 2, "before");
    connect(&mut graph, 11, 2, "after", 3, "effect");
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(
        GraphResourcePath::new("events/main.yssbi-event").unwrap(),
        &graph,
    );
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid graph has lowering basis");

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([]),
            include_default_results: false,
        })
        .unwrap();

    assert_eq!(
        plan.operations
            .iter()
            .map(|operation| operation.source_node_id)
            .collect::<Vec<_>>(),
        vec![node_id(1), node_id(2), node_id(3)]
    );
    assert_eq!(plan.effect_dependencies.len(), 2);
}
