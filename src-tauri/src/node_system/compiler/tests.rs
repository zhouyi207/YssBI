use super::*;
use crate::node_system::analysis::{
    CompileId, NOOP_TRACE_SINK, ProjectSessionId, ResourceKey, ResourceVersion, SpanEvent,
    SpanKind, TraceSink,
};
use crate::node_system::document::{
    ConnectionId, DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
    FunctionDocument, FunctionParameter, FunctionParameterId, FunctionSignature, GraphDocument,
    GraphResourcePath, InputState, NodeId, NodePosition, OrderKey, PortAddress, PortInstanceId,
};
use crate::node_system::plan::{
    BranchResultBinding, CallArgumentBinding, CallResultBinding, CompiledParameterHandle,
    CompiledResourceRequirement, ControlStep, ExecutionDemand, FunctionPlanHandle, GraphOutputRef,
    KernelHandle, LoopCarriedBinding, MaterializationBridge, PlanResult, PlanValueSource,
    RelationalBackendId, RelationalBridgeInput, RelationalExpression, RelationalFragmentId,
    RelationalLiteral, RelationalOperator, RelationalOperatorIndex, RelationalPushdownHint,
    ResourceAccess, ResourceId, ResourceKind, StructuredControlRegion, ValueRef,
};
use crate::node_system::protocol::*;
use crate::node_system::registry::{
    CategoryRegistration, I18nManifest, NodeRegistry, NodeRegistryBuilder, ProtocolFingerprint,
    ProviderRegistration, RegisteredNode, RegistryFingerprint, StructuralNodeRole,
};
use crate::node_system::testing::TestProtocolBuilder;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
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

#[derive(Default)]
struct RecordingTrace(Mutex<Vec<SpanEvent>>);

impl TraceSink for RecordingTrace {
    fn record(&self, event: SpanEvent) {
        self.0.lock().unwrap().push(event);
    }
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
#[test]
fn owned_protocol_fixture_preserves_catalog_style_and_fingerprint() {
    let first = registry();
    let second = registry();
    let type_id = NodeTypeId::new("yssbi.test.constant").unwrap();

    assert_eq!(
        first.catalog_manifest().node_protocols.get(&type_id),
        second.catalog_manifest().node_protocols.get(&type_id),
        "the owned fixture must retain a stable protocol fingerprint"
    );
    assert_eq!(
        first.protocol(&type_id).unwrap().catalog.style_id.as_str(),
        "test"
    );
}

fn protocol() -> NodeProtocol {
    TestProtocolBuilder::new("yssbi.test.constant", "test")
        .style("test")
        .ports(vec![PortSpec {
            key: PortKey::new("value").unwrap(),
            label_key: I18nKey::new("nodes.test.constant.value").unwrap(),
            direction: PortDirection::Output,
            kind: PortKind::Data,
            value_type: TypeExpr::Unknown,
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

struct ProtocolOverrideCompilerRegistry<'a> {
    frozen: &'a NodeRegistry,
    node_type: NodeTypeId,
    protocol: NodeProtocol,
    structural_role: StructuralNodeRole,
}

impl TypeEnvironment for ProtocolOverrideCompilerRegistry<'_> {
    fn concrete_implements(&self, value_type: &TypeId, class: &TypeClassId) -> Option<bool> {
        self.frozen.concrete_implements(value_type, class)
    }

    fn constructor_arity(&self, constructor: &TypeConstructorId) -> Option<usize> {
        self.frozen.constructor_arity(constructor)
    }
}

impl CompilerRegistry for ProtocolOverrideCompilerRegistry<'_> {
    fn fingerprint(&self) -> &RegistryFingerprint {
        self.frozen.fingerprint()
    }

    fn resolve(&self, node_type: &NodeTypeId) -> Option<RegistryNode<'_>> {
        if node_type != &self.node_type {
            return self.frozen.resolve(node_type);
        }
        Some(RegistryNode {
            protocol: &self.protocol,
            protocol_fingerprint: self
                .frozen
                .catalog_manifest()
                .node_protocols
                .get(node_type)?
                .clone(),
            behavior: RegistryNodeBehavior::Structural(self.structural_role),
        })
    }
}

fn registry() -> NodeRegistry {
    let mut provider = ProviderRegistration::new(ProviderId::new("yssbi").unwrap());
    provider.categories = vec![CategoryRegistration {
        id: NodeCategoryId::new("test").unwrap(),
        title_key: I18nKey::new("categories.test.title").unwrap(),
        parent: None,
        order: 0,
    }]
    .into_boxed_slice();
    provider.i18n = I18nManifest {
        keys: BTreeSet::from([
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

#[test]
fn corrupt_registry_snapshot_produces_missing_lowering_blocking_diagnostic() {
    let registry = CorruptCompilerRegistrySnapshot { frozen: registry() };
    let result = GraphCompiler::new(&registry, &Resources)
        .compile(&document(NodeTypeId::new("yssbi.test.constant").unwrap()));

    assert!(result.plan.is_none());
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.lowering.implementation_missing"
            && diagnostic.severity.is_blocking()
    }));
}

#[test]
fn valid_constant_graph_produces_plan_with_same_basis() {
    let registry = registry();
    let result = GraphCompiler::new(&registry, &Resources)
        .compile(&document(NodeTypeId::new("yssbi.test.constant").unwrap()));
    let semantic = result
        .semantic
        .expect("valid graph should retain its semantic graph");
    let plan = result.plan.expect("valid graph should lower");
    assert!(result.analysis.diagnostics.is_empty());
    assert_eq!(plan.operations.len(), 1);
    assert_eq!(semantic.basis, result.analysis.basis);
    assert_eq!(plan.provenance.basis, semantic.basis);
}

#[test]
fn compile_plan_and_trace_keep_the_exact_requested_correlation() {
    let registry = registry();
    let trace = RecordingTrace::default();
    let compiler = GraphCompiler::new(&registry, &Resources)
        .with_observability(ProjectSessionId::new("project-session-1"), &trace);
    let snapshot = compiler.snapshot_with_compile_id(
        CompileId::new(41),
        GraphResourcePath("events/main".into()),
        &document(NodeTypeId::new("yssbi.test.constant").unwrap()),
    );

    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let plan = result.plan.unwrap();

    assert_eq!(plan.provenance, snapshot.provenance);
    for event in trace.0.lock().unwrap().iter() {
        assert_eq!(
            event.correlation.project_session_id,
            snapshot.provenance.project_session_id
        );
        assert_eq!(event.correlation.graph_path, snapshot.provenance.graph_path);
        assert_eq!(
            event.correlation.graph_revision,
            snapshot.provenance.basis.graph_revision
        );
        assert_eq!(event.correlation.compile_id, snapshot.provenance.compile_id);
    }
}

#[test]
fn blocking_compile_emits_no_lowering_or_run_span() {
    let registry = registry();
    let trace = RecordingTrace::default();
    let result = GraphCompiler::new(&registry, &Resources)
        .with_observability(ProjectSessionId::new("project-session-1"), &trace)
        .compile(&document(NodeTypeId::new("yssbi.test.missing").unwrap()));

    assert!(result.plan.is_none());
    assert!(
        trace
            .0
            .lock()
            .unwrap()
            .iter()
            .all(|event| { !matches!(event.kind, SpanKind::Lowering | SpanKind::Run) })
    );
}

#[test]
fn unknown_node_returns_analysis_without_plan() {
    let registry = registry();
    let result = GraphCompiler::new(&registry, &Resources)
        .compile(&document(NodeTypeId::new("yssbi.test.missing").unwrap()));
    assert!(result.semantic.is_none());
    assert!(result.plan.is_none());
    assert_eq!(
        result.analysis.diagnostics[0].code.as_str(),
        "compiler.node.unknown"
    );
}

#[test]
fn unknown_port_and_wrong_direction_return_analysis_without_plan() {
    let registry = registry();
    let mut graph = document(NodeTypeId::new("yssbi.test.constant").unwrap());
    let node = node_id(1);
    let other = node_id(2);
    graph.nodes.insert(
        other,
        DocumentNode {
            id: other,
            node_type: NodeTypeId::new("yssbi.test.constant").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        },
    );
    let unknown = PortAddress::declared(other, key("missing"));
    let output_as_input = PortAddress::declared(other, key("value"));
    graph.connections.insert(
        crate::node_system::document::ConnectionId::from_uuid(Uuid::from_u128(3)),
        DocumentConnection {
            id: crate::node_system::document::ConnectionId::from_uuid(Uuid::from_u128(3)),
            output: PortAddress::declared(node, key("value")),
            input: unknown,
            order: None,
        },
    );
    graph.connections.insert(
        crate::node_system::document::ConnectionId::from_uuid(Uuid::from_u128(4)),
        DocumentConnection {
            id: crate::node_system::document::ConnectionId::from_uuid(Uuid::from_u128(4)),
            output: PortAddress::declared(node, key("value")),
            input: output_as_input,
            order: None,
        },
    );
    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    assert!(result.plan.is_none());
    let codes: Vec<_> = result
        .analysis
        .diagnostics
        .iter()
        .map(|item| item.code.as_str())
        .collect();
    assert!(codes.contains(&"compiler.port.unknown"));
    assert!(codes.contains(&"compiler.connection.input_direction"));
}

struct TestRegistry {
    fingerprint: RegistryFingerprint,
    nodes: BTreeMap<NodeTypeId, (NodeProtocol, NodeImplementation)>,
    structural_roles: BTreeMap<NodeTypeId, StructuralNodeRole>,
    type_classes: BTreeMap<TypeId, BTreeSet<TypeClassId>>,
    constructor_arities: BTreeMap<TypeConstructorId, usize>,
    constructor_classes: BTreeMap<TypeConstructorId, BTreeSet<TypeClassId>>,
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
    ports: Vec<PortSpec>,
    type_parameters: Vec<TypeParameterId>,
    constraints: Vec<TypeConstraint>,
) -> NodeProtocol {
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
        execution: ExecutionSemantics {
            determinism: Determinism::Deterministic,
            purity: Purity::Pure,
            evaluation: EvaluationPolicy::DemandDriven,
            cache: CachePolicy::PerRun,
            effects: EffectSemantics::None,
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

impl NodeLowerer for FailingLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Err(LoweringError::new("expected lowering failure"))
    }
}

#[test]
fn lowering_diagnostic_clears_semantic_and_plan() {
    let protocol = test_protocol("lowering_failure", vec![], vec![], vec![]);
    let node_type = protocol.type_id.clone();
    let registry = TestRegistry::new(vec![protocol]).with_lowerer(&node_type, FailingLowerer);

    let result = GraphCompiler::new(&registry, &Resources)
        .compile(&graph_with_nodes(&[(1, "lowering_failure")]));

    assert!(result.semantic.is_none());
    assert!(result.plan.is_none());
    assert!(
        result
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code.as_str() == "compiler.lowering.failed" })
    );
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

#[test]
fn compiler_maps_data_edges_into_plan_dependencies() {
    let source = test_protocol(
        "plan_data_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let sink = test_protocol(
        "plan_data_sink",
        vec![data_port(
            "in",
            PortDirection::Input,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source, sink]);
    let mut graph = graph_with_nodes(&[(1, "plan_data_source"), (2, "plan_data_sink")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let plan = GraphCompiler::new(&registry, &Resources)
        .compile(&graph)
        .plan
        .expect("data graph should lower");

    assert_eq!(plan.value_dependencies.len(), 1);
    assert_ne!(
        plan.value_dependencies[0].source,
        plan.value_dependencies[0].destination
    );
    assert_eq!(
        plan.value_dependencies[0].source,
        plan.operations[0].outputs[0].value
    );
    assert_eq!(
        plan.value_dependencies[0].destination,
        plan.operations[1].inputs[0].value
    );
}

#[test]
fn compiler_maps_effect_edges_into_plan_dependencies() {
    let mut before = test_protocol(
        "plan_effect_before",
        vec![effect_port("effect", PortDirection::Output)],
        vec![],
        vec![],
    );
    before.execution.purity = Purity::Effectful;
    before.execution.effects = EffectSemantics::Ordered;
    let mut after = test_protocol(
        "plan_effect_after",
        vec![effect_port("effect", PortDirection::Input)],
        vec![],
        vec![],
    );
    after.execution.purity = Purity::Effectful;
    after.execution.effects = EffectSemantics::Ordered;
    let before_type = before.type_id.clone();
    let after_type = after.type_id.clone();
    let registry = TestRegistry::new(vec![before, after])
        .with_lowerer(
            &before_type,
            FragmentLowerer {
                fragment: kernel_fragment(EffectSemantics::Ordered, FragmentMetadata::default()),
            },
        )
        .with_lowerer(
            &after_type,
            FragmentLowerer {
                fragment: kernel_fragment(EffectSemantics::Ordered, FragmentMetadata::default()),
            },
        );
    let mut graph = graph_with_nodes(&[(1, "plan_effect_before"), (2, "plan_effect_after")]);
    connect(&mut graph, 10, 1, "effect", 2, "effect");

    let plan = GraphCompiler::new(&registry, &Resources)
        .compile(&graph)
        .plan
        .expect("effect graph should lower");

    assert_eq!(plan.effect_dependencies.len(), 1);
    assert_eq!(plan.effect_dependencies[0].before.index(), 0);
    assert_eq!(plan.effect_dependencies[0].after.index(), 1);
}

#[test]
fn compiler_plans_relational_islands_with_valid_local_indices() {
    let mut source_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    source_output.production = Some(OutputProduction::Streaming);
    let source = test_protocol("plan_relation_source", vec![source_output], vec![], vec![]);
    let mut sink_input = data_port("in", PortDirection::Input, TypeExpr::Unknown, None);
    sink_input.consumption = Some(InputConsumption::Streaming);
    let mut sink_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    sink_output.production = Some(OutputProduction::Streaming);
    let sink = test_protocol(
        "plan_relation_sink",
        vec![sink_input, sink_output],
        vec![],
        vec![],
    );
    let source_type = source.type_id.clone();
    let sink_type = sink.type_id.clone();
    let backend = RelationalBackendId::new("test.relational").unwrap();
    let registry = TestRegistry::new(vec![source, sink])
        .with_lowerer(
            &source_type,
            FragmentLowerer {
                fragment: LoweredKernel::Relational(RelationalNodeFragment {
                    backend: backend.clone(),
                    fragment: relational::RelationalFragment {
                        id: RelationalFragmentId::new("source").unwrap(),
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
            &sink_type,
            FragmentLowerer {
                fragment: LoweredKernel::Relational(RelationalNodeFragment {
                    backend,
                    fragment: relational::RelationalFragment {
                        id: RelationalFragmentId::new("sink").unwrap(),
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
        );
    let mut graph = graph_with_nodes(&[(1, "plan_relation_source"), (2, "plan_relation_sink")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/relational".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("relational graph should retain pre-group facts");
    let plan = result.plan.expect("relational graph should lower");

    assert_eq!(plan.relational_subplans.len(), 1);
    let subplan = &plan.relational_subplans[0];
    assert_eq!(subplan.compiled_plan.fragment_order.len(), 2);
    assert_eq!(subplan.compiled_plan.roots.len(), 1);
    assert!(subplan.materialization_bridges.is_empty());

    assert_eq!(
        plan.operations.len(),
        1,
        "one operation must own the island"
    );
    let operation = &plan.operations[0];
    assert!(matches!(
        operation.kernel,
        crate::node_system::plan::PlannedKernel::Relational(index) if index.index() == 0
    ));
    assert!(
        operation.inputs.is_empty(),
        "the source makes the island self-contained"
    );
    assert_eq!(operation.outputs.len(), 1, "only the sink root is exposed");
    assert_eq!(operation.outputs[0].production, OutputProduction::Streaming);
    assert!(
        plan.value_dependencies.is_empty(),
        "an internal fragment edge must not make the island depend on itself"
    );
    assert!(matches!(
        plan.root_region,
        crate::node_system::plan::StructuredControlRegion::Sequence(ref steps)
            if matches!(steps.as_ref(), [crate::node_system::plan::ControlStep::Operation(index)] if index.index() == 0)
    ));

    assert_eq!(basis.operations.len(), 2, "basis remains pre-group");
    assert_eq!(basis.relational_connections.len(), 1);
    assert!(matches!(
        &basis.operations[0].kernel,
        super::specialization::IntermediateKernel::Relational { fragment, .. }
            if fragment.id.as_str() == "source"
    ));
    assert!(matches!(
        &basis.operations[1].kernel,
        super::specialization::IntermediateKernel::Relational { fragment, input_bindings, .. }
            if fragment.id.as_str() == "sink" && input_bindings.len() == 1
    ));

    let requested_source = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/relational", 1, "out")]),
            include_default_results: false,
        })
        .expect("same-island intermediate output must be derivable");
    assert_eq!(requested_source.operations.len(), 1);
    assert_eq!(requested_source.operations[0].outputs.len(), 1);
    assert_eq!(requested_source.relational_subplans.len(), 1);
    assert_eq!(
        requested_source.relational_subplans[0]
            .compiled_plan
            .fragment_order
            .as_ref(),
        &[RelationalFragmentId::new("source").unwrap()]
    );
    assert_eq!(
        requested_source.relational_subplans[0]
            .compiled_plan
            .roots
            .len(),
        requested_source.operations[0].outputs.len()
    );
    requested_source
        .validate()
        .expect("requested relational output plan validates");

    let mut reversed = graph_with_nodes(&[(2, "plan_relation_sink"), (1, "plan_relation_source")]);
    connect(&mut reversed, 10, 1, "out", 2, "in");
    let reversed_plan = GraphCompiler::new(&registry, &Resources)
        .compile(&reversed)
        .plan
        .expect("reordered relational graph should lower");
    assert_eq!(reversed_plan.operations, plan.operations);
    assert_eq!(reversed_plan.value_dependencies, plan.value_dependencies);
    assert_eq!(reversed_plan.root_region, plan.root_region);
    assert_eq!(reversed_plan.relational_subplans, plan.relational_subplans);
}

#[test]
fn compiler_aggregates_fragment_resources_and_results() {
    let protocol = test_protocol(
        "plan_metadata",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let node_type = protocol.type_id.clone();
    let resource = CompiledResourceRequirement {
        resource: ResourceId::new("database.main").unwrap(),
        kind: ResourceKind::DatabaseConnection,
        access: ResourceAccess::Shared,
        optional: false,
    };
    let registry = TestRegistry::new(vec![protocol]).with_lowerer(
        &node_type,
        FragmentLowerer {
            fragment: kernel_fragment(
                EffectSemantics::None,
                FragmentMetadata {
                    effect: EffectSemantics::None,
                    resources: Box::new([resource.clone()]),
                    results: Box::new([FragmentResult {
                        name: "answer".into(),
                        output: PortAddress::declared(node_id(1), key("out")),
                    }]),
                },
            ),
        },
    );

    let plan = GraphCompiler::new(&registry, &Resources)
        .compile(&graph_with_nodes(&[(1, "plan_metadata")]))
        .plan
        .expect("metadata graph should lower");

    assert_eq!(plan.resources.as_ref(), &[resource]);
    assert_eq!(
        plan.results.as_ref(),
        &[PlanResult {
            name: "answer".into(),
            output: GraphOutputRef {
                graph_path: plan.provenance.graph_path.clone(),
                port: PortAddress::declared(node_id(1), key("out")),
            },
            value: plan.operations[0].outputs[0].value,
        }]
    );
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
    sources.push(PlanValueSource::ControlProduced(value));
    sources.sort();
    basis.value_sources = sources.into_boxed_slice();
    value
}

#[test]
fn demand_specialization_deletes_disconnected_if_control_sources() {
    let mut basis = compiled_demand_basis();
    let destination = declare_control_value(&mut basis);
    let condition = basis.operations[0].outputs[0].value;
    let then_source = basis.operations[0].outputs[0].value;
    let else_source = basis.operations[2].outputs[0].value;
    append_control_region(
        &mut basis,
        StructuredControlRegion::If {
            condition,
            then_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            else_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            results: Box::new([BranchResultBinding {
                destination,
                then_source,
                else_source,
            }]),
        },
    );

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/main", 2, "out")]),
            include_default_results: false,
        })
        .expect("disconnected If declarations are projected out");

    assert!(!contains_region(&plan.root_region, &|region| matches!(
        region,
        StructuredControlRegion::If { .. }
    )));
    assert!(
        !plan
            .value_sources
            .contains(&PlanValueSource::ControlProduced(destination))
    );
    plan.validate().unwrap();
}

#[test]
fn demand_specialization_keeps_only_requested_if_result_declaration() {
    let mut basis = compiled_demand_basis();
    let retained_destination = declare_control_value(&mut basis);
    let deleted_destination = declare_control_value(&mut basis);
    let condition = basis.operations[0].outputs[0].value;
    let then_source = basis.operations[0].outputs[0].value;
    let else_source = basis.operations[2].outputs[0].value;
    append_control_region(
        &mut basis,
        StructuredControlRegion::If {
            condition,
            then_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            else_region: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            results: Box::new([
                BranchResultBinding {
                    destination: retained_destination,
                    then_source,
                    else_source,
                },
                BranchResultBinding {
                    destination: deleted_destination,
                    then_source,
                    else_source,
                },
            ]),
        },
    );
    let requested = demand_output("events/main", 99, "out");
    basis.nodes.insert(node_id(99));
    basis.port_facts.insert(
        requested.port.clone(),
        super::specialization::DemandPortFact {
            kind: PortKind::Data,
            direction: PortDirection::Output,
        },
    );
    basis.output_results.insert(
        requested.clone(),
        PlanResult {
            name: "requested.branch-result".into(),
            output: requested.clone(),
            value: retained_destination,
        },
    );

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([requested]),
            include_default_results: false,
        })
        .expect("only the requested If result declaration remains");

    let retained_results = match &plan.root_region {
        region
            if contains_region(region, &|candidate| {
                matches!(candidate, StructuredControlRegion::If { .. })
            }) =>
        {
            fn find(region: &StructuredControlRegion) -> Option<&[BranchResultBinding]> {
                match region {
                    StructuredControlRegion::Sequence(steps) => {
                        steps.iter().find_map(|step| match step {
                            ControlStep::Operation(_) => None,
                            ControlStep::Region(region) => find(region),
                        })
                    }
                    StructuredControlRegion::If { results, .. } => Some(results),
                    StructuredControlRegion::Loop { body, .. } => find(body),
                    StructuredControlRegion::Call { .. } => None,
                }
            }
            find(region).unwrap()
        }
        _ => panic!("retained If region"),
    };
    assert_eq!(retained_results.len(), 1);
    assert_eq!(retained_results[0].destination, retained_destination);
    assert!(
        plan.value_sources
            .contains(&PlanValueSource::ControlProduced(retained_destination))
    );
    assert!(
        !plan
            .value_sources
            .contains(&PlanValueSource::ControlProduced(deleted_destination))
    );
    plan.validate().unwrap();
}

#[test]
fn demand_specialization_deletes_disconnected_loop_control_sources() {
    let mut basis = compiled_demand_basis();
    let body_input = declare_control_value(&mut basis);
    let result = declare_control_value(&mut basis);
    let initial_source = basis.operations[0].outputs[0].value;
    let next_source = basis.operations[2].outputs[0].value;
    let continue_condition = basis.operations[0].outputs[0].value;
    append_control_region(
        &mut basis,
        StructuredControlRegion::Loop {
            body: Box::new(StructuredControlRegion::Sequence(Box::new([]))),
            carried: Box::new([LoopCarriedBinding {
                body_input,
                initial_source,
                next_source,
                result,
            }]),
            continue_condition,
            max_iterations: 3,
        },
    );

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/main", 2, "out")]),
            include_default_results: false,
        })
        .expect("disconnected Loop declarations are projected out");

    assert!(!contains_region(&plan.root_region, &|region| matches!(
        region,
        StructuredControlRegion::Loop { .. }
    )));
    for value in [body_input, result] {
        assert!(
            !plan
                .value_sources
                .contains(&PlanValueSource::ControlProduced(value))
        );
    }
    plan.validate().unwrap();
}

#[test]
fn demand_specialization_prunes_independent_pure_chain_and_owned_resource() {
    let (registry, graph) = demand_fixture();
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/main".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid graph has lowering basis");

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/main", 2, "out")]),
            include_default_results: false,
        })
        .unwrap();

    assert_eq!(
        plan.operations
            .iter()
            .map(|operation| operation.source_node_id)
            .collect::<Vec<_>>(),
        vec![node_id(1), node_id(2)]
    );
    assert_eq!(plan.results.len(), 1);
    assert_eq!(
        plan.results[0].output,
        demand_output("events/main", 2, "out")
    );
    assert_eq!(
        plan.resources
            .iter()
            .map(|requirement| requirement.resource.as_str())
            .collect::<Vec<_>>(),
        vec!["database.chain-a"]
    );
}

#[test]
fn demand_normalization_is_order_independent_and_default_modes_are_distinct() {
    let (registry, graph) = demand_fixture();
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/main".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid graph has lowering basis");
    let a = demand_output("events/main", 2, "out");
    let b = demand_output("events/main", 4, "out");

    let first = ExecutionDemand::Outputs {
        outputs: Box::new([a.clone(), b.clone()]),
        include_default_results: false,
    };
    let second = ExecutionDemand::Outputs {
        outputs: Box::new([b.clone(), a.clone(), a.clone()]),
        include_default_results: false,
    };
    let first_key = basis.normalize_demand(&first).unwrap();
    let second_key = basis.normalize_demand(&second).unwrap();
    assert_eq!(first_key, second_key);
    assert_eq!(first_key.digest(), second_key.digest());
    assert_eq!(
        basis.derive_plan(&first).unwrap(),
        basis.derive_plan(&second).unwrap()
    );
    let same_selection_different_mode = ExecutionDemand::Outputs {
        outputs: Box::new([]),
        include_default_results: true,
    };
    let default_key = basis.normalize_demand(&ExecutionDemand::Default).unwrap();
    let explicit_default_key = basis
        .normalize_demand(&same_selection_different_mode)
        .unwrap();
    assert_ne!(
        default_key, explicit_default_key,
        "normalized keys retain request mode even when selected outputs match"
    );
    assert_ne!(default_key.digest(), explicit_default_key.digest());
    let without_defaults = basis
        .normalize_demand(&ExecutionDemand::Outputs {
            outputs: Box::new([a.clone(), b.clone()]),
            include_default_results: false,
        })
        .unwrap();
    let with_defaults = basis
        .normalize_demand(&ExecutionDemand::Outputs {
            outputs: Box::new([a.clone(), b.clone()]),
            include_default_results: true,
        })
        .unwrap();
    assert_ne!(without_defaults, with_defaults);
    assert_ne!(without_defaults.digest(), with_defaults.digest());

    let defaults = basis.derive_plan(&ExecutionDemand::Default).unwrap();
    assert_eq!(defaults.results.len(), 2);
    let only_a = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([a.clone()]),
            include_default_results: false,
        })
        .unwrap();
    assert_eq!(only_a.results.len(), 1);
    let a_with_defaults = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([a]),
            include_default_results: true,
        })
        .unwrap();
    assert_eq!(a_with_defaults.results.len(), 2);
}

#[test]
fn invalid_requested_outputs_are_rejected_before_plan_construction() {
    let source = test_protocol(
        "demand_validation_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let mut protocol = test_protocol(
        "demand_validation",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port("out", PortDirection::Output, TypeExpr::Unknown, None),
            effect_port("effect", PortDirection::Output),
            control_port("control", PortDirection::Output),
        ],
        vec![],
        vec![],
    );
    protocol.execution.effects = EffectSemantics::Ordered;
    let registry = TestRegistry::new(vec![source, protocol]);
    let mut graph = graph_with_nodes(&[(1, "demand_validation_source"), (2, "demand_validation")]);
    connect(&mut graph, 10, 1, "out", 2, "in");
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/main".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid graph has lowering basis");
    let stale_instance = GraphOutputRef {
        graph_path: GraphResourcePath("events/main".into()),
        port: PortAddress::instance(
            node_id(2),
            key("out"),
            PortInstanceId::from_uuid(Uuid::from_u128(99)),
        ),
    };
    let invalid = [
        (
            demand_output("events/other", 2, "out"),
            "graph_path_mismatch",
        ),
        (demand_output("events/main", 99, "out"), "missing_node"),
        (demand_output("events/main", 2, "missing"), "missing_port"),
        (demand_output("events/main", 2, "in"), "input_port"),
        (demand_output("events/main", 2, "effect"), "effect_port"),
        (demand_output("events/main", 2, "control"), "control_port"),
        (stale_instance, "stale_instance"),
    ];

    for (output, expected) in invalid {
        let error = basis
            .derive_plan(&ExecutionDemand::Outputs {
                outputs: Box::new([output.clone()]),
                include_default_results: false,
            })
            .unwrap_err();
        let actual = match error {
            DemandPlanError::GraphPathMismatch(_) => "graph_path_mismatch",
            DemandPlanError::MissingNode(_) => "missing_node",
            DemandPlanError::MissingPort(_) => "missing_port",
            DemandPlanError::StalePortInstance(_) => "stale_instance",
            DemandPlanError::InputPort(_) => "input_port",
            DemandPlanError::ControlPort(_) => "control_port",
            DemandPlanError::EffectPort(_) => "effect_port",
            DemandPlanError::InvalidDerivedPlan(_) => "invalid_derived_plan",
        };
        assert_eq!(actual, expected, "wrong error for {output:?}");
    }
}

#[test]
fn retained_operation_keeps_external_value_dependency_and_source() {
    let mut entry = test_protocol(
        "demand_external_entry",
        vec![
            control_port("then", PortDirection::Output),
            data_port("payload", PortDirection::Output, TypeExpr::Unknown, None),
        ],
        vec![],
        vec![],
    );
    entry.managed_role = Some(ManagedNodeRole::EventBegin);
    let entry_type = entry.type_id.clone();
    let sink = chain_protocol("demand_external_sink");
    let sink_type = sink.type_id.clone();
    let registry = TestRegistry::new(vec![entry, sink])
        .structural(&entry_type, StructuralNodeRole::EventBegin)
        .with_lowerer(
            &sink_type,
            FragmentLowerer {
                fragment: kernel_fragment(
                    EffectSemantics::None,
                    FragmentMetadata {
                        results: Box::new([FragmentResult {
                            name: "external-result".into(),
                            output: PortAddress::declared(node_id(2), key("out")),
                        }]),
                        ..FragmentMetadata::default()
                    },
                ),
            },
        );
    let mut graph = graph_with_nodes(&[(1, "demand_external_entry"), (2, "demand_external_sink")]);
    connect(&mut graph, 10, 1, "payload", 2, "in");
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/external".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid graph has lowering basis");

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/external", 2, "out")]),
            include_default_results: false,
        })
        .expect("external source dependency remains valid");

    assert_eq!(plan.operations.len(), 1);
    assert_eq!(plan.value_dependencies.len(), 1);
    assert!(plan.value_sources.iter().any(|source| {
        matches!(source, PlanValueSource::ExternalInput(value) if *value == plan.value_dependencies[0].source)
    }));
    assert!(!matches!(
        plan.validate(),
        Err(ref errors) if errors.0.iter().any(|error| matches!(error, crate::node_system::plan::PlanValidationError::MissingInputSource { .. }))
    ));
}

#[test]
fn valid_dynamic_output_derives_without_invalid_plan_fallback() {
    let dynamic_output = PortSpec {
        key: key("items"),
        label_key: I18nKey::new("ports.items.label").unwrap(),
        direction: PortDirection::Output,
        kind: PortKind::Data,
        value_type: TypeExpr::Unknown,
        instances: PortInstances::UserCreated { min: 0, max: None },
        connections: ConnectionsPerPort::Multiple {
            max: None,
            ordered: false,
        },
        input_binding: None,
        consumption: None,
        production: None,
        editor: PortEditorSpec::Default,
        schema: None,
    };
    let protocol = test_protocol(
        "demand_dynamic_output",
        vec![dynamic_output],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![protocol]);
    let mut graph = graph_with_nodes(&[(1, "demand_dynamic_output")]);
    let output = bind_member_port(&mut graph, 1, "items", 10, "a");
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/dynamic".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid dynamic graph has basis");
    let requested = GraphOutputRef {
        graph_path: GraphResourcePath("events/dynamic".into()),
        port: output,
    };

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([requested.clone()]),
            include_default_results: false,
        })
        .unwrap_or_else(|error| panic!("accepted dynamic output must derive directly: {error:?}"));
    assert_eq!(plan.results[0].output, requested);
    plan.validate()
        .expect("dynamic requested-output plan validates");
}

#[test]
fn evaluation_policy_and_effect_predecessors_are_authoritative_roots() {
    let mut predecessor = test_protocol(
        "demand_effect_predecessor",
        vec![effect_port("effect", PortDirection::Output)],
        vec![],
        vec![],
    );
    predecessor.execution.purity = Purity::Effectful;
    predecessor.execution.effects = EffectSemantics::Ordered;
    let mut middle = test_protocol(
        "demand_effect_middle",
        vec![
            effect_port("before", PortDirection::Input),
            effect_port("after", PortDirection::Output),
        ],
        vec![],
        vec![],
    );
    middle.execution.purity = Purity::Effectful;
    middle.execution.effects = EffectSemantics::Ordered;
    let mut eager = test_protocol(
        "demand_eager_pure",
        vec![effect_port("effect", PortDirection::Input)],
        vec![],
        vec![],
    );
    eager.execution.purity = Purity::Pure;
    eager.execution.evaluation = EvaluationPolicy::EagerWhenRegionEntered;
    eager.execution.effects = EffectSemantics::Ordered;
    let mut demand_driven_effectful =
        test_protocol("demand_disconnected_effectful", vec![], vec![], vec![]);
    demand_driven_effectful.execution.purity = Purity::Effectful;
    demand_driven_effectful.execution.effects = EffectSemantics::Ordered;
    let types = [
        predecessor.type_id.clone(),
        middle.type_id.clone(),
        eager.type_id.clone(),
        demand_driven_effectful.type_id.clone(),
    ];
    let mut registry = TestRegistry::new(vec![predecessor, middle, eager, demand_driven_effectful]);
    for node_type in &types {
        registry = registry.with_lowerer(
            node_type,
            FragmentLowerer {
                fragment: kernel_fragment(EffectSemantics::Ordered, FragmentMetadata::default()),
            },
        );
    }
    let mut graph = graph_with_nodes(&[
        (1, "demand_effect_predecessor"),
        (2, "demand_effect_middle"),
        (3, "demand_eager_pure"),
        (4, "demand_disconnected_effectful"),
    ]);
    connect(&mut graph, 10, 1, "effect", 2, "before");
    connect(&mut graph, 11, 2, "after", 3, "effect");
    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/main".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .expect("valid graph has lowering basis");

    let plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([]),
            include_default_results: false,
        })
        .unwrap();

    assert_eq!(
        plan.operations
            .iter()
            .map(|operation| operation.source_node_id)
            .collect::<Vec<_>>(),
        vec![node_id(1), node_id(2), node_id(3)]
    );
    assert_eq!(plan.effect_dependencies.len(), 2);
}

#[test]
fn compiler_derives_materialization_bridge_from_consumer_contract() {
    let mut source_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    source_output.production = Some(OutputProduction::Streaming);
    let source = test_protocol("plan_bridge_source", vec![source_output], vec![], vec![]);
    let mut sink_input = data_port("in", PortDirection::Input, TypeExpr::Unknown, None);
    sink_input.consumption = Some(InputConsumption::FullyMaterialized);
    let sink_output = data_port("out", PortDirection::Output, TypeExpr::Unknown, None);
    let sink = test_protocol(
        "plan_bridge_sink",
        vec![sink_input, sink_output],
        vec![],
        vec![],
    );
    let condition = test_protocol(
        "pruned_bridge_condition",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let mut then_source = data_port("then_source", PortDirection::Input, TypeExpr::Unknown, None);
    then_source.instances = PortInstances::UserCreated { min: 0, max: None };
    let mut else_source = data_port("else_source", PortDirection::Input, TypeExpr::Unknown, None);
    else_source.instances = PortInstances::UserCreated { min: 0, max: None };
    let mut branch_result = data_port("result", PortDirection::Output, TypeExpr::Unknown, None);
    branch_result.instances = PortInstances::UserCreated { min: 0, max: None };
    let mut branch = structural_protocol(
        "pruned_bridge_branch",
        vec![
            control_port("enter", PortDirection::Input),
            data_port("condition", PortDirection::Input, TypeExpr::Unknown, None),
            then_source,
            else_source,
            control_port("true", PortDirection::Output),
            control_port("false", PortDirection::Output),
            branch_result,
        ],
        vec![],
    );
    branch.interface = branch
        .interface
        .with_member_groups(vec![PortMemberGroupSpec {
            templates: vec![key("then_source"), key("else_source"), key("result")]
                .into_boxed_slice(),
            min: 0,
            max: None,
        }])
        .unwrap();
    let source_type = source.type_id.clone();
    let sink_type = sink.type_id.clone();
    let branch_type = branch.type_id.clone();
    let backend = RelationalBackendId::new("test.relational").unwrap();
    let registry = TestRegistry::new(vec![source, sink, condition, branch])
        .structural(&branch_type, StructuralNodeRole::Branch)
        .with_lowerer(
            &source_type,
            FragmentLowerer {
                fragment: LoweredKernel::Relational(RelationalNodeFragment {
                    backend: backend.clone(),
                    fragment: relational::RelationalFragment {
                        id: RelationalFragmentId::new("bridge-source").unwrap(),
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
            &sink_type,
            FragmentLowerer {
                fragment: LoweredKernel::Relational(RelationalNodeFragment {
                    backend,
                    fragment: relational::RelationalFragment {
                        id: RelationalFragmentId::new("bridge-sink").unwrap(),
                        operators: Box::new([RelationalOperator::Input {
                            name: "input".into(),
                        }]),
                        root: RelationalOperatorIndex::new(0),
                    },
                    inputs: Box::new([RelationalInputBinding {
                        port: PortAddress::declared(node_id(2), key("in")),
                        operator: RelationalOperatorIndex::new(0),
                    }]),
                    metadata: FragmentMetadata::default(),
                }),
            },
        );
    let mut graph = graph_with_nodes(&[
        (1, "plan_bridge_source"),
        (2, "plan_bridge_sink"),
        (3, "pruned_bridge_condition"),
        (4, "pruned_bridge_branch"),
    ]);
    connect(&mut graph, 10, 1, "out", 2, "in");
    connect(&mut graph, 11, 3, "out", 4, "condition");

    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/bridge-demand".into()), &graph);
    let result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let specialized = result
        .execution_basis
        .as_ref()
        .unwrap_or_else(|| panic!("bridge diagnostics: {:?}", result.analysis.diagnostics))
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/bridge-demand", 2, "out")]),
            include_default_results: false,
        })
        .expect("retained relational bridge specializes after structured pruning");
    assert_eq!(
        specialized
            .operations
            .iter()
            .map(|operation| operation.source_node_id)
            .collect::<Vec<_>>(),
        vec![node_id(1), node_id(2)],
        "the unrelated empty Branch and its pure condition are pruned before grouping"
    );
    assert_eq!(specialized.relational_subplans.len(), 2);
    let specialized_bridges = specialized
        .relational_subplans
        .iter()
        .flat_map(|subplan| subplan.materialization_bridges.iter())
        .collect::<Vec<_>>();
    assert_eq!(specialized_bridges.len(), 1);
    let producer =
        &specialized.relational_subplans[specialized_bridges[0].producer_subplan.index()];
    assert_eq!(
        producer.compiled_plan.requested_fragment_outputs.as_ref(),
        &[specialized_bridges[0].producer_fragment.clone()]
    );
    for operation in &specialized.operations {
        if let crate::node_system::plan::PlannedKernel::Relational(subplan) = operation.kernel {
            assert_eq!(
                operation.outputs.len(),
                specialized.relational_subplans[subplan.index()]
                    .compiled_plan
                    .roots
                    .len(),
                "relational owner outputs and compiled roots keep exact cardinality"
            );
        }
    }
    specialized.validate().unwrap();

    let mut reversed = graph_with_nodes(&[
        (4, "pruned_bridge_branch"),
        (3, "pruned_bridge_condition"),
        (2, "plan_bridge_sink"),
        (1, "plan_bridge_source"),
    ]);
    connect(&mut reversed, 11, 3, "out", 4, "condition");
    connect(&mut reversed, 10, 1, "out", 2, "in");
    let reversed_snapshot =
        compiler.snapshot(GraphResourcePath("events/bridge-demand".into()), &reversed);
    let reversed_result = compiler
        .compile_snapshot(&reversed_snapshot, &CompileCancellationToken::new())
        .unwrap();
    let reversed_specialized = reversed_result
        .execution_basis
        .expect("reversed bridge graph has a specialization basis")
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/bridge-demand", 2, "out")]),
            include_default_results: false,
        })
        .unwrap();
    assert_eq!(specialized.operations, reversed_specialized.operations);
    assert_eq!(
        specialized.relational_subplans,
        reversed_specialized.relational_subplans
    );
    assert_eq!(specialized.root_region, reversed_specialized.root_region);

    let plan = result.plan.expect("bridge graph should lower");

    assert_eq!(plan.relational_subplans.len(), 2);
    let bridges = plan
        .relational_subplans
        .iter()
        .flat_map(|subplan| subplan.materialization_bridges.iter())
        .collect::<Vec<_>>();
    assert_eq!(bridges.len(), 1);
    assert_eq!(bridges[0].bridge, MaterializationBridge::Collect);
    let bridge = bridges[0].clone();
    let producer = &plan.relational_subplans[bridge.producer_subplan.index()];
    assert_eq!(
        producer.compiled_plan.requested_fragment_outputs.as_ref(),
        &[bridge.producer_fragment.clone()]
    );
    let consumer = &plan.relational_subplans[bridge.consumer_subplan.index()];
    assert_eq!(
        consumer.compiled_plan.bridge_inputs.as_ref(),
        &[RelationalBridgeInput {
            operator: RelationalOperatorIndex::new(0),
            bridge,
        }]
    );
}

#[derive(Clone, Copy)]
enum FixtureInsertionOrder {
    Forward,
    Reverse,
}

fn in_fixture_order<T>(mut values: Vec<T>, order: FixtureInsertionOrder) -> Vec<T> {
    if matches!(order, FixtureInsertionOrder::Reverse) {
        values.reverse();
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
            value_type: TypeExpr::Unknown,
            default_value: None,
            constraints: vec![ParameterConstraint::Required],
            editor: ParameterEditorSpec::Auto,
        },
        ParameterSpec {
            key: ParameterKey::new("beta").unwrap(),
            title_key: I18nKey::new("parameters.beta.title").unwrap(),
            description_key: None,
            value_type: TypeExpr::Unknown,
            default_value: None,
            constraints: vec![ParameterConstraint::Required],
            editor: ParameterEditorSpec::Auto,
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

#[test]
fn semantically_identical_documents_serialize_identically() {
    let forward = determinism_fixture(FixtureInsertionOrder::Forward);
    let reverse = determinism_fixture(FixtureInsertionOrder::Reverse);

    assert_reversed_insertions(&forward.node_insertions, &reverse.node_insertions);
    assert_reversed_insertions(
        &forward.connection_insertions,
        &reverse.connection_insertions,
    );
    assert_reversed_insertions(&forward.parameter_insertions, &reverse.parameter_insertions);
    assert_reversed_insertions(
        &forward.port_binding_insertions,
        &reverse.port_binding_insertions,
    );
    assert_reversed_insertions(
        &forward.input_state_insertions,
        &reverse.input_state_insertions,
    );
    assert_eq!(forward.document, reverse.document);

    let registry = determinism_registry(determinism_protocols());
    let compiler = GraphCompiler::new(&registry, &Resources).with_observability(
        ProjectSessionId::new("determinism-session"),
        &NOOP_TRACE_SINK,
    );
    let graph_path = GraphResourcePath("events/determinism".into());
    let compile_id = CompileId::new(73);
    let forward_snapshot =
        compiler.snapshot_with_compile_id(compile_id, graph_path.clone(), &forward.document);
    let reverse_snapshot =
        compiler.snapshot_with_compile_id(compile_id, graph_path, &reverse.document);

    assert_eq!(forward_snapshot.provenance, reverse_snapshot.provenance);
    let forward_result = compiler
        .compile_snapshot(&forward_snapshot, &CompileCancellationToken::new())
        .expect("forward fixture should compile");
    let reverse_result = compiler
        .compile_snapshot(&reverse_snapshot, &CompileCancellationToken::new())
        .expect("reverse fixture should compile");

    assert_eq!(
        forward_result.analysis.diagnostics.as_ref(),
        reverse_result.analysis.diagnostics.as_ref()
    );
    assert!(
        forward_result.analysis.diagnostics.is_empty(),
        "fixture diagnostics: {:#?}",
        forward_result.analysis.diagnostics
    );
    assert_eq!(forward_result.analysis.nodes.len(), 4);
    assert_eq!(
        forward_result.analysis.nodes.as_ref(),
        reverse_result.analysis.nodes.as_ref()
    );
    assert_eq!(forward_result.analysis.resolved_interfaces.len(), 4);
    assert_eq!(
        forward_result.analysis.resolved_interfaces.as_ref(),
        reverse_result.analysis.resolved_interfaces.as_ref()
    );
    assert_eq!(forward_result.analysis.partial_types.len(), 7);
    assert_eq!(
        forward_result.analysis.partial_types,
        reverse_result.analysis.partial_types
    );
    assert!(
        forward_result.analysis.partial_schemas.is_empty(),
        "fixture protocols intentionally declare no schema expressions"
    );
    assert_eq!(
        forward_result.analysis.partial_schemas,
        reverse_result.analysis.partial_schemas
    );
    assert_eq!(
        serde_json::to_vec(&forward_result.analysis).unwrap(),
        serde_json::to_vec(&reverse_result.analysis).unwrap()
    );

    let forward_semantic = forward_result
        .semantic
        .expect("forward fixture should produce a semantic graph");
    let reverse_semantic = reverse_result
        .semantic
        .expect("reverse fixture should produce a semantic graph");
    assert_eq!(
        forward_semantic.dependencies.as_ref(),
        reverse_semantic.dependencies.as_ref()
    );
    assert_eq!(forward_semantic.dependencies.len(), 4);
    assert_eq!(
        serde_json::to_vec(&forward_semantic).unwrap(),
        serde_json::to_vec(&reverse_semantic).unwrap()
    );

    let forward_plan = forward_result
        .plan
        .expect("forward fixture should produce an execution plan");
    let reverse_plan = reverse_result
        .plan
        .expect("reverse fixture should produce an execution plan");
    assert_eq!(forward_plan.operations.len(), 3);
    assert_eq!(
        forward_plan.operations.as_ref(),
        reverse_plan.operations.as_ref()
    );
    assert_eq!(
        forward_plan.relational_subplans.as_ref(),
        reverse_plan.relational_subplans.as_ref()
    );
    assert_eq!(forward_plan.relational_subplans.len(), 2);
    assert!(
        forward_plan
            .relational_subplans
            .iter()
            .any(|subplan| subplan.compiled_plan.fragment_order.len() > 1)
    );
    assert!(
        forward_plan
            .relational_subplans
            .iter()
            .all(|subplan| !subplan.compiled_plan.fragment_roots.is_empty())
    );
    assert!(
        forward_plan
            .relational_subplans
            .iter()
            .any(|subplan| !subplan.compiled_plan.bridge_inputs.is_empty())
    );
    assert!(
        forward_plan
            .relational_subplans
            .iter()
            .any(|subplan| !subplan.materialization_bridges.is_empty())
    );
    assert!(
        forward_plan
            .relational_subplans
            .iter()
            .any(|subplan| { !subplan.compiled_plan.requested_fragment_outputs.is_empty() })
    );
    for (forward_subplan, reverse_subplan) in forward_plan
        .relational_subplans
        .iter()
        .zip(reverse_plan.relational_subplans.iter())
    {
        assert_eq!(
            forward_subplan.compiled_plan.fragment_order,
            reverse_subplan.compiled_plan.fragment_order
        );
        assert_eq!(
            forward_subplan.compiled_plan.fragment_roots,
            reverse_subplan.compiled_plan.fragment_roots
        );
        assert_eq!(
            forward_subplan.compiled_plan.roots,
            reverse_subplan.compiled_plan.roots
        );
        assert_eq!(
            forward_subplan.compiled_plan.bridge_inputs,
            reverse_subplan.compiled_plan.bridge_inputs
        );
        assert_eq!(
            forward_subplan.materialization_bridges,
            reverse_subplan.materialization_bridges
        );
        assert_eq!(
            forward_subplan.compiled_plan.requested_fragment_outputs,
            reverse_subplan.compiled_plan.requested_fragment_outputs
        );
    }
    assert_eq!(
        serde_json::to_vec(&forward_plan).unwrap(),
        serde_json::to_vec(&reverse_plan).unwrap()
    );
}

#[test]
fn generic_binding_is_written_to_type_facts() {
    let generic = TypeParameterId::new("value").unwrap();
    let source = test_protocol(
        "int_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Concrete(type_id("core.int")),
            None,
        )],
        vec![],
        vec![],
    );
    let passthrough = test_protocol(
        "generic",
        vec![
            data_port(
                "in",
                PortDirection::Input,
                TypeExpr::Generic(generic.clone()),
                None,
            ),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Generic(generic),
                None,
            ),
        ],
        vec![TypeParameterId::new("value").unwrap()],
        vec![TypeConstraint::Equal(
            TypeTerm::Port(key("in")),
            TypeTerm::Port(key("out")),
        )],
    );
    let registry = TestRegistry::new(vec![source, passthrough]);
    let mut graph = graph_with_nodes(&[(1, "int_source"), (2, "generic")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(result.plan.is_some());
    assert_eq!(
        result
            .analysis
            .partial_types
            .get(&PortAddress::declared(node_id(2), key("out"))),
        Some(&TypeExpr::Concrete(type_id("core.int")))
    );
}

#[test]
fn implements_and_element_of_solve_registered_type_shapes() {
    let numeric = TypeClassId::new("core.numeric").unwrap();
    let iterable = TypeClassId::new("core.iterable").unwrap();
    let integer = type_id("core.int");
    let float = type_id("core.float");
    let list = TypeConstructorId::new("core.list").unwrap();
    let pair = TypeConstructorId::new("core.pair").unwrap();
    let generic = TypeParameterId::new("item").unwrap();
    let protocol = test_protocol(
        "type_constraints",
        vec![
            data_port(
                "concrete",
                PortDirection::Output,
                TypeExpr::Concrete(integer.clone()),
                None,
            ),
            data_port(
                "generic",
                PortDirection::Output,
                TypeExpr::Generic(generic.clone()),
                None,
            ),
            data_port(
                "applied",
                PortDirection::Output,
                TypeExpr::Applied {
                    constructor: list.clone(),
                    arguments: vec![TypeExpr::Concrete(integer.clone())],
                },
                None,
            ),
            data_port(
                "union",
                PortDirection::Output,
                TypeExpr::Union(vec![
                    TypeExpr::Concrete(integer.clone()),
                    TypeExpr::Concrete(float.clone()),
                ]),
                None,
            ),
            data_port(
                "pair",
                PortDirection::Output,
                TypeExpr::Applied {
                    constructor: pair.clone(),
                    arguments: vec![
                        TypeExpr::Concrete(integer.clone()),
                        TypeExpr::Concrete(float.clone()),
                    ],
                },
                None,
            ),
        ],
        vec![generic.clone()],
        vec![
            TypeConstraint::Equal(
                TypeTerm::Expr(TypeExpr::Generic(generic)),
                TypeTerm::Expr(TypeExpr::Concrete(integer.clone())),
            ),
            TypeConstraint::Implements(TypeTerm::Port(key("concrete")), numeric.clone()),
            TypeConstraint::Implements(TypeTerm::Port(key("generic")), numeric.clone()),
            TypeConstraint::Implements(TypeTerm::Port(key("applied")), iterable.clone()),
            TypeConstraint::Implements(TypeTerm::Port(key("union")), numeric.clone()),
            TypeConstraint::ElementOf(
                TypeTerm::Port(key("concrete")),
                TypeTerm::Port(key("applied")),
            ),
            TypeConstraint::ElementOf(TypeTerm::Port(key("union")), TypeTerm::Port(key("pair"))),
        ],
    );
    let registry = TestRegistry::new(vec![protocol])
        .with_type_class(integer, numeric.clone())
        .with_type_class(float, numeric)
        .with_constructor(list, 1, [iterable])
        .with_constructor(pair, 2, []);

    let result = GraphCompiler::new(&registry, &Resources)
        .compile(&graph_with_nodes(&[(1, "type_constraints")]));

    assert!(result.plan.is_some(), "{:?}", result.analysis.diagnostics);
    assert!(result.analysis.diagnostics.is_empty());
}

#[test]
fn incompatible_types_are_blocking_and_produce_no_plan() {
    let source = test_protocol(
        "int_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Concrete(type_id("core.int")),
            None,
        )],
        vec![],
        vec![],
    );
    let sink = test_protocol(
        "string_sink",
        vec![data_port(
            "in",
            PortDirection::Input,
            TypeExpr::Concrete(type_id("core.string")),
            None,
        )],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source, sink]);
    let mut graph = graph_with_nodes(&[(1, "int_source"), (2, "string_sink")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(result.plan.is_none());
    assert!(
        result
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "compiler.type.incompatible")
    );
}

#[test]
fn value_dependency_cycle_is_blocking() {
    let relay = test_protocol(
        "relay",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port("out", PortDirection::Output, TypeExpr::Unknown, None),
        ],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![relay]);
    let mut graph = graph_with_nodes(&[(1, "relay"), (2, "relay")]);
    connect(&mut graph, 10, 1, "out", 2, "in");
    connect(&mut graph, 11, 2, "out", 1, "in");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(result.plan.is_none());
    assert!(
        result
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code.as_str() == "compiler.dependency.value_cycle" })
    );
}

struct SourceSchemaResolver;
impl SchemaResolver for SourceSchemaResolver {
    fn resolve(
        &self,
        _: &SchemaResolutionContext<'_>,
    ) -> Result<SchemaFact, SchemaResolutionError> {
        Ok(SchemaFact::new(
            SchemaExpr::Input(key("raw")),
            [
                SchemaField {
                    name: SchemaColumnRef("a".into()),
                    scalar_type: RelationalScalarType::Int64,
                },
                SchemaField {
                    name: SchemaColumnRef("b".into()),
                    scalar_type: RelationalScalarType::String,
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
    GraphCompiler::with_schema_resolvers(&registry, &Resources, resolvers).compile(&graph)
}

fn rename_diagnostic_codes(result: &CompileResult) -> BTreeSet<&str> {
    result
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect()
}

#[test]
fn rename_dataframe_configured_builtin_reaches_relational_plan() {
    let result = compile_builtin_rename(serde_json::json!("a"), serde_json::json!("renamed"), true);

    assert!(
        result.semantic.is_some(),
        "{:?}",
        result.analysis.diagnostics
    );
    let plan = result.plan.expect("configured Rename must lower");
    assert!(result.analysis.diagnostics.is_empty());
    assert_eq!(
        result
            .analysis
            .partial_schemas
            .get(&PortAddress::declared(node_id(2), key("result"))),
        Some(&SchemaExpr::Rename {
            input: Box::new(SchemaExpr::Input(key("raw"))),
            mapping: RenameExpr::Explicit(vec![ColumnRename {
                from: SchemaColumnRef("a".into()),
                to: SchemaColumnRef("renamed".into()),
            }]),
        })
    );
    assert_eq!(plan.relational_subplans.len(), 1);
    assert_eq!(
        plan.relational_subplans[0].compiled_plan.operators.as_ref(),
        [
            RelationalOperator::Source {
                resource: ResourceId::new("databases/main").unwrap(),
                relation: "databases/main".into(),
            },
            RelationalOperator::Rename {
                input: RelationalOperatorIndex::new(0),
                columns: Box::new([crate::node_system::plan::RelationalRename {
                    from: "a".into(),
                    to: "renamed".into(),
                }]),
            },
        ]
    );
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
    let compiler = GraphCompiler::with_schema_resolvers(&registry, &Resources, resolvers);
    let snapshot = compiler.snapshot(GraphResourcePath(graph_path.into()), &graph);
    compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap()
}

#[test]
fn builtin_relational_chain_specializes_final_and_intermediate_demands() {
    let graph_path = "events/task-3-fix-round-1";
    let result = compile_builtin_relational_chain(graph_path);
    assert!(
        result.analysis.diagnostics.is_empty(),
        "{:?}",
        result.analysis.diagnostics
    );
    let basis = result.execution_basis.expect("chain keeps demand basis");
    assert_eq!(
        basis.provenance.graph_path,
        GraphResourcePath(graph_path.into())
    );
    assert!(!basis.provenance.basis.resource_versions.is_empty());

    let final_plan = basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output(graph_path, 5, "result")]),
            include_default_results: false,
        })
        .unwrap();
    assert_eq!(final_plan.operations.len(), 1);
    assert_eq!(final_plan.relational_subplans.len(), 1);
    assert!(
        final_plan.relational_subplans[0]
            .materialization_bridges
            .is_empty()
    );
    let final_relational = &final_plan.relational_subplans[0].compiled_plan;
    assert_eq!(final_relational.operators.len(), 5);
    assert_eq!(
        final_relational.roots.as_ref(),
        [RelationalOperatorIndex::new(4)]
    );
    assert_eq!(
        final_relational.fragment_order.as_ref(),
        [1_u128, 2, 3, 4, 5]
            .map(|id| RelationalFragmentId::new(format!("node.{}", node_id(id))).unwrap())
            .as_slice()
    );
    assert_eq!(
        final_relational.pushdown_hints.as_ref(),
        [
            RelationalPushdownHint::Projection {
                source: RelationalOperatorIndex::new(0),
                columns: Box::new(["a".into(), "b".into()]),
            },
            RelationalPushdownHint::Predicate {
                source: RelationalOperatorIndex::new(0),
                predicate: RelationalExpression::Equal(
                    Box::new(RelationalExpression::Column("b".into())),
                    Box::new(RelationalExpression::Literal(RelationalLiteral::String(
                        "paid".into(),
                    ))),
                ),
            },
        ]
    );
    final_plan.validate().unwrap();

    for (node, expected_fragments, expected_operators) in [(2, 2, 2), (3, 3, 3)] {
        let selected_output = demand_output(graph_path, node, "result");
        let plan = basis
            .derive_plan(&ExecutionDemand::Outputs {
                outputs: Box::new([selected_output.clone()]),
                include_default_results: false,
            })
            .unwrap();
        assert_eq!(plan.operations.len(), 1);
        assert_eq!(plan.results.len(), 1);
        assert_eq!(plan.results[0].output, selected_output);
        assert_eq!(plan.results[0].output.graph_path.0.as_ref(), graph_path);
        assert_eq!(
            plan.results[0].output.port,
            PortAddress::declared(node_id(node), key("result"))
        );
        let relational = &plan.relational_subplans[0].compiled_plan;
        let expected_prefix = (1..=node)
            .map(|id| RelationalFragmentId::new(format!("node.{}", node_id(id))).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(relational.fragment_order.as_ref(), expected_prefix);
        assert_eq!(relational.operators.len(), expected_operators);
        assert_eq!(relational.roots.len(), 1);
        for suffix_node in (node + 1)..=5 {
            let suffix =
                RelationalFragmentId::new(format!("node.{}", node_id(suffix_node))).unwrap();
            assert!(!relational.fragment_order.contains(&suffix));
            assert!(
                relational
                    .fragment_roots
                    .iter()
                    .all(|root| root.fragment != suffix)
            );
        }
        assert_eq!(relational.fragment_order.len(), expected_fragments);
        plan.validate().unwrap();
    }
}

#[test]
fn rename_dataframe_rejects_invalid_scalar_parameter_types() {
    for (from, to, expected_key) in [
        (serde_json::json!(1), serde_json::json!("renamed"), "from"),
        (serde_json::json!("a"), serde_json::json!(false), "to"),
    ] {
        let result = compile_builtin_rename(from, to, true);
        assert!(result.plan.is_none());
        assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
            matches!(
                &diagnostic.primary,
                crate::node_system::analysis::DiagnosticLocation::Parameter { node_id: id, key }
                    if *id == node_id(2) && key.as_str() == expected_key
            )
        }));
    }
}

#[test]
fn rename_dataframe_rejects_empty_or_whitespace_padded_names() {
    for (from, to) in [
        ("", "renamed"),
        ("a", ""),
        (" a", "renamed"),
        ("a", "renamed "),
    ] {
        let result = compile_builtin_rename(serde_json::json!(from), serde_json::json!(to), true);
        assert!(result.plan.is_none(), "{from:?} -> {to:?}");
        assert!(
            rename_diagnostic_codes(&result).contains("compiler.schema.parameter_invalid"),
            "{:?}",
            result.analysis.diagnostics
        );
    }
}

#[test]
fn rename_dataframe_rejects_missing_source_and_destination_collision() {
    for (from, to, code) in [
        ("missing", "renamed", "compiler.schema.rename_field_missing"),
        ("a", "b", "compiler.schema.rename_target_conflict"),
    ] {
        let result = compile_builtin_rename(serde_json::json!(from), serde_json::json!(to), true);
        assert!(result.plan.is_none());
        assert!(
            rename_diagnostic_codes(&result).contains(code),
            "{:?}",
            result.analysis.diagnostics
        );
    }
}

#[test]
fn rename_dataframe_same_name_is_schema_noop_but_still_lowers() {
    let result = compile_builtin_rename(serde_json::json!("a"), serde_json::json!("a"), true);

    assert!(result.plan.is_some(), "{:?}", result.analysis.diagnostics);
    assert!(result.analysis.diagnostics.is_empty());
    assert_eq!(
        result
            .analysis
            .partial_schemas
            .get(&PortAddress::declared(node_id(2), key("result"))),
        Some(&SchemaExpr::Input(key("raw")))
    );
}

#[test]
fn rename_dataframe_unknown_input_schema_blocks_with_existing_diagnostic() {
    let result =
        compile_builtin_rename(serde_json::json!("a"), serde_json::json!("renamed"), false);

    assert!(result.plan.is_none());
    assert!(
        rename_diagnostic_codes(&result).contains("compiler.schema.resolver_missing"),
        "{:?}",
        result.analysis.diagnostics
    );
}

#[test]
fn schema_filter_project_and_rename_are_evaluated_into_facts() {
    let resolver_id = SchemaResolverId::new("test.source_schema").unwrap();
    let source = test_protocol(
        "schema_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            Some(SchemaExpr::Derived {
                resolver: resolver_id.clone(),
                dependencies: vec![],
            }),
        )],
        vec![],
        vec![],
    );
    let mut filter = test_protocol(
        "schema_filter",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Unknown,
                Some(SchemaExpr::Filter {
                    input: Box::new(SchemaExpr::Input(key("in"))),
                    predicate: Some(ParameterKey::new("predicate").unwrap()),
                }),
            ),
        ],
        vec![],
        vec![],
    );
    filter.parameters = ParameterSchema::new(vec![ParameterSpec {
        key: ParameterKey::new("predicate").unwrap(),
        title_key: I18nKey::new("parameters.predicate.title").unwrap(),
        description_key: None,
        value_type: TypeExpr::Unknown,
        default_value: None,
        constraints: vec![ParameterConstraint::Required],
        editor: ParameterEditorSpec::Auto,
    }])
    .unwrap();
    let project = test_protocol(
        "schema_project",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Unknown,
                Some(SchemaExpr::Project {
                    input: Box::new(SchemaExpr::Input(key("in"))),
                    columns: ColumnSelectionExpr::Explicit(vec![SchemaColumnRef("a".into())]),
                }),
            ),
        ],
        vec![],
        vec![],
    );
    let rename = test_protocol(
        "schema_rename",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Unknown,
                Some(SchemaExpr::Rename {
                    input: Box::new(SchemaExpr::Input(key("in"))),
                    mapping: RenameExpr::Explicit(vec![ColumnRename {
                        from: SchemaColumnRef("a".into()),
                        to: SchemaColumnRef("renamed".into()),
                    }]),
                }),
            ),
        ],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source, filter, project, rename]);
    let mut resolvers = SchemaResolverSet::new();
    resolvers.insert(resolver_id, SourceSchemaResolver);
    let mut graph = graph_with_nodes(&[
        (1, "schema_source"),
        (2, "schema_filter"),
        (3, "schema_project"),
        (4, "schema_rename"),
    ]);
    graph.nodes.get_mut(&node_id(2)).unwrap().parameters.insert(
        ParameterKey::new("predicate").unwrap(),
        serde_json::json!({
            "column": "a",
            "operator": "greaterThan",
            "value": {"type": "integer", "value": "0"}
        }),
    );
    connect(&mut graph, 10, 1, "out", 2, "in");
    connect(&mut graph, 11, 2, "out", 3, "in");
    connect(&mut graph, 12, 3, "out", 4, "in");

    let result =
        GraphCompiler::with_schema_resolvers(&registry, &Resources, resolvers).compile(&graph);

    assert!(result.plan.is_some(), "{:?}", result.analysis.diagnostics);
    let source_fact = SchemaExpr::Input(key("raw"));
    let filtered = SchemaExpr::Filter {
        input: Box::new(source_fact),
        predicate: Some(ParameterKey::new("predicate").unwrap()),
    };
    assert_eq!(
        result
            .analysis
            .partial_schemas
            .get(&PortAddress::declared(node_id(2), key("out"))),
        Some(&filtered)
    );
    let projected = SchemaExpr::Project {
        input: Box::new(filtered),
        columns: ColumnSelectionExpr::Explicit(vec![SchemaColumnRef("a".into())]),
    };
    assert_eq!(
        result
            .analysis
            .partial_schemas
            .get(&PortAddress::declared(node_id(3), key("out"))),
        Some(&projected)
    );
    assert_eq!(
        result
            .analysis
            .partial_schemas
            .get(&PortAddress::declared(node_id(4), key("out"))),
        Some(&SchemaExpr::Rename {
            input: Box::new(projected),
            mapping: RenameExpr::Explicit(vec![ColumnRename {
                from: SchemaColumnRef("a".into()),
                to: SchemaColumnRef("renamed".into()),
            }]),
        })
    );
    let output = PortAddress::declared(node_id(4), key("out"));
    let expected_fields = vec![SchemaField {
        name: SchemaColumnRef("renamed".into()),
        scalar_type: RelationalScalarType::Int64,
    }];
    assert_eq!(
        result.analysis.resolved_schemas[&output].fields,
        expected_fields
    );
    assert_eq!(
        result.semantic.as_ref().unwrap().resolved_schemas[&output].fields,
        expected_fields
    );
}

#[test]
fn explicit_project_reports_missing_and_duplicate_columns() {
    let resolver_id = SchemaResolverId::new("test.project_source_schema").unwrap();
    let source = test_protocol(
        "project_schema_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            Some(SchemaExpr::Derived {
                resolver: resolver_id.clone(),
                dependencies: vec![],
            }),
        )],
        vec![],
        vec![],
    );
    let project = test_protocol(
        "invalid_schema_project",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Unknown,
                Some(SchemaExpr::Project {
                    input: Box::new(SchemaExpr::Input(key("in"))),
                    columns: ColumnSelectionExpr::Explicit(vec![
                        SchemaColumnRef("missing".into()),
                        SchemaColumnRef("a".into()),
                        SchemaColumnRef("a".into()),
                    ]),
                }),
            ),
        ],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source, project]);
    let mut resolvers = SchemaResolverSet::new();
    resolvers.insert(resolver_id, SourceSchemaResolver);
    let mut graph =
        graph_with_nodes(&[(1, "project_schema_source"), (2, "invalid_schema_project")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let result =
        GraphCompiler::with_schema_resolvers(&registry, &Resources, resolvers).compile(&graph);
    let codes = result
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<BTreeSet<_>>();

    assert!(result.plan.is_none());
    assert!(codes.contains("compiler.schema.project_field_missing"));
    assert!(codes.contains("compiler.schema.project_field_duplicate"));
}

#[test]
fn explicit_rename_reports_missing_duplicate_and_conflicting_fields() {
    let resolver_id = SchemaResolverId::new("test.rename_source_schema").unwrap();
    let source = test_protocol(
        "rename_schema_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            Some(SchemaExpr::Derived {
                resolver: resolver_id.clone(),
                dependencies: vec![],
            }),
        )],
        vec![],
        vec![],
    );
    let rename = test_protocol(
        "invalid_schema_rename",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Unknown,
                Some(SchemaExpr::Rename {
                    input: Box::new(SchemaExpr::Input(key("in"))),
                    mapping: RenameExpr::Explicit(vec![
                        ColumnRename {
                            from: SchemaColumnRef("missing".into()),
                            to: SchemaColumnRef("x".into()),
                        },
                        ColumnRename {
                            from: SchemaColumnRef("a".into()),
                            to: SchemaColumnRef("b".into()),
                        },
                        ColumnRename {
                            from: SchemaColumnRef("a".into()),
                            to: SchemaColumnRef("x".into()),
                        },
                    ]),
                }),
            ),
        ],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![source, rename]);
    let mut resolvers = SchemaResolverSet::new();
    resolvers.insert(resolver_id, SourceSchemaResolver);
    let mut graph = graph_with_nodes(&[(1, "rename_schema_source"), (2, "invalid_schema_rename")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let result =
        GraphCompiler::with_schema_resolvers(&registry, &Resources, resolvers).compile(&graph);
    let codes = result
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<BTreeSet<_>>();

    assert!(result.plan.is_none());
    assert!(codes.contains("compiler.schema.rename_field_missing"));
    assert!(codes.contains("compiler.schema.rename_source_duplicate"));
    assert!(codes.contains("compiler.schema.rename_target_conflict"));
}

#[test]
fn resolver_error_analysis_has_no_plan() {
    let protocol = test_protocol(
        "unresolved_schema",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            Some(SchemaExpr::Derived {
                resolver: SchemaResolverId::new("test.missing_schema").unwrap(),
                dependencies: vec![],
            }),
        )],
        vec![],
        vec![],
    );
    let registry = TestRegistry::new(vec![protocol]);
    let graph = graph_with_nodes(&[(1, "unresolved_schema")]);

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(result.plan.is_none());
    assert!(
        result
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code.as_str() == "compiler.schema.resolver_missing" })
    );
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

#[test]
fn sequence_builds_an_ordered_control_region() {
    let sequence = structural_protocol(
        "sequence",
        vec![
            control_port("enter", PortDirection::Input),
            control_port("then", PortDirection::Output),
        ],
        vec![],
    );
    let leaf = test_protocol(
        "controlled_leaf",
        vec![control_port("enter", PortDirection::Input)],
        vec![],
        vec![],
    );
    let sequence_type = sequence.type_id.clone();
    let registry = TestRegistry::new(vec![sequence, leaf])
        .structural(&sequence_type, StructuralNodeRole::Sequence);
    let mut graph = graph_with_nodes(&[(1, "sequence"), (2, "controlled_leaf")]);
    connect(&mut graph, 10, 1, "then", 2, "enter");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    let plan = result.plan.expect("sequence should produce a plan");
    assert_eq!(plan.operations.len(), 1);
    assert!(contains_region(&plan.root_region, &|region| matches!(
        region,
        crate::node_system::plan::StructuredControlRegion::Sequence(steps)
            if steps.iter().any(|step| matches!(step, crate::node_system::plan::ControlStep::Operation(_)))
    )));
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

#[test]
fn builtin_multi_output_sequence_outside_branch_keeps_walker_order() {
    use crate::node_system::catalog::build_builtin_node_system;

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.control.sequence"),
        (2, "yssbi.control.do"),
        (3, "yssbi.control.do"),
    ]);
    let first = bind_member_port(&mut graph, 1, "then", 10, "a");
    let second = bind_member_port(&mut graph, 1, "then", 11, "z");
    connect_addresses(
        &mut graph,
        100,
        first,
        PortAddress::declared(node_id(2), key("enter")),
    );
    connect_addresses(
        &mut graph,
        101,
        second,
        PortAddress::declared(node_id(3), key("enter")),
    );

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result
        .plan
        .unwrap_or_else(|| panic!("sequence diagnostics: {:?}", result.analysis.diagnostics));

    assert_eq!(plan.operations.len(), 2);
    let first = operation_index_for_node(&plan, 2);
    let second = operation_index_for_node(&plan, 3);
    let StructuredControlRegion::Sequence(steps) = &plan.root_region else {
        panic!("expected root sequence, got {:?}", plan.root_region);
    };
    let first_position = steps
        .iter()
        .position(|step| matches!(step, ControlStep::Operation(operation) if *operation == first))
        .expect("first Sequence output operation");
    let second_position = steps
        .iter()
        .position(|step| matches!(step, ControlStep::Operation(operation) if *operation == second))
        .expect("second Sequence output operation");
    assert!(first_position < second_position);
}

#[test]
fn print_protocol_default_lowers_to_effective_runtime_binding() {
    use crate::node_system::catalog::build_builtin_node_system;

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let graph = builtin_graph_with_nodes(&[(1, "yssbi.debug.print")]);

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result
        .plan
        .unwrap_or_else(|| panic!("print diagnostics: {:?}", result.analysis.diagnostics));
    let print = &plan.operations[operation_index_for_node(&plan, 1).index()];

    assert_eq!(print.inputs.len(), 1);
    assert_eq!(
        print.inputs[0].bound_value,
        Some(Value::String("Hello, World!".into()))
    );
    plan.validate().unwrap();
}

#[test]
fn branch_builds_exclusive_true_and_false_regions() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.int64"),
        (3, "yssbi.constant.int64"),
        (4, "yssbi.control.branch"),
        (5, "yssbi.control.do"),
        (6, "yssbi.control.do"),
        (7, "yssbi.control.merge"),
        (8, "yssbi.debug.view"),
        (9, "yssbi.constant.int64"),
    ]);
    let then_source = bind_member_port(&mut graph, 4, "then_source", 40, "z");
    let else_source = bind_member_port(&mut graph, 4, "else_source", 40, "a");
    let result_port = bind_member_port(&mut graph, 4, "result", 40, "m");
    let demanded_result = result_port.clone();
    let merge_true = bind_member_port(&mut graph, 7, "enter", 70, "z");
    let merge_false = bind_member_port(&mut graph, 7, "enter", 71, "a");

    connect(&mut graph, 100, 1, "value", 4, "condition");
    connect_addresses(
        &mut graph,
        101,
        PortAddress::declared(node_id(2), key("value")),
        then_source,
    );
    connect_addresses(
        &mut graph,
        102,
        PortAddress::declared(node_id(3), key("value")),
        else_source,
    );
    connect_addresses(
        &mut graph,
        103,
        result_port,
        PortAddress::declared(node_id(8), key("data")),
    );
    connect(&mut graph, 104, 4, "true", 5, "enter");
    connect_addresses(
        &mut graph,
        105,
        PortAddress::declared(node_id(5), key("then")),
        merge_true,
    );
    connect(&mut graph, 106, 4, "false", 6, "enter");
    connect_addresses(
        &mut graph,
        107,
        PortAddress::declared(node_id(6), key("then")),
        merge_false,
    );
    connect(&mut graph, 108, 7, "then", 8, "enter");

    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/branch-demand".into()), &graph);
    let mut result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let basis = result
        .execution_basis
        .as_mut()
        .unwrap_or_else(|| panic!("branch diagnostics: {:?}", result.analysis.diagnostics));
    let branch_local_pure = basis
        .operations
        .iter_mut()
        .find(|operation| operation.source_node_id == node_id(5))
        .expect("true arm operation is present in the basis");
    branch_local_pure.evaluation = EvaluationPolicy::DemandDriven;
    branch_local_pure.purity = Purity::Pure;
    branch_local_pure.effects = EffectSemantics::None;

    fn branch_results_mut(
        region: &mut StructuredControlRegion,
    ) -> Option<&mut Box<[crate::node_system::plan::BranchResultBinding]>> {
        match region {
            StructuredControlRegion::Sequence(steps) => {
                steps.iter_mut().find_map(|step| match step {
                    ControlStep::Operation(_) => None,
                    ControlStep::Region(region) => branch_results_mut(region),
                })
            }
            StructuredControlRegion::If { results, .. } => Some(results),
            StructuredControlRegion::Loop { body, .. } => branch_results_mut(body),
            StructuredControlRegion::Call { .. } => None,
        }
    }
    let deleted_result_value = ValueRef::new(basis.value_count);
    basis.value_count += 1;
    let results = branch_results_mut(&mut basis.root_region).expect("Branch result bindings");
    let mut bindings = results.to_vec();
    let mut deleted_binding = bindings[0];
    deleted_binding.destination = deleted_result_value;
    bindings.push(deleted_binding);
    *results = bindings.into_boxed_slice();
    let mut value_sources = basis.value_sources.to_vec();
    value_sources.push(PlanValueSource::ControlProduced(deleted_result_value));
    basis.value_sources = value_sources.into_boxed_slice();

    let mut disconnected_basis = basis.clone();
    for operation in &mut disconnected_basis.operations {
        if [node_id(6), node_id(8)].contains(&operation.source_node_id) {
            operation.evaluation = EvaluationPolicy::DemandDriven;
            operation.purity = Purity::Pure;
            operation.effects = EffectSemantics::None;
        }
    }
    let disconnected = disconnected_basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/branch-demand", 9, "value")]),
            include_default_results: false,
        })
        .expect("a disconnected If with results is deleted without orphan declarations");
    assert!(!contains_region(
        &disconnected.root_region,
        &|region| matches!(region, StructuredControlRegion::If { .. })
    ));
    assert!(
        disconnected
            .value_sources
            .iter()
            .all(|source| !matches!(source, PlanValueSource::ControlProduced(_)))
    );
    disconnected.validate().unwrap();

    let specialized = result
        .execution_basis
        .as_ref()
        .unwrap()
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([GraphOutputRef {
                graph_path: snapshot.provenance.graph_path.clone(),
                port: demanded_result.clone(),
            }]),
            include_default_results: false,
        })
        .expect("demanded Branch result safely specializes");
    let specialized_nodes = specialized
        .operations
        .iter()
        .map(|operation| operation.source_node_id)
        .collect::<BTreeSet<_>>();
    for pruned in [9] {
        assert!(!specialized_nodes.contains(&node_id(pruned)));
    }
    assert!(
        !specialized_nodes.contains(&node_id(5)),
        "unrequested arm-local pure work is pruned"
    );
    for retained in [1, 2, 3, 6] {
        assert!(
            specialized_nodes.contains(&node_id(retained)),
            "Branch condition, both result sources, and arm-local eager work stay retained"
        );
    }
    let retained_else_eager = operation_index_for_node(&specialized, 6);
    assert!(contains_region(
        &specialized.root_region,
        &|region| matches!(
            region,
            StructuredControlRegion::If {
                then_region,
                else_region,
                results,
                ..
            } if results.len() == 1
                && matches!(then_region.as_ref(), StructuredControlRegion::Sequence(steps) if steps.is_empty())
                && region_contains_operation(else_region, retained_else_eager)
        )
    ));
    specialized.validate().unwrap();

    let basis = result.execution_basis.as_ref().unwrap();
    let retained_result_value = basis.output_results[&GraphOutputRef {
        graph_path: snapshot.provenance.graph_path.clone(),
        port: demanded_result,
    }]
        .value;

    assert!(
        specialized
            .value_sources
            .contains(&PlanValueSource::ControlProduced(retained_result_value))
    );
    assert!(
        !specialized
            .value_sources
            .contains(&PlanValueSource::ControlProduced(deleted_result_value))
    );

    let plan = result
        .plan
        .unwrap_or_else(|| panic!("branch diagnostics: {:?}", result.analysis.diagnostics));
    let then_operation = operation_index_for_node(&plan, 5);
    let else_operation = operation_index_for_node(&plan, 6);
    let continuation = operation_index_for_node(&plan, 8);
    let then_output = plan.operations[operation_index_for_node(&plan, 2).index()].outputs[0].value;
    let else_output = plan.operations[operation_index_for_node(&plan, 3).index()].outputs[0].value;
    let continuation_input = plan.operations[continuation.index()].inputs[0].value;
    let then_binding_source = plan
        .value_dependencies
        .iter()
        .find(|dependency| dependency.source == then_output)
        .expect("then constant must feed its exact member")
        .destination;
    let else_binding_source = plan
        .value_dependencies
        .iter()
        .find(|dependency| dependency.source == else_output)
        .expect("else constant must feed its exact member")
        .destination;
    let branch_destination = plan
        .value_dependencies
        .iter()
        .find(|dependency| dependency.destination == continuation_input)
        .expect("branch result must feed the continuation")
        .source;

    let root_steps = match &plan.root_region {
        StructuredControlRegion::Sequence(steps) => steps,
        other => panic!("expected root sequence, got {other:?}"),
    };
    let (branch_index, branch_region) = root_steps
        .iter()
        .enumerate()
        .find_map(|(index, step)| match step {
            ControlStep::Region(region)
                if matches!(region.as_ref(), StructuredControlRegion::If { .. }) =>
            {
                Some((index, region.as_ref()))
            }
            _ => None,
        })
        .expect("root sequence must contain Branch");
    let StructuredControlRegion::If {
        then_region,
        else_region,
        results,
        ..
    } = branch_region
    else {
        unreachable!()
    };
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].destination, branch_destination);
    assert_eq!(results[0].then_source, then_binding_source);
    assert_eq!(results[0].else_source, else_binding_source);
    assert!(region_contains_operation(then_region, then_operation));
    assert!(!region_contains_operation(then_region, else_operation));
    assert!(region_contains_operation(else_region, else_operation));
    assert!(!region_contains_operation(else_region, then_operation));
    assert!(!region_contains_operation(then_region, continuation));
    assert!(!region_contains_operation(else_region, continuation));
    assert!(root_steps[branch_index + 1..].iter().any(
        |step| matches!(step, ControlStep::Operation(operation) if *operation == continuation)
    ));
}

#[test]
fn nested_branch_with_one_terminating_arm_blocks_unstructured_continuation() {
    use crate::node_system::catalog::build_builtin_node_system;

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.bool"),
        (3, "yssbi.control.branch"),
        (4, "yssbi.control.branch"),
        (5, "yssbi.control.merge"),
        (6, "yssbi.control.do"),
    ]);
    let merge_inner_true = bind_member_port(&mut graph, 5, "enter", 50, "z");
    let merge_outer_false = bind_member_port(&mut graph, 5, "enter", 51, "a");
    connect(&mut graph, 100, 1, "value", 3, "condition");
    connect(&mut graph, 101, 2, "value", 4, "condition");
    connect(&mut graph, 102, 3, "true", 4, "enter");
    connect_addresses(
        &mut graph,
        103,
        PortAddress::declared(node_id(4), key("true")),
        merge_inner_true,
    );
    connect_addresses(
        &mut graph,
        104,
        PortAddress::declared(node_id(3), key("false")),
        merge_outer_false,
    );
    connect(&mut graph, 105, 5, "then", 6, "enter");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(
        result.plan.is_none(),
        "must not unconditionally execute node A"
    );
    assert!(result.analysis.has_blocking_errors());
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.control.unstructured_continuation"
            && diagnostic.primary
                == crate::node_system::analysis::DiagnosticLocation::Node(node_id(3))
    }));
}

#[test]
fn branch_postdom_uses_last_walker_successor_for_multi_output_sequence() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.control.branch"),
        (3, "yssbi.control.sequence"),
        (4, "yssbi.control.do"),
        (5, "yssbi.control.merge"),
        (6, "yssbi.control.do"),
    ]);
    let sequence_first = bind_member_port(&mut graph, 3, "then", 30, "a");
    let sequence_second = bind_member_port(&mut graph, 3, "then", 31, "z");
    let merge_true = bind_member_port(&mut graph, 5, "enter", 50, "a");
    let merge_false = bind_member_port(&mut graph, 5, "enter", 51, "z");
    connect(&mut graph, 100, 1, "value", 2, "condition");
    connect(&mut graph, 101, 2, "true", 3, "enter");
    connect_addresses(
        &mut graph,
        102,
        sequence_first,
        PortAddress::declared(node_id(4), key("enter")),
    );
    connect_addresses(&mut graph, 103, sequence_second, merge_true);
    connect_addresses(
        &mut graph,
        104,
        PortAddress::declared(node_id(2), key("false")),
        merge_false,
    );
    connect(&mut graph, 105, 5, "then", 6, "enter");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result
        .plan
        .unwrap_or_else(|| panic!("sequence diagnostics: {:?}", result.analysis.diagnostics));
    let first = operation_index_for_node(&plan, 4);
    let continuation = operation_index_for_node(&plan, 6);
    let root_steps = match &plan.root_region {
        StructuredControlRegion::Sequence(steps) => steps,
        other => panic!("expected root sequence, got {other:?}"),
    };
    let (branch_index, branch) = root_steps
        .iter()
        .enumerate()
        .find_map(|(index, step)| match step {
            ControlStep::Region(region)
                if matches!(region.as_ref(), StructuredControlRegion::If { .. }) =>
            {
                Some((index, region.as_ref()))
            }
            _ => None,
        })
        .expect("Branch region");
    let StructuredControlRegion::If {
        then_region,
        else_region,
        ..
    } = branch
    else {
        unreachable!()
    };
    assert!(region_contains_operation(then_region, first));
    assert!(!region_contains_operation(else_region, first));
    assert!(!region_contains_operation(then_region, continuation));
    assert!(!region_contains_operation(else_region, continuation));
    assert!(root_steps[branch_index + 1..].iter().any(
        |step| matches!(step, ControlStep::Operation(operation) if *operation == continuation)
    ));
}

#[test]
fn branch_continuation_allows_multi_output_sequence_suffix_after_merge() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.control.branch"),
        (3, "yssbi.control.do"),
        (4, "yssbi.control.do"),
        (5, "yssbi.control.merge"),
        (6, "yssbi.control.sequence"),
        (7, "yssbi.control.do"),
        (8, "yssbi.control.do"),
    ]);
    let merge_true = bind_member_port(&mut graph, 5, "enter", 50, "a");
    let merge_false = bind_member_port(&mut graph, 5, "enter", 51, "z");
    let suffix_first = bind_member_port(&mut graph, 6, "then", 60, "a");
    let suffix_second = bind_member_port(&mut graph, 6, "then", 61, "z");
    connect(&mut graph, 100, 1, "value", 2, "condition");
    connect(&mut graph, 101, 2, "true", 3, "enter");
    connect(&mut graph, 102, 2, "false", 4, "enter");
    connect_addresses(
        &mut graph,
        103,
        PortAddress::declared(node_id(3), key("then")),
        merge_true,
    );
    connect_addresses(
        &mut graph,
        104,
        PortAddress::declared(node_id(4), key("then")),
        merge_false,
    );
    connect(&mut graph, 105, 5, "then", 6, "enter");
    connect_addresses(
        &mut graph,
        106,
        suffix_first,
        PortAddress::declared(node_id(7), key("enter")),
    );
    connect_addresses(
        &mut graph,
        107,
        suffix_second,
        PortAddress::declared(node_id(8), key("enter")),
    );

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result
        .plan
        .unwrap_or_else(|| panic!("suffix diagnostics: {:?}", result.analysis.diagnostics));
    let first = operation_index_for_node(&plan, 7);
    let second = operation_index_for_node(&plan, 8);
    let root_steps = match &plan.root_region {
        StructuredControlRegion::Sequence(steps) => steps,
        other => panic!("expected root sequence, got {other:?}"),
    };
    let branch_index = root_steps
        .iter()
        .position(|step| matches!(step, ControlStep::Region(region) if matches!(region.as_ref(), StructuredControlRegion::If { .. })))
        .expect("Branch region");
    let suffix = &root_steps[branch_index + 1..];
    let first_index = suffix
        .iter()
        .position(|step| matches!(step, ControlStep::Operation(operation) if *operation == first))
        .expect("first suffix operation");
    let second_index = suffix
        .iter()
        .position(|step| matches!(step, ControlStep::Operation(operation) if *operation == second))
        .expect("second suffix operation");
    assert!(first_index < second_index);
}

#[test]
fn nested_branch_postdom_stops_before_outer_multi_output_sequence_suffix() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.bool"),
        (3, "yssbi.control.branch"),
        (4, "yssbi.control.branch"),
        (5, "yssbi.control.merge"),
        (6, "yssbi.control.merge"),
        (7, "yssbi.control.sequence"),
        (8, "yssbi.control.do"),
        (9, "yssbi.control.do"),
    ]);
    let inner_true = bind_member_port(&mut graph, 5, "enter", 50, "a");
    let inner_false = bind_member_port(&mut graph, 5, "enter", 51, "z");
    let outer_true = bind_member_port(&mut graph, 6, "enter", 60, "a");
    let outer_false = bind_member_port(&mut graph, 6, "enter", 61, "z");
    let suffix_first = bind_member_port(&mut graph, 7, "then", 70, "a");
    let suffix_second = bind_member_port(&mut graph, 7, "then", 71, "z");
    connect(&mut graph, 100, 1, "value", 3, "condition");
    connect(&mut graph, 101, 2, "value", 4, "condition");
    connect(&mut graph, 102, 3, "true", 4, "enter");
    connect_addresses(
        &mut graph,
        103,
        PortAddress::declared(node_id(4), key("true")),
        inner_true,
    );
    connect_addresses(
        &mut graph,
        104,
        PortAddress::declared(node_id(4), key("false")),
        inner_false,
    );
    connect_addresses(
        &mut graph,
        105,
        PortAddress::declared(node_id(5), key("then")),
        outer_true,
    );
    connect_addresses(
        &mut graph,
        106,
        PortAddress::declared(node_id(3), key("false")),
        outer_false,
    );
    connect(&mut graph, 107, 6, "then", 7, "enter");
    connect_addresses(
        &mut graph,
        108,
        suffix_first,
        PortAddress::declared(node_id(8), key("enter")),
    );
    connect_addresses(
        &mut graph,
        109,
        suffix_second,
        PortAddress::declared(node_id(9), key("enter")),
    );

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result.plan.unwrap_or_else(|| {
        panic!(
            "nested suffix diagnostics: {:?}",
            result.analysis.diagnostics
        )
    });
    let first = operation_index_for_node(&plan, 8);
    let second = operation_index_for_node(&plan, 9);
    let root_steps = match &plan.root_region {
        StructuredControlRegion::Sequence(steps) => steps,
        other => panic!("expected root sequence, got {other:?}"),
    };
    let branch_index = root_steps
        .iter()
        .position(|step| matches!(step, ControlStep::Region(region) if matches!(region.as_ref(), StructuredControlRegion::If { .. })))
        .expect("outer Branch region");
    assert!(
        root_steps[branch_index + 1..]
            .iter()
            .any(|step| matches!(step, ControlStep::Operation(operation) if *operation == first))
    );
    assert!(
        root_steps[branch_index + 1..]
            .iter()
            .any(|step| matches!(step, ControlStep::Operation(operation) if *operation == second))
    );
}

#[test]
fn nested_complete_diamond_uses_the_true_common_continuation() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.bool"),
        (3, "yssbi.control.branch"),
        (4, "yssbi.control.branch"),
        (5, "yssbi.control.merge"),
        (6, "yssbi.control.merge"),
        (7, "yssbi.control.do"),
    ]);
    let inner_true = bind_member_port(&mut graph, 5, "enter", 50, "z");
    let inner_false = bind_member_port(&mut graph, 5, "enter", 51, "a");
    let outer_true = bind_member_port(&mut graph, 6, "enter", 60, "z");
    let outer_false = bind_member_port(&mut graph, 6, "enter", 61, "a");
    connect(&mut graph, 100, 1, "value", 3, "condition");
    connect(&mut graph, 101, 2, "value", 4, "condition");
    connect(&mut graph, 102, 3, "true", 4, "enter");
    connect_addresses(
        &mut graph,
        103,
        PortAddress::declared(node_id(4), key("true")),
        inner_true,
    );
    connect_addresses(
        &mut graph,
        104,
        PortAddress::declared(node_id(4), key("false")),
        inner_false,
    );
    connect_addresses(
        &mut graph,
        105,
        PortAddress::declared(node_id(5), key("then")),
        outer_true,
    );
    connect_addresses(
        &mut graph,
        106,
        PortAddress::declared(node_id(3), key("false")),
        outer_false,
    );
    connect(&mut graph, 107, 6, "then", 7, "enter");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
    let plan = result.plan.unwrap_or_else(|| {
        panic!(
            "nested diamond diagnostics: {:?}",
            result.analysis.diagnostics
        )
    });
    let continuation = operation_index_for_node(&plan, 7);
    let root_steps = match &plan.root_region {
        StructuredControlRegion::Sequence(steps) => steps,
        other => panic!("expected root sequence, got {other:?}"),
    };
    let (outer_index, outer) = root_steps
        .iter()
        .enumerate()
        .find_map(|(index, step)| match step {
            ControlStep::Region(region)
                if matches!(region.as_ref(), StructuredControlRegion::If { .. }) =>
            {
                Some((index, region.as_ref()))
            }
            _ => None,
        })
        .expect("outer Branch must be top-level");
    let StructuredControlRegion::If {
        then_region,
        else_region,
        ..
    } = outer
    else {
        unreachable!()
    };
    assert!(contains_region(then_region, &|region| matches!(
        region,
        StructuredControlRegion::If { .. }
    )));
    assert!(!region_contains_operation(then_region, continuation));
    assert!(!region_contains_operation(else_region, continuation));
    assert!(root_steps[outer_index + 1..].iter().any(
        |step| matches!(step, ControlStep::Operation(operation) if *operation == continuation)
    ));
}

#[test]
fn malformed_builtin_control_members_emit_blocking_structured_diagnostics() {
    use crate::node_system::catalog::build_builtin_node_system;

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut branch = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.int64"),
        (3, "yssbi.constant.int64"),
        (4, "yssbi.control.branch"),
    ]);
    let then_source = bind_member_port(&mut branch, 4, "then_source", 40, "z");
    let else_source = bind_member_port(&mut branch, 4, "else_source", 41, "a");
    bind_member_port(&mut branch, 4, "result", 41, "m");
    connect(&mut branch, 100, 1, "value", 4, "condition");
    connect_addresses(
        &mut branch,
        101,
        PortAddress::declared(node_id(2), key("value")),
        then_source,
    );
    connect_addresses(
        &mut branch,
        102,
        PortAddress::declared(node_id(3), key("value")),
        else_source,
    );

    let branch_result = GraphCompiler::new(&registry, &Resources).compile(&branch);
    assert!(branch_result.plan.is_none());
    assert!(branch_result.semantic.is_none());
    assert!(branch_result.analysis.has_blocking_errors());
    assert!(branch_result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.control.member_group_identity_ambiguous"
            && diagnostic.severity == crate::node_system::analysis::DiagnosticSeverity::Error
            && diagnostic.primary
                == crate::node_system::analysis::DiagnosticLocation::Node(node_id(4))
    }));

    let mut incomplete_branch = branch.clone();
    incomplete_branch.port_bindings.retain(|address, _| {
        matches!(
            &address.port,
            crate::node_system::document::PortRef::Instance { template, .. }
                if template.as_str() == "then_source"
        )
    });
    incomplete_branch
        .connections
        .remove(&ConnectionId::from_uuid(Uuid::from_u128(102)));
    let incomplete_result = GraphCompiler::new(&registry, &Resources).compile(&incomplete_branch);
    assert!(incomplete_result.plan.is_none());
    assert!(
        incomplete_result
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| {
                diagnostic.code.as_str() == "compiler.control.member_group_incomplete"
                    && diagnostic.primary
                        == crate::node_system::analysis::DiagnosticLocation::Node(node_id(4))
            })
    );

    let mut loop_graph =
        builtin_graph_with_nodes(&[(5, "yssbi.constant.bool"), (6, "yssbi.control.loop")]);
    set_parameters(
        &mut loop_graph,
        6,
        &[("max_iterations", serde_json::json!(3))],
    );
    connect(&mut loop_graph, 103, 5, "value", 6, "condition");

    let loop_result = GraphCompiler::new(&registry, &Resources).compile(&loop_graph);
    assert!(loop_result.plan.is_none());
    assert!(loop_result.analysis.has_blocking_errors());
    assert!(loop_result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.control.member_group_count_invalid"
            && diagnostic.primary
                == crate::node_system::analysis::DiagnosticLocation::Node(node_id(6))
    }));
}

#[test]
fn loop_uses_explicit_condition_limit_and_carried_bindings() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::StructuredControlRegion;

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.int64"),
        (3, "yssbi.constant.int64"),
        (4, "yssbi.constant.int64"),
        (5, "yssbi.constant.int64"),
        (6, "yssbi.control.loop"),
        (7, "yssbi.control.do"),
        (8, "yssbi.control.do"),
        (9, "yssbi.constant.int64"),
        (10, "yssbi.control.do"),
        (11, "yssbi.control.sequence"),
    ]);
    set_parameters(&mut graph, 6, &[("max_iterations", serde_json::json!(7))]);
    let mut members = BTreeMap::new();
    for (instance, order) in [(61, "z"), (60, "a")] {
        let initial = bind_member_port(&mut graph, 6, "initial_source", instance, order);
        let body_input = bind_member_port(&mut graph, 6, "body_input", instance, order);
        let next = bind_member_port(&mut graph, 6, "next_source", instance, order);
        let result = bind_member_port(&mut graph, 6, "result", instance, order);
        members.insert(instance, (initial, body_input, next, result));
    }
    connect(&mut graph, 100, 1, "value", 6, "condition");
    for (connection, source, instance) in [(101, 2, 60), (102, 3, 61)] {
        connect_addresses(
            &mut graph,
            connection,
            PortAddress::declared(node_id(source), key("value")),
            members[&instance].0.clone(),
        );
    }
    for (connection, source, instance) in [(103, 4, 60), (104, 5, 61)] {
        connect_addresses(
            &mut graph,
            connection,
            PortAddress::declared(node_id(source), key("value")),
            members[&instance].2.clone(),
        );
    }
    let body_pure = bind_member_port(&mut graph, 11, "then", 110, "a");
    let body_eager = bind_member_port(&mut graph, 11, "then", 111, "z");
    connect(&mut graph, 105, 6, "body", 11, "enter");
    connect(&mut graph, 106, 6, "then", 8, "enter");
    connect_addresses(
        &mut graph,
        107,
        body_pure,
        PortAddress::declared(node_id(7), key("enter")),
    );
    connect_addresses(
        &mut graph,
        108,
        body_eager,
        PortAddress::declared(node_id(10), key("enter")),
    );

    let compiler = GraphCompiler::new(&registry, &Resources);
    let snapshot = compiler.snapshot(GraphResourcePath("events/loop-demand".into()), &graph);
    let mut result = compiler
        .compile_snapshot(&snapshot, &CompileCancellationToken::new())
        .unwrap();
    let body_local_pure = result
        .execution_basis
        .as_mut()
        .expect("loop graph has a specialization basis")
        .operations
        .iter_mut()
        .find(|operation| operation.source_node_id == node_id(7))
        .expect("first body operation is present in the basis");
    body_local_pure.evaluation = EvaluationPolicy::DemandDriven;
    body_local_pure.purity = Purity::Pure;
    body_local_pure.effects = EffectSemantics::None;
    let specialized = result
        .execution_basis
        .as_ref()
        .unwrap()
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([GraphOutputRef {
                graph_path: snapshot.provenance.graph_path.clone(),
                port: members[&60].3.clone(),
            }]),
            include_default_results: false,
        })
        .expect("demanded Loop result safely specializes");
    let specialized_nodes = specialized
        .operations
        .iter()
        .map(|operation| operation.source_node_id)
        .collect::<BTreeSet<_>>();
    assert!(!specialized_nodes.contains(&node_id(9)));
    assert!(
        !specialized_nodes.contains(&node_id(7)),
        "unrequested body-local pure work is pruned"
    );
    for retained in [1, 2, 3, 4, 5, 10] {
        assert!(
            specialized_nodes.contains(&node_id(retained)),
            "Loop condition, carried bindings, and body eager work stay retained"
        );
    }
    let retained_body_eager = operation_index_for_node(&specialized, 10);
    assert!(contains_region(
        &specialized.root_region,
        &|region| matches!(
            region,
            StructuredControlRegion::Loop {
                body,
                carried,
                max_iterations: 7,
                ..
            } if carried.len() == 2 && region_contains_operation(body, retained_body_eager)
        )
    ));
    specialized.validate().unwrap();

    let mut disconnected_basis = result.execution_basis.as_ref().unwrap().clone();
    let body_eager = disconnected_basis
        .operations
        .iter_mut()
        .find(|operation| operation.source_node_id == node_id(10))
        .expect("second body operation is present in the basis");
    body_eager.evaluation = EvaluationPolicy::DemandDriven;
    body_eager.purity = Purity::Pure;
    body_eager.effects = EffectSemantics::None;
    let disconnected = disconnected_basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([demand_output("events/loop-demand", 9, "value")]),
            include_default_results: false,
        })
        .expect("a disconnected carried Loop is deleted without orphan declarations");
    assert!(!contains_region(
        &disconnected.root_region,
        &|region| matches!(region, StructuredControlRegion::Loop { .. })
    ));
    assert!(
        disconnected
            .value_sources
            .iter()
            .all(|source| !matches!(source, PlanValueSource::ControlProduced(_)))
    );
    disconnected.validate().unwrap();

    let plan = result
        .plan
        .unwrap_or_else(|| panic!("loop diagnostics: {:?}", result.analysis.diagnostics));
    let carried = match &plan.root_region {
        region
            if contains_region(region, &|candidate| {
                matches!(candidate, StructuredControlRegion::Loop { .. })
            }) =>
        {
            fn find(
                region: &StructuredControlRegion,
            ) -> Option<&[crate::node_system::plan::LoopCarriedBinding]> {
                use crate::node_system::plan::ControlStep;
                match region {
                    StructuredControlRegion::Loop { carried, .. } => Some(carried),
                    StructuredControlRegion::Sequence(steps) => {
                        steps.iter().find_map(|step| match step {
                            ControlStep::Region(region) => find(region),
                            ControlStep::Operation(_) => None,
                        })
                    }
                    StructuredControlRegion::If {
                        then_region,
                        else_region,
                        ..
                    } => find(then_region).or_else(|| find(else_region)),
                    StructuredControlRegion::Call { .. } => None,
                }
            }
            find(region).unwrap()
        }
        _ => panic!("plan must contain Loop"),
    };
    assert_eq!(carried.len(), 2);
    for (binding, initial_node, next_node) in [(carried[0], 2, 4), (carried[1], 3, 5)] {
        let initial_output =
            plan.operations[operation_index_for_node(&plan, initial_node).index()].outputs[0].value;
        let next_output =
            plan.operations[operation_index_for_node(&plan, next_node).index()].outputs[0].value;
        assert!(plan.value_dependencies.iter().any(|dependency| {
            dependency.source == initial_output && dependency.destination == binding.initial_source
        }));
        assert!(plan.value_dependencies.iter().any(|dependency| {
            dependency.source == next_output && dependency.destination == binding.next_source
        }));
        assert_ne!(binding.body_input, binding.result);
        assert_ne!(binding.initial_source, binding.next_source);
    }
    assert!(contains_region(&plan.root_region, &|region| matches!(
        region,
        StructuredControlRegion::Loop {
            max_iterations: 7,
            ..
        }
    )));
}

#[test]
fn call_binds_exact_function_locators_across_different_value_layouts() {
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::plan::StructuredControlRegion;

    struct FunctionResources {
        path: GraphResourcePath,
        function: FunctionDocument,
        graph: GraphDocument,
    }
    impl ResourceSnapshot for FunctionResources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            BTreeMap::from([(
                ResourceKey::new(self.path.0.as_ref()),
                ResourceVersion::new("function-v1"),
            )])
        }

        fn function_document(&self, path: &GraphResourcePath) -> Option<&FunctionDocument> {
            (path == &self.path).then_some(&self.function)
        }

        fn function_graph_document(&self, path: &GraphResourcePath) -> Option<&GraphDocument> {
            (path == &self.path).then_some(&self.graph)
        }
    }

    let registry = std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
    let function_path = GraphResourcePath("functions/exact-layout".into());
    let parameter_id = FunctionParameterId("amount".into());
    let return_id = FunctionParameterId("return".into());
    let function = FunctionDocument::new(FunctionSignature {
        parameters: vec![FunctionParameter {
            id: parameter_id.clone(),
            name: "Amount".into(),
            type_name: "int64".into(),
        }],
        return_type: Some("int64".into()),
    });

    let mut callee = builtin_graph_with_nodes(&[
        (20, "yssbi.project.function.entry"),
        (21, "yssbi.constant.int64"),
        (30, "yssbi.project.function.return"),
    ]);
    set_parameters(
        &mut callee,
        20,
        &[("function", serde_json::json!(function_path.0.as_ref()))],
    );
    set_parameters(
        &mut callee,
        30,
        &[("function", serde_json::json!(function_path.0.as_ref()))],
    );
    let entry_parameter = bind_resolved_function_port(
        &mut callee,
        20,
        "parameters",
        200,
        "z",
        &function_path,
        &parameter_id,
    );
    let return_result = bind_resolved_function_port(
        &mut callee,
        30,
        "results",
        300,
        "a",
        &function_path,
        &return_id,
    );
    connect(&mut callee, 400, 20, "then", 30, "enter");
    connect_addresses(
        &mut callee,
        401,
        entry_parameter.clone(),
        return_result.clone(),
    );

    let resources = FunctionResources {
        path: function_path.clone(),
        function,
        graph: callee.clone(),
    };
    let compiler = GraphCompiler::with_interface_resolvers(
        &registry,
        &resources,
        build_builtin_interface_resolvers(),
    );
    let callee_products = compiler
        .compile_snapshot(
            &compiler.snapshot(function_path.clone(), &callee),
            &CompileCancellationToken::new(),
        )
        .unwrap();
    let callee_plan = callee_products
        .plan
        .clone()
        .expect("function compilation publishes a complete plan");
    assert_eq!(
        callee_plan
            .operations
            .iter()
            .map(|operation| operation.source_node_id)
            .collect::<Vec<_>>(),
        vec![node_id(21)],
        "the full callee keeps unrequested pure body work"
    );
    let callee_abi = callee_products
        .function_abi
        .expect("function compilation publishes Entry/Return ABI");

    let mut caller = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.int64"),
        (2, "yssbi.constant.int64"),
        (10, "yssbi.project.function.call"),
    ]);
    set_parameters(
        &mut caller,
        10,
        &[("target", serde_json::json!(function_path.0.as_ref()))],
    );
    let call_argument = bind_resolved_function_port(
        &mut caller,
        10,
        "arguments",
        100,
        "z",
        &function_path,
        &parameter_id,
    );
    let call_result = bind_resolved_function_port(
        &mut caller,
        10,
        "results",
        101,
        "a",
        &function_path,
        &return_id,
    );
    connect_addresses(
        &mut caller,
        500,
        PortAddress::declared(node_id(1), key("value")),
        call_argument.clone(),
    );

    let caller_snapshot = compiler.snapshot(GraphResourcePath("events/caller".into()), &caller);
    let eager_caller_products = compiler
        .compile_snapshot(&caller_snapshot, &CompileCancellationToken::new())
        .unwrap();
    let eager_caller_basis = eager_caller_products
        .execution_basis
        .as_ref()
        .expect("eager caller graph has a specialization basis");
    let eager_plan = eager_caller_basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([]),
            include_default_results: false,
        })
        .expect("eager Call remains required without requested outputs");
    assert!(contains_region(
        &eager_plan.root_region,
        &|region| matches!(region, StructuredControlRegion::Call { .. })
    ));

    let call_type = NodeTypeId::new("yssbi.project.function.call").unwrap();
    let mut pure_call_protocol = registry.protocol(&call_type).unwrap().clone();
    pure_call_protocol.execution.purity = Purity::Pure;
    pure_call_protocol.execution.evaluation = EvaluationPolicy::DemandDriven;
    pure_call_protocol.execution.effects = EffectSemantics::None;
    let pure_call_registry = ProtocolOverrideCompilerRegistry {
        frozen: &registry,
        node_type: call_type,
        protocol: pure_call_protocol,
        structural_role: StructuralNodeRole::Call,
    };
    let pure_compiler = GraphCompiler::with_interface_resolvers(
        &pure_call_registry,
        &resources,
        build_builtin_interface_resolvers(),
    );
    let pure_caller_snapshot =
        pure_compiler.snapshot(GraphResourcePath("events/caller".into()), &caller);
    let caller_products = pure_compiler
        .compile_snapshot(&pure_caller_snapshot, &CompileCancellationToken::new())
        .unwrap();
    let caller_basis = caller_products
        .execution_basis
        .as_ref()
        .expect("pure caller graph has a specialization basis");

    for (name, outputs, expected_operations) in [
        (
            "unrelated output",
            Box::new([demand_output("events/caller", 2, "value")]) as Box<[GraphOutputRef]>,
            vec![node_id(2)],
        ),
        ("empty output set", Box::new([]), Vec::new()),
    ] {
        let pruned = caller_basis
            .derive_plan(&ExecutionDemand::Outputs {
                outputs,
                include_default_results: false,
            })
            .unwrap_or_else(|error| panic!("{name} demand must derive: {error}"));
        assert_eq!(
            pruned
                .operations
                .iter()
                .map(|operation| operation.source_node_id)
                .collect::<Vec<_>>(),
            expected_operations,
            "{name} must not retain the caller argument closure"
        );
        assert!(
            !contains_region(&pruned.root_region, &|region| matches!(
                region,
                StructuredControlRegion::Call { .. }
            )),
            "{name} must prune the disconnected pure Call from the unmodified basis"
        );
        assert!(
            pruned
                .value_sources
                .iter()
                .all(|source| !matches!(source, PlanValueSource::ControlProduced(_))),
            "{name} must remove pruned Call result declarations"
        );
        pruned.validate().unwrap();
    }

    let mut controlled_caller = caller.clone();
    controlled_caller.nodes.insert(
        node_id(11),
        DocumentNode {
            id: node_id(11),
            node_type: NodeTypeId::new("yssbi.project.event.begin").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        },
    );
    connect(&mut controlled_caller, 501, 11, "then", 10, "enter");
    let controlled_snapshot = pure_compiler.snapshot(
        GraphResourcePath("events/controlled-caller".into()),
        &controlled_caller,
    );
    let controlled_products = pure_compiler
        .compile_snapshot(&controlled_snapshot, &CompileCancellationToken::new())
        .unwrap();
    let controlled_plan = controlled_products
        .execution_basis
        .expect("control-connected caller has a specialization basis")
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([]),
            include_default_results: false,
        })
        .expect("control-connected Call remains mandatory");
    assert!(contains_region(
        &controlled_plan.root_region,
        &|region| matches!(region, StructuredControlRegion::Call { .. })
    ));
    assert_eq!(
        controlled_plan
            .operations
            .iter()
            .map(|operation| operation.source_node_id)
            .collect::<Vec<_>>(),
        vec![node_id(1)],
        "a retained control-connected Call keeps its complete argument closure"
    );

    let requested_output = GraphOutputRef {
        graph_path: pure_caller_snapshot.provenance.graph_path.clone(),
        port: call_result.clone(),
    };
    let expected_requested_value = caller_basis.output_results[&requested_output].value;
    let (
        original_target,
        original_argument,
        original_result,
        original_argument_count,
        original_result_count,
    ) = find_call(&caller_basis.root_region).expect("basis Call region");
    assert_eq!(original_result.caller_destination, expected_requested_value);
    let specialized = caller_basis
        .derive_plan(&ExecutionDemand::Outputs {
            outputs: Box::new([requested_output.clone()]),
            include_default_results: false,
        })
        .expect("demanded Call result safely specializes");
    assert_eq!(
        specialized
            .operations
            .iter()
            .map(|operation| operation.source_node_id)
            .collect::<Vec<_>>(),
        vec![node_id(1)],
        "caller argument remains while unrelated caller pure work is pruned"
    );
    assert_eq!(
        callee_plan.operations.len(),
        1,
        "caller specialization must not demand-specialize the callee plan"
    );
    specialized.validate().unwrap();

    let plan = caller_products.plan.expect("caller should compile");
    fn find_call(
        region: &StructuredControlRegion,
    ) -> Option<(
        FunctionPlanHandle,
        CallArgumentBinding,
        CallResultBinding,
        usize,
        usize,
    )> {
        match region {
            StructuredControlRegion::Sequence(steps) => steps.iter().find_map(|step| match step {
                ControlStep::Operation(_) => None,
                ControlStep::Region(region) => find_call(region),
            }),
            StructuredControlRegion::If {
                then_region,
                else_region,
                ..
            } => find_call(then_region).or_else(|| find_call(else_region)),
            StructuredControlRegion::Loop { body, .. } => find_call(body),
            StructuredControlRegion::Call {
                target,
                arguments,
                results,
                ..
            } => Some((
                target.clone(),
                arguments[0],
                results[0],
                arguments.len(),
                results.len(),
            )),
        }
    }
    let expected_target = FunctionPlanHandle::new(function_path.0.clone()).unwrap();
    let (
        specialized_target,
        specialized_argument,
        specialized_result,
        argument_count,
        result_count,
    ) = find_call(&specialized.root_region).expect("specialized Call region");
    assert_eq!(original_target, expected_target);
    assert_eq!((original_argument_count, original_result_count), (1, 1));
    assert_eq!(specialized_target, original_target);
    assert_eq!((argument_count, result_count), (1, 1));
    assert_eq!(
        specialized_argument.caller_source,
        original_argument.caller_source
    );
    assert_eq!(
        specialized_result.caller_destination,
        original_result.caller_destination
    );
    assert_eq!(
        specialized_result.caller_destination,
        expected_requested_value
    );
    assert_eq!(
        specialized_argument.callee_destination,
        callee_abi.parameters[&parameter_id]
    );
    assert_eq!(
        specialized_result.callee_source,
        callee_abi.results[&return_id]
    );
    assert!(
        specialized
            .value_sources
            .contains(&PlanValueSource::ControlProduced(
                specialized_result.caller_destination
            ))
    );

    let (target, argument, result, argument_count, result_count) =
        find_call(&plan.root_region).expect("compiled Call region");
    assert_eq!(target, expected_target);
    assert_eq!((argument_count, result_count), (1, 1));
    assert_eq!(
        argument.callee_destination,
        callee_abi.parameters[&parameter_id]
    );
    assert_eq!(result.callee_source, callee_abi.results[&return_id]);
    assert_ne!(argument.caller_source, argument.callee_destination);
    assert_ne!(result.caller_destination, result.callee_source);
    assert_eq!(
        plan.provenance.basis.resource_versions,
        callee_abi.provenance.basis.resource_versions
    );
    assert_eq!(call_result.node_id, node_id(10));

    let recursive_call_id = node_id(40);
    let mut recursive = callee.clone();
    recursive.nodes.insert(
        recursive_call_id,
        DocumentNode {
            id: recursive_call_id,
            node_type: NodeTypeId::new("yssbi.project.function.call").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::from([(
                ParameterKey::new("target").unwrap(),
                serde_json::json!(function_path.0.as_ref()),
            )]),
            user_label: None,
        },
    );
    let recursive_argument = bind_resolved_function_port(
        &mut recursive,
        40,
        "arguments",
        410,
        "a",
        &function_path,
        &parameter_id,
    );
    let recursive_result = bind_resolved_function_port(
        &mut recursive,
        40,
        "results",
        411,
        "b",
        &function_path,
        &return_id,
    );
    recursive.connections.clear();
    connect(&mut recursive, 420, 20, "then", 40, "enter");
    connect(&mut recursive, 421, 40, "then", 30, "enter");
    connect_addresses(
        &mut recursive,
        422,
        entry_parameter.clone(),
        recursive_argument,
    );
    connect_addresses(&mut recursive, 423, recursive_result, return_result.clone());
    let recursive_resources = FunctionResources {
        path: function_path.clone(),
        function: resources.function.clone(),
        graph: recursive.clone(),
    };
    let recursive_compiler = GraphCompiler::with_interface_resolvers(
        &registry,
        &recursive_resources,
        build_builtin_interface_resolvers(),
    );
    let recursive_products = recursive_compiler
        .compile_snapshot(
            &recursive_compiler.snapshot(
                GraphResourcePath("events/self-recursive-caller".into()),
                &caller,
            ),
            &CompileCancellationToken::new(),
        )
        .unwrap();
    assert!(
        recursive_products.plan.is_some(),
        "self-recursive ABI analysis must stay bounded"
    );

    struct MutualResources {
        functions: BTreeMap<GraphResourcePath, FunctionDocument>,
        graphs: BTreeMap<GraphResourcePath, GraphDocument>,
    }
    impl ResourceSnapshot for MutualResources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            self.graphs
                .keys()
                .map(|path| {
                    (
                        ResourceKey::new(path.0.clone()),
                        ResourceVersion::new("mutual-v1"),
                    )
                })
                .collect()
        }

        fn function_document(&self, path: &GraphResourcePath) -> Option<&FunctionDocument> {
            self.functions.get(path)
        }

        fn function_graph_document(&self, path: &GraphResourcePath) -> Option<&GraphDocument> {
            self.graphs.get(path)
        }
    }
    let path_a = GraphResourcePath("functions/mutual-a".into());
    let path_b = GraphResourcePath("functions/mutual-b".into());
    let retarget = |own: &GraphResourcePath, target: &GraphResourcePath| {
        let mut graph = recursive.clone();
        for node in graph.nodes.values_mut() {
            if let Some(value) = node
                .parameters
                .get_mut(&ParameterKey::new("function").unwrap())
            {
                *value = serde_json::json!(own.0.as_ref());
            }
            if let Some(value) = node
                .parameters
                .get_mut(&ParameterKey::new("target").unwrap())
            {
                *value = serde_json::json!(target.0.as_ref());
            }
        }
        for (address, binding) in &mut graph.port_bindings {
            let (DynamicPortBinding::Resolved { origin, .. }
            | DynamicPortBinding::Orphan { origin, .. }) = binding
            else {
                continue;
            };
            let DynamicMemberLocator::FunctionParameter { function, .. } = origin else {
                continue;
            };
            *function = if address.node_id == recursive_call_id {
                target.clone()
            } else {
                own.clone()
            };
        }
        graph
    };
    let mutual_resources = MutualResources {
        functions: BTreeMap::from([
            (path_a.clone(), resources.function.clone()),
            (path_b.clone(), resources.function.clone()),
        ]),
        graphs: BTreeMap::from([
            (path_a.clone(), retarget(&path_a, &path_b)),
            (path_b.clone(), retarget(&path_b, &path_a)),
        ]),
    };
    let mutual_compiler = GraphCompiler::with_interface_resolvers(
        &registry,
        &mutual_resources,
        build_builtin_interface_resolvers(),
    );
    let mutual_products = mutual_compiler
        .compile_snapshot(
            &mutual_compiler.snapshot(path_a.clone(), &mutual_resources.graphs[&path_a]),
            &CompileCancellationToken::new(),
        )
        .unwrap();
    assert!(
        mutual_products.plan.is_some(),
        "mutual ABI analysis must stay bounded"
    );

    let return_address = return_result.clone();
    for (missing, address) in [
        ("entry", entry_parameter.clone()),
        ("return", return_result.clone()),
    ] {
        let mut malformed = callee.clone();
        malformed.port_bindings.remove(&address);
        malformed.connections.clear();
        if missing == "entry" {
            let constant = node_id(10);
            malformed.nodes.insert(
                constant,
                DocumentNode {
                    id: constant,
                    node_type: NodeTypeId::new("yssbi.constant.int64").unwrap(),
                    position: NodePosition { x: 0.0, y: 0.0 },
                    parameters: BTreeMap::new(),
                    user_label: None,
                },
            );
            connect_addresses(
                &mut malformed,
                402,
                PortAddress::declared(constant, key("value")),
                return_address.clone(),
            );
        }
        let resources = FunctionResources {
            path: function_path.clone(),
            function: resources.function.clone(),
            graph: malformed.clone(),
        };
        let compiler = GraphCompiler::with_interface_resolvers(
            &registry,
            &resources,
            build_builtin_interface_resolvers(),
        );
        let products = compiler
            .compile_snapshot(
                &compiler.snapshot(function_path.clone(), &malformed),
                &CompileCancellationToken::new(),
            )
            .unwrap();
        assert!(products.plan.is_none(), "missing {missing} ABI must block");
        let codes = products
            .analysis
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str())
            .collect::<Vec<_>>();
        assert!(
            codes.contains(&"compiler.function.abi.member_missing"),
            "missing {missing} ABI diagnostics: {codes:?}"
        );
    }

    let compile_function_codes = |graph: GraphDocument| {
        let resources = FunctionResources {
            path: function_path.clone(),
            function: resources.function.clone(),
            graph: graph.clone(),
        };
        let compiler = GraphCompiler::with_interface_resolvers(
            &registry,
            &resources,
            build_builtin_interface_resolvers(),
        );
        compiler
            .compile_snapshot(
                &compiler.snapshot(function_path.clone(), &graph),
                &CompileCancellationToken::new(),
            )
            .unwrap()
            .analysis
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code.as_str().to_owned())
            .collect::<Vec<_>>()
    };

    let mut unexpected = callee.clone();
    let DynamicPortBinding::Resolved { origin, .. } = unexpected
        .port_bindings
        .get_mut(&entry_parameter)
        .expect("entry binding")
    else {
        unreachable!()
    };
    *origin = DynamicMemberLocator::FunctionParameter {
        function: function_path.clone(),
        parameter: FunctionParameterId("unexpected".into()),
    };
    assert!(
        compile_function_codes(unexpected)
            .iter()
            .any(|code| code == "compiler.function.abi.member_unexpected")
    );

    let mut duplicate = callee.clone();
    let duplicate_address = PortAddress::instance(
        node_id(20),
        key("parameters"),
        PortInstanceId::from_uuid(Uuid::from_u128(201)),
    );
    duplicate.port_bindings.insert(
        duplicate_address,
        duplicate.port_bindings[&entry_parameter].clone(),
    );
    assert!(
        compile_function_codes(duplicate)
            .iter()
            .any(|code| code == "compiler.function.abi.member_duplicate")
    );

    let mut wrong_template = callee.clone();
    let binding = wrong_template
        .port_bindings
        .remove(&entry_parameter)
        .expect("entry binding");
    wrong_template.port_bindings.insert(
        PortAddress::instance(
            node_id(20),
            key("wrong_parameters"),
            PortInstanceId::from_uuid(Uuid::from_u128(202)),
        ),
        binding,
    );
    assert!(
        compile_function_codes(wrong_template)
            .iter()
            .any(|code| code == "compiler.function.abi.endpoint_invalid")
    );

    let mut missing_call_argument = caller.clone();
    missing_call_argument.port_bindings.remove(&call_argument);
    missing_call_argument.connections.clear();
    let products = compiler
        .compile_snapshot(
            &compiler.snapshot(
                GraphResourcePath("events/missing-call-member".into()),
                &missing_call_argument,
            ),
            &CompileCancellationToken::new(),
        )
        .unwrap();
    let codes = products
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<Vec<_>>();
    assert!(products.plan.is_none());
    assert!(
        codes.contains(&"compiler.control.call.member_missing"),
        "missing Call member diagnostics: {codes:?}"
    );

    let mut extra_call_argument = caller.clone();
    bind_resolved_function_port(
        &mut extra_call_argument,
        10,
        "arguments",
        102,
        "extra",
        &function_path,
        &FunctionParameterId("unexpected".into()),
    );
    let products = compiler
        .compile_snapshot(
            &compiler.snapshot(
                GraphResourcePath("events/extra-call-member".into()),
                &extra_call_argument,
            ),
            &CompileCancellationToken::new(),
        )
        .unwrap();
    let codes = products
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect::<Vec<_>>();
    assert!(products.plan.is_none());
    assert!(
        codes.contains(&"compiler.control.call.member_unexpected"),
        "extra Call member diagnostics: {codes:?}"
    );
}

#[test]
fn function_abi_rejects_wrong_dynamic_member_direction() {
    struct FunctionResources {
        path: GraphResourcePath,
        function: FunctionDocument,
    }
    impl ResourceSnapshot for FunctionResources {
        fn versions(&self) -> crate::node_system::analysis::ResourceVersionSet {
            BTreeMap::new()
        }

        fn function_document(&self, path: &GraphResourcePath) -> Option<&FunctionDocument> {
            (path == &self.path).then_some(&self.function)
        }
    }

    let mut parameters = data_port("parameters", PortDirection::Input, TypeExpr::Unknown, None);
    parameters.instances = PortInstances::UserCreated { min: 0, max: None };
    let mut entry = structural_protocol(
        "wrong_direction_entry",
        vec![control_port("then", PortDirection::Output), parameters],
        vec![],
    );
    entry.managed_role = Some(ManagedNodeRole::FunctionEntry);
    entry.scope = NodeScope::Function;
    let entry_type = entry.type_id.clone();
    let mut return_node = structural_protocol(
        "wrong_direction_return",
        vec![control_port("enter", PortDirection::Input)],
        vec![],
    );
    return_node.managed_role = Some(ManagedNodeRole::FunctionReturn);
    return_node.scope = NodeScope::Function;
    let return_type = return_node.type_id.clone();
    let registry = TestRegistry::new(vec![entry, return_node])
        .structural(&entry_type, StructuralNodeRole::FunctionEntry)
        .structural(&return_type, StructuralNodeRole::FunctionReturn);
    let path = GraphResourcePath("functions/wrong-direction".into());
    let parameter = FunctionParameterId("amount".into());
    let resources = FunctionResources {
        path: path.clone(),
        function: FunctionDocument::new(FunctionSignature {
            parameters: vec![FunctionParameter {
                id: parameter.clone(),
                name: "Amount".into(),
                type_name: "int64".into(),
            }],
            return_type: None,
        }),
    };
    let mut graph =
        graph_with_nodes(&[(1, "wrong_direction_entry"), (2, "wrong_direction_return")]);
    bind_resolved_function_port(&mut graph, 1, "parameters", 10, "a", &path, &parameter);
    connect(&mut graph, 11, 1, "then", 2, "enter");

    let compiler = GraphCompiler::new(&registry, &resources);
    let products = compiler
        .compile_snapshot(
            &compiler.snapshot(path, &graph),
            &CompileCancellationToken::new(),
        )
        .unwrap();

    assert!(products.plan.is_none());
    assert!(products.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.function.abi.endpoint_invalid"
    }));
}

#[test]
fn missing_call_resource_parameter_is_blocking() {
    let call = structural_protocol("call_missing_target", vec![], vec![]);
    let call_type = call.type_id.clone();
    let registry = TestRegistry::new(vec![call]).structural(&call_type, StructuralNodeRole::Call);
    let graph = graph_with_nodes(&[(1, "call_missing_target")]);

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    assert!(result.plan.is_none());
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.control.call.resource_parameter_missing"
    }));
}

struct PanicLowerer;
impl NodeLowerer for PanicLowerer {
    fn lower(&self, _: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Err(LoweringError::new(
            "structural node reached the leaf lowerer",
        ))
    }
}

#[test]
fn structural_nodes_never_invoke_leaf_lowerers() {
    let sequence = structural_protocol(
        "structural_only",
        vec![control_port("then", PortDirection::Output)],
        vec![],
    );
    let sequence_type = sequence.type_id.clone();
    let mut registry =
        TestRegistry::new(vec![sequence]).structural(&sequence_type, StructuralNodeRole::Sequence);
    registry.nodes.get_mut(&sequence_type).unwrap().1 = NodeImplementation::new(PanicLowerer);
    let graph = graph_with_nodes(&[(1, "structural_only")]);

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    let plan = result
        .plan
        .expect("structural-only graph should produce a plan");
    assert!(plan.operations.is_empty());
}
