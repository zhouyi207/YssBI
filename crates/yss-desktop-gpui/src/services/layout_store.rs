//! Application-local DockArea and graph view snapshots; never project documents.
mod viewports;
pub(crate) use viewports::Viewport;

use anyhow::Result;
use gpui_kit::component::dock::DockAreaState;
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
    viewports: Mutex<BTreeMap<String, BTreeMap<String, Viewport>>>,
}

impl LayoutStore {
    pub fn new(app_data: &Path) -> Self {
        Self {
            directory: app_data.join("workbench-layouts"),
            coordinator: FilesystemCoordinator::default(),
            writes: Mutex::new(BTreeMap::new()),
            viewports: Mutex::new(BTreeMap::new()),
        }
    }

    fn file_name(project_root: &str) -> Result<String> {
        let hash = yss_canonical_hash::hash_canonical("yssbi.native.layout", &project_root)?;
        let key = hex::encode(hash);
        Ok(format!("{key}.json"))
    }

    pub fn read(&self, project_root: &str) -> Result<Option<DockAreaState>> {
        self.read_snapshot(&Self::file_name(project_root)?)
    }

    /// The last successfully opened native project, independent of registry favorites,
    /// discovery timestamps, and user preferences. Explicit Close affects this session only.
    pub(crate) fn last_project(&self) -> Result<Option<PathBuf>> {
        Ok(self
            .read_snapshot::<String>("last-project.json")?
            .map(PathBuf::from))
    }

    pub(crate) fn remember_project(
        self: &Arc<Self>,
        path: String,
        executor: &tokio::runtime::Handle,
    ) -> tokio::task::JoinHandle<Result<()>> {
        self.queue_write(
            Ok("last-project.json".into()),
            move || Ok(serde_json::to_vec(&path)?),
            false,
            executor,
        )
    }

    fn read_snapshot<T: serde::de::DeserializeOwned>(&self, file_name: &str) -> Result<Option<T>> {
        if !self.directory.exists() {
            return Ok(None);
        }
        let binding = RootBinding::for_existing(&self.directory)?;
        let _lease = self.coordinator.acquire(binding.normalized().clone())?;
        let path = self.directory.join(file_name);
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
        let layout = self.queue_write(
            Self::file_name(&project_root),
            move || Ok(serde_json::to_vec_pretty(&state)?),
            false,
            executor,
        );
        // Closing a window must start its final write before the Tokio runtime
        // shuts down; a pending debounce alone would be cancelled on exit.
        let viewports = self.save_viewports(project_root, false, executor);
        executor.spawn(async move {
            layout.await??;
            if let Some(viewports) = viewports {
                viewports.await??;
            }
            Ok(())
        })
    }

    fn queue_write(
        self: &Arc<Self>,
        file_name: Result<String>,
        contents: impl FnOnce() -> Result<Vec<u8>> + Send + 'static,
        debounce: bool,
        executor: &tokio::runtime::Handle,
    ) -> tokio::task::JoinHandle<Result<()>> {
        let file_name = match file_name {
            Ok(file_name) => file_name,
            Err(error) => return executor.spawn(async move { Err(error) }),
        };
        let stamp = self
            .writes
            .lock()
            .expect("layout queue lock")
            .entry(file_name.clone())
            .or_insert_with(|| Arc::new(AtomicU64::new(0)))
            .clone();
        let sequence = stamp.fetch_add(1, Ordering::SeqCst) + 1;
        let store = self.clone();
        let write = move || {
            let current = || stamp.load(Ordering::SeqCst) == sequence;
            if !current() {
                return Ok(());
            }
            let result = contents().and_then(|contents| store.write(&file_name, contents, current));
            if result.is_err() {
                tracing::warn!(
                    code = "native_layout_save_failed",
                    "Native layout save failed"
                );
            }
            result
        };
        if debounce {
            let blocking = executor.clone();
            executor.spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                blocking.spawn_blocking(write).await?
            })
        } else {
            executor.spawn_blocking(write)
        }
    }

    fn write(
        &self,
        file_name: &str,
        contents: Vec<u8>,
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
                relative_path: file_name.into(),
                contents,
            }],
        )?;
        transaction.commit()?.finalize();
        Ok(())
    }
}
