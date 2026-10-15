use crate::contract::*;
use crate::contract::model::CapabilityInput;
use crate::tools::decode_input as decode_request;
use serde_json::{Value, json};

fn extension() -> crate::AssistantExtension {
    crate::AssistantExtension::new(std::sync::Arc::new(
        crate::test_support::RejectingCapabilityGateway,
    ))
    .unwrap()
}

/// The single live and replayed model projection of a recorded result.
fn tool_result_json(tool: &str, outcome: Result<AutomationCapabilityResult, CapabilityFailure>) -> Value {
    let extension = extension();
    match outcome {
        Ok(result) => extension
            .project_result(tool, &encode_result(&result).unwrap())
            .unwrap(),
        Err(failure) => json!({"state": "failed", "failure": extension.project_failure(&failure)}),
    }
}

#[test]
fn delegation_arguments_accept_business_scope_and_reject_internal_authority() {
    use crate::contract::model::AgentTaskInput;
    let task = json!({
        "worker": "review", "objective": "Review evidence", "constraints": "Read only",
        "completionCriteria": "Return findings", "dependsOn": [],
        "scope": { "resources": [{"resource": {"kind": "doc", "id": "docs/report.md"}, "operations": ["inspect"]}],
            "results": [], "creations": [], "exportPaths": [] }
    });
    let schema = Value::from(schemars::schema_for!(AgentTaskInput));
    let decoded = decode_arguments::<AgentTaskInput>(task.clone(), &schema).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), task);
    for field in ["key", "revision", "sessionId"] {
        let mut invalid = task.clone();
        invalid[field] = json!("internal");
        assert!(decode_arguments::<AgentTaskInput>(invalid, &schema).is_err());
    }
    let mut invalid = task;
    invalid["scope"]["resources"][0]["version"] =
        json!({"revision": 7, "sessionId": "edit-session"});
    assert!(decode_arguments::<AgentTaskInput>(invalid, &schema).is_err());
    let schema = followup_task_schema();
    decode_arguments::<AgentFollowupInput>(
        json!({"runId": "worker", "instruction": "Continue"}),
        &schema,
    )
    .unwrap();
    assert!(
        decode_arguments::<AgentFollowupInput>(
            json!({"runId": "worker", "instruction": "Continue", "resourceVersions": []}),
            &schema
        )
        .is_err()
    );
}

#[test]
fn provider_arguments_decode_through_the_registered_resource_schemas() {
    let reference = crate::contract::TableRef::new(
        crate::contract::ResultRef::new("session".into(), u64::MAX),
        Some("structured:/values".into()),
    );
    let schema = crate::contract::capability_input_schema(CapabilityId::ReadResultTable);
    let decoded: crate::contract::ReadResultTableRequest = decode_arguments(
        json!({"tableRef": reference, "columns": ["value"], "offset": 7, "limit": 3}),
        schema.as_value(),
    )
    .unwrap();
    assert_eq!(decoded.table_ref, reference);
    assert_eq!(decoded.offset, 7);
    assert!(
        decode_arguments::<crate::contract::ReadResultTableRequest>(
            json!({"tableRef": "invented"}),
            schema.as_value()
        )
        .is_err()
    );

    let schema = serde_json::to_value(crate::contract::capability_input_schema(
        CapabilityId::CreateConstants,
    ))
    .unwrap();
    let mut arguments = json!({"graph":{"kind":"event_graph","id":"events/Main.yssbi-event"},"constants":[
        {"clientId":"exact","name":"Exact","value":{"dataType":{"kind":"Scalar","inner":"Numeric"},"dataValue":{"Integer":"9223372036854775807"}}},
        {"clientId":"table","name":"Table","value":{"dataType":{"kind":"DataFrame"},"tabular":{"columns":{"x":[1,2]}}}}
    ]});
    let request =
        decode_request(CapabilityId::CreateConstants, arguments.clone(), &schema).unwrap();
    assert_eq!(request.capability_id(), CapabilityId::CreateConstants);
    assert_eq!(
        serde_json::to_value(request).unwrap()["payload"]["constants"][0]["value"]["dataValue"]["Integer"],
        "9223372036854775807"
    );
    arguments["constants"][0]["value"]["dataValue"]["Integer"] = json!(42);
    assert!(decode_request(CapabilityId::CreateConstants, arguments, &schema).is_err());
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
            serde_json::to_value(crate::contract::capability_input_schema(capability))
                .unwrap();
        let decode = |arguments| match capability {
            CapabilityId::SearchKnowledge => {
                decode_arguments::<SearchKnowledgeRequest>(arguments, &schema)
                    .map(|request| serde_json::to_value(request).unwrap())
            }
            CapabilityId::ReadKnowledge => {
                decode_arguments::<ReadKnowledgeRequest>(arguments, &schema)
                    .map(|request| serde_json::to_value(request).unwrap())
            }
            _ => unreachable!(),
        };
        let request = decode(arguments.clone()).unwrap();
        for (field, value) in arguments.as_object().unwrap() {
            assert_eq!(&request[field], value);
        }
        for field in ["project", "revision", "sourceHash", "version"] {
            let mut invalid = arguments.clone();
            invalid[field] = json!("internal");
            assert!(decode(invalid).is_err());
        }
    }
    let schema = serde_json::to_value(crate::contract::capability_input_schema(
        CapabilityId::InspectGraph,
    ))
    .unwrap();
    let overview = decode_request(
        CapabilityId::InspectGraph,
        json!({"graph":{"kind":"event_graph","id":"events/Main.yssbi-event"}}),
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
    assert!(
        decode_request(
            CapabilityId::InspectGraph,
            json!({"graph":{"kind":"event_graph","id":"events/Main.yssbi-event"}, "view":"full"}),
            &schema,
        )
        .is_err()
    );
    assert!(
        decode_request(
            CapabilityId::InspectGraph,
            json!({"graph":{"kind":"database","id":"data"}}),
            &schema,
        )
        .is_err()
    );
    let schema = serde_json::to_value(crate::contract::capability_input_schema(
        CapabilityId::InspectNodes,
    ))
    .unwrap();
    let nodes = decode_request(CapabilityId::InspectNodes,
        json!({"graph":{"kind":"event_graph","id":"events/Main.yssbi-event"}, "nodeIds":["node-1"], "fields":["parameters","schema"], "portOffset":10, "portLimit":5}), &schema).unwrap();
    let CapabilityInput::InspectNodes(nodes) = nodes else {
        panic!("nodes")
    };
    nodes.validate().unwrap();
    let schema = serde_json::to_value(crate::contract::capability_input_schema(
        CapabilityId::CreateResource,
    ))
    .unwrap();
    let request = decode_request(
        CapabilityId::CreateResource,
        json!({
            "kind": "doc", "name": "Report"
        }),
        &schema,
    )
    .unwrap();
    assert!(matches!(
        request,
        CapabilityInput::CreateResource(
            crate::contract::model::CreateResourceInput::Doc { .. }
        )
    ));

    let schema = serde_json::to_value(crate::contract::capability_input_schema(
        CapabilityId::ReplaceDocumentText,
    ))
    .unwrap();
    let mut input = json!({
        "document": {"kind": "doc", "id": "docs/Report.md"},
        "replacements": [{"oldText": "原文", "newText": "正文"}]
    });
    let request =
        decode_request(CapabilityId::ReplaceDocumentText, input.clone(), &schema).unwrap();
    assert_eq!(serde_json::to_value(&request).unwrap()["payload"], input);
    input["version"] = json!({"revision": 2, "sessionId": "doc-session"});
    assert!(decode_request(CapabilityId::ReplaceDocumentText, input, &schema).is_err());
    let schema = serde_json::to_value(crate::contract::capability_input_schema(
        CapabilityId::UpdateChart,
    ))
    .unwrap();
    let mut input =
        json!({"chart":{"kind":"chart","id":"charts/Trend.yssbi-chart"},"settings":{"y":null}});
    let decoded = decode_request(CapabilityId::UpdateChart, input.clone(), &schema).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap()["payload"], input);
    for field in ["databaseId", "chartType"] {
        input["settings"] = json!({field:null});
        assert!(decode_request(CapabilityId::UpdateChart, input.clone(), &schema).is_err());
    }
}

#[test]
fn model_capabilities_hide_concurrency_fields_in_schemas_calls_and_live_replay_results() {
    let hidden = [
        "revision",
        "expectedRevision",
        "baseRevision",
        "graphHash",
        "semanticInputHash",
        "observationHash",
        "clientKey",
        "ifUnchanged",
        "sessionId",
        "executionSessionId",
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
    // Current workflow operations can appear in history without admitting model calls.
    for internal in [
        CapabilityId::ApplyGraphEdit,
        CapabilityId::SaveGraph,
        CapabilityId::InspectDatasetSchema,
        CapabilityId::InspectDatasetProfile,
    ] {
        let schema = serde_json::to_value(capability_input_schema(internal)).unwrap();
        assert_eq!(schema, json!(false));
        assert!(decode_request(internal, json!({}), &schema).is_err());
    }
    for (capability, input) in [
        (
            CapabilityId::InspectDocument,
            json!({"document":{"kind":"doc","id":"docs/report.md"}}),
        ),
        (
            CapabilityId::ReadDocument,
            json!({"document":{"kind":"doc","id":"docs/report.md"},"section":{"headingIndex":1,"start":20}}),
        ),
        (
            CapabilityId::SearchDocument,
            json!({"document":{"kind":"doc","id":"docs/report.md"},"query":"Result","range":{"start":0,"end":200}}),
        ),
        (
            CapabilityId::ReplaceDocumentText,
            json!({"document":{"kind":"doc","id":"docs/report.md"},"replacements":[{"oldText":"before","newText":"after"}]}),
        ),
        (
            CapabilityId::AppendDocument,
            json!({"document":{"kind":"doc","id":"docs/report.md"},"text":"Append"}),
        ),
        (
            CapabilityId::WriteDocument,
            json!({"document":{"kind":"doc","id":"docs/report.md"},"markdown":"# Report"}),
        ),
        (
            CapabilityId::InspectMind,
            json!({"mind":{"kind":"mind","id":"minds/plan.yssbi-mind"}}),
        ),
        (
            CapabilityId::FindTopics,
            json!({"mind":{"kind":"mind","id":"minds/plan.yssbi-mind"},"query":"result"}),
        ),
        (
            CapabilityId::InspectTopics,
            json!({"mind":{"kind":"mind","id":"minds/plan.yssbi-mind"},"topicIds":["root"]}),
        ),
        (
            CapabilityId::CreateTopics,
            json!({"mind":{"kind":"mind","id":"minds/plan.yssbi-mind"},"topics":[{"clientId":"child","parentId":"root","content":"Result"}]}),
        ),
        (
            CapabilityId::UpdateTopics,
            json!({"mind":{"kind":"mind","id":"minds/plan.yssbi-mind"},"topics":[{"topicId":"root","reference":null}]}),
        ),
        (
            CapabilityId::MoveTopics,
            json!({"mind":{"kind":"mind","id":"minds/plan.yssbi-mind"},"topics":[{"topicId":"leaf","parentId":"root"}]}),
        ),
        (
            CapabilityId::DeleteTopics,
            json!({"mind":{"kind":"mind","id":"minds/plan.yssbi-mind"},"topicIds":["leaf"]}),
        ),
        (
            CapabilityId::DuplicateTopics,
            json!({"mind":{"kind":"mind","id":"minds/plan.yssbi-mind"},"topicIds":["leaf"],"parentId":"root"}),
        ),
        (
            CapabilityId::InspectDatabase,
            json!({"database":{"kind":"database","id":"data"}}),
        ),
        (
            CapabilityId::InspectDatabaseSchema,
            json!({"database":{"kind":"database","id":"data"},"columns":["x"],"limit":1}),
        ),
        (
            CapabilityId::ProfileDatabase,
            json!({"database":{"kind":"database","id":"data"},"columns":["x"],"metrics":["statistics"]}),
        ),
        (
            CapabilityId::ReadDatabaseRows,
            json!({"database":{"kind":"database","id":"data"},"columns":["x"],"filters":[{"column":"x","comparison":"is_null"}],"order":[{"column":"x","ascending":true,"nullsFirst":false}]}),
        ),
        (
            CapabilityId::ExportDatabase,
            json!({"database":{"kind":"database","id":"data"},"path":"output.csv","format":"csv"}),
        ),
        (
            CapabilityId::ImportDatabase,
            json!({"name":"Chosen name","source":{"kind":"csv","path":"input.csv","delimiter":",","hasHeader":true,"inferSchemaLength":10}}),
        ),
        (
            CapabilityId::MoveNodes,
            json!({"graph":{"kind":"event_graph","id":"events/analysis.yssbi-event"},"positions":[{"nodeId":"node","x":1,"y":2}]}),
        ),
        (
            CapabilityId::ExecuteGraph,
            json!({"graph":{"kind":"event_graph","id":"events/analysis.yssbi-event"}}),
        ),
        (
            CapabilityId::ExecuteGraph,
            json!({"graph":{"kind":"event_graph","id":"events/analysis.yssbi-event"},"nodeId":"node","mode":"currentInputs"}),
        ),
        (
            CapabilityId::ValidateGraph,
            json!({"graph":{"kind":"event_graph","id":"events/analysis.yssbi-event"},"nodeIds":["node"],"offset":0,"limit":1}),
        ),
        (
            CapabilityId::SaveResource,
            json!({"resource":{"kind":"doc","id":"docs/report.md"}}),
        ),
        (
            CapabilityId::UndoResource,
            json!({"resource":{"kind":"event_graph","id":"events/analysis.yssbi-event"}}),
        ),
        (
            CapabilityId::RedoResource,
            json!({"resource":{"kind":"database","id":"data"}}),
        ),
        (
            CapabilityId::InsertRows,
            json!({"database":{"kind":"database","id":"data"},"rows":[{"x":1,"label":"A"},{}],"beforeRowId":4}),
        ),
        (
            CapabilityId::UpdateCells,
            json!({"database":{"kind":"database","id":"data"},"cells":[{"rowId":4,"column":"x","value":null}]}),
        ),
        (
            CapabilityId::DeleteRows,
            json!({"database":{"kind":"database","id":"data"},"rowIds":[4,9]}),
        ),
        (
            CapabilityId::CreateColumns,
            json!({"database":{"kind":"database","id":"data"},"columns":[{"name":"flag","dtype":"Bool"}]}),
        ),
        (
            CapabilityId::RenameColumns,
            json!({"database":{"kind":"database","id":"data"},"columns":[{"column":"x","name":"value"}]}),
        ),
        (
            CapabilityId::DeleteColumns,
            json!({"database":{"kind":"database","id":"data"},"columns":["x"]}),
        ),
        (
            CapabilityId::CastColumns,
            json!({"database":{"kind":"database","id":"data"},"columns":[{"column":"x","dtype":"Int64","force":false}]}),
        ),
        (
            CapabilityId::SetColumnSemantics,
            json!({"database":{"kind":"database","id":"data"},"columns":[{"column":"x","semantic":{"kind":"Identifier","values":[],"positiveValue":null,"numeric":null}}]}),
        ),
        (
            CapabilityId::EditResource,
            json!({"resource":{"kind":"function_graph","id":"functions/fit.yssbi-function"},"edit":{"kind":"function_signature","signature":{"parameters":[],"returnType":"core.numeric"}}}),
        ),
        (
            CapabilityId::RequestUiIntent,
            json!({"intent":{"kind":"showPanel","panel":"project"}}),
        ),
        (
            CapabilityId::RequestUiIntent,
            json!({"intent":{"kind":"openResult","resultRef":ResultRef::new("11111111-1111-4111-8111-111111111111".into(), 7)}}),
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
created_constants: Default::default(),
        graph_path: "events/analysis.yssbi-event".into(), from_revision: 21, to_revision: 22,
        graph_hash: "b".repeat(64), client_key: "host-key".into(),
        created_nodes: [("fit".into(), "real-node-id".into())].into(), created_ports: Default::default(),
        changes: GraphEditChanges {
            base_semantic_input_hash: "c".repeat(64), semantic_input_hash: "d".repeat(64),
            nodes: vec![GraphNodeInspection {
                node_id: "real-node-id".into(), node_type_id: "yssbi.numeric.multiply".into(),
                user_label: Some("Actual label".into()), x: 10.0, y: 20.0, title: "Multiply".into(),
                parameters: vec![GraphParameterInspection {
                    key: "user_value".into(), title: "User value".into(), editor: "select".into(),
                    value: Some(json!({"revision":19})), options: Some(vec!["column".into()]), context_hint: None,
                }],
                ports: vec![GraphPortFacts {
                    address: GraphEditPortRef::Declared { node_id: "real-node-id".into(), port_key: "left".into() },
                    label: "Left".into(), direction: "input".into(), data_type: "core.numeric".into(),
                    accepted_type: "core.numeric".into(), orphan: false, maximum_connections: Some(1), connection_count: 0,
                    schema: [("column".into(), "core.numeric".into())].into(), literal: Some(json!({"revision":20})),
                }], port_templates: vec![],
            }], removed_node_ids: vec![], connections: vec![], removed_connection_ids: vec![],
            constants: [("c1".into(), json!({"id":"c1","name":"Example","contentHash":"internal-constant-hash","dataValue":{"revision":19},"valueIncluded":true}))].into(),
            removed_constant_ids: vec![], ready: true, diagnostics: vec![],
        },
    });
    let live = tool_result_json("apply_graph_edit", Ok(result.clone()));
    assert_eq!(live["payload"]["createdNodes"]["fit"], "real-node-id");
    let node = &live["payload"]["changes"]["nodes"][0];
    assert_eq!(node["parameters"][0]["value"], json!({"revision":19}));
    assert_eq!(node["ports"][0]["literal"], json!({"revision":20}));
    assert_eq!(node["ports"][0]["address"]["nodeId"], "real-node-id");
    assert!(node["parameters"][0].get("options").is_none());
    assert!(node["ports"][0].get("schema").is_none());
    let constant = &live["payload"]["changes"]["constants"]["c1"];
    assert!(constant.get("contentHash").is_none());
    assert_eq!(constant["dataValue"], json!({"revision":19}));
    // Internal owner calls replay with the model-facing arguments the ledger records for them.
    let arguments = crate::run::internal_call(&internal).unwrap().arguments;
    for field in hidden {
        assert!(arguments.get(field).is_none());
    }
    for field in ["fromRevision", "toRevision", "clientKey", "graphHash"] {
        assert!(live["payload"].get(field).is_none());
    }
    for field in ["baseSemanticInputHash", "semanticInputHash"] {
        assert!(live["payload"]["changes"].get(field).is_none());
    }
    let failure = tool_result_json(
        "apply_graph_edit",
        Err(CapabilityFailure::new(CapabilityFailureCode::REVISION_CONFLICT)
            .with_detail("expectedRevision", "21")
            .with_detail("actualRevision", "23")),
    );
    assert!(
        failure["failure"]["details"]
            .get("expectedRevision")
            .is_none()
    );
    assert!(failure["failure"]["details"]["nextStep"].is_string());
    assert_eq!(failure["failure"]["code"], "resource_changed");
}

#[test]
fn actionable_failures_replay_without_internal_sync_diagnostics() {
    for (index, failure) in [
        CapabilityFailure::new(CapabilityFailureCode::REVISION_CONFLICT)
            .with_detail("resourceId", "docs/report.md")
            .with_detail("expectedRevision", "9001")
            .with_detail("sessionId", "private-session"),
        CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST)
            .with_detail("capabilityId", "inspect_result")
            .with_detail("field", "schemaLimit")
            .with_detail("nextStep", "Use a schemaLimit between 1 and 100."),
        CapabilityFailure::new(CapabilityFailureCode::INVALID_REQUEST)
            .with_detail("capabilityId", "edit_resource")
            .with_detail("field", "version.sessionId"),
        CapabilityFailure::new(CapabilityFailureCode::REVISION_CONFLICT)
            .with_detail("reason", "resource_requires_current_read")
            .with_detail("resourceId", "docs/report.md")
            .with_detail("expectedRevision", "9001")
            .with_detail("sessionId", "private-session"),
    ]
    .into_iter()
    .enumerate()
    {
        let live = tool_result_json("inspect_result", Err(failure.clone()));
        if failure.code == CapabilityFailureCode::REVISION_CONFLICT {
            assert_eq!(
                live["failure"]["code"],
                if index == 3 {
                    "resource_read_required"
                } else {
                    "resource_changed"
                }
            );
            assert_eq!(live["failure"]["details"]["resourceId"], "docs/report.md");
        } else if failure.details["field"] == "schemaLimit" {
            assert_eq!(live["failure"]["details"]["field"], "schemaLimit");
        } else {
            assert!(live["failure"]["details"].get("field").is_none());
        }
        let encoded = live.to_string();
        for internal in [
            "revision",
            "Revision",
            "sessionId",
            "private-session",
            "9001",
        ] {
            assert!(!encoded.contains(internal), "{encoded}");
        }
    }
}
