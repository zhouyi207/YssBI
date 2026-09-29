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
        serde_json::to_value(capability_input_schema(CapabilityId::ApplyGraphEdit)).unwrap();
    assert!(
        schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("graphHash"))
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

    let request = AutomationCapabilityRequest::SearchNodeCatalog(SearchNodeCatalogRequest {
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
    assert!(CAPABILITY_DESCRIPTORS[..6].iter().all(|descriptor| {
        descriptor.effect == ToolEffect::Inspect && descriptor.approval == ApprovalPolicy::Automatic
    }));
    assert_eq!(
        CapabilityId::ApplyGraphEdit.descriptor().approval,
        ApprovalPolicy::Automatic
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
