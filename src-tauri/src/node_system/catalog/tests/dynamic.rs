use super::*;

#[test]
fn builtin_function_resolver_projects_function_document_members() {
    struct FunctionResources {
        path: GraphResourcePath,
        document: FunctionDocument,
        graph: GraphDocument,
    }
    impl ResourceSnapshot for FunctionResources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            BTreeMap::from([(
                ResourceKey::new(self.path.as_str()),
                ResourceVersion::new("function-v1"),
            )])
        }

        fn function_name(&self, path: &GraphResourcePath) -> Option<&str> {
            self.function_document(path).map(|_| "Test function")
        }

        fn function_document(&self, path: &GraphResourcePath) -> Option<&FunctionDocument> {
            (path == &self.path).then_some(&self.document)
        }

        fn function_graph_document(&self, path: &GraphResourcePath) -> Option<&GraphDocument> {
            (path == &self.path).then_some(&self.graph)
        }
    }

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let path = GraphResourcePath::new("functions/calculate-sales.yssbi-function").unwrap();
    let resources = FunctionResources {
        path: path.clone(),
        document: FunctionDocument::new(FunctionSignature {
            parameters: vec![FunctionParameter {
                id: FunctionParameterId::new("amount"),
                name: "Amount".into(),
                type_name: "Float64".into(),
            }],
            return_type: Some("Float64".into()),
        }),
        graph: GraphDocument::default(),
    };
    let node_id = NodeId::from_uuid(Uuid::from_u128(42));
    let mut document = GraphDocument::default();
    document.nodes.insert(
        node_id,
        DocumentNode {
            id: node_id,
            node_type: NodeTypeId::new("yssbi.project.function.call").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::from([(
                crate::node_system::protocol::ParameterKey::new("target").unwrap(),
                serde_json::Value::String(path.as_str().to_owned()),
            )]),
            user_label: None,
        },
    );

    let result = GraphCompiler::with_interface_resolvers(
        &registry,
        &resources,
        build_builtin_interface_resolvers(),
    )
    .compile(&document);
    let projection = &result.interface_projection.nodes[&node_id];
    assert_eq!(projection.available_members.len(), 2);
    assert!(projection.available_members.iter().any(|member| {
        match &member.member().locator {
            crate::graph_document::DynamicMemberLocator::FunctionParameter {
                function,
                parameter,
            } => {
                function == &path
                    && parameter == &FunctionParameterId::new("amount")
                    && member.member().value_type
                        == TypeExpr::Concrete(
                            crate::node_system::protocol::TypeId::new("core.float64").unwrap(),
                        )
            }
            _ => false,
        }
    }));
}

#[test]
fn event_begin_compiles_as_a_structural_entry() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let node_id = NodeId::from_uuid(Uuid::from_u128(43));
    let mut document = GraphDocument::default();
    document.nodes.insert(
        node_id,
        DocumentNode {
            id: node_id,
            node_type: NodeTypeId::new("yssbi.project.event.begin").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        },
    );

    let result = GraphCompiler::new(&registry, &EmptyResources).compile(&document);
    assert!(result.analysis.diagnostics.is_empty());
    assert!(result.plan.is_some());
}
#[test]
fn editor_locale_changes_only_display_not_identity_or_address() {
    let en = editor_projection("en-US");
    let zh = editor_projection("zh-CN");
    let en_node = &en.nodes[0];
    let zh_node = &zh.nodes[0];

    assert_ne!(en_node.display.title, zh_node.display.title);
    assert_eq!(en.graph_path, zh.graph_path);
    assert_eq!(en.source_revision, zh.source_revision);
    assert_eq!(en_node.node_id, zh_node.node_id);
    assert_eq!(en_node.node_type_id, zh_node.node_type_id);
    assert_eq!(
        en_node
            .ports
            .iter()
            .map(|port| (&port.address, &port.template_key))
            .collect::<Vec<_>>(),
        zh_node
            .ports
            .iter()
            .map(|port| (&port.address, &port.template_key))
            .collect::<Vec<_>>()
    );
}

#[test]
fn diagnostic_projection_changes_only_localized_message() {
    let (mut document, registry, catalog) = editor_fixture();
    document.nodes.values_mut().next().unwrap().node_type =
        NodeTypeId::new("yssbi.test.unknown").unwrap();
    let analysis = GraphCompiler::new(registry.as_ref(), &EmptyResources)
        .compile(&document)
        .analysis;
    let snapshot_before = serde_json::to_vec(&analysis).unwrap();

    let en = EditorGraphProjectionDto::from_sources(
        "functions/main",
        &analysis,
        &document,
        &registry,
        &catalog.localization("en-US"),
    )
    .unwrap();
    let zh = EditorGraphProjectionDto::from_sources(
        "functions/main",
        &analysis,
        &document,
        &registry,
        &catalog.localization("zh-CN"),
    )
    .unwrap();

    assert_eq!(snapshot_before, serde_json::to_vec(&analysis).unwrap());
    assert_eq!(analysis.diagnostics.len(), 1);
    assert_eq!(en.diagnostics.len(), 1);
    assert_eq!(zh.diagnostics.len(), 1);

    let en_diagnostic = &en.diagnostics[0];
    let zh_diagnostic = &zh.diagnostics[0];
    assert_eq!(en_diagnostic.code.as_ref(), "compiler.node.unknown");
    assert_eq!(en_diagnostic.code, zh_diagnostic.code);
    assert_eq!(en_diagnostic.severity, zh_diagnostic.severity);
    assert_eq!(en_diagnostic.blocking, zh_diagnostic.blocking);
    assert_eq!(en_diagnostic.location, zh_diagnostic.location);
    assert_eq!(en_diagnostic.related, zh_diagnostic.related);
    assert_ne!(en_diagnostic.message, zh_diagnostic.message);
    assert_eq!(
        en_diagnostic.message.as_ref(),
        "Node type yssbi.test.unknown is unknown."
    );
    assert_eq!(
        zh_diagnostic.message.as_ref(),
        "节点类型 yssbi.test.unknown 未知。"
    );

    let snapshot_json = std::str::from_utf8(&snapshot_before).unwrap();
    assert!(!snapshot_json.contains(en_diagnostic.message.as_ref()));
    assert!(!snapshot_json.contains(zh_diagnostic.message.as_ref()));
    assert!(
        analysis
            .diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.arguments.contains_key("detail"))
    );
}

#[test]
fn editor_projection_preserves_blocking_diagnostics() {
    let (document, registry, catalog) = editor_fixture();
    let mut analysis = GraphCompiler::new(registry.as_ref(), &EmptyResources)
        .compile(&document)
        .analysis;
    let node_id = *document.nodes.keys().next().unwrap();
    analysis.diagnostics = vec![NodeDiagnostic {
        code: DiagnosticCode::new("editor.test.blocking"),
        message_key: I18nKey::new("diagnostics.editor.test_blocking").unwrap(),
        arguments: BTreeMap::new(),
        severity: DiagnosticSeverity::Error,
        primary: DiagnosticLocation::Node(node_id),
        related: Box::new([]),
    }]
    .into_boxed_slice();

    let projection = EditorGraphProjectionDto::from_sources(
        "functions/main",
        &analysis,
        &document,
        &registry,
        &catalog.localization("en-US"),
    )
    .unwrap();

    assert!(projection.has_blocking_diagnostics);
    assert!(projection.diagnostics[0].blocking);
    assert_eq!(projection.nodes[0].diagnostics, projection.diagnostics);
}

#[test]
fn fixed_port_projection_has_no_instance_uuid() {
    let projection = editor_projection("en-US");
    let address = serde_json::to_value(&projection.nodes[0].ports[0].address).unwrap();
    let address = address.as_object().unwrap();

    assert_eq!(address.get("kind").unwrap(), "declared");
    assert!(address.contains_key("nodeId"));
    assert!(address.contains_key("portKey"));
    assert!(!address.contains_key("instanceId"));
    assert!(!address.contains_key("portId"));
}
