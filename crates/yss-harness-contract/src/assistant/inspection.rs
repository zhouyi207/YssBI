//! Tool-ledger read projections with explicit field filtering and exact result identities.
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use crate::{
    AutomationCapabilityRequest as Request, AutomationCapabilityResult as Result,
    GraphResultReference, ManageResourceRequest, ResourceChange, ToolInvocationRecord,
};

/// Presentation reads the existing ledger and control events; row values, document bodies and credentials
/// never become tool-card parameters or a second copy of the model's context.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantToolInspection {
    pub target: Option<String>,
    pub parameters: BTreeMap<String, Value>,
    pub artifacts: Vec<ResourceChange>,
    pub results: Vec<AssistantResultReference>,
    pub started_at: u64,
    pub finished_at: Option<u64>,
    pub failure: Option<Value>,
}

impl AssistantToolInspection {
    pub fn from_control_events(
        events: &[crate::HarnessEventEnvelope],
        id: &crate::ToolInvocationId,
    ) -> Option<Self> {
        use crate::{AgentEvent, HarnessEvent};
        let mut inspection = None;
        for envelope in events {
            let event = match &envelope.event {
                HarnessEvent::Agent(event) | HarnessEvent::AgentRunOutput { event, .. } => event,
                _ => continue,
            };
            match event {
                AgentEvent::ControlToolStarted { invocation_id, .. } if invocation_id == id => {
                    inspection = Some(Self {
                        target: None,
                        parameters: BTreeMap::new(),
                        artifacts: vec![],
                        results: vec![],
                        started_at: envelope.occurred_at.get(),
                        finished_at: None,
                        failure: None,
                    });
                }
                AgentEvent::ControlToolFinished {
                    invocation_id,
                    failure_code,
                    failure_details,
                    ..
                } if invocation_id == id => {
                    if let Some(value) = &mut inspection {
                        value.finished_at = Some(envelope.occurred_at.get());
                        value.failure = failure_code.map(|code| {
                            crate::model::failure(
                                &crate::CapabilityFailure {
                                    code,
                                    details: failure_details.clone().unwrap_or_default(),
                                },
                            )
                        });
                    }
                }
                _ => {}
            }
        }
        inspection
    }
}

impl From<ToolInvocationRecord> for AssistantToolInspection {
    fn from(record: ToolInvocationRecord) -> Self {
        let mut dto = Self {
            target: None,
            parameters: BTreeMap::new(),
            artifacts: Vec::new(),
            results: Vec::new(),
            started_at: record.started_at.get(),
            finished_at: record.finished_at.map(|time| time.get()),
            failure: record
                .failure
                .as_ref()
                .map(crate::model::failure),
        };
        if let Some(request) = record.request.bound() {
            match request {
                Request::FindConstants(request) => {
                    dto.target = Some(request.graph.id.clone());
                    dto.parameters.insert("query".into(), json!(request.query));
                    dto.parameters
                        .insert("offset".into(), json!(request.offset));
                    dto.parameters.insert("limit".into(), json!(request.limit));
                }
                Request::InspectConstants(request) => {
                    dto.target = Some(request.graph.id.clone());
                    dto.parameters
                        .insert("constantCount".into(), json!(request.constant_ids.len()));
                    dto.parameters
                        .insert("offset".into(), json!(request.offset));
                    dto.parameters.insert("limit".into(), json!(request.limit));
                }
                Request::InspectGraph(request) => {
                    dto.target = Some(request.graph.id.clone());
                }

                Request::FindNodes(request) => {
                    dto.target = Some(request.graph.id.clone());
                    dto.parameters.insert("query".into(), json!(request.query));
                    dto.parameters
                        .insert("typeIds".into(), json!(request.type_ids));
                    dto.parameters
                        .insert("offset".into(), json!(request.offset));
                    dto.parameters.insert("limit".into(), json!(request.limit));
                }
                Request::InspectNodes(request) => {
                    dto.target = Some(request.graph.id.clone());
                    dto.parameters
                        .insert("nodeCount".into(), json!(request.node_ids.len()));
                    dto.parameters
                        .insert("fields".into(), json!(request.fields));
                }
                Request::FindConnections(request) => {
                    dto.target = Some(request.graph.id.clone());
                    dto.parameters
                        .insert("nodeCount".into(), json!(request.node_ids.len()));
                    dto.parameters
                        .insert("portCount".into(), json!(request.ports.len()));
                    dto.parameters
                        .insert("offset".into(), json!(request.offset));
                    dto.parameters.insert("limit".into(), json!(request.limit));
                }
                Request::ApplyGraphEdit(request) => {
                    dto.target = Some(request.graph_path.clone());
                    dto.parameters
                        .insert("operationCount".into(), json!(request.operations.len()));
                }
                Request::GraphMutation(request) => {
                    dto.target = Some(request.input.graph().id.clone());
                    dto.parameters
                        .insert("itemCount".into(), json!(request.input.item_count()));
                }
                Request::ExecuteGraph(request) => {
                    dto.target = Some(request.graph.id.clone());
                    dto.parameters
                        .insert("demand".into(), json!(request.demand));
                }
                Request::ValidateGraph(request) => {
                    dto.target = Some(request.graph.id.clone());
                    dto.parameters
                        .insert("nodeIds".into(), json!(request.node_ids));
                    dto.parameters
                        .insert("offset".into(), json!(request.offset));
                    dto.parameters.insert("limit".into(), json!(request.limit));
                }
                Request::SaveGraph(request) => dto.target = Some(request.graph_path.clone()),
                Request::ListGraphResults(request) => dto.target = Some(request.graph.id.clone()),
                Request::InspectResource(request) => dto.target = Some(request.resource.id.clone()),
                Request::EditResource(request) => {
                    dto.target = Some(request.resource.id.clone());
                    let operation = match request.capability_id() {
                        crate::CapabilityId::UndoResource => "undo",
                        crate::CapabilityId::RedoResource => "redo",
                        crate::CapabilityId::InsertRows => "insert_rows",
                        crate::CapabilityId::UpdateCells => "update_cells",
                        crate::CapabilityId::DeleteRows => "delete_rows",
                        crate::CapabilityId::CreateColumns => "create_columns",
                        crate::CapabilityId::RenameColumns => "rename_columns",
                        crate::CapabilityId::DeleteColumns => "delete_columns",
                        crate::CapabilityId::CastColumns => "cast_columns",
                        crate::CapabilityId::SetColumnSemantics => {
                            "set_column_semantics"
                        }
                        _ => "edit",
                    };
                    dto.parameters.insert("operation".into(), json!(operation));
                    use crate::ResourceEdit;
                    if let ResourceEdit::UpdateChart { settings } = &request.edit {
                        dto.parameters.insert("settings".into(), json!(settings));
                    }
                    let item_count = match &request.edit {
                        ResourceEdit::InsertRows { rows, .. } => Some(rows.len()),
                        ResourceEdit::UpdateCells { cells } => Some(cells.len()),
                        ResourceEdit::DeleteRows { row_ids } => Some(row_ids.len()),
                        ResourceEdit::CreateColumns { columns } => Some(columns.len()),
                        ResourceEdit::RenameColumns { columns } => Some(columns.len()),
                        ResourceEdit::ReplaceDocumentText { replacements } => {
                            Some(replacements.len())
                        }
                        ResourceEdit::CreateTopics { topics, .. } => Some(topics.len()),
                        ResourceEdit::UpdateTopics { topics } => Some(topics.len()),
                        ResourceEdit::MoveTopics { topics } => Some(topics.len()),
                        ResourceEdit::DeleteTopics { topic_ids }
                        | ResourceEdit::DuplicateTopics { topic_ids, .. } => Some(topic_ids.len()),
                        ResourceEdit::DeleteColumns { columns } => Some(columns.len()),
                        ResourceEdit::CastColumns { columns } => Some(columns.len()),
                        ResourceEdit::SetColumnSemantics { columns } => Some(columns.len()),
                        _ => None,
                    };
                    if let Some(count) = item_count {
                        dto.parameters.insert("itemCount".into(), json!(count));
                    }
                }
                Request::InspectChart(request) => dto.target = Some(request.chart.id.clone()),
                Request::ReadDocument(request) => {
                    dto.target = Some(request.input.document().id.clone());
                    dto.parameters
                        .insert("section".into(), json!(request.input.section()));
                    dto.parameters
                        .insert("range".into(), json!(request.input.range()));
                    use crate::model::DocumentReadInput;
                    let (offset, limit) = match &request.input {
                        DocumentReadInput::Outline(value) => (value.offset, value.limit),
                        DocumentReadInput::Text(value) => (value.offset, value.limit),
                        DocumentReadInput::Search(value) => {
                            dto.parameters.insert("query".into(), json!(value.query));
                            (value.offset, value.limit)
                        }
                    };
                    dto.parameters.insert("offset".into(), json!(offset));
                    dto.parameters.insert("limit".into(), json!(limit));
                }
                Request::ReadMind(request) => {
                    dto.target = Some(request.input.mind().id.clone());
                    use crate::model::MindReadInput;
                    match &request.input {
                        MindReadInput::Outline(value) => {
                            dto.parameters
                                .insert("rootTopicId".into(), json!(value.root_topic_id));
                            dto.parameters.insert("depth".into(), json!(value.depth));
                            dto.parameters.insert("offset".into(), json!(value.offset));
                            dto.parameters.insert("limit".into(), json!(value.limit));
                        }
                        MindReadInput::Find(value) => {
                            dto.parameters.insert("query".into(), json!(value.query));
                            dto.parameters
                                .insert("rootTopicId".into(), json!(value.root_topic_id));
                            dto.parameters.insert("offset".into(), json!(value.offset));
                            dto.parameters.insert("limit".into(), json!(value.limit));
                        }
                        MindReadInput::Topics(value) => {
                            dto.parameters
                                .insert("topicIds".into(), json!(value.topic_ids));
                            dto.parameters
                                .insert("contentOffset".into(), json!(value.content_offset));
                            dto.parameters
                                .insert("contentLimit".into(), json!(value.content_limit));
                            dto.parameters
                                .insert("childrenOffset".into(), json!(value.children_offset));
                            dto.parameters
                                .insert("childrenLimit".into(), json!(value.children_limit));
                        }
                    }
                }
                Request::ReadDatabase(request) => {
                    dto.target = Some(request.input.database().id.clone());
                    use crate::model::DatabaseReadInput;
                    match &request.input {
                        DatabaseReadInput::Overview(_) => {}
                        DatabaseReadInput::Schema(v) => {
                            dto.parameters.insert("columns".into(), json!(v.columns));
                            dto.parameters.insert("offset".into(), json!(v.offset));
                            dto.parameters.insert("limit".into(), json!(v.limit));
                        }
                        DatabaseReadInput::Profile(v) => {
                            dto.parameters.insert("columns".into(), json!(v.columns));
                            dto.parameters.insert("metrics".into(), json!(v.metrics));
                        }
                        DatabaseReadInput::Rows(v) => {
                            dto.parameters.insert("columns".into(), json!(v.columns));
                            dto.parameters.insert("filters".into(), json!(v.filters));
                            dto.parameters.insert("order".into(), json!(v.order));
                            dto.parameters.insert("offset".into(), json!(v.offset));
                            dto.parameters.insert("limit".into(), json!(v.limit));
                        }
                    }
                }
                Request::ExportDatabase(request) => dto.target = Some(request.resource.id.clone()),
                Request::InspectDatasetSchema(request) => {
                    dto.target = Some(request.database_id.clone())
                }
                Request::InspectDatasetProfile(request) => {
                    dto.target = Some(request.database_id.clone())
                }
                Request::BrowseNodes(request) => {
                    dto.parameters.insert("query".into(), json!(request.query));
                    dto.parameters.insert("limit".into(), json!(request.limit));
                }
                Request::InspectNodeType(request) => {
                    dto.parameters
                        .insert("typeIds".into(), json!(request.type_ids));
                }
                Request::SearchKnowledge(request) => {
                    dto.parameters.insert("query".into(), json!(request.query));
                    dto.parameters.insert("limit".into(), json!(request.limit));
                }
                Request::ReadKnowledge(request) => {
                    dto.target = Some(request.reference.document_id.to_string());
                }
                Request::InspectResult(request) => {
                    dto.parameters.insert(
                        "resultId".into(),
                        json!(request.result_ref.result_id().to_string()),
                    );
                    dto.parameters
                        .insert("schemaOffset".into(), json!(request.schema_offset));
                    dto.parameters
                        .insert("schemaLimit".into(), json!(request.schema_limit));
                }
                Request::ReadResultTable(request) => {
                    dto.parameters.insert(
                        "resultId".into(),
                        json!(request.table_ref.result_ref().result_id().to_string()),
                    );
                    dto.parameters
                        .insert("offset".into(), json!(request.offset));
                    dto.parameters.insert("limit".into(), json!(request.limit));
                    dto.parameters
                        .insert("columns".into(), json!(request.columns));
                }
                Request::ManageResource(request) => {
                    let (operation, resource) = match request {
                        ManageResourceRequest::Create { .. } => ("create", None),
                        ManageResourceRequest::Rename { resource, .. } => {
                            ("rename", Some(resource))
                        }
                        ManageResourceRequest::Duplicate { resource, .. } => {
                            ("duplicate", Some(resource))
                        }
                        ManageResourceRequest::Delete { resource, .. } => {
                            ("delete", Some(resource))
                        }
                        ManageResourceRequest::Save { resource, .. } => ("save", Some(resource)),
                    };
                    dto.parameters.insert("operation".into(), json!(operation));
                    dto.target = resource.map(|resource| resource.id.clone());
                }
                Request::InspectUiIntent(_)
                | Request::RequestUiIntent(_)
                | Request::ListResources(_) => {}
            }
        } else {
            dto.parameters
                .insert("argumentStatus".into(), json!("rejected"));
            if let Ok(arguments) = record.request.model_arguments() {
                dto.target = ["resource", "graph", "database", "mind", "document", "chart"]
                    .into_iter()
                    .find_map(|key| arguments.get(key)?.get("id")?.as_str().map(str::to_owned));
            }
        }
        match record.result {
            Some(Result::KnowledgeSearch(result)) => {
                dto.parameters
                    .insert("resultCount".into(), json!(result.matches.len()));
            }
            Some(Result::KnowledgePassage(passage)) => dto.target = Some(passage.title),
            Some(Result::ResourceManaged(receipt) | Result::ResourceEdited(receipt)) => {
                if let Some(edit) = receipt.document_edit {
                    dto.parameters
                        .insert("changedRanges".into(), json!(edit.changes.len()));
                    dto.parameters
                        .insert("characterCount".into(), json!(edit.character_count));
                    dto.parameters.insert("dirty".into(), json!(edit.dirty));
                }
                if let Some(edit) = receipt.mind_edit {
                    dto.parameters
                        .insert("createdTopics".into(), json!(edit.created_topics.len()));
                    dto.parameters.insert(
                        "affectedTopics".into(),
                        json!(edit.affected_topic_ids.len()),
                    );
                    dto.parameters
                        .insert("deletedTopics".into(), json!(edit.deleted_topic_ids.len()));
                    dto.parameters.insert("dirty".into(), json!(edit.dirty));
                }
                if let Some(edit) = receipt.database_edit {
                    dto.parameters
                        .insert("committedItems".into(), json!(edit.item_count));
                    dto.parameters.insert(
                        "insertedRowCount".into(),
                        json!(edit.inserted_row_ids.len()),
                    );
                    dto.parameters.insert("dirty".into(), json!(edit.dirty));
                }
                dto.artifacts = receipt.changes;
                dto.parameters
                    .insert("resourceCount".into(), json!(dto.artifacts.len()));
            }
            Some(Result::GraphExecution(result)) => {
                dto.parameters
                    .insert("resultCount".into(), json!(result.result_count));
                dto.parameters
                    .insert("executionStatus".into(), json!(result.status));
                dto.parameters
                    .insert("executionTiming".into(), json!(result.timing));
                dto.results = result
                    .results
                    .into_iter()
                    .map(AssistantResultReference::from)
                    .collect();
            }
            Some(Result::GraphResults(result)) => {
                dto.parameters
                    .insert("executionStatus".into(), json!(result.run_status));
                dto.parameters
                    .insert("executionTiming".into(), json!(result.run_timing));
                dto.parameters
                    .insert("resultCount".into(), json!(result.results.len()));
                dto.results = result
                    .results
                    .into_iter()
                    .map(AssistantResultReference::from)
                    .collect();
            }
            Some(Result::ResultInspection(result)) => {
                dto.parameters
                    .insert("validity".into(), json!(result.validity));
            }
            Some(Result::ResultTablePage(result)) => {
                dto.parameters
                    .insert("validity".into(), json!(result.validity));
                dto.parameters
                    .insert("rowCount".into(), json!(result.page.returned));
                dto.parameters
                    .insert("columnCount".into(), json!(result.columns.len()));
                dto.parameters
                    .insert("hasMore".into(), json!(result.page.has_more));
            }
            Some(Result::GraphValidation(result)) => {
                dto.parameters.insert("ready".into(), json!(result.ready));
                dto.parameters
                    .insert("diagnosticCount".into(), json!(result.diagnostics.len()));
            }
            Some(Result::GraphInspectionPage(result)) => {
                dto.parameters
                    .insert("totalNodes".into(), json!(result.counts.nodes));
                dto.parameters
                    .insert("connectionCount".into(), json!(result.counts.connections));
                dto.parameters.insert("ready".into(), json!(result.ready));
            }
            Some(Result::GraphInspection(result)) => {
                dto.parameters
                    .insert("totalNodes".into(), json!(result.nodes.len()));
                dto.parameters
                    .insert("connectionCount".into(), json!(result.connections.len()));
                dto.parameters.insert("ready".into(), json!(result.ready));
            }
            Some(Result::GraphEditReceipt(result)) => {
                dto.parameters
                    .insert("createdNodes".into(), json!(result.created_nodes.len()));
                dto.parameters
                    .insert("ready".into(), json!(result.changes.ready));
            }
            Some(Result::GraphSaved(result)) => {
                dto.parameters
                    .insert("savedRevision".into(), json!(result.resource_revision));
            }
            _ => {}
        }
        dto
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantResultReference {
    pub execution_session_id: String,
    pub result_id: String,
    pub output: String,
}

impl From<GraphResultReference> for AssistantResultReference {
    fn from(value: GraphResultReference) -> Self {
        Self {
            execution_session_id: value.result_ref.execution_session_id().into(),
            result_id: value.result_ref.result_id().to_string(),
            output: value.output,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn pre_execution_failure_details_keep_safe_targets_and_authoritative_timing() {
        let input = model::CapabilityInput::WriteDocument(model::WriteDocumentInput {
            document: model::DocumentResourceRef::new("docs/report.md".into()),
            markdown: "private document body".into(),
        });
        let base: ToolInvocationRecord = serde_json::from_value(json!({
            "id":"tool-1", "idempotencyKey":"tool-1", "sessionId":"session-1",
            "turnId":"turn-1", "agentRunId":null, "workflowRunId":null, "workflowStepId":null,
            "project":{"projectInstanceId":"project-1", "projectSessionId":"project-session-1"},
            "capabilityId":"write_document", "request":{"stage":"rejected", "capabilityId":"write_document", "input":null},
            "state":"failed", "result":null, "failure":{"code":"invalid_request", "details":{
                "category":"missing_field", "path":"$.markdown", "expected":"type=string",
                "expectedRevision":"private-version"
            }},
            "startedAt":1000, "deadline":31000, "finishedAt":1037
        })).unwrap();
        for input in [None, Some(input)] {
            let mut record = base.clone();
            let has_input = input.is_some();
            record.request = ToolInvocationRequest::Rejected {
                capability_id: CapabilityId::WriteDocument,
                input,
            };
            let dto = AssistantToolInspection::from(record);
            assert_eq!(dto.target.as_deref(), has_input.then_some("docs/report.md"));
            assert_eq!(dto.parameters["argumentStatus"], "rejected");
            assert_eq!(dto.finished_at.unwrap() - dto.started_at, 37);
            assert!(dto.artifacts.is_empty() && dto.results.is_empty());
            assert_eq!(
                dto.failure,
                Some(json!({
                    "code": "invalid_request", "details": {
                        "category": "missing_field", "path": "$.markdown", "expected": "type=string"
                    }
                }))
            );
            assert!(
                !serde_json::to_string(&dto)
                    .unwrap()
                    .contains("private document body")
            );
        }
    }

    #[test]
    fn tool_inspection_projects_operations_without_copying_private_inputs() {
        let request = Request::ManageResource(ManageResourceRequest::Create {
            specification: ResourceCreation::Database {
                name: None,
                source: DatasetImportSource::Sql {
                    engine: DatasetSqlEngine::Postgres { ssl: true },
                    connection_string: "postgres://private-password@host/db".into(),
                    table: "private_patient_records".into(),
                },
            },
        });
        let record: ToolInvocationRecord = serde_json::from_value(json!({
            "id": "tool-1", "idempotencyKey": "operation-1", "sessionId": "session-1", "turnId": "turn-1",
            "agentRunId": null, "workflowRunId": null, "workflowStepId": null,
            "project": { "projectInstanceId": "00000000-0000-0000-0000-000000000001", "projectSessionId": "00000000-0000-0000-0000-000000000002" },
            "capabilityId": "import_database", "request": ToolInvocationRequest::from(request), "state": "running", "result": null, "failure": null,
            "startedAt": 1000, "deadline": 31000, "finishedAt": null,
        })).unwrap();
        let dto = serde_json::to_value(AssistantToolInspection::from(record)).unwrap();
        assert_eq!(dto["parameters"], json!({ "operation": "create" }));
        assert_eq!(dto["startedAt"], 1000);
        assert!(!dto.to_string().contains("private"));
        let result = AssistantResultReference::from(GraphResultReference {
            result_ref: crate::ResultRef::new("execution-1".into(), u64::MAX),
            validity: crate::ResultValidity::Retained,
            run_id: 1,
            output: "node:result".into(),
            category: crate::ResultCategoryInspection::Value,
        });
        assert_eq!(
            serde_json::to_value(result).unwrap()["resultId"],
            "18446744073709551615"
        );
    }
}
