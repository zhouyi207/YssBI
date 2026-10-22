use gpui_kit::component::ActiveTheme;
use gpui_kit::{App, Hsla, IntoElement, Pixels, Point, div, prelude::*, px};
use yss_node_protocol::PortDirection;

use crate::canvas::GraphCanvas;

pub(in crate::canvas) struct PendingConnection {
    pub start: Point<Pixels>,
    pub end: Point<Pixels>,
    pub from_input: bool,
    pub color: Hsla,
    pub feedback: Option<String>,
}

impl PendingConnection {
    pub fn feedback(
        &self,
        size: gpui_kit::Size<Pixels>,
        cx: &App,
    ) -> Option<impl IntoElement + use<>> {
        let text = self.feedback.clone()?;
        let unit = cx.theme().font_size / 14.;
        Some(
            div()
                .absolute()
                .left((self.end.x + unit * 16.).min((size.width - unit * 300.).max(px(0.))))
                .top((self.end.y + unit * 16.).min((size.height - unit * 64.).max(px(0.))))
                .max_w(unit * 300.)
                .p_2()
                .rounded_md()
                .border_1()
                .border_color(self.color)
                .bg(cx.theme().background)
                .text_xs()
                .text_color(self.color)
                .child(text),
        )
    }
}

impl GraphCanvas {
    pub(in crate::canvas) fn pending_connection(&self, cx: &App) -> Option<PendingConnection> {
        let layer = &self.connection_layer;
        let (source, end, color, feedback) = if let Some(drag) = self.connection_drag() {
            let (color, feedback) = drag.feedback(cx);
            let end = drag
                .target
                .as_ref()
                .and_then(|target| layer.port_position(target, &self.preview))
                .map(|point| self.offset + point * self.zoom)
                .unwrap_or(drag.current - self.bounds.get().origin);
            (&drag.source, end, color, feedback)
        } else {
            let palette = self.palette.as_ref()?;
            (
                palette.view.read(cx).connection_source(&self.graph)?,
                palette.point,
                cx.theme().primary,
                None,
            )
        };
        Some(PendingConnection {
            start: self.offset + layer.port_position(source, &self.preview)? * self.zoom,
            end,
            color,
            feedback,
            from_input: layer.port_direction(source) == Some(PortDirection::Input),
        })
    }
}
