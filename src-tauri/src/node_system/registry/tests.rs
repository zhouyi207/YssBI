use super::*;
use crate::node_system::compiler::{LoweredNode, LoweringContext, LoweringError, NodeLowerer};
use crate::node_system::protocol::*;
use std::any::Any;
use std::collections::BTreeSet;
use std::sync::Arc;

const EXECUTION: ExecutionSemantics = ExecutionSemantics {
    determinism: Determinism::Deterministic,
    purity: Purity::Pure,
    evaluation: EvaluationPolicy::DemandDriven,
    cache: CachePolicy::PerRun,
    effects: EffectSemantics::None,
};
const PROTOCOL: StaticNodeProtocol = StaticNodeProtocol {
    type_id: "yssbi.test.empty",
    catalog: StaticNodeCatalogProtocol {
        title_key: "nodes.test.empty.title",
        description_key: None,
        documentation_key: None,
        aliases_key: None,
        category_id: "test",
        icon_id: "test",
        style_id: "default",
        hidden: false,
    },
    ports: &[],
    execution: EXECUTION,
    scope: NodeScope::Any,
    managed_role: None,
};

fn id<T>(value: &str) -> T
where
    T: std::str::FromStr,
    T::Err: std::fmt::Debug,
{
    value.parse().unwrap()
}
struct RegistryTestLowerer;

impl NodeLowerer for RegistryTestLowerer {
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
    provider_with(RegisteredNode::leaf_static(&PROTOCOL, implementation()).unwrap())
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
            RegistryValidationError::InvalidNode { .. }
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
        RegistryValidationError::InvalidNode { .. }
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
fn leaf_constructor_requires_an_explicit_leaf_implementation() {
    let implementation: LeafImplementation =
        crate::node_system::compiler::NodeImplementation::new(RegistryTestLowerer).into();
    let node = RegisteredNode::leaf(
        Arc::new(NodeProtocol::from_static(&PROTOCOL).unwrap()),
        implementation,
    );

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
        canonical_semantic_protocol_snapshot(&first),
        canonical_semantic_protocol_snapshot(&second)
    );
}

#[test]
fn protocol_snapshot_and_i18n_inventory_are_deterministic() {
    let mut builder = NodeRegistryBuilder::new();
    builder.register_provider(valid_provider()).unwrap();
    let registry = builder.freeze().unwrap();

    let snapshot = canonical_semantic_protocol_snapshot(&registry);
    let parsed: serde_json::Value = serde_json::from_str(&snapshot).unwrap();
    assert_eq!(parsed["nodes"][0]["nodeTypeId"], "yssbi.test.empty");
    assert!(parsed["nodes"][0]["protocol"].get("catalog").is_none());

    let inventory: Vec<String> = serde_json::from_str(&i18n_inventory(&registry)).unwrap();
    assert!(inventory.windows(2).all(|keys| keys[0] < keys[1]));
    assert_eq!(
        inventory,
        vec!["categories.test.title", "nodes.test.empty.title"]
    );
}

#[test]
fn provider_order_does_not_change_fingerprint() {
    let mut second = valid_provider();
    second.provider = id("acme");
    second.categories[0].id = id("acme_test");
    second.categories[0].title_key = id("categories.acme_test.title");
    second.i18n = keys(&["categories.acme_test.title", "nodes.acme.empty.title"]);
    let protocol = Arc::make_mut(&mut second.nodes[0].protocol);
    protocol.type_id = id("acme.test.empty");
    protocol.catalog.category_id = id("acme_test");
    protocol.catalog.title_key = id("nodes.acme.empty.title");
    let mut a = NodeRegistryBuilder::new();
    a.register_provider(valid_provider()).unwrap();
    a.register_provider(second.clone()).unwrap();
    let mut b = NodeRegistryBuilder::new();
    b.register_provider(second).unwrap();
    b.register_provider(valid_provider()).unwrap();
    assert_eq!(
        a.freeze().unwrap().fingerprint(),
        b.freeze().unwrap().fingerprint()
    );
}
