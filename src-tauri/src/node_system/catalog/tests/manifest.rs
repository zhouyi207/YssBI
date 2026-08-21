use super::*;

#[test]
fn every_resource_parameter_has_an_explicit_instance_display_classification() {
    let builtin = build_builtin_node_system().unwrap();
    let expected_identity = BTreeMap::from([
        (
            ("yssbi.project.function.call", "target"),
            ResourceDisplayKind::Function,
        ),
        (
            ("yssbi.project.variable.get", "variable"),
            ResourceDisplayKind::Variable,
        ),
        (
            ("yssbi.project.variable.set", "variable"),
            ResourceDisplayKind::Variable,
        ),
        (
            ("yssbi.dataframe.source.get", "dataframe"),
            ResourceDisplayKind::Database,
        ),
    ]);
    let expected_static = BTreeMap::from([
        (
            ("yssbi.project.function.entry", "function"),
            "managed entry nodes display their graph-boundary role",
        ),
        (
            ("yssbi.project.function.return", "function"),
            "managed return nodes display their graph-boundary role",
        ),
    ]);
    let mut observed = BTreeSet::new();

    for (node_type, registered) in builtin.registry.iter() {
        let protocol = registered.protocol();
        for parameter in
            protocol.parameters.parameters.iter().filter(|parameter| {
                matches!(parameter.editor, ParameterEditorSpec::Resource { .. })
            })
        {
            let key = (node_type.as_ref(), parameter.key.as_ref());
            observed.insert(key);
            let ParameterEditorSpec::Resource {
                kind: parameter_kind,
            } = parameter.editor
            else {
                unreachable!()
            };
            let expected_kind = expected_identity
                .get(&key)
                .copied()
                .or_else(|| match key.1 {
                    "function" => Some(ResourceDisplayKind::Function),
                    _ => None,
                })
                .expect("every built-in resource parameter has an expected kind");
            assert_eq!(
                parameter_kind, expected_kind,
                "resource parameter {key:?} has the wrong kind"
            );
            if let Some(kind) = expected_identity.get(&key) {
                assert_eq!(
                    &protocol.instance_display,
                    &NodeInstanceDisplaySpec::ResourceParameter {
                        parameter: parameter.key.clone(),
                        kind: *kind,
                    },
                    "resource identity parameter {key:?} has the wrong display classification",
                );
            } else if let Some(reason) = expected_static.get(&key) {
                assert!(
                    !reason.is_empty(),
                    "static audit entry {key:?} needs a reason"
                );
                assert_eq!(
                    protocol.instance_display,
                    NodeInstanceDisplaySpec::Static,
                    "static resource parameter {key:?} must retain its protocol title: {reason}",
                );
            } else {
                panic!("resource parameter {key:?} has no explicit display classification");
            }
        }
    }

    let expected = expected_identity
        .keys()
        .chain(expected_static.keys())
        .copied()
        .collect::<BTreeSet<_>>();
    assert_eq!(observed, expected, "audit entries must match built-ins");
}

#[test]
fn english_and_chinese_project_the_same_stable_node_ids() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
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
fn localized_categories_preserve_registry_hierarchy_and_order() {
    let builtin = build_builtin_node_system().unwrap();
    let localized = builtin.catalog.localize(&builtin.registry, "en-US");
    let category = |id: &str| {
        localized
            .categories
            .iter()
            .find(|category| category.category_id.as_ref() == id)
            .unwrap()
    };

    let statistics = category("statistics");
    assert_eq!(statistics.parent_category_id, None);
    assert_eq!(statistics.order, 70);

    let regression = category("statistics.regression");
    assert_eq!(regression.parent_category_id.as_deref(), Some("statistics"));
    assert_eq!(regression.order, 71);
}

#[test]
fn changing_locale_does_not_change_registry_fingerprint() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let fingerprint = registry.fingerprint().clone();
    let _ = catalog.localize(&registry, "zh-CN");
    let _ = catalog.localize(&registry, "en-US");
    assert_eq!(&fingerprint, registry.fingerprint());
}

#[test]
fn locale_fallback_uses_language_then_english_then_stable_key() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
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
fn catalog_projects_distinct_backend_search_and_resource_name_arrays() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let en = catalog.localize(&registry, "en-US");
    let en_add = item(&en, "yssbi.numeric.add.int64");
    assert_eq!(
        en_add.backend_search_text,
        vec![
            Box::<str>::from("Add"),
            "plus".into(),
            "sum".into(),
            "+".into(),
        ]
    );
    assert!(en_add.resource_names.is_empty());
    assert_eq!(
        en_add.aliases,
        vec![Box::<str>::from("plus"), "sum".into(), "+".into()]
    );

    let zh = catalog.localize(&registry, "zh-CN");
    let zh_add = item(&zh, "yssbi.numeric.add.int64");
    assert_eq!(zh_add.title.as_ref(), "加法");
    assert_eq!(
        zh_add.backend_search_text,
        vec![
            Box::<str>::from("加法"),
            "相加".into(),
            "求和".into(),
            "+".into(),
        ]
    );
    assert!(zh_add.resource_names.is_empty());
}

#[test]
fn catalog_items_keep_creation_descriptors_narrow_with_focused_documentation() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let value = serde_json::to_value(item(
        &catalog.localize(&registry, "en-US"),
        "yssbi.numeric.add.int64",
    ))
    .unwrap();

    assert_eq!(value["creation"]["kind"], "static");
    assert_eq!(value["creation"]["nodeTypeId"], "yssbi.numeric.add.int64");
    assert!(value["ports"].is_array());
    assert!(value["parameters"].is_array());
    assert!(value.get("resourcePath").is_none());
    assert!(value.get("resourceRevision").is_none());
    assert!(value["creation"].get("ports").is_none());
    assert!(value["creation"].get("parameters").is_none());
}

#[test]
fn resource_catalog_serializes_opaque_paths_and_revisions() {
    use crate::node_system::document::ResourceRevision;

    for (create_args, expected_kind) in [
        (ResourceBoundCreateArgsDto::Function, "function"),
        (ResourceBoundCreateArgsDto::Variable, "variable"),
        (ResourceBoundCreateArgsDto::Database, "database"),
    ] {
        let descriptor = NodeCreationDescriptor::ResourceBound {
            node_type_id: NodeTypeId::new("yssbi.dataframe.source.get").unwrap(),
            resource_path: CatalogResourcePath::new("opaque/backend-issued/path"),
            resource_revision: ResourceRevision::new(17),
            create_args,
        };
        let value = serde_json::to_value(&descriptor).unwrap();

        assert_eq!(
            value,
            serde_json::json!({
                "kind": "resourceBound",
                "nodeTypeId": "yssbi.dataframe.source.get",
                "resourcePath": "opaque/backend-issued/path",
                "resourceRevision": 17,
                "createArgs": { "kind": expected_kind },
            })
        );
        assert_eq!(
            serde_json::from_value::<NodeCreationDescriptor>(value).unwrap(),
            descriptor
        );
    }

    let parameterized = NodeCreationDescriptor::ParameterizedStatic {
        node_type_id: NodeTypeId::new("yssbi.dataframe.project").unwrap(),
        required_parameters: Box::new([ParameterKey::new("columns").unwrap()]),
    };
    let parameterized_value = serde_json::json!({
        "kind": "parameterizedStatic",
        "nodeTypeId": "yssbi.dataframe.project",
        "requiredParameters": ["columns"],
    });
    assert_eq!(
        serde_json::to_value(&parameterized).unwrap(),
        parameterized_value
    );
    assert_eq!(
        serde_json::from_value::<NodeCreationDescriptor>(parameterized_value).unwrap(),
        parameterized,
    );

    for malformed in [
        serde_json::json!({
            "kind": "parameterizedStatic",
            "nodeTypeId": "yssbi.dataframe.project",
        }),
        serde_json::json!({
            "kind": "parameterizedStatic",
            "nodeTypeId": "yssbi.dataframe.project",
            "requiredParameters": ["columns"],
            "parameters": {},
        }),
        serde_json::json!({
            "kind": "parameterizedStatic",
            "nodeTypeId": "yssbi.dataframe.project",
            "requiredParameters": "columns",
        }),
        serde_json::json!({
            "kind": "resourceBound",
            "nodeTypeId": "yssbi.dataframe.source.get",
            "resourceRevision": 17,
            "createArgs": { "kind": "database" },
        }),
        serde_json::json!({
            "kind": "resourceBound",
            "nodeTypeId": "yssbi.dataframe.source.get",
            "resourcePath": "databases/sales",
            "createArgs": { "kind": "database" },
        }),
        serde_json::json!({
            "kind": "resourceBound",
            "nodeTypeId": "yssbi.dataframe.source.get",
            "resourcePath": "databases/sales",
            "resourceRevision": 17,
            "createArgs": { "kind": "resource" },
        }),
        serde_json::json!({
            "kind": "resourceBound",
            "nodeTypeId": "yssbi.dataframe.source.get",
            "resourcePath": "databases/sales",
            "resourceRevision": 17,
            "createArgs": { "kind": "database" },
            "resource": "compatibility-is-forbidden",
        }),
        serde_json::json!({
            "kind": "resourceBound",
            "resourcePath": "databases/sales",
            "resourceRevision": 17,
            "createArgs": { "kind": "database" },
        }),
        serde_json::json!({
            "kind": "resourceBound",
            "nodeTypeId": "yssbi.dataframe.source.get",
            "resourcePath": "databases/sales",
            "resourceRevision": 17,
        }),
        serde_json::json!({
            "kind": "resourceBound",
            "nodeTypeId": 42,
            "resourcePath": "databases/sales",
            "resourceRevision": 17,
            "createArgs": { "kind": "database" },
        }),
        serde_json::json!({
            "kind": "resourceBound",
            "nodeTypeId": "yssbi.dataframe.source.get",
            "resourcePath": 42,
            "resourceRevision": 17,
            "createArgs": { "kind": "database" },
        }),
        serde_json::json!({
            "kind": "resourceBound",
            "nodeTypeId": "yssbi.dataframe.source.get",
            "resourcePath": "databases/sales",
            "resourceRevision": "17",
            "createArgs": { "kind": "database" },
        }),
        serde_json::json!({
            "kind": "resourceBound",
            "nodeTypeId": "yssbi.dataframe.source.get",
            "resourcePath": "databases/sales",
            "resourceRevision": 17,
            "createArgs": "database",
        }),
        serde_json::json!({
            "kind": "resourceBound",
            "nodeTypeId": "yssbi.dataframe.source.get",
            "resourcePath": "databases/sales",
            "resourceRevision": 17,
            "createArgs": { "kind": "database", "extra": true },
        }),
        serde_json::json!({
            "kind": "static",
            "nodeTypeId": "yssbi.numeric.add.int64",
            "extra": true,
        }),
        serde_json::json!({ "kind": "static" }),
        serde_json::json!({
            "kind": "static",
            "nodeTypeId": 42,
        }),
    ] {
        assert!(
            serde_json::from_value::<NodeCreationDescriptor>(malformed.clone()).is_err(),
            "accepted malformed descriptor: {malformed}"
        );
    }
}

#[test]
fn static_catalog_excludes_managed_and_resource_required_descriptors() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;

    let localized = catalog.localize(&registry, "en-US");
    let node_type_ids = localized
        .items
        .iter()
        .map(|item| item.node_type_id.as_ref())
        .collect::<BTreeSet<_>>();

    assert!(node_type_ids.contains("yssbi.numeric.add.int64"));
    assert!(!node_type_ids.contains("yssbi.project.event.begin"));
    assert!(!node_type_ids.contains("yssbi.project.function.call"));
    assert!(!node_type_ids.contains("yssbi.project.variable.get"));
}

#[test]
fn resource_catalog_projects_localized_docs_ports_parameters_and_opaque_identity() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let resource = CatalogResourceEntry {
        name: "Calculate Sales".into(),
        node_type_id: NodeTypeId::new("yssbi.project.function.call").unwrap(),
        resource_path: CatalogResourcePath::new("functions/calculate-sales"),
        resource_revision: crate::node_system::document::ResourceRevision::INITIAL,
        create_args: ResourceBoundCreateArgsDto::Function,
        technical_terms: vec!["call".into(), "function".into()],
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
    assert_ne!(en_resource.description, zh_resource.description);
    assert_ne!(en_resource.documentation, zh_resource.documentation);
    assert_eq!(
        en_resource.resource_names,
        vec![Box::<str>::from("Calculate Sales")]
    );
    assert_eq!(zh_resource.resource_names, en_resource.resource_names);
    assert_eq!(zh_resource.icon_id.as_ref(), "builtin.project");
    assert_eq!(zh_resource.style_id.as_ref(), "builtin.default");
    assert_eq!(
        zh_resource
            .resource_path
            .as_ref()
            .map(CatalogResourcePath::as_str),
        Some("functions/calculate-sales")
    );
    assert_eq!(
        zh_resource.resource_revision,
        Some(crate::node_system::document::ResourceRevision::INITIAL)
    );
    assert!(zh_resource.ports.iter().any(|port| {
        port.key.as_ref() == "enter"
            && port.label.as_ref() == "进入"
            && port.direction.as_ref() == "input"
            && port.kind.as_ref() == "control"
    }));
    assert!(zh_resource.parameters.iter().any(|parameter| {
        parameter.key.as_ref() == "target"
            && parameter.title.as_ref() == "目标函数"
            && parameter.description.as_deref() == Some("要调用的函数资源。")
    }));
    assert!(matches!(
        &zh_resource.creation,
        NodeCreationDescriptor::ResourceBound {
            resource_path,
            create_args: ResourceBoundCreateArgsDto::Function,
            ..
        } if resource_path.as_str() == "functions/calculate-sales"
    ));
    let serialized = serde_json::to_value(zh_resource).unwrap();
    assert_eq!(serialized["creation"]["createArgs"]["kind"], "function");
    assert_eq!(serialized["resourcePath"], "functions/calculate-sales");
    assert_eq!(serialized["resourceRevision"], 0);
    assert_eq!(
        serialized["ports"].as_array().unwrap()[0]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "direction".to_string(),
            "key".to_string(),
            "kind".to_string(),
            "label".to_string(),
        ])
    );
    assert_eq!(
        serialized["parameters"].as_array().unwrap()[0]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "description".to_string(),
            "key".to_string(),
            "title".to_string(),
        ])
    );
    assert!(serialized["creation"].get("parameters").is_none());
    assert!(serialized["creation"].get("ports").is_none());
}

#[test]
fn resource_catalog_projects_raw_backend_text_and_authoritative_resource_names_separately() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let resource = CatalogResourceEntry {
        name: "Straße_Sales Cafe\u{301} 数据".into(),
        node_type_id: NodeTypeId::new("yssbi.project.function.call").unwrap(),
        resource_path: CatalogResourcePath::new("functions/opaque-sales"),
        resource_revision: crate::node_system::document::ResourceRevision::new(9),
        create_args: ResourceBoundCreateArgsDto::Function,
        technical_terms: vec!["Maße_Value\u{301}".into()],
    };

    let localized = catalog.localize_with_resources(&registry, "zh-CN", &[resource]);
    let resource = localized
        .items
        .iter()
        .find(|item| item.resource_path.is_some())
        .unwrap();

    assert_eq!(
        resource.resource_names,
        vec![Box::<str>::from("Straße_Sales Cafe\u{301} 数据")]
    );
    assert_eq!(
        resource.backend_search_text,
        vec![Box::<str>::from("调用"), "执行".into(), "函数".into()]
    );
    assert!(
        resource
            .technical_terms
            .contains(&Box::<str>::from("Maße_Value\u{301}"))
    );
    assert!(
        !resource
            .backend_search_text
            .iter()
            .any(|term| term.contains("invoke"))
    );
    assert!(
        !resource
            .resource_names
            .iter()
            .any(|name| name.contains("opaque-sales"))
    );
}

#[test]
fn resource_catalog_localization_falls_back_without_changing_identity() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let resource = CatalogResourceEntry {
        name: "Opaque Display Name".into(),
        node_type_id: NodeTypeId::new("yssbi.project.function.call").unwrap(),
        resource_path: CatalogResourcePath::new("functions/Do Not Normalize/Case"),
        resource_revision: crate::node_system::document::ResourceRevision::new(13),
        create_args: ResourceBoundCreateArgsDto::Function,
        technical_terms: Vec::new(),
    };

    let en = catalog.localize_with_resources(&registry, "en-US", &[resource.clone()]);
    let fallback = catalog.localize_with_resources(&registry, "fr-FR", &[resource]);
    let en = en
        .items
        .iter()
        .find(|item| item.resource_path.is_some())
        .unwrap();
    let fallback = fallback
        .items
        .iter()
        .find(|item| item.resource_path.is_some())
        .unwrap();

    assert_eq!(fallback.title.as_ref(), "Opaque Display Name");
    assert_eq!(fallback.description, en.description);
    assert_eq!(fallback.documentation, en.documentation);
    assert_eq!(fallback.ports, en.ports);
    assert_eq!(fallback.parameters, en.parameters);
    assert_eq!(fallback.resource_path, en.resource_path);
    assert_eq!(fallback.resource_revision, en.resource_revision);
}

#[test]
fn resource_catalog_output_is_deterministic_for_shuffled_resources() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let first = CatalogResourceEntry {
        name: "First".into(),
        node_type_id: NodeTypeId::new("yssbi.project.variable.get").unwrap(),
        resource_path: CatalogResourcePath::new("variables/a"),
        resource_revision: crate::node_system::document::ResourceRevision::new(2),
        create_args: ResourceBoundCreateArgsDto::Variable,
        technical_terms: Vec::new(),
    };
    let second = CatalogResourceEntry {
        name: "Second".into(),
        node_type_id: NodeTypeId::new("yssbi.project.function.call").unwrap(),
        resource_path: CatalogResourcePath::new("functions/z"),
        resource_revision: crate::node_system::document::ResourceRevision::new(3),
        create_args: ResourceBoundCreateArgsDto::Function,
        technical_terms: Vec::new(),
    };

    let forward =
        catalog.localize_with_resources(&registry, "en-US", &[first.clone(), second.clone()]);
    let reversed = catalog.localize_with_resources(&registry, "en-US", &[second, first]);

    assert_eq!(forward, reversed);
    let identities = forward
        .items
        .iter()
        .filter_map(|item| {
            item.resource_path
                .as_ref()
                .map(|path| (path.as_str(), item.node_type_id.as_ref()))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        identities,
        vec![
            ("functions/z", "yssbi.project.function.call"),
            ("variables/a", "yssbi.project.variable.get"),
        ]
    );
}
