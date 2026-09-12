use super::{
    enforce_result_bound, ensure_project_binding, inspect_port, inspect_result_category,
    invalid_request, map_catalog_error, map_session_capture_error, map_session_revalidation_error,
};
use crate::catalog_query::{LocalizedCatalogRequest, localized_node_catalog_in_session};
use crate::editor_projection::*;
use crate::events::GraphProjectionReplacement;
use crate::execution::run_graph::{
    RunApplicationEvent, RunApplicationEventKind, RunGraphRequest, run_graph_with_sink,
};
use crate::execution::{ApplicationSession, ApplicationState};
use crate::graph_compile::{CompileGraphDraftReceipt, compile_graph_draft};
use crate::resource_mutation::build_catalog_mutation_validation_snapshot;
use crate::resource_mutation::{GraphDraftSave, GraphDraftTransform};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use yss_automation_contract::*;
use yss_graph_catalog::LocalizedCatalogItem;
use yss_graph_document::GraphDocument;
use yss_graph_document::{
    ConnectionId, GraphResourcePath, NodeId, NodePosition, OrderKey, PortAddress, PortInstanceId,
    PortRef,
};
use yss_graph_document_edit::apply_graph_document_patch;
use yss_graph_editor::{EditorGraphMutation, NodePositionMutation};
use yss_graph_protocol::PortKey;
use yss_project_identity::OperationId;

pub enum AutomationGraphUpdate {
    None,
    Execution {
        terminal_event_sent: bool,
        status: String,
    },
    Draft(GraphDraftTransform),
    Compilation(CompileGraphDraftReceipt),
    Saved(GraphDraftSave),
}

pub struct AutomationGraphAction {
    pub result: AutomationCapabilityResult,
    pub update: AutomationGraphUpdate,
}

pub struct AutomationGraphDraft {
    pub document: GraphDocument,
    pub generation: u64,
    pub locale: String,
}

pub fn prepare_automation_graph_action(
    application: &ApplicationState,
    context: CapabilityInvocationContext,
    request: AutomationCapabilityRequest,
    draft: AutomationGraphDraft,
    control: &CapabilityControl,
    mut deliver: impl FnMut(RunApplicationEvent) -> bool + Send,
) -> Result<AutomationGraphAction, CapabilityFailure> {
    let AutomationGraphDraft {
        document,
        generation: draft_generation,
        locale,
    } = draft;
    control.check()?;
    request
        .validate()
        .map_err(|error| invalid_request(request.capability_id(), error))?;
    let captured = application
        .capture_session()
        .map_err(map_session_capture_error)?;
    ensure_project_binding(&captured, &context)?;
    yss_graph_document_edit::validate_graph_document(&document)
        .map_err(|_| graph_failure(CapabilityFailureCode::InvalidRequest))?;
    let graph_path_text = graph_action_path(&request)
        .ok_or_else(|| graph_failure(CapabilityFailureCode::InvalidRequest))?;
    let path = GraphResourcePath::new(graph_path_text)
        .map_err(|_| graph_failure(CapabilityFailureCode::InvalidRequest))?;
    if !captured
        .project()
        .get_data()
        .map_err(|_| graph_failure(CapabilityFailureCode::GraphUnavailable))?
        .graphs
        .contains_key(&path)
    {
        return Err(graph_failure(CapabilityFailureCode::GraphUnavailable));
    }
    let hash = graph_hash(&document)?;
    let expected = match &request {
        AutomationCapabilityRequest::ApplyGraphEdit(request) => Some(&request.graph_hash),
        AutomationCapabilityRequest::CompileGraph(request) => Some(&request.graph_hash),
        AutomationCapabilityRequest::ExecuteGraph(request) => Some(&request.graph_hash),
        AutomationCapabilityRequest::SaveGraph(request) => Some(&request.graph_hash),
        _ => None,
    };
    if expected.is_some_and(|expected| expected != &hash) {
        return Err(graph_failure(CapabilityFailureCode::GraphDraftChanged));
    }
    let action = match request {
        AutomationCapabilityRequest::InspectGraph(_) => {
            let projection = application
                .resolve_graph_draft(
                    captured.project_instance_id().clone(),
                    path.clone(),
                    document.clone(),
                    locale,
                )
                .map_err(map_graph_error)?;
            AutomationGraphAction {
                result: AutomationCapabilityResult::GraphInspection(inspect_projection(
                    &path,
                    &document,
                    &projection,
                    draft_generation,
                    "draft",
                )?),
                update: AutomationGraphUpdate::None,
            }
        }
        AutomationCapabilityRequest::ApplyGraphEdit(mut request) => {
            request.locale = locale;
            if request.base_revision != draft_generation {
                return Err(graph_failure(CapabilityFailureCode::GraphDraftChanged));
            }
            let (transform, receipt) =
                transform_graph_edit(application, &captured, request, document, control)?;
            AutomationGraphAction {
                result: AutomationCapabilityResult::GraphEditReceipt(receipt),
                update: AutomationGraphUpdate::Draft(transform),
            }
        }
        AutomationCapabilityRequest::CompileGraph(_) => {
            let receipt = compile_graph_draft(
                application,
                captured.project_instance_id().clone(),
                path.clone(),
                document,
                &locale,
            )
            .map_err(|_| graph_failure(CapabilityFailureCode::GraphCompileFailed))?;
            let (artifact_id, projection) = match &receipt {
                CompileGraphDraftReceipt::Ready {
                    artifact_id,
                    projection,
                    ..
                } => (Some(hex(artifact_id)), projection),
                CompileGraphDraftReceipt::Blocked { projection } => (None, projection),
            };
            AutomationGraphAction {
                result: AutomationCapabilityResult::GraphCompilation(GraphCompilation {
                    graph_path: path.as_str().into(),
                    graph_hash: hash,
                    ready: artifact_id.is_some(),
                    artifact_id,
                    diagnostics: diagnostics(projection),
                }),
                update: AutomationGraphUpdate::Compilation(receipt),
            }
        }
        AutomationCapabilityRequest::SaveGraph(_) => {
            let saved = application
                .save_graph_draft(
                    captured.project_instance_id().clone(),
                    path.clone(),
                    locale,
                    OperationId::new(),
                    document,
                )
                .map_err(map_graph_error)?;
            AutomationGraphAction {
                result: AutomationCapabilityResult::GraphSaved(GraphSaved {
                    graph_path: path.as_str().into(),
                    graph_hash: graph_hash(&saved.document)?,
                    resource_revision: saved.resource_revision.get(),
                }),
                update: AutomationGraphUpdate::Saved(saved),
            }
        }
        AutomationCapabilityRequest::ExecuteGraph(request) => {
            let artifact_id = parse_hash(&request.artifact_id)?;
            let projection = application
                .resolve_graph_draft(
                    captured.project_instance_id().clone(),
                    path.clone(),
                    document,
                    locale,
                )
                .map_err(map_graph_error)?;
            if projection.basis.semantic_input_hash != artifact_id
                || captured
                    .graph()
                    .compiled_draft(&path, &artifact_id)
                    .is_none()
            {
                return Err(graph_failure(CapabilityFailureCode::GraphDraftChanged));
            }
            let mut run_id = None;
            let mut failure_code = None;
            let mut failure_location = None;
            let mut status = "failed";
            let mut terminal_event_sent = false;
            let outcome = run_graph_with_sink(
                application,
                RunGraphRequest::new(
                    captured.project_instance_id().clone(),
                    path.clone(),
                    artifact_id,
                )
                .with_cancellation(control.cancellation_flag())
                .with_deadline(control.deadline()),
                |event| {
                    run_id = Some(event.identity().run_id().get());
                    match event.kind() {
                        RunApplicationEventKind::RunCompleted => status = "succeeded",
                        RunApplicationEventKind::RunCancelled => status = "cancelled",
                        RunApplicationEventKind::RunErrored { failure } => {
                            failure_code = Some(format!("{:?}", failure.code));
                            failure_location =
                                Some(format!("{:?}: {:?}", failure.phase, failure.source));
                        }
                        _ => {}
                    }
                    let terminal = matches!(
                        event.kind(),
                        RunApplicationEventKind::RunCompleted
                            | RunApplicationEventKind::RunCancelled
                            | RunApplicationEventKind::RunErrored { .. }
                    );
                    let delivered = deliver(event);
                    terminal_event_sent |= terminal && delivered;
                    delivered
                },
            );
            if outcome.is_err() && failure_code.is_none() && status != "cancelled" {
                failure_code = Some("graph_execution_failed".into());
            }
            let results = list_graph_results(&captured, path.as_str().into())?
                .results
                .into_iter()
                .filter(|result| Some(result.run_id) == run_id && status == "succeeded")
                .collect();
            AutomationGraphAction {
                result: AutomationCapabilityResult::GraphExecution(GraphExecution {
                    graph_path: path.as_str().into(),
                    graph_hash: hash,
                    artifact_id: request.artifact_id,
                    run_id,
                    status: status.into(),
                    failure_code,
                    failure_location,
                    results,
                }),
                update: AutomationGraphUpdate::Execution {
                    terminal_event_sent,
                    status: status.into(),
                },
            }
        }
        _ => return Err(graph_failure(CapabilityFailureCode::InvalidRequest)),
    };
    application
        .revalidate_captured_session(&captured)
        .map_err(map_session_revalidation_error)?;
    // Save has crossed its commit point; the actual receipt survives late cancellation.
    if !matches!(
        action.update,
        AutomationGraphUpdate::Saved(_) | AutomationGraphUpdate::Execution { .. }
    ) {
        control.check()?;
    }
    Ok(action)
}

pub(crate) fn graph_action_path(request: &AutomationCapabilityRequest) -> Option<&str> {
    match request {
        AutomationCapabilityRequest::InspectGraph(r) => Some(&r.graph_path),
        AutomationCapabilityRequest::ApplyGraphEdit(r) => Some(&r.graph_path),
        AutomationCapabilityRequest::CompileGraph(r) => Some(&r.graph_path),
        AutomationCapabilityRequest::ExecuteGraph(r) => Some(&r.graph_path),
        AutomationCapabilityRequest::SaveGraph(r) => Some(&r.graph_path),
        _ => None,
    }
}

fn transform_graph_edit(
    application: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    request: ApplyGraphEditRequest,
    original: GraphDocument,
    control: &CapabilityControl,
) -> Result<(GraphDraftTransform, GraphEditReceipt), CapabilityFailure> {
    let graph_path = GraphResourcePath::new(&request.graph_path)
        .map_err(|_| graph_failure(CapabilityFailureCode::InvalidRequest))?;
    let localized = localized_node_catalog_in_session(
        application,
        captured,
        LocalizedCatalogRequest::new(
            captured.project_instance_id().clone(),
            request.locale.clone(),
        ),
    )
    .map_err(map_catalog_error)?
    .into_transport_parts()
    .into_fields()
    .3;
    let catalog = build_catalog_mutation_validation_snapshot(captured).map_err(map_graph_error)?;
    let mut resolution =
        crate::resource_mutation::DraftResolutionContext::capture(captured, &original)
            .map_err(map_graph_error)?;
    let mut staged = original.clone();
    let mut created_nodes = BTreeMap::new();
    let mut created_ports = BTreeMap::new();
    for mut operation in request.operations {
        control.check()?;
        let alias = match &operation {
            GraphEditOperation::CreateNode { client_id, .. }
            | GraphEditOperation::CreateConstant { client_id, .. }
            | GraphEditOperation::InsertConstantReference { client_id, .. }
            | GraphEditOperation::AddPortInstance { client_id, .. } => client_id.clone(),
            _ => None,
        };
        if alias.as_ref().is_some_and(|alias| {
            alias.is_empty()
                || alias.len() > 64
                || created_nodes.contains_key(alias)
                || created_ports.contains_key(alias)
        }) {
            return Err(invalid_edit_identity("clientId"));
        }
        resolve_aliases(&mut operation, &created_nodes, &created_ports)?;
        if let GraphEditOperation::CreateConstant {
            name,
            value,
            x,
            y,
            client_id,
        } = operation
        {
            use yss_data_contract::{DataType, DataValue};
            let (data_type, data_value) = match value {
                GraphConstantLiteral::Boolean(value) => {
                    (DataType::Boolean, DataValue::Boolean(value))
                }
                GraphConstantLiteral::Integer(value) => (DataType::Int64, DataValue::Int64(value)),
                GraphConstantLiteral::Decimal(value) => {
                    (DataType::Float64, DataValue::Float64(value))
                }
                GraphConstantLiteral::String(value) => (DataType::String, DataValue::String(value)),
            };
            let id = yss_graph_document::ConstantId::new();
            let mutation = EditorGraphMutation::SetConstant {
                id,
                constant: Some(yss_graph_document::GraphConstant {
                    id,
                    name,
                    data_type,
                    data_value,
                    tabular: None,
                    description: String::new(),
                    tags: Vec::new(),
                }),
            };
            let analysis = resolution.resolve(captured, &graph_path, &staged, &request.locale);
            let patch = captured
                .graph()
                .plan_editor_mutation(
                    &graph_path,
                    &staged,
                    mutation,
                    &catalog,
                    analysis.semantic_snapshot(),
                )
                .map_err(|error| {
                    graph_failure(CapabilityFailureCode::MutationRejected)
                        .with_detail("reason", error.code())
                })?;
            apply_graph_document_patch(&mut staged, &patch)
                .map_err(|_| graph_failure(CapabilityFailureCode::MutationRejected))?;
            operation = GraphEditOperation::InsertConstantReference {
                id: id.to_string(),
                x,
                y,
                client_id,
            };
        }
        if let GraphEditOperation::SetParameters {
            node_id,
            parameters,
        } = &mut operation
        {
            let existing = staged
                .nodes
                .get(&parse_node_id(node_id)?)
                .ok_or_else(|| invalid_edit_identity("nodeId"))?;
            let mut merged = existing
                .parameters
                .iter()
                .map(|(key, value)| (key.as_str().to_owned(), value.clone()))
                .collect::<BTreeMap<_, _>>();
            merged.append(parameters);
            *parameters = merged;
        }
        let adds_port = matches!(operation, GraphEditOperation::AddPortInstance { .. });
        let mutation = editor_mutation(operation, &localized.items)?;
        let before_nodes = staged.nodes.keys().copied().collect::<BTreeSet<_>>();
        let before_ports = staged
            .port_bindings
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        let analysis = resolution.resolve(captured, &graph_path, &staged, &request.locale);
        let patch = captured
            .graph()
            .plan_editor_mutation(
                &graph_path,
                &staged,
                mutation,
                &catalog,
                analysis.semantic_snapshot(),
            )
            .map_err(|error| {
                graph_failure(CapabilityFailureCode::MutationRejected)
                    .with_detail("reason", error.code())
            })?;
        apply_graph_document_patch(&mut staged, &patch)
            .map_err(|_| graph_failure(CapabilityFailureCode::MutationRejected))?;
        if let Some(alias) = alias {
            if adds_port {
                let address = staged
                    .port_bindings
                    .keys()
                    .find(|address| !before_ports.contains(*address))
                    .ok_or_else(|| graph_failure(CapabilityFailureCode::MutationRejected))?;
                created_ports.insert(alias, edit_port(address));
            } else {
                let id = staged
                    .nodes
                    .keys()
                    .find(|id| !before_nodes.contains(id))
                    .ok_or_else(|| graph_failure(CapabilityFailureCode::MutationRejected))?;
                created_nodes.insert(alias, id.to_string());
            }
        }
        // A newly inserted function node may introduce additional resource dependencies.
        resolution
            .include_functions(captured, &staged)
            .map_err(map_graph_error)?;
    }
    resolution.revalidate(captured).map_err(map_graph_error)?;
    let changed = staged != original;
    created_nodes.retain(|_, id| parse_node_id(id).is_ok_and(|id| staged.nodes.contains_key(&id)));
    created_ports.retain(|_, port| {
        parse_edit_port(port.clone())
            .is_ok_and(|address| staged.port_bindings.contains_key(&address))
    });
    let projection = application
        .resolve_graph_draft(
            captured.project_instance_id().clone(),
            graph_path.clone(),
            staged.clone(),
            request.locale,
        )
        .map_err(map_graph_error)?;
    let graph_hash = graph_hash(&staged)?;
    let receipt = GraphEditReceipt {
        graph_path: request.graph_path,
        from_revision: request.base_revision,
        to_revision: request
            .base_revision
            .checked_add(1)
            .ok_or_else(|| invalid_edit_identity("baseRevision"))?,
        client_key: request.client_key,
        graph_hash,
        created_nodes,
        created_ports,
    };
    Ok((
        GraphDraftTransform {
            changed,
            document: staged,
            projection_replacement: GraphProjectionReplacement {
                graph_path: graph_path.as_str().into(),
                projection,
                function_editor_projection: None,
            },
        },
        receipt,
    ))
}

pub(super) fn editor_mutation(
    operation: GraphEditOperation,
    catalog: &[LocalizedCatalogItem],
) -> Result<EditorGraphMutation, CapabilityFailure> {
    match operation {
        GraphEditOperation::CreateConstant { .. } => Err(invalid_edit_identity("operation")),
        GraphEditOperation::SetParameters {
            node_id,
            parameters,
        } => Ok(EditorGraphMutation::SetParameters {
            node_id: parse_node_id(&node_id)?,
            parameters: parameters
                .into_iter()
                .map(|(key, value)| {
                    Ok((
                        yss_graph_protocol::ParameterKey::new(key)
                            .map_err(|_| invalid_edit_identity("parameterKey"))?,
                        value,
                    ))
                })
                .collect::<Result<_, CapabilityFailure>>()?,
        }),
        GraphEditOperation::SetConfiguration {
            node_id,
            key,
            values,
        } => Ok(EditorGraphMutation::SetConfiguration {
            node_id: parse_node_id(&node_id)?,
            key: yss_graph_protocol::ParameterKey::new(key)
                .map_err(|_| invalid_edit_identity("parameterKey"))?,
            values: values
                .into_iter()
                .map(|(key, value)| {
                    Ok((
                        yss_graph_protocol::ParameterKey::new(key)
                            .map_err(|_| invalid_edit_identity("parameterKey"))?,
                        value,
                    ))
                })
                .collect::<Result<_, CapabilityFailure>>()?,
        }),
        GraphEditOperation::SetLiteral { address, literal } => {
            Ok(EditorGraphMutation::SetLiteral {
                address: parse_edit_port(address)?,
                literal,
            })
        }
        GraphEditOperation::AddPortInstance {
            node_id,
            template_key,
            ..
        } => Ok(EditorGraphMutation::AddPortInstance {
            node_id: parse_node_id(&node_id)?,
            template_key: PortKey::new(template_key)
                .map_err(|_| invalid_edit_identity("templateKey"))?,
            placement: yss_graph_editor::PortPlacement::Append,
        }),
        GraphEditOperation::RemovePortInstance { address } => {
            Ok(EditorGraphMutation::RemovePortInstance {
                address: parse_edit_port(address)?,
            })
        }
        GraphEditOperation::DisconnectPort { address } => Ok(EditorGraphMutation::DisconnectPort {
            address: parse_edit_port(address)?,
        }),
        GraphEditOperation::DisconnectNode { node_id } => Ok(EditorGraphMutation::DisconnectNode {
            node_id: parse_node_id(&node_id)?,
        }),
        GraphEditOperation::MoveConnections { source, target } => {
            Ok(EditorGraphMutation::MoveConnections {
                source: parse_edit_port(source)?,
                target: parse_edit_port(target)?,
            })
        }
        GraphEditOperation::DuplicateNodes {
            node_ids,
            offset_x,
            offset_y,
        } => Ok(EditorGraphMutation::DuplicateSubgraph {
            node_ids: node_ids
                .iter()
                .map(|id| parse_node_id(id))
                .collect::<Result<_, _>>()?,
            offset: NodePosition {
                x: offset_x,
                y: offset_y,
            },
        }),
        GraphEditOperation::SetConstant { id, constant } => Ok(EditorGraphMutation::SetConstant {
            id: uuid::Uuid::parse_str(&id)
                .map(yss_graph_document::ConstantId::from_uuid)
                .map_err(|_| invalid_edit_identity("constantId"))?,
            constant: constant
                .map(serde_json::from_value)
                .transpose()
                .map_err(|_| invalid_edit_identity("constant"))?,
        }),
        GraphEditOperation::InsertConstantReference { id, x, y, .. } => {
            Ok(EditorGraphMutation::InsertConstantReference {
                id: uuid::Uuid::parse_str(&id)
                    .map(yss_graph_document::ConstantId::from_uuid)
                    .map_err(|_| invalid_edit_identity("constantId"))?,
                position: NodePosition { x, y },
            })
        }
        GraphEditOperation::CreateNode {
            client_id: _,
            node_type_id,
            resource_path,
            x,
            y,
            user_label,
        } => {
            let descriptor = catalog
                .iter()
                .find(|item| {
                    item.node_type_id.as_ref() == node_type_id
                        && item.resource_path.as_ref().map(|path| path.as_str())
                            == resource_path.as_deref()
                })
                .map(|item| item.creation.clone())
                .ok_or_else(|| {
                    CapabilityFailure::new(CapabilityFailureCode::MutationRejected)
                        .with_detail("nodeTypeId", node_type_id)
                })?;
            Ok(EditorGraphMutation::CreateNode {
                descriptor,
                position: NodePosition { x, y },
                user_label,
                connect_from: None,
            })
        }
        GraphEditOperation::MoveNodes { positions } => Ok(EditorGraphMutation::MoveNodes {
            positions: positions
                .into_iter()
                .map(|position| {
                    Ok(NodePositionMutation {
                        node_id: parse_node_id(&position.node_id)?,
                        position: NodePosition {
                            x: position.x,
                            y: position.y,
                        },
                    })
                })
                .collect::<Result<Vec<_>, CapabilityFailure>>()?,
        }),
        GraphEditOperation::DeleteNodes { node_ids } => Ok(EditorGraphMutation::DeleteNodes {
            node_ids: node_ids
                .iter()
                .map(|node_id| parse_node_id(node_id))
                .collect::<Result<Vec<_>, _>>()?,
        }),
        GraphEditOperation::Connect {
            output,
            input,
            order,
        } => Ok(EditorGraphMutation::Connect {
            output: parse_edit_port(output)?,
            input: parse_edit_port(input)?,
            order: order.map(OrderKey::new),
        }),
        GraphEditOperation::DisconnectConnections { connection_ids } => {
            Ok(EditorGraphMutation::DisconnectConnections {
                connection_ids: connection_ids
                    .iter()
                    .map(|id| {
                        uuid::Uuid::parse_str(id)
                            .map(ConnectionId::from_uuid)
                            .map_err(|_| invalid_edit_identity("connectionId"))
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            })
        }
    }
}

fn parse_node_id(value: &str) -> Result<NodeId, CapabilityFailure> {
    uuid::Uuid::parse_str(value)
        .map(NodeId::from_uuid)
        .map_err(|_| invalid_edit_identity("nodeId"))
}

fn parse_edit_port(value: GraphEditPortRef) -> Result<PortAddress, CapabilityFailure> {
    match value {
        GraphEditPortRef::Declared { node_id, port_key } => Ok(PortAddress::declared(
            parse_node_id(&node_id)?,
            PortKey::new(port_key).map_err(|_| invalid_edit_identity("portKey"))?,
        )),
        GraphEditPortRef::Instance {
            node_id,
            template_key,
            instance_id,
        } => Ok(PortAddress::instance(
            parse_node_id(&node_id)?,
            PortKey::new(template_key).map_err(|_| invalid_edit_identity("templateKey"))?,
            uuid::Uuid::parse_str(&instance_id)
                .map(PortInstanceId::from_uuid)
                .map_err(|_| invalid_edit_identity("instanceId"))?,
        )),
    }
}

fn invalid_edit_identity(field: &'static str) -> CapabilityFailure {
    CapabilityFailure::new(CapabilityFailureCode::InvalidRequest).with_detail("field", field)
}

pub(crate) fn inspect_saved_graph(
    application: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    request: InspectGraphRequest,
) -> Result<GraphInspection, CapabilityFailure> {
    let path = GraphResourcePath::new(request.graph_path)
        .map_err(|_| graph_failure(CapabilityFailureCode::InvalidRequest))?;
    let data = captured
        .project()
        .get_data()
        .map_err(|_| graph_failure(CapabilityFailureCode::GraphUnavailable))?;
    let document = &data
        .graphs
        .get(&path)
        .ok_or_else(|| graph_failure(CapabilityFailureCode::GraphUnavailable))?
        .document;
    let projection = application
        .resolve_graph_draft(
            captured.project_instance_id().clone(),
            path.clone(),
            document.clone(),
            "en-US".into(),
        )
        .map_err(map_graph_error)?;
    inspect_projection(&path, document, &projection, 0, "saved")
}

fn inspect_projection(
    path: &GraphResourcePath,
    document: &GraphDocument,
    projection: &EditorProjectionModel,
    revision: u64,
    source: &str,
) -> Result<GraphInspection, CapabilityFailure> {
    enforce_result_bound(
        CapabilityId::InspectGraph,
        projection
            .nodes
            .iter()
            .map(|node| 1 + node.ports.len() + node.parameters.len())
            .sum::<usize>()
            + projection.connections.len(),
    )?;
    let nodes = projection
        .nodes
        .iter()
        .map(|node| GraphNodeInspection {
            node_id: node.node_id.to_string(),
            node_type_id: node.node_type.as_str().into(),
            user_label: node.display.user_label.as_deref().map(str::to_owned),
            x: node.position.x,
            y: node.position.y,
            title: node.display.title.to_string(),
            parameters: node.parameters.iter().map(parameter).collect(),
            ports: node
                .ports
                .iter()
                .map(|port| GraphPortFacts {
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
                    accepted_type: port.accepted_type.display.to_string(),
                    maximum_connections: port.connections.maximum,
                    connection_count: port.connections.current,
                    schema: port
                        .resolved_schema
                        .as_ref()
                        .map(|schema| {
                            schema
                                .fields
                                .iter()
                                .map(|field| {
                                    (field.name.to_string(), format!("{:?}", field.scalar_type))
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                    literal: port.input.as_ref().and_then(|input| {
                        input
                            .literal_override
                            .clone()
                            .or_else(|| input.protocol_default.clone())
                    }),
                })
                .collect(),
            port_templates: node
                .port_instance_additions
                .iter()
                .map(|template| GraphPortTemplateInspection {
                    key: template.template_key.as_str().into(),
                    direction: format!("{:?}", template.direction).to_lowercase(),
                    can_add: template.can_add,
                })
                .collect(),
        })
        .collect();
    let constants = document.constants.iter().map(|(id, constant)| {
        let mut value = serde_json::json!({ "id": id, "name": constant.name, "dataType": constant.data_type, "description": constant.description, "tags": constant.tags, "hasTabularData": constant.tabular.is_some() });
        let primitive = match &constant.data_value { yss_data_contract::DataValue::Boolean(_) | yss_data_contract::DataValue::Int64(_) | yss_data_contract::DataValue::Float64(_) | yss_data_contract::DataValue::Null => true, yss_data_contract::DataValue::String(value) => value.len() <= 4096, _ => false };
        value["valueIncluded"] = serde_json::json!(primitive);
        if primitive { value["dataValue"] = serde_json::to_value(&constant.data_value).map_err(|_| graph_failure(CapabilityFailureCode::InternalFailure))?; }
        Ok((id.to_string(), value))
    }).collect::<Result<_, CapabilityFailure>>()?;
    Ok(GraphInspection {
        graph_path: path.as_str().into(),
        graph_hash: graph_hash(document)?,
        revision,
        source: source.into(),
        nodes,
        connections: projection
            .connections
            .iter()
            .map(|connection| GraphConnectionInspection {
                connection_id: connection.connection_id.to_string(),
                output: inspect_port(&connection.output),
                input: inspect_port(&connection.input),
            })
            .collect(),
        constants,
        diagnostics: diagnostics(projection),
    })
}

fn parameter(value: &EditorParameterModel) -> GraphParameterInspection {
    let (options, fields) = match &value.configuration {
        Some(EditorParameterConfiguration::SelectOptions { options }) => (
            options.iter().map(ToString::to_string).collect(),
            Vec::new(),
        ),
        Some(EditorParameterConfiguration::Configuration { fields }) => {
            (Vec::new(), fields.iter().map(parameter).collect())
        }
        Some(EditorParameterConfiguration::ProjectColumns { options, .. }) => (
            options
                .iter()
                .map(|column| column.name.to_string())
                .collect(),
            Vec::new(),
        ),
        _ => (Vec::new(), Vec::new()),
    };
    GraphParameterInspection {
        key: value.key.as_str().into(),
        title: value.display.title.to_string(),
        editor: format!("{:?}", value.editor),
        value: value.value.clone(),
        options,
        fields,
    }
}

fn diagnostics(projection: &EditorProjectionModel) -> Vec<GraphDiagnosticInspection> {
    let mut seen = BTreeSet::new();
    projection
        .diagnostics
        .iter()
        .chain(
            projection
                .nodes
                .iter()
                .flat_map(|node| node.diagnostics.iter()),
        )
        .take(200)
        .filter(|diagnostic| {
            seen.insert((
                diagnostic.code.to_string(),
                format!("{:?}", diagnostic.location),
            ))
        })
        .map(|diagnostic| GraphDiagnosticInspection {
            code: diagnostic.code.to_string(),
            message_key: diagnostic.message_key.to_string(),
            blocking: diagnostic.blocking,
            severity: format!("{:?}", diagnostic.severity).to_lowercase(),
            location: format!("{:?}", diagnostic.location),
            arguments: diagnostic
                .arguments
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
        })
        .collect()
}

pub(crate) fn list_graph_results(
    captured: &ApplicationSession,
    graph_path: String,
) -> Result<GraphResults, CapabilityFailure> {
    let path = GraphResourcePath::new(&graph_path)
        .map_err(|_| graph_failure(CapabilityFailureCode::InvalidRequest))?;
    if !captured
        .project()
        .get_data()
        .map_err(|_| graph_failure(CapabilityFailureCode::GraphUnavailable))?
        .graphs
        .contains_key(&path)
    {
        return Err(graph_failure(CapabilityFailureCode::GraphUnavailable));
    }
    let results = captured
        .execution()
        .query_graph_results(path.as_str(), 100)
        .into_iter()
        .map(|result| GraphResultReference {
            result_id: result.provenance().result_id().get(),
            run_id: result.provenance().run_id().get(),
            output: result.output().port().as_str().into(),
            category: inspect_result_category(result.value().category()),
        })
        .collect();
    Ok(GraphResults {
        graph_path,
        results,
    })
}

pub fn graph_hash(document: &GraphDocument) -> Result<String, CapabilityFailure> {
    yss_canonical_hash::hash_canonical("yssbi.assistant.graph-draft.v1", document)
        .map(|value| hex(&value))
        .map_err(|_| graph_failure(CapabilityFailureCode::InternalFailure))
}

fn parse_hash(value: &str) -> Result<[u8; 32], CapabilityFailure> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid_edit_identity("artifactId"));
    }
    let mut bytes = [0; 32];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[i * 2..i * 2 + 2], 16)
            .map_err(|_| invalid_edit_identity("artifactId"))?;
    }
    Ok(bytes)
}
fn hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn graph_failure(code: CapabilityFailureCode) -> CapabilityFailure {
    CapabilityFailure::new(code)
}
fn map_graph_error(
    error: crate::resource_mutation::ResourceMutationApplicationError,
) -> CapabilityFailure {
    match error {
        crate::resource_mutation::ResourceMutationApplicationError::Mutation(error) => {
            graph_failure(CapabilityFailureCode::MutationRejected)
                .with_detail("reason", error.code())
        }
        _ => graph_failure(CapabilityFailureCode::GraphUnavailable),
    }
}

fn edit_port(port: &PortAddress) -> GraphEditPortRef {
    match &port.port {
        PortRef::Declared { key } => GraphEditPortRef::Declared {
            node_id: port.node_id.to_string(),
            port_key: key.as_str().into(),
        },
        PortRef::Instance {
            template,
            instance_id,
        } => GraphEditPortRef::Instance {
            node_id: port.node_id.to_string(),
            template_key: template.as_str().into(),
            instance_id: instance_id.to_string(),
        },
    }
}

fn resolve_aliases(
    operation: &mut GraphEditOperation,
    nodes: &BTreeMap<String, String>,
    ports: &BTreeMap<String, GraphEditPortRef>,
) -> Result<(), CapabilityFailure> {
    fn node(value: &mut String, nodes: &BTreeMap<String, String>) -> Result<(), CapabilityFailure> {
        if let Some(alias) = value.strip_prefix('$') {
            *value = nodes
                .get(alias)
                .cloned()
                .ok_or_else(|| invalid_edit_identity("nodeId"))?;
        }
        Ok(())
    }
    fn port(
        value: &mut GraphEditPortRef,
        nodes: &BTreeMap<String, String>,
        ports: &BTreeMap<String, GraphEditPortRef>,
    ) -> Result<(), CapabilityFailure> {
        match value {
            GraphEditPortRef::Declared { node_id, .. } => node(node_id, nodes)?,
            GraphEditPortRef::Instance {
                node_id,
                template_key,
                instance_id,
            } => {
                node(node_id, nodes)?;
                if let Some(alias) = instance_id.strip_prefix('$') {
                    let Some(GraphEditPortRef::Instance {
                        node_id: actual_node,
                        template_key: actual_template,
                        instance_id: actual_id,
                    }) = ports.get(alias)
                    else {
                        return Err(invalid_edit_identity("instanceId"));
                    };
                    if node_id != actual_node || template_key != actual_template {
                        return Err(invalid_edit_identity("instanceId"));
                    }
                    *instance_id = actual_id.clone();
                }
            }
        }
        Ok(())
    }
    match operation {
        GraphEditOperation::MoveNodes { positions } => {
            for position in positions {
                node(&mut position.node_id, nodes)?;
            }
        }
        GraphEditOperation::DeleteNodes { node_ids }
        | GraphEditOperation::DuplicateNodes { node_ids, .. } => {
            for id in node_ids {
                node(id, nodes)?;
            }
        }
        GraphEditOperation::SetParameters { node_id, .. }
        | GraphEditOperation::SetConfiguration { node_id, .. }
        | GraphEditOperation::AddPortInstance { node_id, .. }
        | GraphEditOperation::DisconnectNode { node_id } => node(node_id, nodes)?,
        GraphEditOperation::Connect { output, input, .. } => {
            port(output, nodes, ports)?;
            port(input, nodes, ports)?;
        }
        GraphEditOperation::SetLiteral { address, .. }
        | GraphEditOperation::RemovePortInstance { address }
        | GraphEditOperation::DisconnectPort { address } => port(address, nodes, ports)?,
        GraphEditOperation::MoveConnections { source, target } => {
            port(source, nodes, ports)?;
            port(target, nodes, ports)?;
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests;
