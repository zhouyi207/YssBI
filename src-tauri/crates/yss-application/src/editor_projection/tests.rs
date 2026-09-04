use super::*;
use std::collections::BTreeMap;
use yss_graph_analysis::{
    GraphCompilationOutcome, GraphDiagnosticFact, GraphDiagnosticLocation, GraphNodeSemanticFact,
    GraphPortBacking, GraphPortConnectionFacts, GraphPortEditorFact, GraphPortSemanticFact,
    GraphSemanticSnapshot,
};
use yss_graph_analysis_contract::{
    CompilationBasis, DiagnosticCode, DiagnosticSeverity, ResourceKey, ResourceVersion,
};
use yss_graph_document::{
    ConnectionId, DocumentConnection, DocumentNode, GraphDocument, GraphResourcePath,
    GraphRevision, InputState, NodeId, NodePosition, ParameterValues, PortAddress,
};
use yss_graph_protocol::{
    NodeTypeId, ParameterEditorSpec, ParameterKey, ParameterPresentation, PortDirection, PortKey,
    ResolvedType, TypeDomain, TypeExpr, TypeId, TypeState, TypedValue, Value,
};
use yss_graph_registry::RegistryFingerprint;

fn node_id(value: u128) -> NodeId {
    NodeId::from_uuid(uuid::Uuid::from_u128(value))
}

fn connection_id(value: u128) -> ConnectionId {
    ConnectionId::from_uuid(uuid::Uuid::from_u128(value))
}

fn bool_type() -> TypeExpr {
    TypeExpr::Concrete(TypeId::new("core.bool").expect("test type id is valid"))
}

fn diagnostic_codes(diagnostics: &[EditorDiagnosticModel]) -> Vec<&str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_ref())
        .collect()
}

fn port(
    address: PortAddress,
    label: &str,
    direction: PortDirection,
    current: u32,
) -> GraphPortSemanticFact {
    GraphPortSemanticFact {
        address,
        label: label.into(),
        instance_label: None,
        direction,
        backing: GraphPortBacking::Declared,
        orphan: false,
        can_remove: false,
        connections: GraphPortConnectionFacts {
            current,
            maximum: Some(1),
            ordered: false,
        },
        editor: GraphPortEditorFact::Default,
        protocol_default: None,
        accepted_type: bool_type(),
        accepted_domain: Some(TypeDomain::singleton(ResolvedType::Nominal(
            TypeId::new("core.bool").expect("test type id is valid"),
        ))),
        type_state: TypeState::Exact(ResolvedType::Nominal(
            TypeId::new("core.bool").expect("test type id is valid"),
        )),
        schema: None,
        resolved_schema: None,
    }
}

fn node_facts(
    node_id: NodeId,
    node_type: NodeTypeId,
    ports: Box<[GraphPortSemanticFact]>,
) -> GraphNodeSemanticFact {
    GraphNodeSemanticFact {
        node_id,
        node_type,
        instance_title: None,
        title: "Boolean Constant".into(),
        icon_id: Some("builtin.constants".into()),
        style_id: Some("builtin.default".into()),
        managed: false,
        parameters: Box::new([yss_graph_analysis::GraphParameterFact {
            key: ParameterKey::new("value").expect("test parameter key is valid"),
            title: "Value".into(),
            description: Some("The constant value.".into()),
            editor: ParameterEditorSpec::Toggle,
            presentation: ParameterPresentation::InlineAndDetail,
            value_type: bool_type(),
            inherited_value: None,
            value_source: None,
            options: Box::new([]),
            configuration: None,
        }]),
        ports,
        port_instance_additions: Box::new([]),
        specialization: None,
        semantic_fingerprint: [0; 32],
    }
}

fn analysis_with_facts(
    document: &GraphDocument,
    _path: &GraphResourcePath,
    facts: GraphSemanticSnapshot,
) -> yss_graph_analysis::GraphAnalysis {
    let basis = CompilationBasis {
        graph_revision: GraphRevision::new(document.revision.get()),
        registry_fingerprint: RegistryFingerprint::from_bytes([6; 32]),
        resource_versions: BTreeMap::from([(
            ResourceKey::new("resource/source"),
            ResourceVersion::new("7"),
        )]),
        resource_observations: BTreeMap::new(),
    };
    yss_graph_analysis::analyze(&basis, facts)
}

#[test]
fn application_projection_closes_resource_node_port_and_connection_facts() {
    let source = node_id(2);
    let target = node_id(3);
    let source_type = NodeTypeId::new("yssbi.constant.bool").expect("test node type is valid");
    let target_type = NodeTypeId::new("yssbi.constant.bool").expect("test node type is valid");
    let output = PortAddress::declared(
        source,
        PortKey::new("value").expect("test port key is valid"),
    );
    let input = PortAddress::declared(
        target,
        PortKey::new("value").expect("test port key is valid"),
    );
    let connection = connection_id(4);
    let mut document = GraphDocument {
        revision: GraphRevision::new(7),
        ..GraphDocument::default()
    };
    document.nodes.insert(
        source,
        DocumentNode {
            id: source,
            node_type: source_type.clone(),
            position: NodePosition { x: 120.5, y: -32.0 },
            parameters: ParameterValues::from([(
                ParameterKey::new("value").expect("test parameter key is valid"),
                serde_json::Value::Bool(true),
            )]),
            user_label: Some("Contract Boolean".to_owned()),
        },
    );
    document.nodes.insert(
        target,
        DocumentNode {
            id: target,
            node_type: target_type.clone(),
            position: NodePosition { x: 240.0, y: -32.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        },
    );
    document.connections.insert(
        connection,
        DocumentConnection {
            id: connection,
            output: output.clone(),
            input: input.clone(),
            order: None,
        },
    );
    document.input_states.insert(
        input.clone(),
        InputState {
            literal_override: Some(TypedValue {
                value_type: bool_type(),
                value: Value::Bool(true),
            }),
        },
    );
    let path =
        GraphResourcePath::new("events/contract.yssbi-event").expect("test graph path is valid");
    let facts = GraphSemanticSnapshot::new(
        [
            node_facts(
                source,
                source_type,
                Box::new([port(output.clone(), "Value", PortDirection::Output, 1)]),
            ),
            node_facts(
                target,
                target_type,
                Box::new([port(input.clone(), "Value", PortDirection::Input, 1)]),
            ),
        ],
        [],
        yss_graph_analysis::GraphCompilationOutcome::Complete,
    );
    let analysis = analysis_with_facts(&document, &path, facts);

    let model = build_editor_projection(EditorProjectionInput {
        graph_path: &path,
        document: &document,
        analysis: &analysis,
        registry_fingerprint: [6; 32],
    })
    .expect("complete neutral facts should produce an application model");

    assert_eq!(
        model
            .basis
            .resource_versions
            .get(&ResourceKey::new("resource/source")),
        Some(&ResourceVersion::new("7"))
    );
    assert_eq!(model.nodes.len(), 2);
    assert_eq!(
        model.nodes[0].parameters[0].value,
        Some(serde_json::Value::Bool(true))
    );
    assert_eq!(
        model.nodes[0].display.user_label.as_deref(),
        Some("Contract Boolean")
    );
    assert_eq!(
        model.nodes[1].ports[0]
            .input
            .as_ref()
            .map(|input| input.effective),
        Some(EditorEffectiveInputBinding::Connections)
    );
    assert_eq!(
        model.nodes[1].ports[0]
            .input
            .as_ref()
            .and_then(|input| input.literal_override.as_ref()),
        Some(&serde_json::Value::Bool(true))
    );
    assert_eq!(model.connections[0].output, output);
    assert_eq!(model.connections[0].input, input);
}

#[test]
fn application_projection_preserves_canonical_diagnostics_and_builds_node_indexes() {
    let source = node_id(12);
    let target = node_id(13);
    let source_type = NodeTypeId::new("yssbi.constant.bool").expect("test node type is valid");
    let target_type = NodeTypeId::new("yssbi.constant.bool").expect("test node type is valid");
    let output = PortAddress::declared(
        source,
        PortKey::new("value").expect("test port key is valid"),
    );
    let input = PortAddress::declared(
        target,
        PortKey::new("value").expect("test port key is valid"),
    );
    let connection = connection_id(14);
    let mut document = GraphDocument::default();
    document.nodes.insert(
        source,
        DocumentNode {
            id: source,
            node_type: source_type.clone(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        },
    );
    document.nodes.insert(
        target,
        DocumentNode {
            id: target,
            node_type: target_type.clone(),
            position: NodePosition { x: 100.0, y: 0.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        },
    );
    document.connections.insert(
        connection,
        DocumentConnection {
            id: connection,
            output: output.clone(),
            input: input.clone(),
            order: None,
        },
    );
    let problem = |code: &str, primary: GraphDiagnosticLocation| GraphDiagnosticFact {
        code: DiagnosticCode::new(code),
        severity: DiagnosticSeverity::Error,
        arguments: BTreeMap::new(),
        primary,
        related: Box::new([]),
    };
    let diagnostics = [
        problem(
            "compiler.dependency.value_cycle",
            GraphDiagnosticLocation::Graph,
        ),
        problem(
            "compiler.resource.resolution_failed",
            GraphDiagnosticLocation::Resource("database/source".into()),
        ),
        problem(
            "compiler.connection.limit",
            GraphDiagnosticLocation::Connection(connection),
        ),
        problem(
            "compiler.parameter.invalid",
            GraphDiagnosticLocation::Parameter {
                node_id: source,
                key: ParameterKey::new("value").expect("test parameter key is valid"),
            },
        ),
    ];
    let facts = GraphSemanticSnapshot::new(
        [
            node_facts(
                source,
                source_type,
                Box::new([port(output, "Value", PortDirection::Output, 1)]),
            ),
            node_facts(
                target,
                target_type,
                Box::new([port(input, "Value", PortDirection::Input, 1)]),
            ),
        ],
        diagnostics,
        yss_graph_analysis::GraphCompilationOutcome::Incomplete,
    );
    let path =
        GraphResourcePath::new("events/problems.yssbi-event").expect("test graph path is valid");
    let analysis = analysis_with_facts(&document, &path, facts);

    let model = build_editor_projection(EditorProjectionInput {
        graph_path: &path,
        document: &document,
        analysis: &analysis,
        registry_fingerprint: [6; 32],
    })
    .expect("diagnostic facts should produce an application model");

    assert_eq!(
        diagnostic_codes(&model.diagnostics),
        [
            "compiler.dependency.value_cycle",
            "compiler.resource.resolution_failed",
            "compiler.connection.limit",
            "compiler.parameter.invalid",
        ]
    );
    let source_node = model
        .nodes
        .iter()
        .find(|node| node.node_id == source)
        .expect("source node is projected");
    assert_eq!(
        diagnostic_codes(&source_node.diagnostics),
        ["compiler.connection.limit", "compiler.parameter.invalid"]
    );
    let target_node = model
        .nodes
        .iter()
        .find(|node| node.node_id == target)
        .expect("target node is projected");
    assert_eq!(
        diagnostic_codes(&target_node.diagnostics),
        ["compiler.connection.limit"]
    );
}

#[test]
fn application_projection_fails_closed_when_nonempty_graph_lacks_neutral_facts() {
    let node = node_id(8);
    let node_type = NodeTypeId::new("yssbi.constant.bool").expect("test node type is valid");
    let mut document = GraphDocument::default();
    document.nodes.insert(
        node,
        DocumentNode {
            id: node,
            node_type,
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        },
    );
    let path = GraphResourcePath::new("events/missing-facts.yssbi-event")
        .expect("test graph path is valid");
    let basis = CompilationBasis {
        graph_revision: GraphRevision::new(0),
        registry_fingerprint: RegistryFingerprint::from_bytes([9; 32]),
        resource_versions: BTreeMap::new(),
        resource_observations: BTreeMap::new(),
    };
    let analysis = yss_graph_analysis::analyze(
        &basis,
        GraphSemanticSnapshot::new([], [], GraphCompilationOutcome::Complete),
    );

    assert!(matches!(
        build_editor_projection(EditorProjectionInput {
            graph_path: &path,
            document: &document,
            analysis: &analysis,
            registry_fingerprint: [9; 32],
        }),
        Err(EditorProjectionError::SemanticSnapshotMismatch)
    ));
}
