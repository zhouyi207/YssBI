//! Map typed capabilities and Core control tools into Rig tools.

use crate::error::invalid_response;
use crate::messages::tool_result_json;
use rig_agent::tool::{DynamicTool, ToolExecutionError, ToolOutput};
use std::sync::{Arc, Mutex};
use yss_harness_contract::{
    AgentDriverFailure, AgentDriverFailureCode, AgentEvent, AgentEventOutput,
    ApplyGraphEditRequest, AutomationCapabilityRequest, CapabilityFailure, CapabilityFailureCode,
    CapabilityId, InspectDatasetProfileRequest, InspectDatasetSchemaRequest, InspectGraphRequest,
    InspectProjectRequest, InspectResultRequest, ModelCapabilityExecutor, ModelCapabilityRequest,
    SearchNodeCatalogRequest, StatisticalPlan, ToolDescriptor, statistical_plan_schema,
};

use crate::arguments;

fn runtime_tool_failure(
    sender: &tokio::sync::watch::Sender<Option<AgentDriverFailure>>,
    code: AgentDriverFailureCode,
) -> ToolExecutionError {
    // Rig turns ordinary tool errors into feedback. Stop fatal errors explicitly before another model step.
    sender.send_if_modified(|failure| {
        if failure.is_some() {
            return false;
        }
        *failure = Some(AgentDriverFailure::new(code));
        true
    });
    ToolExecutionError::other(code.to_string())
}

fn fatal_capability_failure(code: CapabilityFailureCode) -> Option<AgentDriverFailureCode> {
    use CapabilityFailureCode::*;
    match code {
        Cancelled | ProjectSessionUnavailable | ProjectSessionMismatch | ProjectSessionChanged => {
            Some(AgentDriverFailureCode::Cancelled)
        }
        DeadlineElapsed => Some(AgentDriverFailureCode::DeadlineElapsed),
        PersistenceUnavailable | InternalFailure => Some(AgentDriverFailureCode::InternalFailure),
        InvalidRequest | GraphUnavailable | ResourceUnavailable | DatabaseUnavailable
        | CatalogUnavailable | ResultUnavailable | ApprovalRequired | RevisionConflict
        | MutationRejected | ResultTooLarge | InvocationConflict | GraphDraftChanged
        | OutcomeUnknown => None,
    }
}

pub(crate) fn dynamic_tool(
    descriptor: ToolDescriptor,
    capabilities: Arc<dyn ModelCapabilityExecutor>,
    tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    tool_failure: tokio::sync::watch::Sender<Option<AgentDriverFailure>>,
) -> Result<DynamicTool, AgentDriverFailure> {
    let parameters =
        serde_json::to_value(&descriptor.input_schema).map_err(|_| invalid_response())?;
    let schema = Arc::new(parameters.clone());
    let capability_id = descriptor.capability_id;
    Ok(DynamicTool::new(
        capability_id.as_str(),
        tool_description(capability_id),
        parameters,
        move |_context, arguments| {
            let schema = Arc::clone(&schema);
            let capabilities = Arc::clone(&capabilities);
            let tasks = Arc::clone(&tasks);
            let tool_failure = tool_failure.clone();
            Box::pin(async move {
                let request = match decode_request(capability_id, arguments, &schema) {
                    Ok(request) => request,
                    Err(failure) => {
                        return tool_result_json(Err(failure))
                            .map(ToolOutput::json)
                            .map_err(|_| {
                                runtime_tool_failure(
                                    &tool_failure,
                                    AgentDriverFailureCode::InternalFailure,
                                )
                            });
                    }
                };
                let (sender, receiver) = tokio::sync::oneshot::channel();
                let task = tokio::spawn(async move {
                    let outcome = capabilities
                        .execute(ModelCapabilityRequest { request })
                        .await;
                    let _ = sender.send(outcome);
                });
                tasks
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .push(task);
                let outcome = receiver.await.map_err(|_| {
                    runtime_tool_failure(&tool_failure, AgentDriverFailureCode::InternalFailure)
                })?;
                if let Err(failure) = &outcome
                    && let Some(code) = fatal_capability_failure(failure.code)
                {
                    return Err(runtime_tool_failure(&tool_failure, code));
                }
                let result =
                    tool_result_json(outcome.map(|outcome| outcome.result)).map_err(|_| {
                        runtime_tool_failure(&tool_failure, AgentDriverFailureCode::InternalFailure)
                    })?;
                Ok(ToolOutput::json(result))
            })
        },
    ))
}

pub(crate) fn delegation_tool(
    capabilities: Arc<dyn ModelCapabilityExecutor>,
    tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    tool_failure: tokio::sync::watch::Sender<Option<AgentDriverFailure>>,
) -> Result<DynamicTool, AgentDriverFailure> {
    let schema = Arc::new(
        serde_json::to_value(yss_harness_contract::agent_task_schema())
            .map_err(|_| invalid_response())?,
    );
    Ok(DynamicTool::new(
        "delegate_task",
        "Delegate one bounded task to DataAgent, StatsAgent, PlotAgent, ReportAgent or ReviewAgent. Only Manager can use this tool. Supply exact input versions, allowed operations/resources, creation specifications, dependencies and completion criteria. Waits for a durable result with evidence; reusing an identical key returns its existing outcome.",
        (*schema).clone(),
        move |_context, arguments| {
            let schema = schema.clone();
            let capabilities = capabilities.clone();
            let tasks = tasks.clone();
            let tool_failure = tool_failure.clone();
            Box::pin(async move {
                let task = match arguments::decode::<yss_harness_contract::AgentTask>(
                    arguments, &schema,
                ) {
                    Ok(task) => task,
                    Err(error) => {
                        return tool_result_json(Err(error))
                            .map(ToolOutput::json)
                            .map_err(|_| {
                                runtime_tool_failure(
                                    &tool_failure,
                                    AgentDriverFailureCode::InternalFailure,
                                )
                            });
                    }
                };
                let (sender, receiver) = tokio::sync::oneshot::channel();
                let task = tokio::spawn(async move {
                    let _ = sender.send(capabilities.delegate(task).await);
                });
                tasks.lock().unwrap_or_else(|e| e.into_inner()).push(task);
                let result = receiver.await.map_err(|_| {
                    runtime_tool_failure(&tool_failure, AgentDriverFailureCode::InternalFailure)
                })?;
                match result {
                    Ok(outcome) => {
                        serde_json::to_value(outcome)
                            .map(ToolOutput::json)
                            .map_err(|_| {
                                runtime_tool_failure(
                                    &tool_failure,
                                    AgentDriverFailureCode::InternalFailure,
                                )
                            })
                    }
                    Err(error) => {
                        if let Some(code) = fatal_capability_failure(error.code) {
                            return Err(runtime_tool_failure(&tool_failure, code));
                        }
                        tool_result_json(Err(error))
                            .map(ToolOutput::json)
                            .map_err(|_| {
                                runtime_tool_failure(
                                    &tool_failure,
                                    AgentDriverFailureCode::InternalFailure,
                                )
                            })
                    }
                }
            })
        },
    ))
}

pub(crate) fn statistical_plan_tool(
    output: Arc<dyn AgentEventOutput>,
    tool_failure: tokio::sync::watch::Sender<Option<AgentDriverFailure>>,
) -> Result<DynamicTool, AgentDriverFailure> {
    let parameters =
        serde_json::to_value(statistical_plan_schema()).map_err(|_| invalid_response())?;
    let schema = Arc::new(parameters.clone());
    Ok(DynamicTool::new(
        "propose_statistical_plan",
        "Propose a complete typed statistical plan for Harness policy validation before analytical execution. Returns accepted and the exact plan recorded by Harness after validation and persistence.",
        parameters,
        move |_context, arguments| {
            let schema = Arc::clone(&schema);
            let output = Arc::clone(&output);
            let tool_failure = tool_failure.clone();
            Box::pin(async move {
                let plan = match arguments::decode::<StatisticalPlan>(arguments, &schema) {
                    Ok(plan) => plan,
                    Err(failure) => {
                        return tool_result_json(Err(failure))
                            .map(ToolOutput::json)
                            .map_err(|_| {
                                runtime_tool_failure(
                                    &tool_failure,
                                    AgentDriverFailureCode::InternalFailure,
                                )
                            });
                    }
                };
                let receipt = serde_json::json!({ "accepted": true, "plan": &plan });
                output
                    .emit(AgentEvent::PlanProposed { plan })
                    .await
                    .map_err(|failure| match failure {
                        yss_harness_contract::AgentOutputFailure::PolicyRejected { reason, available_methods } => {
                            ToolExecutionError::invalid_args(
                                serde_json::json!({"accepted": false, "reason": reason, "availableMethods": available_methods}).to_string(),
                            )
                        }
                        yss_harness_contract::AgentOutputFailure::Closed
                        | yss_harness_contract::AgentOutputFailure::PersistenceFailed => {
                            runtime_tool_failure(
                                &tool_failure,
                                AgentDriverFailureCode::OutputUnavailable,
                            )
                        }
                    })?;
                Ok(ToolOutput::json(receipt))
            })
        },
    ))
}

fn decode_request(
    capability_id: CapabilityId,
    arguments: serde_json::Value,
    schema: &serde_json::Value,
) -> Result<AutomationCapabilityRequest, CapabilityFailure> {
    match capability_id {
        CapabilityId::InspectResource => {
            arguments::decode(arguments, schema).map(AutomationCapabilityRequest::InspectResource)
        }
        CapabilityId::ManageResource => {
            arguments::decode(arguments, schema).map(AutomationCapabilityRequest::ManageResource)
        }
        CapabilityId::EditResource => {
            arguments::decode(arguments, schema).map(AutomationCapabilityRequest::EditResource)
        }
        CapabilityId::ExportDataset => {
            arguments::decode(arguments, schema).map(AutomationCapabilityRequest::ExportDataset)
        }
        CapabilityId::InspectGraph => arguments::decode::<InspectGraphRequest>(arguments, schema)
            .map(AutomationCapabilityRequest::InspectGraph),
        CapabilityId::SearchNodeCatalog => {
            arguments::decode::<SearchNodeCatalogRequest>(arguments, schema)
                .map(AutomationCapabilityRequest::SearchNodeCatalog)
        }
        CapabilityId::InspectDatasetSchema => {
            arguments::decode::<InspectDatasetSchemaRequest>(arguments, schema)
                .map(AutomationCapabilityRequest::InspectDatasetSchema)
        }
        CapabilityId::InspectDatasetProfile => {
            arguments::decode::<InspectDatasetProfileRequest>(arguments, schema)
                .map(AutomationCapabilityRequest::InspectDatasetProfile)
        }
        CapabilityId::InspectResult => arguments::decode::<InspectResultRequest>(arguments, schema)
            .map(AutomationCapabilityRequest::InspectResult),
        CapabilityId::InspectProject => {
            arguments::decode::<InspectProjectRequest>(arguments, schema)
                .map(AutomationCapabilityRequest::InspectProject)
        }
        CapabilityId::ApplyGraphEdit => {
            arguments::decode::<ApplyGraphEditRequest>(arguments, schema)
                .map(AutomationCapabilityRequest::ApplyGraphEdit)
        }
        CapabilityId::ValidateGraph => {
            arguments::decode::<yss_harness_contract::ValidateGraphRequest>(arguments, schema)
                .map(AutomationCapabilityRequest::ValidateGraph)
        }
        CapabilityId::ExecuteGraph => {
            arguments::decode::<yss_harness_contract::ExecuteGraphRequest>(arguments, schema)
                .map(AutomationCapabilityRequest::ExecuteGraph)
        }
        CapabilityId::SaveGraph => {
            arguments::decode::<yss_harness_contract::SaveGraphRequest>(arguments, schema)
                .map(AutomationCapabilityRequest::SaveGraph)
        }
        CapabilityId::InspectUiIntent => {
            arguments::decode(arguments, schema).map(AutomationCapabilityRequest::InspectUiIntent)
        }
        CapabilityId::RequestUiIntent => {
            arguments::decode(arguments, schema).map(AutomationCapabilityRequest::RequestUiIntent)
        }
        CapabilityId::ListGraphResults => {
            arguments::decode::<yss_harness_contract::ListGraphResultsRequest>(arguments, schema)
                .map(AutomationCapabilityRequest::ListGraphResults)
        }
    }
}

fn tool_description(capability_id: CapabilityId) -> &'static str {
    match capability_id {
        CapabilityId::InspectResource => {
            "Read a project resource using its exact {kind,id} reference. Returns current version and typed contents: a graph and function signature, chart settings, paged mind topics, paged Markdown characters, or paged database rows with stable row IDs/schema/history state. offset/limit apply to characters for Doc, topics for Mind and rows for Database. Preserve content outside the returned page. Use the returned version for resource commands and edits."
        }
        CapabilityId::ManageResource => {
            "Manage all project resources through one typed entry: create, rename, duplicate, delete or save. Creation of Database resources imports the specified CSV, Parquet, Excel or SQL source. Other resources are created by name. Existing-resource operations require the exact {kind,id} and version from inspect_resource or current committed facts. Only delete resources the user requested to remove. Returns committed changes, moves and actual new IDs; use them instead of guessing paths. Chart edits persist immediately; saving a Chart does not reach a separate unsaved GUI draft. Doc/Mind and database edits use their existing edit/save lifecycle."
        }
        CapabilityId::EditResource => {
            "Edit typed resource contents using its current version: replace Chart settings and persist, apply an atomic Mind tree batch, change Markdown by complete replacement or character-range edits, apply one Database row/column/history operation, update Function signature, or undo/redo Graph history. Mind add_node declares clientId; later operations can refer to $clientId. IDs for new nodes/parameters are allocated by the host. Doc/Mind edits remain unsaved until manage_resource save; database operations retain their normal history/checkpoint behavior. Reinspect after version conflicts; do not reconstruct unseen Markdown/tree/data pages."
        }
        CapabilityId::ExportDataset => {
            "Export the current version of a project Database to the user-specified CSV or Parquet file using the normal database export owner. Requires a Database reference and current version. Returns the actual destination after successful publication. Do not claim success or blindly retry if publication outcome is uncertain."
        }
        CapabilityId::InspectUiIntent => {
            "Inspect a workbench intent receipt by ID to determine whether a requested UI operation is pending, claimed, applied, failed or expired."
        }
        CapabilityId::RequestUiIntent => {
            "Request opening any existing project resource with openResource and its exact {kind,id}; nodeId is optional for Event/Function Graph node focus only. Also supports opening a retained result or revealing an allowed panel. Use a unique clientKey, reused only for the identical request. Pending is acceptance, not success: inspect the receipt ID until applied/failed/expired. Requires the workbench to be attached; does not edit/save project data or own FlexLayout."
        }
        CapabilityId::InspectGraph => {
            "Inspect the current Project graph by graphPath, whether or not an editor panel is open: revision, graphHash, semanticInputHash, ready, parameters, concrete port IDs/types/column names, connection limits, constants and diagnostics. Establish a baseline before editing or running; later successful edit/save receipts provide fresh facts and revisions. Inspect again when required facts are missing, the semantic baseline differs or a version conflict occurs."
        }
        CapabilityId::SearchNodeCatalog => {
            "Search node IDs, localized names, aliases and technical terms. Use concise terms (e.g. decompose, ols, multiply) or node type IDs. Set includeParameters=true to read parameter definitions, defaults, constraints and configurable initial port counts before creating a node."
        }
        CapabilityId::InspectDatasetSchema => {
            "Inspect a bounded dataset schema and its current runtime/schema revisions."
        }
        CapabilityId::InspectDatasetProfile => {
            "Inspect bounded data-quality and shape statistics. null metrics (including duplicatedRows) mean unknown or not computed, never zero."
        }
        CapabilityId::InspectResult => {
            "Read the complete result JSON produced by YssBI, including all nested fields and data references. DataFrame/DataSeries values are paged, never expanded in full. To read a tableRef from the JSON, call inspect_result with its resultRef.executionSessionId, resultRef.resultId and part. Execution sessions change on project restart; rediscover current result references instead of reusing stale IDs. offset and limit paginate rows; use nextOffset only when hasMore is true."
        }
        CapabilityId::InspectProject => {
            "List all six project resource kinds with exact resource {kind,id}, displayName and current resource revision, including closed files. Reuse resource for inspect_resource/manage_resource/edit_resource and UI openResource. For graph-specific tools use resource.id as graphPath. Project membership comes from the project index; listing does not read resource contents or dataset rows."
        }
        CapabilityId::ApplyGraphEdit => {
            "Apply one atomic, undoable batch to the current Project graph after the user requests edits. No editor panel is required. Use baseRevision and graphHash from the latest inspection or committed receipt (toRevision for edits, resourceRevision for saves); use a unique clientKey per batch. Create nodes with parameters and portCounts (empty maps for defaults) in one operation. With clientId, reference the node as $clientId and initial variable input instances as $clientId.templateKey[0] in instanceId within that batch. Added port instances support the same $clientId in instanceId. Supports create/delete/move/duplicate nodes, parameters/literals/constants, connect/disconnect and add/remove input instances. create_constant adds a boolean/integer/decimal/string constant and its Get node; set_literal accepts a plain JSON value or null to clear. set_parameters atomically merges supplied keys with current parameters; null resets a field to its protocol default. Conditional fields are resolved by the host. Each successful batch automatically persists the complete current graph and retains undo history. File, document, history and receipt commit together; a save failure does not apply the batch. Returns toRevision, graphHash and changes with complete changed nodes/parameters/ports/derived columns, changed connections and order, removals, constants, ready and complete diagnostics. Reuse the returned facts for subsequent edits or execution without inspecting again; apply the changes only to a matching fromRevision and baseSemanticInputHash. Unchanged entities are omitted. Replayed receipts retain the original facts. Oversized receipts are rejected before commit; use smaller batches."
        }
        CapabilityId::ValidateGraph => {
            "Validate the current graph using graphHash from the latest inspection or successful edit receipt. The edit receipt already includes ready and diagnostics for its commit. Returns readiness and blocking diagnostics from editor analysis. This optional read-only check does not save, prepare an execution plan, or execute."
        }
        CapabilityId::ExecuteGraph => {
            "Execute the current graph using its current graphHash and explicit demand. Use {type:default} for the full graph, or {type:node,nodeId,mode:currentInputs} to run one node using already evaluated current inputs. Use mode:dependencies to run to a node and compute missing dependencies; unrelated unfinished nodes do not block this scope. Missing or stale inputs return input_result_unavailable with a source location. Only evaluated results are returned; internal query plans remain deferred. Prepares its execution plan automatically; no prior validation call or artifact ID is required. Returns actual run status, failures and result references captured from this run's committed handoff. resultCount is the number published on success (null on failure); resultsComplete=false means the bounded references are not the complete output list. Use returned references directly with inspect_result for needed content, without listing the same results again. Later edits or runs do not change this receipt; referenced results still require availability checks. Does not save."
        }
        CapabilityId::SaveGraph => {
            "Save the current graph only when the user requests saving. Pass the current graphHash. Uses the normal Save operation and clears its undo history. Returns fromRevision, resourceRevision, graphHash and the committed dirty/canUndo/canRedo state. Use resourceRevision as the next baseRevision without an inspection solely to refresh the version. These facts describe this save, not later edits."
        }
        CapabilityId::ListGraphResults => {
            "List currently retained result IDs, run IDs and output ports for a graph, including manual runs. Use these IDs with inspect_result; do not ask the user to invent or locate an ID."
        }
    }
}

#[cfg(test)]
mod tests;
