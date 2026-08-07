use super::localization::{Aliases, Text};
use super::*;
use crate::node_system::analysis::{
    DiagnosticArguments, DiagnosticCode, DiagnosticLocation, DiagnosticSeverity,
    EditorGraphProjectionDto, LocalizationBundle, NodeDiagnostic,
};
use crate::node_system::compiler::{
    COMPILER_DIAGNOSTIC_DEFINITIONS, CompileCancellationToken, CompilerDiagnosticDefinitionError,
    GraphCompiler, LoweredKernel, LoweringContext, ResourceSnapshot, ValidatedNodeConfig,
    build_builtin_interface_resolvers,
};
use crate::node_system::document::{
    DocumentNode, FunctionDocument, FunctionParameter, FunctionParameterId, FunctionSignature,
    GraphDocument, GraphResourcePath, NodeId, NodePosition,
};

use crate::node_system::protocol::{
    ConnectionsPerPort, I18nKey, ManagedNodeRole, NodeScope, NodeTypeId, OutputProduction,
    ParameterKey, PortDirection, PortInstances, PortKind, TypeExpr,
};
use crate::node_system::registry::{I18nManifest, ImplementationKind, StructuralNodeRole};
use crate::node_system::runtime::build_builtin_kernel_registry;

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error as _;
use uuid::Uuid;

fn item<'a>(catalog: &'a LocalizedCatalog, id: &str) -> &'a LocalizedCatalogItemDto {
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
    std::sync::Arc<crate::node_system::registry::NodeRegistry>,
    std::sync::Arc<BuiltinCatalog>,
) {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
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
    let analysis = GraphCompiler::new(registry.as_ref(), &EmptyResources)
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
fn search_uses_only_current_locale_titles_and_aliases() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let en = catalog.localize(&registry, "en-US");
    let en_add = item(&en, "yssbi.numeric.add.int64");
    assert!(!en_add.search_text.contains("yssbi numeric add int64"));
    assert!(en_add.search_text.contains("plus"));
    assert_eq!(
        en_add.aliases,
        vec![Box::<str>::from("plus"), "sum".into(), "+".into()]
    );

    let zh = catalog.localize(&registry, "zh-CN");
    let zh_add = item(&zh, "yssbi.numeric.add.int64");
    assert_eq!(zh_add.title.as_ref(), "加法");
    assert!(zh_add.search_text.contains("求和"));
    assert!(!zh_add.search_text.contains("plus"));
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
    assert_ne!(en_resource.description, zh_resource.description);
    assert_ne!(en_resource.documentation, zh_resource.documentation);
    assert_eq!(en_resource.pinyin, None);
    assert_eq!(zh_resource.pinyin.as_deref(), Some("calculate sales"));
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
fn resource_catalog_search_uses_only_current_locale_title_and_aliases() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let resource = CatalogResourceEntry {
        name: "Calculate Sales".into(),
        node_type_id: NodeTypeId::new("yssbi.project.function.call").unwrap(),
        resource_path: CatalogResourcePath::new("functions/opaque-sales"),
        resource_revision: crate::node_system::document::ResourceRevision::new(9),
        create_args: ResourceBoundCreateArgsDto::Function,
        technical_terms: vec!["english-technical-only".into()],
        pinyin: Some("pinyin-only-token".into()),
    };

    let localized = catalog.localize_with_resources(&registry, "zh-CN", &[resource]);
    let resource = localized
        .items
        .iter()
        .find(|item| item.resource_path.is_some())
        .unwrap();

    assert!(resource.search_text.contains("calculate sales"));
    assert!(resource.search_text.contains("调用"));
    assert!(!resource.search_text.contains("invoke"));
    assert!(!resource.search_text.contains("english technical only"));
    assert!(!resource.search_text.contains("pinyin only token"));
    assert!(!resource.search_text.contains("yssbi"));
    assert!(!resource.search_text.contains("项目资源接入图执行"));
    assert!(!resource.search_text.contains("资源身份"));
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
        pinyin: None,
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
        pinyin: None,
    };
    let second = CatalogResourceEntry {
        name: "Second".into(),
        node_type_id: NodeTypeId::new("yssbi.project.function.call").unwrap(),
        resource_path: CatalogResourcePath::new("functions/z"),
        resource_revision: crate::node_system::document::ResourceRevision::new(3),
        create_args: ResourceBoundCreateArgsDto::Function,
        technical_terms: Vec::new(),
        pinyin: None,
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
    ])
    .unwrap();
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
    let catalog =
        BuiltinCatalog::new(&[("en-US", "required.aliases", Text("not an alias array"))]).unwrap();
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
fn builtin_factory_hides_raw_assembly_and_registry_shortcuts() {
    let module = include_str!("mod.rs");
    let builtin = include_str!("builtin.rs");

    assert!(!module.contains("build_builtin_provider"));
    assert!(!module.contains("build_builtin_registry"));
    assert!(!builtin.contains("pub fn build_builtin_provider"));
    assert!(!builtin.contains("pub fn build_builtin_registry"));

    let nominal_installer = builtin
        .split("fn register_builtin_nominal_validators(")
        .nth(1)
        .and_then(|tail| tail.split("#[cfg(test)]").next())
        .expect("nominal installer source section");
    assert!(nominal_installer.contains("Result<(), BuiltinAssemblyError>"));
    assert!(!nominal_installer.contains(".expect("));
    assert!(!nominal_installer.contains(".unwrap("));
    assert!(!nominal_installer.contains("panic!("));
}

#[test]
fn builtin_nominal_validator_registration_propagates_duplicate_failure() {
    let mut builder = crate::node_system::registry::NodeRegistryBuilder::new();
    let project_columns = crate::node_system::protocol::TypeId::new(
        crate::node_system::parameter_types::dataframe::PROJECT_COLUMNS_TYPE_ID,
    )
    .unwrap();
    builder
        .register_nominal_validator(
            project_columns.clone(),
            crate::node_system::protocol::TypeId::new("test.nominal.validator").unwrap(),
            1,
            |_| Ok(()),
        )
        .unwrap();

    assert!(matches!(
        register_builtin_nominal_validators_for_test(&mut builder),
        Err(BuiltinInitializationError::Assembly(
            BuiltinAssemblyError::Registration(
                crate::node_system::registry::NodeRegistrationError::InvalidRegistry(
                    crate::node_system::registry::RegistryValidationError::DuplicateNominalValidator(
                        id,
                    ),
                ),
            )
        )) if id == project_columns
    ));
}

#[test]
fn builtin_catalog_invalid_i18n_key_preserves_semantic_source() {
    let expected_source = I18nKey::new("Bad Localization Key").unwrap_err();
    let error =
        BuiltinCatalog::new(&[("en-US", "Bad Localization Key", Text("invalid"))]).unwrap_err();

    assert!(matches!(
        &error,
        crate::node_system::protocol::ProtocolError::InvalidSemanticId { value, source }
            if value.as_ref() == "Bad Localization Key" && source == &expected_source
    ));
    assert!(
        error
            .source()
            .and_then(
                |source| source.downcast_ref::<crate::node_system::protocol::InvalidSemanticId>()
            )
            .is_some_and(|source| source == &expected_source)
    );
}

#[test]
fn builtin_assembly_rejects_invalid_semantic_id_with_source() {
    let expected_source = NodeTypeId::new("Bad Display ID").unwrap_err();
    let error = build_builtin_node_system_with_test_fault(
        BuiltinAssemblyTestFault::InvalidSemanticId("Bad Display ID"),
    )
    .err()
    .expect("invalid semantic ID must fail assembly");

    assert!(matches!(
        &error,
        BuiltinInitializationError::Assembly(
            BuiltinAssemblyError::InvalidSemanticId { value, source }
        ) if value.as_ref() == "Bad Display ID" && source == &expected_source
    ));
    let assembly = std::error::Error::source(&error).unwrap();
    assert!(
        assembly
            .source()
            .and_then(
                |source| source.downcast_ref::<crate::node_system::protocol::InvalidSemanticId>()
            )
            .is_some_and(|source| source == &expected_source)
    );
}

#[test]
fn builtin_assembly_rejects_invalid_protocol_without_fallback() {
    let expected_key = crate::node_system::protocol::PortKey::new("duplicate").unwrap();
    let error = build_builtin_node_system_with_test_fault(
        BuiltinAssemblyTestFault::InvalidProtocol("yssbi.test.invalid_protocol"),
    )
    .err()
    .expect("invalid protocol must fail assembly");

    assert!(matches!(
        &error,
        BuiltinInitializationError::Assembly(
            BuiltinAssemblyError::InvalidProtocol {
                node_type,
                source: crate::node_system::protocol::ProtocolError::DuplicatePortKey(key),
            }
        ) if node_type.as_ref() == "yssbi.test.invalid_protocol" && key.as_str() == expected_key.as_str()
    ));
    let assembly = std::error::Error::source(&error).unwrap();
    assert!(
        assembly
            .source()
            .and_then(|source| source.downcast_ref::<crate::node_system::protocol::ProtocolError>())
            .is_some_and(|source| matches!(source, crate::node_system::protocol::ProtocolError::DuplicatePortKey(key) if key.as_str() == "duplicate"))
    );
}

#[test]
fn builtin_registry_revalidation_preserves_protocol_source_chain() {
    let error = build_builtin_node_system_with_test_fault(
        BuiltinAssemblyTestFault::InvalidRegistryProtocol,
    )
    .err()
    .expect("invalid frozen protocol must fail Registry validation");

    assert!(matches!(
        &error,
        BuiltinInitializationError::Assembly(BuiltinAssemblyError::Registration(
            crate::node_system::registry::NodeRegistrationError::InvalidRegistry(
                crate::node_system::registry::RegistryValidationError::InvalidNodeProtocol {
                    node,
                    source: crate::node_system::protocol::ProtocolError::DuplicatePortKey(key),
                },
            ),
        )) if node.as_str() == "yssbi.constant.bool" && key.as_str() == "value"
    ));

    let assembly = error.source().unwrap();
    let registration = assembly.source().unwrap();
    let registry = registration.source().unwrap();
    let protocol = registry.source().unwrap();
    assert!(matches!(
        protocol.downcast_ref::<crate::node_system::protocol::ProtocolError>(),
        Some(crate::node_system::protocol::ProtocolError::DuplicatePortKey(key))
            if key.as_str() == "value"
    ));
}

#[test]
fn builtin_assembly_rejects_conflicting_localization() {
    let error =
        build_builtin_node_system_with_test_fault(BuiltinAssemblyTestFault::LocalizationConflict)
            .err()
            .expect("localization conflict must fail assembly");
    assert!(matches!(
        &error,
        BuiltinInitializationError::Assembly(
            BuiltinAssemblyError::LocalizationConflict { locale, key }
        ) if locale.as_ref() == "en-US" && key.as_ref() == "nodes.test.title"
    ));
    let assembly = std::error::Error::source(&error).unwrap();
    assert!(assembly.source().is_none());
}

#[test]
fn builtin_assembly_rejects_duplicate_registration() {
    let error =
        build_builtin_node_system_with_test_fault(BuiltinAssemblyTestFault::DuplicateRegistration)
            .err()
            .expect("duplicate registration must fail assembly");
    assert!(matches!(
        &error,
        BuiltinInitializationError::Assembly(BuiltinAssemblyError::Registration(
            crate::node_system::registry::NodeRegistrationError::InvalidRegistry(
                crate::node_system::registry::RegistryValidationError::DuplicateNode(id),
            ),
        )) if id.as_str() == "yssbi.constant.bool"
    ));

    let assembly = std::error::Error::source(&error).unwrap();
    let registration = assembly.source().unwrap();
    assert!(
        registration
            .source()
            .and_then(|source| source.downcast_ref::<crate::node_system::registry::RegistryValidationError>())
            .is_some_and(|source| matches!(source, crate::node_system::registry::RegistryValidationError::DuplicateNode(id) if id.as_str() == "yssbi.constant.bool"))
    );
}

#[test]
fn builtin_assembly_preserves_parameter_decimal_and_default_sources() {
    let builtin = build_builtin_node_system().unwrap();
    let constant = builtin
        .registry
        .protocol(&NodeTypeId::new("yssbi.constant.bool").unwrap())
        .unwrap();
    let parameter = constant.parameters.parameters[0].clone();
    let parameter_error = super::builtin::assembled_parameters(
        "yssbi.test.parameters",
        vec![parameter.clone(), parameter],
    )
    .unwrap_err();
    assert!(matches!(
        &parameter_error,
        BuiltinAssemblyError::InvalidParameterSchema { node_type, .. }
            if node_type.as_ref() == "yssbi.test.parameters"
    ));
    let parameter_source = parameter_error
        .source()
        .and_then(|source| {
            source.downcast_ref::<crate::node_system::protocol::ParameterSchemaError>()
        })
        .expect("parameter schema source");
    assert!(matches!(
        parameter_source,
        crate::node_system::protocol::ParameterSchemaError::DuplicateKey(error)
            if error.0.as_str() == "value"
    ));
    assert!(
        parameter_source
            .source()
            .and_then(|source| source
                .downcast_ref::<crate::node_system::protocol::DuplicateParameterKey>())
            .is_some_and(|source| source.0.as_str() == "value")
    );

    let decimal_error = super::builtin::assembled_decimal("yssbi.test.decimal", "01").unwrap_err();
    assert!(matches!(
        &decimal_error,
        BuiltinAssemblyError::InvalidDecimal { node_type, .. }
            if node_type.as_ref() == "yssbi.test.decimal"
    ));
    assert!(
        decimal_error
            .source()
            .and_then(|source| source.downcast_ref::<crate::node_system::protocol::InvalidDecimal>())
            .is_some_and(|source| source.to_string() == "'01' is not a canonical decimal")
    );

    let mut port = constant.interface.ports[0].clone();
    port.direction = PortDirection::Input;
    port.input_binding = Some(crate::node_system::protocol::InputBindingSpec {
        literal_policy: crate::node_system::protocol::LiteralPolicy::Allowed,
        default_value: Some(crate::node_system::protocol::TypedValue {
            value_type: TypeExpr::Concrete(
                crate::node_system::protocol::TypeId::new("core.string").unwrap(),
            ),
            value: crate::node_system::protocol::Value::String("wrong".into()),
        }),
    });
    port.consumption = Some(crate::node_system::protocol::InputConsumption::FullyMaterialized);
    port.production = None;
    let default_error = super::builtin::assembled_interface(
        "yssbi.test.default",
        vec![port],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap_err();
    assert!(matches!(
        &default_error,
        BuiltinAssemblyError::InvalidDefaultBinding { node_type, .. }
            if node_type.as_ref() == "yssbi.test.default"
    ));
    assert!(
        default_error
            .source()
            .and_then(|source| source.downcast_ref::<crate::node_system::protocol::ProtocolError>())
            .is_some_and(|source| matches!(
                source,
                crate::node_system::protocol::ProtocolError::InvalidPortContract { key, reason }
                    if key.as_str() == "value"
                        && *reason == "typed default does not match the port value type"
            ))
    );
}

#[test]
fn every_production_compiler_definition_is_required_by_builtin_i18n() {
    let (provider, _, _) = builtin_bundle_parts_for_test().unwrap();
    let missing = COMPILER_DIAGNOSTIC_DEFINITIONS
        .iter()
        .filter(|definition| {
            let message_key = I18nKey::new(definition.message_key).unwrap();
            !provider.i18n.keys.contains(&message_key)
        })
        .map(|definition| definition.message_key)
        .collect::<Vec<_>>();

    assert!(
        missing.is_empty(),
        "compiler diagnostic definitions missing from built-in i18n requirements:\n{}",
        missing.join("\n")
    );
}

#[test]
fn builtin_assembly_preserves_diagnostic_definition_error_source() {
    let source = CompilerDiagnosticDefinitionError::DuplicateCode {
        code: "compiler.test.duplicate".into(),
    };
    let error = BuiltinAssemblyError::DiagnosticDefinitions {
        source: source.clone(),
    };

    assert_eq!(
        error.to_string(),
        "built-in compiler diagnostic definitions are invalid: duplicate diagnostic code: compiler.test.duplicate"
    );
    assert_eq!(
        error
            .source()
            .and_then(|source| source.downcast_ref::<CompilerDiagnosticDefinitionError>()),
        Some(&source)
    );
}

#[test]
fn builtin_startup_rejects_missing_compiler_default_template() {
    let (provider, mut catalog, alias_keys) = builtin_bundle_parts_for_test().unwrap();
    let missing = I18nKey::new("diagnostics.compiler.node.scope_mismatch").unwrap();
    catalog.remove_message_for_test("en-US", &missing);

    assert!(matches!(
        validate_builtin_bundle_for_test(provider, catalog, alias_keys),
        Err(BuiltinInitializationError::Localization(
            I18nBundleValidationError::MissingDefaultLocale { keys }
        )) if keys.iter().any(|key| key.as_ref() == missing.as_str())
    ));
}

#[test]
fn builtin_startup_rejects_missing_default_locale_key() {
    let (provider, mut catalog, alias_keys) = builtin_bundle_parts_for_test().unwrap();
    let missing = provider.i18n.keys.iter().next().unwrap().clone();
    catalog.remove_message_for_test("en-US", &missing);

    assert!(matches!(
        validate_builtin_bundle_for_test(provider, catalog, alias_keys),
        Err(BuiltinInitializationError::Localization(
            I18nBundleValidationError::MissingDefaultLocale { keys }
        )) if keys.iter().any(|key| key.as_ref() == missing.as_str())
    ));
}

#[test]
fn builtin_startup_rejects_alias_stored_as_text() {
    let (provider, mut catalog, alias_keys) = builtin_bundle_parts_for_test().unwrap();
    let alias = alias_keys.iter().next().unwrap().clone();
    catalog.replace_message_for_test("en-US", alias.clone(), Text("malformed aliases"));

    assert!(matches!(
        validate_builtin_bundle_for_test(provider, catalog, alias_keys),
        Err(BuiltinInitializationError::Localization(
            I18nBundleValidationError::AliasesNotArray { locale, key }
        )) if locale.as_ref() == "en-US" && key.as_ref() == alias.as_str()
    ));
}

#[test]
fn builtin_startup_rejects_invalid_registry_before_returning_a_bundle() {
    let (mut provider, catalog, alias_keys) = builtin_bundle_parts_for_test().unwrap();
    provider.types[0].title_key = "missing.type.title".parse().unwrap();

    assert!(matches!(
        validate_builtin_bundle_for_test(provider, catalog, alias_keys),
        Err(BuiltinInitializationError::Assembly(
            BuiltinAssemblyError::Registration(_)
        ))
    ));
}

#[test]
fn builtin_startup_returns_one_validated_registry_and_catalog_bundle() {
    let bundle = build_builtin_node_system().unwrap();

    assert!(!bundle.registry.is_empty());
    assert_eq!(
        bundle
            .registry
            .node_provider(&NodeTypeId::new("yssbi.constant.bool").unwrap())
            .map(crate::node_system::protocol::ProviderId::as_str),
        Some("yssbi.builtin")
    );
    assert!(
        !bundle
            .catalog
            .localize(&bundle.registry, "en-US")
            .items
            .is_empty()
    );
}

#[test]
fn compiler_diagnostics_render_in_english_and_chinese() {
    let catalog = build_builtin_node_system().unwrap().catalog;
    let key = I18nKey::new("diagnostics.compiler.type.incompatible").unwrap();
    let arguments = DiagnosticArguments::from([
        (
            Box::<str>::from("actual_type"),
            Box::<str>::from("core.string"),
        ),
        (
            Box::<str>::from("expected_type"),
            Box::<str>::from("core.integer"),
        ),
    ]);

    assert_eq!(
        catalog
            .localization("en-US")
            .text(&key, &arguments)
            .as_ref(),
        "Type core.string is incompatible with core.integer."
    );
    assert_eq!(
        catalog
            .localization("zh-CN")
            .text(&key, &arguments)
            .as_ref(),
        "类型 core.string 与 core.integer 不兼容。"
    );
}

#[test]
fn diagnostic_rendering_does_not_rewrite_inserted_argument_placeholders() {
    let catalog = build_builtin_node_system().unwrap().catalog;
    let key = I18nKey::new("diagnostics.compiler.type.incompatible").unwrap();
    let arguments = DiagnosticArguments::from([
        (
            Box::<str>::from("actual_type"),
            Box::<str>::from("literal {expected_type}"),
        ),
        (
            Box::<str>::from("expected_type"),
            Box::<str>::from("core.integer"),
        ),
    ]);

    assert_eq!(
        catalog
            .localization("en-US")
            .text(&key, &arguments)
            .as_ref(),
        "Type literal {expected_type} is incompatible with core.integer."
    );
}

#[test]
fn trusted_provider_freezes_with_complete_inventory() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    assert!(registry.len() >= 20);
    assert!(
        registry
            .get(&NodeTypeId::new("yssbi.control.branch").unwrap())
            .unwrap()
            .structural_role()
            .is_some()
    );
    assert_eq!(
        item(
            &catalog.localize(&registry, "en-US"),
            "yssbi.control.branch"
        )
        .title
        .as_ref(),
        "Branch"
    );
}

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
            assert!(
                item.search_text
                    .contains(normalize_search_text(&item.title).as_ref())
            );
            assert!(
                !item
                    .search_text
                    .contains(normalize_search_text(id).as_ref())
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
            pinyin: None,
        },
        CatalogResourceEntry {
            name: "Tax Rate".into(),
            node_type_id: NodeTypeId::new("yssbi.project.variable.get").unwrap(),
            resource_path: CatalogResourcePath::new("variables/tax-rate"),
            resource_revision: crate::node_system::document::ResourceRevision::INITIAL,
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

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
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

#[test]
fn legacy_production_catalog_has_complete_stable_manifest_coverage() {
    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
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
        148,
        "legacy node migration manifest changed"
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
fn every_emitted_native_kernel_has_a_production_implementation() {
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
        let parameters = ValidatedNodeConfig::from_analysis(
            &node.protocol(),
            BTreeMap::new(),
            |type_id, value| nodes.prepare_nominal_parameter(type_id, value),
        );
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
