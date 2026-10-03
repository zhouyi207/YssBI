use std::collections::BTreeMap;

use yss_graph_diagnostics::GraphDiagnosticKind;
use yss_graph_document::{GraphDocument, PortAddress};
use yss_node_protocol::{PortDirection, TypeExpr, validate_typed_value};
use yss_node_registry::NodeRegistry;

use crate::{GraphDiagnosticFact, GraphDiagnosticLocation, GraphNodeSemanticFact, graph_problem};

pub(crate) fn validate(
    document: &GraphDocument,
    index: &crate::document_index::DocumentIndex<'_>,
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
        if !index.input_connections(address).is_empty() {
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
        for parameter in &node.parameters {
            let Some(schema) =
                super::parameter_projection::parameter_schema(&node.ports, parameter.key.as_str())
            else {
                continue;
            };
            let Some(value) = super::parameter_projection::parameter_literal_value(parameter)
            else {
                continue;
            };
            if super::parameter_projection::aggregate_parameter_accepts(
                node.node_type.as_str(),
                parameter.key.as_str(),
                yss_node_protocol::RelationalScalarType::Unknown,
            )
            .is_some()
            {
                let valid = super::schema_resolution::aggregate_column_names(Some(&value))
                    .is_ok_and(|columns| {
                        columns.iter().all(|name| {
                            schema.fields.iter().any(|field| {
                                field.name.0 == *name
                                    && super::parameter_projection::aggregate_parameter_accepts(
                                        node.node_type.as_str(),
                                        parameter.key.as_str(),
                                        field.scalar_type,
                                    ) == Some(true)
                            })
                        })
                    });
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
                continue;
            }
            let TypeExpr::Concrete(type_id) = &parameter.value_type else {
                continue;
            };
            use yss_node_protocol::dataframe::{
                FILTER_PREDICATE_TYPE_ID, PROJECT_COLUMNS_TYPE_ID, filter_comparison_is_compatible,
                prepare_filter_predicate_json, prepare_project_columns_json,
            };
            let valid = match type_id.as_str() {
                PROJECT_COLUMNS_TYPE_ID => {
                    prepare_project_columns_json(&value).is_ok_and(|columns| {
                        columns
                            .as_slice()
                            .iter()
                            .all(|name| schema.fields.iter().any(|field| &field.name.0 == name))
                    })
                }
                FILTER_PREDICATE_TYPE_ID => {
                    prepare_filter_predicate_json(&value).is_ok_and(|predicate| {
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
    use yss_data_contract::DataValue;
    use yss_graph_document::{
        ConnectionId, DocumentConnection, DocumentNode, InputState, NodeId, NodePosition, OrderKey,
        ParameterValues,
    };
    use yss_graph_resource_contract::{
        ColumnSchema, DataSchema, GraphResourceId, ResourceCatalogSnapshot,
    };
    use yss_node_protocol::{PortKey, TypeId, TypedValue};

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
            BTreeMap::from([(
                GraphResourceId::new("databases/sales"),
                DataSchema { columns },
            )]),
        )
    }

    #[test]
    fn effective_parameter_defaults_follow_input_schema_and_visibility() {
        use yss_node_protocol::{Parameter, ParameterCondition, normalize_json_literal};
        let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
        let mut protocol = builtin
            .registry
            .protocol(&"yssbi.dataframe.filter.rows".parse().unwrap())
            .unwrap()
            .clone();
        let mut predicate = protocol.parameters.iter().next().unwrap().clone();
        let default = serde_json::json!({"column":"amount", "operator":"greaterThan", "value":{"type":"integer", "value":"1"}});
        predicate.default_value = Some(
            normalize_json_literal(&default, &predicate.value_type, builtin.registry.as_ref())
                .unwrap(),
        );
        let mut enabled = Parameter::number("enabled").int().default(1);
        enabled.title_key = predicate.title_key.clone();
        predicate.visible_when = Some(ParameterCondition {
            key: enabled.key.clone(),
            values: Box::new([DataValue::Integer(1)]),
        });
        protocol.parameters.groups[0].parameters = Box::new([predicate, enabled]);
        protocol.type_id = "tests.graph.parameters".parse().unwrap();
        let registry = crate::tests::registry_with_protocols([protocol]);
        let mut document = GraphDocument::default();
        let source = node(&mut document, "yssbi.dataframe.source.get");
        let consumer = node(&mut document, "tests.graph.parameters");
        document.nodes.get_mut(&source).unwrap().parameters.insert(
            "dataframe".parse().unwrap(),
            serde_json::json!("databases/sales"),
        );
        connect(
            &mut document,
            port(source, "dataframe"),
            port(consumer, "source"),
            None,
        );
        let columns = |name: &str| {
            resources(vec![ColumnSchema {
                name: name.into(),
                data_type: yss_data_contract::ValueType::Scalar(
                    yss_data_contract::SemanticType::Numeric,
                ),
                semantic: None,
                physical_type: None,
            }])
        };
        let mut cache = crate::GraphSemanticCache::default();
        let initial = crate::resolve_graph_semantics_with_cache(
            &document,
            &registry,
            &columns("amount"),
            &mut cache,
        );
        assert!(initial.ready().is_some(), "{:?}", initial.diagnostics());
        let changed = columns("replacement");
        let blocked =
            crate::resolve_graph_semantics_with_cache(&document, &registry, &changed, &mut cache);
        assert!(
            blocked.diagnostics().iter().any(|diagnostic| {
                diagnostic.code.as_str() == GraphDiagnosticKind::SchemaParameterInvalid.code()
                    && diagnostic.primary
                        == GraphDiagnosticLocation::Parameter {
                            node_id: consumer,
                            key: "predicate".parse().unwrap(),
                        }
            }),
            "{:?}",
            blocked.diagnostics()
        );
        assert!(blocked.ready().is_none());
        assert_eq!(
            blocked,
            crate::resolve_graph_semantics(&document, &registry, &changed)
        );
        assert!(document.nodes[&consumer].parameters.is_empty());

        document
            .nodes
            .get_mut(&consumer)
            .unwrap()
            .parameters
            .insert(
                "predicate".parse().unwrap(),
                serde_json::json!({"column":"replacement", "operator":"isNull"}),
            );
        assert!(
            crate::resolve_graph_semantics_with_cache(&document, &registry, &changed, &mut cache)
                .ready()
                .is_some()
        );
        document
            .nodes
            .get_mut(&consumer)
            .unwrap()
            .parameters
            .remove(&"predicate".parse().unwrap());
        document
            .nodes
            .get_mut(&consumer)
            .unwrap()
            .parameters
            .insert("enabled".parse().unwrap(), serde_json::json!(0));
        let hidden =
            crate::resolve_graph_semantics_with_cache(&document, &registry, &changed, &mut cache);
        assert!(hidden.ready().is_some(), "{:?}", hidden.diagnostics());
        assert!(
            hidden
                .node(consumer)
                .unwrap()
                .parameters
                .iter()
                .all(|parameter| parameter.key.as_str() != "predicate")
        );
        document
            .nodes
            .get_mut(&consumer)
            .unwrap()
            .parameters
            .insert("predicate".parse().unwrap(), default);
        let stale = crate::resolve_graph_semantics(&document, &registry, &changed);
        assert!(
            stale
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code.as_str()
                    == GraphDiagnosticKind::ParameterInvalid.code())
        );
        assert!(
            stale
                .diagnostics()
                .iter()
                .all(|diagnostic| diagnostic.code.as_str()
                    != GraphDiagnosticKind::SchemaParameterInvalid.code())
        );
    }

    #[test]
    fn effective_parameter_resources_record_defaults_and_ignore_inactive_values() {
        use yss_node_protocol::{NodeInstanceDisplaySpec, Parameter, ParameterCondition};
        let builtin = yss_node_catalog::build_builtin_node_system().unwrap();
        let mut protocol = builtin
            .registry
            .protocol(&"yssbi.dataframe.source.get".parse().unwrap())
            .unwrap()
            .clone();
        protocol.interface.ports = Box::new([]);
        protocol.instance_display = NodeInstanceDisplaySpec::Static;
        let mut resource = protocol.parameters.iter().next().unwrap().clone();
        resource.default_value = Some(TypedValue {
            value_type: resource.value_type.clone(),
            value: DataValue::String("databases/sales".into()),
        });
        let mut enabled = Parameter::number("enabled").int().default(1);
        enabled.title_key = resource.title_key.clone();
        resource.visible_when = Some(ParameterCondition {
            key: enabled.key.clone(),
            values: Box::new([DataValue::Integer(1)]),
        });
        protocol.parameters.groups[0].parameters = Box::new([resource, enabled]);
        protocol.type_id = "tests.graph.parameters".parse().unwrap();
        let registry = crate::tests::registry_with_protocols([protocol]);
        let mut document = GraphDocument::default();
        let consumer = node(&mut document, "tests.graph.parameters");
        let missing = ResourceCatalogSnapshot::new(BTreeMap::new(), BTreeMap::new());
        let observed = missing.tracked();
        let mut cache = crate::GraphSemanticCache::default();
        let blocked =
            crate::resolve_graph_semantics_with_cache(&document, &registry, &observed, &mut cache);
        assert!(
            blocked.diagnostics().iter().any(|diagnostic| {
                diagnostic.code.as_str() == GraphDiagnosticKind::ResourceResolutionFailed.code()
                    && diagnostic.primary
                        == GraphDiagnosticLocation::Resource("databases/sales".into())
            }),
            "{:?}",
            blocked.diagnostics()
        );
        assert!(blocked.ready().is_none());
        let available = resources(vec![]);
        assert!(!available.matches_dependencies(&observed.dependencies()));
        let recovered =
            crate::resolve_graph_semantics_with_cache(&document, &registry, &available, &mut cache);
        assert!(recovered.ready().is_some(), "{:?}", recovered.diagnostics());
        assert_eq!(
            recovered,
            crate::resolve_graph_semantics(&document, &registry, &available)
        );
        assert!(document.nodes[&consumer].parameters.is_empty());

        document
            .nodes
            .get_mut(&consumer)
            .unwrap()
            .parameters
            .insert(
                "dataframe".parse().unwrap(),
                serde_json::json!("databases/other"),
            );
        let overridden = crate::resolve_graph_semantics(&document, &registry, &available);
        assert!(
            overridden
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.primary
                    == GraphDiagnosticLocation::Resource("databases/other".into()))
        );
        document
            .nodes
            .get_mut(&consumer)
            .unwrap()
            .parameters
            .remove(&"dataframe".parse().unwrap());
        document
            .nodes
            .get_mut(&consumer)
            .unwrap()
            .parameters
            .insert("enabled".parse().unwrap(), serde_json::json!(0));
        let hidden_resources = missing.tracked();
        let hidden = crate::resolve_graph_semantics_with_cache(
            &document,
            &registry,
            &hidden_resources,
            &mut cache,
        );
        assert!(hidden.ready().is_some(), "{:?}", hidden.diagnostics());
        assert!(available.matches_dependencies(&hidden_resources.dependencies()));
        document
            .nodes
            .get_mut(&consumer)
            .unwrap()
            .parameters
            .insert(
                "dataframe".parse().unwrap(),
                serde_json::json!("databases/other"),
            );
        let stale = crate::resolve_graph_semantics(&document, &registry, &available);
        assert!(
            stale
                .diagnostics()
                .iter()
                .any(|diagnostic| diagnostic.code.as_str()
                    == GraphDiagnosticKind::ParameterInvalid.code())
        );
        assert!(
            stale
                .diagnostics()
                .iter()
                .all(|diagnostic| diagnostic.code.as_str()
                    != GraphDiagnosticKind::ResourceResolutionFailed.code())
        );
    }

    #[test]
    fn imported_connection_and_literal_errors_are_canonical_and_locatable() {
        let registry = yss_node_catalog::build_builtin_node_system()
            .unwrap()
            .registry;
        let mut document = GraphDocument::default();
        let source = node(&mut document, "yssbi.constant.get");
        crate::tests::set_constant(
            &mut document,
            source,
            yss_data_contract::ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
            yss_data_contract::DataValue::Integer(0),
        );
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
                        value_type: TypeExpr::Concrete(TypeId::new("core.text").unwrap()),
                        value: DataValue::String("invalid".into()),
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
        let registry = yss_node_catalog::build_builtin_node_system()
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
                semantic: None,
                physical_type: None,
                name: "amount".into(),
                data_type: yss_data_contract::ValueType::Scalar(
                    yss_data_contract::SemanticType::Numeric,
                ),
            }]);
            let ready = crate::resolve_graph_semantics_with_cache(
                &document, &registry, &initial, &mut cache,
            );
            assert!(ready.ready().is_some(), "{:?}", ready.diagnostics());
            use crate::{GraphFilterLiteralType, GraphParameterConfigurationFact};
            use yss_node_protocol::dataframe::FilterOperator;
            match ready.node(consumer).unwrap().parameters[0]
                .configuration
                .as_ref()
                .unwrap()
            {
                GraphParameterConfigurationFact::ProjectColumns {
                    available,
                    options,
                    value,
                    ..
                } => {
                    assert!(*available);
                    assert_eq!(options[0].name.as_ref(), "amount");
                    assert_eq!(value.as_ref(), &[Box::<str>::from("amount")]);
                }
                GraphParameterConfigurationFact::FilterPredicate {
                    available,
                    columns,
                    value,
                    ..
                } => {
                    assert!(*available);
                    assert_eq!(columns[0].name.as_ref(), "amount");
                    assert_eq!(
                        columns[0].literal_types.as_ref(),
                        &[
                            GraphFilterLiteralType::Integer,
                            GraphFilterLiteralType::Decimal
                        ]
                    );
                    assert!(columns[0].operators.contains(&FilterOperator::GreaterThan));
                    assert!(value.is_some());
                }
                other => panic!("missing schema editor: {other:?}"),
            }
            let changed = resources(vec![ColumnSchema {
                semantic: None,
                physical_type: None,
                name: "replacement".into(),
                data_type: yss_data_contract::ValueType::Scalar(
                    yss_data_contract::SemanticType::Text,
                ),
            }]);
            let blocked = crate::resolve_graph_semantics_with_cache(
                &document, &registry, &changed, &mut cache,
            );
            assert!(blocked.ready().is_none());
            match blocked.node(consumer).unwrap().parameters[0]
                .configuration
                .as_ref()
                .unwrap()
            {
                GraphParameterConfigurationFact::ProjectColumns { options, value, .. } => {
                    assert_eq!(options[0].name.as_ref(), "replacement");
                    assert_eq!(value.as_ref(), &[Box::<str>::from("amount")]);
                }
                GraphParameterConfigurationFact::FilterPredicate { columns, .. } => {
                    assert_eq!(columns[0].name.as_ref(), "replacement");
                    assert_eq!(
                        columns[0].literal_types.as_ref(),
                        &[GraphFilterLiteralType::String]
                    );
                }
                other => panic!("missing refreshed editor: {other:?}"),
            }

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
            document.connections.clear();
            let disconnected = crate::resolve_graph_semantics(&document, &registry, &initial);
            match disconnected.node(consumer).unwrap().parameters[0]
                .configuration
                .as_ref()
                .unwrap()
            {
                GraphParameterConfigurationFact::ProjectColumns {
                    available, options, ..
                } => {
                    assert!(!available);
                    assert!(options.is_empty());
                }
                GraphParameterConfigurationFact::FilterPredicate {
                    available, columns, ..
                } => {
                    assert!(!available);
                    assert!(columns.is_empty());
                }
                other => panic!("missing disconnected editor: {other:?}"),
            }
        }
    }
}
