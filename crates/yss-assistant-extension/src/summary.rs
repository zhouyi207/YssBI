//! Context-compaction projections of large repeated graph observations.

use serde_json::{Value, json};

/// Graph and resource inspections are reduced in summary input; other results stay exact.
pub(crate) fn projection(tool: &str, output: &Value) -> Option<Value> {
    if !matches!(tool, "graph_inspect" | "project_inspect_resource") {
        return None;
    }
    let mut value = output.clone();
    project_graph(&mut value);
    Some(value)
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
        let projected = projection("graph_inspect", &graph).unwrap();
        assert!(projected.to_string().len() < graph.to_string().len() / 10);
        for key in ["diagnostics", "version", "constants", "connections"] {
            assert_eq!(projected["payload"][key], graph["payload"][key], "{key}");
        }
        let node = &projected["payload"]["nodes"][0];
        assert_eq!(node["nodeId"], "n1");
        assert_eq!(node["parameters"][0]["value"], "income");
        assert_eq!(node["ports"][0]["literal"], 0.05);

        let statistics = json!({"type":"result_inspection","payload":{
            "resultId":42,"rows":[["Group A",0.0000234,"exact value ".repeat(1000)]]
        }});
        assert_eq!(projection("result_inspect", &statistics), None);

        let page = json!({"type":"resource_inspection","payload":{"content":{
            "kind":"graph_page","graph":{"view":"ports","observationHash":"observation","page":{"nextOffset":50},"content":{"kind":"ports","items":[{"schema":{"x":"Integer"}}]}}
        }}});
        assert_eq!(projection("project_inspect_resource", &page), Some(page));
    }
}
