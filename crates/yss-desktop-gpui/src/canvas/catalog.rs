use super::{Gesture, GraphCanvas, Palette};
use gpui::{Context, Pixels, Point, Window};
use std::sync::Arc;
use yss_application::{
    activity_panel::nodes_activity_panel_from_catalog, graph::catalog::CompatibleCatalogRequest,
};
use yss_graph_document::PortAddress;
use yss_graph_editor::projection::ConnectionIntent;

impl GraphCanvas {
    pub(super) fn query_connections(
        &mut self,
        source: PortAddress,
        moving: bool,
        cx: &mut Context<Self>,
    ) {
        let project = self.graph.project.clone();
        let path = self.graph.projection.graph_path.clone();
        let version = self.graph.editing.version;
        let query = source.clone();
        let task = self.services.run(move |services| {
            Ok(services.application.graph_connection_candidates(
                &project,
                &path,
                version,
                &query,
                if moving {
                    ConnectionIntent::MoveConnections
                } else {
                    ConnectionIntent::Connect
                },
            )?)
        });
        cx.spawn(async move |view,cx| {
            let result=task.await.map_err(anyhow::Error::from).and_then(|result|result);
            let _=view.update(cx,|view,cx| {
                if view.graph.editing.version!=version || !matches!(&view.gesture,Some(Gesture::Connection{source:current,..}) if current==&source){return;}
                match result {
                    Ok(candidates)=>view.connection_candidates=Some(candidates),
                    Err(_error)=>tracing::debug!(code="native_connection_candidates_rejected","Native connection candidates rejected"),
                }
                cx.notify();
            });
        }).detach();
    }

    pub(super) fn show_palette(
        &mut self,
        point: Point<Pixels>,
        source: Option<PortAddress>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.search
            .update(cx, |input, cx| input.set_value("", window, cx));
        self.palette = Some(Palette {
            point: point - self.bounds.get().origin,
            world: self.world(point),
            catalog: source.is_none().then(|| self.catalog.clone()),
            source: source.clone(),
        });
        let Some(source) = source else {
            return;
        };
        let project = self.graph.project.clone();
        let path = self.graph.projection.graph_path.clone();
        let version = self.graph.editing.version;
        let query = source.clone();
        let task = self.services.run(move |services| {
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
                        query,
                        "zh-CN",
                    ))?;
            services
                .application
                .current_graph_document(&project, &path, version)?;
            Ok(Arc::new(nodes_activity_panel_from_catalog(catalog)))
        });
        cx.spawn(async move |view, cx| {
            let result = task
                .await
                .map_err(anyhow::Error::from)
                .and_then(|result| result);
            let _ = view.update(cx, |view, cx| {
                if view.graph.editing.version != version {
                    return;
                }
                let Some(palette) = view
                    .palette
                    .as_mut()
                    .filter(|palette| palette.source.as_ref() == Some(&source))
                else {
                    return;
                };
                match result {
                    Ok(catalog) => palette.catalog = Some(catalog),
                    Err(_error) => {
                        tracing::debug!(
                            code = "native_compatible_node_catalog_rejected",
                            "Native compatible node catalog rejected"
                        );
                        view.palette = None;
                        view.error = Some("当前端口无法创建连接，请检查图状态。".into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}
