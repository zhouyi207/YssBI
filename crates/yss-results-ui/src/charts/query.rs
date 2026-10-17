//! All chart reads use the original project publication and database resource identities.
mod preview;
#[cfg(test)]
mod tests;
use super::ChartEditor;
use gpui_kit::{Context, Window};
pub(super) use preview::{PreviewData, PreviewFailure};
use std::{sync::Arc, time::Duration};
use yss_project::ProjectIndex;
use yss_ui::lifecycle::LogReleased;

impl ChartEditor {
    fn preview_failed(&self, summary: String, cx: &mut Context<Self>) {
        self.file
            .notifications()
            .error_with_retry(summary, cx, |view: &mut Self, window, cx| {
                view.reload(window, cx)
            });
    }

    fn catalog_details_changed(&self, next: &ProjectIndex) -> bool {
        self.catalog.databases.len() != next.databases.len()
            || self
                .catalog
                .databases
                .iter()
                .zip(&next.databases)
                .any(|(current, next)| {
                    current.id != next.id
                        || current.name != next.name
                        || (current.id == self.draft.database_id
                            && current.revision != next.revision)
                })
    }

    pub(super) fn schedule_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self._preferences_subscription.is_none() {
            self._preferences_subscription =
                Some(cx.observe_global_in::<yss_ui::preferences::Preferences>(
                    window,
                    |view, window, cx| {
                        let limit = yss_ui::preferences::current(cx).charts.max_preview_points;
                        if view.preview_limit != limit {
                            view.preview_limit = limit;
                            if !view.file.removed() {
                                view.schedule_preview(window, cx);
                            }
                        }
                    },
                ));
        }
        self.preview_limit = yss_ui::preferences::current(cx).charts.max_preview_points;
        let max_points = self.preview_limit;
        self.preview_generation = self.preview_generation.wrapping_add(1);
        let generation = self.preview_generation;
        self.preview_loading = true;
        self.preview = None;
        let project = self.file.snapshot().project_instance_id.clone();
        let catalog = self.catalog.clone();
        let document = self.draft.clone();
        let source = catalog
            .databases
            .iter()
            .find(|entry| entry.id == document.database_id)
            .map(|entry| (entry.id.clone(), entry.revision));
        if self.meta_source != source {
            self.meta = None;
            self.meta_source = None;
            self.columns_page = 0;
        }
        let metadata = self.meta.clone();
        let owner = self.file.services().clone();
        let timer = cx.background_executor().timer(Duration::from_millis(250));
        self.preview_task = Some(cx.spawn_in(window, async move |view, cx| {
            timer.await;
            let result = owner
                .run(move |services| {
                    Ok(preview::read(
                        services, project, &catalog, &document, metadata, max_points,
                    ))
                })
                .await
                .ok()
                .and_then(Result::ok);
            view.update(cx, |view, cx| {
                if view.preview_generation != generation || view.preview_limit != max_points {
                    return;
                }
                view.preview_loading = false;
                match result {
                    Some(read) => {
                        view.meta = read.meta;
                        view.meta_source = view.meta.as_ref().and(source);
                        view.columns_page =
                            view.columns_page.min(view.meta.as_ref().map_or(0, |meta| {
                                meta.columns.len().saturating_sub(1) / super::details::PAGE_COLUMNS
                            }));
                        view.preview = Some(Arc::new(read.data));
                    }
                    None => {
                        view.preview = Some(Arc::new(PreviewData::Failed(PreviewFailure::Read)))
                    }
                }
                if let Some(PreviewData::Failed(failure)) = view.preview.as_deref() {
                    tracing::warn!(code = failure.code(), "Chart preview failed");
                    view.preview_failed(failure.summary(), cx);
                }
                view.changed(cx);
            })
            .log_released();
        }));
        self.changed(cx);
    }
    pub fn replace_catalog(
        &mut self,
        catalog: Arc<ProjectIndex>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if catalog.publication_revision < self.catalog.publication_revision {
            return;
        }
        let source_revision = |catalog: &ProjectIndex| {
            catalog
                .databases
                .iter()
                .find(|entry| entry.id == self.draft.database_id)
                .map(|entry| entry.revision)
        };
        let source_changed = source_revision(&self.catalog) != source_revision(&catalog);
        if self.catalog_details_changed(&catalog) {
            // Menus capture this epoch. Unrelated publication changes do not stale them.
            self.draft_epoch = self.draft_epoch.wrapping_add(1);
        }
        self.catalog = catalog;
        if !self.file.removed()
            && (source_changed || (self.preview.is_none() && !self.preview_loading))
        {
            self.schedule_preview(window, cx);
        }
        self.changed(cx);
    }
}
