//! Application-local snapshots of the native DockArea; no project documents are persisted here.
use anyhow::Result;
use gpui_component::dock::DockAreaState;
use std::path::{Path, PathBuf};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use yss_filesystem::{
    FilesystemCoordinator, FilesystemTransaction, RootBinding, StagedFilesystemMutation,
    TransactionContext, TransactionId,
};

pub struct LayoutStore {
    directory: PathBuf,
    coordinator: FilesystemCoordinator,
    writes: Mutex<BTreeMap<String, Arc<AtomicU64>>>,
}

impl LayoutStore {
    pub fn new(app_data: &Path) -> Self {
        Self {
            directory: app_data.join("workbench-layouts"),
            coordinator: FilesystemCoordinator::default(),
            writes: Mutex::new(BTreeMap::new()),
        }
    }

    fn file_name(project_root: &str) -> Result<String> {
        let hash = yss_canonical_hash::hash_canonical("yssbi.native.layout", &project_root)?;
        let key = hash
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        Ok(format!("{key}.json"))
    }

    pub fn read(&self, project_root: &str) -> Result<Option<DockAreaState>> {
        if !self.directory.exists() { return Ok(None); }
        let binding = RootBinding::for_existing(&self.directory)?;
        let _lease = self.coordinator.acquire(binding.normalized().clone())?;
        let path = self.directory.join(Self::file_name(project_root)?);
        match std::fs::read(path) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub fn save(
        self: &Arc<Self>,
        project_root: String,
        state: DockAreaState,
        executor: &tokio::runtime::Handle,
    ) -> tokio::task::JoinHandle<Result<()>> {
        let stamp = self
            .writes
            .lock()
            .expect("layout queue lock")
            .entry(project_root.clone())
            .or_insert_with(|| Arc::new(AtomicU64::new(0)))
            .clone();
        let sequence = stamp.fetch_add(1, Ordering::SeqCst) + 1;
        let store = self.clone();
        executor.spawn_blocking(move || {
            let result = store.write(&project_root, &state, || {
                stamp.load(Ordering::SeqCst) == sequence
            });
            if result.is_err() {
                tracing::warn!(
                    code = "native_layout_save_failed",
                    "Native layout save failed"
                );
            }
            result
        })
    }

    fn write(
        &self,
        project_root: &str,
        state: &DockAreaState,
        current: impl FnOnce() -> bool,
    ) -> Result<()> {
        std::fs::create_dir_all(&self.directory)?;
        let binding = RootBinding::for_existing(&self.directory)?;
        let root = binding.normalized().clone();
        let lease = self.coordinator.acquire(root.clone())?;
        // Tasks can acquire their file lease out of issue order. Never let an older snapshot
        // overwrite a newer requested one, including after the window has closed.
        if !current() {
            return Ok(());
        }
        let transaction = FilesystemTransaction::prepare(
            TransactionContext {
                root,
                transaction_id: TransactionId::new(),
                recovery_marker: None,
            },
            lease,
            vec![StagedFilesystemMutation::Write {
                relative_path: Self::file_name(project_root)?.into(),
                contents: serde_json::to_vec_pretty(state)?,
            }],
        )?;
        transaction.commit()?.finalize();
        Ok(())
    }
}
