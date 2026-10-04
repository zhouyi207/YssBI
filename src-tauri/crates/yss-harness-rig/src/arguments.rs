//! Model-facing diagnostics derived from trusted tool schemas, never argument values.

use serde::de::DeserializeOwned;
use serde_json::Value;
use serde_path_to_error::Segment;
use yss_harness_contract::{CapabilityFailure, CapabilityFailureCode};

pub(super) fn decode<T: DeserializeOwned>(
    arguments: Value,
    schema: &Value,
) -> Result<T, CapabilityFailure> {
    serde_path_to_error::deserialize(&arguments)
        .map_err(|error| diagnostic(error, schema, &arguments))
}

fn diagnostic(
    error: serde_path_to_error::Error<serde_json::Error>,
    schema: &Value,
    arguments: &Value,
) -> CapabilityFailure {
    // The raw Serde message can contain secrets. Only recognize fixed prefixes;
    // field names are included only when the schema at that path declares them.
    let message = error.inner().to_string();
    let missing_field = message
        .strip_prefix("missing field `")
        .and_then(|text| text.strip_suffix('`'));
    let mut category = [
        ("missing field ", "missing_field"),
        ("unknown field ", "unknown_field"),
        ("unknown variant ", "invalid_enum_value"),
        ("invalid type: ", "invalid_type"),
        ("invalid value: ", "invalid_value"),
        ("duplicate field ", "duplicate_field"),
    ]
    .into_iter()
    .find_map(|(prefix, category)| message.starts_with(prefix).then_some(category))
    .unwrap_or("schema_mismatch");
    let mut path = "$".to_owned();
    let mut nodes = expand(schema, vec![schema]);
    let mut actual = Some(arguments);
    for segment in error.path().iter().take(32) {
        match segment {
            Segment::Seq { index } => {
                actual = actual.and_then(|value| value.get(index));
                path.push_str(&format!("[{index}]"));
                nodes = expand(
                    schema,
                    nodes.iter().filter_map(|node| node.get("items")).collect(),
                );
            }
            Segment::Map { key } => {
                actual = actual.and_then(|value| value.get(key));
                let children = properties(&nodes, key);
                path.push('.');
                if children.is_empty() {
                    // Map keys are user data unless declared as schema properties.
                    path.push('*');
                    nodes = expand(
                        schema,
                        nodes
                            .iter()
                            .filter_map(|node| node.get("additionalProperties"))
                            .collect(),
                    );
                } else {
                    path.push_str(key);
                    nodes = expand(schema, children);
                }
            }
            Segment::Enum { .. } => {}
            Segment::Unknown => {
                actual = None;
                path.push_str(".*");
            }
        }
    }
    // Serde buffers adjacent-tagged enum payloads, which can end the reported
    // path at the enum. Refine only an already rejected value, using declared
    // schema fields; this never decides whether a request is accepted.
    if let Some(actual) = actual
        && let Some((refined_path, refined_nodes, refined_category)) =
            refine_type_path(schema, &nodes, actual, &path, missing_field, 0, &mut 256)
    {
        path = refined_path;
        nodes = refined_nodes;
        category = refined_category;
    }
    if let Some(field) = message
        .strip_prefix("missing field `")
        .and_then(|text| text.strip_suffix('`'))
    {
        let children = properties(&nodes, field);
        if !children.is_empty() {
            path.push('.');
            path.push_str(field);
            nodes = expand(schema, children);
        }
    }
    let mut expected = std::collections::BTreeSet::new();
    for node in nodes {
        for keyword in ["type", "enum", "const", "minimum", "maximum"] {
            if let Some(value) = node.get(keyword) {
                expected.insert(format!("{keyword}={value}"));
            }
        }
    }
    let expected = expected.into_iter().collect::<Vec<_>>().join("; ");
    CapabilityFailure::new(CapabilityFailureCode::InvalidRequest)
        .with_detail("reason", "tool_arguments_do_not_match_schema")
        .with_detail("category", category)
        .with_detail("path", path.chars().take(256).collect::<String>())
        .with_detail(
            "expected",
            if expected.is_empty() {
                "value matching the tool schema".to_owned()
            } else {
                expected.chars().take(512).collect()
            },
        )
}

fn refine_type_path<'a>(
    root: &'a Value,
    nodes: &[&'a Value],
    actual: &Value,
    path: &str,
    missing_field: Option<&str>,
    depth: usize,
    budget: &mut usize,
) -> Option<(String, Vec<&'a Value>, &'static str)> {
    if depth >= 32 || *budget == 0 {
        return None;
    }
    *budget -= 1;
    let types = nodes
        .iter()
        .filter_map(|node| node.get("type"))
        .flat_map(|value| {
            value
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or(std::slice::from_ref(value))
        })
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    if !types.is_empty()
        && !types.iter().any(|kind| match *kind {
            "object" => actual.is_object(),
            "array" => actual.is_array(),
            "string" => actual.is_string(),
            "boolean" => actual.is_boolean(),
            "null" => actual.is_null(),
            "number" => actual.is_number(),
            "integer" => actual.is_i64() || actual.is_u64(),
            _ => true,
        })
    {
        return Some((path.to_owned(), nodes.to_vec(), "invalid_type"));
    }
    if let Some(object) = actual.as_object() {
        let matching = nodes
            .iter()
            .copied()
            .filter(|node| {
                node.get("properties")
                    .and_then(Value::as_object)
                    .is_none_or(|properties| {
                        properties.iter().all(|(key, field)| {
                            field.get("const").is_none_or(|expected| {
                                object.get(key).is_none_or(|value| value == expected)
                            })
                        })
                    })
            })
            .collect::<Vec<_>>();
        if let Some(field) = missing_field
            && !object.contains_key(field)
            && matching.iter().any(|node| {
                node.get("required")
                    .and_then(Value::as_array)
                    .is_some_and(|required| {
                        required.iter().any(|name| name.as_str() == Some(field))
                    })
            })
        {
            let children = expand(root, properties(&matching, field));
            if !children.is_empty() {
                return Some((format!("{path}.{field}"), children, "missing_field"));
            }
        }
        for (key, value) in object.iter().take(*budget) {
            let children = expand(root, properties(&matching, key));
            if !children.is_empty()
                && let Some(found) = refine_type_path(
                    root,
                    &children,
                    value,
                    &format!("{path}.{key}"),
                    missing_field,
                    depth + 1,
                    budget,
                )
            {
                return Some(found);
            }
        }
    } else if let Some(values) = actual.as_array() {
        let children = expand(
            root,
            nodes.iter().filter_map(|node| node.get("items")).collect(),
        );
        if !children.is_empty() {
            for (index, value) in values.iter().take(*budget).enumerate() {
                if let Some(found) = refine_type_path(
                    root,
                    &children,
                    value,
                    &format!("{path}[{index}]"),
                    missing_field,
                    depth + 1,
                    budget,
                ) {
                    return Some(found);
                }
            }
        }
    }
    None
}

fn properties<'a>(nodes: &[&'a Value], key: &str) -> Vec<&'a Value> {
    nodes
        .iter()
        .filter_map(|node| node.get("properties")?.get(key))
        .collect()
}

/// Resolve only local references and schema alternatives, with a work bound for
/// recursive schemas. This describes Serde failures; it is not another validator.
fn expand<'a>(root: &'a Value, mut pending: Vec<&'a Value>) -> Vec<&'a Value> {
    let mut nodes: Vec<&Value> = Vec::new();
    for _ in 0..256 {
        let Some(node) = pending.pop() else { break };
        if nodes.iter().any(|seen| std::ptr::eq(*seen, node)) {
            continue;
        }
        nodes.push(node);
        if let Some(reference) = node
            .get("$ref")
            .and_then(Value::as_str)
            .and_then(|value| value.strip_prefix('#'))
            && let Some(target) = root.pointer(reference)
        {
            pending.push(target);
        }
        for keyword in ["oneOf", "anyOf", "allOf"] {
            if let Some(branches) = node.get(keyword).and_then(Value::as_array) {
                pending.extend(branches);
            }
        }
    }
    nodes
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_harness_contract::{
        CapabilityId, StatisticalPlan, ToolDescriptor, statistical_plan_schema,
    };

    #[test]
    fn diagnostics_keep_schema_paths_and_expectations_without_echoing_values_or_map_keys() {
        let schema = serde_json::to_value(
            ToolDescriptor::for_capability(CapabilityId::SearchNodeCatalog).input_schema,
        )
        .unwrap();
        for (arguments, category, path, expected) in [
            (
                serde_json::json!({"query": "secret", "locale": "en-US"}),
                "missing_field",
                "$.limit",
                "integer",
            ),
            (
                serde_json::json!({"query": "secret", "locale": "en-US", "limit": "secret"}),
                "invalid_type",
                "$.limit",
                "integer",
            ),
        ] {
            let failure =
                decode::<yss_harness_contract::SearchNodeCatalogRequest>(arguments, &schema)
                    .unwrap_err();
            assert_eq!(failure.details["category"], category);
            assert_eq!(failure.details["path"], path);
            assert!(failure.details["expected"].contains(expected));
            assert!(!serde_json::to_string(&failure).unwrap().contains("secret"));
        }
        let schema =
            serde_json::json!({"type": "object", "additionalProperties": {"type": "integer"}});
        let failure = decode::<std::collections::BTreeMap<String, u32>>(
            serde_json::json!({"secret": "secret"}),
            &schema,
        )
        .unwrap_err();
        assert_eq!(failure.details["path"], "$.*");
        assert!(!serde_json::to_string(&failure).unwrap().contains("secret"));

        let schema = serde_json::to_value(
            ToolDescriptor::for_capability(CapabilityId::ApplyGraphEdit).input_schema,
        )
        .unwrap();
        for payload in [
            serde_json::json!({"nodeTypeId": "secret", "x": "secret", "y": 0, "parameters": {}, "portCounts": {}}),
            serde_json::json!({"nodeTypeId": "secret", "y": 0, "parameters": {}, "portCounts": {}}),
        ] {
            let failure = decode::<yss_harness_contract::ApplyGraphEditRequest>(serde_json::json!({
                "graphPath": "secret", "baseRevision": 1, "graphHash": "secret", "clientKey": "secret", "locale": "en-US",
                "operations": [{"type": "create_node", "payload": payload}]
            }), &schema).unwrap_err();
            assert_eq!(failure.details["path"], "$.operations[0].payload.x");
            assert!(failure.details["expected"].contains("number"));
            assert!(!serde_json::to_string(&failure).unwrap().contains("secret"));
        }

        let schema = serde_json::to_value(statistical_plan_schema()).unwrap();
        let failure =
            decode::<StatisticalPlan>(serde_json::json!({"analysisMode": "secret"}), &schema)
                .unwrap_err();
        assert_eq!(failure.code, CapabilityFailureCode::InvalidRequest);
        assert_eq!(failure.details["category"], "invalid_enum_value");
        assert_eq!(failure.details["path"], "$.analysisMode");
        assert!(failure.details["expected"].contains("confirmatory"));
        assert!(!serde_json::to_string(&failure).unwrap().contains("secret"));
    }
}
