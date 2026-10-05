use super::*;
use serde_json::json;
use yss_harness_contract::{AutomationCapabilityRequest, InspectGraphRequest};

#[test]
fn delegation_arguments_accept_business_scope_and_reject_internal_authority() {
    use yss_harness_contract::model::{AgentFollowupInput, AgentTaskInput};
    let task = json!({
        "worker": "review", "objective": "Review evidence", "constraints": "Read only",
        "completionCriteria": "Return findings", "dependsOn": [],
        "scope": { "resources": [{"resource": {"kind": "doc", "id": "docs/report.md"}, "operations": ["inspect"]}],
            "results": [], "creations": [], "exportPaths": [] }
    });
    let schema = serde_json::to_value(yss_harness_contract::agent_task_schema()).unwrap();
    let decoded = arguments::decode::<AgentTaskInput>(task.clone(), &schema).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), task);
    for field in ["key", "revision", "sessionId"] {
        let mut invalid = task.clone();
        invalid[field] = json!("internal");
        assert!(arguments::decode::<AgentTaskInput>(invalid, &schema).is_err());
    }
    let mut invalid = task;
    invalid["scope"]["resources"][0]["version"] =
        json!({"revision": 7, "sessionId": "edit-session"});
    assert!(arguments::decode::<AgentTaskInput>(invalid, &schema).is_err());
    let schema = serde_json::to_value(yss_harness_contract::agent_followup_schema()).unwrap();
    arguments::decode::<AgentFollowupInput>(
        json!({"runId": "worker", "instruction": "Continue"}),
        &schema,
    )
    .unwrap();
    assert!(
        arguments::decode::<AgentFollowupInput>(
            json!({"runId": "worker", "instruction": "Continue", "resourceVersions": []}),
            &schema
        )
        .is_err()
    );
}

#[test]
fn provider_arguments_decode_through_the_registered_resource_schemas() {
    for (capability, arguments) in [
        (
            CapabilityId::SearchKnowledge,
            json!({"query":"regression diagnostics"}),
        ),
        (
            CapabilityId::ReadKnowledge,
            json!({"reference":{"documentId":"ols","chunkId":"chunk-1"}}),
        ),
    ] {
        let schema =
            serde_json::to_value(yss_harness_contract::capability_input_schema(capability))
                .unwrap();
        let request = decode_request(capability, arguments.clone(), &schema).unwrap();

        assert_eq!(request.capability_id(), capability);
        for field in ["project", "revision", "sourceHash", "version"] {
            let mut invalid = arguments.clone();
            invalid[field] = json!("internal");
            assert!(decode_request(capability, invalid, &schema).is_err());
        }
    }
    let schema = serde_json::to_value(yss_harness_contract::capability_input_schema(
        CapabilityId::InspectGraph,
    ))
    .unwrap();
    let overview = decode_request(
        CapabilityId::InspectGraph,
        json!({"graphPath":"events/Main.yssbi-event"}),
        &schema,
    )
    .unwrap();
    assert_eq!(
        overview,
        AutomationCapabilityRequest::InspectGraph(InspectGraphRequest::overview(
            "events/Main.yssbi-event"
        ))
        .into()
    );
    let ports = decode_request(CapabilityId::InspectGraph, json!({"graphPath":"events/Main.yssbi-event", "view":"ports", "nodeIds":["node-1"], "includeSchema":true, "offset":10, "limit":5}), &schema).unwrap();
    let CapabilityInput::InspectGraph(ports) = ports else {
        panic!("graph input");
    };
    InspectGraphRequest::from(ports).validate().unwrap();
    let invalid = decode_request(
        CapabilityId::InspectGraph,
        json!({"graphPath":"events/Main.yssbi-event", "view":"overview", "includeSchema":true}),
        &schema,
    )
    .unwrap();
    let CapabilityInput::InspectGraph(invalid) = invalid else {
        panic!("graph input");
    };
    assert!(InspectGraphRequest::from(invalid).validate().is_err());
    let schema = serde_json::to_value(yss_harness_contract::capability_input_schema(
        CapabilityId::ManageResource,
    ))
    .unwrap();
    let request = decode_request(
        CapabilityId::ManageResource,
        json!({
            "operation": "create", "specification": {"kind": "doc", "name": "Report"}
        }),
        &schema,
    )
    .unwrap();
    assert!(matches!(
        request,
        CapabilityInput::ManageResource(yss_harness_contract::model::ManageResourceInput::Create {
            specification: yss_harness_contract::ResourceCreation::Doc { .. }
        })
    ));

    let schema = serde_json::to_value(yss_harness_contract::capability_input_schema(
        CapabilityId::EditResource,
    ))
    .unwrap();
    let mut input = json!({
        "resource": {"kind": "doc", "id": "docs/Report.md"},
        "edit": {"kind": "doc", "operations": [{"op": "replace_range", "start": 0, "end": 2, "markdown": "正文"}]}
    });
    let request = decode_request(CapabilityId::EditResource, input.clone(), &schema).unwrap();
    assert_eq!(serde_json::to_value(&request).unwrap()["payload"], input);
    input["version"] = json!({"revision": 2, "sessionId": "doc-session"});
    assert!(decode_request(CapabilityId::EditResource, input, &schema).is_err());
}

#[test]
fn model_capabilities_hide_concurrency_fields_in_schemas_calls_and_live_replay_results() {
    use yss_harness_contract::*;
    let hidden = [
        "revision",
        "baseRevision",
        "graphHash",
        "semanticInputHash",
        "observationHash",
        "clientKey",
        "ifUnchanged",
        "sessionId",
        "publicationRevision",
        "runtimeRevision",
        "schemaRevision",
    ];
    for descriptor in CAPABILITY_DESCRIPTORS {
        let schema = capability_input_schema(descriptor.id);
        let encoded = serde_json::to_string(&schema).unwrap();
        for field in hidden {
            assert!(
                !encoded.contains(&format!("\"{field}\"")),
                "{} exposes {field}",
                descriptor.id.as_str()
            );
        }
    }
    for (capability, input) in [
        (
            CapabilityId::ApplyGraphEdit,
            json!({"graphPath":"events/analysis.yssbi-event", "locale":"en-US", "operations":[{"type":"move_nodes","payload":{"positions":[{"nodeId":"node","x":1,"y":2}]}}]}),
        ),
        (
            CapabilityId::ExecuteGraph,
            json!({"graphPath":"events/analysis.yssbi-event", "demand":{"type":"default"}}),
        ),
        (
            CapabilityId::ManageResource,
            json!({"operation":"save", "resource":{"kind":"doc","id":"docs/report.md"}}),
        ),
        (
            CapabilityId::EditResource,
            json!({"resource":{"kind":"function_graph","id":"functions/fit.yssbi-function"},"edit":{"kind":"function_signature","signature":{"parameters":[],"returnType":"core.numeric"}}}),
        ),
        (
            CapabilityId::RequestUiIntent,
            json!({"intent":{"kind":"showPanel","panel":"project"}}),
        ),
    ] {
        let schema = serde_json::to_value(capability_input_schema(capability)).unwrap();
        assert!(decode_request(capability, input.clone(), &schema).is_ok());
        let mut invalid = input;
        invalid["revision"] = json!(7);
        assert!(decode_request(capability, invalid, &schema).is_err());
    }

    let internal = AutomationCapabilityRequest::ApplyGraphEdit(ApplyGraphEditRequest {
        graph_path: "events/analysis.yssbi-event".into(),
        locale: "en-US".into(),
        operations: vec![],
        base_revision: 21,
        graph_hash: "a".repeat(64),
        client_key: "host-key".into(),
    });
    let result = AutomationCapabilityResult::GraphEditReceipt(GraphEditReceipt {
        graph_path: "events/analysis.yssbi-event".into(), from_revision: 21, to_revision: 22,
        graph_hash: "b".repeat(64), client_key: "host-key".into(),
        created_nodes: [("fit".into(), "real-node-id".into())].into(), created_ports: Default::default(),
        changes: GraphEditChanges {
            base_semantic_input_hash: "c".repeat(64), semantic_input_hash: "d".repeat(64),
            nodes: vec![], removed_node_ids: vec![], connections: vec![], removed_connection_ids: vec![],
            constants: [("c1".into(), json!({"id":"c1","name":"Example","contentHash":"internal-constant-hash","dataValue":{"revision":19},"valueIncluded":true}))].into(),
            removed_constant_ids: vec![], ready: true, diagnostics: vec![],
        },
    });
    let live = crate::messages::tool_result_json(Ok(result.clone())).unwrap();
    assert_eq!(live["payload"]["createdNodes"]["fit"], "real-node-id");
    let constant = &live["payload"]["changes"]["constants"]["c1"];
    assert!(constant.get("contentHash").is_none());
    assert_eq!(constant["dataValue"], json!({"revision":19}));
    let invocation_id = ToolInvocationId::try_new("graph-call").unwrap();
    let prepared = crate::messages::prepare_messages(vec![
        AgentMessage::User {
            content: "Edit the graph".into(),
        },
        AgentMessage::ToolCall {
            invocation_id: invocation_id.clone(),
            request: internal,
        },
        AgentMessage::ToolResult {
            invocation_id,
            capability_id: CapabilityId::ApplyGraphEdit,
            outcome: Ok(result),
        },
        AgentMessage::User {
            content: "Continue".into(),
        },
    ])
    .unwrap();
    let wire: Vec<_> = prepared
        .history
        .into_iter()
        .flat_map(|message| {
            Vec::<rig_core::providers::openai::completion::Message>::try_from(message).unwrap()
        })
        .map(|message| serde_json::to_value(message).unwrap())
        .collect();
    let call = wire
        .iter()
        .find(|message| message.get("tool_calls").is_some())
        .unwrap();
    let arguments: serde_json::Value = serde_json::from_str(
        call["tool_calls"][0]["function"]["arguments"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    for field in hidden {
        assert!(arguments.get(field).is_none());
    }
    let replay = wire
        .iter()
        .find(|message| message["role"] == "tool")
        .unwrap();
    let replay: serde_json::Value =
        serde_json::from_str(replay["content"].as_str().unwrap()).unwrap();
    assert_eq!(live, replay);
    for field in ["fromRevision", "toRevision", "clientKey", "graphHash"] {
        assert!(replay["payload"].get(field).is_none());
    }
    for field in ["baseSemanticInputHash", "semanticInputHash"] {
        assert!(replay["payload"]["changes"].get(field).is_none());
    }
    let failure = crate::messages::tool_result_json(Err(CapabilityFailure::new(
        CapabilityFailureCode::RevisionConflict,
    )
    .with_detail("expectedRevision", "21")
    .with_detail("actualRevision", "23")))
    .unwrap();
    assert!(
        failure["failure"]["details"]
            .get("expectedRevision")
            .is_none()
    );
    assert!(failure["failure"]["details"]["nextStep"].is_string());
}
