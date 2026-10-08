//! Summary input projection; the transcript and native model/tool messages remain intact.
use rig_core::completion::Message;
use rig_core::completion::message::UserContent;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(super) fn summary_source<'a>(
    messages: impl IntoIterator<Item = &'a Message>,
) -> Result<String, serde_json::Error> {
    let mut source = String::new();
    let mut observations = BTreeMap::new();
    for message in messages {
        if let Message::User { content } = message {
            for part in content {
                if let UserContent::ToolResult(result) = part {
                    let mut values = Vec::new();
                    for part in &result.content {
                        let mut value = match part.deserialize_json::<Value>() {
                            Ok(value) => value,
                            Err(_) => serde_json::to_value(part)?,
                        };
                        if matches!(result.name.as_str(), "inspect_graph" | "inspect_resource") {
                            let hash =
                                yss_canonical_hash::content_sha256(&serde_json::to_vec(&value)?);
                            if let Some(previous) = observations.get(&hash) {
                                value = json!({"sameObservationAs": previous});
                            } else {
                                observations.insert(hash, result.call.to_string());
                                project_graph(&mut value);
                            }
                        }
                        values.push(value);
                    }
                    source.push_str(&serde_json::to_string(&json!({
                        "role": "tool", "callId": result.call.wire(),
                        "name": result.name, "results": values,
                    }))?);
                } else {
                    source.push_str(&serde_json::to_string(
                        &json!({"role":"user", "content":part}),
                    )?);
                }
                source.push('\n');
            }
        } else {
            source.push_str(&serde_json::to_string(message)?);
            source.push('\n');
        }
    }
    Ok(source)
}

fn project_graph(value: &mut Value) {
    let graph = match value["type"].as_str() {
        Some("graph_inspection") => value.get_mut("payload"),
        Some("resource_inspection") if value["payload"]["content"]["kind"] == "graph" => {
            value.pointer_mut("/payload/content/graph")
        }
        _ => None,
    };
    let Some(graph) = graph.and_then(Value::as_object_mut) else {
        return;
    };
    // Retain graph meaning and exact addresses while removing repeated editor and
    // resolved-schema descriptions. Statistical result tables are never projected.
    if let Some(nodes) = graph.get_mut("nodes").and_then(Value::as_array_mut) {
        for node in nodes.iter_mut().filter_map(Value::as_object_mut) {
            node.retain(|key, _| {
                matches!(
                    key.as_str(),
                    "nodeId" | "nodeTypeId" | "userLabel" | "title" | "parameters" | "ports"
                )
            });
            if let Some(parameters) = node.get_mut("parameters").and_then(Value::as_array_mut) {
                for parameter in parameters.iter_mut().filter_map(Value::as_object_mut) {
                    parameter.retain(|key, _| matches!(key.as_str(), "key" | "value"));
                }
            }
            if let Some(ports) = node.get_mut("ports").and_then(Value::as_array_mut) {
                for port in ports.iter_mut().filter_map(Value::as_object_mut) {
                    port.retain(|key, _| {
                        matches!(
                            key.as_str(),
                            "address" | "direction" | "dataType" | "literal"
                        )
                    });
                }
            }
        }
    }
    graph.insert("summaryProjection".into(), json!(
        "Editor descriptions, option lists, layout and resolved schemas omitted from summary input. Node/port identities, parameter values, literals and connections are retained. Inspect current details before editing or executing."
    ));
}

#[cfg(test)]
mod tests {
    use super::*;
    use rig_core::message::{CallId, ToolName};

    #[test]
    fn graph_projection_reduces_repeated_bodies_but_preserves_diagnostics_and_statistics() {
        let graph = json!({"type":"graph_inspection","payload":{
            "version":{"revision":7,"sessionId":"graph-session"},
            "graphPath":"events/analysis.yssbi-event","graphHash":"hash","semanticInputHash":"inputs",
            "ready":false,"revision":7,"constants":{"group":"Group A"},
            "diagnostics":[{"code":"missing_input","nodeId":"n1"}],
            "nodes":[{"nodeId":"n1","nodeTypeId":"stat.test","parameters":[{"key":"column","value":"income","options":["choice ".repeat(8_000)]}],"ports":[{"address":{"nodeId":"n1","portKey":"alpha"},"literal":0.05,"schema":{"description":"repeated port metadata ".repeat(8_000)}}]}],
            "connections":[{"id":"c1"}]
        }});
        let statistics = json!({"type":"result_inspection","payload":{
            "resultId":42,"rows":[["Group A",0.0000234,"exact value ".repeat(1000)]]
        }});
        let messages = vec![
            Message::tool_result(
                CallId::from_wire("first"),
                ToolName::new("inspect_graph").unwrap(),
                graph.to_string(),
            ),
            Message::tool_result(
                CallId::from_wire("repeat"),
                ToolName::new("inspect_graph").unwrap(),
                graph.to_string(),
            ),
            Message::tool_result(
                CallId::from_wire("stats"),
                ToolName::new("inspect_result").unwrap(),
                statistics.to_string(),
            ),
        ];
        let original = serde_json::to_string(&messages).unwrap();
        let source = summary_source(&messages).unwrap();
        assert!(source.len() < original.len() / 10);
        let lines: Vec<Value> = source
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(
            lines[0]["results"][0]["payload"]["diagnostics"],
            graph["payload"]["diagnostics"]
        );
        assert_eq!(
            lines[0]["results"][0]["payload"]["version"],
            graph["payload"]["version"]
        );
        assert_eq!(
            lines[0]["results"][0]["payload"]["constants"],
            graph["payload"]["constants"]
        );
        assert_eq!(lines[1]["results"][0]["sameObservationAs"], "first");
        assert_eq!(
            lines[0]["results"][0]["payload"]["nodes"][0]["nodeId"],
            "n1"
        );
        assert_eq!(
            lines[0]["results"][0]["payload"]["nodes"][0]["parameters"][0]["value"],
            "income"
        );
        assert_eq!(
            lines[0]["results"][0]["payload"]["nodes"][0]["ports"][0]["literal"],
            0.05
        );
        assert_eq!(
            lines[0]["results"][0]["payload"]["connections"],
            graph["payload"]["connections"]
        );
        assert_eq!(lines[2]["results"][0], statistics);
        let page = json!({"type":"resource_inspection","payload":{"content":{
            "kind":"graph_page","graph":{"view":"ports","observationHash":"observation","page":{"nextOffset":50},"content":{"kind":"ports","items":[{"schema":{"x":"Integer"}}]}}
        }}});
        let projected = summary_source(&[Message::tool_result(
            CallId::from_wire("page"),
            ToolName::new("inspect_resource").unwrap(),
            page.to_string(),
        )])
        .unwrap();
        let projected: Value = serde_json::from_str(projected.trim()).unwrap();
        assert_eq!(projected["results"][0], page);
        assert_eq!(serde_json::to_string(&messages).unwrap(), original);
    }
}
