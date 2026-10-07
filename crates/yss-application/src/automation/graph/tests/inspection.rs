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
fn catalog_pages_filter_before_projection_and_resolve_exact_type_batches() {
    let mut f = Fixture::new();
    let mut request = BrowseNodesRequest {
        query: String::new(),
        locale: "en-US".into(),
        category: None,
        offset: 0,
        limit: 1,
    };
    let AutomationCapabilityResult::NodeCatalogPage(first) = f
        .action(AutomationCapabilityRequest::BrowseNodes(request.clone()))
        .unwrap()
    else {
        panic!("catalog")
    };
    assert!(first.page.has_more);
    assert_eq!(first.page.returned, 1);
    assert!(
        !serde_json::to_string(&first)
            .unwrap()
            .contains("configurationSchema")
    );
    request.offset = first.page.next_offset.unwrap();
    let AutomationCapabilityResult::NodeCatalogPage(second) = f
        .action(AutomationCapabilityRequest::BrowseNodes(request.clone()))
        .unwrap()
    else {
        panic!("catalog")
    };
    assert_ne!(first.matches, second.matches);
    request.category = Some(first.matches[0].category_id.clone());
    request.offset = 0;
    request.limit = 100;
    let AutomationCapabilityResult::NodeCatalogPage(category) = f
        .action(AutomationCapabilityRequest::BrowseNodes(request))
        .unwrap()
    else {
        panic!("catalog")
    };
    assert!(!category.matches.is_empty());
    assert!(
        category
            .matches
            .iter()
            .all(|item| item.category_id == first.matches[0].category_id)
    );
    let first_id = first.matches[0].type_id.clone();
    let second_id = second.matches[0].type_id.clone();
    let AutomationCapabilityResult::NodeTypeInspection(definitions) = f
        .action(AutomationCapabilityRequest::InspectNodeType(
            InspectNodeTypeRequest {
                type_ids: vec![first_id.clone(), second_id.clone(), first_id.clone()],
                locale: "en-US".into(),
            },
        ))
        .unwrap()
    else {
        panic!("definitions")
    };
    assert_eq!(
        definitions.types.len(),
        if first_id == second_id { 1 } else { 2 }
    );
    assert!(
        definitions
            .types
            .iter()
            .all(|item| item.configuration_schema.is_object())
    );
    let error = f
        .action(AutomationCapabilityRequest::InspectNodeType(
            InspectNodeTypeRequest {
                type_ids: vec![first_id, "yssbi.not_a_node".into()],
                locale: "en-US".into(),
            },
        ))
        .unwrap_err();
    assert_eq!(error.code, CapabilityFailureCode::CatalogUnavailable);

    for query in ["to_numeric", "YSSBI.VALUE.TO_NUMERIC"] {
        let AutomationCapabilityResult::NodeCatalogPage(page) = f
            .action(AutomationCapabilityRequest::BrowseNodes(
                BrowseNodesRequest {
                    query: query.into(),
                    locale: "zh-CN".into(),
                    category: None,
                    offset: 0,
                    limit: 100,
                },
            ))
            .unwrap()
        else {
            panic!("catalog")
        };
        assert_eq!(page.matches.len(), 1, "{query}: {:?}", page.matches);
        assert_eq!(page.matches[0].type_id, "yssbi.value.to_numeric");
    }
    let AutomationCapabilityResult::NodeCatalogPage(page) = f
        .action(AutomationCapabilityRequest::BrowseNodes(
            BrowseNodesRequest {
                query: "bin 分箱".into(),
                locale: "zh-CN".into(),
                category: None,
                offset: 0,
                limit: 100,
            },
        ))
        .unwrap()
    else {
        panic!("catalog")
    };
    assert!(
        !page
            .matches
            .iter()
            .any(|item| item.type_id.contains("durbin") || item.type_id.ends_with(".combine"))
    );
    let AutomationCapabilityResult::NodeCatalogPage(page) = f
        .action(AutomationCapabilityRequest::BrowseNodes(
            BrowseNodesRequest {
                query: "线性回归".into(),
                locale: "zh-CN".into(),
                category: None,
                offset: 0,
                limit: 100,
            },
        ))
        .unwrap()
    else {
        panic!("catalog")
    };
    assert!(
        page.matches
            .iter()
            .any(|item| item.type_id == "yssbi.statistics.linear.fit")
    );
}

#[test]
fn targeted_graph_queries_page_filtered_entities_and_preserve_unknown_schema_without_running() {
    let mut f = Fixture::new();
    let mut first = node("yssbi.numeric.multiply", "first");
    let mut second = node("yssbi.numeric.multiply", "second");
    for (operation, label) in [(&mut first, "Target A"), (&mut second, "target B")] {
        let GraphEditOperation::CreateNode { user_label, .. } = operation else {
            unreachable!()
        };
        *user_label = Some(label.into());
    }
    let receipt = f.edit(vec![
        source(&f),
        node("yssbi.dataframe.decompose", "known"),
        node("yssbi.dataframe.decompose", "unknown"),
        first,
        second,
        connect(port("$source", "dataframe"), port("$known", "dataframe")),
    ]);
    let graph = GraphResourceRef::for_path(&f.path);
    let before = f.inspect();
    let application = f.application.as_ref().unwrap().clone();
    let session = application.capture_session().unwrap();
    let result_revision = session.execution().result_revision();
    let mut request = FindNodesRequest {
        graph: graph.clone(),
        query: Some("TARGET".into()),
        type_ids: vec!["yssbi.numeric.multiply".into()],
        node_ids: vec![],
        offset: 1,
        limit: 1,
    };
    let AutomationCapabilityResult::GraphInspectionPage(page) = f
        .action(AutomationCapabilityRequest::FindNodes(request.clone()))
        .unwrap()
    else {
        panic!("nodes")
    };
    assert_eq!(page.page.as_ref().unwrap().total, 2);
    let GraphInspectionItems::Nodes(nodes) = &page.content else {
        panic!("nodes")
    };
    assert_eq!(nodes.len(), 1);
    assert!(nodes[0].parameters.is_none());
    let public =
        model::capability_result(&AutomationCapabilityResult::GraphInspectionPage(page)).unwrap();
    assert_eq!(public["payload"]["page"]["returned"], 1);
    assert_eq!(public["payload"]["page"]["hasMore"], false);
    request.offset = usize::MAX;
    let AutomationCapabilityResult::GraphInspectionPage(page) = f
        .action(AutomationCapabilityRequest::FindNodes(request))
        .unwrap()
    else {
        panic!("nodes")
    };
    assert_eq!(page.page.unwrap().offset, 2);
    assert!(matches!(page.content, GraphInspectionItems::Nodes(nodes) if nodes.is_empty()));
    let request = InspectNodesRequest {
        graph: graph.clone(),
        node_ids: vec![
            receipt.created_nodes["known"].clone(),
            receipt.created_nodes["unknown"].clone(),
        ],
        fields: vec![NodeInspectionField::Parameters, NodeInspectionField::Schema],
        port_offset: 0,
        port_limit: 100,
        column_offset: 1,
        column_limit: 1,
    };
    let AutomationCapabilityResult::GraphInspectionPage(page) = f
        .action(AutomationCapabilityRequest::InspectNodes(request.clone()))
        .unwrap()
    else {
        panic!("details")
    };
    let GraphInspectionItems::NodeDetails(details) = page.content else {
        panic!("details")
    };
    let known = details
        .iter()
        .find(|node| node.node.node_id == receipt.created_nodes["known"])
        .unwrap();
    let schema = known
        .ports
        .as_ref()
        .unwrap()
        .iter()
        .find(|port| port.schema_known == Some(true))
        .unwrap();
    assert_eq!(schema.schema.as_ref().unwrap().len(), 1);
    assert_eq!(schema.schema_page.as_ref().unwrap().offset, 1);
    assert_eq!(schema.schema_page.as_ref().unwrap().total, Some(2));
    let unknown = details
        .iter()
        .find(|node| node.node.node_id == receipt.created_nodes["unknown"])
        .unwrap();
    assert!(
        unknown
            .ports
            .as_ref()
            .unwrap()
            .iter()
            .all(|port| port.schema_known == Some(false)
                && port.schema.is_none()
                && port.schema_page.is_none())
    );
    let AutomationCapabilityResult::GraphInspectionPage(page) = f
        .action(AutomationCapabilityRequest::InspectNodes(
            InspectNodesRequest {
                port_limit: 1,
                ..request
            },
        ))
        .unwrap()
    else {
        panic!("ports")
    };
    let GraphInspectionItems::NodeDetails(details) = page.content else {
        panic!("ports")
    };
    assert!(
        details
            .iter()
            .all(|node| node.ports.as_ref().unwrap().len() == 1)
    );
    let AutomationCapabilityResult::GraphInspectionPage(page) = f
        .action(AutomationCapabilityRequest::FindConnections(
            FindConnectionsRequest {
                graph: graph.clone(),
                node_ids: vec![receipt.created_nodes["source"].clone()],
                ports: vec![port(&receipt.created_nodes["known"], "dataframe")],
                offset: 0,
                limit: 1,
            },
        ))
        .unwrap()
    else {
        panic!("connections")
    };
    assert_eq!(
        page.content,
        GraphInspectionItems::Connections(before.connections.clone())
    );
    let AutomationCapabilityResult::GraphInspectionPage(summary) = f
        .action(AutomationCapabilityRequest::InspectGraph(
            InspectGraphRequest::summary(graph),
        ))
        .unwrap()
    else {
        panic!("summary")
    };
    assert_eq!(summary.content, GraphInspectionItems::Summary);
    assert_eq!(summary.counts.nodes, 5);
    assert_eq!(summary.runs, Some(vec![]));
    let overview = summary.overview.as_ref().unwrap();
    assert_eq!(overview.detail, GraphOverviewDetail::Configuration);
    assert_eq!(overview.nodes.len(), 5);
    assert_eq!(overview.connections, before.connections);
    for node in &overview.nodes {
        assert!(node.parameter_options.is_none() && node.port_templates.is_none());
        let stored = &f.document.nodes[&parse_node_id(&node.node_id).unwrap()];
        for (key, value) in stored.parameters.iter() {
            assert_eq!(
                node.parameters.as_ref().unwrap()[key.as_str()],
                Some(value.clone())
            );
        }
    }
    let public = model::capability_result(&AutomationCapabilityResult::GraphInspectionPage(
        summary.clone(),
    ))
    .unwrap();
    assert_eq!(
        public["payload"]["overview"],
        serde_json::to_value(overview).unwrap()
    );
    assert!(public["payload"].get("version").is_none());
    assert_eq!(session.execution().result_revision(), result_revision);
    assert_eq!(f.inspect(), before);
}

#[test]
fn initial_graph_context_downgrades_whole_sections_by_encoded_size_without_truncating_structure() {
    let mut f = Fixture::new();
    f.inspect();
    let mut group = node("yssbi.dataframe.groupby", "group");
    let GraphEditOperation::CreateNode { parameters, .. } = &mut group else {
        unreachable!()
    };
    // Escaped bytes count too; character count alone would incorrectly admit this.
    parameters.insert("keys".into(), serde_json::json!(["a\n".repeat(12_000)]));
    let created = f.edit(vec![group, node("yssbi.numeric.multiply", "number")]);
    f.edit(vec![GraphEditOperation::SetLiteral {
        address: port(&created.created_nodes["number"], "left"),
        literal: Some(serde_json::json!(7)),
    }]);
    let graph = GraphResourceRef::for_path(&f.path);
    let before = f.inspect();
    let summary = query(&mut f, InspectGraphRequest::summary(graph.clone()));
    let overview = summary.overview.unwrap();
    assert_eq!(overview.detail, GraphOverviewDetail::Topology);
    assert_eq!(overview.nodes.len(), before.nodes.len());
    assert_eq!(overview.connections, before.connections);
    assert!(overview.nodes.iter().all(|node| node.parameters.is_none()));
    assert!(overview.literals.is_empty());
    assert!(serde_json::to_vec(&overview).unwrap().len() <= 32 * 1024);
    assert_eq!(
        f.inspect(),
        before,
        "a bounded read must not change configuration"
    );

    f.edit(vec![GraphEditOperation::SetParameters {
        node_id: created.created_nodes["group"].clone(),
        parameters: [("keys".into(), serde_json::json!(["label"]))].into(),
    }]);
    let summary = query(&mut f, InspectGraphRequest::summary(graph.clone()));
    let overview = summary.overview.unwrap();
    assert_eq!(overview.detail, GraphOverviewDetail::Configuration);
    assert_eq!(overview.literals.len(), 1);
    assert_eq!(
        overview.literals[0].address,
        port(&created.created_nodes["number"], "left")
    );
    assert_eq!(overview.literals[0].value, serde_json::json!(7));

    f.edit(
        (0..80)
            .map(|index| {
                let mut operation = node("yssbi.numeric.multiply", &format!("n{index}"));
                let GraphEditOperation::CreateNode { user_label, .. } = &mut operation else {
                    unreachable!()
                };
                *user_label = Some("节点".repeat(50));
                operation
            })
            .collect(),
    );
    let before = f.inspect();
    let summary = query(&mut f, InspectGraphRequest::summary(graph));
    assert_eq!(summary.counts.nodes, 82);
    let overview = summary.overview.unwrap();
    assert_eq!(overview.detail, GraphOverviewDetail::Counts);
    assert!(
        overview.nodes.is_empty()
            && overview.connections.is_empty()
            && overview.literals.is_empty()
    );
    assert!(serde_json::to_vec(&overview).unwrap().len() <= 32 * 1024);
    assert_eq!(f.inspect(), before);
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
    let AutomationCapabilityResult::ResourceInspection(value) = f
        .action(AutomationCapabilityRequest::InspectResource(
            InspectResourceRequest {
                resource: resource.clone(),
            },
        ))
        .unwrap()
    else {
        panic!("resource")
    };
    assert_eq!(value.resource, resource);
    assert!(matches!(value.content, ResourceContent::Metadata));
    assert_eq!(value.version, first.version);
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
            edit: ResourceEdit::RenameColumns {
                columns: vec![model::DatabaseColumnRename {
                    column: "x".into(),
                    name: "renamed_x".into(),
                }],
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
