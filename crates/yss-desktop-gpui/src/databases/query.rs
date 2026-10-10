use super::DatabaseEditor;
use gpui_kit::{Context, Window};
use std::time::{Duration, Instant};
pub(super) const PAGE_ROWS: usize = 100;
pub(crate) struct DatabaseRead {
    pub(super) meta: yss_application::database::DatabaseMetaResult,
    pub(super) edit: yss_database_contract::EditState,
    pub(super) page: Option<PageRead>,
    pub(super) offset: usize,
}
pub(super) struct PageRead {
    pub rows: yss_application::database::DatabaseRowsResult,
    pub elapsed: Duration,
}
pub(crate) fn read(
    services: &yss_application::runtime::ApplicationServices,
    project: yss_project_identity::ProjectInstanceId,
    id: String,
    revision: yss_project_identity::ResourceRevision,
    offset: usize,
    retain: Option<u64>,
) -> anyhow::Result<DatabaseRead> {
    let started = Instant::now();
    let meta = services.application.query_database_meta_for_application(
        project.clone(),
        id.clone(),
        revision,
    )?;
    let edit = services
        .application
        .query_database_edit_state_for_application(project.clone(), id.clone(), revision)?;
    let page_offset = offset.min(meta.row_count.saturating_sub(1) / PAGE_ROWS * PAGE_ROWS);
    let page = if retain == Some(meta.data_revision) && page_offset == offset {
        None
    } else {
        let rows = services.application.query_database_rows_for_application(
            project,
            id,
            revision,
            page_offset,
            PAGE_ROWS,
        )?;
        Some(PageRead {
            rows,
            elapsed: started.elapsed(),
        })
    };
    Ok(DatabaseRead {
        meta,
        edit,
        page,
        offset: page_offset,
    })
}
impl DatabaseEditor {
    pub fn reload(&mut self, force_page: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            self.refresh_again = true;
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let project = self.project.clone();
        let id = self.id.clone();
        let revision = self.revision;
        let retain = if force_page {
            None
        } else {
            self.meta.as_ref().map(|meta| meta.data_revision)
        };
        let offset = self.offset;
        self.busy = true;
        self.ready = false;
        self.read_failed = false;
        self.refresh_again = false;
        let job = self
            .services
            .run(move |services| read(services, project, id, revision, offset, retain));
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.generation != generation {
                    return;
                }
                view.busy = false;
                if view.revision == revision {
                    if let Some(read) = result {
                        view.install_read(read, window, cx);
                    } else {
                        view.fail_read(cx);
                    }
                }
                if view.refresh_again {
                    view.reload(true, window, cx);
                }
                view.changed(cx);
            });
        })
        .detach();
        self.changed(cx);
    }
    pub(super) fn page(&mut self, next: bool, window: &mut Window, cx: &mut Context<Self>) {
        if !self.ready || self.busy() {
            return;
        }
        if next && self.grid.read(cx).delegate().has_more {
            self.offset += PAGE_ROWS;
        } else if !next {
            self.offset = self.offset.saturating_sub(PAGE_ROWS);
        } else {
            return;
        }
        self.reload(true, window, cx);
    }
}
