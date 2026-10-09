use super::*;
use yss_application::{
    activity_panel::nodes_activity_panel_from_catalog,
    graph::catalog::{CompatibleCatalogRequest, LocalizedCatalogRequest},
};

impl NodePalette {
    pub(in crate::canvas) fn catalog_changed(
        &mut self,
        catalog: Arc<ActivityPanelDocument>,
        cx: &mut Context<Self>,
    ) {
        if let Some(form) = &self.configuration {
            form.update(cx, |form, cx| form.localize_title(&catalog, cx));
        }
        self.task = None;
        self.generation = self.generation.wrapping_add(1);
        self.loading = false;
        if self.target.source.is_some() {
            self.refresh_pending = true;
        } else {
            self.refresh_pending = false;
            self.language = crate::text::locale();
            self.browser.install(catalog);
            self.error = None;
        }
        cx.notify();
    }

    pub(super) fn query(&mut self, cx: &mut Context<Self>) {
        self.refresh_pending = false;
        if !self.current(cx) || self.creating {
            return;
        }
        self.task = None;
        self.loading = true;
        self.error = None;
        if self.language != crate::text::locale() {
            self.browser.clear_catalog();
        }
        self.language = crate::text::locale();
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let language = self.language;
        let project = self.target.project.clone();
        let path = self.target.path.clone();
        let version = self.target.version;
        let source = self.target.source.clone();
        let task = self.services.run(move |services| {
            let catalog = if let Some(source) = source {
                let document = services
                    .application
                    .current_graph_document(&project, &path, version)?;
                let catalog =
                    services
                        .application
                        .compatible_node_catalog(CompatibleCatalogRequest::new(
                            project.clone(),
                            path.clone(),
                            (*document).clone(),
                            source,
                            language,
                        ))?;
                services
                    .application
                    .current_graph_document(&project, &path, version)?;
                catalog
            } else {
                services
                    .application
                    .localized_node_catalog(LocalizedCatalogRequest::new(project, language))?
            };
            Ok(Arc::new(nodes_activity_panel_from_catalog(catalog)))
        });
        self.task = Some(cx.spawn(async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update(cx, |view, cx| {
                if view.generation != generation || language != crate::text::locale() {
                    return;
                }
                view.loading = false;
                if !view.current(cx) {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(catalog) => {
                        if let Some(form) = &view.configuration {
                            form.update(cx, |form, cx| form.localize_title(&catalog, cx));
                        }
                        view.browser.install(catalog);
                        view.scroll_to_active();
                    }
                    Err(_) => {
                        tracing::warn!(
                            code = "native_node_palette_query_failed",
                            "Node palette query failed"
                        );
                        view.error = Some("native.canvas.catalogFailed");
                    }
                }
                cx.notify();
            });
        }));
        cx.notify();
    }
}
