//! Retained view checkpoints seed reopened canvases; live coordinates stay in each canvas.
use super::LayoutStore;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
use tokio::{runtime::Handle, task::JoinHandle};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Viewport {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

impl Viewport {
    pub const MIN_SCALE: f32 = 0.1;
    pub const MAX_SCALE: f32 = 5.;

    fn valid(self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && (Self::MIN_SCALE..=Self::MAX_SCALE).contains(&self.scale)
    }
}

impl LayoutStore {
    pub(crate) fn load_viewports(&self, project_root: &str) -> Result<()> {
        if self
            .viewports
            .lock()
            .expect("view checkpoint lock")
            .contains_key(project_root)
        {
            return Ok(());
        }
        let snapshots: BTreeMap<String, Viewport> = self
            .read_snapshot(&Self::viewport_file(project_root)?)?
            .unwrap_or_default();
        anyhow::ensure!(
            snapshots.values().all(|view| view.valid()),
            "invalid graph viewport"
        );
        // A same-process close/reopen must see the latest checkpoint even while
        // its disk write is pending. A late read cannot replace those values.
        self.viewports
            .lock()
            .expect("view checkpoint lock")
            .entry(project_root.to_owned())
            .or_insert(snapshots);
        Ok(())
    }

    pub(crate) fn viewport(&self, project_root: &str, graph_path: &str) -> Option<Viewport> {
        self.viewports
            .lock()
            .expect("view checkpoint lock")
            .get(project_root)?
            .get(graph_path)
            .copied()
    }

    pub(crate) fn update_viewport(
        self: &Arc<Self>,
        project_root: &str,
        graph_path: &str,
        viewport: Viewport,
        executor: &Handle,
    ) -> Option<JoinHandle<Result<()>>> {
        if !viewport.valid() {
            return None;
        }
        {
            let mut projects = self.viewports.lock().expect("view checkpoint lock");
            let snapshots = projects.entry(project_root.to_owned()).or_default();
            if snapshots.get(graph_path) == Some(&viewport) {
                return None;
            }
            snapshots.insert(graph_path.to_owned(), viewport);
        }
        self.save_viewports(project_root.to_owned(), true, executor)
    }

    pub(crate) fn remap_viewport(&self, project_root: &str, from: &str, to: Option<&str>) {
        let mut projects = self.viewports.lock().expect("view checkpoint lock");
        let Some(snapshots) = projects.get_mut(project_root) else {
            return;
        };
        let Some(viewport) = snapshots.remove(from) else {
            return;
        };
        if let Some(to) = to {
            snapshots.insert(to.to_owned(), viewport);
        }
    }

    pub(super) fn save_viewports(
        self: &Arc<Self>,
        project_root: String,
        debounce: bool,
        executor: &Handle,
    ) -> Option<JoinHandle<Result<()>>> {
        if !self
            .viewports
            .lock()
            .expect("view checkpoint lock")
            .contains_key(&project_root)
        {
            return None;
        }
        let file = Self::viewport_file(&project_root);
        let store = self.clone();
        Some(self.queue_write(
            file,
            move || {
                let snapshots = store
                    .viewports
                    .lock()
                    .expect("view checkpoint lock")
                    .get(&project_root)
                    .cloned()
                    .unwrap_or_default();
                Ok(serde_json::to_vec_pretty(&snapshots)?)
            },
            debounce,
            executor,
        ))
    }

    fn viewport_file(project_root: &str) -> Result<String> {
        Ok(format!("viewports-{}", Self::file_name(project_root)?))
    }
}
