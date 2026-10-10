//! Replace a loading tab using the current DockArea position and selections.
use super::super::Workbench;
use gpui_kit::component::dock::{BasePanelView, InsertTarget, PaneRef, Panel, PanelId};
use gpui_kit::{Context, Entity, Window};
use std::sync::Arc;

impl Workbench {
    pub(in crate::workbench) fn replace_panel<P: Panel>(
        &self,
        previous: Entity<P>,
        replacement: Arc<dyn BasePanelView>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let old = PanelId::from(previous.entity_id());
        let Some(placement) = self.panel_placement(old, cx) else {
            return false;
        };
        self.dock.update(cx, |dock, cx| {
            let tree = dock.layout(placement).expect("located panel region");
            let node = tree.find_panel_node(old).expect("located panel group");
            let PaneRef::Tabs { panels, .. } = tree.find_node(node).expect("located group").kind()
            else {
                return false;
            };
            let ix = panels
                .iter()
                .position(|id| *id == old)
                .expect("located panel");
            let mut active = Vec::new();
            tree.root().walk(&mut |node| {
                if let PaneRef::Tabs { panels, active_ix } = node.kind()
                    && let Some(id) = panels.get(active_ix)
                {
                    active.push(*id);
                }
            });
            let new = replacement.panel_id(cx);
            // Register before moving, keeping the old tab until its group contains the replacement.
            dock.add_panel_view(replacement, placement, None, window, cx);
            dock.move_panel(
                new,
                InsertTarget::Tabs {
                    node,
                    ix: Some(ix),
                    activate: false,
                },
                window,
                cx,
            );
            dock.remove_panel(previous, window, cx);
            for selected in active {
                dock.select_panel(if selected == old { new } else { selected }, window, cx);
            }
            true
        })
    }
}
