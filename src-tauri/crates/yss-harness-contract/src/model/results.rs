//! Live and replay projections. User values are never recursively filtered by key name.
use crate::*;
use serde_json::{Value, json};

pub fn capability_result(result: &AutomationCapabilityResult) -> Result<Value, serde_json::Error> {
    use AutomationCapabilityResult::*;
    let (kind, payload) = match result {
        ResourceInspection(value) => ("resource_inspection", resource(value)),
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
                "resources": value.resources.iter().map(|value| json!({
                    "resource": value.resource, "displayName": value.display_name,
                })).collect::<Vec<_>>(),
            }),
        ),
        GraphEditReceipt(value) => (
            "graph_edit_receipt",
            json!({
                "graphPath": value.graph_path, "createdNodes": value.created_nodes,
                "createdPorts": value.created_ports,
                "changes": {
                    "nodes": value.changes.nodes, "removedNodeIds": value.changes.removed_node_ids,
                    "connections": value.changes.connections, "removedConnectionIds": value.changes.removed_connection_ids,
                    "constants": constants(&value.changes.constants), "removedConstantIds": value.changes.removed_constant_ids,
                    "ready": value.changes.ready, "diagnostics": value.changes.diagnostics,
                },
            }),
        ),
        GraphValidation(value) => (
            "graph_validation",
            json!({
                "graphPath": value.graph_path, "ready": value.ready, "diagnostics": value.diagnostics,
            }),
        ),
        GraphExecution(value) => (
            "graph_execution",
            json!({
                "graphPath": value.graph_path, "runId": value.run_id, "status": value.status,
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
        DatasetExported(value) => ("dataset_exported", serde_json::to_value(value)?),
        UiIntentInspection(value) => ("ui_intent_inspection", serde_json::to_value(value)?),
        UiIntentReceipt(value) => ("ui_intent_receipt", serde_json::to_value(value)?),
        NodeCatalogSearch(value) => ("node_catalog_search", serde_json::to_value(value)?),
        KnowledgeSearch(value) => ("knowledge_search", serde_json::to_value(value)?),
        KnowledgePassage(value) => ("knowledge_passage", serde_json::to_value(value)?),
        ResultInspection(value) => ("result_inspection", serde_json::to_value(value)?),
        GraphResults(value) => ("graph_results", serde_json::to_value(value)?),
    };
    Ok(json!({ "type": kind, "payload": payload }))
}

fn resource(value: &ResourceInspection) -> Value {
    let content = match &value.content {
        ResourceContent::Metadata => json!({"kind": "metadata"}),
        ResourceContent::Graph {
            graph: value,
            function,
            can_undo,
            can_redo,
        } => json!({
            "kind": "graph", "graph": graph(value),
            "function": function.as_ref().map(super::FunctionSignatureInput::from),
            "canUndo": can_undo, "canRedo": can_redo,
        }),
        ResourceContent::GraphPage {
            graph: value,
            function,
            can_undo,
            can_redo,
        } => json!({
            "kind": "graph_page", "graph": graph_page(value),
            "function": function.as_ref().map(super::FunctionSignatureInput::from),
            "canUndo": can_undo, "canRedo": can_redo,
        }),
        ResourceContent::Chart { settings } => json!({ "kind": "chart", "settings": settings }),
        ResourceContent::Mind {
            root_id,
            nodes,
            total_nodes,
            next_offset,
        } => json!({
            "kind": "mind", "rootId": root_id, "nodes": nodes,
            "totalNodes": total_nodes, "nextOffset": next_offset,
        }),
        ResourceContent::Doc {
            markdown,
            total_characters,
            next_offset,
        } => json!({
            "kind": "doc", "markdown": markdown,
            "totalCharacters": total_characters, "nextOffset": next_offset,
        }),
        ResourceContent::Database {
            schema,
            rows,
            row_ids,
            next_offset,
            can_undo,
            can_redo,
        } => json!({
            "kind": "database", "schema": dataset_schema(schema), "rows": rows, "rowIds": row_ids,
            "nextOffset": next_offset, "canUndo": can_undo, "canRedo": can_redo,
        }),
    };
    json!({ "resource": value.resource, "name": value.name, "dirty": value.dirty, "content": content })
}

fn mutation(value: &ResourceMutationReceipt) -> Value {
    json!({
        "moves": value.moves, "createdNodes": value.created_nodes,
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
    let content = match &value.content {
        GraphInspectionItems::Constants(values) => {
            json!({ "kind": "constants", "items": constants(values) })
        }
        other => json!(other),
    };
    json!({
        "graphPath": value.graph_path, "ready": value.ready, "counts": value.counts,
        "view": value.view, "page": value.page, "content": content,
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

pub fn failure(value: &CapabilityFailure) -> Value {
    let details = if matches!(
        value.code,
        CapabilityFailureCode::RevisionConflict | CapabilityFailureCode::GraphDraftChanged
    ) {
        json!({ "reason": "resource_changed", "nextStep": "Inspect the changed resource and reassess the operation. A worker must return to the Manager to refresh its task before continuing writes." })
    } else {
        Value::Object(
            value
                .details
                .iter()
                .filter(|(key, _)| {
                    matches!(
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
                .collect(),
        )
    };
    json!({ "code": value.code, "details": details })
}
