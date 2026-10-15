//! Connection menu actions reuse the shared popup and original edit transaction.
use crate::canvas::{GraphCanvas, GraphCommand, commands::DeleteSelection};
use gpui_kit::assets::IconName;
use gpui_kit::component::menu::PopupMenuItem;
use gpui_kit::{Context, Pixels, Point, Window, prelude::*};
use yss_graph_editor::EditorGraphMutation;
use yss_ui::lifecycle::LogReleased;

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
        let language = yss_ui::text::locale();
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
                let item = PopupMenuItem::new(yss_ui::text::t(label))
                    .icon(if delete {
                        IconName::Trash
                    } else {
                        IconName::Unlink
                    })
                    .when(delete, |item| item.action(Box::new(DeleteSelection)));
                menu = menu.item(item.on_click(move |_, _, cx| {
                    owner
                        .update(cx, |view, cx| {
                            if view.menu_is_current(menu_id, &projection, version, language) {
                                view.submit(
                                    GraphCommand::Edit(
                                        EditorGraphMutation::DisconnectConnections {
                                            connection_ids: ids.clone(),
                                        },
                                    ),
                                    Some(version),
                                    cx,
                                );
                            }
                        })
                        .log_released();
                }));
            }
            menu
        });
    }
}
