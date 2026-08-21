use super::*;

#[test]
fn type_conformance_requires_every_source_union_member() {
    let source = TypeExpr::Union(vec![concrete("core.int64"), concrete("core.string")]);

    assert_eq!(
        type_exprs_compatibility(&source, &concrete("core.int64"), &[], &[]),
        TypeCompatibility::Incompatible
    );
}

#[test]
fn type_conformance_accepts_numeric_series_members() {
    let target = numeric_data_series_type();

    assert_eq!(
        type_exprs_compatibility(&data_series_type(concrete("core.int64")), &target, &[], &[],),
        TypeCompatibility::Compatible
    );
}

#[test]
fn type_conformance_reports_unknown_as_indeterminate() {
    assert_eq!(
        type_exprs_compatibility(
            &TypeExpr::Unknown,
            &data_series_type(concrete("core.float64")),
            &[],
            &[],
        ),
        TypeCompatibility::Indeterminate
    );
}

#[test]
fn semantic_graph_preserves_protocol_port_order_for_kernel_abi() {
    let protocol = test_protocol(
        "ordered_outputs",
        vec![
            data_port("z_result", PortDirection::Output, TypeExpr::Unknown, None),
            data_port("a_report", PortDirection::Output, TypeExpr::Unknown, None),
        ],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![protocol]);
    let result = GraphCompiler::new(&registry, &Resources).compile(&document(
        NodeTypeId::new("yssbi.test.ordered_outputs").unwrap(),
    ));
    let semantic = result.semantic.expect("ordered output graph must compile");
    let keys = semantic.nodes[0]
        .ports
        .iter()
        .map(|port| match &port.address.port {
            crate::node_system::document::PortRef::Declared { key } => key.as_str(),
            crate::node_system::document::PortRef::Instance { .. } => {
                panic!("expected declared output")
            }
        })
        .collect::<Vec<_>>();

    assert_eq!(keys, ["z_result", "a_report"]);
}

#[test]
fn planned_outputs_preserve_protocol_order_identity_and_presentation() {
    let mut protocol = test_protocol(
        "report",
        vec![
            data_port("z_result", PortDirection::Output, TypeExpr::Unknown, None),
            data_port("report", PortDirection::Output, TypeExpr::Unknown, None),
        ],
        vec![],
        vec![],
    );
    protocol.type_id = NodeTypeId::new("yssbi.statistics.ols.summary").unwrap();
    let registry = TestRegistry::new(vec![protocol]);
    let result = GraphCompiler::new(&registry, &Resources).compile(&document(
        NodeTypeId::new("yssbi.statistics.ols.summary").unwrap(),
    ));
    let plan = result.plan.expect("report fixture must lower");
    let operation = plan
        .operations
        .iter()
        .find(|operation| operation.source_node_type_id.as_str() == "yssbi.statistics.ols.summary")
        .unwrap();

    assert_eq!(
        operation
            .outputs
            .iter()
            .map(
                |output| match &output.public_output.as_ref().unwrap().port.port {
                    PortRef::Declared { key } => key.as_str(),
                    PortRef::Instance { template, .. } => template.as_str(),
                }
            )
            .collect::<Vec<_>>(),
        ["z_result", "report"],
    );
    assert_eq!(
        operation.outputs[0].presentation,
        ResultPresentation::Inspector
    );
    assert_eq!(
        operation.outputs[1].presentation,
        ResultPresentation::Report {
            report: ResultReportKind::OlsSummary,
        },
    );
}

#[test]
fn adapter_output_has_no_public_pin_and_inherits_presentation() {
    let mut report = test_protocol(
        "report",
        vec![data_port(
            "report",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    report.type_id = NodeTypeId::new("yssbi.statistics.ols.summary").unwrap();
    report.interface.ports[0].production = Some(OutputProduction::Streaming);
    let mut consumer = test_protocol(
        "report_consumer",
        vec![data_port(
            "input",
            PortDirection::Input,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    consumer.interface.ports[0].consumption = Some(InputConsumption::FullyMaterialized);
    let registry = TestRegistry::new(vec![report, consumer]);
    let mut graph = graph_with_nodes(&[(1, "report"), (2, "report_consumer")]);
    graph.nodes.get_mut(&node_id(1)).unwrap().node_type =
        NodeTypeId::new("yssbi.statistics.ols.summary").unwrap();
    connect(&mut graph, 1, 1, "report", 2, "input");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result.plan.expect("report adapter fixture must lower");
    let adapter = plan
        .operations
        .iter()
        .find(|operation| matches!(operation.kernel, PlannedKernel::Adapter(_)))
        .unwrap();

    assert!(adapter.outputs[0].public_output.is_none());
    assert_eq!(
        adapter.outputs[0].presentation,
        ResultPresentation::Report {
            report: ResultReportKind::OlsSummary,
        },
    );
}

#[test]
fn generic_binding_is_written_to_type_facts() {
    let generic = TypeParameterId::new("value").unwrap();
    let source = test_protocol(
        "int_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Concrete(type_id("core.int")),
            None,
        )],
        vec![],
        vec![],
    );
    let passthrough = test_protocol(
        "generic",
        vec![
            data_port(
                "in",
                PortDirection::Input,
                TypeExpr::Generic(generic.clone()),
                None,
            ),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Generic(generic),
                None,
            ),
        ],
        vec![TypeParameterId::new("value").unwrap()],
        vec![TypeConstraint::Equal(
            TypeTerm::Port(key("in")),
            TypeTerm::Port(key("out")),
        )],
    );
    let registry = TestRegistry::new(vec![source, passthrough]);
    let mut graph = graph_with_nodes(&[(1, "int_source"), (2, "generic")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(result.plan.is_some());
    assert_eq!(
        result
            .analysis
            .partial_types
            .get(&PortAddress::declared(node_id(2), key("out"))),
        Some(&TypeExpr::Concrete(type_id("core.int")))
    );
}

#[test]
fn implements_and_element_of_solve_registered_type_shapes() {
    let numeric = TypeClassId::new("core.numeric").unwrap();
    let iterable = TypeClassId::new("core.iterable").unwrap();
    let integer = type_id("core.int");
    let float = type_id("core.float");
    let list = TypeConstructorId::new("core.list").unwrap();
    let pair = TypeConstructorId::new("core.pair").unwrap();
    let generic = TypeParameterId::new("item").unwrap();
    let protocol = test_protocol(
        "type_constraints",
        vec![
            data_port(
                "concrete",
                PortDirection::Output,
                TypeExpr::Concrete(integer.clone()),
                None,
            ),
            data_port(
                "generic",
                PortDirection::Output,
                TypeExpr::Generic(generic.clone()),
                None,
            ),
            data_port(
                "applied",
                PortDirection::Output,
                TypeExpr::Applied {
                    constructor: list.clone(),
                    arguments: vec![TypeExpr::Concrete(integer.clone())],
                },
                None,
            ),
            data_port(
                "union",
                PortDirection::Output,
                TypeExpr::Union(vec![
                    TypeExpr::Concrete(integer.clone()),
                    TypeExpr::Concrete(float.clone()),
                ]),
                None,
            ),
            data_port(
                "pair",
                PortDirection::Output,
                TypeExpr::Applied {
                    constructor: pair.clone(),
                    arguments: vec![
                        TypeExpr::Concrete(integer.clone()),
                        TypeExpr::Concrete(float.clone()),
                    ],
                },
                None,
            ),
        ],
        vec![generic.clone()],
        vec![
            TypeConstraint::Equal(
                TypeTerm::Expr(TypeExpr::Generic(generic)),
                TypeTerm::Expr(TypeExpr::Concrete(integer.clone())),
            ),
            TypeConstraint::Implements(TypeTerm::Port(key("concrete")), numeric.clone()),
            TypeConstraint::Implements(TypeTerm::Port(key("generic")), numeric.clone()),
            TypeConstraint::Implements(TypeTerm::Port(key("applied")), iterable.clone()),
            TypeConstraint::Implements(TypeTerm::Port(key("union")), numeric.clone()),
            TypeConstraint::ElementOf(
                TypeTerm::Port(key("concrete")),
                TypeTerm::Port(key("applied")),
            ),
            TypeConstraint::ElementOf(TypeTerm::Port(key("union")), TypeTerm::Port(key("pair"))),
        ],
    );
    let registry = TestRegistry::new(vec![protocol])
        .with_type_class(integer, numeric.clone())
        .with_type_class(float, numeric)
        .with_constructor(list, 1, [iterable])
        .with_constructor(pair, 2, []);

    let result = GraphCompiler::new(&registry, &Resources)
        .compile(&graph_with_nodes(&[(1, "type_constraints")]));

    assert!(result.plan.is_some(), "{:?}", result.analysis.diagnostics);
    assert!(result.analysis.diagnostics.is_empty());
}

#[test]
fn incompatible_types_are_blocking_and_produce_no_plan() {
    let source = test_protocol(
        "int_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Concrete(type_id("core.int")),
            None,
        )],
        vec![],
        vec![],
    );
    let sink = test_protocol(
        "string_sink",
        vec![data_port(
            "in",
            PortDirection::Input,
            TypeExpr::Concrete(type_id("core.string")),
            None,
        )],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source, sink]);
    let mut graph = graph_with_nodes(&[(1, "int_source"), (2, "string_sink")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(result.plan.is_none());
    assert!(
        result
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "compiler.type.incompatible")
    );
}

#[test]
fn production_diagnostics_emit_canonical_protocol_enum_facts() {
    let protocol = structural_protocol("managed_role_mismatch", vec![], vec![]);
    let issues = super::super::control::validate_structural_contract(
        node_id(1),
        StructuralNodeRole::FunctionEntry,
        &protocol,
        &BTreeMap::new(),
    );
    let managed_role = issues
        .iter()
        .find(|issue| {
            issue.diagnostic.definition().code == "compiler.control.managed_role_mismatch"
        })
        .expect("managed-role mismatch");
    let managed_role = managed_role
        .diagnostic
        .clone()
        .into_node(DiagnosticLocation::Node(node_id(1)));
    assert_eq!(
        managed_role.arguments,
        BTreeMap::from([
            (Box::from("actual_role"), Box::from("none")),
            (Box::from("expected_role"), Box::from("function_entry")),
        ])
    );

    let mut scoped = test_protocol("function_scoped", vec![], vec![], vec![]);
    scoped.scope = NodeScope::Function;
    let registry = TestRegistry::new(vec![scoped]);
    let scope_graph = graph_with_nodes(&[(2, "function_scoped")]);
    let scope_compiler = GraphCompiler::new(&registry, &Resources);
    let scope_result = scope_compiler
        .compile_snapshot(
            &scope_compiler.snapshot(
                GraphResourcePath("events/canonical-facts".into()),
                &scope_graph,
            ),
            &CompileCancellationToken::new(),
        )
        .unwrap();
    let scope = scope_result
        .analysis
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "compiler.node.scope_mismatch")
        .expect("scope mismatch");
    assert_eq!(
        scope.arguments,
        BTreeMap::from([
            (Box::from("actual_scope"), Box::from("function")),
            (Box::from("expected_scope"), Box::from("event")),
        ])
    );

    let source = test_protocol(
        "kind_source",
        vec![data_port(
            "value",
            PortDirection::Output,
            TypeExpr::Concrete(type_id("core.int64")),
            None,
        )],
        vec![],
        vec![],
    );
    let sink = test_protocol(
        "kind_sink",
        vec![effect_port("effect", PortDirection::Input)],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source, sink]);
    let mut graph = graph_with_nodes(&[(3, "kind_source"), (4, "kind_sink")]);
    connect(&mut graph, 301, 3, "value", 4, "effect");
    let kind_result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let kind = kind_result
        .analysis
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "compiler.connection.kind_mismatch")
        .expect("connection-kind mismatch");
    assert_eq!(
        kind.arguments,
        BTreeMap::from([
            (Box::from("source_kind"), Box::from("data")),
            (Box::from("target_kind"), Box::from("effect")),
        ])
    );
}
