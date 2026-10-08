//! Build a signed SDK example package for isolated native desktop acceptance.
use ed25519_dalek::{Signer, SigningKey};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::PathBuf};
use yss_plugin_protocol::*;

fn hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let executable = PathBuf::from(args.next().ok_or("expected SDK native_form executable")?);
    let output = PathBuf::from(args.next().ok_or("expected output .yssplugin path")?);
    if args.next().is_some() {
        return Err("usage: native_view_package <executable> <package.yssplugin>".into());
    }
    let view: NativeView = serde_json::from_value(json!({
        "description":"Rust native form: edit values, calculate a square, or run a cancellable task.",
        "fields":[
            {"id":"name","label":"Name","input":{"kind":"text","value":"Native","multiline":false}},
            {"id":"number","label":"Number","input":{"kind":"number","value":12}},
            {"id":"enabled","label":"Enabled","input":{"kind":"boolean","value":true}},
            {"id":"mode","label":"Mode","input":{"kind":"choice","value":"Square","options":["Square","Preview"]}},
            {"id":"notes","label":"Notes","input":{"kind":"text","value":"","multiline":true}},
            {"id":"options","label":"Options","input":{"kind":"json","value":{"precision":2}}}
        ],
        "actions":[
            {"id":"calculate","label":"Calculate","operation":{"kind":"executeCommand","commandId":"calculate"}},
            {"id":"run","label":"Run task","operation":{"kind":"startTask","taskType":"calculate"}},
            {"id":"another","label":"Open second view","operation":{"kind":"openView","viewId":"second"}}
        ]
    }))?;
    let manifest = PluginManifest {
        schema_version: 1,
        id: "example.native".into(),
        name: "Rust native example".into(),
        description: "Native form and task acceptance example".into(),
        publisher: "example".into(),
        version: "0.1.0".into(),
        host_api: "^1".into(),
        protocol: ProtocolRange {
            major: PROTOCOL_MAJOR,
            min_minor: PROTOCOL_MINOR,
            max_minor: PROTOCOL_MINOR,
            required_features: vec![],
        },
        target: yss_plugin_runtime::current_target()
            .ok_or("unsupported example host")?
            .into(),
        executable: "bin/native_form".into(),
        execution: ExecutionMode::TrustedNative,
        contributes: Contributions {
            views: ["main", "second"]
                .map(|id| PluginView {
                    id: id.into(),
                    title: format!("Native {id}"),
                    entry: "views/main.view.json".into(),
                    location: ViewLocation::Editor,
                    scope: ViewScope::Application,
                })
                .into(),
            commands: vec![PluginCommand {
                id: "calculate".into(),
                title: "Calculate".into(),
            }],
            task_types: vec![TaskType {
                id: "calculate".into(),
                produces_artifacts: false,
            }],
        },
        permissions: vec![],
        ui_methods: [
            "commands.execute",
            "tasks.start",
            "tasks.get",
            "tasks.cancel",
            "tasks.result",
            "views.open",
            "views.get_state",
            "views.set_state",
        ]
        .map(str::to_owned)
        .into(),
        resource_budget: ResourceBudget::default(),
        cache_directories: vec![],
    };
    manifest.validate().map_err(|failure| failure.code)?;
    view.validate(&manifest).map_err(|failure| failure.code)?;
    let content = [
        ("bin/native_form", fs::read(executable)?),
        ("views/main.view.json", serde_json::to_vec(&view)?),
    ];
    let files: Vec<_> = content
        .iter()
        .map(|(path, bytes)| FileEntry {
            path: (*path).into(),
            size: bytes.len().to_string(),
            sha256: hash(bytes),
        })
        .collect();
    // Public deterministic example identity; never use this key for a release.
    let key = SigningKey::from_bytes(&[17; 32]);
    let public = key.verifying_key().to_bytes();
    let key_id = hash(&public);
    let signed = serde_jcs::to_vec(
        &json!({"schemaVersion":1,"keyId":key_id,"manifest":manifest,"files":files}),
    )?;
    let signature = PackageSignature {
        key_id,
        public_key: hex::encode(public),
        signature: hex::encode(key.sign(&signed).to_bytes()),
    };
    let mut archive = zip::ZipWriter::new(fs::File::create(&output)?);
    for (name, bytes) in content {
        archive.start_file(name, zip::write::SimpleFileOptions::default())?;
        archive.write_all(&bytes)?;
    }
    for (name, bytes) in [
        ("plugin.json", serde_json::to_vec(&manifest)?),
        ("files.json", serde_json::to_vec(&files)?),
        ("signature.json", serde_json::to_vec(&signature)?),
    ] {
        archive.start_file(name, zip::write::SimpleFileOptions::default())?;
        archive.write_all(&bytes)?;
    }
    archive.finish()?;
    println!("{}", output.display());
    Ok(())
}
