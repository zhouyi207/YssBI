use super::palette::{NodePalette, PaletteEvent, PaletteTarget};
use super::{Gesture, GraphCanvas, Palette};
use gpui::{AppContext, Context, Pixels, Point, Window};
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
        if self.busy {
            return;
        }
        let position = self.world(point);
        let version = self.graph.editing.version;
        let target = PaletteTarget {
            graph: cx.entity().downgrade(),
            project: self.graph.project.clone(),
            path: self.graph.projection.graph_path.clone(),
            version,
            source: source.clone(),
        };
        let catalog = (source.is_none() && self.catalog_language == crate::text::locale())
            .then(|| self.catalog.clone());
        let palette =
            cx.new(|cx| NodePalette::new(self.services.clone(), target, catalog, window, cx));
        let subscription =
            cx.subscribe_in(&palette, window, move |view, palette, event, window, cx| {
                if view
                    .palette
                    .as_ref()
                    .is_none_or(|current| current.view != *palette)
                {
                    return;
                }
                match event {
                    PaletteEvent::ConfigurationChanged => {}
                    PaletteEvent::Dismiss => {
                        view.palette = None;
                        window.focus(&view.focus, cx);
                    }
                    PaletteEvent::Create {
                        descriptor,
                        parameters,
                        port_counts,
                    } => view.submit_creation(
                        yss_graph_editor::EditorGraphMutation::CreateNode {
                            descriptor: descriptor.clone(),
                            position,
                            connect_from: source.clone(),
                            parameters: parameters.clone(),
                            port_counts: port_counts.clone(),
                            user_label: None,
                        },
                        version,
                        cx,
                    ),
                }
                cx.notify();
            });
        self.palette = Some(Palette {
            point: point - self.bounds.get().origin,
            view: palette,
            _subscription: subscription,
        });
        cx.notify();
    }
}
