use super::*;
use crate::node_system::ProjectSessionId;
use crate::node_system::analysis::{
    CompileId, DiagnosticLocation, DiagnosticSeverity, ResourceKey, ResourceVersion,
};
use crate::node_system::document::{
    ConnectionId, DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
    FunctionDocument, FunctionParameter, FunctionParameterId, FunctionSignature, GraphDocument,
    GraphResourcePath, InputState, NodeId, NodePosition, OrderKey, PortAddress, PortInstanceId,
    PortRef,
};
use crate::node_system::plan::{
    BranchResultBinding, CallResultBinding, CompiledParameterHandle, CompiledResourceRequirement,
    ControlStep, ExecutionDemand, ExecutionPlan, ExecutionSemanticsVersion, FunctionPlanAbi,
    FunctionPlanHandle, GraphOutputRef, KernelHandle, LoopCarriedBinding, PlanResult,
    PlanValidationError, PlanValueSource, PlannedAdapter, PlannedKernel, PlannedOperation,
    PlannedPublication, PlannedRetry, RelationalBackendId, RelationalExpression,
    RelationalFragmentId, RelationalLiteral, RelationalOperator, RelationalOperatorIndex,
    RelationalPushdownHint, ResourceAccess, ResourceId, ResourceKind, ResultPresentation,
    ResultReportKind, StructuredControlRegion, ValueDependency, ValueRef, WorkloadClass,
};
use crate::node_system::protocol::*;
use crate::node_system::registry::{
    CategoryRegistration, I18nManifest, NodeRegistry, NodeRegistryBuilder, ProtocolFingerprint,
    ProviderRegistration, RegisteredNode, RegistryFingerprint, StructuralNodeRole,
    TypeRegistration,
};
use crate::node_system::testing::TestProtocolBuilder;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
use uuid::Uuid;

struct ConstantLowerer;
impl NodeLowerer for ConstantLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Ok(LoweredNode {
            kernel: LoweredKernel::Native(KernelHandle::new("test.constant").unwrap()),
            parameters: CompiledParameterHandle::new("test.params").unwrap(),
        })
    }
}

struct CountingLowerer(Arc<AtomicUsize>);

impl NodeLowerer for CountingLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        self.0.fetch_add(1, AtomicOrdering::Relaxed);
        Ok(LoweredNode {
            kernel: LoweredKernel::Native(KernelHandle::new("test.counting").unwrap()),
            parameters: CompiledParameterHandle::new("test.counting.params").unwrap(),
        })
    }
}

fn assert_analysis_blocks_before_lowering(result: &CompileResult, calls: &AtomicUsize) {
    assert!(matches!(
        result.outcome,
        CompilationOutcome::AnalysisBlocked
    ));
    assert!(result.semantic.is_none());
    assert!(result.plan.is_none());
    assert_eq!(calls.load(AtomicOrdering::Relaxed), 0);
}

struct Resources;
impl ResourceSnapshot for Resources {
    fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
        BTreeMap::from([(ResourceKey::new("resource.test"), ResourceVersion::new("1"))])
    }
}

fn node_id(value: u128) -> NodeId {
    NodeId::from_uuid(Uuid::from_u128(value))
}
fn key(value: &str) -> PortKey {
    PortKey::new(value).unwrap()
}

fn protocol() -> NodeProtocol {
    TestProtocolBuilder::new("yssbi.test.constant", "test")
        .style("test")
        .ports(vec![PortSpec {
            key: PortKey::new("value").unwrap(),
            label_key: I18nKey::new("nodes.test.constant.value").unwrap(),
            direction: PortDirection::Output,
            kind: PortKind::Data,
            value_type: TypeExpr::Concrete(TypeId::new("core.int64").unwrap()),
            instances: PortInstances::Declared,
            connections: ConnectionsPerPort::Multiple {
                max: None,
                ordered: false,
            },
            input_binding: None,
            consumption: None,
            production: None,
            editor: PortEditorSpec::Default,
            schema: None,
        }])
        .build()
}

struct CorruptCompilerRegistrySnapshot {
    frozen: NodeRegistry,
}

impl TypeEnvironment for CorruptCompilerRegistrySnapshot {
    fn concrete_implements(&self, value_type: &TypeId, class: &TypeClassId) -> Option<bool> {
        self.frozen
            .types()
            .get(value_type)
            .map(|registration| registration.classes.contains(class))
    }

    fn constructor_arity(&self, constructor: &TypeConstructorId) -> Option<usize> {
        self.frozen
            .types()
            .constructor(constructor)
            .map(|registration| registration.arity as usize)
    }
}

impl CompilerRegistry for CorruptCompilerRegistrySnapshot {
    fn fingerprint(&self) -> &RegistryFingerprint {
        self.frozen.fingerprint()
    }

    fn resolve(&self, node_type: &NodeTypeId) -> Option<RegistryNode<'_>> {
        let registered = self.frozen.get(node_type)?;
        Some(RegistryNode {
            protocol: registered.protocol(),
            protocol_fingerprint: self
                .frozen
                .catalog_manifest()
                .node_protocols
                .get(node_type)?
                .clone(),
            behavior: RegistryNodeBehavior::ProtocolOnly,
        })
    }
}

struct AugmentedCompilerRegistry<'a> {
    frozen: &'a NodeRegistry,
    protocol: NodeProtocol,
    implementation: NodeImplementation,
}

impl TypeEnvironment for AugmentedCompilerRegistry<'_> {
    fn concrete_implements(&self, value_type: &TypeId, class: &TypeClassId) -> Option<bool> {
        self.frozen.concrete_implements(value_type, class)
    }

    fn constructor_arity(&self, constructor: &TypeConstructorId) -> Option<usize> {
        self.frozen.constructor_arity(constructor)
    }
}

impl CompilerRegistry for AugmentedCompilerRegistry<'_> {
    fn fingerprint(&self) -> &RegistryFingerprint {
        self.frozen.fingerprint()
    }

    fn resolve(&self, node_type: &NodeTypeId) -> Option<RegistryNode<'_>> {
        if node_type == &self.protocol.type_id {
            return Some(RegistryNode {
                protocol: &self.protocol,
                protocol_fingerprint: ProtocolFingerprint::from_bytes([0x8; 32]),
                behavior: RegistryNodeBehavior::Leaf(&self.implementation),
            });
        }
        self.frozen.resolve(node_type)
    }

    fn validate_nominal_parameter(
        &self,
        type_id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<(), String>> {
        self.frozen.validate_nominal_parameter(type_id, value)
    }
}

fn registry() -> NodeRegistry {
    let mut provider = ProviderRegistration::new(ProviderId::new("yssbi").unwrap());
    provider.types = vec![TypeRegistration {
        id: TypeId::new("core.int64").unwrap(),
        title_key: I18nKey::new("types.int64.title").unwrap(),
        classes: BTreeSet::new(),
    }]
    .into_boxed_slice();
    provider.categories = vec![CategoryRegistration {
        id: NodeCategoryId::new("test").unwrap(),
        title_key: I18nKey::new("categories.test.title").unwrap(),
        parent: None,
        order: 0,
    }]
    .into_boxed_slice();
    provider.i18n = I18nManifest {
        keys: BTreeSet::from([
            I18nKey::new("types.int64.title").unwrap(),
            I18nKey::new("categories.test.title").unwrap(),
            I18nKey::new("nodes.test.constant.title").unwrap(),
            I18nKey::new("nodes.test.constant.value").unwrap(),
        ]),
    };
    provider.nodes = vec![RegisteredNode::leaf(
        Arc::new(protocol()),
        Arc::new(NodeImplementation::new(ConstantLowerer)),
    )]
    .into_boxed_slice();
    let mut builder = NodeRegistryBuilder::new();
    builder.register_provider(provider).unwrap();
    builder.freeze().unwrap()
}
fn document(node_type: NodeTypeId) -> GraphDocument {
    let id = node_id(1);
    GraphDocument {
        revision: crate::node_system::document::GraphRevision::new(7),
        nodes: BTreeMap::from([(
            id,
            DocumentNode {
                id,
                node_type,
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

struct TestRegistry {
    fingerprint: RegistryFingerprint,
    nodes: BTreeMap<NodeTypeId, (NodeProtocol, NodeImplementation)>,
    structural_roles: BTreeMap<NodeTypeId, StructuralNodeRole>,
    type_classes: BTreeMap<TypeId, BTreeSet<TypeClassId>>,
    constructor_arities: BTreeMap<TypeConstructorId, usize>,
    constructor_classes: BTreeMap<TypeConstructorId, BTreeSet<TypeClassId>>,
    nominal_registry: Option<Arc<NodeRegistry>>,
}

impl TestRegistry {
    fn new(protocols: Vec<NodeProtocol>) -> Self {
        let nodes = protocols
            .into_iter()
            .enumerate()
            .map(|(index, protocol)| {
                (
                    protocol.type_id.clone(),
                    (protocol, NodeImplementation::new(TestLowerer(index as u32))),
                )
            })
            .collect();
        Self {
            fingerprint: RegistryFingerprint::from_bytes([9; 32]),
            nodes,
            structural_roles: BTreeMap::new(),
            type_classes: BTreeMap::new(),
            constructor_arities: BTreeMap::new(),
            constructor_classes: BTreeMap::new(),
            nominal_registry: None,
        }
    }

    fn with_type_class(mut self, value_type: TypeId, class: TypeClassId) -> Self {
        self.type_classes
            .entry(value_type)
            .or_default()
            .insert(class);
        self
    }

    fn with_constructor(
        mut self,
        constructor: TypeConstructorId,
        arity: usize,
        classes: impl IntoIterator<Item = TypeClassId>,
    ) -> Self {
        self.constructor_arities.insert(constructor.clone(), arity);
        self.constructor_classes
            .insert(constructor, classes.into_iter().collect());
        self
    }

    fn structural(mut self, node_type: &NodeTypeId, role: StructuralNodeRole) -> Self {
        self.structural_roles.insert(node_type.clone(), role);
        self
    }

    fn with_lowerer(mut self, node_type: &NodeTypeId, lowerer: impl NodeLowerer + 'static) -> Self {
        self.nodes.get_mut(node_type).expect("test node protocol").1 =
            NodeImplementation::new(lowerer);
        self
    }

    fn with_nominal_registry(mut self, registry: Arc<NodeRegistry>) -> Self {
        self.nominal_registry = Some(registry);
        self
    }
}

impl TypeEnvironment for TestRegistry {
    fn concrete_implements(&self, value_type: &TypeId, class: &TypeClassId) -> Option<bool> {
        Some(
            self.type_classes
                .get(value_type)
                .is_some_and(|classes| classes.contains(class)),
        )
    }

    fn constructor_arity(&self, constructor: &TypeConstructorId) -> Option<usize> {
        self.constructor_arities.get(constructor).copied()
    }

    fn applied_implements(
        &self,
        constructor: &TypeConstructorId,
        class: &TypeClassId,
    ) -> Option<bool> {
        Some(
            self.constructor_classes
                .get(constructor)
                .is_some_and(|classes| classes.contains(class)),
        )
    }
}

impl CompilerRegistry for TestRegistry {
    fn fingerprint(&self) -> &RegistryFingerprint {
        &self.fingerprint
    }

    fn resolve(&self, node_type: &NodeTypeId) -> Option<RegistryNode<'_>> {
        let (protocol, implementation) = self.nodes.get(node_type)?;
        let behavior = self
            .structural_roles
            .get(node_type)
            .copied()
            .map(RegistryNodeBehavior::Structural)
            .unwrap_or(RegistryNodeBehavior::Leaf(implementation));
        Some(RegistryNode {
            protocol,
            protocol_fingerprint: ProtocolFingerprint::from_bytes(
                [node_type.as_str().len() as u8; 32],
            ),
            behavior,
        })
    }

    fn validate_nominal_parameter(
        &self,
        type_id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<(), String>> {
        self.nominal_registry
            .as_ref()?
            .validate_nominal_parameter(type_id, value)
    }

    fn prepare_nominal_parameter(
        &self,
        type_id: &TypeId,
        value: &serde_json::Value,
    ) -> Option<Result<crate::node_system::registry::PreparedNominalValue, String>> {
        self.nominal_registry
            .as_ref()?
            .prepare_nominal_parameter(type_id, value)
    }
}

struct TestLowerer(u32);
impl NodeLowerer for TestLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Ok(LoweredNode {
            kernel: LoweredKernel::Native(
                KernelHandle::new(format!("test.kernel.{}", self.0)).unwrap(),
            ),
            parameters: CompiledParameterHandle::new(format!("test.params.{}", self.0)).unwrap(),
        })
    }
}

fn type_id(value: &str) -> TypeId {
    TypeId::new(value).unwrap()
}

fn concrete(value: &str) -> TypeExpr {
    TypeExpr::Concrete(type_id(value))
}

fn data_port(
    name: &str,
    direction: PortDirection,
    value_type: TypeExpr,
    schema: Option<SchemaExpr>,
) -> PortSpec {
    PortSpec {
        key: key(name),
        label_key: I18nKey::new(format!("ports.{name}.label")).unwrap(),
        direction,
        kind: PortKind::Data,
        value_type,
        instances: PortInstances::Declared,
        connections: if direction == PortDirection::Input {
            ConnectionsPerPort::Single
        } else {
            ConnectionsPerPort::Multiple {
                max: None,
                ordered: false,
            }
        },
        input_binding: (direction == PortDirection::Input).then_some(InputBindingSpec {
            literal_policy: LiteralPolicy::Allowed,
            default_value: None,
        }),
        consumption: None,
        production: None,
        editor: PortEditorSpec::Default,
        schema,
    }
}

fn effect_port(name: &str, direction: PortDirection) -> PortSpec {
    PortSpec {
        key: key(name),
        label_key: I18nKey::new(format!("ports.{name}.label")).unwrap(),
        direction,
        kind: PortKind::Effect,
        value_type: TypeExpr::Unknown,
        instances: PortInstances::Declared,
        connections: if direction == PortDirection::Input {
            ConnectionsPerPort::Single
        } else {
            ConnectionsPerPort::Multiple {
                max: None,
                ordered: false,
            }
        },
        input_binding: None,
        consumption: None,
        production: None,
        editor: PortEditorSpec::Default,
        schema: None,
    }
}

fn test_protocol(
    name: &str,
    mut ports: Vec<PortSpec>,
    type_parameters: Vec<TypeParameterId>,
    constraints: Vec<TypeConstraint>,
) -> NodeProtocol {
    for port in &mut ports {
        if port.kind == PortKind::Data && port.value_type == TypeExpr::Unknown {
            port.value_type = TypeExpr::Concrete(TypeId::new("core.object").unwrap());
        }
    }
    NodeProtocol {
        type_id: NodeTypeId::new(format!("yssbi.test.{name}")).unwrap(),
        catalog: NodeCatalogProtocol {
            title_key: I18nKey::new(format!("nodes.test.{name}.title")).unwrap(),
            description_key: None,
            documentation_key: None,
            aliases_key: None,
            category_id: NodeCategoryId::new("test").unwrap(),
            icon_id: IconId::new("test").unwrap(),
            style_id: NodeStyleId::new("test").unwrap(),
            hidden: false,
        },
        interface: NodeInterfaceProtocol::new(ports, type_parameters, constraints).unwrap(),
        parameters: ParameterSchema::default(),
        instance_display: NodeInstanceDisplaySpec::Static,
        execution: ExecutionSemantics {
            determinism: Determinism::Deterministic,
            purity: Purity::Pure,
            evaluation: EvaluationPolicy::DemandDriven,
            cache: CachePolicy::PerRun,
            effects: EffectSemantics::None,
            idempotent: false,
            retry: None,
        },
        scope: NodeScope::Any,
        managed_role: None,
    }
}

fn graph_with_nodes(nodes: &[(u128, &str)]) -> GraphDocument {
    graph_with_node_types(
        nodes
            .iter()
            .map(|(node, node_type)| (*node, format!("yssbi.test.{node_type}"))),
    )
}

fn builtin_graph_with_nodes(nodes: &[(u128, &str)]) -> GraphDocument {
    graph_with_node_types(
        nodes
            .iter()
            .map(|(node, node_type)| (*node, (*node_type).to_owned())),
    )
}

fn graph_with_node_types(nodes: impl IntoIterator<Item = (u128, String)>) -> GraphDocument {
    GraphDocument {
        revision: crate::node_system::document::GraphRevision::new(11),
        nodes: nodes
            .into_iter()
            .map(|(id, node_type)| {
                let id = node_id(id);
                (
                    id,
                    DocumentNode {
                        id,
                        node_type: NodeTypeId::new(node_type).unwrap(),
                        position: NodePosition { x: 0.0, y: 0.0 },
                        parameters: BTreeMap::new(),
                        user_label: None,
                    },
                )
            })
            .collect(),
        port_bindings: BTreeMap::new(),
        connections: BTreeMap::new(),
        input_states: BTreeMap::new(),
    }
}

fn connect(
    graph: &mut GraphDocument,
    id: u128,
    source_node: u128,
    source_port: &str,
    target_node: u128,
    target_port: &str,
) {
    connect_addresses(
        graph,
        id,
        PortAddress::declared(node_id(source_node), key(source_port)),
        PortAddress::declared(node_id(target_node), key(target_port)),
    );
}

fn connect_addresses(graph: &mut GraphDocument, id: u128, output: PortAddress, input: PortAddress) {
    let id = crate::node_system::document::ConnectionId::from_uuid(Uuid::from_u128(id));
    graph.connections.insert(
        id,
        DocumentConnection {
            id,
            output,
            input,
            order: None,
        },
    );
}

fn bind_resolved_function_port(
    graph: &mut GraphDocument,
    node: u128,
    template: &str,
    instance: u128,
    order: &str,
    function: &GraphResourcePath,
    parameter: &FunctionParameterId,
) -> PortAddress {
    let address = PortAddress::instance(
        node_id(node),
        key(template),
        PortInstanceId::from_uuid(Uuid::from_u128(instance)),
    );
    graph.port_bindings.insert(
        address.clone(),
        DynamicPortBinding::Resolved {
            origin: DynamicMemberLocator::FunctionParameter {
                function: function.clone(),
                parameter: parameter.clone(),
            },
            order: OrderKey(order.into()),
            last_known: crate::node_system::document::LastKnownPortMetadata::default(),
        },
    );
    address
}

fn bind_member_port(
    graph: &mut GraphDocument,
    node: u128,
    template: &str,
    instance: u128,
    order: &str,
) -> PortAddress {
    let address = PortAddress::instance(
        node_id(node),
        key(template),
        PortInstanceId::from_uuid(Uuid::from_u128(instance)),
    );
    graph.port_bindings.insert(
        address.clone(),
        DynamicPortBinding::UserCreated {
            order: OrderKey(order.into()),
        },
    );
    address
}

struct FailingLowerer;
struct CancelledLowerer;

impl NodeLowerer for CancelledLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Err(LoweringError::Cancelled(CompileCancelled))
    }
}

impl NodeLowerer for FailingLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Err(LoweringError::internal(
            LoweringInvariant::InvalidPreparedConfiguration,
        ))
    }
}

struct FragmentLowerer {
    fragment: LoweredKernel,
}

impl NodeLowerer for FragmentLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Ok(LoweredNode {
            kernel: self.fragment.clone(),
            parameters: CompiledParameterHandle::new("test.fragment.params").unwrap(),
        })
    }
}

fn kernel_fragment(effect: EffectSemantics, mut metadata: FragmentMetadata) -> LoweredKernel {
    metadata.effect = effect;
    LoweredKernel::Kernel(KernelFragment {
        kernel: KernelHandle::new("test.fragment.kernel").unwrap(),
        metadata,
    })
}

fn demand_output(graph_path: &str, node: u128, port: &str) -> GraphOutputRef {
    GraphOutputRef {
        graph_path: GraphResourcePath(graph_path.into()),
        port: PortAddress::declared(node_id(node), key(port)),
    }
}

fn chain_protocol(name: &str) -> NodeProtocol {
    test_protocol(
        name,
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port("out", PortDirection::Output, TypeExpr::Unknown, None),
        ],
        vec![],
        vec![],
    )
}

fn demand_fixture() -> (TestRegistry, GraphDocument) {
    let mut entry = test_protocol(
        "demand_event_begin",
        vec![control_port("then", PortDirection::Output)],
        vec![],
        vec![],
    );
    entry.managed_role = Some(ManagedNodeRole::EventBegin);
    let entry_type = entry.type_id.clone();
    let source_a = test_protocol(
        "demand_source_a",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let sink_a = chain_protocol("demand_sink_a");
    let source_b = test_protocol(
        "demand_source_b",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let sink_b = chain_protocol("demand_sink_b");
    let sink_a_type = sink_a.type_id.clone();
    let sink_b_type = sink_b.type_id.clone();
    let registry = TestRegistry::new(vec![entry, source_a, sink_a, source_b, sink_b])
        .structural(&entry_type, StructuralNodeRole::EventBegin)
        .with_lowerer(
            &sink_a_type,
            FragmentLowerer {
                fragment: kernel_fragment(
                    EffectSemantics::None,
                    FragmentMetadata {
                        effect: EffectSemantics::None,
                        resources: Box::new([CompiledResourceRequirement {
                            resource: ResourceId::new("database.chain-a").unwrap(),
                            kind: ResourceKind::DatabaseConnection,
                            access: ResourceAccess::Shared,
                            optional: false,
                        }]),
                        results: Box::new([FragmentResult {
                            name: "chain-a".into(),
                            output: PortAddress::declared(node_id(2), key("out")),
                        }]),
                    },
                ),
            },
        )
        .with_lowerer(
            &sink_b_type,
            FragmentLowerer {
                fragment: kernel_fragment(
                    EffectSemantics::None,
                    FragmentMetadata {
                        effect: EffectSemantics::None,
                        resources: Box::new([CompiledResourceRequirement {
                            resource: ResourceId::new("database.chain-b").unwrap(),
                            kind: ResourceKind::DatabaseConnection,
                            access: ResourceAccess::Shared,
                            optional: false,
                        }]),
                        results: Box::new([FragmentResult {
                            name: "chain-b".into(),
                            output: PortAddress::declared(node_id(4), key("out")),
                        }]),
                    },
                ),
            },
        );
    let mut graph = graph_with_nodes(&[
        (100, "demand_event_begin"),
        (1, "demand_source_a"),
        (2, "demand_sink_a"),
        (3, "demand_source_b"),
        (4, "demand_sink_b"),
    ]);
    connect(&mut graph, 10, 1, "out", 2, "in");
    connect(&mut graph, 11, 3, "out", 4, "in");
    (registry, graph)
}

fn compiled_demand_basis() -> ExecutionPlanBasis {
    let (registry, graph) = demand_fixture();
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/main".into()), &graph);
    compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap()
        .execution_basis
        .expect("demand fixture has a basis")
}

fn append_control_region(basis: &mut ExecutionPlanBasis, region: StructuredControlRegion) {
    let StructuredControlRegion::Sequence(steps) = &mut basis.root_region else {
        panic!("demand fixture root is a sequence")
    };
    let mut projected = steps.to_vec();
    projected.push(ControlStep::Region(Box::new(region)));
    *steps = projected.into_boxed_slice();
}

fn declare_control_value(basis: &mut ExecutionPlanBasis) -> ValueRef {
    let value = ValueRef::new(basis.value_count);
    basis.value_count += 1;
    let mut sources = basis.value_sources.to_vec();
    sources.push(PlanValueSource::ControlProduced(
        value,
        OutputProduction::FullyMaterialized,
    ));
    sources.sort();
    basis.value_sources = sources.into_boxed_slice();
    value
}

#[derive(Clone, Copy)]
enum FixtureInsertionOrder {
    Forward,
    Reverse,
}

fn in_fixture_order<T>(mut values: Vec<T>, order: FixtureInsertionOrder) -> Vec<T> {
    match order {
        FixtureInsertionOrder::Forward => {}
        FixtureInsertionOrder::Reverse => values.reverse(),
    }
    values
}

fn insert_tracked<K: Clone + Ord, V>(
    map: &mut BTreeMap<K, V>,
    trace: &mut Vec<K>,
    key: K,
    value: V,
) {
    trace.push(key.clone());
    assert!(map.insert(key, value).is_none());
}

struct DeterminismFixture {
    document: GraphDocument,
    node_insertions: Vec<NodeId>,
    connection_insertions: Vec<ConnectionId>,
    parameter_insertions: Vec<ParameterKey>,
    port_binding_insertions: Vec<PortAddress>,
    input_state_insertions: Vec<PortAddress>,
}

fn determinism_protocols() -> Vec<NodeProtocol> {
    let mut source_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    source_output.production = Some(OutputProduction::Streaming);
    let source = test_protocol(
        "determinism_relation_source",
        vec![source_output],
        vec![],
        vec![],
    );

    let mut middle_input = data_port("in", PortDirection::Input, TypeExpr::Unknown, None);
    middle_input.consumption = Some(InputConsumption::Streaming);
    let mut middle_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    middle_output.production = Some(OutputProduction::Streaming);
    let middle = test_protocol(
        "determinism_relation_middle",
        vec![middle_input, middle_output],
        vec![],
        vec![],
    );

    let mut sink_input = data_port("in", PortDirection::Input, TypeExpr::Unknown, None);
    sink_input.consumption = Some(InputConsumption::FullyMaterialized);
    let sink_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    let sink = test_protocol(
        "determinism_relation_sink",
        vec![sink_input, sink_output],
        vec![],
        vec![],
    );

    let mut dynamic_input = data_port("values", PortDirection::Input, TypeExpr::Unknown, None);
    dynamic_input.instances = PortInstances::UserCreated {
        min: 0,
        max: Some(2),
    };
    let mut inputs = test_protocol("determinism_inputs", vec![dynamic_input], vec![], vec![]);
    inputs.parameters = ParameterSchema::new(vec![
        ParameterSpec {
            key: ParameterKey::new("alpha").unwrap(),
            title_key: I18nKey::new("parameters.alpha.title").unwrap(),
            description_key: None,
            value_type: TypeExpr::Concrete(type_id("core.int64")),
            default_value: None,
            constraints: vec![ParameterConstraint::Required],
            editor: ParameterEditorSpec::Auto,
            presentation: ParameterPresentation::DetailPanel,
        },
        ParameterSpec {
            key: ParameterKey::new("beta").unwrap(),
            title_key: I18nKey::new("parameters.beta.title").unwrap(),
            description_key: None,
            value_type: TypeExpr::Concrete(type_id("core.int64")),
            default_value: None,
            constraints: vec![ParameterConstraint::Required],
            editor: ParameterEditorSpec::Auto,
            presentation: ParameterPresentation::DetailPanel,
        },
    ])
    .unwrap();

    vec![source, middle, sink, inputs]
}

fn determinism_registry(protocols: Vec<NodeProtocol>) -> TestRegistry {
    let source_type = protocols[0].type_id.clone();
    let middle_type = protocols[1].type_id.clone();
    let sink_type = protocols[2].type_id.clone();
    let backend = RelationalBackendId::new("test.relational").unwrap();

    TestRegistry::new(protocols)
        .with_lowerer(
            &source_type,
            FragmentLowerer {
                fragment: LoweredKernel::Relational(RelationalNodeFragment {
                    backend: backend.clone(),
                    fragment: relational::RelationalFragment {
                        id: RelationalFragmentId::new("determinism-source").unwrap(),
                        operators: Box::new([RelationalOperator::Source {
                            resource: ResourceId::new("database.source").unwrap(),
                            relation: "items".into(),
                        }]),
                        root: RelationalOperatorIndex::new(0),
                    },
                    inputs: Box::new([]),
                    metadata: FragmentMetadata::default(),
                }),
            },
        )
        .with_lowerer(
            &middle_type,
            FragmentLowerer {
                fragment: LoweredKernel::Relational(RelationalNodeFragment {
                    backend: backend.clone(),
                    fragment: relational::RelationalFragment {
                        id: RelationalFragmentId::new("determinism-middle").unwrap(),
                        operators: Box::new([
                            RelationalOperator::Input {
                                name: "input".into(),
                            },
                            RelationalOperator::Limit {
                                input: RelationalOperatorIndex::new(0),
                                rows: 10,
                            },
                        ]),
                        root: RelationalOperatorIndex::new(1),
                    },
                    inputs: Box::new([RelationalInputBinding {
                        port: PortAddress::declared(node_id(2), key("in")),
                        operator: RelationalOperatorIndex::new(0),
                    }]),
                    metadata: FragmentMetadata::default(),
                }),
            },
        )
        .with_lowerer(
            &sink_type,
            FragmentLowerer {
                fragment: LoweredKernel::Relational(RelationalNodeFragment {
                    backend,
                    fragment: relational::RelationalFragment {
                        id: RelationalFragmentId::new("determinism-sink").unwrap(),
                        operators: Box::new([RelationalOperator::Input {
                            name: "input".into(),
                        }]),
                        root: RelationalOperatorIndex::new(0),
                    },
                    inputs: Box::new([RelationalInputBinding {
                        port: PortAddress::declared(node_id(3), key("in")),
                        operator: RelationalOperatorIndex::new(0),
                    }]),
                    metadata: FragmentMetadata::default(),
                }),
            },
        )
}

fn determinism_fixture(order: FixtureInsertionOrder) -> DeterminismFixture {
    let mut parameters = BTreeMap::new();
    let mut parameter_insertions = Vec::new();
    for (key, value) in in_fixture_order(
        vec![
            (ParameterKey::new("alpha").unwrap(), serde_json::json!(1)),
            (ParameterKey::new("beta").unwrap(), serde_json::json!(2)),
        ],
        order,
    ) {
        insert_tracked(&mut parameters, &mut parameter_insertions, key, value);
    }

    let node_entries = vec![
        (1, "determinism_relation_source", BTreeMap::new()),
        (2, "determinism_relation_middle", BTreeMap::new()),
        (3, "determinism_relation_sink", BTreeMap::new()),
        (4, "determinism_inputs", parameters),
    ];
    let mut nodes = BTreeMap::new();
    let mut node_insertions = Vec::new();
    for (id, node_type, parameters) in in_fixture_order(node_entries, order) {
        let id = node_id(id);
        insert_tracked(
            &mut nodes,
            &mut node_insertions,
            id,
            DocumentNode {
                id,
                node_type: NodeTypeId::new(format!("yssbi.test.{node_type}")).unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters,
                user_label: None,
            },
        );
    }

    let dynamic_ports = vec![
        PortAddress::instance(
            node_id(4),
            key("values"),
            PortInstanceId::from_uuid(Uuid::from_u128(40)),
        ),
        PortAddress::instance(
            node_id(4),
            key("values"),
            PortInstanceId::from_uuid(Uuid::from_u128(41)),
        ),
    ];
    let connection_entries = vec![
        DocumentConnection {
            id: ConnectionId::from_uuid(Uuid::from_u128(10)),
            output: PortAddress::declared(node_id(1), key("out")),
            input: PortAddress::declared(node_id(2), key("in")),
            order: None,
        },
        DocumentConnection {
            id: ConnectionId::from_uuid(Uuid::from_u128(11)),
            output: PortAddress::declared(node_id(2), key("out")),
            input: PortAddress::declared(node_id(3), key("in")),
            order: None,
        },
        DocumentConnection {
            id: ConnectionId::from_uuid(Uuid::from_u128(12)),
            output: PortAddress::declared(node_id(1), key("out")),
            input: dynamic_ports[0].clone(),
            order: None,
        },
        DocumentConnection {
            id: ConnectionId::from_uuid(Uuid::from_u128(13)),
            output: PortAddress::declared(node_id(1), key("out")),
            input: dynamic_ports[1].clone(),
            order: None,
        },
    ];
    let mut connections = BTreeMap::new();
    let mut connection_insertions = Vec::new();
    for connection in in_fixture_order(connection_entries, order) {
        insert_tracked(
            &mut connections,
            &mut connection_insertions,
            connection.id,
            connection,
        );
    }

    let binding_entries = vec![
        (
            dynamic_ports[0].clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey("a".into()),
            },
        ),
        (
            dynamic_ports[1].clone(),
            DynamicPortBinding::UserCreated {
                order: OrderKey("b".into()),
            },
        ),
    ];
    let mut port_bindings = BTreeMap::new();
    let mut port_binding_insertions = Vec::new();
    for (address, binding) in in_fixture_order(binding_entries, order) {
        insert_tracked(
            &mut port_bindings,
            &mut port_binding_insertions,
            address,
            binding,
        );
    }

    let state_entries = vec![
        (
            dynamic_ports[0].clone(),
            InputState {
                literal_override: None,
            },
        ),
        (
            dynamic_ports[1].clone(),
            InputState {
                literal_override: None,
            },
        ),
    ];
    let mut input_states = BTreeMap::new();
    let mut input_state_insertions = Vec::new();
    for (address, state) in in_fixture_order(state_entries, order) {
        insert_tracked(
            &mut input_states,
            &mut input_state_insertions,
            address,
            state,
        );
    }

    DeterminismFixture {
        document: GraphDocument {
            revision: crate::node_system::document::GraphRevision::new(19),
            nodes,
            port_bindings,
            connections,
            input_states,
        },
        node_insertions,
        connection_insertions,
        parameter_insertions,
        port_binding_insertions,
        input_state_insertions,
    }
}

fn assert_reversed_insertions<T: std::fmt::Debug + PartialEq>(forward: &[T], reverse: &[T]) {
    assert!(
        forward.len() >= 2,
        "fixture container must have multiple entries"
    );
    assert_eq!(
        forward.iter().rev().collect::<Vec<_>>(),
        reverse.iter().collect::<Vec<_>>()
    );
}

struct DataframeDatabaseResources;

impl ResourceSnapshot for DataframeDatabaseResources {
    fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
        BTreeMap::from([(
            ResourceKey::new("databases/main"),
            ResourceVersion::new("fixture-v1"),
        )])
    }

    fn database_name(&self, id: &str) -> Option<&str> {
        (id == "main").then_some("Main database")
    }

    fn database_schema(&self, id: &str) -> Option<&[crate::schema::ColumnInfoDTO]> {
        (id == "main").then_some(&[])
    }
}

struct SourceSchemaResolver;
impl SchemaResolver for SourceSchemaResolver {
    fn resolve(
        &self,
        _: &mut SchemaResolutionContext<'_, '_>,
    ) -> Result<SchemaFact, SchemaResolutionError> {
        Ok(SchemaFact::new(
            SchemaExpr::Input(key("raw")),
            [
                SchemaField {
                    name: SchemaColumnRef("a".into()),
                    scalar_type: RelationalScalarType::Int64,
                    lineage: None,
                },
                SchemaField {
                    name: SchemaColumnRef("b".into()),
                    scalar_type: RelationalScalarType::String,
                    lineage: None,
                },
            ],
        ))
    }
}

fn compile_builtin_rename(
    from: serde_json::Value,
    to: serde_json::Value,
    include_source_schema: bool,
) -> CompileResult {
    use crate::node_system::catalog::{
        DATAFRAME_RESOURCE_SCHEMA_RESOLVER, build_builtin_node_system,
    };

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.dataframe.source.get"),
        (2, "yssbi.dataframe.rename"),
    ]);
    graph.nodes.get_mut(&node_id(1)).unwrap().parameters = BTreeMap::from([(
        ParameterKey::new("dataframe").unwrap(),
        serde_json::json!("databases/main"),
    )]);
    graph.nodes.get_mut(&node_id(2)).unwrap().parameters = BTreeMap::from([
        (ParameterKey::new("from").unwrap(), from),
        (ParameterKey::new("to").unwrap(), to),
    ]);
    connect(&mut graph, 10, 1, "dataframe", 2, "source");

    let mut resolvers = SchemaResolverSet::new();
    if include_source_schema {
        resolvers.insert(
            SchemaResolverId::new(DATAFRAME_RESOURCE_SCHEMA_RESOLVER).unwrap(),
            SourceSchemaResolver,
        );
    }
    GraphCompiler::with_schema_resolvers(&registry, &DataframeDatabaseResources, resolvers)
        .compile(&graph)
}

fn rename_diagnostic_codes(result: &CompileResult) -> BTreeSet<&str> {
    result
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

fn compile_builtin_relational_chain(graph_path: &str) -> CompileResult {
    use crate::node_system::catalog::{
        DATAFRAME_RESOURCE_SCHEMA_RESOLVER, build_builtin_node_system,
    };

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.dataframe.source.get"),
        (2, "yssbi.dataframe.filter.rows"),
        (3, "yssbi.dataframe.project"),
        (4, "yssbi.dataframe.rename"),
        (5, "yssbi.dataframe.limit"),
    ]);
    graph.nodes.get_mut(&node_id(1)).unwrap().parameters = BTreeMap::from([(
        ParameterKey::new("dataframe").unwrap(),
        serde_json::json!("databases/main"),
    )]);
    graph.nodes.get_mut(&node_id(2)).unwrap().parameters = BTreeMap::from([(
        ParameterKey::new("predicate").unwrap(),
        serde_json::json!({
            "column": "b",
            "operator": "equal",
            "value": { "type": "string", "value": "paid" }
        }),
    )]);
    graph.nodes.get_mut(&node_id(3)).unwrap().parameters = BTreeMap::from([(
        ParameterKey::new("columns").unwrap(),
        serde_json::json!(["a"]),
    )]);
    graph.nodes.get_mut(&node_id(4)).unwrap().parameters = BTreeMap::from([
        (ParameterKey::new("from").unwrap(), serde_json::json!("a")),
        (ParameterKey::new("to").unwrap(), serde_json::json!("total")),
    ]);
    graph.nodes.get_mut(&node_id(5)).unwrap().parameters =
        BTreeMap::from([(ParameterKey::new("rows").unwrap(), serde_json::json!(5))]);
    connect(&mut graph, 10, 1, "dataframe", 2, "source");
    connect(&mut graph, 11, 2, "result", 3, "source");
    connect(&mut graph, 12, 3, "result", 4, "source");
    connect(&mut graph, 13, 4, "result", 5, "source");

    let mut resolvers = SchemaResolverSet::new();
    resolvers.insert(
        SchemaResolverId::new(DATAFRAME_RESOURCE_SCHEMA_RESOLVER).unwrap(),
        SourceSchemaResolver,
    );
    let compiler =
        GraphCompiler::with_schema_resolvers(&registry, &DataframeDatabaseResources, resolvers);
    let snapshot = compiler.snapshot(GraphResourcePath(graph_path.into()), &graph);
    compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap()
}

fn control_port(name: &str, direction: PortDirection) -> PortSpec {
    PortSpec {
        key: key(name),
        label_key: I18nKey::new(format!("ports.{name}.label")).unwrap(),
        direction,
        kind: PortKind::Control,
        value_type: TypeExpr::Unknown,
        instances: PortInstances::Declared,
        connections: if direction == PortDirection::Input {
            ConnectionsPerPort::Single
        } else {
            ConnectionsPerPort::Multiple {
                max: None,
                ordered: true,
            }
        },
        input_binding: None,
        consumption: None,
        production: None,
        editor: PortEditorSpec::Default,
        schema: None,
    }
}

fn structural_protocol(
    name: &str,
    ports: Vec<PortSpec>,
    parameters: Vec<ParameterSpec>,
) -> NodeProtocol {
    let mut protocol = test_protocol(name, ports, vec![], vec![]);
    protocol.parameters = ParameterSchema::new(parameters).unwrap();
    protocol
}

fn set_parameters(graph: &mut GraphDocument, node: u128, values: &[(&str, serde_json::Value)]) {
    graph.nodes.get_mut(&node_id(node)).unwrap().parameters = values
        .iter()
        .map(|(name, value)| (ParameterKey::new(*name).unwrap(), value.clone()))
        .collect();
}

fn contains_region(
    region: &crate::node_system::plan::StructuredControlRegion,
    predicate: &impl Fn(&crate::node_system::plan::StructuredControlRegion) -> bool,
) -> bool {
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};
    if predicate(region) {
        return true;
    }
    match region {
        StructuredControlRegion::Sequence(steps) => steps.iter().any(|step| match step {
            ControlStep::Operation(_) => false,
            ControlStep::Region(region) => contains_region(region, predicate),
        }),
        StructuredControlRegion::If {
            then_region,
            else_region,
            ..
        } => contains_region(then_region, predicate) || contains_region(else_region, predicate),
        StructuredControlRegion::Loop { body, .. } => contains_region(body, predicate),
        StructuredControlRegion::Call { .. } => false,
    }
}

fn operation_index_for_node(
    plan: &crate::node_system::plan::ExecutionPlan,
    node: u128,
) -> crate::node_system::plan::OperationIndex {
    crate::node_system::plan::OperationIndex::new(
        plan.operations
            .iter()
            .position(|operation| operation.source_node_id == node_id(node))
            .expect("built-in operation must be present") as u32,
    )
}

fn region_contains_operation(
    region: &crate::node_system::plan::StructuredControlRegion,
    expected: crate::node_system::plan::OperationIndex,
) -> bool {
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};
    match region {
        StructuredControlRegion::Sequence(steps) => steps.iter().any(|step| match step {
            ControlStep::Operation(operation) => *operation == expected,
            ControlStep::Region(region) => region_contains_operation(region, expected),
        }),
        StructuredControlRegion::If {
            then_region,
            else_region,
            ..
        } => {
            region_contains_operation(then_region, expected)
                || region_contains_operation(else_region, expected)
        }
        StructuredControlRegion::Loop { body, .. } => region_contains_operation(body, expected),
        StructuredControlRegion::Call { .. } => false,
    }
}

struct PanicLowerer;
impl NodeLowerer for PanicLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Err(LoweringError::internal(
            LoweringInvariant::StructuralNodeReachedLeafLowerer,
        ))
    }
}

fn structured_function_plan(
    root_region: StructuredControlRegion,
    value_sources: Box<[PlanValueSource]>,
    value_dependencies: Box<[ValueDependency]>,
    value_count: u32,
    result_value: ValueRef,
) -> (ExecutionPlan, FunctionPlanAbi, FunctionParameterId) {
    let result = FunctionParameterId("return".into());
    let provenance = crate::node_system::analysis::CompileProvenance {
        project_session_id: ProjectSessionId::new("project-a"),
        graph_path: GraphResourcePath("functions/structured-result".into()),
        basis: crate::node_system::analysis::CompilationBasis {
            graph_revision: crate::node_system::document::GraphRevision::new(1),
            registry_fingerprint: RegistryFingerprint::from_bytes([7; 32]),
            resource_versions: BTreeMap::new(),
            resource_observations: BTreeMap::new(),
        },
        compile_id: CompileId::new(1),
    };
    let plan = ExecutionPlan {
        provenance: provenance.clone(),
        value_count,
        operations: Box::new([]),
        value_contracts: BTreeMap::new(),
        value_sources,
        bound_values: BTreeMap::new(),
        value_dependencies,
        root_region,
        effect_dependencies: Box::new([]),
        relational_subplans: Box::new([]),
        resources: Box::new([]),
        results: Box::new([]),
        publications: Box::new([]),
    };
    let abi = FunctionPlanAbi {
        provenance,
        parameters: BTreeMap::new(),
        parameter_contracts: BTreeMap::new(),
        results: BTreeMap::from([(result.clone(), result_value)]),
        result_productions: BTreeMap::from([(result.clone(), OutputProduction::FullyMaterialized)]),
        result_contracts: BTreeMap::new(),
    };
    (plan, abi, result)
}

mod calls;
mod control;
mod demand;
mod dynamic;
mod lowering;
mod pipeline;
mod resources;
mod retry;
mod types;
