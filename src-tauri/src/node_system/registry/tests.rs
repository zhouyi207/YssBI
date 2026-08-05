use super::*;
use crate::node_system::compiler::{LoweredNode, LoweringContext, LoweringError, NodeLowerer};
use crate::node_system::protocol::*;
use crate::node_system::testing::TestProtocolBuilder;
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[test]
fn canonical_encoding_failures_remain_typed() {
    struct RefusesSerialization;

    impl serde::Serialize for RefusesSerialization {
        fn serialize<S>(&self, _: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            Err(serde::ser::Error::custom("canonical encoding rejected"))
        }
    }

    let error = hash_canonical("yssbi.registry.test", &RefusesSerialization).unwrap_err();
    assert_eq!(error.to_string(), "canonical encoding rejected");
    assert_eq!(
        std::error::Error::source(&error).map(ToString::to_string),
        Some("canonical encoding rejected".to_string())
    );
}

#[test]
fn node_registration_error_preserves_typed_sources() {
    let protocol =
        NodeRegistrationError::InvalidProtocol(ProtocolError::InvalidPortMemberGroup("bad group"));
    assert!(
        std::error::Error::source(&protocol)
            .and_then(|source| source.downcast_ref::<ProtocolError>())
            .is_some_and(|source| matches!(
                source,
                ProtocolError::InvalidPortMemberGroup("bad group")
            ))
    );

    let id = NodeTypeId::new("yssbi.test.duplicate").unwrap();
    let registry =
        NodeRegistrationError::InvalidRegistry(RegistryValidationError::DuplicateNode(id.clone()));
    assert!(
        std::error::Error::source(&registry)
            .and_then(|source| source.downcast_ref::<RegistryValidationError>())
            .is_some_and(|source| matches!(source, RegistryValidationError::DuplicateNode(actual) if actual == &id))
    );
}

#[test]
fn frozen_registry_state_is_scoped_to_the_registry_module() {
    let source = include_str!("model.rs");
    for exposed_field in [
        "pub(crate) protocol:",
        "pub(crate) implementation:",
        "pub(crate) structural_role:",
        "pub(crate) by_id:",
        "pub(crate) type_index:",
        "pub(crate) category_index:",
        "pub(crate) catalog_manifest:",
        "pub(crate) nominal_validators:",
        "pub(crate) fingerprint:",
    ] {
        assert!(
            !source.contains(exposed_field),
            "frozen Registry state remains crate-wide: {exposed_field}"
        );
    }
}

fn protocol() -> NodeProtocol {
    TestProtocolBuilder::new("yssbi.test.empty", "test")
        .managed_role(None)
        .build()
}

fn id<T>(value: &str) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Debug,
{
    value.parse().unwrap()
}
struct RegistryTestLowerer;
struct AlternateRegistryTestLowerer;

impl NodeLowerer for RegistryTestLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        unreachable!("registry validation never lowers nodes")
    }
}

impl NodeLowerer for AlternateRegistryTestLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        unreachable!("registry validation never lowers nodes")
    }
}

fn implementation() -> LeafImplementation {
    crate::node_system::compiler::NodeImplementation::new(RegistryTestLowerer).into()
}

struct NotALowerer;

impl NodeImplementation for NotALowerer {
    fn capability(&self) -> NodeImplementationCapability {
        NodeImplementationCapability::Unsupported
    }

    fn implementation_identity(&self) -> &str {
        "registry.tests.not-a-lowerer"
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
fn keys(values: &[&str]) -> I18nManifest {
    I18nManifest {
        keys: values.iter().map(|x| id(x)).collect(),
    }
}

fn provider_with(node: RegisteredNode) -> ProviderRegistration {
    let mut provider = ProviderRegistration::new(id("yssbi"));
    provider.categories = vec![CategoryRegistration {
        id: id("test"),
        title_key: id("categories.test.title"),
        parent: None,
        order: 0,
    }]
    .into_boxed_slice();
    provider.i18n = keys(&["categories.test.title", "nodes.test.empty.title"]);
    provider.nodes = vec![node].into_boxed_slice();
    provider
}
fn valid_provider() -> ProviderRegistration {
    provider_with(RegisteredNode::leaf(Arc::new(protocol()), implementation()))
}
fn error(provider: ProviderRegistration) -> RegistryValidationError {
    let mut builder = NodeRegistryBuilder::new();
    builder.register_provider(provider).unwrap();
    match builder.freeze().unwrap_err() {
        NodeRegistrationError::InvalidRegistry(e) => e,
        other => panic!("unexpected {other}"),
    }
}

fn provider_with_member_groups(groups: serde_json::Value) -> ProviderRegistration {
    let mut provider = valid_provider();
    let node = Arc::make_mut(&mut provider.nodes[0].protocol);
    node.interface.ports = [
        ("first", PortInstances::UserCreated { min: 0, max: None }),
        ("second", PortInstances::UserCreated { min: 0, max: None }),
        ("third", PortInstances::UserCreated { min: 0, max: None }),
        ("declared", PortInstances::Declared),
    ]
    .into_iter()
    .map(|(key, instances)| PortSpec {
        key: id(key),
        label_key: id(&format!("nodes.test.{key}")),
        direction: PortDirection::Input,
        kind: PortKind::Data,
        value_type: TypeExpr::Unknown,
        instances,
        connections: ConnectionsPerPort::Single,
        input_binding: Some(InputBindingSpec {
            literal_policy: LiteralPolicy::Allowed,
            default_value: None,
        }),
        consumption: Some(InputConsumption::FullyMaterialized),
        production: None,
        editor: PortEditorSpec::Default,
        schema: None,
    })
    .collect();
    provider.i18n.keys.extend([
        id("nodes.test.first"),
        id("nodes.test.second"),
        id("nodes.test.third"),
        id("nodes.test.declared"),
    ]);
    let mut interface = serde_json::to_value(&node.interface).unwrap();
    interface["member_groups"] = groups;
    node.interface = serde_json::from_value(interface).unwrap();
    provider
}

fn provider_with_nominal_type(type_name: &str) -> ProviderRegistration {
    let mut provider = valid_provider();
    let title: I18nKey = id(&format!("types.{type_name}.title"));
    provider.types = vec![TypeRegistration {
        id: id(type_name),
        title_key: title.clone(),
        classes: BTreeSet::new(),
    }]
    .into_boxed_slice();
    provider.i18n.keys.insert(title);
    provider
}

fn accepts_any_json(_: &serde_json::Value) -> Result<(), String> {
    Ok(())
}

fn rejects_null(value: &serde_json::Value) -> Result<(), String> {
    if value.is_null() {
        Err("null rejected by first validator".into())
    } else {
        Ok(())
    }
}

#[test]
fn nominal_validators_are_registered_and_looked_up_generically() {
    let mut builder = NodeRegistryBuilder::new();
    builder
        .register_provider(provider_with_nominal_type("acme.nominal"))
        .unwrap();
    builder
        .register_nominal_validator(
            id("acme.nominal"),
            id("acme.nominal.accept_any"),
            1,
            accepts_any_json,
        )
        .unwrap();

    let registry = builder.freeze().unwrap();

    assert_eq!(
        registry.validate_nominal_parameter(&id("acme.nominal"), &serde_json::json!({"any": true})),
        Some(Ok(()))
    );
}

#[test]
fn duplicate_nominal_validator_registration_preserves_first_validator() {
    let mut builder = NodeRegistryBuilder::new();
    builder
        .register_provider(provider_with_nominal_type("acme.nominal"))
        .unwrap();
    builder
        .register_nominal_validator(
            id("acme.nominal"),
            id("acme.nominal.reject_null"),
            1,
            rejects_null,
        )
        .unwrap();

    assert!(matches!(
        builder.register_nominal_validator(
            id("acme.nominal"),
            id("acme.nominal.accept_any"),
            2,
            accepts_any_json,
        ),
        Err(NodeRegistrationError::InvalidRegistry(
            RegistryValidationError::DuplicateNominalValidator(ref value)
        )) if value.as_str() == "acme.nominal"
    ));

    let registry = builder.freeze().unwrap();
    assert!(matches!(
        registry.validate_nominal_parameter(&id("acme.nominal"), &serde_json::Value::Null),
        Some(Err(ref error)) if error == "null rejected by first validator"
    ));
}

#[test]
fn nominal_validator_identity_and_version_change_registry_fingerprint() {
    fn freeze(identity: &'static str, version: u32) -> NodeRegistry {
        let mut builder = NodeRegistryBuilder::new();
        builder
            .register_provider(provider_with_nominal_type("acme.nominal"))
            .unwrap();
        builder
            .register_nominal_validator(id("acme.nominal"), id(identity), version, accepts_any_json)
            .unwrap();
        builder.freeze().unwrap()
    }

    let baseline = freeze("acme.nominal.codec", 1);
    let same = freeze("acme.nominal.codec", 1);
    let changed_identity = freeze("acme.nominal.codec_v2", 1);
    let changed_version = freeze("acme.nominal.codec", 2);

    assert_eq!(baseline.fingerprint(), same.fingerprint());
    assert_ne!(baseline.fingerprint(), changed_identity.fingerprint());
    assert_ne!(baseline.fingerprint(), changed_version.fingerprint());
}

#[test]
fn built_in_nominal_types_require_registered_validators() {
    let mut builder = NodeRegistryBuilder::new();
    builder
        .register_provider(provider_with_nominal_type(
            crate::node_system::parameter_types::dataframe::PROJECT_COLUMNS_TYPE_ID,
        ))
        .unwrap();

    assert!(matches!(
        builder.freeze(),
        Err(NodeRegistrationError::InvalidRegistry(
            RegistryValidationError::MissingNominalValidator(ref value)
        )) if value.as_str()
            == crate::node_system::parameter_types::dataframe::PROJECT_COLUMNS_TYPE_ID
    ));
}

#[test]
fn unrelated_custom_types_preserve_permissive_parameter_behavior() {
    let mut builder = NodeRegistryBuilder::new();
    builder
        .register_provider(provider_with_nominal_type("acme.unvalidated"))
        .unwrap();

    let registry = builder.freeze().unwrap();

    assert_eq!(
        registry.validate_nominal_parameter(
            &id("acme.unvalidated"),
            &serde_json::json!({"legacy": ["shape"]}),
        ),
        None
    );
}

#[test]
fn built_in_registry_exposes_strict_dataframe_nominal_validators() {
    let registry = crate::node_system::catalog::build_builtin_node_system()
        .unwrap()
        .registry;
    let project_type = id(crate::node_system::parameter_types::dataframe::PROJECT_COLUMNS_TYPE_ID);
    let filter_type = id(crate::node_system::parameter_types::dataframe::FILTER_PREDICATE_TYPE_ID);

    assert_eq!(
        registry.validate_nominal_parameter(&project_type, &serde_json::json!(["a"])),
        Some(Ok(()))
    );
    assert!(matches!(
        registry.validate_nominal_parameter(&project_type, &serde_json::json!([])),
        Some(Err(_))
    ));
    assert!(matches!(
        registry.validate_nominal_parameter(
            &filter_type,
            &serde_json::json!({"column":"a","operator":"equal"}),
        ),
        Some(Err(_))
    ));
}

#[test]
fn freezes_immutable_indexes_and_manifests() {
    let mut builder = NodeRegistryBuilder::new();
    builder.register_provider(valid_provider()).unwrap();
    let registry = builder.freeze().unwrap();
    assert_eq!(registry.len(), 1);
    assert!(registry.get(&id("yssbi.test.empty")).is_some());
    assert!(registry.categories().get(&id("test")).is_some());
    assert_eq!(registry.catalog_manifest().node_protocols.len(), 1);
    assert_eq!(registry.fingerprint().as_bytes().len(), 32);
}

#[test]
fn rejects_incomplete_or_invalid_port_member_groups() {
    for groups in [
        serde_json::json!([{"templates": ["first"], "min": 0, "max": null}]),
        serde_json::json!([{"templates": ["first", "missing"], "min": 0, "max": null}]),
        serde_json::json!([{"templates": ["first", "declared"], "min": 0, "max": null}]),
        serde_json::json!([{"templates": ["first", "second"], "min": 2, "max": 1}]),
    ] {
        assert!(matches!(
            error(provider_with_member_groups(groups)),
            RegistryValidationError::InvalidNodeProtocol {
                node,
                source: ProtocolError::InvalidPortMemberGroup(_),
            } if node.as_str() == "yssbi.test.empty"
        ));
    }
}

#[test]
fn rejects_templates_repeated_across_port_member_groups() {
    let groups = serde_json::json!([
        {"templates": ["first", "second"], "min": 0, "max": null},
        {"templates": ["second", "third"], "min": 0, "max": null}
    ]);
    assert!(matches!(
        error(provider_with_member_groups(groups)),
        RegistryValidationError::InvalidNodeProtocol {
            node,
            source: ProtocolError::InvalidPortMemberGroup(_),
        } if node.as_str() == "yssbi.test.empty"
    ));
}

#[test]
fn rejects_duplicate_global_identities() {
    let mut duplicate_node = valid_provider();
    duplicate_node.nodes = vec![
        duplicate_node.nodes[0].clone(),
        duplicate_node.nodes[0].clone(),
    ]
    .into_boxed_slice();
    assert!(matches!(
        error(duplicate_node),
        RegistryValidationError::DuplicateNode(_)
    ));

    let mut duplicate_category = valid_provider();
    duplicate_category.categories = vec![
        duplicate_category.categories[0].clone(),
        duplicate_category.categories[0].clone(),
    ]
    .into_boxed_slice();
    assert!(matches!(
        error(duplicate_category),
        RegistryValidationError::DuplicateCategory(_)
    ));

    let mut builder = NodeRegistryBuilder::new();
    builder.register_provider(valid_provider()).unwrap();
    assert!(matches!(
        builder.register_provider(valid_provider()),
        Err(NodeRegistrationError::InvalidRegistry(
            RegistryValidationError::DuplicateProvider(_)
        ))
    ));
}

#[test]
fn rejects_missing_category_and_i18n_references() {
    let mut missing_category = valid_provider();
    missing_category.categories = Box::new([]);
    assert!(matches!(
        error(missing_category),
        RegistryValidationError::InvalidNode { .. }
    ));

    let mut missing_i18n = valid_provider();
    missing_i18n.i18n.keys.remove(&id("nodes.test.empty.title"));
    assert!(matches!(
        error(missing_i18n),
        RegistryValidationError::InvalidNode { .. }
    ));
}

#[test]
fn validates_type_references_constructor_arity_and_classes() {
    let mut provider = valid_provider();
    let node = Arc::make_mut(&mut provider.nodes[0].protocol);
    node.interface.ports = vec![PortSpec {
        key: id("value"),
        label_key: id("nodes.test.value"),
        direction: PortDirection::Output,
        kind: PortKind::Data,
        value_type: TypeExpr::Concrete(id("core.missing")),
        instances: PortInstances::Declared,
        connections: ConnectionsPerPort::Single,
        input_binding: None,
        consumption: None,
        production: None,
        editor: PortEditorSpec::Default,
        schema: None,
    }]
    .into_boxed_slice();
    provider.i18n.keys.insert(id("nodes.test.value"));
    assert!(matches!(
        error(provider),
        RegistryValidationError::InvalidNode { .. }
    ));

    let mut provider = valid_provider();
    provider.types = vec![TypeRegistration {
        id: id("core.value"),
        title_key: id("types.value.title"),
        classes: BTreeSet::from([id("core.numeric")]),
    }]
    .into_boxed_slice();
    provider.i18n.keys.insert(id("types.value.title"));
    assert!(matches!(
        error(provider),
        RegistryValidationError::InvalidType { .. }
    ));
}

#[test]
fn validates_ports_parameters_schema_and_resolvers() {
    let mut provider = valid_provider();
    let node = Arc::make_mut(&mut provider.nodes[0].protocol);
    node.interface.ports = vec![PortSpec {
        key: id("derived"),
        label_key: id("nodes.test.derived"),
        direction: PortDirection::Output,
        kind: PortKind::Data,
        value_type: TypeExpr::Unknown,
        instances: PortInstances::Derived {
            resolver: id("yssbi.missing"),
        },
        connections: ConnectionsPerPort::Single,
        input_binding: None,
        consumption: None,
        production: None,
        editor: PortEditorSpec::Default,
        schema: None,
    }]
    .into_boxed_slice();
    provider.i18n.keys.insert(id("nodes.test.derived"));
    assert!(matches!(
        error(provider),
        RegistryValidationError::InvalidNode { .. }
    ));

    let mut provider = valid_provider();
    let node = Arc::make_mut(&mut provider.nodes[0].protocol);
    let key: ParameterKey = id("limit");
    let parameter = ParameterSpec {
        key: key.clone(),
        title_key: id("nodes.test.limit"),
        description_key: None,
        value_type: TypeExpr::Unknown,
        default_value: None,
        constraints: vec![],
        editor: ParameterEditorSpec::Auto,
    };
    node.parameters.parameters = vec![parameter.clone(), parameter].into_boxed_slice();
    provider.i18n.keys.insert(id("nodes.test.limit"));
    assert!(matches!(
        error(provider),
        RegistryValidationError::InvalidNode { .. }
    ));
}

fn provider_with_two_parameter_rename_schema() -> ProviderRegistration {
    let mut provider = valid_provider();
    let node = Arc::make_mut(&mut provider.nodes[0].protocol);
    node.interface.ports = vec![
        PortSpec {
            key: id("source"),
            label_key: id("nodes.test.source"),
            direction: PortDirection::Input,
            kind: PortKind::Data,
            value_type: TypeExpr::Unknown,
            instances: PortInstances::Declared,
            connections: ConnectionsPerPort::Single,
            input_binding: Some(InputBindingSpec {
                literal_policy: LiteralPolicy::Forbidden,
                default_value: None,
            }),
            consumption: Some(InputConsumption::Streaming),
            production: None,
            editor: PortEditorSpec::Default,
            schema: None,
        },
        PortSpec {
            key: id("result"),
            label_key: id("nodes.test.result"),
            direction: PortDirection::Output,
            kind: PortKind::Data,
            value_type: TypeExpr::Unknown,
            instances: PortInstances::Declared,
            connections: ConnectionsPerPort::Single,
            input_binding: None,
            consumption: None,
            production: Some(OutputProduction::Streaming),
            editor: PortEditorSpec::Default,
            schema: Some(SchemaExpr::Rename {
                input: Box::new(SchemaExpr::Input(id("source"))),
                mapping: RenameExpr::FromParameters {
                    from: id("from"),
                    to: id("to"),
                },
            }),
        },
    ]
    .into_boxed_slice();
    node.parameters.parameters = ["from", "to"]
        .into_iter()
        .map(|key| ParameterSpec {
            key: id(key),
            title_key: id(&format!("nodes.test.{key}")),
            description_key: None,
            value_type: TypeExpr::Unknown,
            default_value: None,
            constraints: vec![ParameterConstraint::Required],
            editor: ParameterEditorSpec::Text { multiline: false },
        })
        .collect();
    provider.i18n.keys.extend([
        id("nodes.test.source"),
        id("nodes.test.result"),
        id("nodes.test.from"),
        id("nodes.test.to"),
    ]);
    provider
}

#[test]
fn validates_both_two_parameter_rename_schema_references() {
    let mut builder = NodeRegistryBuilder::new();
    builder
        .register_provider(provider_with_two_parameter_rename_schema())
        .unwrap();
    builder.freeze().expect("both rename parameters freeze");

    for missing in ["from", "to"] {
        let mut provider = provider_with_two_parameter_rename_schema();
        let node = Arc::make_mut(&mut provider.nodes[0].protocol);
        node.parameters.parameters = node
            .parameters
            .parameters
            .iter()
            .filter(|parameter| parameter.key.as_str() != missing)
            .cloned()
            .collect();
        assert!(matches!(
            error(provider),
            RegistryValidationError::InvalidNode { reason, .. }
                if reason == format!("schema references unknown parameter '{missing}'")
        ));
    }
}

#[test]
fn freeze_rejects_an_implementation_without_lowerer_capability() {
    let mut provider = valid_provider();
    provider.nodes[0].implementation = Some(LeafImplementation::new(NotALowerer));

    assert!(matches!(
        error(provider),
        RegistryValidationError::InvalidNode { reason, .. }
            if reason == "leaf implementation does not provide lowerer capability"
    ));
}

#[test]
fn freeze_rejects_node_without_executable_interpretation() {
    let node = RegisteredNode {
        protocol: Arc::new(protocol()),
        implementation: None,
        structural_role: None,
    };

    assert!(matches!(
        error(provider_with(node)),
        RegistryValidationError::InvalidNode { reason, .. }
            if reason == "node has no executable interpretation"
    ));
}

#[test]
fn freeze_rejects_node_with_both_leaf_and_structural_interpretations() {
    let mut node = RegisteredNode::leaf(Arc::new(protocol()), implementation());
    node.structural_role = Some(StructuralNodeRole::Branch);

    assert!(matches!(
        error(provider_with(node)),
        RegistryValidationError::InvalidNode { reason, .. }
            if reason == "leaf implementation and structural role are mutually exclusive"
    ));
}

#[test]
fn leaf_and_structural_nodes_are_the_only_frozen_forms() {
    for node in [
        RegisteredNode::leaf(Arc::new(protocol()), implementation()),
        RegisteredNode::structural(Arc::new(protocol()), StructuralNodeRole::Branch),
    ] {
        let mut builder = NodeRegistryBuilder::new();
        builder.register_provider(provider_with(node)).unwrap();
        builder.freeze().expect("executable node freezes");
    }
}

#[test]
fn leaf_constructor_requires_an_explicit_leaf_implementation() {
    let implementation: LeafImplementation =
        crate::node_system::compiler::NodeImplementation::new(RegistryTestLowerer).into();
    let node = RegisteredNode::leaf(Arc::new(protocol()), implementation);

    assert_eq!(
        node.implementation.unwrap().capability(),
        ImplementationKind::CompilerLowering
    );
    // `Arc<()>` has no `Into<LeafImplementation>` implementation and cannot call this constructor.
}

#[test]
fn enforces_leaf_structural_and_managed_scope_contracts() {
    let mut both = valid_provider();
    both.nodes[0].structural_role = Some(StructuralNodeRole::Branch);
    assert!(matches!(
        error(both),
        RegistryValidationError::InvalidNode { .. }
    ));

    let mut managed = valid_provider();
    let protocol = Arc::make_mut(&mut managed.nodes[0].protocol);
    protocol.managed_role = Some(ManagedNodeRole::FunctionEntry);
    protocol.scope = NodeScope::Event;
    assert!(matches!(
        error(managed),
        RegistryValidationError::InvalidNode { .. }
    ));
}

#[test]
fn fingerprints_are_canonical_and_protocol_sensitive() {
    fn frozen(mut providers: Vec<ProviderRegistration>) -> NodeRegistry {
        let mut builder = NodeRegistryBuilder::new();
        for provider in providers.drain(..) {
            builder.register_provider(provider).unwrap();
        }
        builder.freeze().unwrap()
    }
    let first = frozen(vec![valid_provider()]);
    let second = frozen(vec![valid_provider()]);
    assert_eq!(first.fingerprint(), second.fingerprint());

    let mut changed = valid_provider();
    Arc::make_mut(&mut changed.nodes[0].protocol)
        .execution
        .cache = CachePolicy::None;
    let changed = frozen(vec![changed]);
    assert_ne!(first.fingerprint(), changed.fingerprint());
    assert_ne!(
        first.catalog_manifest().node_protocols,
        changed.catalog_manifest().node_protocols
    );
}

#[test]
fn port_member_groups_change_protocol_fingerprint() {
    fn frozen(provider: ProviderRegistration) -> NodeRegistry {
        let mut builder = NodeRegistryBuilder::new();
        builder.register_provider(provider).unwrap();
        builder.freeze().unwrap()
    }

    let without_group = frozen(provider_with_member_groups(serde_json::json!([])));
    let with_group = frozen(provider_with_member_groups(serde_json::json!([{
        "templates": ["first", "second"],
        "min": 0,
        "max": null
    }])));

    assert_ne!(without_group.fingerprint(), with_group.fingerprint());
    assert_ne!(
        without_group.catalog_manifest().node_protocols,
        with_group.catalog_manifest().node_protocols
    );
}

#[test]
fn display_metadata_does_not_change_semantic_snapshot_or_fingerprint() {
    fn frozen(provider: ProviderRegistration) -> NodeRegistry {
        let mut builder = NodeRegistryBuilder::new();
        builder.register_provider(provider).unwrap();
        builder.freeze().unwrap()
    }

    let first = frozen(valid_provider());
    let mut display_changed = valid_provider();
    let protocol = Arc::make_mut(&mut display_changed.nodes[0].protocol);
    protocol.catalog.hidden = true;
    protocol.catalog.icon_id = id("another_icon");
    protocol.catalog.style_id = id("another_style");
    let second = frozen(display_changed);

    assert_eq!(first.fingerprint(), second.fingerprint());
    assert_eq!(
        canonical_semantic_protocol_snapshot(&first).unwrap(),
        canonical_semantic_protocol_snapshot(&second).unwrap()
    );
}

#[test]
fn protocol_snapshot_and_i18n_inventory_are_deterministic() {
    let mut builder = NodeRegistryBuilder::new();
    builder.register_provider(valid_provider()).unwrap();
    let registry = builder.freeze().unwrap();

    let snapshot = canonical_semantic_protocol_snapshot(&registry).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&snapshot).unwrap();
    assert_eq!(parsed["nodes"][0]["nodeTypeId"], "yssbi.test.empty");
    assert!(parsed["nodes"][0]["protocol"].get("catalog").is_none());

    let inventory: Vec<String> = serde_json::from_str(&i18n_inventory(&registry).unwrap()).unwrap();
    assert!(inventory.windows(2).all(|keys| keys[0] < keys[1]));
    assert_eq!(
        inventory,
        vec!["categories.test.title", "nodes.test.empty.title"]
    );
}

fn provenance_providers() -> [ProviderRegistration; 2] {
    let mut builtin = provider_with_nominal_type("yssbi.value");
    builtin.provider = id("yssbi.builtin");

    let mut acme = provider_with_nominal_type("acme.value");
    acme.provider = id("acme.nodes");
    acme.categories[0].id = id("acme_test");
    acme.categories[0].title_key = id("categories.acme_test.title");
    acme.i18n = keys(&[
        "categories.acme_test.title",
        "nodes.acme.empty.title",
        "types.acme.value.title",
    ]);
    let protocol = Arc::make_mut(&mut acme.nodes[0].protocol);
    protocol.type_id = id("acme.test.empty");
    protocol.catalog.category_id = id("acme_test");
    protocol.catalog.title_key = id("nodes.acme.empty.title");

    [builtin, acme]
}

fn freeze_provenance_providers(
    providers: impl IntoIterator<Item = ProviderRegistration>,
) -> Result<NodeRegistry, NodeRegistrationError> {
    let mut builder = NodeRegistryBuilder::new();
    for provider in providers {
        builder.register_provider(provider).unwrap();
    }
    builder.freeze()
}

#[test]
fn provider_provenance_is_exact_and_registration_order_independent() {
    let [builtin, acme] = provenance_providers();
    let forward = freeze_provenance_providers([builtin.clone(), acme.clone()]).unwrap();
    let reverse = freeze_provenance_providers([acme, builtin]).unwrap();

    for registry in [&forward, &reverse] {
        assert_eq!(
            registry
                .node_provider(&id("yssbi.test.empty"))
                .map(ProviderId::as_str),
            Some("yssbi.builtin")
        );
        assert_eq!(
            registry
                .type_provider(&id("yssbi.value"))
                .map(ProviderId::as_str),
            Some("yssbi.builtin")
        );
        assert_eq!(
            registry
                .node_provider(&id("acme.test.empty"))
                .map(ProviderId::as_str),
            Some("acme.nodes")
        );
        assert_eq!(
            registry
                .type_provider(&id("acme.value"))
                .map(ProviderId::as_str),
            Some("acme.nodes")
        );
    }
    assert_eq!(forward.fingerprint(), reverse.fingerprint());
}

#[test]
fn provider_provenance_duplicate_node_returns_no_registry() {
    let [builtin, mut acme] = provenance_providers();
    let duplicate = builtin.nodes[0].protocol().type_id.clone();
    Arc::make_mut(&mut acme.nodes[0].protocol).type_id = duplicate.clone();

    assert!(matches!(
        freeze_provenance_providers([builtin, acme]),
        Err(NodeRegistrationError::InvalidRegistry(
            RegistryValidationError::DuplicateNode(id)
        )) if id == duplicate
    ));
}

#[test]
fn provider_provenance_duplicate_type_returns_no_registry() {
    let [builtin, mut acme] = provenance_providers();
    let duplicate = builtin.types[0].id.clone();
    acme.types[0].id = duplicate.clone();

    assert!(matches!(
        freeze_provenance_providers([builtin, acme]),
        Err(NodeRegistrationError::InvalidRegistry(
            RegistryValidationError::DuplicateType(id)
        )) if id == duplicate
    ));
}

#[test]
fn provider_provenance_covers_every_builtin_node_and_type() {
    let registry = crate::node_system::catalog::build_builtin_node_system()
        .unwrap()
        .registry;

    for (id, _) in registry.iter() {
        assert_eq!(
            registry.node_provider(id).map(ProviderId::as_str),
            Some("yssbi.builtin")
        );
    }
    for (id, _) in registry.types().iter() {
        assert_eq!(
            registry.type_provider(id).map(ProviderId::as_str),
            Some("yssbi.builtin")
        );
    }
}

fn duplicate_provider_pair() -> [ProviderRegistration; 2] {
    provenance_providers()
}

fn duplicate_result(kind: &str) -> Result<NodeRegistry, NodeRegistrationError> {
    let [first, mut second] = duplicate_provider_pair();
    let mut builder = NodeRegistryBuilder::new();
    builder.register_provider(first.clone())?;

    match kind {
        "provider" => builder.register_provider(first)?,
        "node" => {
            Arc::make_mut(&mut second.nodes[0].protocol).type_id =
                first.nodes[0].protocol().type_id.clone();
            builder.register_provider(second)?;
        }
        "type" => {
            second.types[0].id = first.types[0].id.clone();
            builder.register_provider(second)?;
        }
        "type constructor" => {
            let constructor = TypeConstructorRegistration {
                id: id("test.list"),
                title_key: id("types.test.list.title"),
                arity: 1,
            };
            let mut first = first;
            first.type_constructors = vec![constructor.clone()].into_boxed_slice();
            first.i18n.keys.insert(constructor.title_key.clone());
            second.type_constructors = vec![constructor.clone()].into_boxed_slice();
            second.i18n.keys.insert(constructor.title_key);
            let mut builder = NodeRegistryBuilder::new();
            builder.register_provider(first)?;
            builder.register_provider(second)?;
            return builder.freeze();
        }
        "type class" => {
            let class: TypeClassId = id("test.scalar");
            let mut first = first;
            first.type_classes = vec![class.clone()].into_boxed_slice();
            second.type_classes = vec![class].into_boxed_slice();
            let mut builder = NodeRegistryBuilder::new();
            builder.register_provider(first)?;
            builder.register_provider(second)?;
            return builder.freeze();
        }
        "category" => {
            second.categories[0].id = first.categories[0].id.clone();
            builder.register_provider(second)?;
        }
        "i18n key" => {
            second.i18n.keys.insert(id("categories.test.title"));
            builder.register_provider(second)?;
        }
        "interface resolver" => {
            let resolver: InterfaceResolverId = id("test.interface");
            let mut first = first;
            first.interface_resolvers = vec![resolver.clone()].into_boxed_slice();
            second.interface_resolvers = vec![resolver].into_boxed_slice();
            let mut builder = NodeRegistryBuilder::new();
            builder.register_provider(first)?;
            builder.register_provider(second)?;
            return builder.freeze();
        }
        "schema resolver" => {
            let resolver: SchemaResolverId = id("test.schema");
            let mut first = first;
            first.schema_resolvers = vec![resolver.clone()].into_boxed_slice();
            second.schema_resolvers = vec![resolver].into_boxed_slice();
            let mut builder = NodeRegistryBuilder::new();
            builder.register_provider(first)?;
            builder.register_provider(second)?;
            return builder.freeze();
        }
        "nominal validator" => {
            builder.register_nominal_validator(
                id("yssbi.value"),
                id("test.validator"),
                1,
                accepts_any_json,
            )?;
            builder.register_nominal_validator(
                id("yssbi.value"),
                id("test.validator.v2"),
                2,
                accepts_any_json,
            )?;
        }
        other => panic!("unknown duplicate matrix case {other}"),
    }

    builder.freeze()
}

#[test]
fn duplicate_global_identity_matrix() {
    use RegistryValidationError::*;

    for (kind, expected) in [
        ("provider", "DuplicateProvider"),
        ("node", "DuplicateNode"),
        ("type", "DuplicateType"),
        ("type constructor", "DuplicateTypeConstructor"),
        ("type class", "DuplicateTypeClass"),
        ("category", "DuplicateCategory"),
        ("i18n key", "DuplicateI18nKey"),
        ("interface resolver", "DuplicateInterfaceResolver"),
        ("schema resolver", "DuplicateSchemaResolver"),
        ("nominal validator", "DuplicateNominalValidator"),
    ] {
        let result = duplicate_result(kind);
        let actual = match result {
            Err(NodeRegistrationError::InvalidRegistry(DuplicateProvider(_))) => {
                "DuplicateProvider"
            }
            Err(NodeRegistrationError::InvalidRegistry(DuplicateNode(_))) => "DuplicateNode",
            Err(NodeRegistrationError::InvalidRegistry(DuplicateType(_))) => "DuplicateType",
            Err(NodeRegistrationError::InvalidRegistry(DuplicateTypeConstructor(_))) => {
                "DuplicateTypeConstructor"
            }
            Err(NodeRegistrationError::InvalidRegistry(DuplicateTypeClass(_))) => {
                "DuplicateTypeClass"
            }
            Err(NodeRegistrationError::InvalidRegistry(DuplicateCategory(_))) => {
                "DuplicateCategory"
            }
            Err(NodeRegistrationError::InvalidRegistry(DuplicateI18nKey(_))) => "DuplicateI18nKey",
            Err(NodeRegistrationError::InvalidRegistry(DuplicateInterfaceResolver(_))) => {
                "DuplicateInterfaceResolver"
            }
            Err(NodeRegistrationError::InvalidRegistry(DuplicateSchemaResolver(_))) => {
                "DuplicateSchemaResolver"
            }
            Err(NodeRegistrationError::InvalidRegistry(DuplicateNominalValidator(_))) => {
                "DuplicateNominalValidator"
            }
            Err(other) => panic!("{kind}: unexpected error {other}"),
            Ok(_) => panic!("{kind}: duplicate identity returned a frozen Registry"),
        };
        assert_eq!(actual, expected, "{kind}");
    }
}

#[derive(Clone, Copy)]
struct ValidatorFingerprintSpec {
    identity: &'static str,
    version: u32,
}

fn validator_spec(identity: &'static str, version: u32) -> Option<ValidatorFingerprintSpec> {
    Some(ValidatorFingerprintSpec { identity, version })
}

fn freeze_for_fingerprint(
    provider: ProviderRegistration,
    validator: Option<ValidatorFingerprintSpec>,
) -> NodeRegistry {
    let mut builder = NodeRegistryBuilder::new();
    builder.register_provider(provider).unwrap();
    if let Some(validator) = validator {
        builder
            .register_nominal_validator(
                id("acme.nominal"),
                id(validator.identity),
                validator.version,
                accepts_any_json,
            )
            .unwrap();
    }
    builder.freeze().unwrap()
}

fn canonical_for_fingerprint(
    provider: &ProviderRegistration,
    validator: Option<ValidatorFingerprintSpec>,
) -> serde_json::Value {
    let protocols = provider
        .nodes
        .iter()
        .map(|node| {
            (
                node.protocol().type_id.clone(),
                fingerprint::protocol_fingerprint(node.protocol()).unwrap(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let nominal_validators = validator
        .map(|validator| {
            BTreeMap::from([(
                id("acme.nominal"),
                NominalParameterValidator::new(
                    id(validator.identity),
                    validator.version,
                    accepts_any_json,
                ),
            )])
        })
        .unwrap_or_default();
    canonical_registry(
        std::slice::from_ref(provider),
        &protocols,
        &nominal_validators,
    )
}

fn assert_single_canonical_fingerprint_change(
    case: &str,
    left_provider: ProviderRegistration,
    right_provider: ProviderRegistration,
    left_validator: Option<ValidatorFingerprintSpec>,
    right_validator: Option<ValidatorFingerprintSpec>,
    target_pointer: &str,
) {
    let mut left_canonical = canonical_for_fingerprint(&left_provider, left_validator);
    let mut right_canonical = canonical_for_fingerprint(&right_provider, right_validator);
    let left_target = left_canonical
        .pointer(target_pointer)
        .unwrap_or_else(|| panic!("{case}: missing left canonical target {target_pointer}"))
        .clone();
    let right_target = right_canonical
        .pointer(target_pointer)
        .unwrap_or_else(|| panic!("{case}: missing right canonical target {target_pointer}"))
        .clone();
    assert_ne!(left_target, right_target, "{case}: target did not change");
    *left_canonical.pointer_mut(target_pointer).unwrap() = serde_json::json!("target");
    *right_canonical.pointer_mut(target_pointer).unwrap() = serde_json::json!("target");
    assert_eq!(
        left_canonical, right_canonical,
        "{case}: canonical input outside {target_pointer} changed"
    );

    let left = freeze_for_fingerprint(left_provider, left_validator);
    let right = freeze_for_fingerprint(right_provider, right_validator);
    assert_ne!(
        left.fingerprint(),
        right.fingerprint(),
        "{case}: target canonical change did not change fingerprint"
    );
}

fn assert_canonical_fingerprint_invariant(
    case: &str,
    left_provider: ProviderRegistration,
    right_provider: ProviderRegistration,
) {
    assert_eq!(
        canonical_for_fingerprint(&left_provider, None),
        canonical_for_fingerprint(&right_provider, None),
        "{case}: display-only input entered canonical Registry JSON"
    );
    assert_eq!(
        freeze_for_fingerprint(left_provider, None).fingerprint(),
        freeze_for_fingerprint(right_provider, None).fingerprint(),
        "{case}: display-only input changed fingerprint"
    );
}

fn provider_with_type_definition(type_id: &str, classes: &[&str]) -> ProviderRegistration {
    let mut provider = valid_provider();
    provider.type_classes = vec![id("acme.comparable"), id("acme.scalar")].into_boxed_slice();
    provider.types = vec![TypeRegistration {
        id: id(type_id),
        title_key: id("types.acme.value.title"),
        classes: classes.iter().map(|class| id(class)).collect(),
    }]
    .into_boxed_slice();
    provider.i18n.keys.insert(id("types.acme.value.title"));
    provider
}

fn provider_with_constructor_arity(arity: u16) -> ProviderRegistration {
    let mut provider = valid_provider();
    provider.type_constructors = vec![TypeConstructorRegistration {
        id: id("acme.list"),
        title_key: id("types.acme.list.title"),
        arity,
    }]
    .into_boxed_slice();
    provider.i18n.keys.insert(id("types.acme.list.title"));
    provider
}

fn structural_fingerprint_provider(role: StructuralNodeRole) -> ProviderRegistration {
    let mut provider = valid_provider();
    provider.nodes[0] = RegisteredNode::structural(provider.nodes[0].protocol.clone(), role);
    provider
}

fn lowerer_fingerprint_provider(alternate: bool) -> ProviderRegistration {
    let mut provider = valid_provider();
    provider.nodes[0].implementation = Some(if alternate {
        crate::node_system::compiler::NodeImplementation::new(AlternateRegistryTestLowerer).into()
    } else {
        crate::node_system::compiler::NodeImplementation::new(RegistryTestLowerer).into()
    });
    provider
}

#[test]
fn registry_fingerprint_matrix() {
    let mut changed_provider = valid_provider();
    changed_provider.provider = id("acme.changed");
    assert_single_canonical_fingerprint_change(
        "ProviderId",
        valid_provider(),
        changed_provider,
        None,
        None,
        "/providers/0/provider",
    );
    assert_single_canonical_fingerprint_change(
        "lowerer implementation identity",
        lowerer_fingerprint_provider(false),
        lowerer_fingerprint_provider(true),
        None,
        None,
        "/providers/0/nodes/0/2/implementationIdentity",
    );
    assert_single_canonical_fingerprint_change(
        "structural role",
        structural_fingerprint_provider(StructuralNodeRole::Branch),
        structural_fingerprint_provider(StructuralNodeRole::Loop),
        None,
        None,
        "/providers/0/nodes/0/2/role",
    );
    assert_single_canonical_fingerprint_change(
        "type definition",
        provider_with_type_definition("acme.value", &["acme.scalar"]),
        provider_with_type_definition("acme.value.v2", &["acme.scalar"]),
        None,
        None,
        "/providers/0/types/0/0",
    );
    assert_single_canonical_fingerprint_change(
        "type class membership",
        provider_with_type_definition("acme.value", &["acme.scalar"]),
        provider_with_type_definition("acme.value", &["acme.scalar", "acme.comparable"]),
        None,
        None,
        "/providers/0/types/0/1",
    );
    assert_single_canonical_fingerprint_change(
        "constructor arity",
        provider_with_constructor_arity(1),
        provider_with_constructor_arity(2),
        None,
        None,
        "/providers/0/constructors/0/1",
    );

    let mut interface_resolver = valid_provider();
    interface_resolver.interface_resolvers = vec![id("acme.interface")].into_boxed_slice();
    assert_single_canonical_fingerprint_change(
        "interface resolver inventory",
        valid_provider(),
        interface_resolver,
        None,
        None,
        "/providers/0/interface_resolvers",
    );
    let mut schema_resolver = valid_provider();
    schema_resolver.schema_resolvers = vec![id("acme.schema")].into_boxed_slice();
    assert_single_canonical_fingerprint_change(
        "schema resolver inventory",
        valid_provider(),
        schema_resolver,
        None,
        None,
        "/providers/0/schema_resolvers",
    );
    assert_single_canonical_fingerprint_change(
        "nominal validator identity",
        provider_with_nominal_type("acme.nominal"),
        provider_with_nominal_type("acme.nominal"),
        validator_spec("acme.validator", 1),
        validator_spec("acme.validator.v2", 1),
        "/nominalValidators/0/1",
    );
    assert_single_canonical_fingerprint_change(
        "nominal validator version",
        provider_with_nominal_type("acme.nominal"),
        provider_with_nominal_type("acme.nominal"),
        validator_spec("acme.validator", 1),
        validator_spec("acme.validator", 2),
        "/nominalValidators/0/2",
    );

    let [first, second] = provenance_providers();
    let forward = freeze_provenance_providers([first.clone(), second.clone()]).unwrap();
    let reverse = freeze_provenance_providers([second, first]).unwrap();
    assert_eq!(
        forward.fingerprint(),
        reverse.fingerprint(),
        "provider order"
    );

    let mut title_changed = valid_provider();
    Arc::make_mut(&mut title_changed.nodes[0].protocol)
        .catalog
        .title_key = id("nodes.test.changed.title");
    title_changed
        .i18n
        .keys
        .insert(id("nodes.test.changed.title"));
    title_changed
        .i18n
        .keys
        .remove(&id("nodes.test.empty.title"));
    assert_canonical_fingerprint_invariant("title key", valid_provider(), title_changed);

    let mut description_changed = valid_provider();
    Arc::make_mut(&mut description_changed.nodes[0].protocol)
        .catalog
        .description_key = Some(id("nodes.test.changed.description"));
    description_changed
        .i18n
        .keys
        .insert(id("nodes.test.changed.description"));
    assert_canonical_fingerprint_invariant(
        "description key",
        valid_provider(),
        description_changed,
    );

    let mut aliases_changed = valid_provider();
    Arc::make_mut(&mut aliases_changed.nodes[0].protocol)
        .catalog
        .aliases_key = Some(id("nodes.test.changed.aliases"));
    aliases_changed
        .i18n
        .keys
        .insert(id("nodes.test.changed.aliases"));
    assert_canonical_fingerprint_invariant("aliases key", valid_provider(), aliases_changed);

    let mut icon_changed = valid_provider();
    Arc::make_mut(&mut icon_changed.nodes[0].protocol)
        .catalog
        .icon_id = id("changed.icon");
    assert_canonical_fingerprint_invariant("icon", valid_provider(), icon_changed);

    let mut style_changed = valid_provider();
    Arc::make_mut(&mut style_changed.nodes[0].protocol)
        .catalog
        .style_id = id("changed.style");
    assert_canonical_fingerprint_invariant("style", valid_provider(), style_changed);

    let mut hidden_changed = valid_provider();
    Arc::make_mut(&mut hidden_changed.nodes[0].protocol)
        .catalog
        .hidden = true;
    assert_canonical_fingerprint_invariant("hidden", valid_provider(), hidden_changed);

    let mut category_changed = valid_provider();
    category_changed.categories[0].order = 99;
    assert_canonical_fingerprint_invariant(
        "category arrangement",
        valid_provider(),
        category_changed,
    );

    let (left_provider, mut left_catalog, left_aliases) =
        crate::node_system::catalog::builtin_bundle_parts_for_test().unwrap();
    let (right_provider, mut right_catalog, right_aliases) =
        crate::node_system::catalog::builtin_bundle_parts_for_test().unwrap();
    assert_eq!(
        canonical_for_fingerprint(&left_provider, None),
        canonical_for_fingerprint(&right_provider, None),
        "translation fixtures must use identical Registry/provider/protocol input"
    );
    assert_eq!(left_aliases, right_aliases);
    let title_key: I18nKey = id("nodes.yssbi.constant.bool.title");
    left_catalog.replace_text_for_test("en-US", title_key.clone(), "Contract Boolean A");
    right_catalog.replace_text_for_test("en-US", title_key, "Contract Boolean B");
    let left_bundle = crate::node_system::catalog::validate_builtin_bundle_for_test(
        left_provider,
        left_catalog,
        left_aliases,
    )
    .unwrap();
    let right_bundle = crate::node_system::catalog::validate_builtin_bundle_for_test(
        right_provider,
        right_catalog,
        right_aliases,
    )
    .unwrap();
    let localized_title = |bundle: &crate::node_system::catalog::BuiltinNodeSystem| {
        bundle
            .catalog
            .localize(&bundle.registry, "en-US")
            .items
            .into_iter()
            .find(|item| item.node_type_id.as_ref() == "yssbi.constant.bool")
            .unwrap()
            .title
    };
    assert_ne!(
        localized_title(&left_bundle),
        localized_title(&right_bundle)
    );
    assert_eq!(
        left_bundle.registry.fingerprint(),
        right_bundle.registry.fingerprint()
    );
    assert_eq!(
        canonical_semantic_protocol_snapshot(&left_bundle.registry).unwrap(),
        canonical_semantic_protocol_snapshot(&right_bundle.registry).unwrap()
    );

    let providers = vec![valid_provider()];
    let protocols = BTreeMap::from([(
        providers[0].nodes[0].protocol().type_id.clone(),
        fingerprint::protocol_fingerprint(providers[0].nodes[0].protocol()).unwrap(),
    )]);
    let canonical = canonical_registry(&providers, &protocols, &BTreeMap::new()).to_string();
    for forbidden in ["0x", "LeafImplementation", "Arc<", " at 0x"] {
        assert!(
            !canonical.contains(forbidden),
            "canonical Registry contains process-local/debug text {forbidden}: {canonical}"
        );
    }
}
