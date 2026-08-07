use super::dynamic_interface::{
    InterfaceResolver, InterfaceResolverError, InterfaceResolverMember, InterfaceResolverRequest,
    InterfaceResolverSet, SchemaFieldIdentityGuarantee,
};
use super::*;
use crate::node_system::analysis::ResourceVersionSet;
use crate::node_system::document::{
    DocumentNode, DynamicMemberLocator, GraphDocument, GraphRevision, NodeId, NodePosition,
    PortInstanceId, SchemaFieldIdentity, SchemaSourceIdentity,
};
use crate::node_system::plan::{CompiledParameterHandle, KernelHandle};
use crate::node_system::protocol::*;
use crate::node_system::registry::{ProtocolFingerprint, RegistryFingerprint};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

struct Resources;

impl ResourceSnapshot for Resources {
    fn versions(&self) -> ResourceVersionSet {
        BTreeMap::new()
    }
}

struct Lowerer;

impl NodeLowerer for Lowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Ok(LoweredNode {
            kernel: LoweredKernel::Native(KernelHandle::new("test.dynamic").unwrap()),
            parameters: CompiledParameterHandle::new("test.dynamic.params").unwrap(),
        })
    }
}

struct Registry {
    fingerprint: RegistryFingerprint,
    protocol: NodeProtocol,
    implementation: NodeImplementation,
}

impl TypeEnvironment for Registry {
    fn concrete_implements(&self, _: &TypeId, _: &TypeClassId) -> Option<bool> {
        Some(false)
    }

    fn constructor_arity(&self, _: &TypeConstructorId) -> Option<usize> {
        None
    }
}

impl CompilerRegistry for Registry {
    fn fingerprint(&self) -> &RegistryFingerprint {
        &self.fingerprint
    }

    fn resolve(&self, node_type: &NodeTypeId) -> Option<RegistryNode<'_>> {
        (node_type == &self.protocol.type_id).then_some(RegistryNode {
            protocol: &self.protocol,
            protocol_fingerprint: ProtocolFingerprint::from_bytes([3; 32]),
            behavior: RegistryNodeBehavior::Leaf(&self.implementation),
        })
    }
}

#[derive(Clone)]
struct FixedResolver {
    members: Box<[InterfaceResolverMember]>,
}

impl InterfaceResolver for FixedResolver {
    fn resolve(
        &self,
        _: InterfaceResolverRequest<'_>,
    ) -> Result<Box<[InterfaceResolverMember]>, InterfaceResolverError> {
        Ok(self.members.clone())
    }
}

fn node_id() -> NodeId {
    NodeId::from_uuid(Uuid::from_u128(1))
}

fn key(value: &str) -> PortKey {
    PortKey::new(value).unwrap()
}

fn resolver_id() -> InterfaceResolverId {
    InterfaceResolverId::new("test.fields").unwrap()
}

fn locator(field: &str) -> DynamicMemberLocator {
    DynamicMemberLocator::SchemaField {
        source: SchemaSourceIdentity("source".into()),
        field: SchemaFieldIdentity(field.into()),
    }
}

fn protocol() -> NodeProtocol {
    NodeProtocol {
        type_id: NodeTypeId::new("yssbi.test.dynamic_pipeline").unwrap(),
        catalog: NodeCatalogProtocol {
            title_key: I18nKey::new("nodes.test.dynamic_pipeline.title").unwrap(),
            description_key: None,
            documentation_key: None,
            aliases_key: None,
            category_id: NodeCategoryId::new("test").unwrap(),
            icon_id: IconId::new("test").unwrap(),
            style_id: NodeStyleId::new("test").unwrap(),
            hidden: false,
        },
        interface: NodeInterfaceProtocol::new(
            vec![PortSpec {
                key: key("fields"),
                label_key: I18nKey::new("ports.fields.label").unwrap(),
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
            }],
            vec![],
            vec![],
        )
        .unwrap(),
        parameters: ParameterSchema::default(),
        execution: ExecutionSemantics {
            determinism: Determinism::Deterministic,
            purity: Purity::Pure,
            evaluation: EvaluationPolicy::DemandDriven,
            cache: CachePolicy::None,
            effects: EffectSemantics::None,
        },
        scope: NodeScope::Any,
        managed_role: None,
    }
}

fn document() -> GraphDocument {
    let node_id = node_id();
    GraphDocument {
        revision: GraphRevision::new(4),
        nodes: BTreeMap::from([(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: NodeTypeId::new("yssbi.test.dynamic_pipeline").unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: BTreeMap::new(),
                user_label: None,
            },
        )]),
        port_bindings: BTreeMap::new(),
        connections: BTreeMap::new(),
        input_states: BTreeMap::new(),
    }
}

fn registry() -> Registry {
    Registry {
        fingerprint: RegistryFingerprint::from_bytes([7; 32]),
        protocol: protocol(),
        implementation: NodeImplementation::new(Lowerer),
    }
}

fn member(
    basis: crate::node_system::analysis::CompilationBasis<GraphRevision>,
    field: &str,
    identity: SchemaFieldIdentityGuarantee,
) -> InterfaceResolverMember {
    InterfaceResolverMember {
        basis,
        locator: locator(field),
        label: field.into(),
        identity,
    }
}

fn interface_resolvers(members: Vec<InterfaceResolverMember>) -> InterfaceResolverSet {
    let mut resolvers = InterfaceResolverSet::new();
    resolvers
        .insert(
            resolver_id(),
            Arc::new(FixedResolver {
                members: members.into_boxed_slice(),
            }),
        )
        .unwrap();
    resolvers
}

fn expected_basis(
    registry: &Registry,
    document: &GraphDocument,
) -> crate::node_system::analysis::CompilationBasis<GraphRevision> {
    crate::node_system::analysis::CompilationBasis {
        graph_revision: document.revision,
        registry_fingerprint: registry.fingerprint.clone(),
        resource_versions: BTreeMap::new(),
        resource_observations: BTreeMap::new(),
    }
}

#[test]
fn full_compile_projects_unpersisted_derived_members_and_exposes_authorization_source() {
    let registry = registry();
    let document = document();
    let basis = expected_basis(&registry, &document);
    let resolvers = interface_resolvers(vec![member(
        basis.clone(),
        "customer_id",
        SchemaFieldIdentityGuarantee::Stable,
    )]);

    let compiler = GraphCompiler::with_interface_resolvers(&registry, &Resources, resolvers);
    let result = compiler.compile(&document);
    let repeated = compiler.compile(&document);

    assert!(document.port_bindings.is_empty());
    let interface = result
        .analysis
        .resolved_interfaces
        .iter()
        .find(|interface| interface.node_id == node_id())
        .unwrap();
    assert_eq!(interface.ports.len(), 1);
    let projected_address = interface.ports[0].address.clone();
    assert!(projected_address.is_instance());
    assert_eq!(
        repeated.analysis.resolved_interfaces[0].ports[0].address,
        projected_address
    );
    assert_eq!(result.interface_projection.basis, basis);
    let candidate = result
        .interface_projection
        .materialization_candidate(&projected_address)
        .expect("validated unbound member should be authorizable");
    assert_eq!(candidate.member().locator, locator("customer_id"));
    assert_eq!(candidate.template(), &key("fields"));
}

#[test]
fn full_compile_keeps_complete_projection_when_interface_diagnostics_block_lowering() {
    let registry = registry();
    let mut document = document();
    let basis = expected_basis(&registry, &document);
    let mut stale_basis = basis.clone();
    stale_basis.graph_revision = GraphRevision::new(3);
    let gone = crate::node_system::document::PortAddress::instance(
        node_id(),
        key("fields"),
        PortInstanceId::from_uuid(Uuid::from_u128(10)),
    );
    let ephemeral = crate::node_system::document::PortAddress::instance(
        node_id(),
        key("fields"),
        PortInstanceId::from_uuid(Uuid::from_u128(11)),
    );
    document.port_bindings.insert(
        gone.clone(),
        crate::node_system::document::DynamicPortBinding::Resolved {
            origin: locator("gone"),
            order: crate::node_system::document::OrderKey("a".into()),
        },
    );
    document.port_bindings.insert(
        ephemeral.clone(),
        crate::node_system::document::DynamicPortBinding::Resolved {
            origin: locator("ephemeral"),
            order: crate::node_system::document::OrderKey("b".into()),
        },
    );
    document.input_states.insert(
        ephemeral.clone(),
        crate::node_system::document::InputState {
            literal_override: Some(serde_json::json!(1)),
        },
    );
    let resolvers = interface_resolvers(vec![
        member(
            basis.clone(),
            "ephemeral",
            SchemaFieldIdentityGuarantee::None,
        ),
        member(basis, "available", SchemaFieldIdentityGuarantee::Stable),
        member(stale_basis, "stale", SchemaFieldIdentityGuarantee::Stable),
    ]);

    let result = GraphCompiler::with_interface_resolvers(&registry, &Resources, resolvers)
        .compile(&document);

    assert!(result.plan.is_none());
    assert!(
        result.execution_basis.is_none(),
        "orphan and stale-instance diagnostics must block demand specialization"
    );
    let interface = &result.analysis.resolved_interfaces[0];
    assert_eq!(interface.ports.len(), 3);
    assert!(interface.ports.iter().any(|port| {
        port.address == gone
            && port.status == crate::node_system::analysis::ResolvedPortStatus::Orphan
    }));
    assert!(interface.ports.iter().any(|port| {
        port.address == ephemeral
            && port.status == crate::node_system::analysis::ResolvedPortStatus::Resolved
    }));
    let codes = result
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<Vec<_>>();
    assert!(codes.contains(&"compiler.port.orphan"));
    assert!(codes.contains(&"compiler.interface.identity_none_override"));
    assert!(codes.contains(&"compiler.interface.basis_mismatch"));
    let projected = result.interface_projection.nodes.get(&node_id()).unwrap();
    assert!(matches!(
        projected.projected_bindings.get(&ephemeral),
        Some(ProjectedDynamicPortBinding::Resolved {
            identity: SchemaFieldIdentityGuarantee::None,
            ..
        })
    ));
    let available = projected
        .available_members
        .iter()
        .find(|member| member.member().locator == locator("available"))
        .unwrap();
    assert!(
        result
            .interface_projection
            .materialization_candidate(available.projection_address())
            .is_some(),
        "blocking diagnostics must not discard valid projected members"
    );
}
