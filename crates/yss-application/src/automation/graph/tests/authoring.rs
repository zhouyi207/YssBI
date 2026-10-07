use super::*;
use yss_harness_contract::model::*;

fn command(f: &Fixture, input: GraphMutationInput) -> AutomationCapabilityRequest {
    AutomationCapabilityRequest::GraphMutation(GraphMutationRequest {
        input,
        base_revision: f.revision,
        graph_hash: graph_hash(&f.document).unwrap(),
        client_key: uuid::Uuid::new_v4().to_string(),
    })
}

fn apply(f: &mut Fixture, input: GraphMutationInput) -> GraphEditReceipt {
    let AutomationCapabilityResult::GraphEditReceipt(receipt) =
        f.action(command(f, input)).unwrap()
    else {
        panic!("mutation")
    };
    receipt
}

fn history(f: &mut Fixture, redo: bool) {
    let read = f.inspect();
    f.action(AutomationCapabilityRequest::EditResource(
        EditResourceRequest {
            resource: GraphResourceRef::for_path(&f.path).resource(),
            version: read.version,
            edit: ResourceEdit::GraphHistory {
                redo,
                graph_hash: read.graph_hash,
            },
        },
    ))
    .unwrap();
}

#[test]
fn typed_constants_page_values_and_preserve_atomic_references_and_replay() {
    use yss_data_contract::{DataValue, ValueType};
    let mut f = Fixture::new();
    f.inspect();
    let graph = GraphResourceRef::for_path(&f.path);
    let long_text = "文字🙂".repeat(20_000);
    let input: CreateConstantsInput = serde_json::from_value(serde_json::json!({
        "graph": graph,
        "constants": [
            {"clientId":"text","name":"Long text","value":{"dataType":{"kind":"Scalar","inner":"Text"},"dataValue":{"String":long_text}}},
            {"clientId":"table","name":"Table","value":{"dataType":{"kind":"DataFrame"},"tabular":{"columns":{"a":[1,2,3,4],"b":["w","x","y","z"]}}}},
            {"clientId":"exact","name":"Exact integer","value":{"dataType":{"kind":"Scalar","inner":"Numeric"},"dataValue":{"Integer":"9223372036854775807"}},"referenceNode":{"clientId":"source","label":"Exact source"}}
        ]
    })).unwrap();
    let mut invalid = input.clone();
    invalid.constants[2].value.data_value = DataValue::String("wrong type".into());
    assert!(
        f.action(command(&f, GraphMutationInput::CreateConstants(invalid)))
            .is_err()
    );
    assert!(f.inspect().constants.is_empty());
    let request = command(&f, GraphMutationInput::CreateConstants(input));
    let AutomationCapabilityResult::GraphEditReceipt(created) = f.action(request.clone()).unwrap()
    else {
        panic!("receipt")
    };
    assert_eq!(created.created_constants.len(), 3);
    assert_eq!(created.created_nodes.len(), 1);
    let AutomationCapabilityResult::GraphEditReceipt(replayed) = f.action(request).unwrap() else {
        panic!("receipt")
    };
    assert_eq!(replayed, created);
    let exact = &created.created_constants["exact"];
    let source = &created.created_nodes["source"];
    assert_eq!(
        f.document.constants[&constants::parse_id(exact).unwrap()].data_value,
        DataValue::Integer(i64::MAX)
    );
    let AutomationCapabilityResult::GraphInspectionPage(found) = f
        .action(AutomationCapabilityRequest::FindConstants(
            FindConstantsRequest {
                graph: graph.clone(),
                query: Some("LONG".into()),
                offset: 0,
                limit: 1,
            },
        ))
        .unwrap()
    else {
        panic!("page")
    };
    assert!(serde_json::to_string(&found).unwrap().len() < 3000);
    let GraphInspectionItems::ConstantSummaries(items) = found.content else {
        panic!("summary")
    };
    assert_eq!(items[0].constant_id, created.created_constants["text"]);
    let mut read: InspectConstantsRequest = serde_json::from_value(serde_json::json!({"graph":graph,"constantIds":[created.created_constants["text"]],"offset":1,"limit":4})).unwrap();
    let AutomationCapabilityResult::GraphInspectionPage(page) = f
        .action(AutomationCapabilityRequest::InspectConstants(read.clone()))
        .unwrap()
    else {
        panic!("page")
    };
    let GraphInspectionItems::ConstantDetails(details) = page.content else {
        panic!("details")
    };
    let ConstantReadValue::Value { value, page } = &details[0].value else {
        panic!("value")
    };
    assert_eq!(*value, DataValue::String("字🙂文字".into()));
    assert_eq!(page.next_offset, Some(5));
    assert_eq!(page.total, Some(long_text.chars().count()));
    read.constant_ids = vec![created.created_constants["table"].clone()];
    read.columns = vec!["b".into()];
    read.limit = 2;
    let AutomationCapabilityResult::GraphInspectionPage(page) = f
        .action(AutomationCapabilityRequest::InspectConstants(read))
        .unwrap()
    else {
        panic!("page")
    };
    let GraphInspectionItems::ConstantDetails(details) = page.content else {
        panic!("details")
    };
    let ConstantReadValue::Table {
        columns,
        rows,
        page,
        ..
    } = &details[0].value
    else {
        panic!("table")
    };
    assert_eq!(columns, &["b"]);
    assert_eq!(
        serde_json::to_value(rows).unwrap(),
        serde_json::json!([["x"], ["y"]])
    );
    assert_eq!(page.next_offset, Some(3));
    let authored = f.document.clone();
    let updates: UpdateConstantsInput = serde_json::from_value(serde_json::json!({"graph":graph,"constants":[
        {"constantId":exact,"name":"Changed"},
        {"constantId":created.created_constants["table"],"value":{"dataType":{"kind":"Scalar","inner":"Numeric"},"dataValue":{"String":"bad"}}}
    ]})).unwrap();
    assert!(
        f.action(command(&f, GraphMutationInput::UpdateConstants(updates)))
            .is_err()
    );
    f.inspect();
    assert_eq!(f.document, authored);
    apply(
        &mut f,
        GraphMutationInput::UpdateConstants(UpdateConstantsInput {
            graph: graph.clone(),
            constants: vec![ConstantUpdate {
                constant_id: exact.clone(),
                name: Some("Renamed".into()),
                value: Some(ConstantValueInput {
                    data_type: ValueType::number(),
                    data_value: DataValue::Integer(7),
                    tabular: None,
                }),
                description: Some(String::new()),
                tags: Some(vec![]),
            }],
        }),
    );
    let modified = f.document.clone();
    let removed = apply(
        &mut f,
        GraphMutationInput::DeleteConstants(DeleteConstantsInput {
            graph: graph.clone(),
            constant_ids: vec![exact.clone()],
        }),
    );
    assert_eq!(
        removed.changes.removed_constant_ids,
        std::slice::from_ref(exact)
    );
    assert!(
        f.document
            .nodes
            .contains_key(&parse_node_id(source).unwrap())
    );
    assert!(
        removed
            .changes
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.blocking)
    );
    history(&mut f, false);
    assert_eq!(f.document, modified);
    let nodes: CreateNodesInput = serde_json::from_value(serde_json::json!({"graph":graph,"nodes":[{"typeId":"yssbi.constant.get","constantId":exact}]})).unwrap();
    let mut invalid = nodes.clone();
    invalid.nodes[0]
        .parameters
        .insert("constant".into(), serde_json::Value::Null);
    assert!(
        f.action(command(&f, GraphMutationInput::CreateNodes(invalid)))
            .is_err()
    );
    let reference = apply(&mut f, GraphMutationInput::CreateNodes(nodes));
    assert_eq!(reference.created_nodes.len(), 1);
}

#[test]
fn explicit_authoring_batches_keep_pin_order_connection_identity_and_atomic_history() {
    let mut f = Fixture::new();
    f.inspect();
    let graph = GraphResourceRef::for_path(&f.path);
    let input: CreateNodesInput = serde_json::from_value(serde_json::json!({
        "graph": graph,
        "nodes": [
            {"clientId":"a","typeId":"yssbi.numeric.multiply"},
            {"clientId":"b","typeId":"yssbi.numeric.multiply"},
            {"clientId":"c","typeId":"yssbi.numeric.multiply"},
            {"clientId":"fit","typeId":"yssbi.statistics.linear.fit", "parameters":{"method":"OLS"}, "portCounts":{"x":5}, "label":"Fit"}
        ],
        "connections": [
            {"output": port("$a", "result"), "input": port("$b", "left")},
            {"output": port("$c", "result"), "input": port("$b", "right")}
        ]
    })).unwrap();
    let mut invalid = input.clone();
    invalid.connections.push(ConnectionDeclaration {
        output: port("$a", "result"),
        input: port("$fit", "missing"),
        order: None,
    });
    let before = f.document.clone();
    let failure = f
        .action(command(&f, GraphMutationInput::CreateNodes(invalid)))
        .unwrap_err();
    assert_eq!(failure.code, CapabilityFailureCode::MutationRejected);
    assert_eq!(f.inspect().nodes.len(), 0);
    assert_eq!(f.document, before);
    for (reference, reason) in [
        ("a", "invalid_node_reference"),
        ("$missing", "unknown_node_alias"),
    ] {
        let mut invalid = input.clone();
        invalid.connections[0].output = port(reference, "result");
        let failure = f
            .action(command(&f, GraphMutationInput::CreateNodes(invalid)))
            .unwrap_err();
        assert_eq!(failure.code, CapabilityFailureCode::InvalidRequest);
        assert_eq!(failure.details["reason"], reason);
        assert!(failure.details["nextStep"].contains("$clientId"));
        assert_eq!(f.document, before);
    }
    let created = apply(&mut f, GraphMutationInput::CreateNodes(input));
    assert_eq!(created.created_nodes.len(), 4);
    assert_eq!(created.created_ports.len(), 5);
    assert_eq!(created.changes.connections.len(), 2);
    let authored = f.document.clone();
    history(&mut f, false);
    assert!(f.document.nodes.is_empty() && f.document.connections.is_empty());
    history(&mut f, true);
    assert_eq!(f.document, authored);
    let a = &created.created_nodes["a"];
    let b = &created.created_nodes["b"];
    let c = &created.created_nodes["c"];
    let fit = &created.created_nodes["fit"];
    let removals: UpdateNodesInput = serde_json::from_value(serde_json::json!({
        "graph": graph,
        "nodes": [{"nodeId":fit,"removePins":[created.created_ports["fit.x[1]"]]}]
    }))
    .unwrap();
    apply(&mut f, GraphMutationInput::UpdateNodes(removals));
    for index in 0..5 {
        let address =
            parse_edit_port(created.created_ports[&format!("fit.x[{index}]")].clone()).unwrap();
        assert_eq!(f.document.port_bindings.contains_key(&address), index != 1);
    }
    history(&mut f, false);
    assert_eq!(f.document, authored);
    let mut updates: UpdateNodesInput = serde_json::from_value(serde_json::json!({
        "graph": graph, "nodes": [
            {"nodeId":a,"label":"Changed", "literals":[{"address":port(a,"left"),"value":2}]},
            {"nodeId":fit,"portCounts":{"y":2}}
        ]
    }))
    .unwrap();
    let before = f.document.clone();
    assert!(
        f.action(command(
            &f,
            GraphMutationInput::UpdateNodes(updates.clone())
        ))
        .is_err()
    );
    f.inspect();
    assert_eq!(f.document, before);
    updates.nodes[1].port_counts = [("x".into(), 2)].into();
    updates.nodes[1].label = Some(None);
    let edited = apply(&mut f, GraphMutationInput::UpdateNodes(updates));
    assert_eq!(
        f.document.nodes[&parse_node_id(fit).unwrap()].user_label,
        None
    );
    assert_eq!(
        f.document.nodes[&parse_node_id(a).unwrap()]
            .user_label
            .as_deref(),
        Some("Changed")
    );
    for index in 0..5 {
        let address =
            parse_edit_port(created.created_ports[&format!("fit.x[{index}]")].clone()).unwrap();
        assert_eq!(f.document.port_bindings.contains_key(&address), index < 2);
    }
    assert!(edited.changes.nodes.iter().any(|node| node.node_id == *fit));
    let original = f.document.connections.values().cloned().collect::<Vec<_>>();
    let updates = original
        .iter()
        .map(|connection| GraphConnectionUpdate {
            connection_id: connection.id.to_string(),
            output: edit_port(&connection.output),
            input: if connection.input == parse_edit_port(port(b, "left")).unwrap() {
                port(b, "right")
            } else {
                port(b, "left")
            },
            order: None,
        })
        .collect::<Vec<_>>();
    let mut invalid = updates.clone();
    invalid[1].input = port(b, "missing");
    let before = f.document.clone();
    assert!(
        f.action(command(
            &f,
            GraphMutationInput::UpdateConnections(UpdateConnectionsInput {
                graph: graph.clone(),
                connections: invalid
            })
        ))
        .is_err()
    );
    f.inspect();
    assert_eq!(f.document, before);
    apply(
        &mut f,
        GraphMutationInput::UpdateConnections(UpdateConnectionsInput {
            graph: graph.clone(),
            connections: updates.clone(),
        }),
    );
    for update in &updates {
        let id = ConnectionId::from_uuid(uuid::Uuid::parse_str(&update.connection_id).unwrap());
        let actual = &f.document.connections[&id];
        assert_eq!(actual.input, parse_edit_port(update.input.clone()).unwrap());
        assert_eq!(
            actual.output,
            parse_edit_port(update.output.clone()).unwrap()
        );
    }
    let copied = apply(
        &mut f,
        GraphMutationInput::DuplicateNodes(DuplicateNodesInput {
            graph: graph.clone(),
            node_ids: vec![a.clone(), b.clone(), fit.clone()],
            offset: NodePositionInput { x: 100., y: 100. },
        }),
    );
    assert_eq!(copied.changes.nodes.len(), 3);
    assert_eq!(copied.changes.connections.len(), 1);
    assert_eq!(copied.created_nodes.len(), 3);
    for source in [a, b, fit] {
        assert_ne!(&copied.created_nodes[source], source);
    }
    for index in 0..2 {
        let source =
            parse_edit_port(created.created_ports[&format!("fit.x[{index}]")].clone()).unwrap();
        let target = parse_edit_port(copied.created_ports[&source.to_string()].clone()).unwrap();
        assert_ne!(target, source);
        assert_eq!(target.node_id.to_string(), copied.created_nodes[fit]);
        assert!(f.document.port_bindings.contains_key(&target));
    }
    let source = parse_edit_port(port(a, "result")).unwrap();
    assert_eq!(
        copied.created_ports[&source.to_string()],
        port(&copied.created_nodes[a], "result")
    );
    let copied_ids = copied
        .changes
        .nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<Vec<_>>();
    apply(
        &mut f,
        GraphMutationInput::MoveNodes(MoveNodesInput {
            graph: graph.clone(),
            positions: vec![GraphEditPosition {
                node_id: c.clone(),
                x: 200.,
                y: 300.,
            }],
        }),
    );
    assert_eq!(
        f.document.nodes[&parse_node_id(c).unwrap()].position,
        NodePosition { x: 200., y: 300. }
    );
    let removed = apply(
        &mut f,
        GraphMutationInput::DeleteNodes(DeleteNodesInput {
            graph: graph.clone(),
            node_ids: copied_ids,
        }),
    );
    assert_eq!(removed.changes.removed_node_ids.len(), 3);
    assert_eq!(removed.changes.removed_connection_ids.len(), 1);
    apply(
        &mut f,
        GraphMutationInput::DeleteConnections(DeleteConnectionsInput {
            graph: graph.clone(),
            connection_ids: updates
                .iter()
                .map(|connection| connection.connection_id.clone())
                .collect(),
        }),
    );
    assert!(f.document.connections.is_empty());
    let connected = apply(
        &mut f,
        GraphMutationInput::CreateConnections(CreateConnectionsInput {
            graph,
            connections: vec![ConnectionDeclaration {
                output: port(a, "result"),
                input: port(b, "left"),
                order: None,
            }],
        }),
    );
    assert_eq!(connected.changes.connections.len(), 1);
}
