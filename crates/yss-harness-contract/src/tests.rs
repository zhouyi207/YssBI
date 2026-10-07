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
        .unwrap_err()
        .into_failure(CapabilityId::ReadDocument);
    assert_eq!(model::failure(&failure)["details"]["field"], "limit");
    let schema = capability_input_schema(CapabilityId::ReadResultTable);
    assert_eq!(schema.as_value()["properties"]["limit"]["maximum"], 1000);
    for capability in [CapabilityId::UndoResource, CapabilityId::RedoResource] {
        let schema = capability_input_schema(capability);
        assert_eq!(
            schema.as_value()["$defs"]["ProjectResourceKind"]["enum"],
            serde_json::json!(["event_graph", "function_graph", "database"])
        );
    }
}

#[test]
fn model_catalog_preserves_shared_provider_and_generation_configuration() {
    let wire: serde_json::Value = serde_json::from_str(include_str!(
        "../../../react/src/tests/fixtures/node-system-contracts/harness-models.json"
    ))
    .unwrap();
    let catalog: LanguageModelCatalog = serde_json::from_value(wire.clone()).unwrap();
    assert!(
        catalog
            .providers
            .iter()
            .all(|entry| entry.config.validate())
    );
    assert_eq!(serde_json::to_value(catalog).unwrap(), wire);
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
    let schema = serde_json::to_value(capability_input_schema(CapabilityId::MoveNodes)).unwrap();
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
        limit: MAX_CATALOG_RESULTS + 1,
    });
    assert_eq!(
        request.validate(),
        Err(CapabilityContractError::InvalidLimit {
            maximum: MAX_CATALOG_RESULTS,
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
        CapabilityId::ApplyGraphEdit.descriptor().effect,
        ToolEffect::Mutate
    );
    assert_eq!(
        CapabilityId::InspectDatasetSchema.descriptor().id,
        CapabilityId::InspectDatasetSchema
    );
    let _request_schema = schemars::schema_for!(AutomationCapabilityRequest);
    let _result_schema = schemars::schema_for!(AutomationCapabilityResult);
    for capability in CAPABILITY_DESCRIPTORS {
        let descriptor = ToolDescriptor::for_capability(capability.id);
        let schema = serde_json::to_value(&descriptor.input_schema).unwrap();
        assert_eq!(schema["type"], "object", "{}", capability.id.as_str());
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
    let schema =
        serde_json::to_value(capability_input_schema(CapabilityId::RequestUiIntent)).unwrap();
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
