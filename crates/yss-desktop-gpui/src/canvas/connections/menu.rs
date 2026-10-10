//! Connection menu actions reuse the shared popup and original edit transaction.
use crate::canvas::{GraphCanvas, GraphCommand};
use gpui::{Context, Pixels, Point, Window, div, prelude::*};
use gpui_component::{ActiveTheme, Icon, menu::PopupMenuItem};
use gpui_kit_assets::IconName;
use yss_graph_editor::EditorGraphMutation;

impl GraphCanvas {
    pub(super) fn show_connection_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.commit_port_inputs_before(window, cx, move |view, window, cx| {
            view.show_connection_menu(position, window, cx)
        }) {
            return;
        }
        let ids = self
            .selected_connections
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let projection = self.graph.projection.clone();
        let owner = cx.entity().downgrade();
        let version = self.graph.editing.version;
        let language = crate::text::locale();
        self.show_context_menu(position, window, cx, move |mut menu, _, cx| {
            let menu_id = cx.entity_id();
            for delete in [false, true] {
                let owner = owner.clone();
                let projection = projection.clone();
                let ids = ids.clone();
                let label = if delete {
                    "contextMenu.connection.delete"
                } else if ids.len() > 1 {
                    "contextMenu.connection.breakSelectedLinks"
                } else {
                    "contextMenu.connection.breakLink"
                };
                let item = if delete {
                    PopupMenuItem::element(move |_, cx| {
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .text_color(cx.theme().danger)
                            .child(Icon::new(IconName::Trash).size_4())
                            .child(div().flex_1().child(crate::text::t(label)))
                            .child("Del")
                    })
                } else {
                    PopupMenuItem::new(crate::text::t(label)).icon(IconName::Unlink)
                };
                menu = menu.item(item.on_click(move |_, _, cx| {
                    let _ = owner.update(cx, |view, cx| {
                        if view.menu_is_current(menu_id, &projection, version, language) {
                            view.submit(
                                GraphCommand::Edit(EditorGraphMutation::DisconnectConnections {
                                    connection_ids: ids.clone(),
                                }),
                                Some(version),
                                cx,
                            );
                        }
                    });
                }));
            }
            menu
        });
    }
}
