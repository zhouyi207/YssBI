use super::{Gesture, GraphCanvas, Palette};
use gpui::{AppContext, Context, Pixels, Point, Window};
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
                    Ok(candidates)=>view.connection_candidates=Some(candidates.candidates.into_iter().map(|candidate|(candidate.port,candidate.decision)).collect()),
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
            configure_first: false,
            configuration: None,
            configuration_subscription: None,
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

impl GraphCanvas {
    pub(super) fn configure_node(
        &mut self,
        descriptor: yss_node_catalog::NodeCreation,
        title: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy {
            return;
        }
        let Some(palette) = &self.palette else {
            return;
        };
        let position = palette.world;
        let connect_from = palette.source.clone();
        let version = self.graph.editing.version;
        let target = crate::workbench::CreationTarget {
            graph: cx.entity().downgrade(),
            project: self.graph.project.clone(),
            version,
            descriptor: descriptor.clone(),
            title,
        };
        let form = cx.new(|cx| {
            crate::workbench::NodeCreationView::new(self.services.clone(), target, window, cx)
        });
        let id = form.entity_id();
        let subscription = cx.subscribe(&form, move |view, _, event, cx| {
            if view
                .palette
                .as_ref()
                .and_then(|palette| palette.configuration.as_ref())
                .is_none_or(|form| form.entity_id() != id)
            {
                return;
            }
            match event {
                crate::workbench::CreationEvent::Back => {
                    let palette = view.palette.as_mut().expect("current palette");
                    palette.configuration = None;
                    palette.configuration_subscription = None;
                    cx.notify();
                }
                crate::workbench::CreationEvent::Create {
                    parameters,
                    port_counts,
                } => {
                    view.submit_creation(
                        yss_graph_editor::EditorGraphMutation::CreateNode {
                            descriptor: descriptor.clone(),
                            position,
                            connect_from: connect_from.clone(),
                            parameters: parameters.clone(),
                            port_counts: port_counts.clone(),
                            user_label: None,
                        },
                        version,
                        cx,
                    );
                }
            }
        });
        let palette = self.palette.as_mut().expect("palette open");
        palette.configuration = Some(form);
        palette.configuration_subscription = Some(subscription);
        cx.notify();
    }
}
