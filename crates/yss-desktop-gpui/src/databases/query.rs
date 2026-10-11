use super::DatabaseEditor;
use gpui_kit::{Context, Window};
use std::time::{Duration, Instant};
pub(crate) struct DatabaseRead {
    pub(super) meta: yss_application::database::DatabaseMetaResult,
    pub(super) edit: yss_database_contract::EditState,
    pub(super) page: Option<PageRead>,
    pub(super) offset: usize,
    pub(super) page_size: usize,
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
    page_size: usize,
    retain: Option<u64>,
) -> anyhow::Result<DatabaseRead> {
    anyhow::ensure!(
        (1..=10_000).contains(&page_size),
        "Invalid database page size"
    );
    let started = Instant::now();
    let meta = services.application.query_database_meta_for_application(
        project.clone(),
        id.clone(),
        revision,
    )?;
    let edit = services
        .application
        .query_database_edit_state_for_application(project.clone(), id.clone(), revision)?;
    let page_offset = page_offset(offset, meta.row_count, page_size);
    let page = if retain == Some(meta.data_revision) && page_offset == offset {
        None
    } else {
        let rows = services.application.query_database_rows_for_application(
            project,
            id,
            revision,
            page_offset,
            page_size,
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
        page_size,
    })
}

fn page_offset(offset: usize, row_count: usize, page_size: usize) -> usize {
    offset.min(row_count.saturating_sub(1)) / page_size * page_size
}
impl DatabaseEditor {
    pub(super) fn preferences_changed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let page_size = crate::preferences::current(cx).tables.page_size;
        if self.page_size != page_size {
            self.page_size = page_size;
            self.page_epoch = self.page_epoch.wrapping_add(1);
            // Restart only the view page. Application-owned dirty session edits remain intact.
            self.offset = 0;
            self.ready = false;
            self.selection_cursor_sync = None;
            self.grid.update(cx, |table, cx| {
                *table.delegate_mut() = super::grid::DatabaseGrid::empty();
                table.clear_selection(cx);
                table.refresh(cx);
            });
            self.update_selection_preview(window, cx);
            self.reload(true, window, cx);
        }
        self.changed(cx);
    }
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
        let retain = if force_page || !self.ready {
            None
        } else {
            self.meta.as_ref().map(|meta| meta.data_revision)
        };
        let offset = self.offset;
        let page_size = self.page_size;
        let page_epoch = self.page_epoch;
        self.busy = true;
        self.ready = false;
        self.read_failed = false;
        self.refresh_again = false;
        let job = self
            .services
            .run(move |services| read(services, project, id, revision, offset, page_size, retain));
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.generation != generation {
                    return;
                }
                view.busy = false;
                if view.revision == revision
                    && view.page_size == page_size
                    && view.page_epoch == page_epoch
                {
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
            self.offset = self.offset.saturating_add(self.page_size);
        } else if !next {
            self.offset = self.offset.saturating_sub(self.page_size);
        } else {
            return;
        }
        self.reload(true, window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::page_offset;

    #[test]
    fn page_offsets_are_aligned_and_clamped() {
        assert_eq!(page_offset(177, 250, 100), 100);
        assert_eq!(page_offset(300, 250, 100), 200);
        assert_eq!(page_offset(300, 0, 100), 0);
        assert_eq!(page_offset(250, 250, 1), 249);
        assert_eq!(page_offset(usize::MAX, 25_001, 10_000), 20_000);
        assert_eq!(page_offset(200, 100, 100), 0);
    }
}
