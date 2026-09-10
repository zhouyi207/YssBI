use crate::{PluginFailure, PluginManager, PluginStorageUsage, fail};
use std::collections::BTreeSet;
use std::path::Path;

fn redirected(metadata: &std::fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return true;
        }
    }
    false
}

pub(super) fn validate_path(root: &Path, path: &Path) -> Result<(), PluginFailure> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| fail("plugin_path_invalid"))?;
    let mut current = root.to_owned();
    for component in std::iter::once(None).chain(relative.components().map(Some)) {
        if let Some(component) = component {
            if !matches!(component, std::path::Component::Normal(_)) {
                return Err(fail("plugin_path_invalid"));
            }
            current.push(component);
        }
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if redirected(&metadata) => return Err(fail("plugin_path_invalid")),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(fail("plugin_storage_failed")),
        }
    }
    Ok(())
}

pub(super) fn directory_bytes(path: &Path) -> Result<u64, PluginFailure> {
    fn visit(path: &Path, depth: usize) -> Result<u64, PluginFailure> {
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(_) => return Err(fail("plugin_storage_failed")),
        };
        if redirected(&metadata) {
            return Ok(0);
        }
        if !metadata.is_dir() {
            return Ok(metadata.len());
        }
        if depth >= 64 {
            return Err(fail("plugin_path_invalid"));
        }
        let entries = match std::fs::read_dir(path) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
            Err(_) => return Err(fail("plugin_storage_failed")),
        };
        let mut bytes = 0u64;
        for entry in entries {
            let entry = entry.map_err(|_| fail("plugin_storage_failed"))?;
            bytes = bytes
                .checked_add(visit(&entry.path(), depth + 1)?)
                .ok_or_else(|| fail("plugin_resource_exhausted"))?;
        }
        Ok(bytes)
    }
    visit(path, 0)
}

impl PluginManager {
    pub fn storage_usage(&self, plugin: &str) -> Result<PluginStorageUsage, PluginFailure> {
        let entry = self.registration(plugin)?;
        let root = self.inner.root.join("data").join(plugin);
        validate_path(&self.inner.root, &root)?;
        let used_bytes = directory_bytes(&root)?;
        let mut cache_bytes = 0u64;
        for cache in &entry.manifest.cache_directories {
            let path = root.join(cache);
            validate_path(&root, &path)?;
            cache_bytes = cache_bytes.saturating_add(directory_bytes(&path)?);
        }
        Ok(PluginStorageUsage {
            plugin_id: plugin.into(),
            used_bytes,
            budget_bytes: entry.granted_budget.private_storage_bytes,
            cache_bytes,
            enforcement: "applicationSoft".into(),
        })
    }
    pub(super) fn enforce_private_budget(
        &self,
        plugin: &str,
        additional: u64,
    ) -> Result<(), PluginFailure> {
        let usage = self.storage_usage(plugin)?;
        if usage.used_bytes.saturating_add(additional) > usage.budget_bytes {
            return Err(fail("plugin_private_storage_exhausted"));
        }
        Ok(())
    }
    pub fn clear_private_cache(&self, plugin: &str) -> Result<PluginStorageUsage, PluginFailure> {
        let reservation = self.reserve(plugin)?;
        self.stop_process(plugin);
        self.revoke_contexts(plugin);
        let entry = self.registration(plugin)?;
        let root = self.inner.root.join("data").join(plugin);
        for cache in entry.manifest.cache_directories {
            let path = root.join(cache);
            validate_path(&self.inner.root, &path)?;
            if path.exists() {
                if !path.is_dir() {
                    return Err(fail("plugin_path_invalid"));
                }
                std::fs::remove_dir_all(&path).map_err(|_| fail("plugin_storage_failed"))?;
            }
        }
        drop(reservation);
        self.storage_usage(plugin)
    }
    pub fn collect_garbage(&self) -> Result<usize, PluginFailure> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| fail("plugin_state_unavailable"))?;
        if state.maintenance
            || !state.mutating.is_empty()
            || !state.starting.is_empty()
            || state.processes.values().any(|process| process.busy())
        {
            return Err(fail("plugin_busy"));
        }
        state.maintenance = true;
        drop(state);
        struct Admission<'a>(&'a PluginManager);
        impl Drop for Admission<'_> {
            fn drop(&mut self) {
                if let Ok(mut state) = self.0.inner.state.lock() {
                    state.maintenance = false;
                }
            }
        }
        let _admission = Admission(self);
        self.ledger()?.prune()?;
        let mut retained = self
            .ledger()?
            .retained_packages()?
            .into_iter()
            .collect::<BTreeSet<_>>();
        retained.extend(
            self.inner
                .registry
                .lock()
                .map_err(|_| fail("plugin_state_unavailable"))?
                .entries
                .values()
                .map(|entry| entry.digest.clone()),
        );
        let root = self.inner.root.join("packages");
        validate_path(&self.inner.root, &root)?;
        let mut removed = 0;
        for entry in std::fs::read_dir(&root).map_err(|_| fail("plugin_storage_failed"))? {
            let entry = entry.map_err(|_| fail("plugin_storage_failed"))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.len() != 64
                || !name.bytes().all(|byte| byte.is_ascii_hexdigit())
                || retained.contains(&name)
            {
                continue;
            }
            let path = entry.path();
            validate_path(&self.inner.root, &path)?;
            if path.is_dir() {
                std::fs::remove_dir_all(path).map_err(|_| fail("plugin_storage_failed"))?;
                removed += 1;
            }
        }
        Ok(removed)
    }
}
