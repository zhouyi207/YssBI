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
    CallArgumentBinding, CallResultBinding, CompiledParameterHandle, CompiledResourceRequirement,
    ControlStep, KernelHandle, MaterializationBridge, PlanResult, RelationalBackendId,
    RelationalBridgeInput, RelationalFragmentId, RelationalOperator, RelationalOperatorIndex,
    ResourceAccess, ResourceId, ResourceKind,
};
use crate::node_system::protocol::*;
use crate::node_system::registry::{
    CategoryRegistration, I18nManifest, NodeRegistry, NodeRegistryBuilder, ProtocolFingerprint,
    ProviderRegistration, RegisteredNode, RegistryFingerprint, StructuralNodeRole,
};
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
fn protocol() -> NodeProtocol {
    NodeProtocol::from_static(Box::leak(Box::new(StaticNodeProtocol {
        type_id: "yssbi.test.constant",
        catalog: StaticNodeCatalogProtocol {
            title_key: "nodes.test.constant.title",
            description_key: None,
            documentation_key: None,
            aliases_key: None,
            category_id: "test",
            icon_id: "test",
            style_id: "test",
            hidden: false,
        },
        ports: Box::leak(
            vec![StaticPortSpec {
                key: "value",
                label_key: "nodes.test.constant.value",
                direction: PortDirection::Output,
                kind: PortKind::Data,
                instances: PortInstances::Declared,
                connections: ConnectionsPerPort::Multiple {
                    max: None,
                    ordered: false,
                },
                input_binding: None,
            }]
            .into_boxed_slice(),
        ),
        execution: ExecutionSemantics {
            determinism: Determinism::Deterministic,
            purity: Purity::Pure,
            evaluation: EvaluationPolicy::DemandDriven,
            cache: CachePolicy::PerRun,
            effects: EffectSemantics::None,
        },
        scope: NodeScope::Any,
        managed_role: None,
    })))
    .unwrap()
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

    let plan = GraphCompiler::new(&registry, &Resources)
        .compile(&graph)
        .plan
        .expect("relational graph should lower");

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
            value: plan.operations[0].outputs[0].value,
        }]
    );
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
    let mut graph = graph_with_nodes(&[(1, "plan_bridge_source"), (2, "plan_bridge_sink")]);
    connect(&mut graph, 10, 1, "out", 2, "in");

    let plan = GraphCompiler::new(&registry, &Resources)
        .compile(&graph)
        .plan
        .expect("bridge graph should lower");

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
            [SchemaColumnRef("a".into()), SchemaColumnRef("b".into())],
        ))
    }
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
    let filter = test_protocol(
        "schema_filter",
        vec![
            data_port("in", PortDirection::Input, TypeExpr::Unknown, None),
            data_port(
                "out",
                PortDirection::Output,
                TypeExpr::Unknown,
                Some(SchemaExpr::Filter {
                    input: Box::new(SchemaExpr::Input(key("in"))),
                }),
            ),
        ],
        vec![],
        vec![],
    );
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
    connect(&mut graph, 10, 1, "out", 2, "in");
    connect(&mut graph, 11, 2, "out", 3, "in");
    connect(&mut graph, 12, 3, "out", 4, "in");

    let result =
        GraphCompiler::with_schema_resolvers(&registry, &Resources, resolvers).compile(&graph);

    assert!(result.plan.is_some(), "{:?}", result.analysis.diagnostics);
    let source_fact = SchemaExpr::Input(key("raw"));
    assert_eq!(
        result
            .analysis
            .partial_schemas
            .get(&PortAddress::declared(node_id(2), key("out"))),
        Some(&source_fact)
    );
    let projected = SchemaExpr::Project {
        input: Box::new(source_fact),
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
    use crate::node_system::catalog::build_builtin_registry;

    let registry = build_builtin_registry();
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
    assert!(region_contains_operation(
        &plan.root_region,
        operation_index_for_node(&plan, 2)
    ));
    assert!(region_contains_operation(
        &plan.root_region,
        operation_index_for_node(&plan, 3)
    ));
}

#[test]
fn branch_builds_exclusive_true_and_false_regions() {
    use crate::node_system::catalog::build_builtin_registry;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = build_builtin_registry();
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.int64"),
        (3, "yssbi.constant.int64"),
        (4, "yssbi.control.branch"),
        (5, "yssbi.control.do"),
        (6, "yssbi.control.do"),
        (7, "yssbi.control.merge"),
        (8, "yssbi.debug.view"),
    ]);
    let then_source = bind_member_port(&mut graph, 4, "then_source", 40, "z");
    let else_source = bind_member_port(&mut graph, 4, "else_source", 40, "a");
    let result_port = bind_member_port(&mut graph, 4, "result", 40, "m");
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

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
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
    use crate::node_system::catalog::build_builtin_registry;

    let registry = build_builtin_registry();
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
    use crate::node_system::catalog::build_builtin_registry;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = build_builtin_registry();
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
    use crate::node_system::catalog::build_builtin_registry;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = build_builtin_registry();
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
    use crate::node_system::catalog::build_builtin_registry;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = build_builtin_registry();
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
    use crate::node_system::catalog::build_builtin_registry;
    use crate::node_system::plan::{ControlStep, StructuredControlRegion};

    let registry = build_builtin_registry();
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
    use crate::node_system::catalog::build_builtin_registry;

    let registry = build_builtin_registry();
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
    use crate::node_system::catalog::build_builtin_registry;
    use crate::node_system::plan::StructuredControlRegion;

    let registry = build_builtin_registry();
    let mut graph = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.bool"),
        (2, "yssbi.constant.int64"),
        (3, "yssbi.constant.int64"),
        (4, "yssbi.constant.int64"),
        (5, "yssbi.constant.int64"),
        (6, "yssbi.control.loop"),
        (7, "yssbi.control.do"),
        (8, "yssbi.control.do"),
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
    connect(&mut graph, 105, 6, "body", 7, "enter");
    connect(&mut graph, 106, 6, "then", 8, "enter");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);
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
    use crate::node_system::catalog::build_builtin_registry;
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

    let registry = build_builtin_registry();
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
    let callee_abi = callee_products
        .function_abi
        .expect("function compilation publishes Entry/Return ABI");

    let mut caller = builtin_graph_with_nodes(&[
        (1, "yssbi.constant.int64"),
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

    let caller_products = compiler
        .compile_snapshot(
            &compiler.snapshot(GraphResourcePath("events/caller".into()), &caller),
            &CompileCancellationToken::new(),
        )
        .unwrap();
    let plan = caller_products.plan.expect("caller should compile");
    fn find_call(
        region: &StructuredControlRegion,
    ) -> Option<(CallArgumentBinding, CallResultBinding)> {
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
                arguments, results, ..
            } => Some((arguments[0], results[0])),
        }
    }
    let (argument, result) = find_call(&plan.root_region).expect("compiled Call region");

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
