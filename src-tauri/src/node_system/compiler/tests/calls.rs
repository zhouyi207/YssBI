use super::*;

#[test]
fn lowerability_missing_and_blocking_callees_block_at_call_before_lowering() {
    struct FunctionResources {
        path: GraphResourcePath,
        function: FunctionDocument,
        graph: GraphDocument,
    }
    impl ResourceSnapshot for FunctionResources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            BTreeMap::from([(
                ResourceKey::new(self.path.0.as_ref()),
                ResourceVersion::new("callee-v1"),
            )])
        }

        fn function_name(&self, path: &GraphResourcePath) -> Option<&str> {
            self.function_document(path).map(|_| "Test function")
        }

        fn function_document(&self, path: &GraphResourcePath) -> Option<&FunctionDocument> {
            (path == &self.path).then_some(&self.function)
        }

        fn function_graph_document(&self, path: &GraphResourcePath) -> Option<&GraphDocument> {
            (path == &self.path).then_some(&self.graph)
        }
    }

    let builtins = crate::node_system::catalog::build_builtin_node_system().unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = test_protocol("callee_guard", vec![], vec![], vec![]);
    let registry = AugmentedCompilerRegistry {
        frozen: &builtins.registry,
        protocol: counted,
        implementation: NodeImplementation::new(CountingLowerer(calls.clone())),
    };
    let function_path = GraphResourcePath("functions/blocking".into());
    let blocking_resources = FunctionResources {
        path: function_path.clone(),
        function: FunctionDocument::new(FunctionSignature {
            parameters: Vec::new(),
            return_type: None,
        }),
        graph: builtin_graph_with_nodes(&[(20, "yssbi.test.missing")]),
    };

    for (name, target, expected_code) in [
        (
            "missing",
            GraphResourcePath("functions/missing".into()),
            "compiler.resource.resolution_failed",
        ),
        (
            "blocking",
            function_path,
            "compiler.control.call.abi_invalid",
        ),
    ] {
        calls.store(0, AtomicOrdering::Relaxed);
        let mut graph = builtin_graph_with_nodes(&[
            (1, "yssbi.project.function.call"),
            (2, "yssbi.test.callee_guard"),
            (3, "yssbi.project.function.call"),
        ]);
        set_parameters(
            &mut graph,
            1,
            &[("target", serde_json::json!(target.0.as_ref()))],
        );
        set_parameters(
            &mut graph,
            3,
            &[("target", serde_json::json!(target.0.as_ref()))],
        );
        let trace = RecordingTrace::default();
        let compiler = GraphCompiler::with_interface_resolvers(
            &registry,
            &blocking_resources,
            build_builtin_interface_resolvers(),
        );

        let result = compiler
            .with_observability(ProjectSessionId::new("lowerability"), &trace)
            .compile(&graph);

        assert_analysis_blocks_before_lowering(&result, &trace, &calls);
        let locations = result
            .analysis
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code.as_str() == expected_code)
            .map(|diagnostic| diagnostic.primary.clone())
            .collect::<Vec<_>>();
        for expected in [node_id(1), node_id(3)] {
            assert!(
                locations.contains(&DiagnosticLocation::Node(expected)),
                "missing precise {name} callee diagnostic at {expected}: {:?}",
                result.analysis.diagnostics
            );
        }
    }
}

#[test]
fn nested_blocking_callee_projects_to_root_call_with_exact_basis() {
    struct FunctionResources {
        functions: BTreeMap<GraphResourcePath, FunctionDocument>,
        graphs: BTreeMap<GraphResourcePath, GraphDocument>,
        versions: crate::node_system::analysis::ResourceVersionSet,
    }

    impl ResourceSnapshot for FunctionResources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            self.versions.clone()
        }

        fn function_name(&self, path: &GraphResourcePath) -> Option<&str> {
            self.function_document(path).map(|_| "Test function")
        }

        fn function_document(&self, path: &GraphResourcePath) -> Option<&FunctionDocument> {
            self.functions.get(path)
        }

        fn function_graph_document(&self, path: &GraphResourcePath) -> Option<&GraphDocument> {
            self.graphs.get(path)
        }
    }

    let registry = std::sync::Arc::unwrap_or_clone(
        crate::node_system::catalog::build_builtin_node_system()
            .unwrap()
            .registry,
    );
    let outer_path = GraphResourcePath("functions/outer".into());
    let inner_path = GraphResourcePath("functions/inner".into());
    let signature = FunctionDocument::new(FunctionSignature {
        parameters: Vec::new(),
        return_type: None,
    });
    let mut outer = builtin_graph_with_nodes(&[
        (20, "yssbi.project.function.entry"),
        (21, "yssbi.project.function.call"),
        (22, "yssbi.project.function.return"),
    ]);
    set_parameters(
        &mut outer,
        20,
        &[("function", serde_json::json!(outer_path.0.as_ref()))],
    );
    set_parameters(
        &mut outer,
        21,
        &[("target", serde_json::json!(inner_path.0.as_ref()))],
    );
    set_parameters(
        &mut outer,
        22,
        &[("function", serde_json::json!(outer_path.0.as_ref()))],
    );
    connect(&mut outer, 100, 20, "then", 21, "enter");
    connect(&mut outer, 101, 21, "then", 22, "enter");

    let versions = BTreeMap::from([
        (
            ResourceKey::new(outer_path.0.as_ref()),
            ResourceVersion::new("outer-v1"),
        ),
        (
            ResourceKey::new(inner_path.0.as_ref()),
            ResourceVersion::new("inner-v1"),
        ),
    ]);
    let resources = FunctionResources {
        functions: BTreeMap::from([
            (outer_path.clone(), signature.clone()),
            (inner_path.clone(), signature),
        ]),
        graphs: BTreeMap::from([
            (outer_path.clone(), outer),
            (
                inner_path.clone(),
                builtin_graph_with_nodes(&[(30, "yssbi.test.missing")]),
            ),
        ]),
        versions: versions.clone(),
    };
    let mut caller = builtin_graph_with_nodes(&[(1, "yssbi.project.function.call")]);
    set_parameters(
        &mut caller,
        1,
        &[("target", serde_json::json!(outer_path.0.as_ref()))],
    );
    let trace = RecordingTrace::default();
    let calls = AtomicUsize::new(0);

    let result = GraphCompiler::with_interface_resolvers(
        &registry,
        &resources,
        build_builtin_interface_resolvers(),
    )
    .with_observability(ProjectSessionId::new("nested-call"), &trace)
    .compile(&caller);

    assert_analysis_blocks_before_lowering(&result, &trace, &calls);
    assert_eq!(result.analysis.basis.resource_versions, versions);
    assert!(result.analysis.basis.resource_observations.is_empty());
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.control.call.abi_invalid"
            && diagnostic.primary == DiagnosticLocation::Node(node_id(1))
    }));
    assert!(result.analysis.diagnostics.iter().all(|diagnostic| {
        matches!(diagnostic.primary, DiagnosticLocation::Node(id) if id == node_id(1))
    }));
}

#[test]
fn locally_invalid_callee_still_discovers_complete_outgoing_call_closure() {
    struct FunctionResources {
        functions: BTreeMap<GraphResourcePath, FunctionDocument>,
        graphs: BTreeMap<GraphResourcePath, GraphDocument>,
        versions: crate::node_system::analysis::ResourceVersionSet,
    }

    impl ResourceSnapshot for FunctionResources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            self.versions.clone()
        }

        fn function_name(&self, path: &GraphResourcePath) -> Option<&str> {
            self.function_document(path).map(|_| "Test function")
        }

        fn function_document(&self, path: &GraphResourcePath) -> Option<&FunctionDocument> {
            self.functions.get(path)
        }

        fn function_graph_document(&self, path: &GraphResourcePath) -> Option<&GraphDocument> {
            self.graphs.get(path)
        }
    }

    fn function_graph(
        own_path: &GraphResourcePath,
        calls: &[(u128, &GraphResourcePath)],
        locally_invalid: bool,
    ) -> GraphDocument {
        let mut nodes = vec![(20, "yssbi.project.function.entry")];
        nodes.extend(
            calls
                .iter()
                .map(|(id, _)| (*id, "yssbi.project.function.call")),
        );
        nodes.push((30, "yssbi.project.function.return"));
        if locally_invalid {
            nodes.push((40, "yssbi.test.missing"));
        }
        let mut graph = builtin_graph_with_nodes(&nodes);
        set_parameters(
            &mut graph,
            20,
            &[("function", serde_json::json!(own_path.0.as_ref()))],
        );
        set_parameters(
            &mut graph,
            30,
            &[("function", serde_json::json!(own_path.0.as_ref()))],
        );
        let mut previous = 20;
        for (index, (id, target)) in calls.iter().enumerate() {
            set_parameters(
                &mut graph,
                *id,
                &[("target", serde_json::json!(target.0.as_ref()))],
            );
            connect(
                &mut graph,
                100 + index as u128,
                previous,
                "then",
                *id,
                "enter",
            );
            previous = *id;
        }
        connect(&mut graph, 199, previous, "then", 30, "enter");
        graph
    }

    let registry = std::sync::Arc::unwrap_or_clone(
        crate::node_system::catalog::build_builtin_node_system()
            .unwrap()
            .registry,
    );
    let path_a = GraphResourcePath("functions/local-invalid-a".into());
    let path_b = GraphResourcePath("functions/present-b".into());
    let path_c = GraphResourcePath("functions/missing-c".into());
    let path_d = GraphResourcePath("functions/transitive-d".into());
    let signature = FunctionDocument::new(FunctionSignature {
        parameters: Vec::new(),
        return_type: None,
    });
    let versions = BTreeMap::from([
        (
            ResourceKey::new(path_a.0.as_ref()),
            ResourceVersion::new("a-v1"),
        ),
        (
            ResourceKey::new(path_b.0.as_ref()),
            ResourceVersion::new("b-v1"),
        ),
        (
            ResourceKey::new(path_d.0.as_ref()),
            ResourceVersion::new("d-v1"),
        ),
    ]);
    let resources = FunctionResources {
        functions: BTreeMap::from([
            (path_a.clone(), signature.clone()),
            (path_b.clone(), signature.clone()),
            (path_d.clone(), signature),
        ]),
        graphs: BTreeMap::from([
            (
                path_a.clone(),
                function_graph(&path_a, &[(21, &path_b), (22, &path_c)], true),
            ),
            (
                path_b.clone(),
                function_graph(&path_b, &[(23, &path_d)], false),
            ),
            (path_d.clone(), function_graph(&path_d, &[], false)),
        ]),
        versions: versions.clone(),
    };
    let mut caller = builtin_graph_with_nodes(&[(1, "yssbi.project.function.call")]);
    set_parameters(
        &mut caller,
        1,
        &[("target", serde_json::json!(path_a.0.as_ref()))],
    );

    let result = GraphCompiler::with_interface_resolvers(
        &registry,
        &resources,
        build_builtin_interface_resolvers(),
    )
    .compile(&caller);

    assert_eq!(result.analysis.basis.resource_versions, versions);
    assert_eq!(
        result
            .analysis
            .basis
            .resource_observations
            .keys()
            .map(ResourceKey::as_str)
            .collect::<Vec<_>>(),
        vec![path_c.0.as_ref()],
    );
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.control.call.abi_invalid"
            && diagnostic.primary == DiagnosticLocation::Node(node_id(1))
    }));
    assert!(result.analysis.diagnostics.iter().all(|diagnostic| {
        matches!(diagnostic.primary, DiagnosticLocation::Node(id) if id == node_id(1))
    }));
}

#[test]
fn mutual_call_scc_propagates_external_blocking_to_every_root_site() {
    struct FunctionResources {
        functions: BTreeMap<GraphResourcePath, FunctionDocument>,
        graphs: BTreeMap<GraphResourcePath, GraphDocument>,
        versions: crate::node_system::analysis::ResourceVersionSet,
    }

    impl ResourceSnapshot for FunctionResources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            self.versions.clone()
        }

        fn function_name(&self, path: &GraphResourcePath) -> Option<&str> {
            self.function_document(path).map(|_| "Test function")
        }

        fn function_document(&self, path: &GraphResourcePath) -> Option<&FunctionDocument> {
            self.functions.get(path)
        }

        fn function_graph_document(&self, path: &GraphResourcePath) -> Option<&GraphDocument> {
            self.graphs.get(path)
        }
    }

    fn function_with_calls(
        own_path: &GraphResourcePath,
        calls: &[(u128, &GraphResourcePath)],
    ) -> GraphDocument {
        let mut nodes = vec![(20, "yssbi.project.function.entry")];
        nodes.extend(
            calls
                .iter()
                .map(|(id, _)| (*id, "yssbi.project.function.call")),
        );
        nodes.push((30, "yssbi.project.function.return"));
        let mut graph = builtin_graph_with_nodes(&nodes);
        set_parameters(
            &mut graph,
            20,
            &[("function", serde_json::json!(own_path.0.as_ref()))],
        );
        set_parameters(
            &mut graph,
            30,
            &[("function", serde_json::json!(own_path.0.as_ref()))],
        );
        let mut previous = 20;
        for (index, (id, target)) in calls.iter().enumerate() {
            set_parameters(
                &mut graph,
                *id,
                &[("target", serde_json::json!(target.0.as_ref()))],
            );
            connect(
                &mut graph,
                100 + index as u128,
                previous,
                "then",
                *id,
                "enter",
            );
            previous = *id;
        }
        connect(&mut graph, 199, previous, "then", 30, "enter");
        graph
    }

    let registry = std::sync::Arc::unwrap_or_clone(
        crate::node_system::catalog::build_builtin_node_system()
            .unwrap()
            .registry,
    );
    let path_a = GraphResourcePath("functions/cycle-a".into());
    let path_b = GraphResourcePath("functions/cycle-b".into());
    let blocking_path = GraphResourcePath("functions/cycle-z-blocking".into());
    let signature = FunctionDocument::new(FunctionSignature {
        parameters: Vec::new(),
        return_type: None,
    });
    let versions = [
        (&path_a, "a-v1"),
        (&path_b, "b-v1"),
        (&blocking_path, "c-v1"),
    ]
    .into_iter()
    .map(|(path, version)| {
        (
            ResourceKey::new(path.0.as_ref()),
            ResourceVersion::new(version),
        )
    })
    .collect();
    let resources = FunctionResources {
        functions: BTreeMap::from([
            (path_a.clone(), signature.clone()),
            (path_b.clone(), signature.clone()),
            (blocking_path.clone(), signature),
        ]),
        graphs: BTreeMap::from([
            (
                path_a.clone(),
                function_with_calls(&path_a, &[(21, &path_b), (22, &blocking_path)]),
            ),
            (
                path_b.clone(),
                function_with_calls(&path_b, &[(23, &path_a)]),
            ),
            (
                blocking_path.clone(),
                builtin_graph_with_nodes(&[(40, "yssbi.test.missing")]),
            ),
        ]),
        versions,
    };
    let mut caller = builtin_graph_with_nodes(&[
        (1, "yssbi.project.function.call"),
        (2, "yssbi.project.function.call"),
        (3, "yssbi.project.function.call"),
        (4, "yssbi.project.function.call"),
    ]);
    for (id, target) in [(1, &path_a), (2, &path_a), (3, &path_b), (4, &path_b)] {
        set_parameters(
            &mut caller,
            id,
            &[("target", serde_json::json!(target.0.as_ref()))],
        );
    }

    let result = GraphCompiler::with_interface_resolvers(
        &registry,
        &resources,
        build_builtin_interface_resolvers(),
    )
    .compile(&caller);

    let invalid_sites = result
        .analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code.as_str() == "compiler.control.call.abi_invalid")
        .filter_map(|diagnostic| match diagnostic.primary {
            DiagnosticLocation::Node(id) => Some(id),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        invalid_sites,
        BTreeSet::from([node_id(1), node_id(2), node_id(3), node_id(4)])
    );
    assert_eq!(result.analysis.basis.resource_versions, resources.versions);
    assert!(result.analysis.basis.resource_observations.is_empty());
    assert!(result.semantic.is_none());
    assert!(result.plan.is_none());
}

#[test]
fn function_abi_managed_role_error_emits_expected_role_and_actual_count() {
    struct FunctionResources {
        path: GraphResourcePath,
        function: FunctionDocument,
        graph: GraphDocument,
    }
    impl ResourceSnapshot for FunctionResources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            BTreeMap::from([(
                ResourceKey::new(self.path.0.clone()),
                ResourceVersion::new("fixture-v1"),
            )])
        }

        fn function_name(&self, path: &GraphResourcePath) -> Option<&str> {
            self.function_document(path).map(|_| "Test function")
        }

        fn function_document(&self, path: &GraphResourcePath) -> Option<&FunctionDocument> {
            (path == &self.path).then_some(&self.function)
        }

        fn function_graph_document(&self, path: &GraphResourcePath) -> Option<&GraphDocument> {
            (path == &self.path).then_some(&self.graph)
        }
    }

    let mut entry = structural_protocol(
        "duplicate_function_entry",
        vec![control_port("then", PortDirection::Output)],
        vec![],
    );
    entry.managed_role = Some(ManagedNodeRole::FunctionEntry);
    entry.scope = NodeScope::Function;
    let entry_type = entry.type_id.clone();
    let mut return_node = structural_protocol(
        "single_function_return",
        vec![control_port("enter", PortDirection::Input)],
        vec![],
    );
    return_node.managed_role = Some(ManagedNodeRole::FunctionReturn);
    return_node.scope = NodeScope::Function;
    let return_type = return_node.type_id.clone();
    let registry = TestRegistry::new(vec![entry, return_node])
        .structural(&entry_type, StructuralNodeRole::FunctionEntry)
        .structural(&return_type, StructuralNodeRole::FunctionReturn);
    let path = GraphResourcePath("functions/duplicate-entry".into());
    let resources = FunctionResources {
        path: path.clone(),
        function: FunctionDocument::new(FunctionSignature {
            parameters: vec![],
            return_type: None,
        }),
        graph: GraphDocument::default(),
    };
    let graph = graph_with_nodes(&[
        (1, "duplicate_function_entry"),
        (2, "duplicate_function_entry"),
        (3, "single_function_return"),
    ]);
    let compiler = GraphCompiler::new(&registry, &resources);

    let products = compiler
        .compile_snapshot(
            &compiler.snapshot(path, &graph),
            &CompileCancellationToken::new(),
        )
        .unwrap();

    let diagnostic = products
        .analysis
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "compiler.function.abi.managed_role_invalid")
        .expect("managed role count diagnostic");
    assert_eq!(
        diagnostic.arguments,
        BTreeMap::from([
            (Box::from("actual_count"), Box::from("2")),
            (Box::from("expected_role"), Box::from("function_entry")),
        ])
    );
    let singleton = products
        .analysis
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "compiler.node.managed_singleton")
        .expect("managed singleton diagnostic");
    assert_eq!(
        singleton.arguments,
        BTreeMap::from([(Box::from("managed_role"), Box::from("function_entry"))])
    );
}

#[test]
fn missing_call_resource_parameter_is_blocking() {
    let call = structural_protocol("call_missing_target", vec![], vec![]);
    let call_type = call.type_id.clone();
    let registry = TestRegistry::new(vec![call]).structural(&call_type, StructuralNodeRole::Call);
    let graph = graph_with_nodes(&[(1, "call_missing_target")]);

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(result.plan.is_none());
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.control.call.resource_parameter_missing"
    }));
}

#[test]
fn structural_nodes_never_invoke_leaf_lowerers() {
    let sequence = structural_protocol(
        "structural_only",
        vec![control_port("then", PortDirection::Output)],
        vec![],
    );
    let sequence_type = sequence.type_id.clone();
    let mut registry =
        TestRegistry::new(vec![sequence]).structural(&sequence_type, StructuralNodeRole::Sequence);
    registry.nodes.get_mut(&sequence_type).unwrap().1 = NodeImplementation::new(PanicLowerer);
    let graph = graph_with_nodes(&[(1, "structural_only")]);

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    let plan = result
        .plan
        .expect("structural-only graph should produce a plan");
    assert!(plan.operations.is_empty());
}

#[test]
fn branch_function_abi_finalizes_from_lowered_result_source() {
    let (plan, mut abi, result) = structured_function_plan(
        StructuredControlRegion::If {
            condition: ValueRef::new(3),
            then_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            else_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            results: Box::new([BranchResultBinding {
                destination: ValueRef::new(2),
                then_source: ValueRef::new(0),
                else_source: ValueRef::new(1),
                production: Some(OutputProduction::Streaming),
            }]),
        },
        Box::new([
            PlanValueSource::ExternalInput(ValueRef::new(0), OutputProduction::Streaming),
            PlanValueSource::ExternalInput(ValueRef::new(1), OutputProduction::Streaming),
            PlanValueSource::ExternalInput(ValueRef::new(3), OutputProduction::FullyMaterialized),
            PlanValueSource::ControlProduced(ValueRef::new(2), OutputProduction::Streaming),
        ]),
        Box::new([ValueDependency {
            source: ValueRef::new(2),
            destination: ValueRef::new(4),
        }]),
        5,
        ValueRef::new(4),
    );

    crate::node_system::compiler::pipeline::finalize_function_abi_productions(&plan, &mut abi)
        .unwrap();

    assert_eq!(abi.result_productions[&result], OutputProduction::Streaming);
}

#[test]
fn loop_function_abi_finalizes_from_lowered_result_source() {
    let (plan, mut abi, result) = structured_function_plan(
        StructuredControlRegion::Loop {
            body: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            carried: Box::new([LoopCarriedBinding {
                initial_source: ValueRef::new(0),
                body_input: ValueRef::new(2),
                next_source: ValueRef::new(1),
                result: ValueRef::new(4),
                production: Some(OutputProduction::Streaming),
            }]),
            continue_condition: ValueRef::new(3),
            max_iterations: 2,
        },
        Box::new([
            PlanValueSource::ExternalInput(ValueRef::new(0), OutputProduction::Streaming),
            PlanValueSource::ExternalInput(ValueRef::new(1), OutputProduction::Streaming),
            PlanValueSource::ExternalInput(ValueRef::new(3), OutputProduction::FullyMaterialized),
            PlanValueSource::ControlProduced(ValueRef::new(2), OutputProduction::Streaming),
            PlanValueSource::ControlProduced(ValueRef::new(4), OutputProduction::Streaming),
        ]),
        Box::new([]),
        5,
        ValueRef::new(4),
    );

    crate::node_system::compiler::pipeline::finalize_function_abi_productions(&plan, &mut abi)
        .unwrap();

    assert_eq!(abi.result_productions[&result], OutputProduction::Streaming);
}

#[test]
fn call_function_abi_finalizes_from_lowered_result_source() {
    let (plan, mut abi, result) = structured_function_plan(
        StructuredControlRegion::Call {
            target: FunctionPlanHandle::new("functions/callee").unwrap(),
            arguments: Box::new([]),
            results: Box::new([CallResultBinding {
                callee_source: ValueRef::new(0),
                caller_destination: ValueRef::new(1),
                production: Some(OutputProduction::Streaming),
            }]),
            mandatory: true,
        },
        Box::new([PlanValueSource::ControlProduced(
            ValueRef::new(1),
            OutputProduction::Streaming,
        )]),
        Box::new([]),
        2,
        ValueRef::new(1),
    );

    crate::node_system::compiler::pipeline::finalize_function_abi_productions(&plan, &mut abi)
        .unwrap();

    assert_eq!(abi.result_productions[&result], OutputProduction::Streaming);
}
