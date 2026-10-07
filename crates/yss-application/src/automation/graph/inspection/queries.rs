//! Filter against the captured semantic/editor snapshot before projecting a page.
use super::*;

pub(in crate::automation::graph) fn base(
    query: &impl serde::Serialize,
    graph: &GraphResourceRef,
    path: &GraphResourcePath,
    document: &GraphDocument,
    projection: &EditorProjectionModel,
    version: ResourceVersion,
    hash: String,
) -> Result<GraphInspectionPage, CapabilityFailure> {
    let AutomationCapabilityResult::GraphInspectionPage(mut result) = inspect(
        InspectGraphRequest::summary(graph.clone()),
        path,
        document,
        projection,
        version,
        hash,
    )?
    else {
        unreachable!("overview is paged")
    };
    result.observation_hash = yss_canonical_hash::hash_canonical(
        "yssbi.graph-query.v1",
        &(
            query,
            &result.version,
            &result.graph_hash,
            &result.semantic_input_hash,
        ),
    )
    .map(|hash| hex(&hash))
    .map_err(|_| graph_failure(CapabilityFailureCode::InternalFailure))?;
    Ok(result)
}

fn selected_nodes<'a>(
    projection: &'a EditorProjectionModel,
    ids: &[String],
) -> Result<Vec<&'a EditorNodeModel>, CapabilityFailure> {
    let ids = ids
        .iter()
        .map(|id| parse_node_id(id))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if ids
        .iter()
        .any(|id| !projection.nodes.iter().any(|node| node.node_id == *id))
    {
        return Err(invalid_edit_identity("nodeIds"));
    }
    Ok(projection
        .nodes
        .iter()
        .filter(|node| ids.is_empty() || ids.contains(&node.node_id))
        .collect())
}

fn pagination(offset: usize, returned: usize, total: usize) -> GraphInspectionPagination {
    let page = InspectionPage::known(offset, returned, total);
    GraphInspectionPagination {
        offset,
        total,
        next_offset: page.next_offset,
    }
}

pub(in crate::automation::graph) fn find_nodes(
    request: FindNodesRequest,
    path: &GraphResourcePath,
    document: &GraphDocument,
    projection: &EditorProjectionModel,
    version: ResourceVersion,
    hash: String,
) -> Result<AutomationCapabilityResult, CapabilityFailure> {
    let mut result = base(
        &request,
        &request.graph,
        path,
        document,
        projection,
        version,
        hash,
    )?;
    let query = request.query.as_deref().unwrap_or("").to_lowercase();
    let nodes = selected_nodes(projection, &request.node_ids)?
        .into_iter()
        .filter(|node| {
            request.type_ids.is_empty()
                || request
                    .type_ids
                    .iter()
                    .any(|id| id == node.node_type.as_str())
        })
        .filter(|node| {
            node.display.title.to_lowercase().contains(&query)
                || node
                    .display
                    .user_label
                    .as_deref()
                    .is_some_and(|label| label.to_lowercase().contains(&query))
                || node.node_id.to_string().contains(&query)
        })
        .collect::<Vec<_>>();
    let offset = request.offset.min(nodes.len());
    let items = nodes
        .iter()
        .skip(offset)
        .take(request.limit)
        .map(|node| node_summary(node, false, false))
        .collect::<Vec<_>>();
    result.page = Some(pagination(offset, items.len(), nodes.len()));
    result.view = GraphInspectionView::Overview;
    result.content = GraphInspectionItems::Nodes(items);
    Ok(AutomationCapabilityResult::GraphInspectionPage(result))
}

pub(in crate::automation::graph) fn inspect_nodes(
    request: InspectNodesRequest,
    path: &GraphResourcePath,
    document: &GraphDocument,
    projection: &EditorProjectionModel,
    version: ResourceVersion,
    hash: String,
) -> Result<AutomationCapabilityResult, CapabilityFailure> {
    let mut result = base(
        &request,
        &request.graph,
        path,
        document,
        projection,
        version,
        hash,
    )?;
    let has = |field| request.fields.contains(&field);
    let include_ports = has(NodeInspectionField::Ports) || has(NodeInspectionField::Schema);
    let nodes = selected_nodes(projection, &request.node_ids)?;
    let items = nodes
        .iter()
        .map(|node| {
            let offset = request.port_offset.min(node.ports.len());
            let ports = include_ports.then(|| {
                node.ports
                    .iter()
                    .skip(offset)
                    .take(request.port_limit)
                    .map(|port| {
                        let mut detail = port_detail(port, false);
                        if has(NodeInspectionField::Schema) {
                            detail.schema_known = Some(port.resolved_schema.is_some());
                            if let Some(schema) = &port.resolved_schema {
                                let offset = request.column_offset.min(schema.fields.len());
                                let fields: BTreeMap<_, _> = schema
                                    .fields
                                    .iter()
                                    .skip(offset)
                                    .take(request.column_limit)
                                    .map(|field| {
                                        (field.name.to_string(), format!("{:?}", field.scalar_type))
                                    })
                                    .collect();
                                detail.schema_page = Some(InspectionPage::known(
                                    offset,
                                    fields.len(),
                                    schema.fields.len(),
                                ));
                                detail.schema = Some(fields);
                            }
                        }
                        detail
                    })
                    .collect::<Vec<_>>()
            });
            GraphNodeDetails {
                node: node_summary(
                    node,
                    has(NodeInspectionField::Parameters),
                    has(NodeInspectionField::Options),
                ),
                port_page: ports
                    .as_ref()
                    .map(|ports| InspectionPage::known(offset, ports.len(), node.ports.len())),
                ports,
            }
        })
        .collect::<Vec<_>>();
    result.view = GraphInspectionView::Nodes;
    result.page = Some(pagination(0, items.len(), items.len()));
    result.content = GraphInspectionItems::NodeDetails(items);
    Ok(AutomationCapabilityResult::GraphInspectionPage(result))
}

pub(in crate::automation::graph) fn find_connections(
    request: FindConnectionsRequest,
    path: &GraphResourcePath,
    document: &GraphDocument,
    projection: &EditorProjectionModel,
    version: ResourceVersion,
    hash: String,
) -> Result<AutomationCapabilityResult, CapabilityFailure> {
    let mut result = base(
        &request,
        &request.graph,
        path,
        document,
        projection,
        version,
        hash,
    )?;
    let nodes = selected_nodes(projection, &request.node_ids)?
        .iter()
        .map(|node| node.node_id)
        .collect::<BTreeSet<_>>();
    let ports = request
        .ports
        .into_iter()
        .map(parse_edit_port)
        .collect::<Result<BTreeSet<_>, _>>()?;
    if ports.iter().any(|address| {
        !projection
            .nodes
            .iter()
            .any(|node| node.ports.iter().any(|port| port.address == *address))
    }) {
        return Err(invalid_edit_identity("ports"));
    }
    let connections = projection
        .connections
        .iter()
        .filter(|connection| {
            (nodes.contains(&connection.input.node_id)
                || nodes.contains(&connection.output.node_id))
                && (ports.is_empty()
                    || ports.contains(&connection.input)
                    || ports.contains(&connection.output))
        })
        .collect::<Vec<_>>();
    let offset = request.offset.min(connections.len());
    let items = connections
        .iter()
        .skip(offset)
        .take(request.limit)
        .map(|connection| GraphConnectionInspection {
            connection_id: connection.connection_id.to_string(),
            output: inspect_port(&connection.output),
            input: inspect_port(&connection.input),
            order: connection.order.as_deref().map(str::to_owned),
        })
        .collect::<Vec<_>>();
    result.view = GraphInspectionView::Connections;
    result.page = Some(pagination(offset, items.len(), connections.len()));
    result.content = GraphInspectionItems::Connections(items);
    Ok(AutomationCapabilityResult::GraphInspectionPage(result))
}
