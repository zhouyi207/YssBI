use super::*;

#[test]
fn sequence_builds_an_ordered_control_region() {
    let sequence = structural_protocol(
        "sequence",
        vec![
            control_port("enter", PortDirection::Input),
            control_port("then", PortDirection::Output),
        ],
        vec![],
    );
    let leaf = test_protocol(
        "controlled_leaf",
        vec![control_port("enter", PortDirection::Input)],
        vec![],
        vec![],
    );
    let sequence_type = sequence.type_id.clone();
    let registry = TestRegistry::new(vec![sequence, leaf])
        .structural(&sequence_type, StructuralNodeRole::Sequence);
    let mut graph = graph_with_nodes(&[(1, "sequence"), (2, "controlled_leaf")]);
    connect(&mut graph, 10, 1, "then", 2, "enter");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    let plan = result.plan.expect("sequence should produce a plan");
    assert_eq!(plan.operations.len(), 1);
    assert!(contains_region(&plan.root_region, &|region| matches!(
        region,
        crate::node_system::plan::StructuredControlRegion::Sequence(steps)
            if steps.iter().any(|step| matches!(step, crate::node_system::plan::ControlStep::Operation(_)))
    )));
}

#[test]
fn builtin_multi_output_sequence_outside_branch_keeps_walker_order() {
    use crate::node_system::catalog::build_builtin_node_system;

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.control.sequence"),
        (2, "yssbi.control.do"),
        (3, "yssbi.control.do"),
    ]);
    let first = bind_member_port(&mut graph, 1, "then", 10, "a");
    let second = bind_member_port(&mut graph, 1, "then", 11, "z");
    connect_addresses(
        &mut graph,
        100,
        first,
        PortAddress::declared(node_id(2), key("enter")),
    );
    connect_addresses(
        &mut graph,
        101,
        second,
        PortAddress::declared(node_id(3), key("enter")),
    );

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result
        .plan
        .unwrap_or_else(|| panic!("sequence diagnostics: {:?}", result.analysis.diagnostics));

    assert_eq!(plan.operations.len(), 2);
    let first = operation_index_for_node(&plan, 2);
    let second = operation_index_for_node(&plan, 3);
    let StructuredControlRegion::Sequence(steps) = &plan.root_region else {
        panic!("expected root sequence, got {:?}", plan.root_region);
    };
    let first_position = steps
        .iter()
        .position(|step| matches!(step, ControlStep::Operation(operation) if *operation == first))
        .expect("first Sequence output operation");
    let second_position = steps
        .iter()
        .position(|step| matches!(step, ControlStep::Operation(operation) if *operation == second))
        .expect("second Sequence output operation");
    assert!(first_position < second_position);
}

#[test]
fn print_protocol_default_lowers_to_effective_runtime_binding() {
    use crate::node_system::catalog::build_builtin_node_system;

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let graph = builtin_graph_with_nodes(&[(1, "yssbi.debug.print")]);

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result
        .plan
        .unwrap_or_else(|| panic!("print diagnostics: {:?}", result.analysis.diagnostics));
    let print = &plan.operations[operation_index_for_node(&plan, 1).index()];

    assert_eq!(print.inputs.len(), 1);
    assert_eq!(
        print.inputs[0].bound_value,
        Some(Value::String("Hello, World!".into()))
    );
    plan.validate().unwrap();
}

#[test]
fn unconnected_branch_condition_uses_the_protocol_default() {
    use crate::node_system::catalog::build_builtin_node_system;

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.project.event.begin"),
        (2, "yssbi.control.branch"),
        (3, "yssbi.constant.string"),
        (4, "yssbi.debug.print"),
    ]);
    connect(&mut graph, 100, 1, "then", 2, "enter");
    connect(&mut graph, 101, 2, "true", 4, "enter");
    connect(&mut graph, 102, 3, "value", 4, "message");
    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(
        matches!(result.outcome, CompilationOutcome::Succeeded),
        "unexpected outcome {:?} with diagnostics {:?}",
        result.outcome,
        result.analysis.diagnostics
    );
    let plan = result
        .plan
        .expect("Branch protocol default makes a full plan");
    let StructuredControlRegion::Sequence(steps) = &plan.root_region else {
        panic!("event root is a sequence")
    };
    let condition = steps
        .iter()
        .find_map(|step| match step {
            ControlStep::Region(region) => match region.as_ref() {
                StructuredControlRegion::If { condition, .. } => Some(*condition),
                _ => None,
            },
            ControlStep::Operation(_) => None,
        })
        .expect("event plan contains Branch");
    assert_eq!(plan.bound_values.get(&condition), Some(&Value::Bool(true)));
}

#[test]
fn branch_builds_exclusive_true_and_false_regions() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.int64"),
        (3, "yssbi.constant.int64"),
        (4, "yssbi.control.branch"),
        (5, "yssbi.control.do"),
        (6, "yssbi.control.do"),
        (7, "yssbi.control.merge"),
        (8, "yssbi.debug.view"),
        (9, "yssbi.constant.int64"),
    ]);
    let then_source = bind_member_port(&mut graph, 4, "then_source", 40, "z");
    let else_source = bind_member_port(&mut graph, 4, "else_source", 40, "a");
    let result_port = bind_member_port(&mut graph, 4, "result", 40, "m");
    let demanded_result = result_port.clone();
    let merge_true = bind_member_port(&mut graph, 7, "enter", 70, "z");
    let merge_false = bind_member_port(&mut graph, 7, "enter", 71, "a");

    connect(&mut graph, 100, 1, "value", 4, "condition");
    connect_addresses(
        &mut graph,
        101,
        PortAddress::declared(node_id(2), key("value")),
        then_source,
    );
    connect_addresses(
        &mut graph,
        102,
        PortAddress::declared(node_id(3), key("value")),
        else_source,
    );
    connect_addresses(
        &mut graph,
        103,
        result_port,
        PortAddress::declared(node_id(8), key("data")),
    );
    connect(&mut graph, 104, 4, "true", 5, "enter");
    connect_addresses(
        &mut graph,
        105,
        PortAddress::declared(node_id(5), key("then")),
        merge_true,
    );
    connect(&mut graph, 106, 4, "false", 6, "enter");
    connect_addresses(
        &mut graph,
        107,
        PortAddress::declared(node_id(6), key("then")),
        merge_false,
    );
    connect(&mut graph, 108, 7, "then", 8, "enter");

    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(
        GraphResourcePath::new("events/branch-demand.yssbi-event").unwrap(),
        &graph,
    );
    let mut result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result.execution_basis.as_mut().unwrap_or_else(|| {
        panic!(
            "branch diagnostics: {:?}; outcome: {:?}",
            result.analysis.diagnostics, result.outcome
        )
    });
    let branch_local_pure = basis
        .operations
        .iter_mut()
        .find(|operation| operation.source_node_id == node_id(5))
        .expect("true arm operation is present in the basis");
    branch_local_pure.evaluation = EvaluationPolicy::DemandDriven;
    branch_local_pure.purity = Purity::Pure;
    branch_local_pure.effects = EffectSemantics::None;

    fn branch_results_mut(
        region: &mut StructuredControlRegion,
    ) -> Option<&mut Box<[crate::node_system::plan::BranchResultBinding]>> {
        match region {
            StructuredControlRegion::Sequence(steps) => {
                steps.iter_mut().find_map(|step| match step {
                    ControlStep::Operation(_) => None,
                    ControlStep::Region(region) => branch_results_mut(region),
                })
            }
            StructuredControlRegion::If { results, .. } => Some(results),
            StructuredControlRegion::Loop { body, .. } => branch_results_mut(body),
            StructuredControlRegion::Call { .. } => None,
        }
    }
    let deleted_result_value = ValueRef::new(basis.value_count);
    basis.value_count += 1;
    let results = branch_results_mut(&mut basis.root_region).expect("Branch result bindings");
    let mut bindings = results.to_vec();
    let mut deleted_binding = bindings[0];
    deleted_binding.destination = deleted_result_value;
    bindings.push(deleted_binding);
    *results = bindings.into_boxed_slice();
    let mut value_sources = basis.value_sources.to_vec();
    value_sources.push(PlanValueSource::ControlProduced(
        deleted_result_value,
        OutputProduction::FullyMaterialized,
    ));
    basis.value_sources = value_sources.into_boxed_slice();

    let mut disconnected_basis = basis.clone();
    for operation in &mut disconnected_basis.operations {
        if [node_id(6), node_id(8)].contains(&operation.source_node_id) {
            operation.evaluation = EvaluationPolicy::DemandDriven;
            operation.purity = Purity::Pure;
            operation.effects = EffectSemantics::None;
        }
    }
    let disconnected = disconnected_basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output(
                "events/branch-demand.yssbi-event",
                9,
                "value",
            )]),
            include_default_results: false,
        })
        .expect("a disconnected If with results is deleted without orphan declarations");
    assert!(!contains_region(
        &disconnected.root_region,
        &|region| matches!(region, StructuredControlRegion::If { .. })
    ));
    assert!(
        disconnected
            .value_sources
            .iter()
            .all(|source| !matches!(source, PlanValueSource::ControlProduced(..)))
    );
    disconnected.validate().unwrap();

    let specialized = result
        .execution_basis
        .as_ref()
        .unwrap()
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([GraphOutputRef {
                graph_path: snapshot.provenance.graph_path.clone(),
                port: demanded_result.clone(),
            }]),
            include_default_results: false,
        })
        .expect("demanded Branch result safely specializes");
    let specialized_nodes = specialized
        .operations
        .iter()
        .map(|operation| operation.source_node_id)
        .collect::<BTreeSet<_>>();
    for pruned in [9] {
        assert!(!specialized_nodes.contains(&node_id(pruned)));
    }
    assert!(
        !specialized_nodes.contains(&node_id(5)),
        "unrequested arm-local pure work is pruned"
    );
    for retained in [1, 2, 3, 6] {
        assert!(
            specialized_nodes.contains(&node_id(retained)),
            "Branch condition, both result sources, and arm-local eager work stay retained"
        );
    }
    let retained_else_eager = operation_index_for_node(&specialized, 6);
    assert!(contains_region(
        &specialized.root_region,
        &|region| matches!(
            region,
            StructuredControlRegion::If {
                then_region,
                else_region,
                results,
                ..
            } if results.len() == 1
                && matches!(then_region.as_ref(), StructuredControlRegion::Sequence(steps) if steps.is_empty())
                && region_contains_operation(else_region, retained_else_eager)
        )
    ));
    specialized.validate().unwrap();

    let basis = result.execution_basis.as_ref().unwrap();
    let retained_result_value = basis.output_results[&GraphOutputRef {
        graph_path: snapshot.provenance.graph_path.clone(),
        port: demanded_result,
    }]
        .value;

    assert!(
        specialized
            .value_sources
            .contains(&PlanValueSource::ControlProduced(
                retained_result_value,
                OutputProduction::FullyMaterialized,
            ))
    );
    assert!(
        !specialized
            .value_sources
            .contains(&PlanValueSource::ControlProduced(
                deleted_result_value,
                OutputProduction::FullyMaterialized,
            ))
    );

    let plan = result
        .plan
        .unwrap_or_else(|| panic!("branch diagnostics: {:?}", result.analysis.diagnostics));
    let then_operation = operation_index_for_node(&plan, 5);
    let else_operation = operation_index_for_node(&plan, 6);
    let continuation = operation_index_for_node(&plan, 8);
    let then_output = plan.operations[operation_index_for_node(&plan, 2).index()].outputs[0].value;
    let else_output = plan.operations[operation_index_for_node(&plan, 3).index()].outputs[0].value;
    let continuation_input = plan.operations[continuation.index()].inputs[0].value;
    let then_binding_source = plan
        .value_dependencies
        .iter()
        .find(|dependency| dependency.source == then_output)
        .expect("then constant must feed its exact member")
        .destination;
    let else_binding_source = plan
        .value_dependencies
        .iter()
        .find(|dependency| dependency.source == else_output)
        .expect("else constant must feed its exact member")
        .destination;
    let branch_destination = plan
        .value_dependencies
        .iter()
        .find(|dependency| dependency.destination == continuation_input)
        .expect("branch result must feed the continuation directly")
        .source;
    assert!(
        plan.operations
            .iter()
            .all(|operation| !matches!(operation.kernel, PlannedKernel::Adapter(_)))
    );

    let root_steps = match &plan.root_region {
        StructuredControlRegion::Sequence(steps) => steps,
        other => panic!("expected root sequence, got {other:?}"),
    };
    let branch_region = root_steps
        .iter()
        .find_map(|step| match step {
            ControlStep::Region(region)
                if matches!(region.as_ref(), StructuredControlRegion::If { .. }) =>
            {
                Some(region.as_ref())
            }
            _ => None,
        })
        .expect("root sequence must contain Branch");
    let StructuredControlRegion::If {
        then_region,
        else_region,
        results,
        ..
    } = branch_region
    else {
        unreachable!()
    };
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].destination, branch_destination);
    assert_eq!(results[0].then_source, then_binding_source);
    assert_eq!(results[0].else_source, else_binding_source);
    assert_eq!(
        results[0].production,
        Some(OutputProduction::FullyMaterialized)
    );
    assert!(region_contains_operation(then_region, then_operation));
    assert!(!region_contains_operation(then_region, else_operation));
    assert!(region_contains_operation(else_region, else_operation));
    assert!(!region_contains_operation(else_region, then_operation));
    assert!(!region_contains_operation(then_region, continuation));
    assert!(!region_contains_operation(else_region, continuation));
    assert!(root_steps.iter().any(
        |step| matches!(step, ControlStep::Operation(operation) if *operation == continuation)
    ));
}

#[test]
fn nested_branch_with_one_terminating_arm_blocks_unstructured_continuation() {
    use crate::node_system::catalog::build_builtin_node_system;

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.bool"),
        (3, "yssbi.control.branch"),
        (4, "yssbi.control.branch"),
        (5, "yssbi.control.merge"),
        (6, "yssbi.control.do"),
    ]);
    let merge_inner_true = bind_member_port(&mut graph, 5, "enter", 50, "z");
    let merge_outer_false = bind_member_port(&mut graph, 5, "enter", 51, "a");
    connect(&mut graph, 100, 1, "value", 3, "condition");
    connect(&mut graph, 101, 2, "value", 4, "condition");
    connect(&mut graph, 102, 3, "true", 4, "enter");
    connect_addresses(
        &mut graph,
        103,
        PortAddress::declared(node_id(4), key("true")),
        merge_inner_true,
    );
    connect_addresses(
        &mut graph,
        104,
        PortAddress::declared(node_id(3), key("false")),
        merge_outer_false,
    );
    connect(&mut graph, 105, 5, "then", 6, "enter");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(
        result.plan.is_none(),
        "must not unconditionally execute node A"
    );
    assert!(result.semantic.is_some());
    assert!(!result.analysis.has_blocking_errors());
    assert!(matches!(
        result.outcome,
        CompilationOutcome::InternalFailure(ref failure)
            if failure.stage == CompilationStage::Lowering
                && failure.code.as_ref() == "compiler.control.unstructured_continuation"
                && failure.node_id == Some(node_id(3))
    ));
}

#[test]
fn branch_postdom_uses_last_walker_successor_for_multi_output_sequence() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.control.branch"),
        (3, "yssbi.control.sequence"),
        (4, "yssbi.control.do"),
        (5, "yssbi.control.merge"),
        (6, "yssbi.control.do"),
    ]);
    let sequence_first = bind_member_port(&mut graph, 3, "then", 30, "a");
    let sequence_second = bind_member_port(&mut graph, 3, "then", 31, "z");
    let merge_true = bind_member_port(&mut graph, 5, "enter", 50, "a");
    let merge_false = bind_member_port(&mut graph, 5, "enter", 51, "z");
    connect(&mut graph, 100, 1, "value", 2, "condition");
    connect(&mut graph, 101, 2, "true", 3, "enter");
    connect_addresses(
        &mut graph,
        102,
        sequence_first,
        PortAddress::declared(node_id(4), key("enter")),
    );
    connect_addresses(&mut graph, 103, sequence_second, merge_true);
    connect_addresses(
        &mut graph,
        104,
        PortAddress::declared(node_id(2), key("false")),
        merge_false,
    );
    connect(&mut graph, 105, 5, "then", 6, "enter");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result
        .plan
        .unwrap_or_else(|| panic!("sequence diagnostics: {:?}", result.analysis.diagnostics));
    let first = operation_index_for_node(&plan, 4);
    let continuation = operation_index_for_node(&plan, 6);
    let root_steps = match &plan.root_region {
        StructuredControlRegion::Sequence(steps) => steps,
        other => panic!("expected root sequence, got {other:?}"),
    };
    let (branch_index, branch) = root_steps
        .iter()
        .enumerate()
        .find_map(|(index, step)| match step {
            ControlStep::Region(region)
                if matches!(region.as_ref(), StructuredControlRegion::If { .. }) =>
            {
                Some((index, region.as_ref()))
            }
            _ => None,
        })
        .expect("Branch region");
    let StructuredControlRegion::If {
        then_region,
        else_region,
        ..
    } = branch
    else {
        unreachable!()
    };
    assert!(region_contains_operation(then_region, first));
    assert!(!region_contains_operation(else_region, first));
    assert!(!region_contains_operation(then_region, continuation));
    assert!(!region_contains_operation(else_region, continuation));
    assert!(root_steps[branch_index + 1..].iter().any(
        |step| matches!(step, ControlStep::Operation(operation) if *operation == continuation)
    ));
}

#[test]
fn branch_continuation_allows_multi_output_sequence_suffix_after_merge() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.control.branch"),
        (3, "yssbi.control.do"),
        (4, "yssbi.control.do"),
        (5, "yssbi.control.merge"),
        (6, "yssbi.control.sequence"),
        (7, "yssbi.control.do"),
        (8, "yssbi.control.do"),
    ]);
    let merge_true = bind_member_port(&mut graph, 5, "enter", 50, "a");
    let merge_false = bind_member_port(&mut graph, 5, "enter", 51, "z");
    let suffix_first = bind_member_port(&mut graph, 6, "then", 60, "a");
    let suffix_second = bind_member_port(&mut graph, 6, "then", 61, "z");
    connect(&mut graph, 100, 1, "value", 2, "condition");
    connect(&mut graph, 101, 2, "true", 3, "enter");
    connect(&mut graph, 102, 2, "false", 4, "enter");
    connect_addresses(
        &mut graph,
        103,
        PortAddress::declared(node_id(3), key("then")),
        merge_true,
    );
    connect_addresses(
        &mut graph,
        104,
        PortAddress::declared(node_id(4), key("then")),
        merge_false,
    );
    connect(&mut graph, 105, 5, "then", 6, "enter");
    connect_addresses(
        &mut graph,
        106,
        suffix_first,
        PortAddress::declared(node_id(7), key("enter")),
    );
    connect_addresses(
        &mut graph,
        107,
        suffix_second,
        PortAddress::declared(node_id(8), key("enter")),
    );

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result
        .plan
        .unwrap_or_else(|| panic!("suffix diagnostics: {:?}", result.analysis.diagnostics));
    let first = operation_index_for_node(&plan, 7);
    let second = operation_index_for_node(&plan, 8);
    let root_steps = match &plan.root_region {
        StructuredControlRegion::Sequence(steps) => steps,
        other => panic!("expected root sequence, got {other:?}"),
    };
    let branch_index = root_steps
        .iter()
        .position(|step| matches!(step, ControlStep::Region(region) if matches!(region.as_ref(), StructuredControlRegion::If { .. })))
        .expect("Branch region");
    let suffix = &root_steps[branch_index + 1..];
    let first_index = suffix
        .iter()
        .position(|step| matches!(step, ControlStep::Operation(operation) if *operation == first))
        .expect("first suffix operation");
    let second_index = suffix
        .iter()
        .position(|step| matches!(step, ControlStep::Operation(operation) if *operation == second))
        .expect("second suffix operation");
    assert!(first_index < second_index);
}

#[test]
fn nested_branch_postdom_stops_before_outer_multi_output_sequence_suffix() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.bool"),
        (3, "yssbi.control.branch"),
        (4, "yssbi.control.branch"),
        (5, "yssbi.control.merge"),
        (6, "yssbi.control.merge"),
        (7, "yssbi.control.sequence"),
        (8, "yssbi.control.do"),
        (9, "yssbi.control.do"),
    ]);
    let inner_true = bind_member_port(&mut graph, 5, "enter", 50, "a");
    let inner_false = bind_member_port(&mut graph, 5, "enter", 51, "z");
    let outer_true = bind_member_port(&mut graph, 6, "enter", 60, "a");
    let outer_false = bind_member_port(&mut graph, 6, "enter", 61, "z");
    let suffix_first = bind_member_port(&mut graph, 7, "then", 70, "a");
    let suffix_second = bind_member_port(&mut graph, 7, "then", 71, "z");
    connect(&mut graph, 100, 1, "value", 3, "condition");
    connect(&mut graph, 101, 2, "value", 4, "condition");
    connect(&mut graph, 102, 3, "true", 4, "enter");
    connect_addresses(
        &mut graph,
        103,
        PortAddress::declared(node_id(4), key("true")),
        inner_true,
    );
    connect_addresses(
        &mut graph,
        104,
        PortAddress::declared(node_id(4), key("false")),
        inner_false,
    );
    connect_addresses(
        &mut graph,
        105,
        PortAddress::declared(node_id(5), key("then")),
        outer_true,
    );
    connect_addresses(
        &mut graph,
        106,
        PortAddress::declared(node_id(3), key("false")),
        outer_false,
    );
    connect(&mut graph, 107, 6, "then", 7, "enter");
    connect_addresses(
        &mut graph,
        108,
        suffix_first,
        PortAddress::declared(node_id(8), key("enter")),
    );
    connect_addresses(
        &mut graph,
        109,
        suffix_second,
        PortAddress::declared(node_id(9), key("enter")),
    );

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result.plan.unwrap_or_else(|| {
        panic!(
            "nested suffix diagnostics: {:?}",
            result.analysis.diagnostics
        )
    });
    let first = operation_index_for_node(&plan, 8);
    let second = operation_index_for_node(&plan, 9);
    let root_steps = match &plan.root_region {
        StructuredControlRegion::Sequence(steps) => steps,
        other => panic!("expected root sequence, got {other:?}"),
    };
    let branch_index = root_steps
        .iter()
        .position(|step| matches!(step, ControlStep::Region(region) if matches!(region.as_ref(), StructuredControlRegion::If { .. })))
        .expect("outer Branch region");
    assert!(
        root_steps[branch_index + 1..]
            .iter()
            .any(|step| matches!(step, ControlStep::Operation(operation) if *operation == first))
    );
    assert!(
        root_steps[branch_index + 1..]
            .iter()
            .any(|step| matches!(step, ControlStep::Operation(operation) if *operation == second))
    );
}

#[test]
fn nested_complete_diamond_uses_the_true_common_continuation() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.bool"),
        (3, "yssbi.control.branch"),
        (4, "yssbi.control.branch"),
        (5, "yssbi.control.merge"),
        (6, "yssbi.control.merge"),
        (7, "yssbi.control.do"),
    ]);
    let inner_true = bind_member_port(&mut graph, 5, "enter", 50, "z");
    let inner_false = bind_member_port(&mut graph, 5, "enter", 51, "a");
    let outer_true = bind_member_port(&mut graph, 6, "enter", 60, "z");
    let outer_false = bind_member_port(&mut graph, 6, "enter", 61, "a");
    connect(&mut graph, 100, 1, "value", 3, "condition");
    connect(&mut graph, 101, 2, "value", 4, "condition");
    connect(&mut graph, 102, 3, "true", 4, "enter");
    connect_addresses(
        &mut graph,
        103,
        PortAddress::declared(node_id(4), key("true")),
        inner_true,
    );
    connect_addresses(
        &mut graph,
        104,
        PortAddress::declared(node_id(4), key("false")),
        inner_false,
    );
    connect_addresses(
        &mut graph,
        105,
        PortAddress::declared(node_id(5), key("then")),
        outer_true,
    );
    connect_addresses(
        &mut graph,
        106,
        PortAddress::declared(node_id(3), key("false")),
        outer_false,
    );
    connect(&mut graph, 107, 6, "then", 7, "enter");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result.plan.unwrap_or_else(|| {
        panic!(
            "nested diamond diagnostics: {:?}",
            result.analysis.diagnostics
        )
    });
    let continuation = operation_index_for_node(&plan, 7);
    let root_steps = match &plan.root_region {
        StructuredControlRegion::Sequence(steps) => steps,
        other => panic!("expected root sequence, got {other:?}"),
    };
    let (outer_index, outer) = root_steps
        .iter()
        .enumerate()
        .find_map(|(index, step)| match step {
            ControlStep::Region(region)
                if matches!(region.as_ref(), StructuredControlRegion::If { .. }) =>
            {
                Some((index, region.as_ref()))
            }
            _ => None,
        })
        .expect("outer Branch must be top-level");
    let StructuredControlRegion::If {
        then_region,
        else_region,
        ..
    } = outer
    else {
        unreachable!()
    };
    assert!(contains_region(then_region, &|region| matches!(
        region,
        StructuredControlRegion::If { .. }
    )));
    assert!(!region_contains_operation(then_region, continuation));
    assert!(!region_contains_operation(else_region, continuation));
    assert!(root_steps[outer_index + 1..].iter().any(
        |step| matches!(step, ControlStep::Operation(operation) if *operation == continuation)
    ));
}

#[test]
fn malformed_builtin_control_members_emit_blocking_structured_diagnostics() {
    use crate::node_system::catalog::build_builtin_node_system;

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut branch = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.int64"),
        (3, "yssbi.constant.int64"),
        (4, "yssbi.control.branch"),
    ]);
    let then_source = bind_member_port(&mut branch, 4, "then_source", 40, "z");
    let else_source = bind_member_port(&mut branch, 4, "else_source", 41, "a");
    bind_member_port(&mut branch, 4, "result", 41, "m");
    connect(&mut branch, 100, 1, "value", 4, "condition");
    connect_addresses(
        &mut branch,
        101,
        PortAddress::declared(node_id(2), key("value")),
        then_source,
    );
    connect_addresses(
        &mut branch,
        102,
        PortAddress::declared(node_id(3), key("value")),
        else_source,
    );

    let branch_result = GraphCompiler::new(&registry, &Resources).compile(&branch);
    assert!(branch_result.plan.is_none());
    assert!(branch_result.semantic.is_some());
    assert!(!branch_result.analysis.has_blocking_errors());
    assert!(matches!(
        branch_result.outcome,
        CompilationOutcome::InternalFailure(ref failure)
            if failure.stage == CompilationStage::Lowering
                && failure.code.as_ref() == "compiler.control.member_group_identity_ambiguous"
                && failure.node_id == Some(node_id(4))
    ));

    let mut incomplete_branch = branch.clone();
    incomplete_branch.port_bindings.retain(|address, _| {
        matches!(
            &address.port,
            crate::graph_document::PortRef::Instance { template, .. }
                if template.as_str() == "then_source"
        )
    });
    incomplete_branch
        .connections
        .remove(&ConnectionId::from_uuid(Uuid::from_u128(102)));
    let incomplete_result = GraphCompiler::new(&registry, &Resources).compile(&incomplete_branch);
    assert!(incomplete_result.plan.is_none());
    assert!(incomplete_result.semantic.is_some());
    assert!(!incomplete_result.analysis.has_blocking_errors());
    assert!(matches!(
        incomplete_result.outcome,
        CompilationOutcome::InternalFailure(ref failure)
            if failure.stage == CompilationStage::Lowering
                && failure.code.as_ref() == "compiler.control.member_group_incomplete"
                && failure.node_id == Some(node_id(4))
    ));

    let mut loop_graph =
        builtin_graph_with_nodes(&[(5, "yssbi.constant.bool"), (6, "yssbi.control.loop")]);
    set_parameters(
        &mut loop_graph,
        6,
        &[("max_iterations", serde_json::json!(3))],
    );
    connect(&mut loop_graph, 103, 5, "value", 6, "condition");

    let loop_result = GraphCompiler::new(&registry, &Resources).compile(&loop_graph);
    assert!(loop_result.plan.is_none());
    assert!(loop_result.semantic.is_some());
    assert!(!loop_result.analysis.has_blocking_errors());
    assert!(matches!(
        loop_result.outcome,
        CompilationOutcome::InternalFailure(ref failure)
            if failure.stage == CompilationStage::Lowering
                && failure.code.as_ref() == "compiler.control.member_group_count_invalid"
                && failure.node_id == Some(node_id(6))
    ));
}

#[test]
fn loop_uses_explicit_condition_limit_and_carried_bindings() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::StructuredControlRegion;

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.int64"),
        (3, "yssbi.constant.int64"),
        (4, "yssbi.constant.int64"),
        (5, "yssbi.constant.int64"),
        (6, "yssbi.control.loop"),
        (7, "yssbi.control.do"),
        (8, "yssbi.control.do"),
        (9, "yssbi.constant.int64"),
        (10, "yssbi.control.do"),
        (11, "yssbi.control.sequence"),
    ]);
    set_parameters(&mut graph, 6, &[("max_iterations", serde_json::json!(7))]);
    let mut members = BTreeMap::new();
    for (instance, order) in [(61, "z"), (60, "a")] {
        let initial = bind_member_port(&mut graph, 6, "initial_source", instance, order);
        let body_input = bind_member_port(&mut graph, 6, "body_input", instance, order);
        let next = bind_member_port(&mut graph, 6, "next_source", instance, order);
        let result = bind_member_port(&mut graph, 6, "result", instance, order);
        members.insert(instance, (initial, body_input, next, result));
    }
    connect(&mut graph, 100, 1, "value", 6, "condition");
    for (connection, source, instance) in [(101, 2, 60), (102, 3, 61)] {
        connect_addresses(
            &mut graph,
            connection,
            PortAddress::declared(node_id(source), key("value")),
            members[&instance].0.clone(),
        );
    }
    for (connection, source, instance) in [(103, 4, 60), (104, 5, 61)] {
        connect_addresses(
            &mut graph,
            connection,
            PortAddress::declared(node_id(source), key("value")),
            members[&instance].2.clone(),
        );
    }
    let body_pure = bind_member_port(&mut graph, 11, "then", 110, "a");
    let body_eager = bind_member_port(&mut graph, 11, "then", 111, "z");
    connect(&mut graph, 105, 6, "body", 11, "enter");
    connect(&mut graph, 106, 6, "then", 8, "enter");
    connect_addresses(
        &mut graph,
        107,
        body_pure,
        PortAddress::declared(node_id(7), key("enter")),
    );
    connect_addresses(
        &mut graph,
        108,
        body_eager,
        PortAddress::declared(node_id(10), key("enter")),
    );

    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(
        GraphResourcePath::new("events/loop-demand.yssbi-event").unwrap(),
        &graph,
    );
    let mut result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let body_local_pure = result
        .execution_basis
        .as_mut()
        .expect("loop graph has a specialization basis")
        .operations
        .iter_mut()
        .find(|operation| operation.source_node_id == node_id(7))
        .expect("first body operation is present in the basis");
    body_local_pure.evaluation = EvaluationPolicy::DemandDriven;
    body_local_pure.purity = Purity::Pure;
    body_local_pure.effects = EffectSemantics::None;
    let specialized = result
        .execution_basis
        .as_ref()
        .unwrap()
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([GraphOutputRef {
                graph_path: snapshot.provenance.graph_path.clone(),
                port: members[&60].3.clone(),
            }]),
            include_default_results: false,
        })
        .expect("demanded Loop result safely specializes");
    let specialized_nodes = specialized
        .operations
        .iter()
        .map(|operation| operation.source_node_id)
        .collect::<BTreeSet<_>>();
    assert!(!specialized_nodes.contains(&node_id(9)));
    assert!(
        !specialized_nodes.contains(&node_id(7)),
        "unrequested body-local pure work is pruned"
    );
    for retained in [1, 2, 3, 4, 5, 10] {
        assert!(
            specialized_nodes.contains(&node_id(retained)),
            "Loop condition, carried bindings, and body eager work stay retained"
        );
    }
    let retained_body_eager = operation_index_for_node(&specialized, 10);
    assert!(contains_region(
        &specialized.root_region,
        &|region| matches!(
            region,
            StructuredControlRegion::Loop {
                body,
                carried,
                max_iterations: 7,
                ..
            } if carried.len() == 2 && region_contains_operation(body, retained_body_eager)
        )
    ));
    specialized.validate().unwrap();

    let mut disconnected_basis = result.execution_basis.as_ref().unwrap().clone();
    let body_eager = disconnected_basis
        .operations
        .iter_mut()
        .find(|operation| operation.source_node_id == node_id(10))
        .expect("second body operation is present in the basis");
    body_eager.evaluation = EvaluationPolicy::DemandDriven;
    body_eager.purity = Purity::Pure;
    body_eager.effects = EffectSemantics::None;
    let disconnected = disconnected_basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/loop-demand.yssbi-event", 9, "value")]),
            include_default_results: false,
        })
        .expect("a disconnected carried Loop is deleted without orphan declarations");
    assert!(!contains_region(
        &disconnected.root_region,
        &|region| matches!(region, StructuredControlRegion::Loop { .. })
    ));
    assert!(
        disconnected
            .value_sources
            .iter()
            .all(|source| !matches!(source, PlanValueSource::ControlProduced(..)))
    );
    disconnected.validate().unwrap();

    let plan = result
        .plan
        .unwrap_or_else(|| panic!("loop diagnostics: {:?}", result.analysis.diagnostics));
    let carried = match &plan.root_region {
        region
            if contains_region(region, &|candidate| {
                matches!(candidate, StructuredControlRegion::Loop { .. })
            }) =>
        {
            fn find(
                region: &StructuredControlRegion,
            ) -> Option<&[crate::node_system::plan::LoopCarriedBinding]> {
                use crate::node_system::plan::ControlStep;
                match region {
                    StructuredControlRegion::Loop { carried, .. } => Some(carried),
                    StructuredControlRegion::Sequence(steps) => {
                        steps.iter().find_map(|step| match step {
                            ControlStep::Region(region) => find(region),
                            ControlStep::Operation(_) => None,
                        })
                    }
                    StructuredControlRegion::If {
                        then_region,
                        else_region,
                        ..
                    } => find(then_region).or_else(|| find(else_region)),
                    StructuredControlRegion::Call { .. } => None,
                }
            }
            find(region).unwrap()
        }
        _ => panic!("plan must contain Loop"),
    };
    assert_eq!(carried.len(), 2);
    for (binding, initial_node, next_node) in [(carried[0], 2, 4), (carried[1], 3, 5)] {
        let initial_output =
            plan.operations[operation_index_for_node(&plan, initial_node).index()].outputs[0].value;
        let next_output =
            plan.operations[operation_index_for_node(&plan, next_node).index()].outputs[0].value;
        assert!(plan.value_dependencies.iter().any(|dependency| {
            dependency.source == initial_output && dependency.destination == binding.initial_source
        }));
        assert!(plan.value_dependencies.iter().any(|dependency| {
            dependency.source == next_output && dependency.destination == binding.next_source
        }));
        assert_ne!(binding.body_input, binding.result);
        assert_ne!(binding.initial_source, binding.next_source);
    }
    assert!(contains_region(&plan.root_region, &|region| matches!(
        region,
        StructuredControlRegion::Loop {
            max_iterations: 7,
            ..
        }
    )));
}
