use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::AppHandle;
use tauri_plugin_log::log::warn;

use crate::event::{Event as ProjectEvent, EventResource, emit_project_event};
use crate::project::read_project_index;

pub struct ProjectWatcherState {
    watcher: Mutex<Option<RecommendedWatcher>>,
}

impl ProjectWatcherState {
    pub fn new() -> Self {
        Self {
            watcher: Mutex::new(None),
        }
    }

    pub fn watch_project(&self, app: AppHandle, metadata_path: &str) -> Result<(), String> {
        self.stop();

        let root = crate::project::project_root_from_path(metadata_path);
        let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
        let mut watcher = notify::recommended_watcher(move |result| {
            let _ = tx.send(result);
        })
        .map_err(|e| e.to_string())?;
        watcher
            .watch(root.as_path(), RecursiveMode::Recursive)
            .map_err(|e| e.to_string())?;

        spawn_project_watcher_thread(app, metadata_path.to_string(), root, rx);
        *self.watcher.lock().unwrap() = Some(watcher);
        Ok(())
    }

    pub fn stop(&self) {
        *self.watcher.lock().unwrap() = None;
    }
}

fn spawn_project_watcher_thread(
    app: AppHandle,
    metadata_path: String,
    root: PathBuf,
    rx: mpsc::Receiver<notify::Result<Event>>,
) {
    thread::spawn(move || {
        let mut version = 0u64;
        loop {
            match rx.recv() {
                Ok(Ok(event)) => {
                    if !is_relevant_project_event(root.as_path(), &event) {
                        continue;
                    }
                    while rx.recv_timeout(Duration::from_millis(250)).is_ok() {}
                    match read_project_index(&metadata_path) {
                        Ok(_) => {
                            version = version.saturating_add(1);
                            emit_project_index_invalidated_from_watcher(&app, version);
                        }
                        Err(error) => warn!("Failed to refresh watched project index: {}", error),
                    }
                }
                Ok(Err(error)) => warn!("Project watcher error: {}", error),
                Err(_) => break,
            }
        }
    });
}

fn emit_project_index_invalidated_from_watcher(app: &AppHandle, version: u64) {
    emit_project_event(
        app,
        ProjectEvent::Resource(EventResource::ProjectIndexInvalidated {
            source: "watcher".to_string(),
            version,
        }),
    );
}

fn is_relevant_project_event(root: &Path, event: &Event) -> bool {
    event.paths.iter().any(|path| {
        let relative = path.strip_prefix(root).unwrap_or(path.as_path());
        let normalized = relative.to_string_lossy().replace('\\', "/");
        normalized.starts_with("events/")
            || normalized.starts_with("functions/")
            || normalized.starts_with("worksheets/")
            || normalized == "variables.yssbi-vars"
            || normalized == "metadata.yssbi"
            || normalized.starts_with("database/")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use notify::EventKind;

    fn path_event(root: &Path, relative: &str) -> Event {
        Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Any),
            paths: vec![root.join(relative)],
            attrs: notify::event::EventAttributes::default(),
        }
    }

    #[test]
    fn is_relevant_project_event_matches_graph_and_metadata_paths() {
        let root = PathBuf::from(r"C:\project");
        assert!(is_relevant_project_event(
            root.as_path(),
            &path_event(&root, "events/foo.yssbi-event")
        ));
        assert!(is_relevant_project_event(
            root.as_path(),
            &path_event(&root, "functions/bar.yssbi-function")
        ));
        assert!(is_relevant_project_event(
            root.as_path(),
            &path_event(&root, "metadata.yssbi")
        ));
        assert!(!is_relevant_project_event(
            root.as_path(),
            &path_event(&root, "README.md")
        ));
    }
}
