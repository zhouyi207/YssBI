//! All chart reads use the original project publication and database resource identities.
mod preview;
use super::ChartEditor;
use gpui::{Context, Window};
pub(super) use preview::{PreviewData, PreviewFailure};
use std::{sync::Arc, time::Duration};
use yss_application::runtime::ApplicationServices;
use yss_chart_document::{ChartDocument, ChartResourcePath};
use yss_project::ProjectIndex;
use yss_project_identity::{ProjectInstanceId, ResourceRevision};

pub(crate) struct ChartRead {
    pub project: ProjectInstanceId,
    pub path: ChartResourcePath,
    pub revision: ResourceRevision,
    pub catalog: Arc<ProjectIndex>,
    pub document: ChartDocument,
}
pub(crate) fn read(
    services: &ApplicationServices,
    project: ProjectInstanceId,
    path: ChartResourcePath,
) -> anyhow::Result<ChartRead> {
    let catalog = services
        .application
        .query_project_index(project.clone(), crate::text::locale(), true)?
        .index;
    let revision = catalog
        .charts
        .iter()
        .find(|entry| entry.chart_path == path)
        .ok_or_else(|| anyhow::anyhow!("chart absent from project index"))?
        .revision;
    let document = services.application.load_chart_resource(
        project.clone(),
        path.clone(),
        Some(catalog.publication_revision),
    )?;
    Ok(ChartRead {
        project,
        path,
        revision,
        catalog: Arc::new(catalog),
        document,
    })
}
impl ChartEditor {
    pub(super) fn schedule_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preview_generation = self.preview_generation.wrapping_add(1);
        let generation = self.preview_generation;
        self.preview_loading = true;
        self.preview = None;
        let project = self.project.clone();
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
        let owner = self.services.clone();
        let timer = cx.background_executor().timer(Duration::from_millis(250));
        self.preview_task = Some(cx.spawn_in(window, async move |view, cx| {
            timer.await;
            let result = owner
                .run(move |services| {
                    Ok(preview::read(
                        services, project, &catalog, &document, metadata,
                    ))
                })
                .await
                .ok()
                .and_then(Result::ok);
            let _ = view.update(cx, |view, cx| {
                if view.preview_generation != generation {
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
                view.changed(cx);
            });
        }));
        self.changed(cx);
    }
    pub(crate) fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            self.refresh_again = true;
            return;
        }
        self.read_generation = self.read_generation.wrapping_add(1);
        let generation = self.read_generation;
        self.reading = true;
        self.error = None;
        let project = self.project.clone();
        let path = self.path.clone();
        let task = self
            .services
            .run(move |services| read(services, project, path));
        cx.spawn_in(window, async move |view, cx| {
            let result = task.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.read_generation != generation {
                    return;
                }
                view.reading = false;
                if let Some(read) = result {
                    if read.catalog.publication_revision < view.catalog.publication_revision {
                        view.refresh_again = view.available;
                    } else {
                        let dirty = view.dirty();
                        view.external_change = dirty
                            && view.draft != read.document
                            && (view.external_change || read.document != view.saved);
                        view.revision = read.revision;
                        view.catalog = read.catalog;
                        view.saved = read.document.clone();
                        if !dirty {
                            view.draft = read.document;
                            view.draft_epoch = view.draft_epoch.wrapping_add(1);
                        }
                        view.available = true;
                        view.schedule_preview(window, cx);
                    }
                } else {
                    view.error = Some(crate::text::t("native.charts.readFailed").into());
                }
                if view.refresh_again {
                    view.refresh_again = false;
                    view.refresh(window, cx);
                }
                view.changed(cx);
            });
        })
        .detach();
        self.changed(cx);
    }
    pub(crate) fn replace_catalog(
        &mut self,
        catalog: Arc<ProjectIndex>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if catalog.publication_revision < self.catalog.publication_revision {
            return;
        }
        let revision = catalog
            .charts
            .iter()
            .find(|entry| entry.chart_path == self.path)
            .map(|entry| entry.revision);
        let source_revision = |catalog: &ProjectIndex| {
            catalog
                .databases
                .iter()
                .find(|entry| entry.id == self.draft.database_id)
                .map(|entry| entry.revision)
        };
        let source_changed = source_revision(&self.catalog) != source_revision(&catalog);
        self.catalog = catalog;
        let Some(revision) = revision else {
            self.available = false;
            self.read_generation = self.read_generation.wrapping_add(1);
            self.reading = false;
            self.refresh_again = false;
            self.preview_generation = self.preview_generation.wrapping_add(1);
            self.preview_task = None;
            self.preview_loading = false;
            self.preview = None;
            self.error = Some(crate::text::t("native.charts.removed").into());
            self.changed(cx);
            return;
        };
        self.available = true;
        if revision != self.revision {
            self.refresh(window, cx);
        } else if source_changed || (self.preview.is_none() && !self.preview_loading) {
            self.schedule_preview(window, cx);
        }
        self.changed(cx);
    }
}
