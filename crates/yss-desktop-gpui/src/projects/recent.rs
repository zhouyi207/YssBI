//! Recent entries are a read projection of the existing project registration owner.
mod picker;
pub(crate) use picker::RecentDelegate;

use crate::services::NativeServices;
use gpui::{Context, EntityId, EventEmitter};
use std::sync::Arc;
use yss_project_registry_contract::ProjectRecord;

pub(crate) enum RecentProjectEvent {
    Changed,
    Open {
        record: ProjectRecord,
        generation: u64,
        picker: EntityId,
    },
    Dismiss(EntityId),
}

#[derive(Clone, Default)]
pub(crate) struct RecentSnapshot {
    pub records: Arc<[ProjectRecord]>,
    pub generation: u64,
    pub loading: bool,
    pub error: Option<&'static str>,
}

pub(crate) struct RecentProjects {
    services: Arc<NativeServices>,
    snapshot: RecentSnapshot,
    again: bool,
}

impl RecentProjects {
    pub(crate) fn new(services: Arc<NativeServices>) -> Self {
        Self {
            services,
            snapshot: Default::default(),
            again: false,
        }
    }

    pub(crate) fn snapshot(&self) -> RecentSnapshot {
        self.snapshot.clone()
    }

    pub(crate) fn can_open(&self, record: &ProjectRecord, generation: u64) -> bool {
        self.snapshot.generation == generation
            && !self.snapshot.loading
            && self.snapshot.error.is_none()
            && self.snapshot.records.contains(record)
    }

    pub(crate) fn reload(&mut self, cx: &mut Context<Self>) {
        if self.snapshot.loading {
            self.again = true;
            return;
        }
        self.snapshot.generation = self.snapshot.generation.wrapping_add(1);
        self.snapshot.loading = true;
        self.snapshot.error = None;
        let generation = self.snapshot.generation;
        let services = self.services.clone();
        let job = self
            .services
            .executor
            .spawn(async move { services.application.projects.list_projects().await });
        // The workbench owns this read; closing its optional picker must not strand loading.
        cx.spawn(async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update(cx, |view, cx| {
                if view.snapshot.generation != generation {
                    return;
                }
                view.snapshot.loading = false;
                if let Some(records) = result {
                    // The owner supplies ordering; unopened scan entries aren't recent opens.
                    view.snapshot.records = records
                        .into_iter()
                        .filter(|record| record.last_opened_at.is_some())
                        .collect();
                } else {
                    view.snapshot.error = Some("native.projects.loadFailed");
                }
                if std::mem::take(&mut view.again) {
                    view.reload(cx);
                }
                cx.emit(RecentProjectEvent::Changed);
                cx.notify();
            });
        })
        .detach();
        cx.emit(RecentProjectEvent::Changed);
        cx.notify();
    }
}

impl EventEmitter<RecentProjectEvent> for RecentProjects {}
