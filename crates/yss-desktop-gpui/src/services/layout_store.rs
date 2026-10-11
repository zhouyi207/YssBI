//! Application-local DockArea and graph view snapshots; never project documents.
mod viewports;
pub(crate) use viewports::Viewport;

use anyhow::Result;
use gpui_kit::component::dock::{DockAreaState, DockState, PanelInfo, PanelState};
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

    pub(crate) fn for_application() -> Result<Self> {
        Ok(Self::new(
            &super::paths::NativePaths::resolve()?
                .application
                .app_data_dir,
        ))
    }

    pub(crate) fn window_bounds(&self) -> Result<Option<gpui_kit::Size<gpui_kit::Pixels>>> {
        self.read_snapshot("main-window.json")
    }

    pub(crate) fn save_window_bounds(
        self: &Arc<Self>,
        size: gpui_kit::Size<gpui_kit::Pixels>,
        debounce: bool,
        executor: &tokio::runtime::Handle,
    ) -> tokio::task::JoinHandle<Result<()>> {
        self.queue_write(
            Ok("main-window.json".into()),
            move || Ok(serde_json::to_vec(&size)?),
            debounce,
            executor,
        )
    }

    fn file_name(project_root: &str) -> Result<String> {
        let hash = yss_canonical_hash::hash_canonical("yssbi.native.layout", &project_root)?;
        let key = hex::encode(hash);
        Ok(format!("{key}.json"))
    }

    pub fn read(&self, project_root: &str) -> Result<Option<DockAreaState>> {
        Ok(self
            .read_snapshot::<DockAreaState>(&Self::file_name(project_root)?)?
            .map(persistent_layout))
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
            move || Ok(serde_json::to_vec_pretty(&persistent_layout(state))?),
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

// Execution results belong to this process. Filter only the persisted snapshot;
// the live DockArea must keep its panels and leases when users rearrange them.
fn persistent_layout(mut state: DockAreaState) -> DockAreaState {
    if !retain_persistent_panels(&mut state.center) {
        state.center = PanelState {
            info: PanelInfo::tabs(0),
            ..PanelState::new("TabPanel")
        };
    }
    for dock in [
        &mut state.left_dock,
        &mut state.right_dock,
        &mut state.bottom_dock,
    ] {
        let Some(current) = dock.as_ref() else {
            continue;
        };
        if !contains_result_panel(current.panel()) {
            continue;
        }
        // DockState exposes its panel by shared reference only.
        let mut panel = current.panel().clone();
        *dock = retain_persistent_panels(&mut panel)
            .then(|| DockState::new(panel, current.placement(), current.size(), current.open()));
    }
    state
}

fn contains_result_panel(state: &PanelState) -> bool {
    state.panel_name == "result" || state.children.iter().any(contains_result_panel)
}

fn retain_persistent_panels(state: &mut PanelState) -> bool {
    if state.panel_name == "result" {
        return false;
    }
    if state.children.is_empty() {
        return true;
    }
    match &mut state.info {
        PanelInfo::Tabs { active_index } => {
            let active = *active_index;
            let mut index = 0;
            state.children.retain_mut(|child| {
                let keep = retain_persistent_panels(child);
                if !keep && index < active {
                    *active_index = active_index.saturating_sub(1);
                }
                index += 1;
                keep
            });
            *active_index = (*active_index).min(state.children.len().saturating_sub(1));
        }
        PanelInfo::Stack { sizes, .. } => {
            let mut index = 0;
            let mut retained = 0;
            state.children.retain_mut(|child| {
                let keep = retain_persistent_panels(child);
                if keep {
                    if let Some(size) = sizes.get(index).copied() {
                        sizes[retained] = size;
                    }
                    retained += 1;
                }
                index += 1;
                keep
            });
            sizes.truncate(retained);
        }
        PanelInfo::Panel(_) => {
            state.children.retain_mut(retain_persistent_panels);
            return true;
        }
    }
    !state.children.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::component::dock::DockPlacement;
    use gpui_kit::{Axis, px};

    fn tabs(names: &[&str], active_index: usize) -> PanelState {
        PanelState {
            children: names.iter().map(|name| PanelState::new(*name)).collect(),
            info: PanelInfo::tabs(active_index),
            ..PanelState::new("TabPanel")
        }
    }

    #[test]
    fn closing_window_flushes_latest_size_before_pending_resize() -> Result<()> {
        let directory = std::env::temp_dir().join(format!("yssbi-window-{}", uuid::Uuid::new_v4()));
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let store = Arc::new(LayoutStore::new(&directory));
        let final_size = gpui_kit::size(px(1120.), px(760.));
        runtime.block_on(async {
            let pending =
                store.save_window_bounds(gpui_kit::size(px(1280.), px(800.)), true, runtime.handle());
            store
                .save_window_bounds(final_size, false, runtime.handle())
                .await??;
            assert_eq!(
                LayoutStore::new(&directory).window_bounds()?,
                Some(final_size)
            );
            pending.await??;
            assert_eq!(
                LayoutStore::new(&directory).window_bounds()?,
                Some(final_size)
            );
            anyhow::Ok(())
        })?;
        std::fs::remove_dir_all(directory)?;
        Ok(())
    }

    #[test]
    fn result_tabs_are_not_persisted_and_live_snapshot_is_unchanged() {
        let live = DockAreaState {
            center: tabs(&["result", "document-editor", "result", "graph-editor"], 3),
            ..Default::default()
        };
        let persisted = persistent_layout(live.clone());
        assert_eq!(live.center.children.len(), 4);
        assert_eq!(live.center.info.active_index(), Some(3));
        assert_eq!(persisted.center.children.len(), 2);
        assert_eq!(persisted.center.children[0].panel_name, "document-editor");
        assert_eq!(persisted.center.children[1].panel_name, "graph-editor");
        assert_eq!(persisted.center.info.active_index(), Some(1));
    }

    #[test]
    fn result_only_groups_are_removed_without_changing_surviving_sizes() {
        let missing = PanelState {
            info: PanelInfo::panel(serde_json::json!({"documentPath": "missing.md"})),
            ..PanelState::new("document-editor")
        };
        let mut document = tabs(&[], 0);
        document.children.push(missing.clone());
        let state = DockAreaState {
            center: PanelState {
                children: vec![tabs(&["result"], 0), document, tabs(&["graph-editor"], 0)],
                info: PanelInfo::stack(vec![px(120.), px(240.), px(360.)], Axis::Horizontal),
                ..PanelState::new("StackPanel")
            },
            left_dock: Some(DockState::new(
                tabs(&["result"], 0),
                DockPlacement::Left,
                px(200.),
                true,
            )),
            right_dock: Some(DockState::new(
                tabs(&["result", "details"], 1),
                DockPlacement::Right,
                px(280.),
                false,
            )),
            bottom_dock: Some(DockState::new(
                tabs(&["results", "logs"], 1),
                DockPlacement::Bottom,
                px(180.),
                true,
            )),
            ..Default::default()
        };
        let persisted = persistent_layout(state);
        assert_eq!(persisted.center.children.len(), 2);
        assert_eq!(
            persisted.center.info.sizes(),
            Some(&vec![px(240.), px(360.)])
        );
        assert_eq!(persisted.center.children[0].children, vec![missing]);
        assert!(persisted.left_dock.is_none());
        let right = persisted.right_dock.unwrap();
        assert_eq!(right.panel(), &tabs(&["details"], 0));
        assert_eq!(right.size(), px(280.));
        assert!(!right.open());
        assert_eq!(
            persisted.bottom_dock.unwrap().panel(),
            &tabs(&["results", "logs"], 1)
        );
    }

    #[test]
    fn result_only_center_restores_as_empty_editor_space() {
        let persisted = persistent_layout(DockAreaState {
            center: tabs(&["result"], 0),
            ..Default::default()
        });
        assert_eq!(persisted.center, tabs(&[], 0));
    }
}
