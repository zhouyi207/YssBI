use super::palette::{NodePalette, PaletteEvent, PaletteTarget};
use super::{GraphCanvas, Palette};
use gpui::{AppContext, Context, Pixels, Point, Window};
use yss_graph_document::PortAddress;

impl GraphCanvas {
    pub(super) fn show_palette(
        &mut self,
        point: Point<Pixels>,
        source: Option<PortAddress>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let after_source = source.clone();
        if self.commit_port_inputs_before(window, cx, move |view, window, cx| {
            view.show_palette(point, after_source, window, cx)
        }) {
            return;
        }
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
