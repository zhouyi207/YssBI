//! One popup lifecycle for graph element menus; commands retain their original owners.
use super::GraphCanvas;
use gpui::{
    Context, DismissEvent, Entity, EntityId, Focusable, IntoElement, Pixels, Point, Subscription,
    Window, anchored, deferred, prelude::*, px,
};
use gpui_component::menu::PopupMenu;
use std::sync::Arc;
use yss_graph_editor::projection::EditorProjectionModel;
use yss_project::GraphEditVersion;

pub(super) struct CanvasMenu {
    position: Point<Pixels>,
    menu: Entity<PopupMenu>,
    _dismiss: Subscription,
    pub port: Option<yss_graph_document::PortAddress>,
}

impl CanvasMenu {
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
    pub(super) fn menu_is_current(
        &self,
        id: EntityId,
        projection: &Arc<EditorProjectionModel>,
        version: GraphEditVersion,
        language: &str,
    ) -> bool {
        self.can_edit()
            && self
                .context_menu
                .as_ref()
                .is_some_and(|menu| menu.menu.entity_id() == id)
            && Arc::ptr_eq(&self.graph.projection, projection)
            && self.graph.editing.version == version
            && crate::text::locale() == language
    }

    pub(super) fn show_context_menu(
        &mut self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
        build: impl FnOnce(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu,
    ) {
        let focus = self.focus.clone();
        let menu = PopupMenu::build(window, cx, move |menu, window, cx| {
            let menu = menu.action_context(focus.clone());
            // Projection updates can remove the menu without its normal dismiss action.
            cx.on_release_in(window, move |menu, window, cx| {
                if menu.focus_handle(cx).is_focused(window) {
                    window.focus(&focus, cx);
                }
            })
            .detach();
            build(menu, window, cx)
        });
        let dismiss = cx.subscribe_in(&menu, window, |view, menu, _: &DismissEvent, _, cx| {
            if view
                .context_menu
                .as_ref()
                .is_some_and(|current| current.menu == *menu)
            {
                view.context_menu = None;
                cx.notify();
            }
        });
        window.focus(&menu.focus_handle(cx), cx);
        self.context_menu = Some(CanvasMenu {
            position,
            menu,
            _dismiss: dismiss,
            port: None,
        });
        cx.notify();
    }
}
