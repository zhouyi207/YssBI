use crate::{PluginManager, Registration, fail, ledger, package};
use std::{fs, path::Path};
use yss_plugin_protocol::{
    InstalledPlugin, PackageInspection, PluginFailure, validate_operation_id,
};

fn same_release(left: &str, right: &str) -> Result<bool, PluginFailure> {
    let left = semver::Version::parse(left).map_err(|_| fail("plugin_manifest_invalid"))?;
    let right = semver::Version::parse(right).map_err(|_| fail("plugin_manifest_invalid"))?;
    Ok(left.major == right.major
        && left.minor == right.minor
        && left.patch == right.patch
        && left.pre == right.pre)
}
impl PluginManager {
    pub fn inspect(&self, path: &Path) -> Result<PackageInspection, PluginFailure> {
        let mut description = package::inspect(path)?.description;
        description.previous_signer_key = self
            .inner
            .registry
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))?
            .signers
            .get(&description.manifest.id)
            .cloned();
        Ok(description)
    }
    pub fn install(
        &self,
        path: &Path,
        expected_digest: &str,
        operation_id: &str,
        approve_native: bool,
        approved_previous_signer: Option<&str>,
    ) -> Result<InstalledPlugin, PluginFailure> {
        if !approve_native {
            return Err(fail("plugin_trust_required"));
        }
        self.available()?;
        self.ledger()?.prune()?;
        if let Some(receipt) = self.ledger()?.installation(operation_id)? {
            if receipt.package_digest != expected_digest {
                return Err(fail("plugin_operation_conflict"));
            }
            return Ok(receipt);
        }
        validate_operation_id(operation_id, ledger::now_ms())?;
        let inspected = package::inspect(path)?;
        if inspected.description.package_digest != expected_digest {
            return Err(fail("plugin_package_changed"));
        }
        let id = inspected.description.manifest.id.clone();
        let reservation = self.reserve(&id)?;
        let previous_signer = self
            .inner
            .registry
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))?
            .signers
            .get(&id)
            .cloned();
        if previous_signer
            .as_deref()
            .is_some_and(|signer| signer != inspected.description.signer_key)
            && approved_previous_signer != previous_signer.as_deref()
        {
            return Err(fail("plugin_signer_change_requires_approval"));
        }
        if let Ok(previous) = self.registration(&id)
            && same_release(
                &previous.manifest.version,
                &inspected.description.manifest.version,
            )?
            && previous.digest != expected_digest
        {
            return Err(fail("plugin_version_content_conflict"));
        }
        let granted_budget = inspected.description.manifest.resource_budget.grant()?;
        let staging = self
            .inner
            .root
            .join("staging")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir(&staging).map_err(|_| fail("plugin_storage_failed"))?;
        let target = package::package_path(&self.inner.root, expected_digest)?;
        let result = (|| {
            package::extract(path, &staging, &inspected)?;
            if !target.exists() {
                fs::rename(&staging, &target).map_err(|_| fail("plugin_storage_failed"))?;
            }
            self.stop_process(&id);
            self.update_registry(|registry| {
                if let Some(previous) = self.ledger()?.installation(operation_id)? {
                    return if previous.package_digest == expected_digest {
                        Ok(())
                    } else {
                        Err(fail("plugin_operation_conflict"))
                    };
                }
                registry.revision += 1;
                registry
                    .signers
                    .insert(id.clone(), inspected.description.signer_key.clone());
                registry.entries.insert(
                    id.clone(),
                    Registration {
                        manifest: inspected.description.manifest.clone(),
                        digest: expected_digest.into(),
                        generation: registry.revision,
                        signer: inspected.description.signer_key.clone(),
                        enabled: true,
                        files: inspected.files.clone(),
                        granted_budget: granted_budget.clone(),
                    },
                );
                registry.installations.insert(
                    operation_id.into(),
                    InstalledPlugin {
                        manifest: inspected.description.manifest.clone(),
                        package_digest: expected_digest.into(),
                        installation_generation: registry.revision.to_string(),
                        enabled: true,
                        process_state: "stopped".into(),
                        granted_budget: granted_budget.clone(),
                        signer_key: inspected.description.signer_key.clone(),
                    },
                );
                Ok(())
            })?;
            self.revoke_contexts(&id);
            let receipt = self
                .list()?
                .into_iter()
                .find(|entry| entry.manifest.id == id)
                .ok_or_else(|| fail("plugin_registry_invalid"))?;
            Ok(receipt)
        })();
        if staging.exists() {
            let _ = fs::remove_dir_all(&staging);
        }
        drop(reservation);
        let _ = self.collect_garbage();
        result
    }
    pub fn set_enabled(&self, id: &str, enabled: bool) -> Result<(), PluginFailure> {
        let reservation = self.reserve(id)?;
        self.stop_process(id);
        self.update_registry(|registry| {
            registry.revision += 1;
            let entry = registry
                .entries
                .get_mut(id)
                .ok_or_else(|| fail("plugin_not_installed"))?;
            entry.enabled = enabled;
            entry.generation = registry.revision;
            Ok(())
        })?;
        self.revoke_contexts(id);
        drop(reservation);
        Ok(())
    }
    pub fn uninstall(&self, id: &str) -> Result<(), PluginFailure> {
        let reservation = self.reserve(id)?;
        self.stop_process(id);
        self.update_registry(|registry| {
            registry
                .entries
                .remove(id)
                .ok_or_else(|| fail("plugin_not_installed"))?;
            registry.revision += 1;
            Ok(())
        })?;
        self.revoke_contexts(id);
        // Content-addressed packages and private data remain recoverable. They
        // are never removed together with project resources or system runtimes.
        drop(reservation);
        Ok(())
    }
}
