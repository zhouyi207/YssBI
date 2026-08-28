use super::*;
use crate::execution::plan::{
    PlanCompilationBasis, PlanGraphRevision, PlanProjectSessionId, PlanRegistryFingerprint,
};
use crate::graph::analysis::{
    GraphNodeProjectionFacts, GraphPortConnectionFacts, GraphPortEditorFact,
    GraphPortInstanceKind, GraphProjectionFacts,
};
use crate::graph::resource_catalog::{ResourceCatalogFingerprint, ResourceCatalogSnapshot};
use crate::graph::settings::GraphCompileSettings;
use crate::graph_document::{
    ConnectionId, DocumentConnection, DocumentNode, GraphDocument, GraphResourcePath,
    GraphRevision, NodeId, NodePosition, ParameterValues, PortAddress,
};
use crate::node_system::analysis::{ResourceKey, ResourceVersion};
use crate::node_system::protocol::{
    NodeTypeId, ParameterEditorSpec, ParameterKey, ParameterPresentation, PortDirection, PortKey,
    PortKind, TypeExpr, TypeId,
};
use std::collections::BTreeMap;

fn node_id(value: u128) -> NodeId {
    NodeId::from_uuid(uuid::Uuid::from_u128(value))
}

fn connection_id(value: u128) -> ConnectionId {
    ConnectionId::from_uuid(uuid::Uuid::from_u128(value))
}

fn bool_type() -> TypeExpr {
    TypeExpr::Concrete(TypeId::new("core.bool").expect("test type id is valid"))
}

fn port(
    address: PortAddress,
    template_key: &str,
    label: &str,
    direction: PortDirection,
    current: u32,
) -> crate::graph::analysis::GraphPortFact {
    crate::graph::analysis::GraphPortFact {
        address,
        template_key: PortKey::new(template_key).expect("test port key is valid"),
        label: label.into(),
        instance_label: None,
        direction,
        kind: PortKind::Data,
        instance_kind: GraphPortInstanceKind::Declared,
        orphan: false,
        connections: GraphPortConnectionFacts {
            current,
            maximum: Some(1),
            ordered: false,
        },
        member_minimum: 0,
        member_instance_count: 0,
        member_complete: true,
        editor: GraphPortEditorFact::Default,
        protocol_default: None,
        value_type: bool_type(),
        schema: None,
        resolved_schema: None,
    }
}

fn node_facts(
    node_id: NodeId,
    node_type: NodeTypeId,
    ports: Box<[crate::graph::analysis::GraphPortFact]>,
) -> GraphNodeProjectionFacts {
    GraphNodeProjectionFacts {
        node_id,
        node_type,
        instance_title: None,
        title: "Boolean Constant".into(),
        icon_id: Some("builtin.constants".into()),
        style_id: Some("builtin.default".into()),
        managed: false,
        parameters: Box::new([crate::graph::analysis::GraphParameterFact {
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
    }
}

fn analysis_with_facts(
    document: &GraphDocument,
    path: &GraphResourcePath,
    facts: GraphProjectionFacts,
) -> crate::graph::analysis::GraphAnalysis {
    let basis = PlanCompilationBasis::new(
        PlanProjectSessionId::from_existing("project".into()),
        PlanGraphRevision::from_existing(document.revision.get()),
        PlanRegistryFingerprint::from_bytes([6; 32]),
        std::collections::BTreeMap::from([(
            crate::execution::plan::PlanResourceId::new("resource/source".into())
                .expect("test resource id is valid"),
            crate::execution::plan::PlanResourceVersion::new("7".into())
                .expect("test resource version is valid"),
        )]),
        BTreeMap::new(),
    );
    let catalog = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
        ResourceCatalogFingerprint::from_bytes([1; 32]),
    );
    let settings = GraphCompileSettings {
        absolute_tolerance: 1e-12,
        relative_tolerance: 1e-9,
    };
    let _ = path;
    crate::graph::analysis::analyze(crate::graph::analysis::GraphAnalysisInput {
        document,
        catalog: &catalog,
        settings: &settings,
        basis: &basis,
    })
    .with_projection_facts(facts)
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
    let mut document = GraphDocument::default();
    document.revision = GraphRevision::new(7);
    document.nodes.insert(
        source,
        DocumentNode {
            id: source,
            node_type: source_type.clone(),
            position: NodePosition { x: 120.5, y: -32.0 },
            parameters: ParameterValues::from([(
                ParameterKey::new("value").expect("test parameter key is valid"),
                crate::graph_document::TypedValue::Bool(true),
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
    let path = GraphResourcePath::new("events/contract.yssbi-event")
        .expect("test graph path is valid");
    let facts = GraphProjectionFacts::new(
        [
            node_facts(source, source_type, Box::new([port(output, "value", "Value", PortDirection::Output, 1)])),
            node_facts(target, target_type, Box::new([port(input, "value", "Value", PortDirection::Input, 1)])),
        ],
        [],
        crate::graph::analysis::GraphCompilationOutcome::Complete,
    );
    let analysis = analysis_with_facts(&document, &path, facts);

    let model = build_editor_projection(EditorProjectionInput {
        graph_path: &path,
        document: &document,
        analysis: &analysis,
        registry_fingerprint: [6; 32],
    })
    .expect("complete neutral facts should produce an application model");

    assert_eq!(model.basis.graph_revision, GraphRevision::new(7));
    assert_eq!(
        model.basis.resource_versions.get(&ResourceKey::new("resource/source".into())),
        Some(&ResourceVersion::new("7".into()))
    );
    assert_eq!(model.nodes.len(), 2);
    assert_eq!(
        model.nodes[0].parameters[0].value,
        Some(crate::graph_document::TypedValue::Bool(true))
    );
    assert_eq!(model.nodes[0].display.user_label.as_deref(), Some("Contract Boolean"));
    assert_eq!(model.nodes[1].ports[0].input.as_ref().map(|input| input.effective), Some(EditorEffectiveInputBinding::Connections));
    assert_eq!(model.connections[0].output, output);
    assert_eq!(model.connections[0].input, input);
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
    let basis = PlanCompilationBasis::new(
        PlanProjectSessionId::from_existing("project".into()),
        PlanGraphRevision::from_existing(0),
        PlanRegistryFingerprint::from_bytes([9; 32]),
        BTreeMap::new(),
        BTreeMap::new(),
    );
    let catalog = ResourceCatalogSnapshot::new(
        BTreeMap::new(),
        BTreeMap::new(),
        BTreeMap::new(),
        ResourceCatalogFingerprint::from_bytes([1; 32]),
    );
    let settings = GraphCompileSettings {
        absolute_tolerance: 1e-12,
        relative_tolerance: 1e-9,
    };
    let analysis = crate::graph::analysis::analyze(crate::graph::analysis::GraphAnalysisInput {
        document: &document,
        catalog: &catalog,
        settings: &settings,
        basis: &basis,
    });

    assert!(matches!(
        build_editor_projection(EditorProjectionInput {
            graph_path: &path,
            document: &document,
            analysis: &analysis,
            registry_fingerprint: [9; 32],
        }),
        Err(EditorProjectionError::MissingProjectionFacts)
    ));
}
