use std::path::Path;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc::{Receiver, SyncSender, sync_channel},
};
use std::thread;
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::AppHandle;

use crate::event::{Event as ProjectEvent, EventResource, emit_project_event};
use crate::project::{ProjectInstanceId, read_project_index};

struct ActiveProjectWatcher {
    _watcher: RecommendedWatcher,
    cancelled: Arc<AtomicBool>,
}

pub struct ProjectWatcherState {
    active: Mutex<Option<ActiveProjectWatcher>>,
}

impl ProjectWatcherState {
    pub fn new() -> Self {
        Self {
            active: Mutex::new(None),
        }
    }

    pub fn watch_project(
        &self,
        app: AppHandle,
        metadata_path: &str,
        project_instance_id: ProjectInstanceId,
    ) -> Result<(), String> {
        let root = crate::project::project_root_from_path(metadata_path);
        let cancelled = Arc::new(AtomicBool::new(false));
        // A burst only needs one invalidation. Filter before enqueueing so an
        // unrelated path cannot occupy the sole pending signal.
        let (tx, rx) = coalescing_channel();
        let callback_root = root.clone();
        let mut watcher = notify::recommended_watcher(move |result| {
            enqueue_relevant_change(&tx, callback_root.as_path(), result);
        })
        .map_err(|e| e.to_string())?;
        watcher
            .watch(root.as_path(), RecursiveMode::Recursive)
            .map_err(|e| e.to_string())?;

        spawn_project_watcher_thread(
            app,
            metadata_path.to_string(),
            project_instance_id,
            rx,
            Arc::clone(&cancelled),
        )?;
        let previous = {
            let mut active = self.active.lock().unwrap();
            let previous = active.replace(ActiveProjectWatcher {
                _watcher: watcher,
                cancelled,
            });
            if let Some(previous) = previous.as_ref() {
                previous.cancelled.store(true, Ordering::Release);
            }
            previous
        };
        drop(previous);
        Ok(())
    }

    pub fn stop(&self) {
        let active = self.active.lock().unwrap().take();
        if let Some(active) = active.as_ref() {
            active.cancelled.store(true, Ordering::Release);
        }
        drop(active);
    }
}

fn coalescing_channel() -> (SyncSender<()>, Receiver<()>) {
    sync_channel(1)
}

fn enqueue_relevant_change(tx: &SyncSender<()>, root: &Path, result: notify::Result<Event>) {
    match result {
        Ok(event) if is_relevant_project_event(root, &event) => {
            let _ = tx.try_send(());
        }
        Ok(_) => {}
        Err(error) => {
            tracing::warn!(
                target: "yssbi::project::watcher",
                diagnostic_domain = "system",
                diagnostic_event = "watcherError",
                error = %error,
                "Project watcher error"
            );
            // A watcher error may hide a relevant change, so conservatively
            // invalidate the index instead of silently losing synchronization.
            let _ = tx.try_send(());
        }
    }
}

fn spawn_project_watcher_thread(
    app: AppHandle,
    metadata_path: String,
    project_instance_id: ProjectInstanceId,
    rx: Receiver<()>,
    cancelled: Arc<AtomicBool>,
) -> Result<(), String> {
    let worker = thread::Builder::new()
        .name("yssbi-project-watcher".into())
        .spawn(move || {
            let mut version = 0u64;
            loop {
                match rx.recv() {
                    Ok(()) => {
                        while rx.recv_timeout(Duration::from_millis(250)).is_ok() {}
                        if cancelled.load(Ordering::Acquire) {
                            break;
                        }
                        match read_project_index(&metadata_path) {
                            Ok(_) => {
                                let Some(next_version) = version.checked_add(1) else {
                                    tracing::error!(
                                        target: "yssbi::project::watcher",
                                        diagnostic_domain = "system",
                                        diagnostic_event = "watcherVersionExhausted",
                                        "Project watcher event version is exhausted"
                                    );
                                    break;
                                };
                                version = next_version;
                                emit_project_index_invalidated_from_watcher(
                                    &app,
                                    &project_instance_id,
                                    version,
                                );
                            }
                            Err(error) => tracing::warn!(
                                target: "yssbi::project::watcher",
                                diagnostic_domain = "system",
                                diagnostic_event = "projectIndexRefreshFailed",
                                error = %error,
                                "Failed to refresh watched project index"
                            ),
                        }
                    }
                    Err(_) => break,
                }
            }
        })
        .map_err(|error| error.to_string())?;
    drop(worker);
    Ok(())
}

fn emit_project_index_invalidated_from_watcher(
    app: &AppHandle,
    project_instance_id: &ProjectInstanceId,
    version: u64,
) {
    emit_project_event(
        app,
        ProjectEvent::Resource(EventResource::ProjectIndexInvalidated {
            project_instance_id: project_instance_id.clone(),
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
    use std::path::PathBuf;

    fn path_event(root: &Path, relative: &str) -> Event {
        Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Any),
            paths: vec![root.join(relative)],
            attrs: notify::event::EventAttributes::default(),
        }
    }

    #[test]
    fn unrelated_event_does_not_block_a_relevant_invalidation() {
        let root = PathBuf::from(r"C:\project");
        let (tx, rx) = coalescing_channel();

        enqueue_relevant_change(&tx, root.as_path(), Ok(path_event(&root, "README.md")));
        enqueue_relevant_change(
            &tx,
            root.as_path(),
            Ok(path_event(&root, "events/foo.yssbi-event")),
        );

        rx.recv().expect("relevant event is retained");
        assert!(rx.try_recv().is_err());
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
