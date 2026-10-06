//! Query projections over the captured editor snapshot; no separate graph state or cache.
use super::*;
mod overview;
pub(super) mod queries;
pub(super) use queries::{find_connections, find_nodes, inspect_nodes};

pub(super) fn inspect(
    request: InspectGraphRequest,
    path: &GraphResourcePath,
    document: &GraphDocument,
    projection: &EditorProjectionModel,
    version: ResourceVersion,
    hash: String,
) -> Result<AutomationCapabilityResult, CapabilityFailure> {
    validate_graph_reference(&request.graph, path)?;
    if request.view == GraphInspectionView::Full {
        return inspect_projection(path, document, projection, version, hash)
            .map(AutomationCapabilityResult::GraphInspection);
    }
    let node_ids = request
        .node_ids
        .iter()
        .map(|id| parse_node_id(id))
        .collect::<Result<BTreeSet<_>, _>>()?;
    if node_ids
        .iter()
        .any(|id| !projection.nodes.iter().any(|node| &node.node_id == id))
    {
        return Err(invalid_edit_identity("nodeIds"));
    }
    let addresses = request
        .port_addresses
        .iter()
        .cloned()
        .map(parse_edit_port)
        .collect::<Result<BTreeSet<_>, _>>()?;
    let nodes = projection
        .nodes
        .iter()
        .filter(|node| node_ids.is_empty() || node_ids.contains(&node.node_id))
        .collect::<Vec<_>>();
    if addresses.iter().any(|address| {
        !nodes
            .iter()
            .any(|node| node.ports.iter().any(|port| &port.address == address))
    }) {
        return Err(invalid_edit_identity("portAddresses"));
    }
    let semantic_input_hash = hex(&projection.basis.semantic_input_hash);
    let mut identity_query = request.clone();
    identity_query.if_unchanged = None;
    let observation_hash = yss_canonical_hash::hash_canonical(
        "yssbi.graph-inspection.v1",
        &(&identity_query, &version, &hash, &semantic_input_hash),
    )
    .map(|value| hex(&value))
    .map_err(|_| graph_failure(CapabilityFailureCode::InternalFailure))?;
    let diagnostics = diagnostics(projection);
    let counts = GraphInspectionCounts {
        nodes: projection.nodes.len(),
        ports: projection.nodes.iter().map(|node| node.ports.len()).sum(),
        connections: projection.connections.len(),
        constants: document.constants.len(),
        diagnostics: diagnostics.len(),
        blocking_diagnostics: diagnostics.iter().filter(|item| item.blocking).count(),
    };
    let mut result = GraphInspectionPage {
        graph_path: path.as_str().into(),
        version,
        graph_hash: hash,
        semantic_input_hash,
        ready: matches!(projection.outcome, EditorResolutionOutcome::Complete)
            && counts.blocking_diagnostics == 0,
        counts,
        view: request.view,
        observation_hash,
        page: None,
        content: GraphInspectionItems::Unchanged,
        runs: None,
        overview: None,
    };
    if request.if_unchanged.as_ref() == Some(&result.observation_hash) {
        return Ok(AutomationCapabilityResult::GraphInspectionPage(result));
    }
    if request.view == GraphInspectionView::Summary {
        result.content = GraphInspectionItems::Summary;
        result.overview = Some(overview::project(document, projection));
        return Ok(AutomationCapabilityResult::GraphInspectionPage(result));
    }
    let (total, content) = match request.view {
        GraphInspectionView::Overview | GraphInspectionView::Nodes => {
            let items = nodes
                .iter()
                .skip(request.offset)
                .take(request.limit)
                .map(|node| {
                    let details = request.view == GraphInspectionView::Nodes;
                    node_summary(node, details, request.include_options)
                })
                .collect();
            (nodes.len(), GraphInspectionItems::Nodes(items))
        }
        GraphInspectionView::Ports => {
            let ports = nodes
                .iter()
                .flat_map(|node| &node.ports)
                .filter(|port| addresses.is_empty() || addresses.contains(&port.address))
                .collect::<Vec<_>>();
            let items = ports
                .iter()
                .skip(request.offset)
                .take(request.limit)
                .map(|port| port_detail(port, request.include_schema))
                .collect();
            (ports.len(), GraphInspectionItems::Ports(items))
        }
        GraphInspectionView::Connections => {
            let connections = projection
                .connections
                .iter()
                .filter(|connection| {
                    node_ids.is_empty()
                        || node_ids.contains(&connection.output.node_id)
                        || node_ids.contains(&connection.input.node_id)
                })
                .collect::<Vec<_>>();
            let items = connections
                .iter()
                .skip(request.offset)
                .take(request.limit)
                .map(|connection| GraphConnectionInspection {
                    connection_id: connection.connection_id.to_string(),
                    output: inspect_port(&connection.output),
                    input: inspect_port(&connection.input),
                    order: connection.order.as_deref().map(str::to_owned),
                })
                .collect();
            (connections.len(), GraphInspectionItems::Connections(items))
        }
        GraphInspectionView::Diagnostics => (
            diagnostics.len(),
            GraphInspectionItems::Diagnostics(
                diagnostics
                    .into_iter()
                    .skip(request.offset)
                    .take(request.limit)
                    .collect(),
            ),
        ),
        GraphInspectionView::Constants => (
            document.constants.len(),
            GraphInspectionItems::Constants(constants(document, request.offset, request.limit)?),
        ),
        GraphInspectionView::Summary | GraphInspectionView::Full => {
            unreachable!("inspection returned above")
        }
    };
    let end = request.offset.saturating_add(request.limit).min(total);
    result.page = Some(GraphInspectionPagination {
        offset: request.offset,
        total,
        next_offset: (end < total).then_some(end),
    });
    result.content = content;
    Ok(AutomationCapabilityResult::GraphInspectionPage(result))
}

pub(super) fn validate_graph_reference(
    graph: &GraphResourceRef,
    path: &GraphResourcePath,
) -> Result<(), CapabilityFailure> {
    let expected = match graph.kind {
        GraphResourceKind::EventGraph => yss_graph_document::GraphResourceKind::EventGraph,
        GraphResourceKind::FunctionGraph => yss_graph_document::GraphResourceKind::FunctionGraph,
    };
    if path.kind() == expected {
        Ok(())
    } else {
        Err(invalid_edit_identity("graph"))
    }
}

fn port_detail(port: &EditorPortModel, include_schema: bool) -> GraphPortDetail {
    GraphPortDetail {
        address: edit_port(&port.address),
        label: port
            .display
            .instance_label
            .as_deref()
            .unwrap_or(&port.display.label)
            .into(),
        direction: format!("{:?}", port.direction).to_lowercase(),
        orphan: port.orphan,
        data_type: match &port.type_state {
            EditorPortTypeState::Exact { display, .. }
            | EditorPortTypeState::Constrained { display, .. } => display.to_string(),
            EditorPortTypeState::Unknown { .. } => "unknown".into(),
            EditorPortTypeState::Conflict { .. } => "conflict".into(),
        },
        accepted_type: port.accepted_type.to_string(),
        maximum_connections: port.connections.maximum,
        connection_count: port.connections.current,
        schema_known: include_schema.then_some(port.resolved_schema.is_some()),
        schema: include_schema.then(|| {
            port.resolved_schema
                .as_ref()
                .map(|schema| {
                    schema
                        .fields
                        .iter()
                        .map(|field| (field.name.to_string(), format!("{:?}", field.scalar_type)))
                        .collect()
                })
                .unwrap_or_default()
        }),
        schema_page: None,
        literal: port.input.as_ref().and_then(|input| {
            input
                .literal_override
                .clone()
                .or_else(|| input.protocol_default.clone())
        }),
    }
}

pub(super) fn port_facts(port: &EditorPortModel) -> GraphPortFacts {
    let value = port_detail(port, true);
    GraphPortFacts {
        address: value.address,
        label: value.label,
        direction: value.direction,
        data_type: value.data_type,
        accepted_type: value.accepted_type,
        orphan: value.orphan,
        maximum_connections: value.maximum_connections,
        connection_count: value.connection_count,
        schema: value.schema.expect("schema requested"),
        literal: value.literal,
    }
}

pub(super) fn port_templates(node: &EditorNodeModel) -> Vec<GraphPortTemplateInspection> {
    node.port_instance_additions
        .iter()
        .map(|template| GraphPortTemplateInspection {
            key: template.template_key.as_str().into(),
            direction: format!("{:?}", template.direction).to_lowercase(),
            can_add: template.can_add,
        })
        .collect()
}

pub(super) fn constants(
    document: &GraphDocument,
    offset: usize,
    limit: usize,
) -> Result<BTreeMap<String, serde_json::Value>, CapabilityFailure> {
    document.constants.iter().skip(offset).take(limit).map(|(id, constant)| {
        let mut value = serde_json::json!({ "id": id, "name": constant.name, "dataType": constant.data_type, "description": constant.description, "tags": constant.tags, "hasTabularData": constant.tabular.is_some() });
        let content_hash = yss_canonical_hash::hash_canonical("yssbi.assistant.graph-constant.v1", constant)
            .map_err(|_| graph_failure(CapabilityFailureCode::InternalFailure))?;
        value["contentHash"] = serde_json::json!(hex(&content_hash));
        let primitive = match &constant.data_value { yss_data_contract::DataValue::Bool(_) | yss_data_contract::DataValue::Integer(_) | yss_data_contract::DataValue::Decimal(_) | yss_data_contract::DataValue::Null => true, yss_data_contract::DataValue::String(value) => value.len() <= 4096, _ => false };
        value["valueIncluded"] = serde_json::json!(primitive);
        if primitive { value["dataValue"] = serde_json::to_value(&constant.data_value).map_err(|_| graph_failure(CapabilityFailureCode::InternalFailure))?; }
        Ok((id.to_string(), value))
    }).collect()
}

fn node_summary(
    node: &EditorNodeModel,
    include_parameters: bool,
    include_options: bool,
) -> GraphNodeSummary {
    GraphNodeSummary {
        node_id: node.node_id.to_string(),
        node_type_id: node.node_type.as_str().into(),
        user_label: node.display.user_label.as_deref().map(str::to_owned),
        title: node.display.title.to_string(),
        port_count: node.ports.len(),
        parameters: include_parameters.then(|| {
            node.parameter_groups
                .iter()
                .flat_map(|group| &group.parameters)
                .map(|parameter| (parameter.key.as_str().into(), parameter.value.clone()))
                .collect()
        }),
        parameter_options: include_options.then(|| {
            node.parameter_groups
                .iter()
                .flat_map(|group| &group.parameters)
                .map(super::parameter)
                .collect()
        }),
        port_templates: include_parameters.then(|| port_templates(node)),
    }
}
