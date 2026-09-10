use crate::{read_bounded, resolve_data_file};
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use yss_plugin_protocol::{
    ExecutionMode, FileEntry, MAX_PACKAGE_BYTES, PackageInspection, PackageSignature,
    PluginFailure, PluginManifest, valid_relative_path,
};

pub fn current_target() -> &'static str {
    #[cfg(all(windows, target_arch = "x86_64"))]
    {
        "x86_64-pc-windows-msvc"
    }
    #[cfg(not(all(windows, target_arch = "x86_64")))]
    {
        "unsupported"
    }
}
pub(crate) fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn invalid() -> PluginFailure {
    PluginFailure::new("plugin_package_invalid")
}
pub(crate) fn from_hex(value: &str) -> Result<Vec<u8>, PluginFailure> {
    if !value.len().is_multiple_of(2) || !value.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    (0..value.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&value[index..index + 2], 16).map_err(|_| invalid()))
        .collect()
}
pub(crate) struct InspectedPackage {
    pub description: PackageInspection,
    pub files: Vec<FileEntry>,
}

pub(crate) fn inspect(path: &Path) -> Result<InspectedPackage, PluginFailure> {
    let file = fs::File::open(path).map_err(|_| invalid())?;
    if file.metadata().map_err(|_| invalid())?.len() > MAX_PACKAGE_BYTES {
        return Err(invalid());
    }
    let mut archive = zip::ZipArchive::new(file).map_err(|_| invalid())?;
    if archive.len() > 4096 {
        return Err(invalid());
    }
    let mut read = |name: &str, limit: u64| -> Result<Vec<u8>, PluginFailure> {
        let entry = archive.by_name(name).map_err(|_| invalid())?;
        if entry.size() > limit {
            return Err(invalid());
        }
        let mut bytes = Vec::new();
        entry
            .take(limit + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| invalid())?;
        if bytes.len() as u64 > limit {
            return Err(invalid());
        }
        Ok(bytes)
    };
    let manifest: PluginManifest =
        serde_json::from_slice(&read("plugin.json", 128 * 1024)?).map_err(|_| invalid())?;
    manifest.validate()?;
    if manifest.target != current_target() {
        return Err(PluginFailure::new("plugin_platform_incompatible"));
    }
    if manifest.execution != ExecutionMode::TrustedNative {
        return Err(PluginFailure::new("plugin_sandbox_unavailable"));
    }
    let files: Vec<FileEntry> =
        serde_json::from_slice(&read("files.json", 1024 * 1024)?).map_err(|_| invalid())?;
    let signature: PackageSignature =
        serde_json::from_slice(&read("signature.json", 4096)?).map_err(|_| invalid())?;
    let mut names = BTreeSet::new();
    let mut total = 0u64;
    for entry in &files {
        let size = entry.size.parse::<u64>().map_err(|_| invalid())?;
        total = total.checked_add(size).ok_or_else(invalid)?;
        if total > MAX_PACKAGE_BYTES
            || !valid_relative_path(&entry.path)
            || matches!(
                entry.path.as_str(),
                "plugin.json" | "files.json" | "signature.json"
            )
            || !names.insert(entry.path.to_ascii_lowercase())
            || entry.sha256.len() != 64
            || !entry.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(invalid());
        }
    }
    if files
        .windows(2)
        .any(|pair| pair[0].path.as_bytes() >= pair[1].path.as_bytes())
        || !files.iter().any(|file| file.path == manifest.executable)
        || manifest
            .contributes
            .views
            .iter()
            .any(|view| !files.iter().any(|file| file.path == view.entry))
    {
        return Err(invalid());
    }
    let public: [u8; 32] = from_hex(&signature.public_key)?
        .try_into()
        .map_err(|_| invalid())?;
    if signature.key_id != hash(&public) {
        return Err(PluginFailure::new("plugin_signature_invalid"));
    }
    let signed = serde_jcs::to_vec(&serde_json::json!({"schemaVersion":1,"keyId":signature.key_id,"manifest":manifest,"files":files})).map_err(|_| invalid())?;
    let key = VerifyingKey::from_bytes(&public)
        .map_err(|_| PluginFailure::new("plugin_signature_invalid"))?;
    let signature_bytes: [u8; 64] = from_hex(&signature.signature)?
        .try_into()
        .map_err(|_| invalid())?;
    key.verify_strict(&signed, &Signature::from_bytes(&signature_bytes))
        .map_err(|_| PluginFailure::new("plugin_signature_invalid"))?;
    Ok(InspectedPackage {
        description: PackageInspection {
            manifest,
            package_digest: hash(&signed),
            signer_key: hash(&public),
            previous_signer_key: None,
        },
        files,
    })
}

pub(crate) fn extract(
    path: &Path,
    staging: &Path,
    inspected: &InspectedPackage,
) -> Result<(), PluginFailure> {
    let file = fs::File::open(path).map_err(|_| invalid())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|_| invalid())?;
    let mut names = BTreeSet::new();
    let mut total = 0u64;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|_| invalid())?;
        let name = entry.name().to_owned();
        let size = entry.size();
        if entry.is_dir() {
            let directory = name.trim_end_matches('/');
            if !valid_relative_path(directory)
                || size != 0
                || !inspected
                    .files
                    .iter()
                    .any(|file| file.path.starts_with(&format!("{directory}/")))
            {
                return Err(invalid());
            }
            continue;
        }
        total = total.checked_add(size).ok_or_else(invalid)?;
        if total > MAX_PACKAGE_BYTES
            || !valid_relative_path(&name)
            || !names.insert(name.to_ascii_lowercase())
            || entry.is_dir()
            || entry
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
            || !matches!(
                name.as_str(),
                "plugin.json" | "files.json" | "signature.json"
            ) && !inspected.files.iter().any(|file| file.path == name)
        {
            return Err(invalid());
        }
        let destination = staging.join(&name);
        fs::create_dir_all(destination.parent().ok_or_else(invalid)?).map_err(|_| invalid())?;
        let mut output = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(destination)
            .map_err(|_| invalid())?;
        let copied = std::io::copy(&mut entry.by_ref().take(size + 1), &mut output)
            .map_err(|_| invalid())?;
        if copied != size {
            return Err(invalid());
        }
        output.flush().map_err(|_| invalid())?;
    }
    for file in &inspected.files {
        verify_file(staging, file)?;
    }
    let staged_manifest: PluginManifest =
        serde_json::from_slice(&read_bounded(&staging.join("plugin.json"), 128 * 1024)?)
            .map_err(|_| invalid())?;
    if serde_jcs::to_vec(&staged_manifest).map_err(|_| invalid())?
        != serde_jcs::to_vec(&inspected.description.manifest).map_err(|_| invalid())?
    {
        return Err(invalid());
    }
    Ok(())
}
pub(crate) fn verify_file(root: &Path, entry: &FileEntry) -> Result<(), PluginFailure> {
    let path = resolve_data_file(root, &entry.path)?;
    let mut file = fs::File::open(path).map_err(|_| invalid())?;
    if file.metadata().map_err(|_| invalid())?.len().to_string() != entry.size {
        return Err(invalid());
    }
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let count = file.read(&mut buffer).map_err(|_| invalid())?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    if format!("{:x}", digest.finalize()) != entry.sha256 {
        return Err(PluginFailure::new("plugin_checksum_failed"));
    }
    Ok(())
}
pub(crate) fn atomic_json(path: &Path, value: &impl serde::Serialize) -> Result<(), PluginFailure> {
    let temporary = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
    let operation = (|| {
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| invalid())?;
        serde_json::to_writer(&mut file, value).map_err(|_| invalid())?;
        file.sync_all().map_err(|_| invalid())?;
        drop(file);
        yss_file_replace::atomic_replace(&temporary, path)
            .map_err(|_| PluginFailure::new("plugin_storage_failed"))
    })();
    if operation.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    operation
}
pub(crate) fn package_path(root: &Path, digest: &str) -> Result<PathBuf, PluginFailure> {
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid());
    }
    Ok(root.join("packages").join(digest))
}
