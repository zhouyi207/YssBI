//! Map typed capabilities and Core control tools into Rig tools.

use crate::error::invalid_response;
use crate::messages::tool_result_json;
use rig_agent::tool::{DynamicTool, ToolExecutionError, ToolOutput};
use std::sync::{Arc, Mutex};
use yss_harness_contract::{
    AgentDriverFailure, AgentDriverFailureCode, AgentEvent, AgentEventOutput, CapabilityFailure,
    CapabilityFailureCode, CapabilityId, ModelCapabilityExecutor, ModelCapabilityRequest,
    StatisticalPlan, ToolDescriptor, model::CapabilityInput, statistical_plan_schema,
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
        move |arguments| {
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

pub(crate) fn worker_tool(
    followup: bool,
    capabilities: Arc<dyn ModelCapabilityExecutor>,
    tasks: Arc<Mutex<Vec<tokio::task::JoinHandle<()>>>>,
    tool_failure: tokio::sync::watch::Sender<Option<AgentDriverFailure>>,
) -> Result<DynamicTool, AgentDriverFailure> {
    let (name, description, parameters) = if followup {
        (
            "followup_task",
            "Continue an existing worker by runId, keeping its history, checkpoints and committed receipts. Inspect changed inputs before resuming; the host binds observed facts within the original grant. Use after partial completion or interruption instead of creating a fresh worker.",
            serde_json::to_value(yss_harness_contract::agent_followup_schema()),
        )
    } else {
        (
            "delegate_task",
            "Delegate a task with precise resources, permissions and completion criteria. The host captures read observations and deduplicates identical task specifications within this turn. Use followup_task to continue a previous worker.",
            serde_json::to_value(yss_harness_contract::agent_task_schema()),
        )
    };
    let schema = Arc::new(parameters.map_err(|_| invalid_response())?);
    Ok(DynamicTool::new(
        name,
        description,
        (*schema).clone(),
        move |arguments| {
            let schema = schema.clone();
            let capabilities = capabilities.clone();
            let tasks = tasks.clone();
            let tool_failure = tool_failure.clone();
            Box::pin(async move {
                enum Work {
                    New(yss_harness_contract::model::AgentTaskInput),
                    Followup(yss_harness_contract::model::AgentFollowupInput),
                }
                let decoded = if followup {
                    arguments::decode(arguments, &schema).map(Work::Followup)
                } else {
                    arguments::decode(arguments, &schema).map(Work::New)
                };
                let work = match decoded {
                    Ok(work) => work,
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
                    let result = match work {
                        Work::New(task) => capabilities.delegate(task).await,
                        Work::Followup(request) => capabilities.followup(request).await,
                    };
                    let _ = sender.send(result);
                });
                tasks.lock().unwrap_or_else(|e| e.into_inner()).push(task);
                let result = receiver.await.map_err(|_| {
                    runtime_tool_failure(&tool_failure, AgentDriverFailureCode::InternalFailure)
                })?;
                match result {
                    Ok(outcome) => Ok(ToolOutput::json(yss_harness_contract::model::task_outcome(
                        &outcome,
                    ))),
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
        move |arguments| {
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
) -> Result<CapabilityInput, CapabilityFailure> {
    match capability_id {
        CapabilityId::SearchKnowledge => {
            arguments::decode(arguments, schema).map(CapabilityInput::SearchKnowledge)
        }
        CapabilityId::ReadKnowledge => {
            arguments::decode(arguments, schema).map(CapabilityInput::ReadKnowledge)
        }
        CapabilityId::InspectResource => {
            arguments::decode(arguments, schema).map(CapabilityInput::InspectResource)
        }
        CapabilityId::ManageResource => {
            arguments::decode(arguments, schema).map(CapabilityInput::ManageResource)
        }
        CapabilityId::EditResource => {
            arguments::decode(arguments, schema).map(CapabilityInput::EditResource)
        }
        CapabilityId::ExportDataset => {
            arguments::decode(arguments, schema).map(CapabilityInput::ExportDataset)
        }
        CapabilityId::InspectGraph => {
            arguments::decode(arguments, schema).map(CapabilityInput::InspectGraph)
        }
        CapabilityId::SearchNodeCatalog => {
            arguments::decode(arguments, schema).map(CapabilityInput::SearchNodeCatalog)
        }
        CapabilityId::InspectDatasetSchema => {
            arguments::decode(arguments, schema).map(CapabilityInput::InspectDatasetSchema)
        }
        CapabilityId::InspectDatasetProfile => {
            arguments::decode(arguments, schema).map(CapabilityInput::InspectDatasetProfile)
        }
        CapabilityId::InspectResult => {
            arguments::decode(arguments, schema).map(CapabilityInput::InspectResult)
        }
        CapabilityId::InspectProject => {
            arguments::decode(arguments, schema).map(CapabilityInput::InspectProject)
        }
        CapabilityId::ApplyGraphEdit => {
            arguments::decode(arguments, schema).map(CapabilityInput::ApplyGraphEdit)
        }
        CapabilityId::ValidateGraph => {
            arguments::decode(arguments, schema).map(CapabilityInput::ValidateGraph)
        }
        CapabilityId::ExecuteGraph => {
            arguments::decode(arguments, schema).map(CapabilityInput::ExecuteGraph)
        }
        CapabilityId::SaveGraph => {
            arguments::decode(arguments, schema).map(CapabilityInput::SaveGraph)
        }
        CapabilityId::InspectUiIntent => {
            arguments::decode(arguments, schema).map(CapabilityInput::InspectUiIntent)
        }
        CapabilityId::RequestUiIntent => {
            arguments::decode(arguments, schema).map(CapabilityInput::RequestUiIntent)
        }
        CapabilityId::ListGraphResults => {
            arguments::decode(arguments, schema).map(CapabilityInput::ListGraphResults)
        }
    }
}

fn tool_description(capability_id: CapabilityId) -> &'static str {
    match capability_id {
        CapabilityId::SearchKnowledge => {
            "Search statistical knowledge when background, methods or interpretation guidance is needed. Returns excerpts and passage references. Refine queries or scopes; read a passage before citing it. Sources never substitute for this project's computed results."
        }
        CapabilityId::ReadKnowledge => {
            "Read a previously discovered knowledge passage. The host checks access and currentness and records its citation. If unavailable, search again."
        }
        CapabilityId::InspectResource => {
            "Read an exact resource {kind,id}. Returns its name, dirty state and typed, paged contents. metadataOnly:true omits contents. Graphs default to a paged overview; use inspect_graph for targeted details. Markdown offsets count Unicode characters. Read function contents before changing its signature. The host captures the read baseline."
        }
        CapabilityId::ManageResource => {
            "Create, rename, duplicate, delete or save an authorized resource. Create Databases from the specified source; create other resources by name. Use actual IDs and moves from receipts. Inspect a newly created or renamed resource before further edits or saves. Doc/Mind edits require explicit save; Chart edits persist immediately. Delete only requested resources."
        }
        CapabilityId::EditResource => {
            "Edit Chart settings, an atomic Mind tree batch, Markdown, Database rows/columns/history, function signature or graph history. The host binds the task's read baseline. Mind add_node declares clientId aliases referenced as $clientId; IDs for new nodes/parameters are host allocated. Save Doc/Mind after editing. Preserve unread content. On a conflict, return to the Manager to reassess the changed task."
        }
        CapabilityId::ExportDataset => {
            "Export an authorized Database to the requested CSV or Parquet path. The host binds the captured resource. Returns the actual destination after publication; uncertain outcomes must be checked before retrying."
        }
        CapabilityId::InspectUiIntent => {
            "Inspect a workbench intent receipt by ID: pending, claimed, applied, failed or expired."
        }
        CapabilityId::RequestUiIntent => {
            "Request opening a resource with exact {kind,id}, a retained result, or an allowed panel. nodeId optionally focuses a graph node. A pending/claimed receipt only acknowledges acceptance; inspect its ID before claiming completion. Does not edit or save data."
        }
        CapabilityId::InspectGraph => {
            "Read graph facts without executing nodes. Default view:overview pages node identities/names, counts and readiness. view:nodes includes parameter values and variable-pin templates; includeOptions adds current choices. view:ports includes addresses/types/constraints/literals; includeSchema adds columns. nodeIds filters nodes, ports and incident connections; portAddresses narrows ports. Diagnostics and constants have separate views. Follow page.nextOffset for additional needed items; omitted fields are unknown. view:full is expensive. Reuse committed edit facts and read only missing details. Execution outputs come from inspect_result."
        }
        CapabilityId::SearchNodeCatalog => {
            "Search node IDs, names, aliases and technical terms. Use concise terms or type IDs. includeParameters:true returns configurationSchema for parameters and portCounts plus pin declarations. Supply both maps in one create_node operation. Omitted values use defaults or remain incomplete; null resets. x-yss-requiredForExecution marks runtime requirements; x-yss-activeWhen describes conditional fields; x-yss-linkedPorts configures a shared pin count. Read connected choices through inspect_graph view:nodes includeOptions:true."
        }
        CapabilityId::InspectDatasetSchema => {
            "Read dataset columns, physical types, semantic annotations and nullability. The host captures data currentness."
        }
        CapabilityId::InspectDatasetProfile => {
            "Read dataset quality/profile facts. Null metrics mean unknown or not computed, never zero."
        }
        CapabilityId::InspectResult => {
            "Read complete result JSON, including nested fields and table references. DataFrame/DataSeries values are paged. To read tableRef use its resultRef.executionSessionId, resultRef.resultId and part. Follow nextOffset only while hasMore. Old execution references can expire; rediscover current results after project restart."
        }
        CapabilityId::InspectProject => {
            "List all six project resource kinds with exact resource {kind,id} and displayName, including closed files. Reuse resource.id as graphPath. Listing does not read contents or data rows."
        }
        CapabilityId::ApplyGraphEdit => {
            "Apply and save an atomic, undoable graph edit batch. The host binds the task's observed graph and deduplicates retries. Create nodes with parameters and portCounts (empty maps for defaults) at once. clientId aliases use $clientId; initial variable pins use $clientId.templateKey[0] in instanceId. Supports node/connection/parameter/literal/constant and variable-pin edits. set_parameters merges keys; null resets defaults. Returns actual created IDs and committed changes: complete changed nodes, ports, parameters, connections, removals, constants, readiness and diagnostics. Reuse them for subsequent edits or execution. Unchanged entities are omitted; preserve unrelated content. A save failure leaves the batch uncommitted; oversized receipts require smaller batches."
        }
        CapabilityId::ValidateGraph => {
            "Read current readiness and blocking diagnostics. Edit receipts already contain these facts. This optional check does not save or execute; the host binds the observed graph."
        }
        CapabilityId::ExecuteGraph => {
            "Execute with explicit demand: {type:default} reruns the graph; {type:node,nodeId,mode:currentInputs} consumes existing inputs; mode:dependencies computes missing dependencies. Unrelated incomplete branches do not block node scope. The host binds the observed graph and prepares its plan. Check actual status and failures. Use returned result references directly with inspect_result. resultCount is null on failure; resultsComplete:false means references are partial. Does not save."
        }
        CapabilityId::SaveGraph => {
            "Save current manual graph changes when requested. Graph edit batches already save automatically. Returns committed dirty/canUndo/canRedo state; the host binds and advances the captured baseline."
        }
        CapabilityId::ListGraphResults => {
            "List currently retained result references, run IDs and output ports, including manual runs. Use the returned references with inspect_result."
        }
    }
}

#[cfg(test)]
mod tests;
