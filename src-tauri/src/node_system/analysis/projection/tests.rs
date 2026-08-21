use super::*;
use crate::graph::value::DataType;
use crate::node_system::analysis::{ResourceVersionSet, SemanticDependency};
use crate::node_system::catalog::{
    CONTROL_REROUTE_NODE_TYPE, DATA_REROUTE_NODE_TYPE, EFFECT_REROUTE_NODE_TYPE,
    REROUTE_INPUT_PORT, REROUTE_OUTPUT_PORT, build_builtin_node_system,
};
use crate::node_system::compiler::{CompilationOutcome, GraphCompiler, ResourceSnapshot};
use crate::node_system::document::{
    ConnectionId, DocumentConnection, DocumentNode, DynamicMemberLocator, DynamicPortBinding,
    FunctionDocument, FunctionParameterId, FunctionSignature, GraphDocument, GraphResourcePath,
    InputState, LastKnownPortMetadata, NodeId, NodePosition, OrderKey, PortAddress, PortAddressDto,
    PortInstanceId,
};
use crate::node_system::protocol::{
    ConnectionsPerPort, NodeTypeId, ParameterKey, PortKey, SchemaExpr, TypeConstructorId, TypeExpr,
    TypeId, TypedValue, Value, numeric_data_series_type,
};
use crate::node_system::registry::{RegistryFingerprint, TransparentNodeRole};
use serde_json::json;
use std::collections::BTreeMap;
use uuid::Uuid;

#[test]
fn function_data_types_accept_canonical_display_strings() {
    let cases = [
        ("Boolean", DataType::Boolean),
        ("Int64", DataType::Int64),
        ("Float64", DataType::Float64),
        ("String", DataType::String),
        ("Object", DataType::Object),
        ("Number", DataType::number()),
    ];

    for (type_name, expected) in cases {
        assert_eq!(resolve_function_data_type(type_name), Ok(expected));
    }
}

#[test]
fn function_data_types_reject_noncanonical_aliases() {
    for type_name in [
        "bool",
        "boolean",
        "core.bool",
        "int",
        "integer",
        "int64",
        "core.int64",
        "float",
        "float64",
        "number",
        "core.float64",
        "string",
        "core.string",
        "json",
        "object",
    ] {
        assert!(
            resolve_function_data_type(type_name).is_err(),
            "noncanonical function data type alias {type_name:?} was accepted"
        );
    }
}

#[test]
fn projection_projects_unknown_without_any() {
    let summary = project_type_summary(&TypeExpr::Unknown);

    assert!(!summary.resolved);
    assert_eq!(summary.data_type, None);
}

#[test]
fn projection_projects_canonical_numeric_series_union() {
    let summary = project_type_summary(&numeric_data_series_type());

    assert!(summary.resolved);
    assert_eq!(
        summary.display.as_ref(),
        "core.data_series<core.float64> | core.data_series<core.int64>"
    );
    assert_eq!(
        summary.data_type,
        Some(DataType::one_of(vec![
            DataType::DataSeries(Box::new(DataType::Float64)),
            DataType::DataSeries(Box::new(DataType::Int64)),
        ]))
    );
}

#[test]
fn type_summary_serializes_structured_data_type_without_display_parsing() {
    let summary = project_type_summary(&TypeExpr::Applied {
        constructor: TypeConstructorId::new("core.data_series").unwrap(),
        arguments: vec![TypeExpr::Concrete(TypeId::new("core.float64").unwrap())],
    });

    assert_eq!(
        serde_json::to_value(summary).unwrap(),
        json!({
            "display": "core.data_series<core.float64>",
            "resolved": true,
            "dataType": {
                "kind": "DataSeries",
                "inner": { "kind": "Float64" }
            }
        })
    );
    assert_eq!(
        project_data_type(&TypeExpr::Concrete(
            TypeId::new("statistics.model").unwrap()
        )),
        Some(DataType::Struct("statistics.model".into()))
    );
    assert_eq!(
        project_data_type(&TypeExpr::Generic(
            crate::node_system::protocol::TypeParameterId::new("value").unwrap()
        )),
        None
    );
}

fn connection_capability(
    capability: ConnectionsPerPort,
    orphan: bool,
    current: u128,
) -> PortConnectionCapabilityDto {
    let address = PortAddress::declared(
        NodeId::from_uuid(Uuid::from_u128(10)),
        PortKey::new("port").unwrap(),
    );
    let mut document = GraphDocument::default();
    for index in 0..current {
        let connection_id = ConnectionId::from_uuid(Uuid::from_u128(100 + index));
        document.connections.insert(
            connection_id,
            DocumentConnection {
                id: connection_id,
                output: address.clone(),
                input: PortAddress::declared(
                    NodeId::from_uuid(Uuid::from_u128(1_000 + index)),
                    PortKey::new("input").unwrap(),
                ),
                order: None,
            },
        );
    }
    project_connection_capability(&document, &address, capability, orphan)
}

#[test]
fn projects_connection_capabilities_single_append_replace_and_move_truth_table() {
    assert_eq!(
        connection_capability(ConnectionsPerPort::Single, false, 0),
        PortConnectionCapabilityDto {
            current: 0,
            maximum: Some(1),
            ordered: false,
            can_append: true,
            can_replace: false,
            can_move: false,
        }
    );
    let occupied = connection_capability(ConnectionsPerPort::Single, false, 1);
    assert_eq!(
        occupied,
        PortConnectionCapabilityDto {
            current: 1,
            maximum: Some(1),
            ordered: false,
            can_append: false,
            can_replace: true,
            can_move: true,
        }
    );
    assert_eq!(
        serde_json::to_value(occupied).unwrap(),
        json!({
            "current": 1,
            "maximum": 1,
            "ordered": false,
            "canAppend": false,
            "canReplace": true,
            "canMove": true
        })
    );
}

#[test]
fn projects_connection_capabilities_loaded_single_overflow_is_never_replaceable() {
    assert_eq!(
        connection_capability(ConnectionsPerPort::Single, false, 2),
        PortConnectionCapabilityDto {
            current: 2,
            maximum: Some(1),
            ordered: false,
            can_append: false,
            can_replace: false,
            can_move: true,
        }
    );
}

#[test]
fn projects_connection_capabilities_multiple_capacity_and_order_truth_table() {
    let bounded = ConnectionsPerPort::Multiple {
        max: Some(2),
        ordered: false,
    };
    assert_eq!(
        connection_capability(bounded, false, 1),
        PortConnectionCapabilityDto {
            current: 1,
            maximum: Some(2),
            ordered: false,
            can_append: true,
            can_replace: false,
            can_move: true,
        }
    );
    assert_eq!(
        connection_capability(bounded, false, 2),
        PortConnectionCapabilityDto {
            current: 2,
            maximum: Some(2),
            ordered: false,
            can_append: false,
            can_replace: false,
            can_move: true,
        }
    );
    assert_eq!(
        connection_capability(
            ConnectionsPerPort::Multiple {
                max: None,
                ordered: true,
            },
            false,
            2,
        ),
        PortConnectionCapabilityDto {
            current: 2,
            maximum: None,
            ordered: true,
            can_append: true,
            can_replace: false,
            can_move: true,
        }
    );
}

#[test]
fn projects_connection_capabilities_disable_orphans_and_track_connected_control_effect_ports() {
    assert_eq!(
        connection_capability(ConnectionsPerPort::Single, true, 1),
        PortConnectionCapabilityDto {
            current: 1,
            maximum: Some(1),
            ordered: false,
            can_append: false,
            can_replace: false,
            can_move: false,
        }
    );
    let connected_control = connection_capability(ConnectionsPerPort::Single, false, 1);
    let connected_effect = connection_capability(
        ConnectionsPerPort::Multiple {
            max: None,
            ordered: true,
        },
        false,
        1,
    );
    assert!(connected_control.can_replace && connected_control.can_move);
    assert!(connected_effect.can_append && connected_effect.can_move);
}

struct EmptyResources;

impl ResourceSnapshot for EmptyResources {
    fn versions(&self) -> ResourceVersionSet {
        BTreeMap::new()
    }
}

fn insert_projection_node(
    document: &mut GraphDocument,
    id: u128,
    node_type: &str,
    position: NodePosition,
) -> NodeId {
    let id = NodeId::from_uuid(Uuid::from_u128(id));
    document.nodes.insert(
        id,
        DocumentNode {
            id,
            node_type: NodeTypeId::new(node_type).unwrap(),
            position,
            parameters: BTreeMap::new(),
            user_label: None,
        },
    );
    id
}

fn insert_projection_connection(
    document: &mut GraphDocument,
    id: u128,
    output: (NodeId, &str),
    input: (NodeId, &str),
) {
    let id = ConnectionId::from_uuid(Uuid::from_u128(id));
    document.connections.insert(
        id,
        DocumentConnection {
            id,
            output: PortAddress::declared(output.0, PortKey::new(output.1).unwrap()),
            input: PortAddress::declared(input.0, PortKey::new(input.1).unwrap()),
            order: None,
        },
    );
}

fn reroute_projection_document(include_data_endpoints: bool) -> GraphDocument {
    let mut document = GraphDocument::default();
    for (index, (reroute_type, endpoint_type, output_port, input_port)) in [
        (DATA_REROUTE_NODE_TYPE, "yssbi.logic.not", "result", "input"),
        (
            CONTROL_REROUTE_NODE_TYPE,
            "yssbi.control.do",
            "then",
            "enter",
        ),
        (
            EFFECT_REROUTE_NODE_TYPE,
            "yssbi.control.do",
            "effect_out",
            "effect_in",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let y = 80.0 + index as f64 * 60.0;
        let source = (index != 0 || include_data_endpoints).then(|| {
            insert_projection_node(
                &mut document,
                10 + index as u128 * 2,
                endpoint_type,
                NodePosition { x: 0.0, y },
            )
        });

        if index == 0 && include_data_endpoints {
            document.input_states.insert(
                PortAddress::declared(source.unwrap(), PortKey::new("input").unwrap()),
                InputState {
                    literal_override: Some(
                        serde_json::to_value(TypedValue {
                            value_type: TypeExpr::Concrete(TypeId::new("core.bool").unwrap()),
                            value: Value::Bool(true),
                        })
                        .unwrap(),
                    ),
                },
            );
        }
        let first = insert_projection_node(
            &mut document,
            100 + index as u128 * 2,
            reroute_type,
            NodePosition { x: 40.0, y },
        );
        let second = insert_projection_node(
            &mut document,
            101 + index as u128 * 2,
            reroute_type,
            NodePosition { x: 140.0, y },
        );
        let target = (index != 0 || include_data_endpoints).then(|| {
            insert_projection_node(
                &mut document,
                11 + index as u128 * 2,
                endpoint_type,
                NodePosition { x: 180.0, y },
            )
        });
        if index != 0 || include_data_endpoints {
            insert_projection_connection(
                &mut document,
                300 + index as u128 * 3,
                (source.unwrap(), output_port),
                (first, REROUTE_INPUT_PORT),
            );
        }
        insert_projection_connection(
            &mut document,
            301 + index as u128 * 3,
            (first, REROUTE_OUTPUT_PORT),
            (second, REROUTE_INPUT_PORT),
        );
        if index != 0 || include_data_endpoints {
            insert_projection_connection(
                &mut document,
                302 + index as u128 * 3,
                (second, REROUTE_OUTPUT_PORT),
                (target.unwrap(), input_port),
            );
        }
    }
    document
}

fn reroute_locale_independent_contract(projection: &EditorGraphProjectionDto) -> serde_json::Value {
    json!({
        "nodes": projection.nodes.iter().map(|node| json!({
            "nodeId": node.node_id,
            "nodeTypeId": node.node_type_id,
            "position": node.position,
            "styleId": node.display.style_id,
            "ports": node.ports.iter().map(|port| json!({
                "address": port.address,
                "templateKey": port.template_key,
                "direction": port.direction,
                "kind": port.kind,
                "resolvedType": port.resolved_type,
            })).collect::<Vec<_>>(),
            "parameterEditors": node.parameter_editors,
            "capabilities": node.capabilities,
        })).collect::<Vec<_>>(),
        "connections": projection.connections,
    })
}

#[test]
fn phase2_reroute_projection_preserves_all_kinds_after_semantic_collapse_across_locales() {
    let builtin = build_builtin_node_system().unwrap();
    let document = reroute_projection_document(true);
    let result = GraphCompiler::new(builtin.registry.as_ref(), &EmptyResources).compile(&document);

    assert_eq!(
        result.outcome,
        CompilationOutcome::Succeeded,
        "{:?}",
        result.analysis.diagnostics
    );
    let semantic = result.semantic.as_ref().unwrap();
    assert_eq!(
        semantic
            .nodes
            .iter()
            .filter(|node| {
                builtin
                    .registry
                    .get(&node.node_type_id)
                    .is_some_and(|registered| {
                        registered.transparent_role() == Some(TransparentNodeRole::Reroute)
                    })
            })
            .count(),
        0
    );
    assert_eq!(semantic.dependencies.len(), 3);
    for (index, expected_kind) in [PortKindDto::Data, PortKindDto::Control, PortKindDto::Effect]
        .into_iter()
        .enumerate()
    {
        let source = NodeId::from_uuid(Uuid::from_u128(10 + index as u128 * 2));
        let target = NodeId::from_uuid(Uuid::from_u128(11 + index as u128 * 2));
        assert!(
            semantic
                .dependencies
                .iter()
                .any(|dependency| match dependency {
                    SemanticDependency::Value(edge) => {
                        expected_kind == PortKindDto::Data
                            && edge.source.node_id == source
                            && edge.target.node_id == target
                    }
                    SemanticDependency::Control(edge) => {
                        expected_kind == PortKindDto::Control
                            && edge.source_node == source
                            && edge.target_node == target
                    }
                    SemanticDependency::Effect(edge) => {
                        expected_kind == PortKindDto::Effect
                            && edge.predecessor == source
                            && edge.successor == target
                    }
                })
        );
    }

    let en = build_editor_graph_projection(
        "functions/reroute-projection",
        &document,
        &result.analysis,
        &result.outcome,
        builtin.registry.as_ref(),
        &builtin.catalog.localization("en-US"),
    )
    .unwrap();
    let zh = build_editor_graph_projection(
        "functions/reroute-projection",
        &document,
        &result.analysis,
        &result.outcome,
        builtin.registry.as_ref(),
        &builtin.catalog.localization("zh-CN"),
    )
    .unwrap();

    assert_eq!(en.nodes.len(), 12);
    assert_eq!(en.connections.len(), 9);
    assert_eq!(
        en.nodes
            .iter()
            .filter(|node| node.display.style_id.as_deref() == Some("builtin.reroute"))
            .count(),
        6
    );
    let first_reroute = en
        .nodes
        .iter()
        .position(|node| node.display.style_id.as_deref() == Some("builtin.reroute"))
        .unwrap();
    assert_ne!(
        en.nodes[first_reroute].display.title,
        zh.nodes[first_reroute].display.title
    );
    assert_eq!(
        reroute_locale_independent_contract(&en),
        reroute_locale_independent_contract(&zh)
    );

    for (index, kind) in [PortKindDto::Data, PortKindDto::Control, PortKindDto::Effect]
        .into_iter()
        .enumerate()
    {
        let source_id = NodeId::from_uuid(Uuid::from_u128(100 + index as u128 * 2));
        let target_id = NodeId::from_uuid(Uuid::from_u128(101 + index as u128 * 2));
        for (node_id, x) in [(source_id, 40.0), (target_id, 140.0)] {
            let node = en
                .nodes
                .iter()
                .find(|node| node.node_id.as_ref() == node_id.to_string())
                .unwrap();
            assert_eq!(
                node.position,
                NodePositionDto {
                    x,
                    y: 80.0 + index as f64 * 60.0,
                }
            );
            assert_eq!(node.display.style_id.as_deref(), Some("builtin.reroute"));
            assert!(node.parameter_editors.is_empty());
            assert!(!node.capabilities.managed);
            assert!(node.capabilities.can_delete);
            assert_eq!(node.ports.len(), 2);
            assert_eq!(node.ports[0].direction, PortDirectionDto::Input);
            assert_eq!(node.ports[1].direction, PortDirectionDto::Output);
            assert!(node.ports.iter().all(|port| port.kind == kind));
            if kind == PortKindDto::Data {
                assert_eq!(node.ports[0].resolved_type, node.ports[1].resolved_type);
            }
        }
        let connection_id = ConnectionId::from_uuid(Uuid::from_u128(301 + index as u128 * 3));
        let connection = en
            .connections
            .iter()
            .find(|connection| connection.connection_id.as_ref() == connection_id.to_string())
            .unwrap();
        assert_eq!(connection.order, None);
        assert_eq!(
            connection.output,
            PortAddressDto::Declared {
                node_id: source_id.to_string().into(),
                port_key: REROUTE_OUTPUT_PORT.into(),
            }
        );
        assert_eq!(
            connection.input,
            PortAddressDto::Declared {
                node_id: target_id.to_string().into(),
                port_key: REROUTE_INPUT_PORT.into(),
            }
        );
    }
}

struct NamedResources {
    function_path: GraphResourcePath,
    function: FunctionDocument,
    function_graph: GraphDocument,
    function_name: Box<str>,
    variable: crate::variable::VariableInstance,
    database_name: Box<str>,
    database_columns: Vec<crate::schema::ColumnInfoDTO>,
}

impl ResourceSnapshot for NamedResources {
    fn versions(&self) -> ResourceVersionSet {
        BTreeMap::from([
            (
                crate::node_system::analysis::ResourceKey::new(self.function_path.0.clone()),
                crate::node_system::analysis::ResourceVersion::new("function-v1"),
            ),
            (
                crate::node_system::analysis::ResourceKey::new(format!(
                    "variables/{}",
                    self.variable.id
                )),
                crate::node_system::analysis::ResourceVersion::new("variable-v1"),
            ),
            (
                crate::node_system::analysis::ResourceKey::new("databases/sales"),
                crate::node_system::analysis::ResourceVersion::new("database-v1"),
            ),
        ])
    }

    fn function_name(&self, path: &GraphResourcePath) -> Option<&str> {
        (path == &self.function_path).then_some(self.function_name.as_ref())
    }

    fn function_document(&self, path: &GraphResourcePath) -> Option<&FunctionDocument> {
        (path == &self.function_path).then_some(&self.function)
    }

    fn function_graph_document(&self, path: &GraphResourcePath) -> Option<&GraphDocument> {
        (path == &self.function_path).then_some(&self.function_graph)
    }

    fn variable(
        &self,
        id: &crate::variable::VariableId,
    ) -> Option<&crate::variable::VariableInstance> {
        (id == &self.variable.id).then_some(&self.variable)
    }

    fn database_name(&self, id: &str) -> Option<&str> {
        (id == "sales").then_some(self.database_name.as_ref())
    }

    fn database_schema(&self, id: &str) -> Option<&[crate::schema::ColumnInfoDTO]> {
        (id == "sales").then_some(self.database_columns.as_slice())
    }
}

#[test]
fn resource_bound_editor_titles_use_authoritative_names_and_preserve_labels() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let variable_id = crate::variable::VariableId::from(Uuid::from_u128(100));
    let function_path = GraphResourcePath("functions/calculate-sales".into());
    let resources = NamedResources {
        function_path: function_path.clone(),
        function: FunctionDocument::new(FunctionSignature::default()),
        function_graph: GraphDocument::default(),
        function_name: "Calculate Sales".into(),
        variable: crate::variable::VariableInstance {
            id: variable_id,
            name: "Revenue".into(),
            data_type: DataType::Int64,
            data_value: crate::graph::value::DataValue::Int64(1),
            tabular: None,
            description: String::new(),
            scope: crate::variable::VariableScope::Global,
            tags: Vec::new(),
        },
        database_name: "Sales Database".into(),
        database_columns: Vec::new(),
    };
    let variable_path = format!("variables/{variable_id}");
    let mut document = GraphDocument::default();
    for (index, node_type, parameter, resource, user_label) in [
        (
            1,
            "yssbi.project.function.call",
            "target",
            function_path.0.as_ref(),
            None,
        ),
        (
            2,
            "yssbi.project.variable.get",
            "variable",
            variable_path.as_str(),
            Some("Previous period"),
        ),
        (
            3,
            "yssbi.dataframe.source.get",
            "dataframe",
            "databases/sales",
            None,
        ),
        (
            4,
            "yssbi.dataframe.source.get",
            "dataframe",
            "databases/missing",
            None,
        ),
    ] {
        let node_id = NodeId::from_uuid(Uuid::from_u128(index));
        document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: NodeTypeId::new(node_type).unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: BTreeMap::from([(
                    ParameterKey::new(parameter).unwrap(),
                    json!(resource),
                )]),
                user_label: user_label.map(str::to_owned),
            },
        );
    }

    let analysis = GraphCompiler::new(registry.as_ref(), &resources)
        .compile(&document)
        .analysis;
    let projection = EditorGraphProjectionDto::from_sources(
        "events/resource-titles",
        &analysis,
        &document,
        registry.as_ref(),
        &catalog.localization("en-US"),
    )
    .unwrap();
    let titles = projection
        .nodes
        .iter()
        .filter(|node| node.node_id.as_ref() != Uuid::from_u128(4).to_string())
        .map(|node| (node.node_type_id.as_ref(), node.display.title.as_ref()))
        .collect::<BTreeMap<_, _>>();

    assert_eq!(titles["yssbi.project.function.call"], "Calculate Sales");
    assert_eq!(titles["yssbi.project.variable.get"], "Revenue");
    assert_eq!(titles["yssbi.dataframe.source.get"], "Sales Database");
    let missing = projection
        .nodes
        .iter()
        .find(|node| node.node_id.as_ref() == Uuid::from_u128(4).to_string())
        .unwrap();
    assert_eq!(missing.display.title.as_ref(), "Get DataFrame");
    assert!(
        missing.diagnostics.iter().any(|diagnostic| {
            diagnostic.code.as_ref() == "compiler.resource.resolution_failed"
        })
    );
    assert_eq!(
        projection
            .nodes
            .iter()
            .find(|node| node.node_type_id.as_ref() == "yssbi.project.variable.get")
            .unwrap()
            .display
            .user_label
            .as_deref(),
        Some("Previous period"),
    );
}

fn basis(revision: u64) -> ProjectionBasis {
    ProjectionBasis {
        graph_path: "functions/main".into(),
        graph_revision: revision,
        registry_fingerprint: RegistryFingerprint::from_bytes([7; 32]),
        resource_versions: BTreeMap::new(),
    }
}

fn capabilities() -> NodeCapabilitiesDto {
    NodeCapabilitiesDto {
        managed: false,
        can_copy: true,
        can_delete: true,
        can_edit_label: true,
        can_edit_parameters: false,
        has_dynamic_ports: true,
        supports_inline_literals: false,
    }
}

fn port(key: &str) -> ResolvedPortDto {
    ResolvedPortDto {
        address: PortAddressDto::Declared {
            node_id: "node-1".into(),
            port_key: key.into(),
        },
        template_key: key.into(),
        display: PortDisplayDto {
            label: key.into(),
            instance_label: None,
        },
        direction: PortDirectionDto::Input,
        kind: PortKindDto::Data,
        instance_kind: PortInstanceKindDto::Declared,
        orphan: false,
        can_remove: false,
        connections: PortConnectionCapabilityDto {
            current: 0,
            maximum: Some(1),
            ordered: false,
            can_append: true,
            can_replace: false,
            can_move: false,
        },
        input: Some(EditorInputBindingDto {
            literal_override: None,
            protocol_default: None,
            effective: EffectiveInputBindingKindDto::Unbound,
        }),
        resolved_type: Some(TypeSummaryDto {
            display: "core.string".into(),
            resolved: true,
            data_type: Some(DataType::String),
            internal_type_expr: Some(TypeExpr::Concrete(TypeId::new("core.string").unwrap())),
        }),
        resolved_schema: None,
        status: ResolvedPortStatusDto::Resolved,
    }
}

fn node(revision: u64, ports: Vec<ResolvedPortDto>) -> EditorNodeProjectionDto {
    EditorNodeProjectionDto {
        graph_path: "functions/main".into(),
        source_revision: revision,
        node_id: "node-1".into(),
        node_type_id: "test.node".into(),
        position: NodePositionDto { x: 0.0, y: 0.0 },
        display: NodeDisplayDto {
            title: "Test".into(),
            description: None,
            user_label: None,
            icon_id: None,
            style_id: None,
        },
        ports,
        parameter_editors: Vec::new(),
        capabilities: capabilities(),
        diagnostics: Vec::new(),
    }
}

fn projection(revision: u64, ports: Vec<ResolvedPortDto>) -> EditorGraphProjectionDto {
    EditorGraphProjectionDto {
        basis: basis(revision),
        graph_path: "functions/main".into(),
        source_revision: revision,
        nodes: vec![node(revision, ports)],
        connections: Vec::new(),
        diagnostics: Vec::new(),
        outcome: CompilationOutcomeDto::Success,
        has_blocking_diagnostics: false,
    }
}

#[test]
fn projection_basis_serializes_registry_fingerprint_as_lowercase_sha256_hex() {
    let value = serde_json::to_value(basis(7)).unwrap();
    assert_eq!(
        value["registryFingerprint"],
        "0707070707070707070707070707070707070707070707070707070707070707"
    );
    assert_eq!(
        serde_json::from_value::<ProjectionBasis>(value).unwrap(),
        basis(7)
    );
}

#[test]
fn projection_basis_rejects_malformed_registry_fingerprint_wire_values() {
    let valid = serde_json::to_value(basis(7)).unwrap();
    for malformed in [
        serde_json::json!("070707070707070707070707070707070707070707070707070707070707070A"),
        serde_json::json!("070707070707070707070707070707070707070707070707070707070707070"),
        serde_json::json!("07070707070707070707070707070707070707070707070707070707070707070"),
        serde_json::json!("070707070707070707070707070707070707070707070707070707070707070g"),
    ] {
        let mut value = valid.clone();
        value["registryFingerprint"] = malformed;
        assert!(serde_json::from_value::<ProjectionBasis>(value).is_err());
    }
}

#[test]
fn schema_aware_editors_are_unavailable_without_source_schema() {
    for (node_type, value, expected_kind) in [
        (
            "yssbi.dataframe.project",
            Some(json!(["amount"])),
            "projectColumns",
        ),
        (
            "yssbi.dataframe.filter.rows",
            Some(json!({
                "column": "amount",
                "operator": "greaterThan",
                "value": { "type": "decimal", "value": "10.5" }
            })),
            "filterPredicate",
        ),
    ] {
        let editor = project_schema_aware_editor(
            node_type,
            value.as_ref(),
            None,
            "Connect DataFrame input".into(),
        )
        .expect("schema-aware editor");
        let serialized = serde_json::to_value(editor).unwrap();
        assert_eq!(serialized["kind"], expected_kind);
        assert_eq!(serialized["available"], false);
        assert_eq!(serialized["unavailableReason"], "Connect DataFrame input");
        let options = serialized
            .get("options")
            .or_else(|| serialized.get("columns"))
            .unwrap();
        assert_eq!(options, &json!([]));
    }
}

#[test]
fn schema_aware_editors_project_typed_options_and_operator_matrix() {
    use crate::node_system::protocol::{
        RelationalScalarType, ResolvedSchemaFact, SchemaColumnRef, SchemaField,
    };

    let fact = ResolvedSchemaFact::new(
        SchemaExpr::Input(PortKey::new("source").unwrap()),
        [
            SchemaField {
                name: SchemaColumnRef("active".into()),
                scalar_type: RelationalScalarType::Boolean,
                lineage: None,
            },
            SchemaField {
                name: SchemaColumnRef("count".into()),
                scalar_type: RelationalScalarType::Int64,
                lineage: None,
            },
            SchemaField {
                name: SchemaColumnRef("amount".into()),
                scalar_type: RelationalScalarType::Float64,
                lineage: None,
            },
            SchemaField {
                name: SchemaColumnRef("status".into()),
                scalar_type: RelationalScalarType::String,
                lineage: None,
            },
            SchemaField {
                name: SchemaColumnRef("day".into()),
                scalar_type: RelationalScalarType::Date,
                lineage: None,
            },
            SchemaField {
                name: SchemaColumnRef("created".into()),
                scalar_type: RelationalScalarType::DateTime,
                lineage: None,
            },
            SchemaField {
                name: SchemaColumnRef("opaque".into()),
                scalar_type: RelationalScalarType::Unknown,
                lineage: None,
            },
        ],
    );
    let project = project_schema_aware_editor(
        "yssbi.dataframe.project",
        Some(&json!(["status", "count"])),
        Some(&fact),
        "unused".into(),
    )
    .unwrap();
    let project = serde_json::to_value(project).unwrap();
    assert_eq!(project["value"], json!(["status", "count"]));
    assert_eq!(
        project["options"][0],
        json!({ "name": "active", "dataType": "boolean" })
    );
    assert_eq!(
        project["options"][6],
        json!({ "name": "opaque", "dataType": "unknown" })
    );

    let predicate = json!({
        "column": "count",
        "operator": "greaterThan",
        "value": { "type": "integer", "value": "9007199254740993" }
    });
    let filter = project_schema_aware_editor(
        "yssbi.dataframe.filter.rows",
        Some(&predicate),
        Some(&fact),
        "unused".into(),
    )
    .unwrap();
    let filter = serde_json::to_value(filter).unwrap();
    assert_eq!(filter["value"], predicate);
    assert_eq!(
        filter["columns"][0]["operators"],
        json!(["equal", "notEqual", "isNull", "isNotNull"])
    );
    assert_eq!(
        filter["columns"][1]["operators"],
        json!([
            "equal",
            "notEqual",
            "lessThan",
            "lessThanOrEqual",
            "greaterThan",
            "greaterThanOrEqual",
            "isNull",
            "isNotNull"
        ])
    );
    assert_eq!(
        filter["columns"][2]["operators"],
        filter["columns"][1]["operators"]
    );
    assert_eq!(
        filter["columns"][3]["operators"],
        filter["columns"][1]["operators"]
    );
    assert_eq!(
        filter["columns"][4]["operators"],
        json!(["isNull", "isNotNull"])
    );
    assert_eq!(
        filter["columns"][5]["operators"],
        json!(["isNull", "isNotNull"])
    );
    assert_eq!(filter["columns"][6]["operators"], json!([]));
    assert_eq!(filter["columns"][0]["literalTypes"], json!(["boolean"]));
    assert_eq!(filter["columns"][1]["literalTypes"], json!(["integer"]));
    assert_eq!(
        filter["columns"][2]["literalTypes"],
        json!(["integer", "decimal"])
    );
    assert_eq!(filter["columns"][3]["literalTypes"], json!(["string"]));
    assert_eq!(filter["columns"][4]["literalTypes"], json!([]));
    assert_eq!(filter["columns"][6]["literalTypes"], json!([]));
}

#[test]
fn diagnostic_locations_serialize_struct_fields_as_camel_case() {
    let locations = vec![
        DiagnosticLocationDto::Node {
            node_id: "node-1".into(),
        },
        DiagnosticLocationDto::Port {
            address: PortAddressDto::Declared {
                node_id: "node-1".into(),
                port_key: "input".into(),
            },
        },
        DiagnosticLocationDto::Connection {
            connection_id: "connection-1".into(),
        },
        DiagnosticLocationDto::Parameter {
            node_id: "node-1".into(),
            key: "formula".into(),
        },
    ];

    assert_eq!(
        serde_json::to_value(locations).unwrap(),
        json!([
            { "kind": "node", "nodeId": "node-1" },
            {
                "kind": "port",
                "address": {
                    "kind": "declared",
                    "nodeId": "node-1",
                    "portKey": "input"
                }
            },
            { "kind": "connection", "connectionId": "connection-1" },
            { "kind": "parameter", "nodeId": "node-1", "key": "formula" }
        ])
    );
}

#[test]
fn editor_projection_includes_positions_connections_and_input_bindings() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let branch_id = NodeId::from_uuid(Uuid::from_u128(1));
    let sleep_id = NodeId::from_uuid(Uuid::from_u128(2));
    let connection_id = ConnectionId::from_uuid(Uuid::from_u128(3));
    let branch_enter = PortAddress::declared(branch_id, PortKey::new("enter").unwrap());
    let branch_condition = PortAddress::declared(branch_id, PortKey::new("condition").unwrap());
    let branch_true = PortAddress::declared(branch_id, PortKey::new("true").unwrap());
    let sleep_enter = PortAddress::declared(sleep_id, PortKey::new("enter").unwrap());
    let mut document = GraphDocument::default();
    document.nodes.insert(
        branch_id,
        DocumentNode {
            id: branch_id,
            node_type: NodeTypeId::new("yssbi.control.branch").unwrap(),
            position: NodePosition { x: 12.5, y: -4.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        },
    );
    document.nodes.insert(
        sleep_id,
        DocumentNode {
            id: sleep_id,
            node_type: NodeTypeId::new("yssbi.control.sleep").unwrap(),
            position: NodePosition { x: 48.0, y: 8.0 },
            parameters: BTreeMap::new(),
            user_label: None,
        },
    );
    document.connections.insert(
        connection_id,
        DocumentConnection {
            id: connection_id,
            output: branch_true,
            input: sleep_enter,
            order: Some(OrderKey("rank-1".into())),
        },
    );
    document.input_states.insert(
        branch_condition,
        InputState {
            literal_override: Some(json!(true)),
        },
    );
    let analysis = GraphCompiler::new(registry.as_ref(), &EmptyResources)
        .compile(&document)
        .analysis;
    let localization = catalog.localization("en-US");

    let projection = EditorGraphProjectionDto::from_sources(
        "functions/main",
        &analysis,
        &document,
        &registry,
        &localization,
    )
    .unwrap();

    assert_eq!(
        projection.nodes[0].position,
        NodePositionDto { x: 12.5, y: -4.0 }
    );
    assert_eq!(
        projection.connections[0].connection_id.as_ref(),
        connection_id.to_string()
    );
    assert!(matches!(
        projection.connections[0].output,
        PortAddressDto::Declared { .. }
    ));
    assert_eq!(projection.connections[0].order.as_deref(), Some("rank-1"));
    let branch = &projection.nodes[0];
    let sleep = &projection.nodes[1];
    assert!(
        branch
            .ports
            .iter()
            .find(|port| port.template_key.as_ref() == "true")
            .unwrap()
            .input
            .is_none()
    );
    fn binding<'a>(node: &'a EditorNodeProjectionDto, key: &str) -> &'a EditorInputBindingDto {
        node.ports
            .iter()
            .find(|port| port.template_key.as_ref() == key)
            .unwrap()
            .input
            .as_ref()
            .unwrap()
    }
    let effective = |node: &EditorNodeProjectionDto, key: &str| binding(node, key).effective;
    assert_eq!(
        effective(branch, "condition"),
        EffectiveInputBindingKindDto::Literal
    );
    assert_eq!(
        effective(sleep, "enter"),
        EffectiveInputBindingKindDto::Connections
    );
    assert_eq!(
        effective(sleep, "duration"),
        EffectiveInputBindingKindDto::ProtocolDefault
    );
    assert_eq!(
        effective(branch, "enter"),
        EffectiveInputBindingKindDto::Unbound
    );
    assert_eq!(
        binding(branch, "condition").literal_override,
        Some(json!(true))
    );
    assert_eq!(
        binding(sleep, "duration").protocol_default,
        Some(json!({ "Decimal": "1" }))
    );

    let zh_projection = EditorGraphProjectionDto::from_sources(
        "functions/main",
        &analysis,
        &document,
        &registry,
        &catalog.localization("zh-CN"),
    )
    .unwrap();
    assert_eq!(zh_projection.basis, projection.basis);
    assert_eq!(zh_projection.graph_path, projection.graph_path);
    assert_eq!(zh_projection.source_revision, projection.source_revision);
    assert_eq!(zh_projection.connections, projection.connections);
    assert_eq!(zh_projection.nodes.len(), projection.nodes.len());
    for (localized, original) in zh_projection.nodes.iter().zip(&projection.nodes) {
        assert_eq!(localized.node_id, original.node_id);
        assert_eq!(localized.node_type_id, original.node_type_id);
        assert_eq!(localized.position, original.position);
        assert_eq!(
            localized
                .ports
                .iter()
                .map(|port| &port.address)
                .collect::<Vec<_>>(),
            original
                .ports
                .iter()
                .map(|port| &port.address)
                .collect::<Vec<_>>()
        );
    }

    let old_input = projection.connections[0].input.clone();
    let mut current = projection.clone();
    let mut next = projection;
    next.basis.graph_revision += 1;
    next.source_revision += 1;
    for node in &mut next.nodes {
        node.source_revision += 1;
    }
    next.connections[0].input = project_address(&branch_enter);
    let delta = GraphProjectionDelta::between(&current, &next).unwrap();
    current.apply_delta(delta).unwrap();

    assert_ne!(current.connections[0].input, old_input);
    assert_eq!(current.connections, next.connections);
}

#[test]
fn grouped_port_removal_capability_distinguishes_complete_and_partial_members() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let loop_id = NodeId::from_uuid(Uuid::from_u128(20));
    let complete_id = PortInstanceId::from_uuid(Uuid::from_u128(21));
    let partial_id = PortInstanceId::from_uuid(Uuid::from_u128(22));
    let mut parameters = BTreeMap::new();
    parameters.insert(ParameterKey::new("max_iterations").unwrap(), json!(100));
    let mut document = GraphDocument::default();
    document
        .create_node(DocumentNode {
            id: loop_id,
            node_type: NodeTypeId::new("yssbi.control.loop").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters,
            user_label: None,
        })
        .unwrap();
    for (template, instance_id) in [
        ("initial_source", complete_id),
        ("body_input", complete_id),
        ("next_source", complete_id),
        ("result", complete_id),
        ("initial_source", partial_id),
    ] {
        document
            .bind_port(
                PortAddress::instance(loop_id, PortKey::new(template).unwrap(), instance_id),
                DynamicPortBinding::UserCreated {
                    order: OrderKey(instance_id.to_string().into()),
                },
            )
            .unwrap();
    }
    let analysis = GraphCompiler::new(registry.as_ref(), &EmptyResources)
        .compile(&document)
        .analysis;
    let projection = EditorGraphProjectionDto::from_sources(
        "events/grouped-capability",
        &analysis,
        &document,
        &registry,
        &catalog.localization("en-US"),
    )
    .unwrap();
    let loop_node = projection
        .nodes
        .iter()
        .find(|node| node.node_id.as_ref() == loop_id.to_string())
        .unwrap();

    for port in &loop_node.ports {
        let PortAddressDto::Instance { instance_id, .. } = &port.address else {
            continue;
        };
        if instance_id.as_ref() == complete_id.to_string() {
            assert!(!port.can_remove, "complete member must preserve Loop min=1");
        } else if instance_id.as_ref() == partial_id.to_string() {
            assert!(port.can_remove, "partial endpoints must remain removable");
        }
    }
}

#[test]
fn grouped_port_capability_ignores_non_user_created_siblings() {
    let builtin = build_builtin_node_system().unwrap();
    let registry = builtin.registry;
    let catalog = builtin.catalog;
    let loop_id = NodeId::from_uuid(Uuid::from_u128(30));
    let complete_id = PortInstanceId::from_uuid(Uuid::from_u128(31));
    let mixed_id = PortInstanceId::from_uuid(Uuid::from_u128(32));
    let mut parameters = BTreeMap::new();
    parameters.insert(ParameterKey::new("max_iterations").unwrap(), json!(100));
    let mut document = GraphDocument::default();
    document
        .create_node(DocumentNode {
            id: loop_id,
            node_type: NodeTypeId::new("yssbi.control.loop").unwrap(),
            position: NodePosition { x: 0.0, y: 0.0 },
            parameters,
            user_label: None,
        })
        .unwrap();
    for template in ["initial_source", "body_input", "next_source", "result"] {
        document
            .bind_port(
                PortAddress::instance(loop_id, PortKey::new(template).unwrap(), complete_id),
                DynamicPortBinding::UserCreated {
                    order: OrderKey("complete".into()),
                },
            )
            .unwrap();
    }
    let locator = || DynamicMemberLocator::FunctionParameter {
        function: GraphResourcePath("functions/mixed".into()),
        parameter: FunctionParameterId("value".into()),
    };
    for (template, binding) in [
        (
            "initial_source",
            DynamicPortBinding::UserCreated {
                order: OrderKey("partial".into()),
            },
        ),
        (
            "body_input",
            DynamicPortBinding::Resolved {
                origin: locator(),
                order: OrderKey("resolved-body".into()),
                last_known: LastKnownPortMetadata::default(),
            },
        ),
        (
            "next_source",
            DynamicPortBinding::Orphan {
                origin: locator(),
                order: OrderKey("orphan-next".into()),
                last_known: LastKnownPortMetadata {
                    label: "Next".into(),
                    value_type: None,
                },
            },
        ),
        (
            "result",
            DynamicPortBinding::Resolved {
                origin: locator(),
                order: OrderKey("resolved-result".into()),
                last_known: LastKnownPortMetadata::default(),
            },
        ),
    ] {
        document
            .bind_port(
                PortAddress::instance(loop_id, PortKey::new(template).unwrap(), mixed_id),
                binding,
            )
            .unwrap();
    }

    let analysis = GraphCompiler::new(registry.as_ref(), &EmptyResources)
        .compile(&document)
        .analysis;
    let projection = EditorGraphProjectionDto::from_sources(
        "events/mixed-binding-capability",
        &analysis,
        &document,
        &registry,
        &catalog.localization("en-US"),
    )
    .unwrap();
    let loop_node = projection
        .nodes
        .iter()
        .find(|node| node.node_id.as_ref() == loop_id.to_string())
        .unwrap();
    let mut saw_partial_user_created = false;
    let mut saw_orphan = false;

    for port in &loop_node.ports {
        let PortAddressDto::Instance {
            template_key,
            instance_id,
            ..
        } = &port.address
        else {
            continue;
        };
        if instance_id.as_ref() == complete_id.to_string() {
            assert!(
                !port.can_remove,
                "non-user siblings must not inflate complete_count"
            );
        } else if instance_id.as_ref() == mixed_id.to_string() {
            if template_key.as_ref() == "initial_source" {
                saw_partial_user_created = true;
                assert!(!port.orphan);
                assert!(
                    port.can_remove,
                    "partial UserCreated endpoint must be removable"
                );
            } else {
                saw_orphan = true;
                assert!(port.orphan);
                assert!(port.can_remove, "orphan endpoints keep the UI removal rule");
            }
        }
    }
    assert!(saw_partial_user_created);
    assert!(saw_orphan);
}

#[test]
fn projection_basis_is_consistent_with_envelope() {
    let projection = projection(4, vec![port("value")]);

    assert_eq!(projection.basis.graph_path, projection.graph_path);
    assert_eq!(projection.basis.graph_revision, projection.source_revision);
}

#[test]
fn stale_basis_is_rejected_without_mutation() {
    let mut current = projection(2, vec![port("old")]);
    let original = current.clone();
    let stale = projection(1, vec![port("stale")]);
    let next = projection(3, vec![port("new")]);
    let delta = GraphProjectionDelta::between(&stale, &next).unwrap();

    assert_eq!(
        current.apply_delta(delta).unwrap_err(),
        ProjectionError::StaleProjectionBasis
    );
    assert_eq!(current, original);
}

#[test]
fn dynamic_interface_is_replaced_atomically_as_a_whole_node() {
    let mut current = projection(5, vec![port("a"), port("b")]);
    let next = projection(6, vec![port("c")]);
    let delta = GraphProjectionDelta::between(&current, &next).unwrap();

    assert_eq!(delta.node_replacements, next.nodes);
    assert_eq!(delta.node_replacements[0].ports, vec![port("c")]);
    let serialized = serde_json::to_value(&delta).unwrap();
    assert!(serialized.get("addedPins").is_none());

    current.apply_delta(delta).unwrap();
    assert_eq!(current, next);
}

#[test]
fn editor_schema_summary_projects_transformed_typed_fields() {
    let fact = crate::node_system::protocol::ResolvedSchemaFact::new(
        SchemaExpr::Rename {
            input: Box::new(SchemaExpr::Input(PortKey::new("source").unwrap())),
            mapping: crate::node_system::protocol::RenameExpr::Explicit(vec![]),
        },
        [crate::node_system::protocol::SchemaField {
            name: crate::node_system::protocol::SchemaColumnRef("total".into()),
            scalar_type: crate::node_system::protocol::RelationalScalarType::Float64,
            lineage: None,
        }],
    );

    let summary = project_schema_summary(&fact.expression, Some(&fact));

    assert_eq!(
        summary.fields,
        vec![SchemaFieldDto {
            name: "total".into(),
            scalar_type: RelationalScalarTypeDto::Float64,
        }]
    );
}

#[test]
fn editor_dto_does_not_serialize_protocol_ast() {
    let json = serde_json::to_string(&projection(1, vec![port("value")])).unwrap();

    for forbidden in [
        "execution",
        "interface",
        "typeConstraints",
        "valueType",
        "managedRole",
        "protocolFingerprint",
    ] {
        assert!(
            !json.contains(forbidden),
            "unexpected protocol field: {forbidden}"
        );
    }
}
