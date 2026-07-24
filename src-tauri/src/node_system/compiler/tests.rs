use super::*;
use crate::node_system::analysis::{
    CompileId, ProjectSessionId, ResourceKey, ResourceVersion, SpanEvent, SpanKind, TraceSink,
};
use crate::node_system::document::{
    DocumentConnection, DocumentNode, GraphDocument, GraphResourcePath, NodeId, NodePosition,
    PortAddress,
};
use crate::node_system::plan::{
    CompiledParameterHandle, CompiledResourceRequirement, KernelHandle, MaterializationBridge,
    PlanResult, RelationalBackendId, RelationalFragmentId, RelationalOperator,
    RelationalOperatorIndex, ResourceAccess, ResourceId, ResourceKind,
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
    let plan = result.plan.expect("valid graph should lower");
    assert!(result.analysis.diagnostics.is_empty());
    assert_eq!(plan.operations.len(), 1);
    assert_eq!(plan.provenance.basis, result.analysis.basis);
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
    GraphDocument {
        revision: crate::node_system::document::GraphRevision::new(11),
        nodes: nodes
            .iter()
            .map(|(id, node_type)| {
                let id = node_id(*id);
                (
                    id,
                    DocumentNode {
                        id,
                        node_type: NodeTypeId::new(format!("yssbi.test.{node_type}")).unwrap(),
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
    let id = crate::node_system::document::ConnectionId::from_uuid(Uuid::from_u128(id));
    graph.connections.insert(
        id,
        DocumentConnection {
            id,
            output: PortAddress::declared(node_id(source_node), key(source_port)),
            input: PortAddress::declared(node_id(target_node), key(target_port)),
            order: None,
        },
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
    let sink = test_protocol("plan_relation_sink", vec![sink_input], vec![], vec![]);
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
    assert!(
        !plan.relational_subplans[0]
            .compiled_plan
            .operators
            .is_empty()
    );
    for operation in &plan.operations {
        if let crate::node_system::plan::PlannedKernel::Relational(index) = &operation.kernel {
            assert!(index.index() < plan.relational_subplans.len());
        } else {
            assert!(
                matches!(
                    &operation.kernel,
                    crate::node_system::plan::PlannedKernel::Relational(_)
                ),
                "relational fragment should become a relational operation"
            );
        }
    }
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
    let sink = test_protocol("plan_bridge_sink", vec![sink_input], vec![], vec![]);
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

fn parameter(name: &str) -> ParameterSpec {
    ParameterSpec {
        key: ParameterKey::new(name).unwrap(),
        title_key: I18nKey::new(format!("parameters.{name}.title")).unwrap(),
        description_key: None,
        value_type: TypeExpr::Unknown,
        default_value: None,
        constraints: vec![],
        editor: ParameterEditorSpec::Auto,
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

#[test]
fn branch_builds_exclusive_true_and_false_regions() {
    use crate::node_system::plan::StructuredControlRegion;
    let source = test_protocol(
        "condition_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let branch = structural_protocol(
        "branch",
        vec![
            data_port("condition", PortDirection::Input, TypeExpr::Unknown, None),
            control_port("true", PortDirection::Output),
            control_port("false", PortDirection::Output),
        ],
        vec![],
    );
    let true_leaf = test_protocol(
        "true_leaf",
        vec![control_port("enter", PortDirection::Input)],
        vec![],
        vec![],
    );
    let false_leaf = test_protocol(
        "false_leaf",
        vec![control_port("enter", PortDirection::Input)],
        vec![],
        vec![],
    );
    let branch_type = branch.type_id.clone();
    let registry = TestRegistry::new(vec![source, branch, true_leaf, false_leaf])
        .structural(&branch_type, StructuralNodeRole::Branch);
    let mut graph = graph_with_nodes(&[
        (1, "condition_source"),
        (2, "branch"),
        (3, "true_leaf"),
        (4, "false_leaf"),
    ]);
    connect(&mut graph, 10, 1, "out", 2, "condition");
    connect(&mut graph, 11, 2, "true", 3, "enter");
    connect(&mut graph, 12, 2, "false", 4, "enter");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    let plan = result.plan.expect("branch should produce a plan");
    assert_eq!(plan.operations.len(), 3);
    assert!(contains_region(&plan.root_region, &|region| matches!(
        region,
        StructuredControlRegion::If { then_region, else_region, .. }
            if matches!(then_region.as_ref(), StructuredControlRegion::Sequence(steps) if steps.len() == 1)
                && matches!(else_region.as_ref(), StructuredControlRegion::Sequence(steps) if steps.len() == 1)
    )));
}

#[test]
fn loop_uses_explicit_condition_limit_and_carried_bindings() {
    use crate::node_system::plan::StructuredControlRegion;
    let source = test_protocol(
        "loop_condition",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let loop_node = structural_protocol(
        "loop",
        vec![
            data_port("condition", PortDirection::Input, TypeExpr::Unknown, None),
            control_port("body", PortDirection::Output),
        ],
        vec![parameter("max_iterations"), parameter("carried")],
    );
    let body = test_protocol(
        "loop_body",
        vec![control_port("enter", PortDirection::Input)],
        vec![],
        vec![],
    );
    let loop_type = loop_node.type_id.clone();
    let registry = TestRegistry::new(vec![source, loop_node, body])
        .structural(&loop_type, StructuralNodeRole::Loop);
    let mut graph = graph_with_nodes(&[(1, "loop_condition"), (2, "loop"), (3, "loop_body")]);
    set_parameters(
        &mut graph,
        2,
        &[
            ("max_iterations", serde_json::json!(7)),
            ("carried", serde_json::json!([])),
        ],
    );
    connect(&mut graph, 10, 1, "out", 2, "condition");
    connect(&mut graph, 11, 2, "body", 3, "enter");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    let plan = result.plan.expect("explicit loop should produce a plan");
    assert!(contains_region(&plan.root_region, &|region| matches!(
        region,
        StructuredControlRegion::Loop { max_iterations: 7, carried, .. } if carried.is_empty()
    )));
}

#[test]
fn call_parses_target_and_region_value_bindings() {
    use crate::node_system::plan::StructuredControlRegion;
    let source = test_protocol(
        "call_source",
        vec![data_port(
            "out",
            PortDirection::Output,
            TypeExpr::Unknown,
            None,
        )],
        vec![],
        vec![],
    );
    let call = structural_protocol(
        "call",
        vec![
            data_port("argument", PortDirection::Input, TypeExpr::Unknown, None),
            data_port("result", PortDirection::Output, TypeExpr::Unknown, None),
        ],
        vec![
            parameter("target"),
            parameter("arguments"),
            parameter("results"),
        ],
    );
    let call_type = call.type_id.clone();
    let registry =
        TestRegistry::new(vec![source, call]).structural(&call_type, StructuralNodeRole::Call);
    let mut graph = graph_with_nodes(&[(1, "call_source"), (2, "call")]);
    set_parameters(
        &mut graph,
        2,
        &[
            ("target", serde_json::json!("functions/test")),
            (
                "arguments",
                serde_json::json!([{"destination": "argument", "source": "argument"}]),
            ),
            (
                "results",
                serde_json::json!([{"destination": "result", "source": "result"}]),
            ),
        ],
    );
    connect(&mut graph, 10, 1, "out", 2, "argument");

    let result = GraphCompiler::new(&registry, &Resources).compile(&graph);

    let plan = result.plan.expect("call should produce a plan");
    assert!(contains_region(&plan.root_region, &|region| matches!(
        region,
        StructuredControlRegion::Call { target, arguments, results }
            if target.as_str() == "functions/test" && arguments.len() == 1 && results.len() == 1
    )));
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
