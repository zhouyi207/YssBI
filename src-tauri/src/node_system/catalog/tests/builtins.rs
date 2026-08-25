use super::*;

#[test]
fn project_and_control_nodes_freeze_with_complete_protocol_contracts() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let expected = [
        ("yssbi.project.event.begin", StructuralNodeRole::EventBegin),
        (
            "yssbi.project.function.entry",
            StructuralNodeRole::FunctionEntry,
        ),
        (
            "yssbi.project.function.return",
            StructuralNodeRole::FunctionReturn,
        ),
        ("yssbi.project.function.call", StructuralNodeRole::Call),
        ("yssbi.control.branch", StructuralNodeRole::Branch),
        ("yssbi.control.sequence", StructuralNodeRole::Sequence),
        ("yssbi.control.loop", StructuralNodeRole::Loop),
    ];
    for (id, role) in expected {
        let node = registry.get(&NodeTypeId::new(id).unwrap()).unwrap();
        assert_eq!(node.structural_role(), Some(role));
    }

    let entry = &registry
        .get(&NodeTypeId::new("yssbi.project.function.entry").unwrap())
        .unwrap()
        .protocol();
    assert_eq!(entry.scope, NodeScope::Function);
    assert_eq!(entry.managed_role, Some(ManagedNodeRole::FunctionEntry));
    assert!(
        entry
            .interface
            .ports
            .iter()
            .any(|port| matches!(port.instances, PortInstances::Derived { .. }))
    );

    for id in ["yssbi.project.variable.get", "yssbi.project.variable.set"] {
        let protocol = &registry
            .get(&NodeTypeId::new(id).unwrap())
            .unwrap()
            .protocol();
        assert!(
            protocol
                .interface
                .ports
                .iter()
                .any(|port| matches!(&port.value_type, TypeExpr::Generic(_)))
        );
        assert!(protocol.parameters.parameters.iter().any(|parameter| {
            parameter.key.as_str() == "variable" && parameter.value_type != TypeExpr::Unknown
        }));
    }

    let branch_protocol = &registry
        .get(&NodeTypeId::new("yssbi.control.branch").unwrap())
        .unwrap()
        .protocol();
    for (key, direction) in [
        ("then_source", PortDirection::Input),
        ("else_source", PortDirection::Input),
        ("result", PortDirection::Output),
    ] {
        let port = branch_protocol
            .interface
            .ports
            .iter()
            .find(|port| port.key.as_str() == key)
            .unwrap_or_else(|| panic!("branch must declare stable {key} result members"));
        assert_eq!(port.direction, direction);
        assert_eq!(port.kind, PortKind::Data);
        assert_eq!(
            port.instances,
            PortInstances::UserCreated { min: 0, max: None }
        );
        assert_eq!(port.connections, ConnectionsPerPort::Single);
        assert_eq!(
            port.input_binding.is_some(),
            direction == PortDirection::Input
        );
        assert_eq!(
            port.production,
            (direction == PortDirection::Output).then_some(OutputProduction::FullyMaterialized)
        );
    }
    assert_eq!(
        serde_json::to_value(&branch_protocol.interface).unwrap()["member_groups"],
        serde_json::json!([{
            "templates": ["then_source", "else_source", "result"],
            "min": 0,
            "max": null
        }])
    );

    let loop_protocol = &registry
        .get(&NodeTypeId::new("yssbi.control.loop").unwrap())
        .unwrap()
        .protocol();
    for (key, direction) in [
        ("initial_source", PortDirection::Input),
        ("body_input", PortDirection::Output),
        ("next_source", PortDirection::Input),
        ("result", PortDirection::Output),
    ] {
        let port = loop_protocol
            .interface
            .ports
            .iter()
            .find(|port| port.key.as_str() == key)
            .unwrap_or_else(|| panic!("loop must declare stable {key} carried members"));
        assert_eq!(port.direction, direction);
        assert_eq!(port.kind, PortKind::Data);
        assert_eq!(
            port.instances,
            PortInstances::UserCreated { min: 0, max: None }
        );
        assert_eq!(port.connections, ConnectionsPerPort::Single);
        assert_eq!(
            port.input_binding.is_some(),
            direction == PortDirection::Input
        );
        assert_eq!(
            port.production,
            (direction == PortDirection::Output).then_some(OutputProduction::FullyMaterialized)
        );
    }
    assert_eq!(
        serde_json::to_value(&loop_protocol.interface).unwrap()["member_groups"],
        serde_json::json!([{
            "templates": ["initial_source", "body_input", "next_source", "result"],
            "min": 1,
            "max": null
        }])
    );
    assert!(
        loop_protocol
            .interface
            .ports
            .iter()
            .any(|port| port.key.as_str() == "condition")
    );
    assert!(
        loop_protocol
            .parameters
            .parameters
            .iter()
            .any(|parameter| parameter.key.as_str() == "max_iterations")
    );
    assert!(
        loop_protocol
            .parameters
            .parameters
            .iter()
            .all(|parameter| parameter.key.as_str() != "carried")
    );

    for id in ["yssbi.control.do", "yssbi.control.sleep"] {
        let protocol = &registry
            .get(&NodeTypeId::new(id).unwrap())
            .unwrap()
            .protocol();
        for (key, direction) in [
            ("effect_in", PortDirection::Input),
            ("effect_out", PortDirection::Output),
        ] {
            let port = protocol
                .interface
                .ports
                .iter()
                .find(|port| port.key.as_str() == key)
                .unwrap_or_else(|| panic!("{id} must declare stable {key}"));
            assert_eq!(port.direction, direction);
            assert_eq!(port.kind, PortKind::Effect);
            assert_eq!(port.value_type, TypeExpr::Unknown);
            assert_eq!(port.instances, PortInstances::Declared);
            assert_eq!(
                port.connections,
                if direction == PortDirection::Input {
                    ConnectionsPerPort::Single
                } else {
                    ConnectionsPerPort::Multiple {
                        max: None,
                        ordered: false,
                    }
                }
            );
            assert!(port.input_binding.is_none());
            assert!(port.consumption.is_none());
            assert!(port.production.is_none());
            assert!(port.schema.is_none());
        }
    }

    assert_eq!(
        item(
            &catalog.localize(&registry, "en-US"),
            "yssbi.control.branch",
        )
        .title
        .as_ref(),
        "Branch"
    );
}

#[test]
fn eligible_static_and_resource_bound_catalog_items_are_localized() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    for locale in ["en-US", "zh-CN"] {
        let localized = catalog.localize(&registry, locale);
        for id in ["yssbi.numeric.add.int64"] {
            let item = item(&localized, id);
            assert!(!item.title.is_empty());
            assert!(
                item.description
                    .as_ref()
                    .is_some_and(|value| !value.is_empty())
            );
            assert!(
                item.documentation
                    .as_ref()
                    .is_some_and(|value| !value.is_empty())
            );
            assert!(item.backend_search_text.contains(&item.title));
            assert!(
                !item
                    .backend_search_text
                    .iter()
                    .any(|term| term.as_ref() == id)
            );
        }
    }

    let resources = [
        CatalogResourceEntry {
            name: "Calculate Sales".into(),
            node_type_id: NodeTypeId::new("yssbi.project.function.call").unwrap(),
            resource_path: CatalogResourcePath::new("functions/calculate-sales"),
            resource_revision: crate::node_system::document::ResourceRevision::INITIAL,
            create_args: ResourceBoundCreateArgsDto::Function,
            technical_terms: vec!["call".into()],
        },
        CatalogResourceEntry {
            name: "Tax Rate".into(),
            node_type_id: NodeTypeId::new("yssbi.project.variable.get").unwrap(),
            resource_path: CatalogResourcePath::new("variables/tax-rate"),
            resource_revision: crate::node_system::document::ResourceRevision::INITIAL,
            create_args: ResourceBoundCreateArgsDto::Variable,
            technical_terms: vec!["variable".into()],
        },
    ];
    let localized = catalog.localize_with_resources(&registry, "en-US", &resources);
    assert!(localized.items.iter().any(|item| matches!(
        &item.creation,
        NodeCreationDescriptor::ResourceBound {
            create_args: ResourceBoundCreateArgsDto::Function,
            ..
        }
    )));
    assert!(localized.items.iter().any(|item| matches!(
        &item.creation,
        NodeCreationDescriptor::ResourceBound {
            create_args: ResourceBoundCreateArgsDto::Variable,
            ..
        }
    )));
}

#[test]
fn static_catalog_documentation_uses_localized_markdown_resources() {
    let builtin = build_builtin_node_system().unwrap();
    let en = builtin.catalog.localize(&builtin.registry, "en-US");
    let zh = builtin.catalog.localize(&builtin.registry, "zh-CN");
    let en_add = item(&en, "yssbi.numeric.add.int64");
    let zh_add = item(&zh, "yssbi.numeric.add.int64");

    assert!(
        en_add
            .documentation
            .as_deref()
            .is_some_and(|documentation| documentation.trim_start().starts_with("# Add (+)"))
    );
    assert!(
        zh_add
            .documentation
            .as_deref()
            .is_some_and(|documentation| documentation.trim_start().starts_with("# Add (+)"))
    );
    assert_ne!(en_add.documentation, zh_add.documentation);
}

#[test]
fn builtin_factory_freezes_function_interface_nodes() {
    let builtin = build_builtin_node_system().unwrap();
    for id in [
        "yssbi.project.function.call",
        "yssbi.project.function.entry",
        "yssbi.project.function.return",
    ] {
        assert!(
            builtin
                .registry
                .get(&NodeTypeId::new(id).unwrap())
                .is_some()
        );
    }
}
#[test]
fn builtin_factory_is_deterministic_and_has_single_owners() {
    let first = build_builtin_node_system().unwrap();
    let second = build_builtin_node_system().unwrap();
    let first_node_ids = first
        .registry
        .iter()
        .map(|(id, _)| id.as_str())
        .collect::<Vec<_>>();
    let second_node_ids = second
        .registry
        .iter()
        .map(|(id, _)| id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(first_node_ids, second_node_ids);
    assert_eq!(first.registry.fingerprint(), second.registry.fingerprint());
    assert!(first_node_ids.windows(2).all(|ids| ids[0] < ids[1]));
    assert!(first.registry.iter().all(|(id, _)| {
        first
            .registry
            .node_provider(id)
            .is_some_and(|owner| owner.as_str() == "yssbi.builtin")
    }));
}

#[test]
fn every_emitted_ordinary_native_kernel_has_a_production_implementation() {
    let nodes = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let kernels = build_builtin_kernel_registry();
    let cancellation = CompileCancellationToken::new();

    for (node_id, node) in nodes.iter() {
        let Some(implementation) = &node.implementation() else {
            continue;
        };
        assert_eq!(
            implementation.capability(),
            ImplementationKind::CompilerLowering
        );
        let lowerer = implementation
            .as_any()
            .downcast_ref::<crate::node_system::compiler::NodeImplementation>()
            .unwrap_or_else(|| panic!("implemented node '{node_id}' has no compiler lowerer"));
        let parameters =
            ValidatedNodeConfig::from_analysis(&node.protocol(), BTreeMap::new(), &BTreeMap::new())
                .expect("empty configuration is valid");
        let context = LoweringContext {
            cancellation: &cancellation,
            node_id: NodeId::new(),
            protocol: &node.protocol(),
            parameters: &parameters,
            inputs: &[],
            outputs: &[],
        };
        let native_implementation = implementation
            .implementation_identity()
            .ends_with("::KernelLowerer");
        let lowered = match lowerer.lowerer.lower(&context) {
            Ok(lowered) => lowered,
            Err(error) if !native_implementation => {
                let _ = error;
                continue;
            }
            Err(error) => panic!("native node '{node_id}' failed to lower: {error}"),
        };
        assert!(
            !native_implementation || matches!(lowered.kernel, LoweredKernel::Native(_)),
            "native implementation for '{node_id}' emitted a non-native fragment",
        );
        if node_id.as_str() == "yssbi.debug.view" {
            assert!(
                matches!(&lowered.kernel, LoweredKernel::Kernel(_)),
                "View Data must retain its current scheduler-intrinsic lowering kind",
            );
            continue;
        }
        let native = match &lowered.kernel {
            LoweredKernel::Native(handle) => Some(handle),
            LoweredKernel::Scalar(fragment) => Some(&fragment.kernel),
            LoweredKernel::Kernel(fragment) => Some(&fragment.kernel),
            LoweredKernel::Relational(_) => None,
        };
        if let Some(handle) = native {
            assert!(
                kernels.get(handle).is_some(),
                "implemented node '{node_id}' emits missing native kernel '{}'",
                handle.as_str(),
            );
        }
    }
}
