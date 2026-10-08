use super::{
    ensure_project_binding, inspect_port, inspect_result_category, map_session_capture_error,
    map_session_revalidation_error,
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

mod constants;
mod inspection;
mod validation;

pub fn invoke_graph_capability(
    application: &ApplicationState,
    context: CapabilityInvocationContext,
    request: AutomationCapabilityRequest,
    control: &CapabilityControl,
) -> Result<AutomationCapabilityResult, CapabilityFailure> {
    control.check()?;
    if let Some(agent) = context.agent() {
        yss_harness_core::authorize_agent_capability(agent, &request)?;
    }
    request
        .validate()
        .map_err(|error| error.into_failure(request.capability_id()))?;
    let requested_graph = match &request {
        AutomationCapabilityRequest::GraphMutation(value) => Some(value.input.graph().clone()),
        AutomationCapabilityRequest::ValidateGraph(value) => Some(value.graph.clone()),
        AutomationCapabilityRequest::ExecuteGraph(value) => Some(value.graph.clone()),
        _ => None,
    };
    // Authorization and the ledger use the original public intention. Only this
    // owner adapter translates it to the existing staged, atomic edit contract.
    let request = match request {
        AutomationCapabilityRequest::GraphMutation(value) => {
            AutomationCapabilityRequest::ApplyGraphEdit(value.edit_request())
        }
        request => request,
    };
    let captured = application
        .capture_session()
        .map_err(map_session_capture_error)?;
    ensure_project_binding(&captured, &context)?;
    let path = GraphResourcePath::new(
        graph_action_path(&request)
            .ok_or_else(|| graph_failure(CapabilityFailureCode::InvalidRequest))?,
    )
    .map_err(|_| graph_failure(CapabilityFailureCode::InvalidRequest))?;
    if let Some(graph) = requested_graph {
        inspection::validate_graph_reference(&graph, &path)?;
    }
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
    if context
        .graph_observation()
        .is_some_and(|expected| expected != hex(&projection.basis.semantic_input_hash))
    {
        return Err(graph_failure(CapabilityFailureCode::RevisionConflict)
            .with_detail("reason", "graph_inputs_changed"));
    }
    let read_only = request.capability_id().descriptor().effect == ToolEffect::Inspect;
    let version = ResourceVersion {
        revision: current.version.revision.get(),
        session_id: Some(current.version.session_id.to_string()),
    };
    let mut result = match request {
        AutomationCapabilityRequest::FindConstants(request) => {
            constants::find(request, &path, document, projection, version, hash)?
        }
        AutomationCapabilityRequest::InspectConstants(request) => {
            constants::inspect(request, &path, document, projection, version, hash)?
        }
        AutomationCapabilityRequest::FindNodes(request) => {
            inspection::find_nodes(request, &path, document, projection, version, hash)?
        }
        AutomationCapabilityRequest::InspectNodes(request) => {
            inspection::inspect_nodes(request, &path, document, projection, version, hash)?
        }
        AutomationCapabilityRequest::FindConnections(request) => {
            inspection::find_connections(request, &path, document, projection, version, hash)?
        }
        AutomationCapabilityRequest::InspectGraph(request) => inspection::inspect(
            request,
            &path,
            document,
            projection,
            ResourceVersion {
                revision: current.version.revision.get(),
                session_id: Some(current.version.session_id.to_string()),
            },
            hash,
        )?,
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
                ResourceVersion {
                    revision: current.version.revision.get(),
                    session_id: Some(current.version.session_id.to_string()),
                },
                hash,
            )?;
            let (transform, mut receipt) = transform_graph_edit(
                application,
                &captured,
                request,
                Arc::clone(&operation.document),
                before,
                control,
            )?;
            operation
                .set_edit_correlation(yss_project::GraphEditCorrelation {
                    created_constants: receipt
                        .created_constants
                        .iter()
                        .map(|(alias, id)| Ok((alias.clone(), constants::parse_id(id)?)))
                        .collect::<Result<_, CapabilityFailure>>()?,
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
                .save_graph_edit(operation, transform.document, transform.patch)
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
        AutomationCapabilityRequest::ValidateGraph(request) => {
            validation::inspect(request, &path, opened.analysis(), hash)?
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
        AutomationCapabilityRequest::ExecuteGraph(request) => {
            let mut run_id = None;
            let mut failure_code = None;
            let mut failure_location = None;
            let mut status = "failed";
            let mut run_request = RunGraphRequest::new(
                captured.project_instance_id().clone(),
                path.clone(),
                document.clone(),
                projection.basis.semantic_input_hash,
            )
            .with_demand(match &request.demand {
                yss_harness_contract::GraphExecutionDemand::Default => {
                    crate::graph::run::RunDemand::Default
                }
                yss_harness_contract::GraphExecutionDemand::Node { node_id, mode } => {
                    crate::graph::run::RunDemand::Node {
                        node_id: parse_node_id(node_id)?,
                        mode: match mode {
                            yss_harness_contract::NodeExecutionMode::CurrentInputs => {
                                yss_graph_execution::plan::NodeExecutionMode::CurrentInputs
                            }
                            yss_harness_contract::NodeExecutionMode::Dependencies => {
                                yss_graph_execution::plan::NodeExecutionMode::Dependencies
                            }
                        },
                    }
                }
            })
            .with_cancellation(control.cancellation_flag())
            .with_deadline(control.deadline());
            if let Some(agent) = context.agent() {
                let task = agent
                    .task
                    .as_ref()
                    .ok_or_else(|| graph_failure(CapabilityFailureCode::InvalidRequest))?;
                let authorizations = task
                    .resources
                    .iter()
                    .filter(|entry| {
                        entry.operations.contains(&AgentResourceOperation::Inspect)
                            || entry.operations.contains(&AgentResourceOperation::Execute)
                    })
                    .map(|entry| {
                        let resource = if entry.resource.kind == ProjectResourceKind::Database {
                            format!("databases/{}", entry.resource.id)
                        } else {
                            entry.resource.id.clone()
                        };
                        Ok(crate::graph::run::RunResourceAuthorization {
                            resource: yss_project::execution_authority::ProjectResourceId::new(
                                resource.into_boxed_str(),
                            )
                            .map_err(|_| graph_failure(CapabilityFailureCode::InvalidRequest))?,
                            access: if yss_harness_core::authorize_agent_resource(
                                agent,
                                &entry.resource,
                                AgentResourceOperation::Edit,
                            )
                            .is_ok()
                            {
                                yss_project::execution_authority::ProjectResourceAccess::Exclusive
                            } else {
                                yss_project::execution_authority::ProjectResourceAccess::Shared
                            },
                            expected_version: entry
                                .version
                                .as_ref()
                                .map(|version| version.revision),
                        })
                    })
                    .collect::<Result<Vec<_>, CapabilityFailure>>()?;
                // Manager's project read access follows only the graph dependencies
                // selected by the execution owner. Explicit grants retain their versions.
                run_request = run_request
                    .with_resource_authorizations(authorizations)
                    .with_inherited_graph_reads();
            }
            let outcome = run_graph_with_sink(application, run_request, |event| {
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
            });
            if let Err(error) = &outcome
                && failure_code.is_none()
                && status != "cancelled"
            {
                if let crate::graph::run::ExecutionApplicationError::ResourceBindings(binding) =
                    error
                {
                    failure_location = Some(binding.to_string());
                }
                failure_code = Some(
                    match error {
                        crate::graph::run::ExecutionApplicationError::ResourceBindings(
                            crate::graph::run::ResourceBindingError::ScopeDenied { .. },
                        ) => "agent_scope_denied",
                        crate::graph::run::ExecutionApplicationError::ResourceBindings(
                            crate::graph::run::ResourceBindingError::VersionChanged { .. },
                        ) => "resource_version_changed",
                        crate::graph::run::ExecutionApplicationError::DraftChanged => {
                            "resource_version_changed"
                        }
                        crate::graph::run::ExecutionApplicationError::PreparedExecution(error)
                            if error.failure().code == yss_graph_execution::error::RunFailureCode::InputResultUnavailable => {
                                let failure = error.failure();
                                failure_location = Some(format!("{:?}: {:?}", failure.phase, failure.source));
                                "input_result_unavailable"
                            }
                        _ => "graph_execution_failed",
                    }
                    .into(),
                );
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
                        result_ref: ResultRef::new(
                            receipt
                                .identity
                                .execution_session_id()
                                .as_uuid()
                                .to_string(),
                            result.result_id.get(),
                        ),
                        validity: ResultValidity::CurrentValid,
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
                timing: run_id.and_then(|id| inspect_run(&captured, id).map(|(_, timing)| timing)),
                failure_code,
                failure_location,
                result_count,
                results_complete: result_count.is_some_and(|count| count == results.len()),
                results,
            })
        }
        _ => return Err(graph_failure(CapabilityFailureCode::InvalidRequest)),
    };
    if let AutomationCapabilityResult::GraphInspectionPage(page) = &mut result
        && page.view == GraphInspectionView::Summary
    {
        let mut runs = BTreeMap::new();
        for event in captured
            .execution_snapshot()
            .into_iter()
            .filter(|event| event.identity().graph_path() == &path)
        {
            let status = match event.kind() {
                RunApplicationEventKind::RunStarted { .. } => "running",
                RunApplicationEventKind::RunCompleted => "completed",
                RunApplicationEventKind::RunCancelled => "cancelled",
                RunApplicationEventKind::RunErrored { .. } => "failed",
                RunApplicationEventKind::ResultInspectionRequested { .. } => continue,
            };
            let run_id = event.identity().run_id().get();
            let observation = inspect_run(&captured, run_id);
            runs.insert(
                run_id,
                GraphRunInspection {
                    run_id,
                    status: observation
                        .as_ref()
                        .map_or_else(|| status.into(), |(status, _)| status.clone()),
                    timing: observation.map(|(_, timing)| timing),
                    current_inputs: hex(event.identity().semantic_input_hash())
                        == page.semantic_input_hash,
                },
            );
        }
        page.runs = Some(runs.into_values().collect());
    }
    if read_only {
        control.check()?;
        application
            .revalidate_captured_session(&captured)
            .map_err(map_session_revalidation_error)?;
    }
    Ok(result)
}

pub(super) fn inspect_run(
    captured: &ApplicationSession,
    run_id: u64,
) -> Option<(String, GraphRunTiming)> {
    use yss_graph_execution::run_registry::{RunId, RunState};
    let snapshot = captured
        .execution()
        .runs()
        .snapshot(RunId::from_existing(run_id))?;
    let milliseconds =
        |duration: std::time::Duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
    let status = match snapshot.state {
        RunState::Admitted => "admitted",
        RunState::Running => "running",
        RunState::Finalizing => "finalizing",
        RunState::Succeeded => "succeeded",
        RunState::Cancelled => "cancelled",
        RunState::Failed => "failed",
    };
    Some((
        status.into(),
        GraphRunTiming {
            elapsed_ms: milliseconds(snapshot.timing.elapsed()),
            admission_ms: milliseconds(snapshot.timing.admission),
            running_ms: milliseconds(snapshot.timing.running),
            finalization_ms: milliseconds(snapshot.timing.finalization),
        },
    ))
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
            .map_err(|error| error.into_failure(CapabilityId::ApplyGraphEdit))?;
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
        created_constants: correlation
            .created_constants
            .into_iter()
            .map(|(alias, id)| (alias, id.to_string()))
            .collect(),
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
        AutomationCapabilityRequest::InspectGraph(r) => Some(&r.graph.id),
        AutomationCapabilityRequest::FindNodes(r) => Some(&r.graph.id),
        AutomationCapabilityRequest::FindConstants(r) => Some(&r.graph.id),
        AutomationCapabilityRequest::InspectConstants(r) => Some(&r.graph.id),
        AutomationCapabilityRequest::InspectNodes(r) => Some(&r.graph.id),
        AutomationCapabilityRequest::FindConnections(r) => Some(&r.graph.id),
        AutomationCapabilityRequest::ApplyGraphEdit(r) => Some(&r.graph_path),
        AutomationCapabilityRequest::ValidateGraph(r) => Some(&r.graph.id),
        AutomationCapabilityRequest::ExecuteGraph(r) => Some(&r.graph.id),
        AutomationCapabilityRequest::SaveGraph(r) => Some(&r.graph_path),
        _ => None,
    }
}

fn transform_graph_edit(
    application: &ApplicationState,
    captured: &Arc<ApplicationSession>,
    request: ApplyGraphEditRequest,
    original: Arc<GraphDocument>,
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
    let mut created_constants = BTreeMap::new();
    for mut operation in request.operations {
        control.check()?;
        match &operation {
            GraphEditOperation::CreateConstant { declaration } => {
                constants::create(
                    &mut editor,
                    declaration.clone(),
                    &mut created_constants,
                    &mut created_nodes,
                )?;
                continue;
            }
            GraphEditOperation::UpdateConstant { update } => {
                constants::update(&mut editor, update.clone())?;
                continue;
            }
            _ => {}
        }
        let alias = match &operation {
            GraphEditOperation::CreateNode { client_id, .. }
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
        if let Some(copied) = editor.apply(mutation).map_err(map_graph_error)? {
            for (source, target) in copied.nodes {
                if created_nodes
                    .insert(source.to_string(), target.to_string())
                    .is_some()
                {
                    return Err(invalid_edit_identity("nodeIds"));
                }
            }
            for (source, target) in copied.ports {
                if created_ports
                    .insert(source.to_string(), edit_port(&target))
                    .is_some()
                {
                    return Err(invalid_edit_identity("nodeIds"));
                }
            }
        }
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
                record_created_port_aliases(
                    editor.document(),
                    *id,
                    &alias,
                    &created_nodes,
                    &mut created_ports,
                )?;
                created_nodes.insert(alias, id.to_string());
            }
        }
    }
    control.check()?;
    let transformed = editor.finish(application).map_err(map_graph_error)?;
    let staged = &transformed.document;
    created_constants
        .retain(|_, id| constants::parse_id(id).is_ok_and(|id| staged.constants.contains_key(&id)));
    created_nodes.retain(|_, id| parse_node_id(id).is_ok_and(|id| staged.nodes.contains_key(&id)));
    created_ports.retain(|_, port| {
        parse_edit_port(port.clone()).is_ok_and(|address| {
            staged.nodes.contains_key(&address.node_id)
                && (matches!(address.port, PortRef::Declared { .. })
                    || staged.port_bindings.contains_key(&address))
        })
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
        ResourceVersion {
            revision: to_revision,
            session_id: before.version.session_id.clone(),
        },
        graph_hash.clone(),
    )?;
    let receipt = GraphEditReceipt {
        created_constants,
        graph_path: request.graph_path,
        from_revision: request.base_revision,
        to_revision,
        client_key: request.client_key,
        graph_hash,
        created_nodes,
        created_ports,
        changes: graph_changes(before, after),
    };
    AutomationCapabilityResult::GraphEditReceipt(receipt.clone())
        .validate_budget(MAX_CAPABILITY_RESULT_BYTES)?;
    Ok((transformed, receipt))
}

pub(super) fn editor_mutation(
    operation: GraphEditOperation,
    catalog: &[LocalizedCatalogItem],
) -> Result<EditorGraphMutation, CapabilityFailure> {
    match operation {
        GraphEditOperation::CreateConstant { .. } | GraphEditOperation::UpdateConstant { .. } => {
            Err(invalid_edit_identity("operation"))
        }
        GraphEditOperation::SetNodeLabel { node_id, label } => {
            Ok(EditorGraphMutation::SetNodeLabel {
                node_id: parse_node_id(&node_id)?,
                label,
            })
        }
        GraphEditOperation::SetPortCounts { node_id, counts } => {
            Ok(EditorGraphMutation::SetPortCounts {
                node_id: parse_node_id(&node_id)?,
                counts: parse_port_counts(counts)?,
            })
        }
        GraphEditOperation::UpdateConnections { connections } => {
            Ok(EditorGraphMutation::UpdateConnections {
                connections: connections
                    .into_iter()
                    .map(|connection| {
                        Ok(yss_graph_editor::ConnectionUpdate {
                            connection_id: uuid::Uuid::parse_str(&connection.connection_id)
                                .map(ConnectionId::from_uuid)
                                .map_err(|_| invalid_edit_identity("connectionId"))?,
                            output: parse_edit_port(connection.output)?,
                            input: parse_edit_port(connection.input)?,
                            order: connection.order.map(|order| order.map(OrderKey::new)),
                        })
                    })
                    .collect::<Result<_, CapabilityFailure>>()?,
            })
        }
        GraphEditOperation::SetParameters {
            node_id,
            parameters,
        } => Ok(EditorGraphMutation::SetParameters {
            node_id: parse_node_id(&node_id)?,
            parameters: parse_edit_parameters(parameters)?,
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
        GraphEditOperation::DeleteConstant { constant_id } => {
            Ok(EditorGraphMutation::SetConstant {
                id: constants::parse_id(&constant_id)?,
                constant: None,
            })
        }
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
            parameters,
            port_counts,
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
                parameters: parse_edit_parameters(parameters)?,
                port_counts: parse_port_counts(port_counts)?,
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

fn parse_edit_parameters(
    parameters: BTreeMap<String, serde_json::Value>,
) -> Result<yss_graph_document::ParameterValues, CapabilityFailure> {
    parameters
        .into_iter()
        .map(|(key, value)| {
            Ok((
                yss_node_protocol::ParameterKey::new(key)
                    .map_err(|_| invalid_edit_identity("parameterKey"))?,
                value,
            ))
        })
        .collect()
}

fn parse_port_counts(
    counts: BTreeMap<String, u16>,
) -> Result<yss_node_protocol::InitialPortCounts, CapabilityFailure> {
    counts
        .into_iter()
        .map(|(key, count)| {
            PortKey::new(key)
                .map(|key| (key, count))
                .map_err(|_| invalid_edit_identity("portCounts"))
        })
        .collect()
}

fn parse_node_id(value: &str) -> Result<NodeId, CapabilityFailure> {
    uuid::Uuid::parse_str(value)
        .map(NodeId::from_uuid)
        .map_err(|_| invalid_edit_identity("nodeId")
            .with_detail("reason", "invalid_node_reference")
            .with_detail("nextStep", "Use an existing node UUID, or $clientId for a node created in this same batch. Later calls must use the actual UUID from createdNodes."))
}

pub(super) fn parse_edit_port(value: GraphEditPortRef) -> Result<PortAddress, CapabilityFailure> {
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
    version: ResourceVersion,
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
            ports: node.ports.iter().map(inspection::port_facts).collect(),
            port_templates: inspection::port_templates(node),
        })
        .collect();
    let constants = inspection::constants(document, 0, usize::MAX)?;
    let diagnostics = diagnostics(projection);
    Ok(GraphInspection {
        graph_path: path.as_str().into(),
        semantic_input_hash: hex(&projection.basis.semantic_input_hash),
        ready: matches!(projection.outcome, EditorResolutionOutcome::Complete)
            && !diagnostics.iter().any(|diagnostic| diagnostic.blocking),
        graph_hash,
        revision: version.revision,
        version,
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
    let (options, context_hint) = match &value.configuration {
        Some(EditorParameterConfiguration::SelectOptions { options }) => (
            Some(options.iter().map(ToString::to_string).collect()),
            None,
        ),
        Some(EditorParameterConfiguration::ProjectColumns {
            schema_known,
            context_hint,
            options,
            ..
        }) => (
            schema_known.then(|| {
                options
                    .iter()
                    .map(|column| column.name.to_string())
                    .collect()
            }),
            context_hint.as_ref().map(ToString::to_string),
        ),
        Some(EditorParameterConfiguration::FilterPredicate {
            schema_known,
            context_hint,
            columns,
            ..
        }) => (
            schema_known.then(|| {
                columns
                    .iter()
                    .map(|column| column.name.to_string())
                    .collect()
            }),
            context_hint.as_ref().map(ToString::to_string),
        ),
        _ => (None, None),
    };
    GraphParameterInspection {
        key: value.key.as_str().into(),
        title: value.display.title.to_string(),
        editor: format!("{:?}", value.editor),
        value: value.value.clone(),
        options,
        context_hint,
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

fn record_created_port_aliases(
    document: &GraphDocument,
    node_id: NodeId,
    alias: &str,
    nodes: &BTreeMap<String, String>,
    ports: &mut BTreeMap<String, GraphEditPortRef>,
) -> Result<(), CapabilityFailure> {
    let mut templates = BTreeMap::<_, Vec<_>>::new();
    for (address, binding) in &document.port_bindings {
        if address.node_id != node_id {
            continue;
        }
        if let (
            PortRef::Instance { template, .. },
            yss_graph_document::DynamicPortBinding::UserCreated { order },
        ) = (&address.port, binding)
        {
            templates
                .entry(template)
                .or_default()
                .push((order, address));
        }
    }
    for (template, mut members) in templates {
        members.sort();
        for (index, (_, address)) in members.into_iter().enumerate() {
            let key = format!("{alias}.{template}[{index}]");
            if nodes.contains_key(&key) || ports.insert(key, edit_port(address)).is_some() {
                return Err(invalid_edit_identity("clientId"));
            }
        }
    }
    Ok(())
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
                .ok_or_else(|| invalid_edit_identity("nodeId")
                    .with_detail("reason", "unknown_node_alias")
                    .with_detail("nextStep", "Use $clientId only for a node declared earlier in this same batch. For existing nodes, use their actual UUID."))?;
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
        | GraphEditOperation::SetNodeLabel { node_id, .. }
        | GraphEditOperation::SetPortCounts { node_id, .. }
        | GraphEditOperation::AddPortInstance { node_id, .. }
        | GraphEditOperation::DisconnectNode { node_id } => node(node_id, nodes)?,
        GraphEditOperation::Connect { output, input, .. } => {
            port(output, nodes, ports)?;
            port(input, nodes, ports)?;
        }
        GraphEditOperation::UpdateConnections { connections } => {
            for connection in connections {
                port(&mut connection.output, nodes, ports)?;
                port(&mut connection.input, nodes, ports)?;
            }
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
