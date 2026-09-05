use std::collections::BTreeMap;

use yss_graph_compiler_diagnostics::GraphDiagnosticKind;
use yss_graph_document::{GraphDocument, PortAddress};
use yss_graph_protocol::{PortDirection, TypeExpr, validate_typed_value};
use yss_graph_registry::NodeRegistry;

use crate::{GraphDiagnosticFact, GraphDiagnosticLocation, GraphNodeSemanticFact, graph_problem};

pub(crate) fn validate(
    document: &GraphDocument,
    registry: &NodeRegistry,
    nodes: &[GraphNodeSemanticFact],
) -> Vec<GraphDiagnosticFact> {
    let ports = nodes
        .iter()
        .flat_map(|node| node.ports.iter())
        .map(|port| (&port.address, port))
        .collect::<BTreeMap<_, _>>();
    let mut diagnostics = Vec::new();
    for connection in document.connections.values() {
        for (address, direction, kind) in [
            (
                &connection.output,
                PortDirection::Output,
                GraphDiagnosticKind::ConnectionOutputDirection,
            ),
            (
                &connection.input,
                PortDirection::Input,
                GraphDiagnosticKind::ConnectionInputDirection,
            ),
        ] {
            let issue = match ports.get(address) {
                None => Some(GraphDiagnosticKind::PortUnknown),
                Some(port) if port.direction != direction => Some(kind),
                _ => None,
            };
            if let Some(kind) = issue {
                diagnostics.push(graph_problem(
                    kind,
                    GraphDiagnosticLocation::Connection(connection.id),
                    [("port", address.to_string().into())],
                ));
            }
        }
        if let Some(input) = ports.get(&connection.input) {
            if input.orphan {
                continue;
            }
            let kind = match (input.connections.ordered, connection.order.is_some()) {
                (true, false) => Some(GraphDiagnosticKind::ConnectionOrderRequired),
                (false, true) => Some(GraphDiagnosticKind::ConnectionOrderForbidden),
                _ => None,
            };
            if let Some(kind) = kind {
                diagnostics.push(graph_problem(
                    kind,
                    GraphDiagnosticLocation::Connection(connection.id),
                    [("port", connection.input.to_string().into())],
                ));
            }
        }
    }
    for port in ports.values() {
        if port
            .connections
            .maximum
            .is_some_and(|maximum| port.connections.current > maximum)
        {
            diagnostics.push(port_problem(
                GraphDiagnosticKind::ConnectionLimit,
                &port.address,
            ));
        }
    }
    for (address, input) in &document.input_states {
        let Some(literal) = &input.literal_override else {
            continue;
        };
        let Some(port) = ports.get(address) else {
            diagnostics.push(port_problem(GraphDiagnosticKind::InputUnknownPort, address));
            continue;
        };
        if port.orphan {
            continue;
        }
        if port.direction != PortDirection::Input {
            diagnostics.push(port_problem(GraphDiagnosticKind::InputNotInput, address));
        }
        if !port.literal_allowed {
            diagnostics.push(port_problem(
                GraphDiagnosticKind::InputLiteralForbidden,
                address,
            ));
        }
        if document
            .connections
            .values()
            .any(|connection| &connection.input == address)
        {
            diagnostics.push(port_problem(
                GraphDiagnosticKind::InputConflictingBindings,
                address,
            ));
        }
        if validate_typed_value(literal.clone(), &port.accepted_type, registry).is_err() {
            diagnostics.push(port_problem(
                GraphDiagnosticKind::InputLiteralInvalid,
                address,
            ));
        }
    }
    for node in nodes {
        let Some(document_node) = document.nodes.get(&node.node_id) else {
            continue;
        };
        let Some(protocol) = registry.protocol(&document_node.node_type) else {
            continue;
        };
        let schema = node
            .ports
            .iter()
            .filter(|port| port.direction == PortDirection::Input)
            .find_map(|port| port.schema_state.exact());
        let Some(schema) = schema else {
            continue;
        };
        for parameter in &protocol.parameters.parameters {
            let Some(value) = document_node.parameters.get(&parameter.key) else {
                continue;
            };
            let TypeExpr::Concrete(type_id) = &parameter.value_type else {
                continue;
            };
            use yss_graph_protocol::dataframe::{
                FILTER_PREDICATE_TYPE_ID, PROJECT_COLUMNS_TYPE_ID, filter_comparison_is_compatible,
                prepare_filter_predicate_json, prepare_project_columns_json,
            };
            let valid = match type_id.as_str() {
                PROJECT_COLUMNS_TYPE_ID => {
                    prepare_project_columns_json(value).is_ok_and(|columns| {
                        columns
                            .as_slice()
                            .iter()
                            .all(|name| schema.fields.iter().any(|field| &field.name.0 == name))
                    })
                }
                FILTER_PREDICATE_TYPE_ID => {
                    prepare_filter_predicate_json(value).is_ok_and(|predicate| {
                        schema
                            .fields
                            .iter()
                            .find(|field| field.name.0 == predicate.column)
                            .is_some_and(|field| {
                                filter_comparison_is_compatible(
                                    field.scalar_type,
                                    predicate.operator,
                                    predicate.value.as_ref(),
                                )
                            })
                    })
                }
                _ => true,
            };
            if !valid {
                diagnostics.push(graph_problem(
                    GraphDiagnosticKind::SchemaParameterInvalid,
                    GraphDiagnosticLocation::Parameter {
                        node_id: node.node_id,
                        key: parameter.key.clone(),
                    },
                    [("parameter_key", parameter.key.as_str().into())],
                ));
            }
        }
    }
    for diagnostic in &mut diagnostics {
        if let GraphDiagnosticLocation::Connection(id) = diagnostic.primary
            && let Some(connection) = document.connections.get(&id)
        {
            diagnostic.related = Box::new([
                GraphDiagnosticLocation::Port(connection.output.clone()),
                GraphDiagnosticLocation::Port(connection.input.clone()),
            ]);
        }
    }
    diagnostics
}

fn port_problem(kind: GraphDiagnosticKind, address: &PortAddress) -> GraphDiagnosticFact {
    graph_problem(
        kind,
        GraphDiagnosticLocation::Port(address.clone()),
        [("port", address.to_string().into())],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_graph_document::{
        ConnectionId, DocumentConnection, DocumentNode, InputState, NodeId, NodePosition, OrderKey,
        ParameterValues,
    };
    use yss_graph_protocol::{PortKey, TypeId, TypedValue, Value};
    use yss_graph_resource_contract::{
        ColumnSchema, DataSchema, GraphResourceId, ResourceCatalogFingerprint,
        ResourceCatalogSnapshot,
    };

    fn node(document: &mut GraphDocument, kind: &str) -> NodeId {
        let id = NodeId::new();
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::new(),
                user_label: None,
            },
        );
        id
    }

    fn port(node: NodeId, key: &str) -> PortAddress {
        PortAddress::declared(node, PortKey::new(key).unwrap())
    }

    fn connect(
        document: &mut GraphDocument,
        output: PortAddress,
        input: PortAddress,
        order: Option<OrderKey>,
    ) -> ConnectionId {
        let id = ConnectionId::new();
        document.connections.insert(
            id,
            DocumentConnection {
                id,
                output,
                input,
                order,
            },
        );
        id
    }

    fn resources(columns: Vec<ColumnSchema>) -> ResourceCatalogSnapshot {
        ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::from([(
                GraphResourceId::new("databases/sales"),
                DataSchema { columns },
            )]),
            ResourceCatalogFingerprint::from_bytes([0; 32]),
        )
    }

    #[test]
    fn imported_connection_and_literal_errors_are_canonical_and_locatable() {
        let registry = yss_graph_catalog::build_builtin_node_system()
            .unwrap()
            .registry;
        let mut document = GraphDocument::default();
        let source = node(&mut document, "yssbi.constant.int64");
        let consumer = node(&mut document, "yssbi.numeric.subtract");
        connect(
            &mut document,
            port(source, "value"),
            port(consumer, "left"),
            Some(OrderKey::new("a")),
        );
        connect(
            &mut document,
            port(source, "value"),
            port(consumer, "left"),
            None,
        );
        let reversed = connect(
            &mut document,
            port(consumer, "right"),
            port(source, "value"),
            None,
        );
        for address in [port(consumer, "left"), port(source, "value")] {
            document.input_states.insert(
                address,
                InputState {
                    literal_override: Some(TypedValue {
                        value_type: TypeExpr::Concrete(TypeId::new("core.string").unwrap()),
                        value: Value::String("invalid".into()),
                    }),
                },
            );
        }
        let snapshot = crate::resolve_graph_semantics(&document, &registry, &resources(vec![]));
        assert!(snapshot.ready().is_none());
        for kind in [
            GraphDiagnosticKind::ConnectionLimit,
            GraphDiagnosticKind::ConnectionOrderForbidden,
            GraphDiagnosticKind::InputConflictingBindings,
            GraphDiagnosticKind::InputLiteralInvalid,
            GraphDiagnosticKind::InputLiteralForbidden,
            GraphDiagnosticKind::InputNotInput,
        ] {
            assert!(
                snapshot
                    .diagnostics()
                    .iter()
                    .any(
                        |diagnostic| diagnostic.code.as_str() == kind.code() && diagnostic.blocking
                    ),
                "{kind:?}"
            );
        }
        for kind in [
            GraphDiagnosticKind::ConnectionOutputDirection,
            GraphDiagnosticKind::ConnectionInputDirection,
        ] {
            let diagnostic = snapshot
                .diagnostics()
                .iter()
                .find(|diagnostic| diagnostic.code.as_str() == kind.code())
                .unwrap();
            assert_eq!(
                diagnostic.primary,
                GraphDiagnosticLocation::Connection(reversed)
            );
            assert_eq!(diagnostic.related.len(), 2);
        }
    }

    #[test]
    fn nominal_parameters_are_revalidated_against_changed_input_schema() {
        let registry = yss_graph_catalog::build_builtin_node_system()
            .unwrap()
            .registry;
        for (kind, key, value) in [
            (
                "yssbi.dataframe.project",
                "columns",
                serde_json::json!(["amount"]),
            ),
            (
                "yssbi.dataframe.filter.rows",
                "predicate",
                serde_json::json!({"column":"amount", "operator":"greaterThan", "value":{"type":"integer", "value":"1"}}),
            ),
        ] {
            let mut document = GraphDocument::default();
            let source = node(&mut document, "yssbi.dataframe.source.get");
            let consumer = node(&mut document, kind);
            document.nodes.get_mut(&source).unwrap().parameters.insert(
                "dataframe".parse().unwrap(),
                serde_json::json!("databases/sales"),
            );
            document
                .nodes
                .get_mut(&consumer)
                .unwrap()
                .parameters
                .insert(key.parse().unwrap(), value);
            connect(
                &mut document,
                port(source, "dataframe"),
                port(consumer, "source"),
                None,
            );
            let mut cache = crate::GraphSemanticCache::default();
            let initial = resources(vec![ColumnSchema {
                name: "amount".into(),
                data_type: yss_data_contract::DataType::Int64,
            }]);
            let ready = crate::resolve_graph_semantics_with_cache(
                &document, &registry, &initial, &mut cache,
            );
            assert!(ready.ready().is_some(), "{:?}", ready.diagnostics());
            let changed = resources(vec![ColumnSchema {
                name: "replacement".into(),
                data_type: yss_data_contract::DataType::String,
            }]);
            let blocked = crate::resolve_graph_semantics_with_cache(
                &document, &registry, &changed, &mut cache,
            );
            assert!(blocked.ready().is_none());
            assert!(
                blocked
                    .diagnostics()
                    .iter()
                    .any(|diagnostic| diagnostic.code.as_str()
                        == GraphDiagnosticKind::SchemaParameterInvalid.code()
                        && diagnostic.primary
                            == GraphDiagnosticLocation::Parameter {
                                node_id: consumer,
                                key: key.parse().unwrap()
                            })
            );
            assert_eq!(
                blocked,
                crate::resolve_graph_semantics(&document, &registry, &changed)
            );
            let recovered = crate::resolve_graph_semantics_with_cache(
                &document, &registry, &initial, &mut cache,
            );
            assert!(recovered.ready().is_some());
        }
    }
}
