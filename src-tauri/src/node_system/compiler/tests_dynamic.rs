use super::dynamic_interface::*;
use crate::node_system::analysis::{CompilationBasis, ResolvedPortStatus};
use crate::node_system::document::*;
use crate::node_system::protocol::*;
use crate::node_system::registry::RegistryFingerprint;
use crate::node_system::testing::TestProtocolBuilder;
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

fn node_id() -> NodeId {
    NodeId::from_uuid(Uuid::from_u128(1))
}

fn address(value: u128) -> PortAddress {
    PortAddress::instance(
        node_id(),
        PortKey::new("fields").unwrap(),
        PortInstanceId::from_uuid(Uuid::from_u128(value)),
    )
}

fn resolver_id() -> InterfaceResolverId {
    InterfaceResolverId::new("test.schema").unwrap()
}

fn basis(revision: u64) -> CompilationBasis<GraphRevision> {
    CompilationBasis {
        graph_revision: GraphRevision::new(revision),
        registry_fingerprint: RegistryFingerprint::from_bytes([7; 32]),
        resource_versions: BTreeMap::new(),
        resource_observations: BTreeMap::new(),
    }
}

fn locator(field: &str) -> DynamicMemberLocator {
    DynamicMemberLocator::SchemaField {
        source: SchemaSourceIdentity("source".into()),
        field: SchemaFieldIdentity(field.into()),
    }
}

fn protocol() -> NodeProtocol {
    TestProtocolBuilder::new("yssbi.test.dynamic", "test")
        .style("test")
        .ports(vec![PortSpec {
            key: PortKey::new("fields").unwrap(),
            label_key: I18nKey::new("nodes.test.dynamic.fields").unwrap(),
            direction: PortDirection::Input,
            kind: PortKind::Data,
            value_type: TypeExpr::Unknown,
            instances: PortInstances::Derived {
                resolver: resolver_id(),
            },
            connections: ConnectionsPerPort::Single,
            input_binding: Some(InputBindingSpec {
                literal_policy: LiteralPolicy::Allowed,
                default_value: None,
            }),
            consumption: None,
            production: None,
            editor: PortEditorSpec::Default,
            schema: None,
        }])
        .execution(ExecutionSemantics {
            determinism: Determinism::Deterministic,
            purity: Purity::Pure,
            evaluation: EvaluationPolicy::DemandDriven,
            cache: CachePolicy::Disabled,
            effects: EffectSemantics::None,
            idempotent: false,
            retry: None,
        })
        .build()
}

fn document(binding: Option<(PortAddress, DynamicPortBinding)>) -> GraphDocument {
    GraphDocument {
        revision: GraphRevision::new(1),
        nodes: BTreeMap::new(),
        port_bindings: binding.into_iter().collect(),
        connections: BTreeMap::new(),
        input_states: BTreeMap::new(),
    }
}

#[derive(Clone)]
struct FixedResolver {
    members: Box<[super::dynamic_interface::InterfaceResolverMember]>,
}

impl InterfaceResolver for FixedResolver {
    fn resolve(
        &self,
        _request: InterfaceResolverRequest<'_>,
    ) -> Result<Box<[super::dynamic_interface::InterfaceResolverMember]>, InterfaceResolverError>
    {
        Ok(self.members.clone())
    }
}

fn member(
    basis: CompilationBasis<GraphRevision>,
    field: &str,
    label: &str,
    identity: SchemaFieldIdentityGuarantee,
) -> super::dynamic_interface::InterfaceResolverMember {
    super::dynamic_interface::InterfaceResolverMember {
        basis,
        locator: locator(field),
        label: label.into(),
        identity,
    }
}

fn resolver_set(
    members: Vec<super::dynamic_interface::InterfaceResolverMember>,
) -> InterfaceResolverSet {
    let mut set = InterfaceResolverSet::new();
    set.insert(
        resolver_id(),
        Arc::new(FixedResolver {
            members: members.into_boxed_slice(),
        }),
    )
    .unwrap();
    set
}

fn resolved_binding(field: &str) -> DynamicPortBinding {
    DynamicPortBinding::Resolved {
        origin: locator(field),
        order: OrderKey("a".into()),
    }
}

#[test]
fn exact_locator_materializes_existing_binding_and_exposes_unbound_members() {
    let current = basis(1);
    let bound = address(10);
    let graph = document(Some((bound.clone(), resolved_binding("customer_id"))));
    let set = resolver_set(vec![
        member(
            current.clone(),
            "customer_id",
            "Customer",
            SchemaFieldIdentityGuarantee::Stable,
        ),
        member(
            current.clone(),
            "unbound",
            "Unbound",
            SchemaFieldIdentityGuarantee::Stable,
        ),
    ]);

    let result = materialize_dynamic_interface(&current, node_id(), &protocol(), &graph, &set);

    assert!(result.diagnostics.is_empty());
    assert_eq!(result.interface.ports.len(), 2);
    assert!(
        result
            .interface
            .ports
            .iter()
            .all(|port| port.status == ResolvedPortStatus::Resolved)
    );
    assert_eq!(result.available_members.len(), 2);
    assert_eq!(
        result
            .available_members
            .iter()
            .find(|value| value.member().locator == locator("customer_id"))
            .unwrap()
            .bound_address(),
        Some(&bound)
    );
}

#[test]
fn matching_label_never_reconnects_a_different_locator() {
    let current = basis(1);
    let bound = address(10);
    let graph = document(Some((bound.clone(), resolved_binding("old_id"))));
    let set = resolver_set(vec![member(
        current.clone(),
        "new_id",
        "Same visible name",
        SchemaFieldIdentityGuarantee::Stable,
    )]);

    let result = materialize_dynamic_interface(&current, node_id(), &protocol(), &graph, &set);

    assert_eq!(
        result
            .interface
            .ports
            .iter()
            .find(|port| port.address == bound)
            .unwrap()
            .status,
        ResolvedPortStatus::Orphan
    );
    assert_eq!(result.available_members[0].bound_address(), None);
    assert!(matches!(
        result.projected_bindings.get(&bound),
        Some(ProjectedDynamicPortBinding::Orphan { origin, .. }) if origin == &locator("old_id")
    ));
}

#[test]
fn disappeared_member_becomes_orphan_with_last_known_metadata() {
    let current = basis(1);
    let bound = address(10);
    let graph = document(Some((bound.clone(), resolved_binding("gone"))));

    let result = materialize_dynamic_interface(
        &current,
        node_id(),
        &protocol(),
        &graph,
        &resolver_set(Vec::new()),
    );

    match result.projected_bindings.get(&bound).unwrap() {
        ProjectedDynamicPortBinding::Orphan { last_known, .. } => {
            assert_eq!(last_known.label, "schema:source/gone")
        }
        other => panic!("expected orphan, got {other:?}"),
    }
    assert_eq!(
        graph.port_bindings.get(&bound),
        Some(&resolved_binding("gone"))
    );
}

#[test]
fn existing_orphan_restores_only_by_exact_locator() {
    let current = basis(1);
    let bound = address(10);
    let graph = document(Some((
        bound.clone(),
        DynamicPortBinding::Orphan {
            origin: locator("restored"),
            order: OrderKey("a".into()),
            last_known: LastKnownPortMetadata {
                label: "Old label".into(),
            },
        },
    )));
    let set = resolver_set(vec![member(
        current.clone(),
        "restored",
        "New label",
        SchemaFieldIdentityGuarantee::SnapshotScoped,
    )]);

    let result = materialize_dynamic_interface(&current, node_id(), &protocol(), &graph, &set);

    assert_eq!(
        result.interface.ports[0].status,
        ResolvedPortStatus::Resolved
    );
    assert!(matches!(
        result.projected_bindings.get(&bound),
        Some(ProjectedDynamicPortBinding::Resolved { last_known, identity: SchemaFieldIdentityGuarantee::SnapshotScoped, .. })
            if last_known.label == "New label"
    ));
}

#[test]
fn rejects_members_from_a_different_basis() {
    let current = basis(1);
    let bound = address(10);
    let graph = document(Some((bound, resolved_binding("field"))));
    let set = resolver_set(vec![member(
        basis(2),
        "field",
        "Field",
        SchemaFieldIdentityGuarantee::Stable,
    )]);

    let result = materialize_dynamic_interface(&current, node_id(), &protocol(), &graph, &set);

    assert_eq!(result.interface.ports[0].status, ResolvedPortStatus::Orphan);
    assert!(
        result
            .diagnostics
            .iter()
            .any(|value| value.code.as_str() == "compiler.interface.basis_mismatch")
    );
}

#[test]
fn identity_none_diagnoses_persistent_connection_and_override() {
    let current = basis(1);
    let bound = address(10);
    let mut graph = document(Some((bound.clone(), resolved_binding("ephemeral"))));
    let connection_id = ConnectionId::from_uuid(Uuid::from_u128(20));
    graph.connections.insert(
        connection_id,
        DocumentConnection {
            id: connection_id,
            output: bound.clone(),
            input: PortAddress::declared(node_id(), PortKey::new("other").unwrap()),
            order: None,
        },
    );
    graph.input_states.insert(
        bound,
        InputState {
            literal_override: Some(serde_json::json!(1)),
        },
    );
    let set = resolver_set(vec![member(
        current.clone(),
        "ephemeral",
        "Ephemeral",
        SchemaFieldIdentityGuarantee::None,
    )]);

    let result = materialize_dynamic_interface(&current, node_id(), &protocol(), &graph, &set);
    let codes = result
        .diagnostics
        .iter()
        .map(|value| value.code.as_str())
        .collect::<Vec<_>>();

    assert!(codes.contains(&"compiler.interface.identity_none_connection"));
    assert!(codes.contains(&"compiler.interface.identity_none_override"));
    assert!(!SchemaFieldIdentityGuarantee::None.permits_persistent_state());
    assert!(SchemaFieldIdentityGuarantee::SnapshotScoped.permits_persistent_state());
}

#[test]
fn user_created_dynamic_interface_materializes_without_member_locator() {
    let current = basis(1);
    let bound = address(40);
    let graph = document(Some((
        bound.clone(),
        DynamicPortBinding::UserCreated {
            order: OrderKey("a".into()),
        },
    )));
    let mut user_protocol = protocol();
    user_protocol.interface.ports[0].instances = PortInstances::UserCreated {
        min: 0,
        max: Some(2),
    };

    let result = materialize_dynamic_interface(
        &current,
        node_id(),
        &user_protocol,
        &graph,
        &InterfaceResolverSet::new(),
    );

    assert!(result.diagnostics.is_empty());
    assert!(result.projected_bindings.is_empty());
    assert_eq!(result.interface.ports.len(), 1);
    assert_eq!(result.interface.ports[0].address, bound);
    assert_eq!(
        result.interface.ports[0].status,
        ResolvedPortStatus::Resolved
    );
}

#[test]
fn user_created_dynamic_interface_binding_mismatch_is_diagnosed() {
    let current = basis(1);
    let bound = address(41);
    let graph = document(Some((
        bound.clone(),
        DynamicPortBinding::UserCreated {
            order: OrderKey("a".into()),
        },
    )));

    let result = materialize_dynamic_interface(
        &current,
        node_id(),
        &protocol(),
        &graph,
        &resolver_set(Vec::new()),
    );

    assert_eq!(result.interface.ports[0].address, bound);
    assert_eq!(result.interface.ports[0].status, ResolvedPortStatus::Orphan);
    assert!(
        result.diagnostics.iter().any(|diagnostic| {
            diagnostic.code.as_str() == "compiler.port.binding_kind_mismatch"
        })
    );
}

#[test]
fn duplicate_interface_locator_emits_separate_port_and_locator_facts() {
    let current = basis(1);
    let duplicate = member(
        current.clone(),
        "customer_id",
        "Customer",
        SchemaFieldIdentityGuarantee::Stable,
    );
    let result = materialize_dynamic_interface(
        &current,
        node_id(),
        &protocol(),
        &document(None),
        &resolver_set(vec![duplicate.clone(), duplicate]),
    );

    let diagnostic = result
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "compiler.interface.duplicate_locator")
        .expect("duplicate locator diagnostic");
    assert_eq!(
        diagnostic.arguments,
        BTreeMap::from([
            (
                Box::from("locator"),
                Box::from(r#"{"kind":"schema_field","source":"source","field":"customer_id"}"#),
            ),
            (Box::from("port_key"), Box::from("fields")),
        ])
    );
}

#[test]
fn missing_and_duplicate_resolvers_are_reported() {
    let current = basis(1);
    let graph = document(None);
    let missing = materialize_dynamic_interface(
        &current,
        node_id(),
        &protocol(),
        &graph,
        &InterfaceResolverSet::new(),
    );
    assert_eq!(
        missing.diagnostics[0].code.as_str(),
        "compiler.interface.resolver_missing"
    );

    let mut set = InterfaceResolverSet::new();
    let resolver = Arc::new(FixedResolver {
        members: Box::new([]),
    });
    set.insert(resolver_id(), resolver.clone()).unwrap();
    assert!(set.insert(resolver_id(), resolver).is_err());
}
