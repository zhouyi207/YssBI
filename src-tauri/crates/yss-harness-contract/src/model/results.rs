//! Live and replay projections. User values are never recursively filtered by key name.
use crate::*;
use serde_json::{Value, json};

pub fn capability_result(result: &AutomationCapabilityResult) -> Result<Value, serde_json::Error> {
    use AutomationCapabilityResult::*;
    let (kind, payload) = match result {
        ChartInspection(value) => (
            "chart_inspection",
            json!({"chart": value.chart, "settings": value.settings}),
        ),
        ResourceInspection(value) => ("resource_inspection", resource(value)),
        DocumentRead(value) => (
            "document_read",
            json!({"document": value.document, "dirty": value.dirty, "content": value.content}),
        ),
        MindRead(value) => (
            "mind_read",
            json!({"mind": value.mind, "dirty": value.dirty, "content": value.content}),
        ),
        DatabaseRead(value) => (
            "database_read",
            json!({ "database": value.database, "content": value.content }),
        ),
        ResourceManaged(value) => ("resource_managed", mutation(value)),
        ResourceEdited(value) => ("resource_edited", mutation(value)),
        GraphInspection(value) => ("graph_inspection", graph(value)),
        GraphInspectionPage(value) => ("graph_inspection_page", graph_page(value)),
        DatasetSchemaInspection(value) => ("dataset_schema_inspection", dataset_schema(value)),
        DatasetProfileInspection(value) => (
            "dataset_profile_inspection",
            json!({
                "databaseId": value.database_id, "rowCount": value.row_count,
                "columnCount": value.column_count, "estimatedMemoryBytes": value.estimated_memory_bytes,
                "duplicatedRows": value.duplicated_rows, "numericColumns": value.numeric_columns,
                "categoricalColumns": value.categorical_columns, "stringColumns": value.string_columns,
                "temporalColumns": value.temporal_columns, "booleanColumns": value.boolean_columns,
                "totalNulls": value.total_nulls, "nullRatio": value.null_ratio,
                "columnsWithNulls": value.columns_with_nulls, "rowsWithNulls": value.rows_with_nulls,
            }),
        ),
        ProjectInspection(value) => (
            "project_inspection",
            json!({
                "projectName": value.project_name,
                "page": value.page,
                "resources": value.resources.iter().map(|value| json!({
                    "resource": value.resource, "displayName": value.display_name,
                })).collect::<Vec<_>>(),
            }),
        ),
        GraphEditReceipt(value) => (
            "graph_edit_receipt",
            json!({
                "graphPath": value.graph_path, "createdNodes": value.created_nodes,
                "createdPorts": value.created_ports, "createdConstants": value.created_constants,
                "changes": {
                    "nodes": value.changes.nodes.iter().map(edited_node).collect::<Vec<_>>(),
                    "removedNodeIds": value.changes.removed_node_ids,
                    "connections": value.changes.connections, "removedConnectionIds": value.changes.removed_connection_ids,
                    "constants": constants(&value.changes.constants), "removedConstantIds": value.changes.removed_constant_ids,
                    "ready": value.changes.ready, "diagnostics": value.changes.diagnostics,
                },
            }),
        ),
        GraphValidation(value) => (
            "graph_validation",
            json!({
                "graph": GraphResourceRef::for_path(value.graph_path.clone()), "ready": value.ready,
                "nodeIds": value.node_ids, "scopeNodeCount": value.scope_node_count, "page": value.page, "diagnostics": value.diagnostics,
            }),
        ),
        GraphExecution(value) => (
            "graph_execution",
            json!({
                "graphPath": value.graph_path, "runId": value.run_id, "status": value.status,
                "timing": value.timing,
                "failureCode": value.failure_code, "failureLocation": value.failure_location,
                "resultCount": value.result_count, "resultsComplete": value.results_complete,
                "results": value.results,
            }),
        ),
        GraphSaved(value) => (
            "graph_saved",
            json!({
                "graphPath": value.graph_path, "dirty": value.dirty,
                "canUndo": value.can_undo, "canRedo": value.can_redo,
            }),
        ),
        DatabaseExported(value) => ("database_exported", serde_json::to_value(value)?),
        UiIntentInspection(value) => ("ui_intent_inspection", ui_intent(value)?),
        UiIntentReceipt(value) => ("ui_intent_receipt", ui_intent(value)?),
        NodeCatalogPage(value) => ("node_catalog_page", serde_json::to_value(value)?),
        NodeTypeInspection(value) => ("node_type_inspection", serde_json::to_value(value)?),
        KnowledgeSearch(value) => ("knowledge_search", serde_json::to_value(value)?),
        KnowledgePassage(value) => ("knowledge_passage", serde_json::to_value(value)?),
        ResultInspection(value) => ("result_inspection", serde_json::to_value(value)?),
        ResultTablePage(value) => ("result_table_page", serde_json::to_value(value)?),
        GraphResults(value) => ("graph_results", serde_json::to_value(value)?),
    };
    Ok(json!({ "type": kind, "payload": payload }))
}

fn ui_intent(value: &yss_ui_contract::UiIntentReceipt) -> Result<Value, serde_json::Error> {
    let intent =
        super::UiIntentInput::try_from(&value.intent).map_err(serde::ser::Error::custom)?;
    Ok(json!({"id": value.id, "status": value.status, "intent": intent}))
}

fn edited_node(value: &GraphNodeInspection) -> Value {
    // Receipts confirm committed facts. Repeated editor metadata and column
    // choices belong to targeted inspection, not every edit in model history.
    json!({
        "nodeId": value.node_id, "nodeTypeId": value.node_type_id,
        "userLabel": value.user_label, "x": value.x, "y": value.y,
        "parameters": value.parameters.iter().map(|parameter| json!({
            "key": parameter.key, "value": parameter.value,
        })).collect::<Vec<_>>(),
        "ports": value.ports.iter().map(|port| json!({
            "address": port.address, "direction": port.direction,
            "dataType": port.data_type, "orphan": port.orphan,
            "connectionCount": port.connection_count, "literal": port.literal,
        })).collect::<Vec<_>>(),
    })
}

fn resource(value: &ResourceInspection) -> Value {
    let content = match &value.content {
        ResourceContent::Metadata | ResourceContent::DatabaseMetadata { .. } => {
            json!({"kind":"metadata"})
        }
        ResourceContent::Function { signature } => {
            json!({"kind":"function", "signature":super::FunctionSignatureInput::from(signature)})
        }
    };
    json!({ "resource": value.resource, "name": value.name, "dirty": value.dirty, "content": content })
}

fn mutation(value: &ResourceMutationReceipt) -> Value {
    json!({
        "moves": value.moves, "mindEdit": value.mind_edit,
        "databaseEdit": value.database_edit, "documentEdit": value.document_edit,
        "resources": value.resources.iter().map(|state| json!({
            "resource": state.resource, "name": state.name, "dirty": state.dirty,
            "rootTopicId": state.root_topic_id,
        })).collect::<Vec<_>>(),
        "changes": value.changes.iter().map(|change| json!({
            "resource": change.resource, "deleted": change.deleted,
        })).collect::<Vec<_>>(),
    })
}

fn dataset_schema(value: &DatasetSchemaInspection) -> Value {
    json!({ "databaseId": value.database_id, "columns": value.columns })
}

fn graph(value: &GraphInspection) -> Value {
    json!({
        "graphPath": value.graph_path, "ready": value.ready, "nodes": value.nodes,
        "connections": value.connections, "constants": constants(&value.constants), "diagnostics": value.diagnostics,
    })
}

fn graph_page(value: &GraphInspectionPage) -> Value {
    if value.view == GraphInspectionView::Summary {
        return json!({
            "graph": GraphResourceRef::for_path(value.graph_path.clone()),
            "ready": value.ready, "counts": value.counts, "runs": value.runs,
            "overview": value.overview,
        });
    }
    let content = match &value.content {
        GraphInspectionItems::Constants(values) => {
            json!({ "kind": "constants", "items": constants(values) })
        }
        other => json!(other),
    };
    let returned = match &value.content {
        GraphInspectionItems::Summary | GraphInspectionItems::Unchanged => 0,
        GraphInspectionItems::Nodes(items) => items.len(),
        GraphInspectionItems::NodeDetails(items) => items.len(),
        GraphInspectionItems::Ports(items) => items.len(),
        GraphInspectionItems::Connections(items) => items.len(),
        GraphInspectionItems::Diagnostics(items) => items.len(),
        GraphInspectionItems::Constants(items) => items.len(),
        GraphInspectionItems::ConstantSummaries(items) => items.len(),
        GraphInspectionItems::ConstantDetails(items) => items.len(),
    };
    let page = value
        .page
        .as_ref()
        .map(|page| InspectionPage::known(page.offset.min(page.total), returned, page.total));
    json!({
        "graph": GraphResourceRef::for_path(value.graph_path.clone()), "ready": value.ready, "counts": value.counts,
        "view": value.view, "page": page, "content": content,
    })
}

fn constants(values: &std::collections::BTreeMap<String, Value>) -> Value {
    Value::Object(
        values
            .iter()
            .map(|(id, metadata)| {
                // These are owner-produced constant metadata, not arbitrary result JSON.
                let mut projected = serde_json::Map::new();
                for key in [
                    "id",
                    "name",
                    "dataType",
                    "description",
                    "tags",
                    "hasTabularData",
                    "valueIncluded",
                    "dataValue",
                ] {
                    if let Some(value) = metadata.get(key) {
                        projected.insert(key.into(), value.clone());
                    }
                }
                (id.clone(), Value::Object(projected))
            })
            .collect(),
    )
}

pub fn failure_code(value: &CapabilityFailure) -> String {
    match value.code {
        CapabilityFailureCode::RevisionConflict | CapabilityFailureCode::InvalidRequest
            if matches!(
                value.details.get("reason").map(String::as_str),
                Some("resource_requires_current_read" | "resource_read_required")
            ) =>
        {
            "resource_read_required".to_owned()
        }
        CapabilityFailureCode::RevisionConflict | CapabilityFailureCode::GraphDraftChanged => {
            "resource_changed".to_owned()
        }
        CapabilityFailureCode::ProjectSessionUnavailable
        | CapabilityFailureCode::ProjectSessionMismatch
        | CapabilityFailureCode::ProjectSessionChanged => "project_unavailable".to_owned(),
        CapabilityFailureCode::InvocationConflict => "operation_conflict".to_owned(),
        other => other.to_string(),
    }
}

pub fn failure(value: &CapabilityFailure) -> Value {
    let code = failure_code(value);
    let mut details = value
        .details
        .iter()
        .filter(|(key, detail)| {
            (key.as_str() == "field" && public_field(value, detail))
                || matches!(
                    key.as_str(),
                    "reason"
                        | "nextStep"
                        | "capabilityId"
                        | "resourceId"
                        | "databaseId"
                        | "resultId"
                        | "nodeTypeId"
                        | "invocationId"
                        | "uiCode"
                        | "category"
                        | "path"
                        | "expected"
                        | "maximumResults"
                        | "maximumBytes"
                )
        })
        .map(|(key, value)| (key.clone(), json!(value)))
        .collect::<serde_json::Map<_, _>>();
    if matches!(code.as_str(), "resource_changed" | "resource_read_required") {
        // A historical read needing verification does not prove a content change.
        details.insert("reason".into(), json!(code));
        details.insert("nextStep".into(), json!(if code == "resource_read_required" {
            "Read the indicated resource before continuing the operation, delegating or resuming. For a document section, inspect the document outline and use its current section reference. Its current contents have not been verified; do not assume they have changed."
        } else if value.details.get("reason").map(String::as_str) == Some("document_section_changed") {
            "Inspect the document outline again and use its current section reference."
        } else {
            "Inspect the affected resource and reassess the operation. A worker must return to the Manager to refresh its task before continuing writes."
        }));
        details.retain(|key, _| {
            matches!(
                key.as_str(),
                "reason" | "nextStep" | "capabilityId" | "resourceId" | "databaseId"
            )
        });
    }
    json!({ "code": code, "details": details })
}

fn public_field(failure: &CapabilityFailure, field: &str) -> bool {
    let Some(capability) = failure.details.get("capabilityId").and_then(|id| {
        CAPABILITY_DESCRIPTORS
            .iter()
            .find(|entry| entry.id.as_str() == id)
            .map(|entry| entry.id)
    }) else {
        return false;
    };
    let schema = capability_input_schema(capability);
    // Only schema-declared business names can become diagnostics. Owner-only fields
    // such as version.sessionId never appear in this schema.
    field
        .split('.')
        .all(|name| declares_field(schema.as_value(), name))
}

fn declares_field(schema: &Value, name: &str) -> bool {
    match schema {
        Value::Object(object) => {
            object
                .get("properties")
                .is_some_and(|fields| fields.get(name).is_some())
                || object.values().any(|child| declares_field(child, name))
        }
        Value::Array(items) => items.iter().any(|child| declares_field(child, name)),
        _ => false,
    }
}
