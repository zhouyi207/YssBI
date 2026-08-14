use super::*;
use crate::node_system::analysis::{
    DiagnosticLocation, DiagnosticSeverity, ResourceVersionSet, SemanticDependency,
};
use crate::node_system::catalog::{
    CONTROL_REROUTE_NODE_TYPE, DATA_REROUTE_NODE_TYPE, EFFECT_REROUTE_NODE_TYPE,
    REROUTE_INPUT_PORT, REROUTE_OUTPUT_PORT,
};
use crate::node_system::document::{
    ConnectionId, DocumentConnection, DocumentNode, GraphDocument, GraphResourcePath, NodeId,
    NodePosition, PortAddress,
};
use crate::node_system::plan::{CompiledParameterHandle, KernelHandle};
use crate::node_system::protocol::*;
use crate::node_system::registry::{
    CategoryRegistration, I18nManifest, NodeRegistry, NodeRegistryBuilder, ProviderRegistration,
    RegisteredNode, TransparentNodeRole, TypeRegistration,
};
use crate::node_system::testing::TestProtocolBuilder;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use uuid::Uuid;

const DATA_SOURCE: &str = "yssbi.test.reroute.data_source";
const DATA_SINK: &str = "yssbi.test.reroute.data_sink";
const DATA_PASS: &str = "yssbi.test.reroute.data_pass";
const FLOW_NODE: &str = "yssbi.test.reroute.flow";
const TEST_SCHEMA_RESOLVER: &str = "yssbi.test.reroute.schema";

struct EmptyResources;

impl ResourceSnapshot for EmptyResources {
    fn versions(&self) -> ResourceVersionSet {
        ResourceVersionSet::new()
    }
}

struct TestSchemaResolver;

impl SchemaResolver for TestSchemaResolver {
    fn resolve(
        &self,
        _context: &mut SchemaResolutionContext<'_, '_>,
    ) -> Result<SchemaFact, SchemaResolutionError> {
        Ok(ResolvedSchemaFact::new(
            SchemaExpr::Derived {
                resolver: SchemaResolverId::new(TEST_SCHEMA_RESOLVER).unwrap(),
                dependencies: vec![],
            },
            [SchemaColumnRef("value".into())],
        ))
    }
}

struct TestLowerer;

impl NodeLowerer for TestLowerer {
    fn lower(&self, context: &LoweringContext<'_>) -> Result<LoweredNode, LoweringError> {
        Ok(LoweredNode {
            kernel: LoweredKernel::Native(
                KernelHandle::new(format!("test.reroute.{}", context.node_id))
                    .map_err(|_| LoweringError::internal(LoweringInvariant::InvalidStaticHandle))?,
            ),
            parameters: CompiledParameterHandle::new(format!("test.reroute.{}", context.node_id))
                .map_err(|_| {
                LoweringError::internal(LoweringInvariant::InvalidStaticHandle)
            })?,
        })
    }
}

fn node_id(value: u128) -> NodeId {
    NodeId::from_uuid(Uuid::from_u128(value))
}

fn connection_id(value: u128) -> ConnectionId {
    ConnectionId::from_uuid(Uuid::from_u128(value))
}

fn key(value: &str) -> PortKey {
    PortKey::new(value).unwrap()
}

fn address(node: u128, port: &str) -> PortAddress {
    PortAddress::declared(node_id(node), key(port))
}

fn port(
    node_type: &str,
    name: &str,
    direction: PortDirection,
    kind: PortKind,
    value_type: TypeExpr,
) -> PortSpec {
    PortSpec {
        key: key(name),
        label_key: I18nKey::new(format!("nodes.{node_type}.{name}")).unwrap(),
        direction,
        kind,
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
        input_binding: (direction == PortDirection::Input && kind == PortKind::Data).then_some(
            InputBindingSpec {
                literal_policy: LiteralPolicy::Forbidden,
                default_value: None,
            },
        ),
        consumption: (direction == PortDirection::Input && kind == PortKind::Data)
            .then_some(InputConsumption::FullyMaterialized),
        production: (direction == PortDirection::Output && kind == PortKind::Data)
            .then_some(OutputProduction::FullyMaterialized),
        editor: PortEditorSpec::Default,
        schema: None,
    }
}

fn protocol(
    id: &str,
    ports: Vec<PortSpec>,
    type_parameters: BTreeSet<TypeParameterId>,
) -> NodeProtocol {
    let mut protocol = TestProtocolBuilder::new(id, "reroute")
        .style("test.reroute")
        .ports(ports)
        .build();
    protocol.interface.type_parameters = type_parameters
        .into_iter()
        .collect::<Vec<_>>()
        .into_boxed_slice();
    protocol
}

fn data_reroute_protocol() -> NodeProtocol {
    let generic = TypeParameterId::new("t").unwrap();
    let mut output = port(
        DATA_REROUTE_NODE_TYPE,
        REROUTE_OUTPUT_PORT,
        PortDirection::Output,
        PortKind::Data,
        TypeExpr::Generic(generic.clone()),
    );
    output.schema = Some(SchemaExpr::Input(key(REROUTE_INPUT_PORT)));
    protocol(
        DATA_REROUTE_NODE_TYPE,
        vec![
            port(
                DATA_REROUTE_NODE_TYPE,
                REROUTE_INPUT_PORT,
                PortDirection::Input,
                PortKind::Data,
                TypeExpr::Generic(generic.clone()),
            ),
            output,
        ],
        BTreeSet::from([generic]),
    )
}

fn transparent_protocol(id: &str, kind: PortKind) -> NodeProtocol {
    protocol(
        id,
        vec![
            port(
                id,
                REROUTE_INPUT_PORT,
                PortDirection::Input,
                kind,
                TypeExpr::Unknown,
            ),
            port(
                id,
                REROUTE_OUTPUT_PORT,
                PortDirection::Output,
                kind,
                TypeExpr::Unknown,
            ),
        ],
        BTreeSet::new(),
    )
}

fn registry() -> NodeRegistry {
    let concrete = TypeExpr::Concrete(TypeId::new("core.string").unwrap());
    let mut source_output = port(
        DATA_SOURCE,
        "out",
        PortDirection::Output,
        PortKind::Data,
        concrete.clone(),
    );
    source_output.schema = Some(SchemaExpr::Derived {
        resolver: SchemaResolverId::new(TEST_SCHEMA_RESOLVER).unwrap(),
        dependencies: vec![],
    });
    let nodes = vec![
        RegisteredNode::leaf(
            Arc::new(protocol(DATA_SOURCE, vec![source_output], BTreeSet::new())),
            Arc::new(NodeImplementation::new(TestLowerer)),
        ),
        RegisteredNode::leaf(
            Arc::new(protocol(
                DATA_SINK,
                vec![port(
                    DATA_SINK,
                    "in",
                    PortDirection::Input,
                    PortKind::Data,
                    concrete.clone(),
                )],
                BTreeSet::new(),
            )),
            Arc::new(NodeImplementation::new(TestLowerer)),
        ),
        RegisteredNode::leaf(
            Arc::new(protocol(
                DATA_PASS,
                vec![
                    port(
                        DATA_PASS,
                        "in",
                        PortDirection::Input,
                        PortKind::Data,
                        concrete.clone(),
                    ),
                    port(
                        DATA_PASS,
                        "out",
                        PortDirection::Output,
                        PortKind::Data,
                        concrete,
                    ),
                ],
                BTreeSet::new(),
            )),
            Arc::new(NodeImplementation::new(TestLowerer)),
        ),
        RegisteredNode::leaf(
            Arc::new(protocol(
                FLOW_NODE,
                vec![
                    port(
                        FLOW_NODE,
                        "control_in",
                        PortDirection::Input,
                        PortKind::Control,
                        TypeExpr::Unknown,
                    ),
                    port(
                        FLOW_NODE,
                        "control_out",
                        PortDirection::Output,
                        PortKind::Control,
                        TypeExpr::Unknown,
                    ),
                    port(
                        FLOW_NODE,
                        "effect_in",
                        PortDirection::Input,
                        PortKind::Effect,
                        TypeExpr::Unknown,
                    ),
                    port(
                        FLOW_NODE,
                        "effect_out",
                        PortDirection::Output,
                        PortKind::Effect,
                        TypeExpr::Unknown,
                    ),
                ],
                BTreeSet::new(),
            )),
            Arc::new(NodeImplementation::new(TestLowerer)),
        ),
        RegisteredNode::transparent(
            Arc::new(data_reroute_protocol()),
            TransparentNodeRole::Reroute,
        ),
        RegisteredNode::transparent(
            Arc::new(transparent_protocol(
                CONTROL_REROUTE_NODE_TYPE,
                PortKind::Control,
            )),
            TransparentNodeRole::Reroute,
        ),
        RegisteredNode::transparent(
            Arc::new(transparent_protocol(
                EFFECT_REROUTE_NODE_TYPE,
                PortKind::Effect,
            )),
            TransparentNodeRole::Reroute,
        ),
    ];
    let type_title = I18nKey::new("types.core.string.title").unwrap();
    let mut i18n = BTreeSet::from([
        I18nKey::new("categories.reroute.title").unwrap(),
        type_title.clone(),
    ]);
    for node in &nodes {
        i18n.insert(node.protocol().catalog.title_key.clone());
        i18n.extend(node.protocol().catalog.description_key.iter().cloned());
        i18n.extend(node.protocol().catalog.documentation_key.iter().cloned());
        for port in node.protocol().interface.ports.iter() {
            i18n.insert(port.label_key.clone());
        }
    }
    let mut provider = ProviderRegistration::new(ProviderId::new("yssbi.reroute.tests").unwrap());
    provider.types = vec![TypeRegistration {
        id: TypeId::new("core.string").unwrap(),
        title_key: type_title,
        classes: BTreeSet::new(),
    }]
    .into_boxed_slice();
    provider.categories = vec![CategoryRegistration {
        id: NodeCategoryId::new("reroute").unwrap(),
        title_key: I18nKey::new("categories.reroute.title").unwrap(),
        parent: None,
        order: 0,
    }]
    .into_boxed_slice();
    provider.i18n = I18nManifest { keys: i18n };
    provider.schema_resolvers =
        vec![SchemaResolverId::new(TEST_SCHEMA_RESOLVER).unwrap()].into_boxed_slice();
    provider.nodes = nodes.into_boxed_slice();
    let mut builder = NodeRegistryBuilder::new();
    builder.register_provider(provider).unwrap();
    builder.freeze().unwrap()
}

fn add_node(document: &mut GraphDocument, id: u128, node_type: &str) {
    let id = node_id(id);
    document.nodes.insert(
        id,
        DocumentNode {
            id,
            node_type: NodeTypeId::new(node_type).unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        },
    );
}

fn connect(
    document: &mut GraphDocument,
    id: u128,
    output_node: u128,
    output_port: &str,
    input_node: u128,
    input_port: &str,
) {
    let id = connection_id(id);
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output: address(output_node, output_port),
            input: address(input_node, input_port),
            order: None,
        },
    );
}

fn dependency_endpoints(
    dependency: &SemanticDependency<NodeId, PortAddress, ConnectionId>,
) -> Option<(NodeId, NodeId)> {
    match dependency {
        SemanticDependency::Value(edge) => Some((edge.source.node_id, edge.target.node_id)),
        SemanticDependency::Control(edge) => Some((edge.source_node, edge.target_node)),
        SemanticDependency::Effect(edge) => Some((edge.predecessor, edge.successor)),
    }
}

#[test]
fn phase2_reroute_normalization_preserves_persisted_analysis_and_collapses_semantics() {
    let registry = registry();
    let mut graph = GraphDocument::default();
    add_node(&mut graph, 1, DATA_SOURCE);
    add_node(&mut graph, 2, DATA_REROUTE_NODE_TYPE);
    add_node(&mut graph, 3, DATA_REROUTE_NODE_TYPE);
    add_node(&mut graph, 4, DATA_SINK);
    add_node(&mut graph, 5, DATA_SINK);
    connect(&mut graph, 11, 1, "out", 2, REROUTE_INPUT_PORT);
    connect(
        &mut graph,
        12,
        2,
        REROUTE_OUTPUT_PORT,
        3,
        REROUTE_INPUT_PORT,
    );
    connect(&mut graph, 13, 3, REROUTE_OUTPUT_PORT, 4, "in");
    connect(&mut graph, 14, 3, REROUTE_OUTPUT_PORT, 5, "in");

    let mut resolvers = SchemaResolverSet::new();
    resolvers.insert(
        SchemaResolverId::new(TEST_SCHEMA_RESOLVER).unwrap(),
        TestSchemaResolver,
    );
    let result =
        GraphCompiler::with_schema_resolvers(&registry, &EmptyResources, resolvers).compile(&graph);

    assert!(
        result.analysis.diagnostics.is_empty(),
        "{:?}",
        result.analysis.diagnostics
    );
    for reroute in [2, 3] {
        assert!(
            result
                .analysis
                .nodes
                .iter()
                .any(|node| node.node_id == node_id(reroute)),
            "persisted reroute {reroute} must remain in analysis"
        );
        let interface = result
            .analysis
            .resolved_interfaces
            .iter()
            .find(|interface| interface.node_id == node_id(reroute))
            .expect("persisted reroute must retain its resolved interface");
        assert!(
            interface
                .ports
                .iter()
                .any(|port| port.address == address(reroute, REROUTE_INPUT_PORT))
        );
        assert!(
            interface
                .ports
                .iter()
                .any(|port| port.address == address(reroute, REROUTE_OUTPUT_PORT))
        );
        assert!(
            result
                .interface_projection
                .nodes
                .contains_key(&node_id(reroute))
        );
    }
    for (connection, output, input) in [
        (11, address(1, "out"), address(2, REROUTE_INPUT_PORT)),
        (
            12,
            address(2, REROUTE_OUTPUT_PORT),
            address(3, REROUTE_INPUT_PORT),
        ),
        (13, address(3, REROUTE_OUTPUT_PORT), address(4, "in")),
        (14, address(3, REROUTE_OUTPUT_PORT), address(5, "in")),
    ] {
        let persisted = &graph.connections[&connection_id(connection)];
        assert_eq!(persisted.output, output);
        assert_eq!(persisted.input, input);
    }

    let semantic = result
        .semantic
        .as_ref()
        .expect("reroute graph must retain normalized semantic output");
    assert!(
        semantic
            .nodes
            .iter()
            .all(|node| node.node_id != node_id(2) && node.node_id != node_id(3))
    );
    assert_eq!(semantic.dependencies.len(), 2);
    assert!(semantic.dependencies.iter().all(|dependency| {
        matches!(dependency_endpoints(dependency), Some((source, target)) if source == node_id(1) && [node_id(4), node_id(5)].contains(&target))
    }));
    for sink in [4, 5] {
        let port = semantic
            .nodes
            .iter()
            .find(|node| node.node_id == node_id(sink))
            .unwrap()
            .ports
            .iter()
            .find(|port| port.address == address(sink, "in"))
            .unwrap();
        assert_eq!(
            port.resolved_type,
            Some(TypeExpr::Concrete(TypeId::new("core.string").unwrap()))
        );
        assert!(port.resolved_schema.is_some());
        assert_eq!(
            semantic.resolved_schemas[&address(sink, "in")].fields,
            vec![SchemaField::from(SchemaColumnRef("value".into()))]
        );
    }
    let plan = result
        .plan
        .as_ref()
        .expect("transparent reroutes must lower without identity operations");
    assert!(
        plan.operations
            .iter()
            .all(|operation| operation.source_node_id != node_id(2)
                && operation.source_node_id != node_id(3))
    );
}

#[test]
fn phase2_reroute_compile_control_and_effect_preserve_direction_without_runtime_identity() {
    let registry = registry();
    let mut graph = GraphDocument::default();
    add_node(&mut graph, 1, FLOW_NODE);
    add_node(&mut graph, 2, FLOW_NODE);
    add_node(&mut graph, 3, CONTROL_REROUTE_NODE_TYPE);
    add_node(&mut graph, 4, EFFECT_REROUTE_NODE_TYPE);
    connect(&mut graph, 21, 1, "control_out", 3, REROUTE_INPUT_PORT);
    connect(&mut graph, 22, 3, REROUTE_OUTPUT_PORT, 2, "control_in");
    connect(&mut graph, 23, 1, "effect_out", 4, REROUTE_INPUT_PORT);
    connect(&mut graph, 24, 4, REROUTE_OUTPUT_PORT, 2, "effect_in");

    let result = GraphCompiler::new(&registry, &EmptyResources).compile(&graph);

    assert!(
        result.analysis.diagnostics.is_empty(),
        "{:?}",
        result.analysis.diagnostics
    );
    let semantic = result.semantic.as_ref().unwrap();
    assert_eq!(semantic.dependencies.len(), 2);
    assert!(
        semantic
            .dependencies
            .iter()
            .all(|dependency| dependency_endpoints(dependency) == Some((node_id(1), node_id(2))))
    );
    let plan = result
        .plan
        .as_ref()
        .expect("transparent flow reroutes must lower");
    assert_eq!(plan.operations.len(), 2);
    assert_eq!(plan.effect_dependencies.len(), 1);
    assert!(
        plan.operations
            .iter()
            .all(|operation| operation.source_node_id != node_id(3)
                && operation.source_node_id != node_id(4))
    );

    let mut cycle = GraphDocument::default();
    add_node(&mut cycle, 11, DATA_PASS);
    add_node(&mut cycle, 12, DATA_PASS);
    add_node(&mut cycle, 13, DATA_REROUTE_NODE_TYPE);
    connect(&mut cycle, 31, 11, "out", 13, REROUTE_INPUT_PORT);
    connect(&mut cycle, 32, 13, REROUTE_OUTPUT_PORT, 12, "in");
    connect(&mut cycle, 33, 12, "out", 11, "in");

    let cycle_result = GraphCompiler::new(&registry, &EmptyResources).compile(&cycle);
    assert!(cycle_result.plan.is_none());
    assert!(
        cycle_result
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| { diagnostic.code.as_str() == "compiler.dependency.value_cycle" })
    );
}

#[test]
fn phase2_reroute_compile_called_function_is_transparent_to_dependency_lowering() {
    struct FunctionResources {
        path: GraphResourcePath,
        function: crate::node_system::document::FunctionDocument,
        graph: GraphDocument,
    }

    impl ResourceSnapshot for FunctionResources {
        fn versions(&self) -> ResourceVersionSet {
            BTreeMap::from([(
                crate::node_system::analysis::ResourceKey::new(self.path.0.as_ref()),
                crate::node_system::analysis::ResourceVersion::new("callee-v1"),
            )])
        }

        fn function_name(&self, path: &GraphResourcePath) -> Option<&str> {
            (path == &self.path).then_some("Reroute callee")
        }

        fn function_document(
            &self,
            path: &GraphResourcePath,
        ) -> Option<&crate::node_system::document::FunctionDocument> {
            (path == &self.path).then_some(&self.function)
        }

        fn function_graph_document(&self, path: &GraphResourcePath) -> Option<&GraphDocument> {
            (path == &self.path).then_some(&self.graph)
        }
    }

    let builtin = crate::node_system::catalog::build_builtin_node_system().unwrap();
    let path = GraphResourcePath("functions/reroute-callee".into());
    let mut callee = GraphDocument::default();
    add_node(&mut callee, 20, "yssbi.project.function.entry");
    add_node(&mut callee, 21, CONTROL_REROUTE_NODE_TYPE);
    add_node(&mut callee, 22, "yssbi.project.function.return");
    callee
        .nodes
        .get_mut(&node_id(20))
        .unwrap()
        .parameters
        .insert(
            ParameterKey::new("function").unwrap(),
            serde_json::json!(path.0.as_ref()),
        );
    callee
        .nodes
        .get_mut(&node_id(22))
        .unwrap()
        .parameters
        .insert(
            ParameterKey::new("function").unwrap(),
            serde_json::json!(path.0.as_ref()),
        );
    connect(&mut callee, 101, 20, "then", 21, REROUTE_INPUT_PORT);
    connect(&mut callee, 102, 21, REROUTE_OUTPUT_PORT, 22, "enter");
    let resources = FunctionResources {
        path: path.clone(),
        function: crate::node_system::document::FunctionDocument::new(
            crate::node_system::document::FunctionSignature {
                parameters: Vec::new(),
                return_type: None,
            },
        ),
        graph: callee,
    };
    let mut caller = GraphDocument::default();
    add_node(&mut caller, 1, "yssbi.project.function.call");
    caller
        .nodes
        .get_mut(&node_id(1))
        .unwrap()
        .parameters
        .insert(
            ParameterKey::new("target").unwrap(),
            serde_json::json!(path.0.as_ref()),
        );

    let result = GraphCompiler::with_interface_resolvers(
        builtin.registry.as_ref(),
        &resources,
        build_builtin_interface_resolvers(),
    )
    .compile(&caller);

    assert!(
        result.analysis.diagnostics.is_empty(),
        "{:?}",
        result.analysis.diagnostics
    );
    let plan = result
        .plan
        .expect("called function reroute must not invalidate dependency lowering");
    assert!(
        plan.operations
            .iter()
            .all(|operation| operation.source_node_id != node_id(21))
    );
}

#[test]
fn phase2_reroute_connection_limit_overcapacity_is_blocking() {
    let registry = registry();
    let mut graph = GraphDocument::default();
    add_node(&mut graph, 1, DATA_SOURCE);
    add_node(&mut graph, 2, DATA_SOURCE);
    add_node(&mut graph, 3, DATA_REROUTE_NODE_TYPE);
    connect(&mut graph, 41, 1, "out", 3, REROUTE_INPUT_PORT);
    connect(&mut graph, 42, 2, "out", 3, REROUTE_INPUT_PORT);

    let result = GraphCompiler::new(&registry, &EmptyResources).compile(&graph);

    assert_eq!(result.outcome, CompilationOutcome::AnalysisBlocked);
    assert!(result.plan.is_none());
    assert!(result.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "compiler.connection.limit"
            && diagnostic.severity == DiagnosticSeverity::Error
            && matches!(diagnostic.primary, DiagnosticLocation::Port(ref port) if *port == address(3, REROUTE_INPUT_PORT))
    }));
}
