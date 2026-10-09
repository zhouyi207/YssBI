//! Menus capture the displayed projection and use the normal graph edit transaction.
use crate::canvas::{GraphCanvas, GraphCommand};
use gpui::{
    Context, DismissEvent, Entity, Focusable, IntoElement, Pixels, Point, Subscription, Window,
    anchored, deferred, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Icon,
    menu::{PopupMenu, PopupMenuItem},
};
use gpui_kit_assets::IconName;
use std::sync::Arc;
use yss_graph_editor::EditorGraphMutation;

pub(in crate::canvas) struct ConnectionMenu {
    position: Point<Pixels>,
    menu: Entity<PopupMenu>,
    _dismiss: Subscription,
}

impl ConnectionMenu {
    pub fn render(&self) -> impl IntoElement + use<> {
        deferred(
            anchored()
                .position(self.position)
                .snap_to_window_with_margin(px(8.))
                .child(self.menu.clone()),
        )
        .with_priority(1)
    }
}

impl GraphCanvas {
    pub(super) fn show_connection_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let ids = self
            .selected_connections
            .iter()
            .copied()
            .collect::<Vec<_>>();
        let projection = self.graph.projection.clone();
        let owner = cx.entity().downgrade();
        let focus = self.focus.clone();
        let version = self.graph.editing.version;
        let menu = PopupMenu::build(window, cx, move |mut menu, window, cx| {
            let menu_id = cx.entity_id();
            menu = menu.action_context(focus.clone());
            // Projection updates can remove the menu without its normal dismiss action.
            cx.on_release_in(window, move |menu, window, cx| {
                if menu.focus_handle(cx).is_focused(window) {
                    window.focus(&focus, cx);
                }
            })
            .detach();
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
                        if !view.busy
                            && view
                                .connection_menu
                                .as_ref()
                                .is_some_and(|menu| menu.menu.entity_id() == menu_id)
                            && Arc::ptr_eq(&view.graph.projection, &projection)
                        {
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
        let dismiss = cx.subscribe_in(&menu, window, |view, menu, _: &DismissEvent, _, cx| {
            if view
                .connection_menu
                .as_ref()
                .is_some_and(|current| current.menu == *menu)
            {
                view.connection_menu = None;
                cx.notify();
            }
        });
        window.focus(&menu.focus_handle(cx), cx);
        self.connection_menu = Some(ConnectionMenu {
            position,
            menu,
            _dismiss: dismiss,
        });
    }
}
