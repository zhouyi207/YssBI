use super::{
    enforce_result_bound, ensure_project_binding, inspect_port, inspect_result_category,
    invalid_request, map_session_capture_error, map_session_revalidation_error,
};
use crate::graph::edit::GraphDocumentChange;
use crate::graph::edit::GraphDocumentEditor;
use crate::graph::editing::{GraphActivity, GraphEditRequest};
use crate::graph::run::{RunApplicationEventKind, RunGraphRequest, run_graph_with_sink};
use crate::session::{ApplicationSession, ApplicationState};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use yss_graph_document::GraphDocument;
use yss_graph_document::{
    ConnectionId, GraphResourcePath, NodeId, NodePosition, OrderKey, PortAddress, PortInstanceId,
    PortRef,
};
use yss_graph_editor::projection::*;
use yss_graph_editor::{EditorGraphMutation, NodePositionMutation};
use yss_harness_contract::*;
use yss_node_catalog::LocalizedCatalogItem;
use yss_node_protocol::PortKey;
use yss_project_identity::OperationId;

pub fn invoke_graph_capability(
    application: &ApplicationState,
    context: CapabilityInvocationContext,
    request: AutomationCapabilityRequest,
    control: &CapabilityControl,
) -> Result<AutomationCapabilityResult, CapabilityFailure> {
    control.check()?;
    request
        .validate()
        .map_err(|error| invalid_request(request.capability_id(), error))?;
    let captured = application
        .capture_session()
        .map_err(map_session_capture_error)?;
    ensure_project_binding(&captured, &context)?;
    let path = GraphResourcePath::new(
        graph_action_path(&request)
            .ok_or_else(|| graph_failure(CapabilityFailureCode::InvalidRequest))?,
    )
    .map_err(|_| graph_failure(CapabilityFailureCode::InvalidRequest))?;
    let locale = match &request {
        AutomationCapabilityRequest::ApplyGraphEdit(request) => request.locale.clone(),
        _ => "en-US".into(),
    };
    // Capture the baseline and commit under the same per-graph operation reservation.
    let editing = if matches!(&request, AutomationCapabilityRequest::ApplyGraphEdit(_)) {
        Some(
            captured
                .coordinate_graph_edit(&path)
                .map_err(|_| graph_failure(CapabilityFailureCode::InvocationConflict))?,
        )
    } else {
        None
    };
    let open_request = crate::graph::open::OpenGraphRequest::new(
        captured.project_instance_id().clone(),
        path.clone(),
        0,
        locale.clone(),
    );
    let opened = if editing.is_some() {
        crate::graph::open::open_graph_in_session(application, &captured, open_request)
    } else {
        application.open_graph(open_request)
    }
    .map_err(|_| graph_failure(CapabilityFailureCode::GraphUnavailable))?;
    let current = opened.editing();
    let document = opened.document();
    let projection = opened.projection();
    let edit_identity = if let AutomationCapabilityRequest::ApplyGraphEdit(edit) = &request {
        let identity = graph_edit_identity(&context, edit)?;
        if let Some(receipt) = captured
            .project()
            .graph_edit_command_receipt(
                captured.project_instance_id(),
                &path,
                current.version.session_id,
                identity.0,
            )
            .map_err(|_| graph_failure(CapabilityFailureCode::GraphUnavailable))?
        {
            return graph_edit_replay(receipt, edit, identity.1)
                .map(AutomationCapabilityResult::GraphEditReceipt);
        }
        Some(identity)
    } else {
        None
    };
    let hash = graph_hash(document)?;
    let expected = match &request {
        AutomationCapabilityRequest::ApplyGraphEdit(request) => Some(&request.graph_hash),
        AutomationCapabilityRequest::ValidateGraph(request) => Some(&request.graph_hash),
        AutomationCapabilityRequest::ExecuteGraph(request) => Some(&request.graph_hash),
        AutomationCapabilityRequest::SaveGraph(request) => Some(&request.graph_hash),
        _ => None,
    };
    if expected.is_some_and(|expected| expected != &hash) {
        return Err(graph_failure(CapabilityFailureCode::GraphDraftChanged));
    }
    let read_only = request.capability_id().descriptor().effect == ToolEffect::Inspect;
    let result = match request {
        AutomationCapabilityRequest::InspectGraph(_) => {
            enforce_result_bound(
                CapabilityId::InspectGraph,
                projection
                    .nodes
                    .iter()
                    .map(|node| {
                        1 + node.ports.len()
                            + node
                                .parameter_groups
                                .iter()
                                .map(|group| group.parameters.len())
                                .sum::<usize>()
                    })
                    .sum::<usize>()
                    + projection.connections.len(),
            )?;
            AutomationCapabilityResult::GraphInspection(inspect_projection(
                &path,
                document,
                projection,
                current.version.revision.get(),
                hash,
            )?)
        }
        AutomationCapabilityRequest::ApplyGraphEdit(request) => {
            if request.base_revision != current.version.revision.get() {
                return Err(graph_failure(CapabilityFailureCode::GraphDraftChanged));
            }
            let (operation_id, fingerprint) = edit_identity.expect("edit identity was prepared");
            let mut operation = captured
                .project()
                .capture_graph_edit(
                    captured.project_instance_id(),
                    &path,
                    current.version,
                    operation_id,
                    fingerprint,
                )
                .map_err(|_| graph_failure(CapabilityFailureCode::GraphDraftChanged))?;
            let before = inspect_projection(
                &path,
                document,
                projection,
                current.version.revision.get(),
                hash,
            )?;
            let (transform, mut receipt) = transform_graph_edit(
                application,
                &captured,
                request,
                document.clone(),
                before,
                control,
            )?;
            operation
                .set_edit_correlation(yss_project::GraphEditCorrelation {
                    client_key: receipt.client_key.clone(),
                    document_hash: receipt.graph_hash.clone(),
                    created_nodes: receipt
                        .created_nodes
                        .iter()
                        .map(|(alias, id)| Ok((alias.clone(), parse_node_id(id)?)))
                        .collect::<Result<_, CapabilityFailure>>()?,
                    created_ports: receipt
                        .created_ports
                        .iter()
                        .map(|(alias, port)| Ok((alias.clone(), parse_edit_port(port.clone())?)))
                        .collect::<Result<_, CapabilityFailure>>()?,
                    result_facts: serde_json::to_value(&receipt.changes)
                        .map_err(|_| graph_failure(CapabilityFailureCode::InternalFailure))?,
                })
                .map_err(|_| graph_failure(CapabilityFailureCode::ResultTooLarge))?;
            control.check()?;
            let committed = captured
                .project()
                .save_graph_edit(operation, Arc::new(transform.document), transform.patch)
                .map_err(|error| match error {
                    yss_project::ProjectGraphSaveError::Filesystem(_) => {
                        graph_failure(CapabilityFailureCode::PersistenceUnavailable)
                    }
                    yss_project::ProjectGraphSaveError::Commit(_) => {
                        graph_failure(CapabilityFailureCode::GraphDraftChanged)
                    }
                })?;
            captured
                .execution()
                .observe_graph_result_inputs(path.as_str(), transform.result_inputs);
            captured.publish_graph_activity(GraphActivity::Changed {
                graph_path: path.as_str().into(),
                editing: committed.editing.clone(),
            });
            receipt.from_revision = committed.from_revision.get();
            receipt.to_revision = committed.to_revision.get();
            AutomationCapabilityResult::GraphEditReceipt(receipt)
        }
        AutomationCapabilityRequest::ValidateGraph(_) => {
            AutomationCapabilityResult::GraphValidation(GraphValidation {
                graph_path: path.as_str().into(),
                graph_hash: hash,
                ready: matches!(projection.outcome, EditorResolutionOutcome::Complete)
                    && !projection
                        .diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.blocking),
                diagnostics: diagnostics(projection),
            })
        }
        AutomationCapabilityRequest::SaveGraph(_) => {
            let saved = application
                .save_current_graph(GraphEditRequest {
                    project_instance_id: captured.project_instance_id().clone(),
                    graph_path: path.clone(),
                    version: current.version,
                    operation_id: OperationId::new(),
                    locale,
                })
                .map_err(map_graph_error)?;
            AutomationCapabilityResult::GraphSaved(GraphSaved {
                graph_path: path.as_str().into(),
                graph_hash: graph_hash(&saved.graph.update.document)?,
                from_revision: current.version.revision.get(),
                resource_revision: saved.resource_revision.get(),
                dirty: saved.graph.editing.dirty,
                can_undo: saved.graph.editing.can_undo,
                can_redo: saved.graph.editing.can_redo,
            })
        }
        AutomationCapabilityRequest::ExecuteGraph(_) => {
            let mut run_id = None;
            let mut failure_code = None;
            let mut failure_location = None;
            let mut status = "failed";
            let outcome = run_graph_with_sink(
                application,
                RunGraphRequest::new(
                    captured.project_instance_id().clone(),
                    path.clone(),
                    document.clone(),
                    projection.basis.semantic_input_hash,
                )
                .with_cancellation(control.cancellation_flag())
                .with_deadline(control.deadline()),
                |event| {
                    run_id = Some(event.identity().run_id().get());
                    match event.kind() {
                        RunApplicationEventKind::RunCancelled => status = "cancelled",
                        RunApplicationEventKind::RunErrored { failure } => {
                            failure_code = Some(format!("{:?}", failure.code));
                            failure_location =
                                Some(format!("{:?}: {:?}", failure.phase, failure.source));
                        }
                        _ => {}
                    }
                    true
                },
            );
            if outcome.is_err() && failure_code.is_none() && status != "cancelled" {
                failure_code = Some("graph_execution_failed".into());
            }
            let mut result_count = None;
            let mut results = Vec::new();
            if let Ok(receipt) = outcome {
                run_id = Some(receipt.identity.run_id().get());
                status = "succeeded";
                result_count = Some(receipt.results.len());
                results = receipt
                    .results
                    .iter()
                    .take(usize::from(
                        CapabilityId::ExecuteGraph.descriptor().maximum_results,
                    ))
                    .map(|result| GraphResultReference {
                        execution_session_id: receipt
                            .identity
                            .execution_session_id()
                            .as_uuid()
                            .to_string(),
                        result_id: result.result_id.get(),
                        run_id: receipt.identity.run_id().get(),
                        output: result.output.port().as_str().into(),
                        category: inspect_result_category(result.category),
                    })
                    .collect();
            }
            AutomationCapabilityResult::GraphExecution(GraphExecution {
                graph_path: path.as_str().into(),
                graph_hash: hash,
                run_id,
                status: status.into(),
                failure_code,
                failure_location,
                result_count,
                results_complete: result_count.is_some_and(|count| count == results.len()),
                results,
            })
        }
        _ => return Err(graph_failure(CapabilityFailureCode::InvalidRequest)),
    };
    if read_only {
        control.check()?;
        application
            .revalidate_captured_session(&captured)
            .map_err(map_session_revalidation_error)?;
    }
    Ok(result)
}

fn graph_edit_identity(
    context: &CapabilityInvocationContext,
    request: &ApplyGraphEditRequest,
) -> Result<(OperationId, [u8; 32]), CapabilityFailure> {
    let digest = yss_canonical_hash::hash_canonical(
        "yssbi.graph-capability-operation.v1",
        &(
            context.principal_id(),
            context.harness_session_id(),
            context.invocation_id(),
            context.project(),
            &request.graph_path,
            &request.client_key,
        ),
    )
    .map_err(|_| graph_failure(CapabilityFailureCode::InvalidRequest))?;
    let fingerprint =
        yss_canonical_hash::hash_canonical("yssbi.graph-capability-request.v1", request)
            .map_err(|_| graph_failure(CapabilityFailureCode::InvalidRequest))?;
    let mut bytes: [u8; 16] = digest[..16].try_into().expect("digest has sixteen bytes");
    bytes[6] = (bytes[6] & 0x0f) | 0x80;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok((
        OperationId::from_uuid(uuid::Uuid::from_bytes(bytes)),
        fingerprint,
    ))
}

impl ApplicationState {
    pub fn recover_automation_graph_edit(
        &self,
        context: CapabilityInvocationContext,
        request: ApplyGraphEditRequest,
    ) -> Result<Option<GraphEditReceipt>, CapabilityFailure> {
        AutomationCapabilityRequest::ApplyGraphEdit(request.clone())
            .validate()
            .map_err(|error| invalid_request(CapabilityId::ApplyGraphEdit, error))?;
        let captured = self.capture_session().map_err(map_session_capture_error)?;
        ensure_project_binding(&captured, &context)?;
        let path = GraphResourcePath::new(&request.graph_path)
            .map_err(|_| graph_failure(CapabilityFailureCode::InvalidRequest))?;
        let _editing = captured
            .coordinate_graph_edit(&path)
            .map_err(|_| graph_failure(CapabilityFailureCode::InvocationConflict))?;
        let snapshot = captured
            .project()
            .read_graph_editing(captured.project_instance_id(), &path)
            .map_err(|_| graph_failure(CapabilityFailureCode::GraphUnavailable))?;
        let (operation_id, fingerprint) = graph_edit_identity(&context, &request)?;
        let receipt = captured
            .project()
            .graph_edit_command_receipt(
                captured.project_instance_id(),
                &path,
                snapshot.state.version.session_id,
                operation_id,
            )
            .map_err(|_| graph_failure(CapabilityFailureCode::GraphUnavailable))?;
        self.revalidate_captured_session(&captured)
            .map_err(map_session_revalidation_error)?;
        receipt
            .map(|receipt| graph_edit_replay(receipt, &request, fingerprint))
            .transpose()
    }
}

fn graph_edit_replay(
    receipt: yss_project::GraphEditCommandReceipt,
    request: &ApplyGraphEditRequest,
    fingerprint: [u8; 32],
) -> Result<GraphEditReceipt, CapabilityFailure> {
    if receipt.fingerprint != fingerprint
        || receipt.request_version.revision.get() != request.base_revision
    {
        return Err(graph_failure(CapabilityFailureCode::InvocationConflict));
    }
    let correlation = receipt
        .correlation
        .ok_or_else(|| graph_failure(CapabilityFailureCode::InvocationConflict))?;
    Ok(GraphEditReceipt {
        graph_path: request.graph_path.clone(),
        from_revision: receipt.commit.from_revision.get(),
        to_revision: receipt.commit.to_revision.get(),
        client_key: correlation.client_key,
        graph_hash: correlation.document_hash,
        created_nodes: correlation
            .created_nodes
            .into_iter()
            .map(|(alias, node)| (alias, node.to_string()))
            .collect(),
        created_ports: correlation
            .created_ports
            .into_iter()
            .map(|(alias, port)| (alias, edit_port(&port)))
            .collect(),
        changes: serde_json::from_value(correlation.result_facts)
            .map_err(|_| graph_failure(CapabilityFailureCode::InternalFailure))?,
    })
}

fn graph_action_path(request: &AutomationCapabilityRequest) -> Option<&str> {
    match request {
        AutomationCapabilityRequest::InspectGraph(r) => Some(&r.graph_path),
        AutomationCapabilityRequest::ApplyGraphEdit(r) => Some(&r.graph_path),
        AutomationCapabilityRequest::ValidateGraph(r) => Some(&r.graph_path),
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
    before: GraphInspection,
    control: &CapabilityControl,
) -> Result<(GraphDocumentChange, GraphEditReceipt), CapabilityFailure> {
    let graph_path = GraphResourcePath::new(&request.graph_path)
        .map_err(|_| graph_failure(CapabilityFailureCode::InvalidRequest))?;
    let mut editor = GraphDocumentEditor::new(captured, &graph_path, &request.locale, original)
        .map_err(map_graph_error)?;
    let localized = editor.localized_catalog();
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
            use yss_data_contract::{DataValue, ValueType};
            let (data_type, data_value) = match value {
                GraphConstantLiteral::Boolean(value) => (
                    ValueType::Scalar(yss_data_contract::SemanticType::Binary),
                    DataValue::Bool(value),
                ),
                GraphConstantLiteral::Integer(value) => (
                    ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
                    DataValue::Integer(value),
                ),
                GraphConstantLiteral::Decimal(value) => (
                    ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
                    DataValue::Decimal(
                        yss_data_contract::DecimalLiteral::try_from(value)
                            .map_err(|_| invalid_edit_identity("value"))?,
                    ),
                ),
                GraphConstantLiteral::String(value) => (
                    ValueType::Scalar(yss_data_contract::SemanticType::Text),
                    DataValue::String(value.into()),
                ),
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
            editor.apply(mutation).map_err(map_graph_error)?;
            operation = GraphEditOperation::InsertConstantReference {
                id: id.to_string(),
                x,
                y,
                client_id,
            };
        }
        let adds_port = matches!(operation, GraphEditOperation::AddPortInstance { .. });
        let mutation = editor_mutation(operation, &localized.items)?;
        let before_nodes = editor
            .document()
            .nodes
            .keys()
            .copied()
            .collect::<BTreeSet<_>>();
        let before_ports = editor
            .document()
            .port_bindings
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        editor.apply(mutation).map_err(map_graph_error)?;
        if let Some(alias) = alias {
            if adds_port {
                let address = editor
                    .document()
                    .port_bindings
                    .keys()
                    .find(|address| !before_ports.contains(*address))
                    .ok_or_else(|| graph_failure(CapabilityFailureCode::MutationRejected))?;
                created_ports.insert(alias, edit_port(address));
            } else {
                let id = editor
                    .document()
                    .nodes
                    .keys()
                    .find(|id| !before_nodes.contains(id))
                    .ok_or_else(|| graph_failure(CapabilityFailureCode::MutationRejected))?;
                created_nodes.insert(alias, id.to_string());
            }
        }
    }
    control.check()?;
    let transformed = editor.finish(application).map_err(map_graph_error)?;
    let staged = &transformed.document;
    created_nodes.retain(|_, id| parse_node_id(id).is_ok_and(|id| staged.nodes.contains_key(&id)));
    created_ports.retain(|_, port| {
        parse_edit_port(port.clone())
            .is_ok_and(|address| staged.port_bindings.contains_key(&address))
    });
    let graph_hash = graph_hash(staged)?;
    let to_revision = request
        .base_revision
        .checked_add(1)
        .ok_or_else(|| invalid_edit_identity("baseRevision"))?;
    let after = inspect_projection(
        &graph_path,
        staged,
        &transformed.projection_replacement.projection,
        to_revision,
        graph_hash.clone(),
    )?;
    let receipt = GraphEditReceipt {
        graph_path: request.graph_path,
        from_revision: request.base_revision,
        to_revision,
        client_key: request.client_key,
        graph_hash,
        created_nodes,
        created_ports,
        changes: graph_changes(before, after),
    };
    AutomationCapabilityResult::GraphEditReceipt(receipt.clone()).validate_budget(
        ToolDescriptor::for_capability(CapabilityId::ApplyGraphEdit)
            .map_err(|_| graph_failure(CapabilityFailureCode::InternalFailure))?
            .result_budget
            .maximum_bytes as usize,
    )?;
    Ok((transformed, receipt))
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
                        yss_node_protocol::ParameterKey::new(key)
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

fn inspect_projection(
    path: &GraphResourcePath,
    document: &GraphDocument,
    projection: &EditorProjectionModel,
    revision: u64,
    graph_hash: String,
) -> Result<GraphInspection, CapabilityFailure> {
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
            parameters: node
                .parameter_groups
                .iter()
                .flat_map(|group| group.parameters.iter())
                .map(parameter)
                .collect(),
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
        let content_hash = yss_canonical_hash::hash_canonical("yssbi.assistant.graph-constant.v1", constant)
            .map_err(|_| graph_failure(CapabilityFailureCode::InternalFailure))?;
        value["contentHash"] = serde_json::json!(hex(&content_hash));
        let primitive = match &constant.data_value { yss_data_contract::DataValue::Bool(_) | yss_data_contract::DataValue::Integer(_) | yss_data_contract::DataValue::Decimal(_) | yss_data_contract::DataValue::Null => true, yss_data_contract::DataValue::String(value) => value.len() <= 4096, _ => false };
        value["valueIncluded"] = serde_json::json!(primitive);
        if primitive { value["dataValue"] = serde_json::to_value(&constant.data_value).map_err(|_| graph_failure(CapabilityFailureCode::InternalFailure))?; }
        Ok((id.to_string(), value))
    }).collect::<Result<_, CapabilityFailure>>()?;
    let diagnostics = diagnostics(projection);
    Ok(GraphInspection {
        graph_path: path.as_str().into(),
        semantic_input_hash: hex(&projection.basis.semantic_input_hash),
        ready: matches!(projection.outcome, EditorResolutionOutcome::Complete)
            && !diagnostics.iter().any(|diagnostic| diagnostic.blocking),
        graph_hash,
        revision,
        nodes,
        connections: projection
            .connections
            .iter()
            .map(|connection| GraphConnectionInspection {
                connection_id: connection.connection_id.to_string(),
                output: inspect_port(&connection.output),
                input: inspect_port(&connection.input),
                order: connection.order.as_deref().map(str::to_owned),
            })
            .collect(),
        constants,
        diagnostics,
    })
}

fn graph_changes(before: GraphInspection, after: GraphInspection) -> GraphEditChanges {
    let previous_nodes = before
        .nodes
        .iter()
        .map(|node| (node.node_id.as_str(), node))
        .collect::<BTreeMap<_, _>>();
    let next_nodes = after
        .nodes
        .iter()
        .map(|node| node.node_id.clone())
        .collect::<BTreeSet<_>>();
    let previous_connections = before
        .connections
        .iter()
        .map(|connection| (connection.connection_id.as_str(), connection))
        .collect::<BTreeMap<_, _>>();
    let next_connections = after
        .connections
        .iter()
        .map(|connection| connection.connection_id.clone())
        .collect::<BTreeSet<_>>();
    GraphEditChanges {
        base_semantic_input_hash: before.semantic_input_hash,
        semantic_input_hash: after.semantic_input_hash,
        nodes: after
            .nodes
            .into_iter()
            .filter(|node| previous_nodes.get(node.node_id.as_str()).copied() != Some(node))
            .collect(),
        removed_node_ids: previous_nodes
            .keys()
            .filter(|id| !next_nodes.contains(**id))
            .map(|id| (*id).to_owned())
            .collect(),
        connections: after
            .connections
            .into_iter()
            .filter(|connection| {
                previous_connections
                    .get(connection.connection_id.as_str())
                    .copied()
                    != Some(connection)
            })
            .collect(),
        removed_connection_ids: previous_connections
            .keys()
            .filter(|id| !next_connections.contains(**id))
            .map(|id| (*id).to_owned())
            .collect(),
        removed_constant_ids: before
            .constants
            .keys()
            .filter(|id| !after.constants.contains_key(*id))
            .cloned()
            .collect(),
        constants: after
            .constants
            .into_iter()
            .filter(|(id, value)| before.constants.get(id) != Some(value))
            .collect(),
        ready: after.ready,
        diagnostics: after.diagnostics,
    }
}

fn parameter(value: &EditorParameterModel) -> GraphParameterInspection {
    let options = match &value.configuration {
        Some(EditorParameterConfiguration::SelectOptions { options }) => {
            options.iter().map(ToString::to_string).collect()
        }
        Some(EditorParameterConfiguration::ProjectColumns { options, .. }) => options
            .iter()
            .map(|column| column.name.to_string())
            .collect(),
        _ => Vec::new(),
    };
    GraphParameterInspection {
        key: value.key.as_str().into(),
        title: value.display.title.to_string(),
        editor: format!("{:?}", value.editor),
        value: value.value.clone(),
        options,
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
        .filter(|diagnostic| seen.insert(diagnostic.clone()))
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
    let results = crate::graph::results::query_graph_results(captured, &path, 100)
        .map_err(|_| graph_failure(CapabilityFailureCode::GraphUnavailable))?
        .into_iter()
        .map(|result| GraphResultReference {
            execution_session_id: result
                .provenance()
                .reference()
                .execution_session_id
                .as_uuid()
                .to_string(),
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

fn hex(value: &[u8]) -> String {
    value.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn graph_failure(code: CapabilityFailureCode) -> CapabilityFailure {
    CapabilityFailure::new(code)
}
fn map_graph_error(
    error: impl Into<crate::graph::resources::ResourceMutationApplicationError>,
) -> CapabilityFailure {
    match error.into() {
        crate::graph::resources::ResourceMutationApplicationError::Mutation(error) => {
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
