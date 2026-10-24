#[test]
fn graph_port_inspection_uses_current_field_names() {
    for (current, old) in [
        (
            serde_json::json!({"kind": "declared", "nodeId": "n", "portKey": "value"}),
            serde_json::json!({"kind": "declared", "node_id": "n", "port_key": "value"}),
        ),
        (
            serde_json::json!({"kind": "instance", "nodeId": "n", "templateKey": "x", "instanceId": "i"}),
            serde_json::json!({"kind": "instance", "node_id": "n", "template_key": "x", "instance_id": "i"}),
        ),
    ] {
        let port: super::GraphPortInspection = serde_json::from_value(current.clone()).unwrap();
        assert_eq!(serde_json::to_value(port).unwrap(), current);
        assert!(serde_json::from_value::<super::GraphPortInspection>(old).is_err());
    }
}

use super::*;

#[test]
fn resource_identity_reads_and_domain_page_limits_have_distinct_schemas() {
    let input = serde_json::json!({"resource":{"kind":"doc","id":"docs/Report.md"}});
    let request: InspectResourceRequest =
        serde_json::from_value::<model::InspectResourceInput>(input.clone())
            .unwrap()
            .into();
    request.validate().unwrap();
    for field in ["limit", "offset", "graphView", "metadataOnly"] {
        let mut invalid = input.clone();
        invalid[field] = serde_json::Value::Null;
        assert!(serde_json::from_value::<model::InspectResourceInput>(invalid).is_err());
    }
    let read: model::ReadDocumentInput = serde_json::from_value(
        serde_json::json!({"document":{"kind":"doc","id":"docs/Report.md"}}),
    )
    .unwrap();
    assert_eq!(read.limit, 8192);
    let mut invalid = read;
    invalid.limit = 16385;
    let failure = model::DocumentReadInput::Text(invalid)
        .validate()
        .unwrap_err();
    let failure = contract_failure(failure, CapabilityId::DocumentRead);
    assert_eq!(model::failure(&failure)["details"]["field"], "limit");
    let schema = capability_input_schema(CapabilityId::ResultReadTable);
    assert_eq!(schema.as_value()["properties"]["limit"]["maximum"], 1000);
    for capability in [
        CapabilityId::ProjectUndoResource,
        CapabilityId::ProjectRedoResource,
    ] {
        let schema = capability_input_schema(capability);
        assert_eq!(
            schema.as_value()["$defs"]["ProjectResourceKind"]["enum"],
            serde_json::json!(["event_graph", "function_graph", "database"])
        );
    }
}

#[test]
fn graph_tool_contracts_require_the_facts_used_by_followup_edits() {
    let mut request = serde_json::json!({
        "graphPath": "events/Main.yssbi-event", "baseRevision": 1,
        "graphHash": "0".repeat(64), "clientKey": "batch", "locale": "en-US",
        "operations": [{"type": "move_nodes", "payload": {
            "positions": [{"nodeId": "node-1", "x": 10.0, "y": 20.0}]
        }}]
    });
    AutomationCapabilityRequest::ApplyGraphEdit(
        serde_json::from_value::<ApplyGraphEditRequest>(request.clone()).unwrap(),
    )
    .validate()
    .unwrap();
    request.as_object_mut().unwrap().remove("graphHash");
    assert!(serde_json::from_value::<ApplyGraphEditRequest>(request).is_err());
    let schema =
        serde_json::to_value(capability_input_schema(CapabilityId::GraphMoveNodes)).unwrap();
    for field in ["graphHash", "baseRevision", "clientKey"] {
        assert!(schema["properties"].get(field).is_none());
    }
    assert!(
        schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("positions"))
    );

    let mut node = serde_json::json!({
        "nodeId": "node-1", "nodeTypeId": "yssbi.numeric.multiply",
        "x": 10.0, "y": 20.0, "title": "Multiply",
        "parameters": [], "ports": [], "portTemplates": []
    });
    serde_json::from_value::<GraphNodeInspection>(node.clone()).unwrap();
    node.as_object_mut().unwrap().remove("ports");
    assert!(serde_json::from_value::<GraphNodeInspection>(node).is_err());
    let schema = serde_json::to_value(schemars::schema_for!(GraphEditReceipt)).unwrap();
    assert!(
        schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("graphHash"))
    );
}

#[test]
fn identities_and_requests_reject_ambiguous_or_unbounded_input() {
    assert!(PrincipalId::try_new(" ").is_err());
    assert!(HarnessSessionId::try_new("session-1").is_ok());

    let request = AutomationCapabilityRequest::BrowseNodes(BrowseNodesRequest {
        category: None,
        offset: 0,
        query: "regression".to_owned(),
        locale: "en-US".to_owned(),
        limit: 0,
    });
    assert_eq!(
        request.validate(),
        Err(CapabilityContractError::InvalidLimit {
            maximum: MAX_CATALOG_SEARCH_RESULTS,
        })
    );
}

#[test]
fn capability_registry_is_closed_and_schema_generation_is_available() {
    assert!(
        CAPABILITY_DESCRIPTORS[..6]
            .iter()
            .all(|descriptor| descriptor.effect == ToolEffect::Inspect)
    );
    assert_eq!(
        CapabilityId::GraphApplyEdit.descriptor().effect,
        ToolEffect::Mutate
    );
    assert_eq!(
        CapabilityId::GraphSave.descriptor().id,
        CapabilityId::GraphSave
    );
    let _request_schema = schemars::schema_for!(AutomationCapabilityRequest);
    let _result_schema = schemars::schema_for!(AutomationCapabilityResult);
    for capability in CAPABILITY_DESCRIPTORS {
        assert_eq!(capability.id.descriptor(), &capability);
        assert_eq!(
            serde_json::to_value(capability.id).unwrap(),
            serde_json::json!(capability.id.as_str())
        );
        let schema = serde_json::to_value(capability_input_schema(capability.id)).unwrap();
        assert_eq!(schema["type"], "object", "{}", capability.id.as_str());
    }
}

#[test]
fn capability_names_are_domain_prefixed() {
    const DOMAINS: [&str; 12] = [
        "project",
        "data",
        "document",
        "mind",
        "chart",
        "graph",
        "catalog",
        "result",
        "knowledge",
        "web",
        "python",
        "ui",
    ];
    for capability in CAPABILITY_DESCRIPTORS {
        let name = capability.id.as_str();
        let domain = name.split_once('_').map(|(domain, _)| domain);
        assert!(
            domain.is_some_and(|domain| DOMAINS.contains(&domain)),
            "{name} must start with a tool domain"
        );
    }
}

#[test]
fn result_ui_intentions_preserve_opaque_references_in_live_results_and_history() {
    let reference = ResultRef::new("11111111-1111-4111-8111-111111111111".into(), 42);
    let arguments = serde_json::json!({"intent":{"kind":"openResult","resultRef":reference}});
    let input: model::RequestUiIntentInput = serde_json::from_value(arguments.clone()).unwrap();
    let request = RequestUiIntent {
        client_key: "private-call-key".into(),
        input,
    };
    request.validate().unwrap();
    let internal: yss_ui_contract::RequestUiIntent = request.clone().into();
    assert!(
        matches!(&internal.intent, yss_ui_contract::UiIntent::OpenResult { source }
        if source.execution_session_id == reference.execution_session_id() && source.result_id == "42")
    );
    let stored = AutomationCapabilityRequest::RequestUiIntent(request);
    let replay: AutomationCapabilityRequest =
        serde_json::from_value(serde_json::to_value(stored).unwrap()).unwrap();
    let public = serde_json::to_value(model::CapabilityInput::from(&replay)).unwrap();
    assert_eq!(public["payload"], arguments);
    let receipt = yss_ui_contract::UiIntentReceipt {
        id: "intent-1".into(),
        intent: internal.intent,
        status: yss_ui_contract::UiIntentStatus::Pending,
    };
    for result in [
        AutomationCapabilityResult::UiIntentReceipt(receipt.clone()),
        AutomationCapabilityResult::UiIntentInspection(receipt),
    ] {
        let result = model::capability_result(&result).unwrap();
        assert_eq!(result["payload"]["intent"], arguments["intent"]);
        assert_eq!(result["payload"]["id"], "intent-1");
        assert!(!result.to_string().contains("executionSessionId"));
    }
    let schema = serde_json::to_value(capability_input_schema(CapabilityId::UiOpen)).unwrap();
    for private in ["executionSessionId", "clientKey", "revision"] {
        assert!(!schema.to_string().contains(private));
        assert!(!public.to_string().contains(private));
    }
    assert!(serde_json::from_value::<model::RequestUiIntentInput>(serde_json::json!({"intent": {
        "kind":"openResult", "source":{"executionSessionId":reference.execution_session_id(),"resultId":"42"}
    }})).is_err());
    let invalid = RequestUiIntent {
        client_key: "call".into(),
        input: model::RequestUiIntentInput {
            intent: model::UiIntentInput::OpenResult {
                result_ref: ResultRef::new(reference.execution_session_id().into(), 0),
            },
        },
    };
    assert!(invalid.validate().is_err());
}

#[test]
fn argument_diagnostics_use_business_schema_paths_without_echoing_values() {
    let schema =
        serde_json::to_value(capability_input_schema(CapabilityId::CatalogSearchNodes)).unwrap();
    for (arguments, category, path, expected) in [
        (
            serde_json::json!({"query": "secret"}),
            "missing_field",
            "$.locale",
            "string",
        ),
        (
            serde_json::json!({"query": "secret", "locale": "en-US", "limit": "secret"}),
            "invalid_type",
            "$.limit",
            "integer",
        ),
    ] {
        let failure = decode_arguments::<BrowseNodesRequest>(arguments, &schema).unwrap_err();
        assert_eq!(failure.details["category"], category);
        assert_eq!(failure.details["path"], path);
        assert!(failure.details["expected"].contains(expected));
        assert!(!serde_json::to_string(&failure).unwrap().contains("secret"));
    }
    let schema =
        serde_json::to_value(capability_input_schema(CapabilityId::GraphMoveNodes)).unwrap();
    for payload in [
        serde_json::json!({"nodeId": "secret", "x": "secret", "y": 0}),
        serde_json::json!({"nodeId": "secret", "y": 0}),
    ] {
        let failure = decode_arguments::<model::MoveNodesInput>(
            serde_json::json!({
                "graph": {"kind":"event_graph", "id":"secret"}, "positions": [payload]
            }),
            &schema,
        )
        .unwrap_err();
        assert_eq!(failure.details["path"], "$.positions[0].x");
        assert!(failure.details["expected"].contains("number"));
        assert!(!serde_json::to_string(&failure).unwrap().contains("secret"));
    }

    let schema = serde_json::to_value(statistical_plan_schema()).unwrap();
    let failure =
        decode_arguments::<StatisticalPlan>(serde_json::json!({"analysisMode": "secret"}), &schema)
            .unwrap_err();
    assert_eq!(failure.code, CapabilityFailureCode::INVALID_REQUEST);
    assert_eq!(failure.details["category"], "invalid_enum_value");
    assert_eq!(failure.details["path"], "$.analysisMode");
    assert!(failure.details["expected"].contains("confirmatory"));
    assert!(!serde_json::to_string(&failure).unwrap().contains("secret"));
    let failure = decode_arguments::<StatisticalPlan>(
        serde_json::json!({
            "selectedWorkflow": "description instead of identifier".repeat(10),
        }),
        &schema,
    )
    .unwrap_err();
    assert_eq!(failure.details["path"], "$.selectedWorkflow");
    assert!(failure.details["expected"].contains("maxLength=128"));
    assert!(failure.details["expected"].contains("workflow identifier"));
}
