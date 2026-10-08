use serde_json::json;
use yss_plugin_protocol::*;

fn manifest() -> PluginManifest {
    serde_json::from_value(json!({
        "schemaVersion":1,"id":"example.native","name":"Native","description":"","publisher":"example",
        "version":"0.1.0","hostApi":"^1","protocol":{"major":PROTOCOL_MAJOR,"minMinor":0,"maxMinor":PROTOCOL_MINOR,"requiredFeatures":[]},
        "target":"x86_64-unknown-linux-gnu","executable":"bin/plugin","execution":"trustedNative",
        "contributes":{"views":[{"id":"main","title":"Main","entry":"main.view.json","location":"editor","scope":"application"}],
            "commands":[{"id":"calculate","title":"Calculate"}],"taskTypes":[{"id":"calculate","producesArtifacts":false}]},
        "permissions":[],"uiMethods":["commands.execute","tasks.start","tasks.get","views.open"],"resourceBudget":ResourceBudget::default()
    })).unwrap()
}
fn view() -> NativeView {
    serde_json::from_value(json!({"description":"Native","fields":[
        {"id":"number","label":"Number","input":{"kind":"number","value":12}},
        {"id":"mode","label":"Mode","input":{"kind":"choice","value":"Square","options":["Square","Preview"]}}
    ],"actions":[{"id":"calculate","label":"Calculate","operation":{"kind":"executeCommand","commandId":"calculate"}}]})).unwrap()
}

#[test]
fn native_contract_roundtrips_typed_forms_and_rejects_html_and_unknown_fields() {
    let mut manifest = manifest();
    manifest.validate().unwrap();
    let view = view();
    view.validate(&manifest).unwrap();
    let session = ViewSession {
        session_id: "session".into(),
        view,
        installation_generation: "1".into(),
    };
    let encoded = serde_json::to_value(session).unwrap();
    assert!(encoded.get("html").is_none());
    serde_json::from_value::<ViewSession>(encoded.clone())
        .unwrap()
        .view
        .validate(&manifest)
        .unwrap();
    let mut invalid = encoded;
    invalid["view"]["fields"][0]["input"]["script"] = json!("execute");
    assert!(serde_json::from_value::<ViewSession>(invalid).is_err());
    manifest.contributes.views[0].entry = "main.html".into();
    assert_eq!(
        manifest.validate().unwrap_err().code,
        "plugin_manifest_invalid"
    );
}

#[test]
fn native_form_validation_bounds_controls_and_authorizes_actions_and_replies() {
    let manifest = manifest();
    let mut form = view();
    form.fields.push(form.fields[0].clone());
    assert_eq!(
        form.validate(&manifest).unwrap_err().code,
        "plugin_view_invalid"
    );
    let mut form = view();
    form.fields[1].input = NativeInput::Choice {
        value: "Missing".into(),
        options: vec!["Square".into()],
    };
    assert_eq!(
        form.validate(&manifest).unwrap_err().code,
        "plugin_view_invalid"
    );
    let mut form = view();
    form.actions[0].operation = NativeOperation::ExecuteCommand {
        command_id: "undeclared".into(),
    };
    assert_eq!(
        form.validate(&manifest).unwrap_err().code,
        "plugin_method_denied"
    );
    let mut form = view();
    form.actions[0].operation = NativeOperation::StartTask {
        task_type: "calculate".into(),
    };
    form.validate(&manifest).unwrap();
    let mut no_poll = manifest.clone();
    no_poll.ui_methods.retain(|method| method != "tasks.get");
    assert_eq!(
        form.validate(&no_poll).unwrap_err().code,
        "plugin_method_denied"
    );
    let reply = NativeCommandReply {
        message: "Calculated".into(),
        view: Some(form),
    };
    reply.validate(&manifest).unwrap();
    let mut oversized = view();
    oversized.description = "x".repeat(16 * 1024 + 1);
    assert_eq!(
        oversized.validate(&manifest).unwrap_err().code,
        "plugin_view_invalid"
    );
}
