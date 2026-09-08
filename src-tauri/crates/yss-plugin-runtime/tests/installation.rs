use ed25519_dalek::{Signer, SigningKey};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    sync::Arc,
};
use yss_plugin_protocol::*;
use yss_plugin_runtime::PluginManager;

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("yssbi-plugin-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
struct Host;
impl HostServices for Host {
    fn current_project(&self) -> Result<Option<ProjectContext>, PluginFailure> {
        Ok(None)
    }
    fn invoke(&self, _: &CallContext, _: &str, _: Value, _: &Path) -> Result<Value, PluginFailure> {
        Err(PluginFailure::new("permission_denied"))
    }
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn package(path: &Path, escape: bool) {
    let manifest = PluginManifest {
        schema_version: 1,
        id: "example.statistics".into(),
        name: "Statistics".into(),
        description: "Test fixture".into(),
        publisher: "example".into(),
        version: "1.0.0".into(),
        host_api: "^1".into(),
        protocol: ProtocolRange {
            major: 1,
            min_minor: 0,
            max_minor: 0,
            required_features: vec![],
        },
        target: "x86_64-pc-windows-msvc".into(),
        executable: "bin/plugin.exe".into(),
        execution: ExecutionMode::TrustedNative,
        contributes: Contributions {
            views: vec![PluginView {
                id: "main".into(),
                title: "Statistics".into(),
                entry: "web/index.html".into(),
                location: ViewLocation::Sidebar,
                scope: ViewScope::Application,
            }],
            commands: vec![],
            task_types: vec![],
        },
        permissions: vec![],
        ui_methods: vec![],
        resource_budget: ResourceBudget::default(),
    };
    let content = [
        ("bin/plugin.exe", b"test-executable".as_slice()),
        ("web/index.html", b"<p>Fixture</p>".as_slice()),
    ];
    let files = content
        .iter()
        .map(|(path, bytes)| FileEntry {
            path: (*path).into(),
            size: bytes.len().to_string(),
            sha256: hash(bytes),
        })
        .collect::<Vec<_>>();
    let key = SigningKey::from_bytes(&[7; 32]);
    let public = key.verifying_key().to_bytes();
    let key_id = hash(&public);
    let signed = serde_jcs::to_vec(
        &json!({"schemaVersion":1,"keyId":key_id,"manifest":manifest,"files":files}),
    )
    .unwrap();
    let signature = json!({"keyId":key_id,"publicKey":public.iter().map(|byte|format!("{byte:02x}")).collect::<String>(),"signature":key.sign(&signed).to_bytes().iter().map(|byte|format!("{byte:02x}")).collect::<String>()});
    let mut archive = zip::ZipWriter::new(fs::File::create(path).unwrap());
    for (name, bytes) in content {
        archive
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(bytes).unwrap();
    }
    for (name, value) in [
        ("plugin.json", serde_json::to_value(manifest).unwrap()),
        ("files.json", serde_json::to_value(files).unwrap()),
        ("signature.json", signature),
    ] {
        archive
            .start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        archive
            .write_all(&serde_json::to_vec(&value).unwrap())
            .unwrap();
    }
    if escape {
        archive
            .start_file("../escape", zip::write::SimpleFileOptions::default())
            .unwrap();
        archive.write_all(b"bad").unwrap();
    }
    archive.finish().unwrap();
}
#[test]
fn installation_is_digest_bound_idempotent_and_preserves_private_data_on_uninstall() {
    let root = Root::new();
    let archive = root.0.join("valid.yssplugin");
    package(&archive, false);
    let manager = PluginManager::new(&root.0, Arc::new(Host)).unwrap();
    let inspected = manager.inspect(&archive).unwrap();
    assert_eq!(
        manager
            .install(&archive, "wrong", "op-failure", true)
            .unwrap_err()
            .code,
        "plugin_package_changed"
    );
    let installed = manager
        .install(&archive, &inspected.package_digest, "op-install", true)
        .unwrap();
    assert_eq!(
        manager
            .install(&archive, &inspected.package_digest, "op-install", true)
            .unwrap()
            .installation_generation,
        installed.installation_generation
    );
    assert_eq!(manager.list().unwrap().len(), 1);
    let reopened = PluginManager::new(&root.0, Arc::new(Host)).unwrap();
    assert_eq!(
        reopened
            .install(&archive, &inspected.package_digest, "op-install", true)
            .unwrap()
            .installation_generation,
        installed.installation_generation
    );
    manager.set_enabled("example.statistics", false).unwrap();
    assert!(!manager.list().unwrap()[0].enabled);
    let data = root.0.join("extensions/data/example.statistics");
    fs::create_dir_all(&data).unwrap();
    fs::write(data.join("private.json"), "{}").unwrap();
    manager.uninstall("example.statistics").unwrap();
    assert!(manager.list().unwrap().is_empty());
    assert!(data.join("private.json").exists());
}
#[test]
fn unlisted_archive_paths_are_rejected_without_committing_an_installation() {
    let root = Root::new();
    let archive = root.0.join("bad.yssplugin");
    package(&archive, true);
    let manager = PluginManager::new(&root.0, Arc::new(Host)).unwrap();
    let inspected = manager.inspect(&archive).unwrap();
    assert!(
        manager
            .install(&archive, &inspected.package_digest, "op-install", true)
            .is_err()
    );
    assert!(manager.list().unwrap().is_empty());
    assert!(!root.0.join("extensions/escape").exists());
}
