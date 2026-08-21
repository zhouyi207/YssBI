use super::*;

#[test]
fn valid_dynamic_output_derives_without_invalid_plan_fallback() {
    let dynamic_output = PortSpec {
        key: key("items"),
        label_key: I18nKey::new("ports.items.label").unwrap(),
        direction: PortDirection::Output,
        kind: PortKind::Data,
        value_type: TypeExpr::Unknown,
        instances: PortInstances::UserCreated { min: 0, max: None },
        connections: ConnectionsPerPort::Multiple {
            max: None,
            ordered: false,
        },
        input_binding: None,
        consumption: None,
        production: None,
        editor: PortEditorSpec::Default,
        schema: None,
    };
    let protocol = test_protocol(
        "demand_dynamic_output",
        vec![dynamic_output],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![protocol]);
    let mut graph = graph_with_nodes(&[(1, "demand_dynamic_output")]);
    let output = bind_member_port(&mut graph, 1, "items", 10, "a");
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/dynamic".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid dynamic graph has basis");
    let requested = GraphOutputRef {
        graph_path: GraphResourcePath("events/dynamic".into()),
        port: output,
    };

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([requested.clone()]),
            include_default_results: false,
        })
        .unwrap_or_else(|error| panic!("accepted dynamic output must derive directly: {error:?}"));
    assert_eq!(plan.results[0].output, requested);
    plan.validate()
        .expect("dynamic requested-output plan validates");
}

#[test]
fn function_abi_rejects_wrong_dynamic_member_direction() {
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

    let mut parameters = data_port("parameters", PortDirection::Input, TypeExpr::Unknown, None);
    parameters.instances = PortInstances::UserCreated { min: 0, max: None };
    let mut entry = structural_protocol(
        "wrong_direction_entry",
        vec![control_port("then", PortDirection::Output), parameters],
        vec![],
    );
    entry.managed_role = Some(ManagedNodeRole::FunctionEntry);
    entry.scope = NodeScope::Function;
    let entry_type = entry.type_id.clone();
    let mut return_node = structural_protocol(
        "wrong_direction_return",
        vec![control_port("enter", PortDirection::Input)],
        vec![],
    );
    return_node.managed_role = Some(ManagedNodeRole::FunctionReturn);
    return_node.scope = NodeScope::Function;
    let return_type = return_node.type_id.clone();
    let registry = TestRegistry::new(vec![entry, return_node])
        .structural(&entry_type, StructuralNodeRole::FunctionEntry)
        .structural(&return_type, StructuralNodeRole::FunctionReturn);
    let path = GraphResourcePath("functions/wrong-direction".into());
    let parameter = FunctionParameterId("amount".into());
    let resources = FunctionResources {
        path: path.clone(),
        function: FunctionDocument::new(FunctionSignature {
            parameters: vec![FunctionParameter {
                id: parameter.clone(),
                name: "Amount".into(),
                type_name: "Int64".into(),
            }],
            return_type: None,
        }),
        graph: GraphDocument::default(),
    };
    let mut graph =
        graph_with_nodes(&[(1, "wrong_direction_entry"), (2, "wrong_direction_return")]);
    bind_resolved_function_port(&mut graph, 1, "parameters", 10, "a", &path, &parameter);
    connect(&mut graph, 11, 1, "then", 2, "enter");

    let compiler = GraphCompiler::new(&registry, &resources);
    let products = compiler
        .compile_snapshot(
            &compiler.snapshot(path, &graph),
            &CompileCancellationToken::new(),
        )
        .unwrap();

    assert!(products.plan.is_none());
    assert!(products.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.function.abi.endpoint_invalid"
    }));
}
