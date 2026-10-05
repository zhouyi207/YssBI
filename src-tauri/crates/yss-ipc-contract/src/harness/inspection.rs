use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use yss_harness_contract::{
    AutomationCapabilityRequest as Request, AutomationCapabilityResult as Result,
    GraphResultReference, ManageResourceRequest, ResourceChange, ToolInvocationRecord,
};

/// Presentation reads the existing ledger; row values, document bodies and credentials
/// never become tool-card parameters or a second copy of the model's context.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessToolInspectionDto {
    pub target: Option<String>,
    pub parameters: BTreeMap<String, Value>,
    pub artifacts: Vec<ResourceChange>,
    pub results: Vec<HarnessResultReferenceDto>,
    pub started_at: u64,
    pub finished_at: Option<u64>,
}

impl From<ToolInvocationRecord> for HarnessToolInspectionDto {
    fn from(record: ToolInvocationRecord) -> Self {
        let mut dto = Self {
            target: None,
            parameters: BTreeMap::new(),
            artifacts: Vec::new(),
            results: Vec::new(),
            started_at: record.started_at.get(),
            finished_at: record.finished_at.map(|time| time.get()),
        };
        match &record.request {
            Request::InspectGraph(request) => {
                dto.target = Some(request.graph_path.clone());
                dto.parameters.insert("view".into(), json!(request.view));
                dto.parameters
                    .insert("offset".into(), json!(request.offset));
                dto.parameters.insert("limit".into(), json!(request.limit));
                if !request.node_ids.is_empty() {
                    dto.parameters
                        .insert("nodeCount".into(), json!(request.node_ids.len()));
                }
            }
            Request::ApplyGraphEdit(request) => {
                dto.target = Some(request.graph_path.clone());
                dto.parameters
                    .insert("operationCount".into(), json!(request.operations.len()));
            }
            Request::ExecuteGraph(request) => {
                dto.target = Some(request.graph_path.clone());
                dto.parameters
                    .insert("demand".into(), json!(request.demand));
            }
            Request::ValidateGraph(request) => dto.target = Some(request.graph_path.clone()),
            Request::SaveGraph(request) => dto.target = Some(request.graph_path.clone()),
            Request::ListGraphResults(request) => dto.target = Some(request.graph_path.clone()),
            Request::InspectResource(request) => {
                dto.target = Some(request.resource.id.clone());
                dto.parameters
                    .insert("offset".into(), json!(request.offset));
                dto.parameters.insert("limit".into(), json!(request.limit));
            }
            Request::EditResource(request) => {
                dto.target = Some(request.resource.id.clone());
                dto.parameters.insert("operation".into(), json!("edit"));
            }
            Request::ExportDataset(request) => dto.target = Some(request.resource.id.clone()),
            Request::InspectDatasetSchema(request) => {
                dto.target = Some(request.database_id.clone())
            }
            Request::InspectDatasetProfile(request) => {
                dto.target = Some(request.database_id.clone())
            }
            Request::SearchNodeCatalog(request) => {
                dto.parameters.insert("query".into(), json!(request.query));
                dto.parameters.insert("limit".into(), json!(request.limit));
            }
            Request::SearchKnowledge(request) => {
                dto.parameters.insert("query".into(), json!(request.query));
                dto.parameters.insert("limit".into(), json!(request.limit));
            }
            Request::ReadKnowledge(request) => {
                dto.target = Some(request.reference.document_id.to_string());
            }
            Request::InspectResult(request) => {
                dto.parameters
                    .insert("resultId".into(), json!(request.result_id.to_string()));
                dto.parameters
                    .insert("offset".into(), json!(request.offset));
                dto.parameters.insert("limit".into(), json!(request.limit));
            }
            Request::ManageResource(request) => {
                let (operation, resource) = match request {
                    ManageResourceRequest::Create { .. } => ("create", None),
                    ManageResourceRequest::Rename { resource, .. } => ("rename", Some(resource)),
                    ManageResourceRequest::Duplicate { resource, .. } => {
                        ("duplicate", Some(resource))
                    }
                    ManageResourceRequest::Delete { resource, .. } => ("delete", Some(resource)),
                    ManageResourceRequest::Save { resource, .. } => ("save", Some(resource)),
                };
                dto.parameters.insert("operation".into(), json!(operation));
                dto.target = resource.map(|resource| resource.id.clone());
            }
            Request::InspectUiIntent(_)
            | Request::RequestUiIntent(_)
            | Request::InspectProject(_) => {}
        }
        match record.result {
            Some(Result::KnowledgeSearch(result)) => {
                dto.parameters
                    .insert("resultCount".into(), json!(result.matches.len()));
            }
            Some(Result::KnowledgePassage(passage)) => dto.target = Some(passage.title),
            Some(Result::ResourceManaged(receipt) | Result::ResourceEdited(receipt)) => {
                dto.artifacts = receipt.changes;
                dto.parameters
                    .insert("resourceCount".into(), json!(dto.artifacts.len()));
            }
            Some(Result::GraphExecution(result)) => {
                dto.parameters
                    .insert("resultCount".into(), json!(result.result_count));
                dto.parameters
                    .insert("executionStatus".into(), json!(result.status));
                dto.results = result
                    .results
                    .into_iter()
                    .map(HarnessResultReferenceDto::from)
                    .collect();
            }
            Some(Result::GraphResults(result)) => {
                dto.parameters
                    .insert("resultCount".into(), json!(result.results.len()));
                dto.results = result
                    .results
                    .into_iter()
                    .map(HarnessResultReferenceDto::from)
                    .collect();
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
pub struct HarnessResultReferenceDto {
    pub execution_session_id: String,
    pub result_id: String,
    pub output: String,
}

impl From<GraphResultReference> for HarnessResultReferenceDto {
    fn from(value: GraphResultReference) -> Self {
        Self {
            execution_session_id: value.execution_session_id,
            result_id: value.result_id.to_string(),
            output: value.output,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_harness_contract::{DatasetImportSource, DatasetSqlEngine, ResourceCreation};

    #[test]
    fn tool_inspection_projects_operations_without_copying_private_inputs() {
        let request = Request::ManageResource(ManageResourceRequest::Create {
            specification: ResourceCreation::Database {
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
            "capabilityId": "manage_resource", "request": request, "state": "running", "result": null, "failure": null,
            "startedAt": 1000, "deadline": 31000, "finishedAt": null,
        })).unwrap();
        let dto = serde_json::to_value(HarnessToolInspectionDto::from(record)).unwrap();
        assert_eq!(dto["parameters"], json!({ "operation": "create" }));
        assert_eq!(dto["startedAt"], 1000);
        assert!(!dto.to_string().contains("private"));
        let result = HarnessResultReferenceDto::from(GraphResultReference {
            execution_session_id: "execution-1".into(),
            result_id: u64::MAX,
            run_id: 1,
            output: "node:result".into(),
            category: yss_harness_contract::ResultCategoryInspection::Value,
        });
        assert_eq!(
            serde_json::to_value(result).unwrap()["resultId"],
            "18446744073709551615"
        );
    }
}
