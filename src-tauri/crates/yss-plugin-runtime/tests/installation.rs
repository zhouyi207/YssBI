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
    package_variant(path, escape, "1.0.0", 7, ResourceBudget::default());
}
fn operation(nonce: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    operation_id(now, nonce).unwrap()
}
fn package_variant(path: &Path, escape: bool, version: &str, key_seed: u8, budget: ResourceBudget) {
    let manifest = PluginManifest {
        schema_version: 1,
        id: "example.statistics".into(),
        name: "Statistics".into(),
        description: "Test fixture".into(),
        publisher: "example".into(),
        version: version.into(),
        host_api: "^1".into(),
        protocol: ProtocolRange {
            major: PROTOCOL_MAJOR,
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
        resource_budget: budget,
        cache_directories: vec!["cache".into()],
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
    let key = SigningKey::from_bytes(&[key_seed; 32]);
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
    let install = operation("install");
    let root = Root::new();
    let archive = root.0.join("valid.yssplugin");
    package(&archive, false);
    let manager = PluginManager::new(&root.0, Arc::new(Host)).unwrap();
    let inspected = manager.inspect(&archive).unwrap();
    assert_eq!(
        manager
            .install(&archive, "wrong", &operation("failure"), true, None)
            .unwrap_err()
            .code,
        "plugin_package_changed"
    );
    let installed = manager
        .install(&archive, &inspected.package_digest, &install, true, None)
        .unwrap();
    assert_eq!(
        manager
            .install(&archive, &inspected.package_digest, &install, true, None)
            .unwrap()
            .installation_generation,
        installed.installation_generation
    );
    assert_eq!(manager.list().unwrap().len(), 1);
    let reopened = PluginManager::new(&root.0, Arc::new(Host)).unwrap();
    assert_eq!(
        reopened
            .install(&archive, &inspected.package_digest, &install, true, None)
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
            .install(
                &archive,
                &inspected.package_digest,
                &operation("bad-install"),
                true,
                None
            )
            .is_err()
    );
    assert!(manager.list().unwrap().is_empty());
    assert!(!root.0.join("extensions/escape").exists());
}

#[test]
fn installation_receipts_do_not_impose_a_lifetime_install_limit() {
    let root = Root::new();
    let archive = root.0.join("package.yssplugin");
    package(&archive, false);
    let manager = PluginManager::new(&root.0, Arc::new(Host)).unwrap();
    let inspected = manager.inspect(&archive).unwrap();
    let first = operation("first");
    let receipt = manager
        .install(&archive, &inspected.package_digest, &first, true, None)
        .unwrap();
    for index in 0..520 {
        manager
            .install(
                &archive,
                &inspected.package_digest,
                &operation(&format!("install-{index}")),
                true,
                None,
            )
            .unwrap();
    }
    assert_eq!(
        manager
            .install(&archive, &inspected.package_digest, &first, true, None)
            .unwrap()
            .installation_generation,
        receipt.installation_generation
    );
    drop(manager);
    let manager = PluginManager::new(&root.0, Arc::new(Host)).unwrap();
    assert_eq!(
        manager
            .install(&archive, &inspected.package_digest, &first, true, None)
            .unwrap()
            .installation_generation,
        receipt.installation_generation
    );
}

#[test]
fn signer_rotation_requires_the_previously_trusted_identity_and_a_new_release() {
    let root = Root::new();
    let original = root.0.join("original.yssplugin");
    let changed = root.0.join("changed.yssplugin");
    package(&original, false);
    package_variant(&changed, false, "1.1.0", 8, ResourceBudget::default());
    let manager = PluginManager::new(&root.0, Arc::new(Host)).unwrap();
    let first = manager.inspect(&original).unwrap();
    manager
        .install(
            &original,
            &first.package_digest,
            &operation("first"),
            true,
            None,
        )
        .unwrap();
    let next = manager.inspect(&changed).unwrap();
    assert_eq!(
        next.previous_signer_key.as_deref(),
        Some(first.signer_key.as_str())
    );
    assert_eq!(
        manager
            .install(
                &changed,
                &next.package_digest,
                &operation("rejected"),
                true,
                None
            )
            .unwrap_err()
            .code,
        "plugin_signer_change_requires_approval"
    );
    manager
        .install(
            &changed,
            &next.package_digest,
            &operation("rotation"),
            true,
            Some(&first.signer_key),
        )
        .unwrap();
    manager.uninstall(&next.manifest.id).unwrap();
    assert_eq!(
        manager.inspect(&original).unwrap().previous_signer_key,
        Some(next.signer_key)
    );
    let metadata_only = root.0.join("metadata.yssplugin");
    package_variant(
        &metadata_only,
        false,
        "1.1.0+changed",
        8,
        ResourceBudget {
            snapshot_bytes: 1024,
            ..ResourceBudget::default()
        },
    );
    manager
        .install(
            &changed,
            &next.package_digest,
            &operation("reinstall"),
            true,
            None,
        )
        .unwrap();
    let content = manager.inspect(&metadata_only).unwrap();
    assert_eq!(
        manager
            .install(
                &metadata_only,
                &content.package_digest,
                &operation("version-conflict"),
                true,
                None
            )
            .unwrap_err()
            .code,
        "plugin_version_content_conflict"
    );
}

#[test]
fn private_storage_limits_and_cleanup_preserve_settings_and_project_results() {
    let root = Root::new();
    let archive = root.0.join("package.yssplugin");
    package_variant(
        &archive,
        false,
        "1.0.0",
        7,
        ResourceBudget {
            private_storage_bytes: 32,
            snapshot_bytes: 16,
            ..ResourceBudget::default()
        },
    );
    let manager = PluginManager::new(&root.0, Arc::new(Host)).unwrap();
    let inspected = manager.inspect(&archive).unwrap();
    let installed = manager
        .install(
            &archive,
            &inspected.package_digest,
            &operation("install"),
            true,
            None,
        )
        .unwrap();
    assert_eq!(installed.granted_budget.snapshot_bytes, 16);
    let data = root.0.join("extensions/data/example.statistics");
    fs::create_dir_all(data.join("cache")).unwrap();
    fs::write(data.join("cache/temporary.bin"), [0; 64]).unwrap();
    fs::write(data.join("settings.json"), "{}").unwrap();
    fs::create_dir_all(root.0.join("project/extension-results")).unwrap();
    fs::write(root.0.join("project/extension-results/result.json"), "{}").unwrap();
    assert_eq!(
        manager.acquire("example.statistics").err().unwrap().code,
        "plugin_private_storage_exhausted"
    );
    let usage = manager.clear_private_cache("example.statistics").unwrap();
    assert_eq!(usage.used_bytes, 2);
    assert_eq!(usage.cache_bytes, 0);
    assert!(data.join("settings.json").exists());
    assert!(
        root.0
            .join("project/extension-results/result.json")
            .exists()
    );
    let orphan = root.0.join("extensions/packages").join("f".repeat(64));
    fs::create_dir_all(&orphan).unwrap();
    assert_eq!(manager.collect_garbage().unwrap(), 1);
    assert!(!orphan.exists());
}
