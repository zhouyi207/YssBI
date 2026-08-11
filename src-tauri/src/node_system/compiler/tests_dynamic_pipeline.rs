use super::dynamic_interface::{
    InterfaceResolver, InterfaceResolverError, InterfaceResolverMember, InterfaceResolverRequest,
    InterfaceResolverSet, SchemaFieldIdentityGuarantee,
};
use super::*;
use crate::node_system::analysis::ResourceVersionSet;
use crate::node_system::document::{
    ConnectionId, DocumentConnection, DocumentNode, DynamicMemberLocator, GraphDocument,
    GraphRevision, NodeId, NodePosition, PortAddress, PortInstanceId, SchemaFieldIdentity,
    SchemaSourceIdentity,
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
    protocols: Vec<NodeProtocol>,
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
        self.protocols
            .iter()
            .find(|protocol| node_type == &protocol.type_id)
            .map(|protocol| RegistryNode {
                protocol,
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
            cache: CachePolicy::Disabled,
            effects: EffectSemantics::None,
            idempotent: false,
            retry: None,
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
        protocols: vec![protocol()],
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

struct FixedSourceSchemaResolver;

impl SchemaResolver for FixedSourceSchemaResolver {
    fn resolve(
        &self,
        _: &mut SchemaResolutionContext<'_, '_>,
    ) -> Result<SchemaFact, SchemaResolutionError> {
        Ok(SchemaFact::new(
            SchemaExpr::Derived {
                resolver: SchemaResolverId::new("test.fixed_source_schema").unwrap(),
                dependencies: vec![],
            },
            [SchemaColumnRef("amount".into())],
        ))
    }
}

struct SchemaDependentResolver;

impl InterfaceResolver for SchemaDependentResolver {
    fn schema_dependencies(&self) -> &[PortKey] {
        static DEPENDENCIES: std::sync::OnceLock<Box<[PortKey]>> = std::sync::OnceLock::new();
        DEPENDENCIES.get_or_init(|| vec![key("dataframe")].into_boxed_slice())
    }

    fn resolve(
        &self,
        request: InterfaceResolverRequest<'_>,
    ) -> Result<Box<[InterfaceResolverMember]>, InterfaceResolverError> {
        let address = PortAddress::declared(request.node_id, key("dataframe"));
        let schema = request.resolved_schemas.get(&address).ok_or_else(|| {
            InterfaceResolverError::new("staged schema dependency was not supplied")
        })?;
        Ok(schema
            .fields
            .iter()
            .map(|field| InterfaceResolverMember {
                basis: request.basis.clone(),
                locator: locator(field.name.0.as_ref()),
                label: field.name.0.to_string(),
                identity: SchemaFieldIdentityGuarantee::SnapshotScoped,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice())
    }
}

fn staged_port(
    name: &str,
    direction: PortDirection,
    instances: PortInstances,
    schema: Option<SchemaExpr>,
) -> PortSpec {
    PortSpec {
        key: key(name),
        label_key: I18nKey::new(format!("ports.{name}.label")).unwrap(),
        direction,
        kind: PortKind::Data,
        value_type: TypeExpr::Unknown,
        instances,
        connections: ConnectionsPerPort::Single,
        input_binding: (direction == PortDirection::Input).then_some(InputBindingSpec {
            literal_policy: LiteralPolicy::Forbidden,
            default_value: None,
        }),
        consumption: None,
        production: None,
        editor: PortEditorSpec::Default,
        schema,
    }
}

fn staged_protocol(name: &str, ports: Vec<PortSpec>) -> NodeProtocol {
    let mut protocol = protocol();
    protocol.type_id = NodeTypeId::new(format!("yssbi.test.{name}")).unwrap();
    protocol.interface = NodeInterfaceProtocol::new(ports, vec![], vec![]).unwrap();
    protocol
}

fn staged_registry() -> Registry {
    let schema_resolver = SchemaResolverId::new("test.fixed_source_schema").unwrap();
    let source = staged_protocol(
        "schema_source",
        vec![staged_port(
            "dataframe",
            PortDirection::Output,
            PortInstances::Declared,
            Some(SchemaExpr::Derived {
                resolver: schema_resolver,
                dependencies: vec![],
            }),
        )],
    );
    let consumer = staged_protocol(
        "schema_consumer",
        vec![
            staged_port(
                "dataframe",
                PortDirection::Input,
                PortInstances::Declared,
                None,
            ),
            staged_port(
                "columns",
                PortDirection::Output,
                PortInstances::Derived {
                    resolver: InterfaceResolverId::new("test.schema_dependent").unwrap(),
                },
                None,
            ),
        ],
    );
    Registry {
        fingerprint: RegistryFingerprint::from_bytes([7; 32]),
        protocols: vec![source, consumer],
        implementation: NodeImplementation::new(Lowerer),
    }
}

fn staged_document(connected: bool) -> GraphDocument {
    let source_id = NodeId::from_uuid(Uuid::from_u128(10));
    let consumer_id = NodeId::from_uuid(Uuid::from_u128(11));
    let mut document = GraphDocument {
        revision: GraphRevision::new(5),
        nodes: BTreeMap::from([
            (
                source_id,
                DocumentNode {
                    id: source_id,
                    node_type: NodeTypeId::new("yssbi.test.schema_source").unwrap(),
                    position: NodePosition { x: 0.0, y: 0.0 },
                    parameters: BTreeMap::new(),
                    user_label: None,
                },
            ),
            (
                consumer_id,
                DocumentNode {
                    id: consumer_id,
                    node_type: NodeTypeId::new("yssbi.test.schema_consumer").unwrap(),
                    position: NodePosition { x: 1.0, y: 0.0 },
                    parameters: BTreeMap::new(),
                    user_label: None,
                },
            ),
        ]),
        port_bindings: BTreeMap::new(),
        connections: BTreeMap::new(),
        input_states: BTreeMap::new(),
    };
    if connected {
        let connection_id = ConnectionId::from_uuid(Uuid::from_u128(12));
        document.connections.insert(
            connection_id,
            DocumentConnection {
                id: connection_id,
                output: PortAddress::declared(source_id, key("dataframe")),
                input: PortAddress::declared(consumer_id, key("dataframe")),
                order: None,
            },
        );
    }
    document
}

fn staged_resolvers() -> (SchemaResolverSet, InterfaceResolverSet) {
    let mut schema_resolvers = SchemaResolverSet::new();
    schema_resolvers.insert(
        SchemaResolverId::new("test.fixed_source_schema").unwrap(),
        FixedSourceSchemaResolver,
    );
    let mut interface_resolvers = InterfaceResolverSet::new();
    interface_resolvers
        .insert(
            InterfaceResolverId::new("test.schema_dependent").unwrap(),
            Arc::new(SchemaDependentResolver),
        )
        .unwrap();
    (schema_resolvers, interface_resolvers)
}

fn derived_output_labels(result: &CompileResult) -> Vec<String> {
    result
        .interface_projection
        .nodes
        .values()
        .flat_map(|projection| projection.available_members.iter())
        .filter(|member| member.template() == &key("columns"))
        .map(|member| member.member().label.clone())
        .collect()
}

#[test]
fn schema_dependent_interface_resolves_after_preliminary_schema_analysis() {
    let registry = staged_registry();
    let document = staged_document(true);
    let (schema_resolvers, interface_resolvers) = staged_resolvers();

    let result =
        GraphCompiler::with_resolvers(&registry, &Resources, schema_resolvers, interface_resolvers)
            .compile(&document);
    let codes = result
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<Vec<_>>();

    assert!(!codes.contains(&"compiler.interface.resolver_missing"));
    assert!(!codes.contains(&"compiler.interface.resolver_failed"));
    assert_eq!(derived_output_labels(&result), vec!["amount"]);
}

#[test]
fn unresolved_schema_dependency_defers_without_interface_resolver_diagnostic() {
    let registry = staged_registry();
    let document = staged_document(false);
    let (schema_resolvers, interface_resolvers) = staged_resolvers();

    let result =
        GraphCompiler::with_resolvers(&registry, &Resources, schema_resolvers, interface_resolvers)
            .compile(&document);
    let codes = result
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<Vec<_>>();

    assert!(codes.contains(&"compiler.input.unbound"));
    assert!(!codes.contains(&"compiler.interface.resolver_missing"));
    assert!(!codes.contains(&"compiler.interface.resolver_failed"));
    assert!(derived_output_labels(&result).is_empty());
}

#[test]
fn unregistered_schema_dependent_interface_resolver_is_still_missing() {
    let registry = staged_registry();
    let document = staged_document(true);
    let (schema_resolvers, _) = staged_resolvers();

    let result = GraphCompiler::with_resolvers(
        &registry,
        &Resources,
        schema_resolvers,
        InterfaceResolverSet::new(),
    )
    .compile(&document);
    let codes = result
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<Vec<_>>();

    assert!(codes.contains(&"compiler.interface.resolver_missing"));
}

struct DatabaseResources {
    versions: ResourceVersionSet,
    columns: Vec<crate::schema::ColumnInfoDTO>,
}

impl ResourceSnapshot for DatabaseResources {
    fn versions(&self) -> ResourceVersionSet {
        self.versions.clone()
    }

    fn database_schema(&self, id: &str) -> Option<&[crate::schema::ColumnInfoDTO]> {
        (id == "main").then_some(self.columns.as_slice())
    }
}

struct DatabaseSourceSchemaResolver;

impl SchemaResolver for DatabaseSourceSchemaResolver {
    fn resolve(
        &self,
        context: &mut SchemaResolutionContext<'_, '_>,
    ) -> Result<SchemaFact, SchemaResolutionError> {
        let database = context
            .resources
            .as_deref_mut()
            .ok_or_else(|| SchemaResolutionError::new("missing analysis resolver"))?
            .resolve_database("main")
            .map_err(|error| SchemaResolutionError::from_resource(&error))?;
        Ok(SchemaFact::new(
            SchemaExpr::Derived {
                resolver: SchemaResolverId::new("test.fixed_source_schema").unwrap(),
                dependencies: vec![],
            },
            database.value.iter().map(|column| SchemaField {
                name: SchemaColumnRef(column.name.clone().into()),
                scalar_type: RelationalScalarType::from_database_dtype(&column.dtype),
                lineage: None,
            }),
        ))
    }
}

#[test]
fn staged_schema_resource_reads_remain_set_based() {
    let registry = staged_registry();
    let document = staged_document(true);
    let resources = DatabaseResources {
        versions: BTreeMap::from([(
            crate::node_system::analysis::ResourceKey::new("databases/main"),
            crate::node_system::analysis::ResourceVersion::new("main-v1"),
        )]),
        columns: vec![crate::schema::ColumnInfoDTO {
            name: "amount".into(),
            dtype: "BIGINT".into(),
        }],
    };
    let mut schema_resolvers = SchemaResolverSet::new();
    schema_resolvers.insert(
        SchemaResolverId::new("test.fixed_source_schema").unwrap(),
        DatabaseSourceSchemaResolver,
    );
    let (_, interface_resolvers) = staged_resolvers();

    let result =
        GraphCompiler::with_resolvers(&registry, &resources, schema_resolvers, interface_resolvers)
            .compile(&document);

    assert_eq!(
        result
            .analysis
            .basis
            .resource_versions
            .keys()
            .map(crate::node_system::analysis::ResourceKey::as_str)
            .collect::<Vec<_>>(),
        vec!["databases/main"]
    );
    assert!(result.analysis.basis.resource_observations.is_empty());
    assert_eq!(derived_output_labels(&result), vec!["amount"]);
}
