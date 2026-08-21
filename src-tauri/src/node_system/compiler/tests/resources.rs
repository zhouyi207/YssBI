use super::*;

#[test]
fn concrete_resource_lowerers_force_database_variable_and_filesystem_retry_off() {
    let policy = RetryPolicy::new(
        std::num::NonZeroU32::new(2).unwrap(),
        std::time::Duration::ZERO,
        std::time::Duration::ZERO,
    )
    .unwrap();
    for (name, resource, kind) in [
        (
            "retry_database_write",
            "database/main",
            ResourceKind::DatabaseConnection,
        ),
        (
            "retry_variable_write",
            "variables/value",
            ResourceKind::ExternalArtifact,
        ),
        (
            "retry_filesystem_write",
            "filesystem/output",
            ResourceKind::TemporaryStorage,
        ),
    ] {
        let mut protocol = test_protocol(name, vec![], vec![], vec![]);
        protocol.execution.idempotent = true;
        protocol.execution.retry = Some(policy);
        let node_type = protocol.type_id.clone();
        let registry = TestRegistry::new(vec![protocol]).with_lowerer(
            &node_type,
            FragmentLowerer {
                fragment: kernel_fragment(
                    EffectSemantics::None,
                    FragmentMetadata {
                        resources: Box::new([CompiledResourceRequirement {
                            resource: ResourceId::new(resource).unwrap(),
                            kind,
                            access: ResourceAccess::Exclusive,
                            optional: false,
                        }]),
                        ..FragmentMetadata::default()
                    },
                ),
            },
        );
        let plan = GraphCompiler::new(&registry, &Resources)
            .compile(&graph_with_nodes(&[(1, name)]))
            .plan
            .unwrap();
        assert_eq!(plan.operations[0].retry, PlannedRetry::default(), "{name}");
    }
}

#[test]
fn compiler_aggregates_fragment_resources_and_results() {
    let protocol = test_protocol(
        "plan_metadata",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let node_type = protocol.type_id.clone();
    let resource = CompiledResourceRequirement {
        resource: ResourceId::new("database.main").unwrap(),
        kind: ResourceKind::DatabaseConnection,
        access: ResourceAccess::Shared,
        optional: false,
    };
    let registry = TestRegistry::new(vec![protocol]).with_lowerer(
        &node_type,
        FragmentLowerer {
            fragment: kernel_fragment(
                EffectSemantics::None,
                FragmentMetadata {
                    effect: EffectSemantics::None,
                    resources: Box::new([resource.clone()]),
                    results: Box::new([FragmentResult {
                        name: "answer".into(),
                        output: PortAddress::declared(node_id(1), key("out")),
                    }]),
                },
            ),
        },
    );

    let plan = GraphCompiler::new(&registry, &Resources)
        .compile(&graph_with_nodes(&[(1, "plan_metadata")]))
        .plan
        .expect("metadata graph should lower");

    assert_eq!(plan.resources.as_ref(), &[resource]);
    assert_eq!(
        plan.operations[0].resource_dependencies.as_ref(),
        &[crate::node_system::analysis::ResourceKey::new(
            "database.main"
        )]
    );
    assert_eq!(
        plan.results.as_ref(),
        &[PlanResult {
            name: "answer".into(),
            output: GraphOutputRef {
                graph_path: plan.provenance.graph_path.clone(),
                port: PortAddress::declared(node_id(1), key("out")),
            },
            value: plan.operations[0].outputs[0].value,
        }]
    );
}

#[test]
fn demand_specialization_prunes_independent_pure_chain_and_owned_resource() {
    let (registry, graph) = demand_fixture();
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/main".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid graph has lowering basis");

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/main", 2, "out")]),
            include_default_results: false,
        })
        .unwrap();

    assert_eq!(
        plan.operations
            .iter()
            .filter(|operation| !matches!(operation.kernel, PlannedKernel::Adapter(_)))
            .map(|operation| operation.source_node_id)
            .collect::<Vec<_>>(),
        vec![node_id(1), node_id(2)]
    );
    assert_eq!(plan.results.len(), 1);
    assert_eq!(
        plan.results[0].output,
        demand_output("events/main", 2, "out")
    );
    assert_eq!(
        plan.resources
            .iter()
            .map(|requirement| requirement.resource.as_str())
            .collect::<Vec<_>>(),
        vec!["database.chain-a"]
    );
}

#[test]
fn value_dependency_cycle_is_blocking() {
    let relay = test_protocol(
        "relay",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port("out", PortDirection::Output, TypeExpr::Unknown, None),
        ],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![relay]);
    let mut graph = graph_with_nodes(&[(1, "relay"), (2, "relay")]);
    connect(&mut graph, 10, 1, "out", 2, "in");
    connect(&mut graph, 11, 2, "out", 1, "in");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(result.plan.is_none());
    assert!(
        result
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code.as_str() == "compiler.dependency.value_cycle" })
    );
}

#[test]
fn rename_dataframe_configured_builtin_reaches_relational_plan() {
    let result = compile_builtin_rename(serde_json::json!("a"), serde_json::json!("renamed"), true);

    assert!(
        result.semantic.is_some(),
        "{:?}",
        result.analysis.diagnostics
    );
    let plan = result.plan.expect("configured Rename must lower");
    assert!(result.analysis.diagnostics.is_empty());
    assert_eq!(
        result
            .analysis
            .partial_schemas
            .get(&PortAddress::declared(node_id(2), key("result"))),
        Some(&SchemaExpr::Rename {
            input: Box::new(SchemaExpr::Input(key("raw"))),
            mapping: RenameExpr::Explicit(vec![ColumnRename {
                from: SchemaColumnRef("a".into()),
                to: SchemaColumnRef("renamed".into()),
            }]),
        })
    );
    assert_eq!(plan.relational_subplans.len(), 1);
    assert_eq!(
        plan.relational_subplans[0].compiled_plan.operators.as_ref(),
        [
            RelationalOperator::Source {
                resource: ResourceId::new("databases/main").unwrap(),
                relation: "databases/main".into(),
            },
            RelationalOperator::Rename {
                input: RelationalOperatorIndex::new(0),
                columns: Box::new([crate::node_system::plan::RelationalRename {
                    from: "a".into(),
                    to: "renamed".into(),
                }]),
            },
        ]
    );
}

#[test]
fn builtin_relational_chain_specializes_final_and_intermediate_demands() {
    let graph_path = "events/task-3-fix-round-1";
    let result = compile_builtin_relational_chain(graph_path);
    assert!(
        result.analysis.diagnostics.is_empty(),
        "{:?}",
        result.analysis.diagnostics
    );
    let basis = result.execution_basis.expect("chain keeps demand basis");
    assert_eq!(
        basis.provenance.graph_path,
        GraphResourcePath(graph_path.into())
    );
    assert_eq!(
        basis.provenance.basis.resource_versions,
        BTreeMap::from([(
            ResourceKey::new("databases/main"),
            ResourceVersion::new("fixture-v1"),
        )])
    );

    let final_plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output(graph_path, 5, "result")]),
            include_default_results: false,
        })
        .unwrap();
    assert_eq!(final_plan.operations.len(), 1);
    assert_eq!(final_plan.relational_subplans.len(), 1);

    let final_relational = &final_plan.relational_subplans[0].compiled_plan;
    assert_eq!(final_relational.operators.len(), 5);
    assert_eq!(
        final_relational.roots.as_ref(),
        [RelationalOperatorIndex::new(4)]
    );
    assert_eq!(
        final_relational.fragment_order.as_ref(),
        [1_u128, 2, 3, 4, 5]
            .map(|id| RelationalFragmentId::new(format!("node.{}", node_id(id))).unwrap())
            .as_slice()
    );
    assert_eq!(
        final_relational.pushdown_hints.as_ref(),
        [
            RelationalPushdownHint::Projection {
                source: RelationalOperatorIndex::new(0),
                columns: Box::new(["a".into(), "b".into()]),
            },
            RelationalPushdownHint::Predicate {
                source: RelationalOperatorIndex::new(0),
                predicate: RelationalExpression::Equal(
                    Box::new(RelationalExpression::Column("b".into())),
                    Box::new(RelationalExpression::Literal(RelationalLiteral::String(
                        "paid".into(),
                    ))),
                ),
            },
        ]
    );
    final_plan.validate().unwrap();

    for (node, expected_fragments, expected_operators) in [(2, 2, 2), (3, 3, 3)] {
        let selected_output = demand_output(graph_path, node, "result");
        let plan = basis
            .derive_plan(&ExecutionDemand::Outputs {
                outputs: Box::new([selected_output.clone()]),
                include_default_results: false,
            })
            .unwrap();
        assert_eq!(plan.operations.len(), 1);
        assert_eq!(plan.results.len(), 1);
        assert_eq!(plan.results[0].output, selected_output);
        assert_eq!(plan.results[0].output.graph_path.0.as_ref(), graph_path);
        assert_eq!(
            plan.results[0].output.port,
            PortAddress::declared(node_id(node), key("result"))
        );
        let relational = &plan.relational_subplans[0].compiled_plan;
        let expected_prefix = (1..=node)
            .map(|id| RelationalFragmentId::new(format!("node.{}", node_id(id))).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(relational.fragment_order.as_ref(), expected_prefix);
        assert_eq!(relational.operators.len(), expected_operators);
        assert_eq!(relational.roots.len(), 1);
        for suffix_node in (node + 1)..=5 {
            let suffix =
                RelationalFragmentId::new(format!("node.{}", node_id(suffix_node))).unwrap();
            assert!(!relational.fragment_order.contains(&suffix));
            assert!(
                relational
                    .fragment_roots
                    .iter()
                    .all(|root| root.fragment != suffix)
            );
        }
        assert_eq!(relational.fragment_order.len(), expected_fragments);
        plan.validate().unwrap();
    }
}

#[test]
fn rename_dataframe_rejects_invalid_scalar_parameter_types() {
    for (from, to, expected_key) in [
        (serde_json::json!(1), serde_json::json!("renamed"), "from"),
        (serde_json::json!("a"), serde_json::json!(false), "to"),
    ] {
        let result = compile_builtin_rename(from, to, true);
        assert!(result.plan.is_none());
        assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
            matches!(
                &diagnostic.primary,
                crate::node_system::analysis::DiagnosticLocation::Parameter { node_id: id, key }
                    if *id == node_id(2) && key.as_str() == expected_key
            )
        }));
    }
}

#[test]
fn rename_dataframe_rejects_empty_or_whitespace_padded_names() {
    for (from, to) in [
        ("", "renamed"),
        ("a", ""),
        (" a", "renamed"),
        ("a", "renamed "),
    ] {
        let result = compile_builtin_rename(serde_json::json!(from), serde_json::json!(to), true);
        assert!(result.plan.is_none(), "{from:?} -> {to:?}");
        assert!(
            rename_diagnostic_codes(&result).contains("compiler.schema.parameter_invalid"),
            "{:?}",
            result.analysis.diagnostics
        );
    }
}

#[test]
fn rename_dataframe_rejects_missing_source_and_destination_collision() {
    for (from, to, code) in [
        ("missing", "renamed", "compiler.schema.rename_field_missing"),
        ("a", "b", "compiler.schema.rename_target_conflict"),
    ] {
        let result = compile_builtin_rename(serde_json::json!(from), serde_json::json!(to), true);
        assert!(result.plan.is_none());
        assert!(
            rename_diagnostic_codes(&result).contains(code),
            "{:?}",
            result.analysis.diagnostics
        );
    }
}

#[test]
fn rename_dataframe_same_name_is_schema_noop_but_still_lowers() {
    let result = compile_builtin_rename(serde_json::json!("a"), serde_json::json!("a"), true);

    assert!(result.plan.is_some(), "{:?}", result.analysis.diagnostics);
    assert!(result.analysis.diagnostics.is_empty());
    assert_eq!(
        result
            .analysis
            .partial_schemas
            .get(&PortAddress::declared(node_id(2), key("result"))),
        Some(&SchemaExpr::Input(key("raw")))
    );
}

#[test]
fn rename_dataframe_unknown_input_schema_blocks_with_existing_diagnostic() {
    let result =
        compile_builtin_rename(serde_json::json!("a"), serde_json::json!("renamed"), false);

    assert!(result.plan.is_none());
    assert!(
        rename_diagnostic_codes(&result).contains("compiler.schema.resolver_missing"),
        "{:?}",
        result.analysis.diagnostics
    );
}

#[test]
fn schema_filter_project_and_rename_are_evaluated_into_facts() {
    let resolver_id = SchemaResolverId::new("test.source_schema").unwrap();
    let source = test_protocol(
        "schema_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            Some(SchemaExpr::Derived {
                resolver: resolver_id.clone(),
                dependencies: vec![],
            }),
        )],
        vec![],
        vec![],
    );
    let mut filter = test_protocol(
        "schema_filter",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Unknown,
                Some(SchemaExpr::Filter {
                    input: Box::new(SchemaExpr::Input(key("in"))),
                    predicate: Some(ParameterKey::new("predicate").unwrap()),
                }),
            ),
        ],
        vec![],
        vec![],
    );
    filter.parameters = ParameterSchema::new(vec![ParameterSpec {
        key: ParameterKey::new("predicate").unwrap(),
        title_key: I18nKey::new("parameters.predicate.title").unwrap(),
        description_key: None,
        value_type: TypeExpr::Concrete(type_id(
            crate::node_system::protocol::dataframe::FILTER_PREDICATE_TYPE_ID,
        )),
        default_value: None,
        constraints: vec![ParameterConstraint::Required],
        editor: ParameterEditorSpec::Auto,
        presentation: ParameterPresentation::DetailPanel,
    }])
    .unwrap();
    let project = test_protocol(
        "schema_project",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Unknown,
                Some(SchemaExpr::Project {
                    input: Box::new(SchemaExpr::Input(key("in"))),
                    columns: ColumnSelectionExpr::Explicit(vec![SchemaColumnRef("a".into())]),
                }),
            ),
        ],
        vec![],
        vec![],
    );
    let rename = test_protocol(
        "schema_rename",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Unknown,
                Some(SchemaExpr::Rename {
                    input: Box::new(SchemaExpr::Input(key("in"))),
                    mapping: RenameExpr::Explicit(vec![ColumnRename {
                        from: SchemaColumnRef("a".into()),
                        to: SchemaColumnRef("renamed".into()),
                    }]),
                }),
            ),
        ],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source, filter, project, rename]).with_nominal_registry(
        crate::node_system::catalog::build_builtin_node_system()
            .unwrap()
            .registry,
    );
    let mut resolvers = SchemaResolverSet::new();
    resolvers.insert(resolver_id, SourceSchemaResolver);
    let mut graph = graph_with_nodes(&[
        (1, "schema_source"),
        (2, "schema_filter"),
        (3, "schema_project"),
        (4, "schema_rename"),
    ]);
    graph.nodes.get_mut(&node_id(2)).unwrap().parameters.insert(
        ParameterKey::new("predicate").unwrap(),
        serde_json::json!({
            "column": "a",
            "operator": "greaterThan",
            "value": {"type": "integer", "value": "0"}
        }),
    );
    connect(&mut graph, 10, 1, "out", 2, "in");
    connect(&mut graph, 11, 2, "out", 3, "in");
    connect(&mut graph, 12, 3, "out", 4, "in");

    let result =
        GraphCompiler::with_schema_resolvers(&registry, &Resources, resolvers).compile(&graph);

    assert!(result.plan.is_some(), "{:?}", result.analysis.diagnostics);
    let source_fact = SchemaExpr::Input(key("raw"));
    let filtered = SchemaExpr::Filter {
        input: Box::new(source_fact),
        predicate: Some(ParameterKey::new("predicate").unwrap()),
    };
    assert_eq!(
        result
            .analysis
            .partial_schemas
            .get(&PortAddress::declared(node_id(2), key("out"))),
        Some(&filtered)
    );
    let projected = SchemaExpr::Project {
        input: Box::new(filtered),
        columns: ColumnSelectionExpr::Explicit(vec![SchemaColumnRef("a".into())]),
    };
    assert_eq!(
        result
            .analysis
            .partial_schemas
            .get(&PortAddress::declared(node_id(3), key("out"))),
        Some(&projected)
    );
    assert_eq!(
        result
            .analysis
            .partial_schemas
            .get(&PortAddress::declared(node_id(4), key("out"))),
        Some(&SchemaExpr::Rename {
            input: Box::new(projected),
            mapping: RenameExpr::Explicit(vec![ColumnRename {
                from: SchemaColumnRef("a".into()),
                to: SchemaColumnRef("renamed".into()),
            }]),
        })
    );
    let output = PortAddress::declared(node_id(4), key("out"));
    let expected_fields = vec![SchemaField {
        name: SchemaColumnRef("renamed".into()),
        scalar_type: RelationalScalarType::Int64,
        lineage: None,
    }];
    assert_eq!(
        result.analysis.resolved_schemas[&output].fields,
        expected_fields
    );
    assert_eq!(
        result.semantic.as_ref().unwrap().resolved_schemas[&output].fields,
        expected_fields
    );
}

#[test]
fn explicit_project_reports_missing_and_duplicate_columns() {
    let resolver_id = SchemaResolverId::new("test.project_source_schema").unwrap();
    let source = test_protocol(
        "project_schema_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            Some(SchemaExpr::Derived {
                resolver: resolver_id.clone(),
                dependencies: vec![],
            }),
        )],
        vec![],
        vec![],
    );
    let project = test_protocol(
        "invalid_schema_project",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Unknown,
                Some(SchemaExpr::Project {
                    input: Box::new(SchemaExpr::Input(key("in"))),
                    columns: ColumnSelectionExpr::Explicit(vec![
                        SchemaColumnRef("missing".into()),
                        SchemaColumnRef("a".into()),
                        SchemaColumnRef("a".into()),
                    ]),
                }),
            ),
        ],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source, project]);
    let mut resolvers = SchemaResolverSet::new();
    resolvers.insert(resolver_id, SourceSchemaResolver);
    let mut graph =
        graph_with_nodes(&[(1, "project_schema_source"), (2, "invalid_schema_project")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let result =
        GraphCompiler::with_schema_resolvers(&registry, &Resources, resolvers).compile(&graph);
    let codes = result
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<BTreeSet<_>>();

    assert!(result.plan.is_none());
    assert!(codes.contains("compiler.schema.project_field_missing"));
    assert!(codes.contains("compiler.schema.project_field_duplicate"));
}

#[test]
fn explicit_rename_reports_missing_duplicate_and_conflicting_fields() {
    let resolver_id = SchemaResolverId::new("test.rename_source_schema").unwrap();
    let source = test_protocol(
        "rename_schema_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            Some(SchemaExpr::Derived {
                resolver: resolver_id.clone(),
                dependencies: vec![],
            }),
        )],
        vec![],
        vec![],
    );
    let rename = test_protocol(
        "invalid_schema_rename",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Unknown,
                Some(SchemaExpr::Rename {
                    input: Box::new(SchemaExpr::Input(key("in"))),
                    mapping: RenameExpr::Explicit(vec![
                        ColumnRename {
                            from: SchemaColumnRef("missing".into()),
                            to: SchemaColumnRef("x".into()),
                        },
                        ColumnRename {
                            from: SchemaColumnRef("a".into()),
                            to: SchemaColumnRef("b".into()),
                        },
                        ColumnRename {
                            from: SchemaColumnRef("a".into()),
                            to: SchemaColumnRef("x".into()),
                        },
                    ]),
                }),
            ),
        ],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source, rename]);
    let mut resolvers = SchemaResolverSet::new();
    resolvers.insert(resolver_id, SourceSchemaResolver);
    let mut graph = graph_with_nodes(&[(1, "rename_schema_source"), (2, "invalid_schema_rename")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let result =
        GraphCompiler::with_schema_resolvers(&registry, &Resources, resolvers).compile(&graph);
    let codes = result
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<BTreeSet<_>>();

    assert!(result.plan.is_none());
    assert!(codes.contains("compiler.schema.rename_field_missing"));
    assert!(codes.contains("compiler.schema.rename_source_duplicate"));
    assert!(codes.contains("compiler.schema.rename_target_conflict"));
}

#[test]
fn resolver_error_analysis_has_no_plan() {
    let protocol = test_protocol(
        "unresolved_schema",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            Some(SchemaExpr::Derived {
                resolver: SchemaResolverId::new("test.missing_schema").unwrap(),
                dependencies: vec![],
            }),
        )],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![protocol]);
    let graph = graph_with_nodes(&[(1, "unresolved_schema")]);

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(result.plan.is_none());
    assert!(
        result
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code.as_str() == "compiler.schema.resolver_missing" })
    );
}
