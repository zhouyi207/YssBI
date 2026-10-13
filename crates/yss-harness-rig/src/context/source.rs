//! Summary input projection; the transcript and native model/tool messages remain intact.
use rig_core::completion::Message;
use rig_core::completion::message::UserContent;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use yss_harness_contract::AgentToolExecutor;

pub(super) fn summary_source<'a>(
    messages: impl IntoIterator<Item = &'a Message>,
    executor: &dyn AgentToolExecutor,
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
                        if let Some(projection) =
                            executor.summary_projection(result.name.as_str(), &value)
                        {
                            let hash =
                                yss_canonical_hash::content_sha256(&serde_json::to_vec(&value)?);
                            if let Some(previous) = observations.get(&hash) {
                                value = json!({"sameObservationAs": previous});
                            } else {
                                observations.insert(hash, result.call.to_string());
                                value = projection;
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

#[cfg(test)]
mod tests {
    use super::*;
    use rig_core::message::{CallId, ToolName};

    struct Projection;

    impl AgentToolExecutor for Projection {
        fn call<'a>(
            &'a self,
            _call: yss_harness_contract::ToolCall,
        ) -> yss_harness_contract::AgentFuture<'a, yss_harness_contract::ToolCallOutcome> {
            unreachable!("summary projection never executes tools")
        }

        fn summary_projection(&self, tool: &str, output: &Value) -> Option<Value> {
            (tool == "inspect").then(|| json!({"kind": output["kind"], "summarized": true}))
        }
    }

    #[test]
    fn projected_outputs_are_reduced_once_and_other_results_stay_exact() {
        let large = json!({"kind": "large", "body": "repeated body ".repeat(8_000)});
        let exact = json!({"rows": [["Group A", 0.0000234]]});
        let messages = vec![
            Message::tool_result(
                CallId::from_wire("first"),
                ToolName::new("inspect").unwrap(),
                large.to_string(),
            ),
            Message::tool_result(
                CallId::from_wire("repeat"),
                ToolName::new("inspect").unwrap(),
                large.to_string(),
            ),
            Message::tool_result(
                CallId::from_wire("exact"),
                ToolName::new("read").unwrap(),
                exact.to_string(),
            ),
        ];
        let original = serde_json::to_string(&messages).unwrap();
        let source = summary_source(&messages, &Projection).unwrap();
        assert!(source.len() < original.len() / 10);
        let lines: Vec<Value> = source
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(
            lines[0]["results"][0],
            json!({"kind": "large", "summarized": true})
        );
        assert_eq!(lines[1]["results"][0]["sameObservationAs"], "first");
        assert_eq!(lines[2]["results"][0], exact);
        assert_eq!(serde_json::to_string(&messages).unwrap(), original);
    }
}
