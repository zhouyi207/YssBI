//! Retained view checkpoints seed reopened canvases; live coordinates stay in each canvas.
use super::LayoutStore;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};
use tokio::task::JoinHandle;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Viewport {
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
    pub fn load_viewports(&self, project_root: &str) -> Result<()> {
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

    pub fn viewport(&self, project_root: &str, graph_path: &str) -> Option<Viewport> {
        self.viewports
            .lock()
            .expect("view checkpoint lock")
            .get(project_root)?
            .get(graph_path)
            .copied()
    }

    pub fn update_viewport(
        self: &Arc<Self>,
        project_root: &str,
        graph_path: &str,
        viewport: Viewport,
    ) -> Option<JoinHandle<Result<()>>> {
        if !viewport.valid() {
            return None;
        }
        {
            let mut projects = self.viewports.lock().expect("view checkpoint lock");
            if let Some(snapshots) = projects.get_mut(project_root) {
                if let Some(current) = snapshots.get_mut(graph_path) {
                    if *current == viewport {
                        return None;
                    }
                    *current = viewport;
                } else {
                    snapshots.insert(graph_path.to_owned(), viewport);
                }
            } else {
                projects.insert(
                    project_root.to_owned(),
                    BTreeMap::from([(graph_path.to_owned(), viewport)]),
                );
            }
        }
        self.save_viewports(project_root.to_owned(), true)
    }

    pub fn remap_viewport(&self, project_root: &str, from: &str, to: Option<&str>) {
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
        ))
    }

    fn viewport_file(project_root: &str) -> Result<String> {
        Ok(format!("viewports-{}", Self::file_name(project_root)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::component::dock::DockAreaState;

    #[test]
    fn latest_viewports_survive_close_while_debounced_writes_are_pending() -> Result<()> {
        let directory =
            std::env::temp_dir().join(format!("yssbi-viewports-{}", uuid::Uuid::new_v4()));
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let store = Arc::new(LayoutStore::new(&directory, runtime.handle().clone()));
        let first = Viewport {
            x: 12.,
            y: 24.,
            scale: 1.,
        };
        let latest = Viewport {
            x: -48.,
            y: 72.,
            scale: 0.5,
        };
        runtime.block_on(async {
            store.load_viewports("project")?;
            let initial_write = store
                .update_viewport("project", "events/first.yssbi-event", first)
                .ok_or_else(|| anyhow::anyhow!("initial viewport was not accepted"))?;
            let other_write = store
                .update_viewport("project", "events/second.yssbi-event", first)
                .ok_or_else(|| anyhow::anyhow!("second viewport was not accepted"))?;
            let latest_write = store
                .update_viewport("project", "events/first.yssbi-event", latest)
                .ok_or_else(|| anyhow::anyhow!("latest viewport was not accepted"))?;

            // Reopening a tab reads the live checkpoint before its disk debounce completes.
            store.load_viewports("project")?;
            assert_eq!(
                store.viewport("project", "events/first.yssbi-event"),
                Some(latest)
            );
            assert_eq!(
                store.viewport("project", "events/second.yssbi-event"),
                Some(first)
            );

            store
                .save("project".into(), DockAreaState::default())
                .await??;
            initial_write.await??;
            other_write.await??;
            latest_write.await??;

            let reopened = LayoutStore::new(&directory, runtime.handle().clone());
            reopened.load_viewports("project")?;
            assert_eq!(
                reopened.viewport("project", "events/first.yssbi-event"),
                Some(latest)
            );
            assert_eq!(
                reopened.viewport("project", "events/second.yssbi-event"),
                Some(first)
            );
            anyhow::Ok(())
        })?;
        std::fs::remove_dir_all(directory)?;
        Ok(())
    }
}
