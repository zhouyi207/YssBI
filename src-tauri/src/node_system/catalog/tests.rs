use super::localization::{Aliases, Text};
use super::*;
use crate::node_system::analysis::{
    DiagnosticArguments, DiagnosticCode, DiagnosticLocation, DiagnosticSeverity,
    EditorGraphProjectionDto, LocalizationBundle, NodeDiagnostic,
};
use crate::node_system::compiler::{
    GraphCompiler, ResourceSnapshot, build_builtin_interface_resolvers,
};
use crate::node_system::document::{
    DocumentNode, FunctionDocument, FunctionParameter, FunctionParameterId, FunctionSignature,
    GraphDocument, GraphResourcePath, NodeId, NodePosition,
};
use crate::node_system::plan::KernelHandle;
use crate::node_system::protocol::{
    I18nKey, ManagedNodeRole, NodeScope, NodeTypeId, PortInstances, TypeExpr,
};
use crate::node_system::registry::{I18nManifest, StructuralNodeRole};
use crate::node_system::runtime::build_builtin_kernel_registry;
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

fn item<'a>(catalog: &'a LocalizedCatalogDto, id: &str) -> &'a LocalizedCatalogItemDto {
    catalog
        .items
        .iter()
        .find(|item| item.node_type_id.as_ref() == id)
        .unwrap()
}

struct EmptyResources;

impl ResourceSnapshot for EmptyResources {
    fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
        BTreeMap::new()
    }
}

fn editor_fixture() -> (
    GraphDocument,
    crate::node_system::registry::NodeRegistry,
    BuiltinCatalog,
) {
    let registry = build_builtin_registry();
    let (_, catalog) = build_builtin_provider();
    let node_id = NodeId::from_uuid(Uuid::from_u128(1));
    let node_type = NodeTypeId::new("yssbi.constant.bool").unwrap();
    let mut document = GraphDocument::default();
    document.nodes.insert(
        node_id,
        DocumentNode {
            id: node_id,
            node_type,
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        },
    );
    (document, registry, catalog)
}

fn editor_projection(locale: &str) -> EditorGraphProjectionDto {
    let (document, registry, catalog) = editor_fixture();
    let analysis = GraphCompiler::new(&registry, &EmptyResources)
        .compile(&document)
        .analysis;
    EditorGraphProjectionDto::from_sources(
        "functions/main",
        &analysis,
        &document,
        &registry,
        &catalog.localization(locale),
    )
    .unwrap()
}

#[test]
fn english_and_chinese_project_the_same_stable_node_ids() {
    let registry = build_builtin_registry();
    let (_, catalog) = build_builtin_provider();
    let en = catalog.localize(&registry, "en-US");
    let zh = catalog.localize(&registry, "zh-CN");
    assert_eq!(
        en.items
            .iter()
            .map(|item| &item.node_type_id)
            .collect::<Vec<_>>(),
        zh.items
            .iter()
            .map(|item| &item.node_type_id)
            .collect::<Vec<_>>()
    );
    assert_ne!(
        item(&en, "yssbi.numeric.add.int64").title,
        item(&zh, "yssbi.numeric.add.int64").title
    );
}

#[test]
fn changing_locale_does_not_change_registry_fingerprint() {
    let registry = build_builtin_registry();
    let fingerprint = registry.fingerprint().clone();
    let (_, catalog) = build_builtin_provider();
    let _ = catalog.localize(&registry, "zh-CN");
    let _ = catalog.localize(&registry, "en-US");
    assert_eq!(&fingerprint, registry.fingerprint());
}

#[test]
fn locale_fallback_uses_language_then_english_then_stable_key() {
    let registry = build_builtin_registry();
    let (_, catalog) = build_builtin_provider();
    assert_eq!(
        item(&catalog.localize(&registry, "zh-TW"), "yssbi.logic.not")
            .title
            .as_ref(),
        "非"
    );
    assert_eq!(
        item(&catalog.localize(&registry, "fr-FR"), "yssbi.logic.not")
            .title
            .as_ref(),
        "Not"
    );
}

#[test]
fn aliases_stable_ids_and_english_technical_terms_are_searchable() {
    let registry = build_builtin_registry();
    let (_, catalog) = build_builtin_provider();
    let en = catalog.localize(&registry, "en-US");
    let en_add = item(&en, "yssbi.numeric.add.int64");
    assert!(en_add.search_text.contains("yssbi numeric add int64"));
    assert!(en_add.search_text.contains("plus"));
    assert_eq!(
        en_add.aliases,
        vec![Box::<str>::from("plus"), "sum".into(), "+".into()]
    );

    let zh = catalog.localize(&registry, "zh-CN");
    let zh_add = item(&zh, "yssbi.numeric.add.int64");
    assert_eq!(zh_add.title.as_ref(), "加法");
    assert!(zh_add.search_text.contains("plus"));
}

#[test]
fn catalog_items_serialize_narrow_creation_descriptors_without_ports_or_parameter_maps() {
    let registry = build_builtin_registry();
    let (_, catalog) = build_builtin_provider();
    let value = serde_json::to_value(item(
        &catalog.localize(&registry, "en-US"),
        "yssbi.numeric.add.int64",
    ))
    .unwrap();

    assert_eq!(value["creation"]["kind"], "static");
    assert_eq!(value["creation"]["nodeTypeId"], "yssbi.numeric.add.int64");
    assert!(value.get("ports").is_none());
    assert!(value.get("parameters").is_none());
    assert!(value["creation"].get("ports").is_none());
    assert!(value["creation"].get("parameters").is_none());
}

#[test]
fn resource_entries_merge_without_translating_user_names() {
    let registry = build_builtin_registry();
    let (_, catalog) = build_builtin_provider();
    let resource = CatalogResourceEntry {
        name: "Calculate Sales".into(),
        node_type_id: NodeTypeId::new("yssbi.project.function.call").unwrap(),
        resource: GraphResourcePath("functions/calculate-sales".into()),
        create_args: ResourceBoundCreateArgsDto::Function,
        technical_terms: vec!["call".into(), "function".into()],
        pinyin: Some("calculate sales".into()),
    };

    let en = catalog.localize_with_resources(&registry, "en-US", &[resource.clone()]);
    let zh = catalog.localize_with_resources(&registry, "zh-CN", &[resource]);
    let en_resource = en
        .items
        .iter()
        .find(|item| item.title.as_ref() == "Calculate Sales")
        .unwrap();
    let zh_resource = zh
        .items
        .iter()
        .find(|item| item.title.as_ref() == "Calculate Sales")
        .unwrap();

    assert_eq!(en_resource.title, zh_resource.title);
    assert_eq!(en_resource.pinyin, None);
    assert_eq!(zh_resource.pinyin.as_deref(), Some("calculate sales"));
    assert!(zh_resource.search_text.contains("calculate sales"));
    assert!(zh_resource.search_text.contains("function"));
    assert!(matches!(
        &zh_resource.creation,
        NodeCreationDescriptor::ResourceBound {
            resource: GraphResourcePath(path),
            create_args: ResourceBoundCreateArgsDto::Function,
            ..
        } if path.as_ref() == "functions/calculate-sales"
    ));
    let serialized = serde_json::to_value(zh_resource).unwrap();
    assert_eq!(serialized["creation"]["createArgs"]["kind"], "function");
    assert!(serialized["creation"].get("parameters").is_none());
    assert!(serialized["creation"].get("ports").is_none());
}

#[test]
fn search_normalization_folds_case_width_diacritics_and_punctuation() {
    assert_eq!(
        normalize_search_text("  ＡＤＤ—Café，SUM_value  ").as_ref(),
        "add cafe sum_value"
    );
    assert_eq!(normalize_search_text("PLUS").as_ref(), "plus");
}

#[test]
fn i18n_validation_uses_default_locale_not_the_locale_union() {
    let catalog = BuiltinCatalog::new(&[
        ("en-US", "required.title", Text("Title")),
        ("en-US", "required.aliases", Aliases(&["one"])),
        ("en-US", "unused.key", Text("Unused")),
        ("zh-CN", "required.aliases", Aliases(&["一"])),
        ("zh-CN", "masked.title", Text("不能掩盖默认缺失")),
    ]);
    let required = I18nManifest {
        keys: ["required.title", "required.aliases", "masked.title"]
            .into_iter()
            .map(|key| I18nKey::new(key).unwrap())
            .collect(),
    };
    let aliases = BTreeSet::from([I18nKey::new("required.aliases").unwrap()]);

    let inventory = catalog.audit(&required, &aliases);
    assert_eq!(
        inventory.default_locale_missing,
        vec![Box::<str>::from("masked.title")]
    );
    assert_eq!(
        inventory.missing_by_locale["zh-CN"],
        vec![Box::<str>::from("required.title")]
    );
    assert_eq!(
        inventory.unused_by_locale["en-US"],
        vec![Box::<str>::from("unused.key")]
    );
    assert!(matches!(
        catalog.validate(&required, &aliases),
        Err(I18nBundleValidationError::MissingDefaultLocale { .. })
    ));
}

#[test]
fn i18n_validation_requires_alias_messages_to_be_arrays() {
    let catalog = BuiltinCatalog::new(&[("en-US", "required.aliases", Text("not an alias array"))]);
    let key = I18nKey::new("required.aliases").unwrap();
    let required = I18nManifest {
        keys: BTreeSet::from([key.clone()]),
    };

    assert!(matches!(
        catalog.validate(&required, &BTreeSet::from([key])),
        Err(I18nBundleValidationError::AliasesNotArray { .. })
    ));
}

#[test]
fn compiler_diagnostics_render_in_english_and_chinese() {
    let (_, catalog) = build_builtin_provider();
    let key = I18nKey::new("diagnostics.compiler.type.incompatible").unwrap();
    let arguments = DiagnosticArguments::from([(
        Box::<str>::from("detail"),
        Box::<str>::from("left and right differ"),
    )]);

    assert_eq!(
        catalog
            .localization("en-US")
            .text(&key, &arguments)
            .as_ref(),
        "Compiler diagnostic: left and right differ"
    );
    assert_eq!(
        catalog
            .localization("zh-CN")
            .text(&key, &arguments)
            .as_ref(),
        "编译诊断：left and right differ"
    );
}

#[test]
fn trusted_provider_freezes_with_complete_inventory() {
    let registry = build_builtin_registry();
    assert!(registry.len() >= 20);
    assert!(
        registry
            .get(&NodeTypeId::new("yssbi.control.branch").unwrap())
            .unwrap()
            .structural_role
            .is_some()
    );
    let (_, catalog) = build_builtin_provider();
    let inventory = catalog.audit(&registry.catalog_manifest().i18n, &BTreeSet::new());
    assert!(inventory.default_locale_missing.is_empty());
    assert!(inventory.unused_by_locale["en-US"].is_empty());
}

#[test]
fn project_and_control_nodes_freeze_with_complete_protocol_contracts() {
    let registry = build_builtin_registry();
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
        assert_eq!(node.structural_role, Some(role));
    }

    let entry = &registry
        .get(&NodeTypeId::new("yssbi.project.function.entry").unwrap())
        .unwrap()
        .protocol;
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
            .protocol;
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

    let loop_protocol = &registry
        .get(&NodeTypeId::new("yssbi.control.loop").unwrap())
        .unwrap()
        .protocol;
    assert!(
        loop_protocol
            .interface
            .ports
            .iter()
            .any(|port| port.key.as_str() == "condition")
    );
    assert!(
        loop_protocol
            .interface
            .ports
            .iter()
            .any(|port| port.key.as_str() == "carried"
                && matches!(port.instances, PortInstances::UserCreated { .. }))
    );
    assert!(
        loop_protocol
            .parameters
            .parameters
            .iter()
            .any(|parameter| parameter.key.as_str() == "max_iterations")
    );
}

#[test]
fn project_nodes_are_fully_localized_searchable_and_resource_bound() {
    let registry = build_builtin_registry();
    let (_, catalog) = build_builtin_provider();
    for locale in ["en-US", "zh-CN"] {
        let localized = catalog.localize(&registry, locale);
        for id in [
            "yssbi.project.event.begin",
            "yssbi.project.function.entry",
            "yssbi.project.function.return",
            "yssbi.project.function.call",
            "yssbi.project.variable.get",
            "yssbi.project.variable.set",
            "yssbi.control.loop",
        ] {
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
            assert!(
                item.search_text
                    .contains(normalize_search_text(id).as_ref())
            );
        }
    }

    let resources = [
        CatalogResourceEntry {
            name: "Calculate Sales".into(),
            node_type_id: NodeTypeId::new("yssbi.project.function.call").unwrap(),
            resource: GraphResourcePath("functions/calculate-sales".into()),
            create_args: ResourceBoundCreateArgsDto::Function,
            technical_terms: vec!["call".into()],
            pinyin: None,
        },
        CatalogResourceEntry {
            name: "Tax Rate".into(),
            node_type_id: NodeTypeId::new("yssbi.project.variable.get").unwrap(),
            resource: GraphResourcePath("variables/tax-rate".into()),
            create_args: ResourceBoundCreateArgsDto::Variable,
            technical_terms: vec!["variable".into()],
            pinyin: None,
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
fn builtin_function_resolver_projects_function_document_members() {
    struct FunctionResources {
        path: GraphResourcePath,
        document: FunctionDocument,
    }
    impl ResourceSnapshot for FunctionResources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            BTreeMap::new()
        }

        fn function_document(&self, path: &GraphResourcePath) -> Option<&FunctionDocument> {
            (path == &self.path).then_some(&self.document)
        }
    }

    let registry = build_builtin_registry();
    let path = GraphResourcePath("functions/calculate-sales".into());
    let resources = FunctionResources {
        path: path.clone(),
        document: FunctionDocument::new(FunctionSignature {
            parameters: vec![FunctionParameter {
                id: FunctionParameterId("amount".into()),
                name: "Amount".into(),
                type_name: "float64".into(),
            }],
            return_type: Some("float64".into()),
        }),
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
                serde_json::Value::String(path.0.to_string()),
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
            crate::node_system::document::DynamicMemberLocator::FunctionParameter {
                function,
                parameter,
            } => function == &path && parameter == &FunctionParameterId("amount".into()),
            _ => false,
        }
    }));
}

#[test]
fn event_begin_compiles_as_a_structural_entry() {
    let registry = build_builtin_registry();
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
fn builtin_provider_declares_named_function_interface_resolvers() {
    let (provider, _) = build_builtin_provider();
    let ids = provider
        .interface_resolvers
        .iter()
        .map(|id| id.as_str())
        .collect::<BTreeSet<_>>();
    assert!(ids.contains("yssbi.project.function.call.arguments"));
    assert!(ids.contains("yssbi.project.function.call.results"));
    assert!(ids.contains("yssbi.project.function.entry.parameters"));
    assert!(ids.contains("yssbi.project.function.return.results"));
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
fn editor_projection_preserves_blocking_diagnostics() {
    let (document, registry, catalog) = editor_fixture();
    let mut analysis = GraphCompiler::new(&registry, &EmptyResources)
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

#[test]
fn legacy_production_catalog_has_complete_stable_manifest_coverage() {
    let registry = build_builtin_registry();
    let mut manifest = super::core_nodes::legacy_coverage()
        .iter()
        .map(|entry| (entry.legacy_node_type, entry.stable_ids.to_vec()))
        .collect::<Vec<_>>();
    manifest.extend(
        super::dataframe::LEGACY_NODE_IDS
            .iter()
            .map(|(legacy, stable)| (*legacy, vec![*stable])),
    );
    manifest.extend(
        super::statistics::LEGACY_NODE_IDS
            .iter()
            .map(|(legacy, stable)| (*legacy, vec![*stable])),
    );
    manifest.extend(
        super::distribution::legacy_manifest().map(|(legacy, stable)| (legacy, vec![stable])),
    );
    manifest.extend(super::plot::legacy_manifest().map(|(legacy, stable)| (legacy, vec![stable])));
    manifest.extend([
        ("Event:Event Begin", vec!["yssbi.project.event.begin"]),
        (
            "Functions:Function Entry",
            vec!["yssbi.project.function.entry"],
        ),
        (
            "Functions:Function Return",
            vec!["yssbi.project.function.return"],
        ),
        (
            "Functions:Call Function",
            vec!["yssbi.project.function.call"],
        ),
        ("Variables:Get Variable", vec!["yssbi.project.variable.get"]),
        ("Variables:Set Variable", vec!["yssbi.project.variable.set"]),
    ]);

    assert_eq!(
        manifest.len(),
        147,
        "legacy NodeDefinition manifest changed"
    );
    assert_eq!(
        manifest
            .iter()
            .map(|(legacy, _)| *legacy)
            .collect::<BTreeSet<_>>()
            .len(),
        manifest.len(),
        "legacy functionality must occur exactly once in the migration manifest",
    );
    for (legacy, stable_ids) in manifest {
        assert!(
            !stable_ids.is_empty(),
            "legacy node '{legacy}' has no stable ID"
        );
        assert_eq!(
            stable_ids.iter().copied().collect::<BTreeSet<_>>().len(),
            stable_ids.len(),
            "legacy node '{legacy}' repeats a stable family member",
        );
        for stable_id in stable_ids {
            assert!(
                registry.get(&NodeTypeId::new(stable_id).unwrap()).is_some(),
                "legacy node '{legacy}' is missing stable node '{stable_id}'",
            );
        }
    }
}

#[test]
fn builtin_provider_is_deterministic_and_has_single_owners() {
    let (first, _) = build_builtin_provider();
    let (second, _) = build_builtin_provider();
    let first_node_ids = first
        .nodes
        .iter()
        .map(|node| node.protocol.type_id.as_str())
        .collect::<Vec<_>>();
    let second_node_ids = second
        .nodes
        .iter()
        .map(|node| node.protocol.type_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(first_node_ids, second_node_ids);
    assert!(first_node_ids.windows(2).all(|ids| ids[0] < ids[1]));
    assert!(
        first
            .types
            .windows(2)
            .all(|items| items[0].id < items[1].id)
    );
    assert!(
        first
            .type_constructors
            .windows(2)
            .all(|items| items[0].id < items[1].id)
    );
    assert!(
        first
            .categories
            .windows(2)
            .all(|items| items[0].id < items[1].id)
    );
    assert!(
        first
            .interface_resolvers
            .windows(2)
            .all(|ids| ids[0] < ids[1])
    );
    assert!(first.schema_resolvers.windows(2).all(|ids| ids[0] < ids[1]));
}

#[test]
fn every_leaf_has_a_protocol_lowerer_and_production_kernel() {
    let nodes = build_builtin_registry();
    let kernels = build_builtin_kernel_registry();

    for (node_id, node) in nodes.iter() {
        let Some(implementation) = &node.implementation else {
            assert!(node.structural_role.is_some(), "{node_id}");
            continue;
        };
        assert!(
            implementation
                .as_any()
                .downcast_ref::<crate::node_system::compiler::NodeImplementation>()
                .is_some(),
            "leaf node '{node_id}' has no protocol lowerer",
        );
        let handle = match node_id.as_str() {
            "yssbi.logic.equal" => "yssbi.compare.equal",
            "yssbi.logic.not_equal" => "yssbi.compare.not_equal",
            "yssbi.logic.less" => "yssbi.compare.less",
            "yssbi.logic.less_equal" => "yssbi.compare.less_equal",
            "yssbi.logic.greater" => "yssbi.compare.greater",
            "yssbi.logic.greater_equal" => "yssbi.compare.greater_equal",
            id => id,
        };
        let handle = KernelHandle::new(handle).unwrap();
        assert!(
            kernels.get(&handle).is_some(),
            "leaf node '{node_id}' lowers to missing kernel '{}'",
            handle.as_str(),
        );
    }
}
