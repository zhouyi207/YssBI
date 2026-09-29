use super::*;
use serde_json::json;

#[test]
fn provider_arguments_decode_through_the_registered_resource_schemas() {
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
    request.validate().unwrap();
    assert!(matches!(
        request,
        AutomationCapabilityRequest::ManageResource(
            yss_harness_contract::ManageResourceRequest::Create {
                specification: yss_harness_contract::ResourceCreation::Doc { .. }
            }
        )
    ));

    let schema = serde_json::to_value(yss_harness_contract::capability_input_schema(
        CapabilityId::EditResource,
    ))
    .unwrap();
    let mut input = json!({
        "resource": {"kind": "doc", "id": "docs/Report.md"},
        "version": {"revision": 2, "sessionId": "doc-session"},
        "edit": {"kind": "doc", "operations": [{"op": "replace_range", "start": 0, "end": 2, "markdown": "正文"}]}
    });
    let request = decode_request(CapabilityId::EditResource, input.clone(), &schema).unwrap();
    request.validate().unwrap();
    input["resource"]["kind"] = json!("mind");
    assert!(
        decode_request(CapabilityId::EditResource, input, &schema)
            .unwrap()
            .validate()
            .is_err()
    );
}
