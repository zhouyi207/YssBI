//! Map typed capabilities and Core control tools into Rig tools.

use crate::error::invalid_response;
use crate::messages::tool_result_json;
use rig_agent::tool::{DynamicTool, ToolExecutionError, ToolOutput};
use std::sync::{Arc, Mutex};
use yss_harness_contract::{
    AgentDriverFailure, AgentDriverFailureCode, CapabilityFailure, CapabilityFailureCode,
    CapabilityId, ModelCapabilityExecutor, ModelCapabilityRequest, ToolDescriptor,
    model::CapabilityInput,
};

use crate::arguments;
mod feedback;
pub(crate) use feedback::ToolCallFeedback;

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
                let request = decode_request(capability_id, arguments, &schema);
                let (sender, receiver) = tokio::sync::oneshot::channel();
                let task = tokio::spawn(async move {
                    let outcome = match request {
                        Ok(request) => {
                            capabilities
                                .execute(ModelCapabilityRequest { request })
                                .await
                        }
                        Err(failure) => capabilities.reject_arguments(capability_id, failure).await,
                    };
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

mod control;
pub(crate) use control::control_tool;

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
        CapabilityId::CreateResource => {
            arguments::decode(arguments, schema).map(CapabilityInput::CreateResource)
        }
        CapabilityId::InspectDatabase => {
            arguments::decode(arguments, schema).map(CapabilityInput::InspectDatabase)
        }
        CapabilityId::InspectDatabaseSchema => {
            arguments::decode(arguments, schema).map(CapabilityInput::InspectDatabaseSchema)
        }
        CapabilityId::ProfileDatabase => {
            arguments::decode(arguments, schema).map(CapabilityInput::ProfileDatabase)
        }
        CapabilityId::InspectMind => {
            arguments::decode(arguments, schema).map(CapabilityInput::InspectMind)
        }
        CapabilityId::FindTopics => {
            arguments::decode(arguments, schema).map(CapabilityInput::FindTopics)
        }
        CapabilityId::InspectTopics => {
            arguments::decode(arguments, schema).map(CapabilityInput::InspectTopics)
        }
        CapabilityId::CreateTopics => {
            arguments::decode(arguments, schema).map(CapabilityInput::CreateTopics)
        }
        CapabilityId::UpdateTopics => {
            arguments::decode(arguments, schema).map(CapabilityInput::UpdateTopics)
        }
        CapabilityId::MoveTopics => {
            arguments::decode(arguments, schema).map(CapabilityInput::MoveTopics)
        }
        CapabilityId::DeleteTopics => {
            arguments::decode(arguments, schema).map(CapabilityInput::DeleteTopics)
        }
        CapabilityId::DuplicateTopics => {
            arguments::decode(arguments, schema).map(CapabilityInput::DuplicateTopics)
        }
        CapabilityId::InspectChart => {
            arguments::decode(arguments, schema).map(CapabilityInput::InspectChart)
        }
        CapabilityId::UpdateChart => {
            arguments::decode(arguments, schema).map(CapabilityInput::UpdateChart)
        }
        CapabilityId::InspectDocument => {
            arguments::decode(arguments, schema).map(CapabilityInput::InspectDocument)
        }
        CapabilityId::ReadDocument => {
            arguments::decode(arguments, schema).map(CapabilityInput::ReadDocument)
        }
        CapabilityId::SearchDocument => {
            arguments::decode(arguments, schema).map(CapabilityInput::SearchDocument)
        }
        CapabilityId::ReplaceDocumentText => {
            arguments::decode(arguments, schema).map(CapabilityInput::ReplaceDocumentText)
        }
        CapabilityId::AppendDocument => {
            arguments::decode(arguments, schema).map(CapabilityInput::AppendDocument)
        }
        CapabilityId::WriteDocument => {
            arguments::decode(arguments, schema).map(CapabilityInput::WriteDocument)
        }
        CapabilityId::ReadDatabaseRows => {
            arguments::decode(arguments, schema).map(CapabilityInput::ReadDatabaseRows)
        }
        CapabilityId::ImportDatabase => {
            arguments::decode(arguments, schema).map(CapabilityInput::ImportDatabase)
        }
        CapabilityId::InsertRows => {
            arguments::decode(arguments, schema).map(CapabilityInput::InsertRows)
        }
        CapabilityId::UpdateCells => {
            arguments::decode(arguments, schema).map(CapabilityInput::UpdateCells)
        }
        CapabilityId::CreateColumns => {
            arguments::decode(arguments, schema).map(CapabilityInput::CreateColumns)
        }
        CapabilityId::RenameColumns => {
            arguments::decode(arguments, schema).map(CapabilityInput::RenameColumns)
        }
        CapabilityId::DeleteColumns => {
            arguments::decode(arguments, schema).map(CapabilityInput::DeleteColumns)
        }
        CapabilityId::CastColumns => {
            arguments::decode(arguments, schema).map(CapabilityInput::CastColumns)
        }
        CapabilityId::SetColumnSemantics => {
            arguments::decode(arguments, schema).map(CapabilityInput::SetColumnSemantics)
        }
        CapabilityId::DeleteRows => {
            arguments::decode(arguments, schema).map(CapabilityInput::DeleteRows)
        }
        CapabilityId::RenameResource => {
            arguments::decode(arguments, schema).map(CapabilityInput::RenameResource)
        }
        CapabilityId::DuplicateResource => {
            arguments::decode(arguments, schema).map(CapabilityInput::DuplicateResource)
        }
        CapabilityId::DeleteResource => {
            arguments::decode(arguments, schema).map(CapabilityInput::DeleteResource)
        }
        CapabilityId::SaveResource => {
            arguments::decode(arguments, schema).map(CapabilityInput::SaveResource)
        }
        CapabilityId::UndoResource => {
            arguments::decode(arguments, schema).map(CapabilityInput::UndoResource)
        }
        CapabilityId::RedoResource => {
            arguments::decode(arguments, schema).map(CapabilityInput::RedoResource)
        }
        CapabilityId::EditResource => {
            arguments::decode(arguments, schema).map(CapabilityInput::EditResource)
        }
        CapabilityId::ExportDatabase => {
            arguments::decode(arguments, schema).map(CapabilityInput::ExportDatabase)
        }
        CapabilityId::InspectGraph => {
            arguments::decode(arguments, schema).map(CapabilityInput::InspectGraph)
        }
        CapabilityId::BrowseNodes => {
            arguments::decode(arguments, schema).map(CapabilityInput::BrowseNodes)
        }
        CapabilityId::InspectNodeType => {
            arguments::decode(arguments, schema).map(CapabilityInput::InspectNodeType)
        }
        CapabilityId::FindNodes => {
            arguments::decode(arguments, schema).map(CapabilityInput::FindNodes)
        }
        CapabilityId::FindConstants => {
            arguments::decode(arguments, schema).map(CapabilityInput::FindConstants)
        }
        CapabilityId::InspectConstants => {
            arguments::decode(arguments, schema).map(CapabilityInput::InspectConstants)
        }
        CapabilityId::CreateConstants => {
            arguments::decode(arguments, schema).map(CapabilityInput::CreateConstants)
        }
        CapabilityId::UpdateConstants => {
            arguments::decode(arguments, schema).map(CapabilityInput::UpdateConstants)
        }
        CapabilityId::DeleteConstants => {
            arguments::decode(arguments, schema).map(CapabilityInput::DeleteConstants)
        }

        CapabilityId::InspectNodes => {
            arguments::decode(arguments, schema).map(CapabilityInput::InspectNodes)
        }
        CapabilityId::FindConnections => {
            arguments::decode(arguments, schema).map(CapabilityInput::FindConnections)
        }

        CapabilityId::InspectDatasetSchema => Err(CapabilityFailure::new(
            CapabilityFailureCode::InvalidRequest,
        )),
        CapabilityId::InspectDatasetProfile => Err(CapabilityFailure::new(
            CapabilityFailureCode::InvalidRequest,
        )),
        CapabilityId::InspectResult => {
            arguments::decode(arguments, schema).map(CapabilityInput::InspectResult)
        }
        CapabilityId::ReadResultTable => {
            arguments::decode(arguments, schema).map(CapabilityInput::ReadResultTable)
        }
        CapabilityId::ListResources => {
            arguments::decode(arguments, schema).map(CapabilityInput::ListResources)
        }
        CapabilityId::ApplyGraphEdit => Err(CapabilityFailure::new(
            CapabilityFailureCode::InvalidRequest,
        )),
        CapabilityId::CreateNodes => {
            arguments::decode(arguments, schema).map(CapabilityInput::CreateNodes)
        }
        CapabilityId::UpdateNodes => {
            arguments::decode(arguments, schema).map(CapabilityInput::UpdateNodes)
        }
        CapabilityId::DeleteNodes => {
            arguments::decode(arguments, schema).map(CapabilityInput::DeleteNodes)
        }
        CapabilityId::DuplicateNodes => {
            arguments::decode(arguments, schema).map(CapabilityInput::DuplicateNodes)
        }
        CapabilityId::MoveNodes => {
            arguments::decode(arguments, schema).map(CapabilityInput::MoveNodes)
        }
        CapabilityId::CreateConnections => {
            arguments::decode(arguments, schema).map(CapabilityInput::CreateConnections)
        }
        CapabilityId::UpdateConnections => {
            arguments::decode(arguments, schema).map(CapabilityInput::UpdateConnections)
        }
        CapabilityId::DeleteConnections => {
            arguments::decode(arguments, schema).map(CapabilityInput::DeleteConnections)
        }

        CapabilityId::ValidateGraph => {
            arguments::decode(arguments, schema).map(CapabilityInput::ValidateGraph)
        }
        CapabilityId::ExecuteGraph => {
            arguments::decode(arguments, schema).map(CapabilityInput::ExecuteGraph)
        }
        CapabilityId::SaveGraph => Err(CapabilityFailure::new(
            CapabilityFailureCode::InvalidRequest,
        )),
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
        CapabilityId::InsertRows => {
            "Insert and initialize database rows atomically. Supply values by existing column name; omitted values become null. Optionally insert before a stable beforeRowId, otherwise append. Returns actual row IDs in input order; the whole batch is one undo step."
        }
        CapabilityId::UpdateCells => {
            "Update database cells atomically using stable rowId and column names. Each pair occurs once; null clears a cell. Every value must satisfy its existing physical type and semantic constraints. A rejected item leaves the entire batch unchanged."
        }
        CapabilityId::CreateColumns => {
            "Atomically create named nullable columns with canonical physical types. Existing rows start with null in each new column. Any invalid type or duplicate name rejects the whole batch; one undo unit."
        }
        CapabilityId::RenameColumns => {
            "Atomically rename existing columns while preserving their identities and values. Validate all final names together; name swaps are allowed, duplicate final names are rejected. One undo unit."
        }
        CapabilityId::DeleteColumns => {
            "Atomically delete the selected existing columns. The dataset must retain at least one user column. One undo restores the whole batch and exact prior values."
        }
        CapabilityId::CastColumns => {
            "Atomically convert selected columns to canonical physical types, preserving row and column identities and compatible semantic constraints. force must be explicit for each column: false rejects lossy values; true allows unrepresentable values to become null. Any failure rejects the entire batch; one undo restores exact original data."
        }
        CapabilityId::SetColumnSemantics => {
            "Atomically set the declared business semantics of selected columns. Validate existing values and domains across every column before committing metadata. Physical values and types remain intact; one undo unit."
        }
        CapabilityId::DeleteRows => {
            "Delete database rows by stable rowIds from a read or insert receipt. Never use display row numbers from a sorted or filtered page. The batch commits once and forms one undo step."
        }
        CapabilityId::FindConstants => {
            "Find named constants in one graph, with optional case-insensitive name search and paging. Returns real constant IDs, types and bounded previews, without loading table values into the conversation."
        }
        CapabilityId::InspectConstants => {
            "Read exact constant IDs using the shared typed value contract. Page text by Unicode characters, lists/objects by items, and tables by rows and columns. valuePath navigates nested values. Use returned nextOffset; pages are fragments, never complete replacement values. Does not execute nodes."
        }
        CapabilityId::CreateConstants => {
            "Atomically create named typed constants and optional initial reference nodes. Each declaration has a clientId; returns createdConstants and createdNodes mappings. Reuse the DataValue/ValueType schema; table values use tabular.columns. One saved, undoable graph batch."
        }
        CapabilityId::UpdateConstants => {
            "Atomically update exact constants. Omitted fields stay unchanged; an explicit value replaces type and value together. Graph validates values and reference effects before committing. Does not execute nodes."
        }
        CapabilityId::DeleteConstants => {
            "Delete exact constants in one saved, undoable graph batch. Existing reference nodes remain and report missing-constant diagnostics; the receipt reports affected nodes and removed constant IDs."
        }
        CapabilityId::SearchKnowledge => {
            "Search statistical knowledge when background, methods or interpretation guidance is needed. Returns excerpts and passage references. Refine queries or scopes; read a passage before citing it. Sources never substitute for this project's computed results."
        }
        CapabilityId::ReadKnowledge => {
            "Read a previously discovered knowledge passage. The host checks access and currentness and records its citation. If unavailable, search again."
        }
        CapabilityId::InspectResource => {
            "Inspect exact resource identity, name and saved/dirty state without document, tree, row or graph content. Function resources retain their existing signature inspection. Use each domain's read tools for contents."
        }
        CapabilityId::CreateResource => {
            "Create an authorized graph, chart, mind map or Markdown document by name. Returns the actual resource identity. Databases use import_database."
        }
        CapabilityId::ImportDatabase => {
            "Import an authorized Database from an explicit CSV, Parquet, Excel or SQL source. Returns the actual database identity. Check an uncertain outcome before retrying."
        }
        CapabilityId::RenameResource => {
            "Rename an authorized resource. Continue with the exact resource identity returned in the receipt; never infer the new path."
        }
        CapabilityId::DuplicateResource => {
            "Duplicate an authorized resource with an optional name, allocating new identities and the name in one owner transaction. Returns the actual new resource and its state."
        }
        CapabilityId::DeleteResource => {
            "Delete an explicitly authorized resource. The receipt reports actual deletions and reference effects."
        }
        CapabilityId::SaveResource => {
            "Save an authorized resource through its owner. Doc and Mind edits require this after the final edit; graph edit batches and chart updates already persist."
        }
        CapabilityId::UndoResource => {
            "Undo one committed history unit in an authorized Graph or Database. Returns the actual resource and dirty state; Graph undo does not save automatically."
        }
        CapabilityId::RedoResource => {
            "Redo one available history unit in an authorized Graph or Database. A new edit can clear redo history. Returns the actual resource and dirty state; use save_resource when an explicit save is needed."
        }
        CapabilityId::EditResource => {
            "Edit an existing function signature after inspecting it with inspect_resource. Preserve other function content. On conflict, return to the Manager to reassess the changed task."
        }
        CapabilityId::ExportDatabase => {
            "Export an authorized Database to the requested CSV or Parquet path. The host binds the captured resource. Returns the actual destination after publication; uncertain outcomes must be checked before retrying."
        }
        CapabilityId::InspectUiIntent => {
            "Inspect a workbench intent receipt by ID: pending, claimed, applied, failed or expired."
        }
        CapabilityId::RequestUiIntent => {
            "Request opening a resource with exact {kind,id}, a retained result using its complete resultRef, or an allowed panel. nodeId optionally focuses a graph node. A pending/claimed receipt only acknowledges acceptance; inspect its ID before claiming completion. Does not edit or save data."
        }
        CapabilityId::InspectGraph => {
            "Read an exact graph's counts, readiness, execution status and size-bounded overview. overview.detail=configuration includes all nodes, effective parameters, literal overrides and connections; topology omits parameters/literals; counts omits structure. Omitted information is unknown, never empty. Reuse this overview, then use find_nodes, inspect_nodes and find_connections only for needed missing details, validate_graph for diagnostics and result tools for runtime outputs. Does not execute, edit or save."
        }
        CapabilityId::FindNodes => {
            "Find existing nodes in an exact graph resource. Filter by case-insensitive name/title/ID query, exact typeIds or nodeIds. Returns identities and short configuration status without parameters, ports or values. Follow page.nextOffset when more matches are needed."
        }
        CapabilityId::InspectNodes => {
            "Read selected nodeIds from one captured graph. Choose parameters, ports, schema or options; parameters and ports are the default. Port and schema column pages apply independently to each selected node/port. Follow their returned page.nextOffset. schemaKnown:false means unknown, never an empty DataFrame. Does not execute or return runtime values."
        }
        CapabilityId::FindConnections => {
            "Find existing graph connections, optionally incident to specified nodeIds and exact ports. Returns connection IDs and exact endpoints. Node and port filters intersect; pagination follows filtering. Does not change or execute the graph."
        }
        CapabilityId::BrowseNodes => {
            "Find creatable node types by names, aliases or technical terms, optionally filtered by category. Returns typeId, title, summary and resource bindings without configuration schemas. Follow page.nextOffset for additional matches. Query exact definitions with inspect_node_type."
        }
        CapabilityId::InspectNodeType => {
            "Read known exact typeIds together in one call; no prior browse_nodes call is required. Reuse definitions already read. Returns configurationSchema, parameter defaults and fixed/configurable/derived pin declarations. Configure parameters and portCounts together when creating a node. Missing runtime requirements can remain incomplete until execution; null resets defaults. x-yss-requiredForExecution and x-yss-activeWhen describe runtime and conditional requirements. This query needs no graph, connections or execution."
        }
        CapabilityId::InspectDatabase => {
            "Inspect database counts, name and editing state without rows or the full schema."
        }
        CapabilityId::InspectDatabaseSchema => {
            "Inspect selected database columns with type and semantic metadata. Empty columns selects all, then offset/limit page the schema."
        }
        CapabilityId::ProfileDatabase => {
            "Profile only the named database columns and requested metric groups: completeness, statistics or distribution."
        }
        CapabilityId::InspectMind => {
            "Read a bounded topic outline and counts, scoped to an optional root and relative depth. Does not return full topic bodies. Follow page.nextOffset."
        }
        CapabilityId::FindTopics => {
            "Find case-insensitive literal topic content within an optional subtree. Returns summaries and ancestor IDs; pathComplete is false for paths longer than 32 IDs."
        }
        CapabilityId::InspectTopics => {
            "Read selected topic bodies, references, and paged child IDs. Content offsets count Unicode characters; follow each contentPage and childrenPage independently."
        }
        CapabilityId::CreateTopics => {
            "Create an atomic tree batch. Each clientId becomes a fresh topic ID; parentId can refer to $clientId anywhere in this call. References may point to unavailable resources. Returns actual IDs and dirty state; save_resource persists."
        }
        CapabilityId::UpdateTopics => {
            "Atomically update selected topic content or references. Omit a reference to retain it; null clears it. Save after the final edit."
        }
        CapabilityId::MoveTopics => {
            "Atomically move topics to parentId, optionally before a sibling ID. The final tree must have one unchanged root, existing parents and no cycles. Save after editing."
        }
        CapabilityId::DeleteTopics => {
            "Delete selected topics and their entire subtrees atomically. Returns all actual deleted topic IDs. Delete the resource to remove its root. Save after editing."
        }
        CapabilityId::DuplicateTopics => {
            "Copy disjoint subtrees under parentId with fresh identities, preserving content and external references. Returns the original-to-copy mapping for every copied topic. Save after editing."
        }
        CapabilityId::InspectDocument => {
            "Inspect Markdown length and a paged outline without reading its body. Returns exact section references that distinguish repeated titles. Omit section to refresh the outline after edits."
        }
        CapabilityId::ReadDocument => {
            "Read a section, an absolute Unicode range, or the whole document in pages. Defaults to 8192 characters, maximum 16384. Pages may end earlier at block boundaries. Follow returned page.nextOffset in the same scope; a fragment is not the complete document."
        }
        CapabilityId::SearchDocument => {
            "Search exact case-sensitive Markdown text within an optional section or Unicode range. Includes overlapping matches, exact locations and limited context. Use these passages to build a unique replace_document_text request."
        }
        CapabilityId::ReplaceDocumentText => {
            "Apply exact text replacements in order to one candidate. Every oldText must be nonempty and occur exactly once at that step; zero or multiple matches reject the whole batch. Preserves unread content. Save after the final edit."
        }
        CapabilityId::AppendDocument => {
            "Append exact Markdown to the current document, preserving the body. Include required newline separators in text. Returns actual changed Unicode ranges and dirty state. Save after editing."
        }
        CapabilityId::WriteDocument => {
            "Replace the complete Markdown or initialize an empty document. Supply the entire intended body, never just a read fragment. The file owner enforces its existing size limit independently of read page size. Save after editing."
        }
        CapabilityId::InspectChart => {
            "Read the chart configuration and data source. Supported configurable fields and chart types are declared in update_chart's schema; this does not calculate or preview data."
        }
        CapabilityId::UpdateChart => {
            "Update only specified chart settings and persist immediately. Omitted fields remain unchanged; null clears an axis. An empty databaseId disconnects the source. Chart types and encodings are limited to this schema."
        }
        CapabilityId::ReadDatabaseRows => {
            "Read selected columns with stable rowIds and a page continuation. All filters are ANDed. Order uses owner semantics, then stable row order. Use returned rowIds for edits; never use page positions."
        }
        CapabilityId::InspectDatasetSchema => {
            "Read dataset columns, physical types, semantic annotations and nullability. The host captures data currentness."
        }
        CapabilityId::InspectDatasetProfile => {
            "Read dataset quality/profile facts. Null metrics mean unknown or not computed, never zero."
        }
        CapabilityId::InspectResult => {
            "Inspect a complete resultRef copied from a receipt. Returns scalar values or a structural overview with complete tableRef values; never reads a table's rows. Tabular schema is paged with schemaOffset/schemaLimit. Validity distinguishes current values, stale pins and retained historical results. Queries never recompute."
        }
        CapabilityId::ReadResultTable => {
            "Read rows from a complete tableRef copied from an overview or nested table page. Select exact columns or page the schema with columnOffset/columnLimit; rows use offset/limit. Follow the actual returned page. Never construct a reference or table part. Queries never recompute."
        }
        CapabilityId::ListResources => {
            "List project resources, including closed files. Filter by kinds and case-insensitive name/path query; limit is 1..100, default 100. Follow page.nextOffset only for additional needed entries. Returns exact resource {kind,id} and displayName without reading contents or data rows."
        }
        CapabilityId::CreateNodes => {
            "Create and save nodes with parameters, configurable portCounts, labels and optional positions, plus initial connections, in one atomic batch. Use exact typeId and optional resourcePath from node discovery; constantId inserts an existing constant's yssbi.constant.get reference. clientId aliases use $clientId and initial pin instances use $clientId.templateKey[0]. Receipts return actual IDs and pins for subsequent calls. Nothing is created if any configuration or connection fails."
        }
        CapabilityId::UpdateNodes => {
            "Update selected nodes in one saved, undoable batch. Parameter keys merge; null resets a parameter. Omitted labels remain unchanged; null clears a label. Literal value:null clears the pin override. portCounts changes configurable templates only, retains earliest existing members and removes trailing members with their connections. Alternatively removePins deletes exact configurable pin identities; do not combine it with portCounts. No calculation runs implicitly."
        }
        CapabilityId::DeleteNodes => {
            "Delete selected nodes and their incident connections in one saved, undoable batch. Reports actual removals; managed function nodes remain protected."
        }
        CapabilityId::DuplicateNodes => {
            "Duplicate selected nodes and only their internal connections, with an optional position offset. Saves once, preserves original nodes, and returns the newly created node and pin identities."
        }
        CapabilityId::MoveNodes => {
            "Move the specified nodes to exact positions in one saved layout edit. Does not change values or execute nodes."
        }
        CapabilityId::CreateConnections => {
            "Create connections between exact returned pin addresses in one saved, atomic batch. Returns actual connection IDs and any replacements required by owner capacity rules. Does not execute."
        }
        CapabilityId::UpdateConnections => {
            "Reconnect selected existing connectionIds to exact output/input pins atomically, retaining connection identities. Endpoint swaps are supported. Omit order to retain it, or use null to clear it. Competing updates or an invalid endpoint reject the whole batch."
        }
        CapabilityId::DeleteConnections => {
            "Delete the exact connectionIds in one saved, undoable batch. Nodes remain present; no computation is triggered."
        }
        CapabilityId::ApplyGraphEdit => "Historical operation; unavailable as a model tool.",
        CapabilityId::ValidateGraph => {
            "Read readiness and paged diagnostics for a graph or selected nodeIds and their execution dependencies. Unrelated branch failures do not block selected scopes. Readiness uses all relevant diagnostics even when a page is empty. This optional check does not save or execute; missing runtime inputs still require explicit execution."
        }
        CapabilityId::ExecuteGraph => {
            "Execute graph; omit nodeId and mode to rerun the whole graph. For a selected nodeId, mode:currentInputs consumes current inputs without rerunning upstream; mode:dependencies (default) computes needed dependencies. Unrelated incomplete branches do not block node scope. The host binds the observed graph and prepares its plan. Check actual status and failures. Use returned result references directly with inspect_result. resultCount is null on failure; resultsComplete:false means references are partial. Does not save."
        }
        CapabilityId::SaveGraph => "Historical operation; unavailable as a model tool.",
        CapabilityId::ListGraphResults => {
            "List results for graph with optional runId, nodeIds or exact outputs and pagination. Without runId, returns current pins including stale values; a selected run returns only still-retained values. Check validity and runStatus. Copy complete resultRef values to inspect_result; queries do not execute."
        }
    }
}

#[cfg(test)]
mod tests;
