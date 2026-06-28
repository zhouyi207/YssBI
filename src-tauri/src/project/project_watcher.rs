use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::AppHandle;
use tauri_plugin_log::log::warn;

use crate::event::{
    Event as ProjectEvent, EventResource, ProjectResourceMetaEvent, emit_project_event,
};
use crate::project::{GraphDocumentKind, ProjectIndex, read_project_index};

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
        let initial = read_project_index(metadata_path).map_err(|e| e.to_string())?;
        let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
        let mut watcher = notify::recommended_watcher(move |result| {
            let _ = tx.send(result);
        })
        .map_err(|e| e.to_string())?;
        watcher
            .watch(root.as_path(), RecursiveMode::Recursive)
            .map_err(|e| e.to_string())?;

        spawn_project_watcher_thread(app, metadata_path.to_string(), root, initial, rx);
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
    initial: ProjectIndex,
    rx: mpsc::Receiver<notify::Result<Event>>,
) {
    thread::spawn(move || {
        let mut previous = ProjectResourceSnapshot::from_index(initial);
        loop {
            match rx.recv() {
                Ok(Ok(event)) => {
                    if !is_relevant_project_event(root.as_path(), &event) {
                        continue;
                    }
                    while rx.recv_timeout(Duration::from_millis(250)).is_ok() {}
                    match read_project_index(&metadata_path) {
                        Ok(index) => {
                            let next = ProjectResourceSnapshot::from_index(index);
                            let diff = ProjectWatcherDiff::between(&previous, &next);
                            emit_resource_diff(&app, diff);
                            previous = next;
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

fn emit_resource_diff(app: &AppHandle, diff: ProjectWatcherDiff) {
    for resource in diff.changed {
        emit_project_event(
            app,
            ProjectEvent::Resource(EventResource::ResourceChanged {
                id: resource.id.clone(),
                kind: resource.kind.clone(),
                source: "watcher".to_string(),
                data: resource.into(),
            }),
        );
    }
    for (id, kind) in diff.deleted {
        emit_project_event(
            app,
            ProjectEvent::Resource(EventResource::ResourceDeleted {
                id,
                kind,
                source: "watcher".to_string(),
            }),
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchedResourceMeta {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub uri: String,
    pub folder_path: Option<String>,
}

impl From<WatchedResourceMeta> for ProjectResourceMetaEvent {
    fn from(value: WatchedResourceMeta) -> Self {
        Self {
            id: value.id,
            kind: value.kind,
            name: value.name,
            uri: value.uri,
            folder_path: value.folder_path,
            exists: true,
            loaded: false,
            has_dirty_document: false,
            has_stale_document: false,
            has_conflict_document: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectResourceSnapshot {
    resources: HashMap<(String, String), WatchedResourceMeta>,
}

impl ProjectResourceSnapshot {
    pub fn from_index(index: ProjectIndex) -> Self {
        let mut resources = HashMap::new();
        for graph in index.graphs {
            let kind = match graph.graph_type {
                GraphDocumentKind::Event => "event",
                GraphDocumentKind::Function => "function",
            }
            .to_string();
            let id = graph.id.to_string();
            resources.insert(
                (id.clone(), kind.clone()),
                WatchedResourceMeta {
                    uri: format!("yssbi://graph/{kind}/{id}"),
                    id,
                    kind,
                    name: graph.name,
                    folder_path: Some(graph.folder_path),
                },
            );
        }
        for worksheet in index.worksheets {
            let id = worksheet.id;
            resources.insert(
                (id.clone(), "worksheet".to_string()),
                WatchedResourceMeta {
                    uri: format!("yssbi://worksheet/{id}"),
                    id,
                    kind: "worksheet".to_string(),
                    name: worksheet.name,
                    folder_path: Some(worksheet.folder_path),
                },
            );
        }
        Self { resources }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProjectWatcherDiff {
    pub changed: Vec<WatchedResourceMeta>,
    pub deleted: Vec<(String, String)>,
}

impl ProjectWatcherDiff {
    pub fn between(previous: &ProjectResourceSnapshot, next: &ProjectResourceSnapshot) -> Self {
        let mut diff = ProjectWatcherDiff::default();
        for (key, next_resource) in &next.resources {
            if previous.resources.get(key) != Some(next_resource) {
                diff.changed.push(next_resource.clone());
            }
        }
        for key in previous.resources.keys() {
            if !next.resources.contains_key(key) {
                diff.deleted.push(key.clone());
            }
        }
        diff
    }
}
