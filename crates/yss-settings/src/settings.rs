mod model;
pub use model::*;

use anyhow::Result;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};
use yss_filesystem::{
    FilesystemCoordinator, FilesystemTransaction, RootBinding, StagedFilesystemMutation,
    TransactionContext, TransactionId,
};

pub struct SettingsStore {
    directory: PathBuf,
    coordinator: FilesystemCoordinator,
    current: Mutex<Arc<UserSettings>>,
}

impl SettingsStore {
    pub fn new(app_data: &Path) -> Self {
        Self {
            directory: app_data.to_owned(),
            coordinator: FilesystemCoordinator::default(),
            current: Mutex::new(Arc::new(UserSettings::default())),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Arc<UserSettings>> {
        // Updates mutate a private candidate; an unwinding caller cannot corrupt the published snapshot.
        match self.current.lock() {
            Ok(current) => current,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    pub fn snapshot(&self) -> Arc<UserSettings> {
        Arc::clone(&self.lock())
    }

    pub fn load(&self) -> Result<Arc<UserSettings>> {
        let mut current = self.lock();
        let settings = match std::fs::read(self.directory.join("preferences.json")) {
            Ok(bytes) => serde_json::from_slice::<UserSettings>(&bytes)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => UserSettings::default(),
            Err(error) => return Err(error.into()),
        };
        settings.validate()?;
        *current = Arc::new(settings);
        Ok(Arc::clone(&current))
    }

    pub fn update(&self, update: impl FnOnce(&mut UserSettings)) -> Result<Arc<UserSettings>> {
        let mut current = self.lock();
        let mut candidate = current.as_ref().clone();
        update(&mut candidate);
        candidate.validate()?;
        if candidate == **current {
            return Ok(Arc::clone(&current));
        }
        std::fs::create_dir_all(&self.directory)?;
        let binding = RootBinding::for_existing(&self.directory)?;
        let root = binding.normalized().clone();
        let lease = self.coordinator.acquire(root.clone())?;
        let transaction = FilesystemTransaction::prepare(
            TransactionContext {
                root,
                transaction_id: TransactionId::new(),
                recovery_marker: None,
            },
            lease,
            vec![StagedFilesystemMutation::Write {
                relative_path: "preferences.json".into(),
                contents: serde_json::to_vec_pretty(&candidate)?,
            }],
        )?;
        transaction.commit()?.finalize();
        *current = Arc::new(candidate);
        Ok(Arc::clone(&current))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn independent_updates_preserve_other_preferences() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = Arc::new(SettingsStore::new(directory.path()));
        let worker = {
            let store = store.clone();
            std::thread::spawn(move || {
                store.update(|settings| settings.appearance.ui_font_size = 18.)
            })
        };
        store.update(|settings| settings.language = Language::English)?;
        worker
            .join()
            .map_err(|_| anyhow::anyhow!("settings worker panicked"))??;
        let restored = SettingsStore::new(directory.path()).load()?;
        assert_eq!(restored.language, Language::English);
        assert_eq!(restored.appearance.ui_font_size, 18.);
        Ok(())
    }

    #[test]
    fn invalid_or_unwritable_update_preserves_effective_settings() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = SettingsStore::new(directory.path());
        store.update(|settings| settings.tables.page_size = 200)?;
        let saved = std::fs::read(directory.path().join("preferences.json"))?;
        assert!(
            store
                .update(|settings| settings.tables.page_size = 0)
                .is_err()
        );
        assert_eq!(store.snapshot().tables.page_size, 200);
        assert_eq!(
            std::fs::read(directory.path().join("preferences.json"))?,
            saved
        );
        let blocked = directory.path().join("not-a-directory");
        std::fs::write(&blocked, "blocked")?;
        let store = SettingsStore::new(&blocked);
        assert!(
            store
                .update(|settings| settings.language = Language::English)
                .is_err()
        );
        assert_eq!(store.snapshot().language, Language::Chinese);
        Ok(())
    }

    #[test]
    fn malformed_reload_does_not_replace_last_good_snapshot() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = SettingsStore::new(directory.path());
        store.update(|settings| settings.editor.soft_wrap = false)?;
        std::fs::write(directory.path().join("preferences.json"), "{invalid")?;
        assert!(store.load().is_err());
        assert!(!store.snapshot().editor.soft_wrap);
        Ok(())
    }
}
