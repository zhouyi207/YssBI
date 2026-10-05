use super::*;

fn query(f: &mut Fixture, request: InspectGraphRequest) -> GraphInspectionPage {
    let result = f
        .action(AutomationCapabilityRequest::InspectGraph(request))
        .unwrap();
    let json = serde_json::to_value(&result).unwrap();
    assert_eq!(
        serde_json::from_value::<AutomationCapabilityResult>(json).unwrap(),
        result
    );
    let AutomationCapabilityResult::GraphInspectionPage(page) = result else {
        panic!("paged graph inspection")
    };
    page
}

fn source(f: &Fixture) -> GraphEditOperation {
    GraphEditOperation::CreateNode {
        port_counts: Default::default(),
        parameters: Default::default(),
        client_id: Some("source".into()),
        node_type_id: "yssbi.dataframe.source.get".into(),
        resource_path: Some(format!("databases/{}", f.dataset)),
        x: 0.,
        y: 0.,
        user_label: None,
    }
}

#[test]
fn graph_reads_page_overviews_and_fetch_only_requested_node_and_port_facts() {
    let mut f = Fixture::new();
    let mut operations = vec![source(&f), node("yssbi.dataframe.decompose", "columns")];
    operations.push(connect(
        port("$source", "dataframe"),
        port("$columns", "dataframe"),
    ));
    let receipt = f.edit(operations);
    let extra_nodes = (0..106)
        .flat_map(|index| {
            let alias = format!("product-{index}");
            [
                node("yssbi.numeric.multiply", &alias),
                GraphEditOperation::SetLiteral {
                    address: port(&format!("${alias}"), "left"),
                    literal: Some(serde_json::json!(1)),
                },
                GraphEditOperation::SetLiteral {
                    address: port(&format!("${alias}"), "right"),
                    literal: Some(serde_json::json!(2)),
                },
            ]
        })
        .collect::<Vec<_>>();
    for batch in extra_nodes.chunks(60) {
        f.edit(batch.to_vec());
    }
    let full = f.inspect();
    let mut request = InspectGraphRequest::overview(&f.path);
    let first = query(&mut f, request.clone());
    assert_eq!(first.counts.nodes, 108);
    assert_eq!(first.version, full.version);
    assert_eq!(first.graph_hash, full.graph_hash);
    assert_eq!(first.semantic_input_hash, full.semantic_input_hash);
    assert_eq!(first.ready, full.ready);
    assert_eq!(first.counts.diagnostics, full.diagnostics.len());
    let mut observed = BTreeSet::new();
    let mut overview_bytes = 0;
    loop {
        let page = query(&mut f, request.clone());
        overview_bytes += serde_json::to_vec(&page).unwrap().len();
        let GraphInspectionItems::Nodes(nodes) = &page.content else {
            panic!("nodes")
        };
        assert!(nodes.len() <= request.limit);
        for node in nodes {
            assert!(
                node.parameters.is_none()
                    && node.parameter_options.is_none()
                    && node.port_templates.is_none()
            );
            assert!(observed.insert(node.node_id.clone()));
        }
        let pagination = page.page.unwrap();
        assert_eq!(pagination.total, full.nodes.len());
        let Some(next) = pagination.next_offset else {
            break;
        };
        request.offset = next;
    }
    assert_eq!(
        observed,
        full.nodes.iter().map(|node| node.node_id.clone()).collect()
    );
    let full_bytes = serde_json::to_vec(&full).unwrap().len();
    eprintln!(
        "graph inspection: 108 nodes, full={full_bytes} bytes, all overview pages={overview_bytes} bytes"
    );
    assert!(
        overview_bytes * 4 < full_bytes,
        "overview should avoid bulk editor/pin data"
    );

    let mut detail = InspectGraphRequest {
        view: GraphInspectionView::Nodes,
        node_ids: vec![receipt.created_nodes["source"].clone()],
        ..InspectGraphRequest::overview(&f.path)
    };
    let page = query(&mut f, detail.clone());
    let GraphInspectionItems::Nodes(nodes) = page.content else {
        panic!("nodes")
    };
    let expected = full
        .nodes
        .iter()
        .find(|node| node.node_id == detail.node_ids[0])
        .unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(
        nodes[0].parameters.as_ref().unwrap(),
        &expected
            .parameters
            .iter()
            .map(|parameter| (parameter.key.clone(), parameter.value.clone()))
            .collect()
    );
    assert!(nodes[0].parameter_options.is_none());
    detail.include_options = true;
    let GraphInspectionItems::Nodes(nodes) = query(&mut f, detail).content else {
        panic!("nodes")
    };
    assert_eq!(
        nodes[0].parameter_options.as_ref().unwrap(),
        &expected.parameters
    );

    let column_node = full
        .nodes
        .iter()
        .find(|node| node.node_id == receipt.created_nodes["columns"])
        .unwrap();
    let mut ports = InspectGraphRequest {
        view: GraphInspectionView::Ports,
        node_ids: vec![column_node.node_id.clone()],
        limit: 1,
        ..InspectGraphRequest::overview(&f.path)
    };
    let mut addresses = Vec::new();
    loop {
        let page = query(&mut f, ports.clone());
        let GraphInspectionItems::Ports(items) = page.content else {
            panic!("ports")
        };
        assert_eq!(items.len(), 1);
        assert!(items[0].schema.is_none());
        assert!(items[0].schema_known.is_none());
        addresses.push(items[0].address.clone());
        let pagination = page.page.unwrap();
        assert_eq!(pagination.total, column_node.ports.len());
        let Some(next) = pagination.next_offset else {
            break;
        };
        ports.offset = next;
    }
    assert_eq!(
        addresses,
        column_node
            .ports
            .iter()
            .map(|port| port.address.clone())
            .collect::<Vec<_>>()
    );
    let expected = column_node
        .ports
        .iter()
        .find(|port| !port.schema.is_empty())
        .unwrap();
    ports.offset = 0;
    ports.port_addresses = vec![expected.address.clone()];
    ports.include_schema = true;
    let page = query(&mut f, ports);
    assert_eq!(page.page.as_ref().unwrap().total, 1);
    let GraphInspectionItems::Ports(items) = page.content else {
        panic!("ports")
    };
    assert_eq!(items[0].schema.as_ref(), Some(&expected.schema));
    assert_eq!(items[0].schema_known, Some(true));
    assert_eq!(items[0].data_type, expected.data_type);
    assert_eq!(items[0].maximum_connections, expected.maximum_connections);
    let edges = query(
        &mut f,
        InspectGraphRequest {
            view: GraphInspectionView::Connections,
            node_ids: vec![column_node.node_id.clone()],
            ..InspectGraphRequest::overview(&full.graph_path)
        },
    );
    assert_eq!(
        edges.content,
        GraphInspectionItems::Connections(full.connections.clone())
    );

    let resource = ProjectResourceRef {
        kind: ProjectResourceKind::EventGraph,
        id: f.path.clone(),
    };
    let mut resource_request = InspectResourceRequest {
        resource,
        graph_view: GraphInspectionView::Overview,
        metadata_only: false,
        offset: 0,
        limit: 50,
    };
    let AutomationCapabilityResult::ResourceInspection(value) = f
        .action(AutomationCapabilityRequest::InspectResource(
            resource_request.clone(),
        ))
        .unwrap()
    else {
        panic!("resource")
    };
    assert!(matches!(value.content, ResourceContent::GraphPage { graph, .. } if graph == first));
    resource_request.graph_view = GraphInspectionView::Full;
    let AutomationCapabilityResult::ResourceInspection(value) = f
        .action(AutomationCapabilityRequest::InspectResource(
            resource_request,
        ))
        .unwrap()
    else {
        panic!("resource")
    };
    assert!(matches!(value.content, ResourceContent::Graph { graph, .. } if graph == full));
}

#[test]
fn conditional_graph_reads_bind_query_pages_and_current_edit_and_dependency_identity() {
    let mut f = Fixture::new();
    let receipt = f.edit(vec![
        source(&f),
        node("yssbi.dataframe.decompose", "columns"),
        connect(port("$source", "dataframe"), port("$columns", "dataframe")),
    ]);
    let mut request = InspectGraphRequest {
        limit: 1,
        ..InspectGraphRequest::overview(&f.path)
    };
    let first = query(&mut f, request.clone());
    request.if_unchanged = Some(first.observation_hash.clone());
    let unchanged = query(&mut f, request.clone());
    assert_eq!(unchanged.content, GraphInspectionItems::Unchanged);
    assert!(unchanged.page.is_none());
    let second_page = query(
        &mut f,
        InspectGraphRequest {
            offset: 1,
            ..request.clone()
        },
    );
    assert!(matches!(
        second_page.content,
        GraphInspectionItems::Nodes(_)
    ));
    assert_ne!(second_page.observation_hash, first.observation_hash);
    let filtered = query(
        &mut f,
        InspectGraphRequest {
            node_ids: vec![receipt.created_nodes["columns"].clone()],
            ..request.clone()
        },
    );
    assert!(matches!(filtered.content, GraphInspectionItems::Nodes(_)));
    let parameters = query(
        &mut f,
        InspectGraphRequest {
            view: GraphInspectionView::Nodes,
            ..request.clone()
        },
    );
    assert!(matches!(parameters.content, GraphInspectionItems::Nodes(_)));
    let past_end = query(
        &mut f,
        InspectGraphRequest {
            offset: usize::MAX,
            ..request.clone()
        },
    );
    assert_eq!(past_end.content, GraphInspectionItems::Nodes(vec![]));
    assert_eq!(past_end.page.unwrap().next_offset, None);
    assert_eq!(
        f.action(AutomationCapabilityRequest::InspectGraph(
            InspectGraphRequest {
                node_ids: vec![uuid::Uuid::new_v4().to_string()],
                ..request.clone()
            }
        ))
        .unwrap_err()
        .code,
        CapabilityFailureCode::InvalidRequest
    );

    // A dataset schema change leaves the graph document/version unchanged, but changes its semantics.
    let database = ProjectResourceRef {
        kind: ProjectResourceKind::Database,
        id: f.dataset.clone(),
    };
    let AutomationCapabilityResult::ResourceInspection(value) = f
        .action(AutomationCapabilityRequest::InspectResource(
            InspectResourceRequest {
                resource: database.clone(),
                metadata_only: true,
                graph_view: GraphInspectionView::Overview,
                offset: 0,
                limit: 1,
            },
        ))
        .unwrap()
    else {
        panic!("database")
    };
    f.action(AutomationCapabilityRequest::EditResource(
        EditResourceRequest {
            resource: database,
            version: value.version,
            edit: ResourceEdit::Database {
                operation: DatasetOperation::RenameColumn {
                    old_name: "x".into(),
                    new_name: "renamed_x".into(),
                },
            },
        },
    ))
    .unwrap();
    let dependency_changed = query(&mut f, request.clone());
    assert_eq!(dependency_changed.version, first.version);
    assert_eq!(dependency_changed.graph_hash, first.graph_hash);
    assert_ne!(
        dependency_changed.semantic_input_hash,
        first.semantic_input_hash
    );
    assert!(matches!(
        dependency_changed.content,
        GraphInspectionItems::Nodes(_)
    ));

    request.if_unchanged = Some(dependency_changed.observation_hash.clone());
    f.edit(vec![node("yssbi.numeric.multiply", "added")]);
    let edited = query(&mut f, request.clone());
    assert_ne!(edited.version, dependency_changed.version);
    assert!(matches!(edited.content, GraphInspectionItems::Nodes(_)));
    request.if_unchanged = Some(edited.observation_hash.clone());
    let captured = f.application.as_ref().unwrap().capture_session().unwrap();
    captured
        .project()
        .unload_graph_resource(&GraphResourcePath::new(&f.path).unwrap())
        .unwrap();
    let reopened = query(&mut f, request);
    assert_ne!(reopened.version.session_id, edited.version.session_id);
    assert!(matches!(reopened.content, GraphInspectionItems::Nodes(_)));
}
