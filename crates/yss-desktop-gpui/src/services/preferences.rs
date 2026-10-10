//! Application language preferences; runtime locale stays with the component library.
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use yss_filesystem::{
    FilesystemCoordinator, FilesystemTransaction, RootBinding, StagedFilesystemMutation,
    TransactionContext, TransactionId,
};

pub struct PreferencesStore {
    directory: PathBuf,
    coordinator: FilesystemCoordinator,
}

impl PreferencesStore {
    pub fn new(app_data: &Path) -> Self {
        Self {
            directory: app_data.to_owned(),
            coordinator: FilesystemCoordinator::default(),
        }
    }

    pub fn read_language(&self) -> Result<&'static str> {
        let bytes = match std::fs::read(self.directory.join("preferences.json")) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(crate::text::DEFAULT_LANGUAGE);
            }
            Err(error) => return Err(error.into()),
        };
        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
        crate::text::LANGUAGES
            .into_iter()
            .find(|language| {
                value.get("language").and_then(serde_json::Value::as_str) == Some(*language)
            })
            .context("invalid language preference")
    }

    pub fn save_language(&self, language: &str) -> Result<()> {
        anyhow::ensure!(
            crate::text::LANGUAGES.contains(&language),
            "unsupported language"
        );
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
                contents: serde_json::to_vec_pretty(&serde_json::json!({ "language": language }))?,
            }],
        )?;
        transaction.commit()?.finalize();
        Ok(())
    }
}
