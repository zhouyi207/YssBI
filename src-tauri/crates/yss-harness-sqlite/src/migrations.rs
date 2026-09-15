use serde_json::Value;
use sqlx::{Row, SqliteConnection};
use yss_harness_contract::PersistenceFailure;

use crate::{invalid_record, unavailable};

pub(super) async fn migrate(
    connection: &mut SqliteConnection,
    version: i64,
) -> Result<(), PersistenceFailure> {
    if version < 1 {
        sqlx::query(
            "UPDATE tool_invocation
             SET payload_json = json_remove(payload_json, '$.result.payload.operationId')
             WHERE json_extract(payload_json, '$.result.type') = 'graph_edit_receipt'
               AND json_type(payload_json, '$.result.payload.operationId') IS NOT NULL",
        )
        .execute(&mut *connection)
        .await
        .map_err(|_| unavailable())?;
    }
    if version < 3 {
        // Migrate typed fields only. Messages, instructions, graph literals and
        // result data may contain identical words and must remain untouched.
        macro_rules! table {
            ($name:literal) => {
                (
                    $name,
                    concat!(
                        "SELECT rowid, payload_json FROM ",
                        $name,
                        " WHERE rowid > ? ORDER BY rowid LIMIT 32"
                    ),
                    concat!("UPDATE ", $name, " SET payload_json = ? WHERE rowid = ?"),
                )
            };
        }
        for (table, select, update) in [
            table!("tool_invocation"),
            table!("assistant_event"),
            table!("workflow_definition"),
            table!("approval_grant"),
            table!("skill_installation"),
        ] {
            if version >= 2 && table != "tool_invocation" {
                continue;
            }
            let mut after = 0_i64;
            loop {
                let rows = sqlx::query(select)
                    .bind(after)
                    .fetch_all(&mut *connection)
                    .await
                    .map_err(|_| unavailable())?;
                if rows.is_empty() {
                    break;
                }
                for row in rows {
                    after = row.get("rowid");
                    let mut value: Value = serde_json::from_str(row.get("payload_json"))
                        .map_err(|_| invalid_record())?;
                    let mut changed = version < 2 && migrate_graph_record(table, &mut value);
                    if table == "tool_invocation" {
                        changed |= migrate_graph_diagnostics(&mut value);
                    }
                    if changed {
                        let encoded =
                            serde_json::to_string(&value).map_err(|_| invalid_record())?;
                        sqlx::query(update)
                            .bind(encoded)
                            .bind(after)
                            .execute(&mut *connection)
                            .await
                            .map_err(|_| unavailable())?;
                    }
                }
            }
        }
        sqlx::query("PRAGMA user_version = 3")
            .execute(connection)
            .await
            .map_err(|_| unavailable())?;
    }
    Ok(())
}

fn rename(value: Option<&mut Value>, old: &str, new: &str) -> bool {
    if let Some(value) = value
        && value.as_str() == Some(old)
    {
        *value = Value::String(new.into());
        true
    } else {
        false
    }
}

fn migrate_graph_diagnostics(record: &mut Value) -> bool {
    if !matches!(
        record.pointer("/result/type").and_then(Value::as_str),
        Some("graph_validation" | "graph_inspection")
    ) {
        return false;
    }
    let Some(diagnostics) = record
        .pointer_mut("/result/payload/diagnostics")
        .and_then(Value::as_array_mut)
    else {
        return false;
    };
    let mut changed = false;
    for diagnostic in diagnostics {
        for (field, old, new) in [
            ("code", "compiler.", "graph."),
            ("messageKey", "diagnostics.compiler.", "diagnostics.graph."),
        ] {
            if let Some(value) = diagnostic.get_mut(field)
                && let Some(suffix) = value.as_str().and_then(|value| value.strip_prefix(old))
            {
                *value = format!("{new}{suffix}").into();
                changed = true;
            }
        }
    }
    changed
}

fn migrate_request(request: &mut Value) -> bool {
    if rename(request.get_mut("type"), "compile_graph", "validate_graph") {
        return true;
    }
    request.get("type").and_then(Value::as_str) == Some("execute_graph")
        && request
            .get_mut("payload")
            .and_then(Value::as_object_mut)
            .is_some_and(|payload| payload.remove("artifactId").is_some())
}

fn migrate_graph_record(table: &str, record: &mut Value) -> bool {
    let mut changed = false;
    match table {
        "tool_invocation" => {
            changed |= rename(
                record.get_mut("capabilityId"),
                "compile_graph",
                "validate_graph",
            );
            if let Some(request) = record.get_mut("request") {
                changed |= migrate_request(request);
            }
            if let Some(result) = record.get_mut("result") {
                changed |= rename(
                    result.get_mut("type"),
                    "graph_compilation",
                    "graph_validation",
                );
                if matches!(
                    result.get("type").and_then(Value::as_str),
                    Some("graph_validation" | "graph_execution")
                ) && let Some(payload) = result.get_mut("payload").and_then(Value::as_object_mut)
                {
                    changed |= payload.remove("artifactId").is_some();
                }
            }
            changed |= rename(
                record.pointer_mut("/failure/code"),
                "graph_compile_failed",
                "graph_validation_failed",
            );
        }
        "assistant_event" => {
            if record.pointer("/event/type").and_then(Value::as_str) == Some("agent") {
                changed |= rename(
                    record.pointer_mut("/event/payload/payload/capability_id"),
                    "compile_graph",
                    "validate_graph",
                );
                changed |= rename(
                    record.pointer_mut("/event/payload/payload/failure_code"),
                    "graph_compile_failed",
                    "graph_validation_failed",
                );
            }
        }
        "workflow_definition" => {
            if let Some(steps) = record.get_mut("steps").and_then(Value::as_array_mut) {
                for step in steps {
                    match step.pointer("/kind/type").and_then(Value::as_str) {
                        Some("capability") => {
                            if let Some(request) = step.pointer_mut("/kind/payload") {
                                changed |= migrate_request(request);
                            }
                        }
                        Some("approval") => {
                            changed |= rename(
                                step.pointer_mut("/kind/payload/capability_id"),
                                "compile_graph",
                                "validate_graph",
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
        "approval_grant" => {
            changed |= rename(
                record.get_mut("capabilityId"),
                "compile_graph",
                "validate_graph",
            );
        }
        "skill_installation" => {
            if let Some(capabilities) = record
                .pointer_mut("/manifest/allowedCapabilities")
                .and_then(Value::as_array_mut)
            {
                for capability in capabilities {
                    changed |= rename(Some(capability), "compile_graph", "validate_graph");
                }
            }
        }
        _ => unreachable!("fixed migration table"),
    }
    changed
}
