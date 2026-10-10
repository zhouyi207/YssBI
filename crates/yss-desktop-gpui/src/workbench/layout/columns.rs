//! Conversation and editor columns share the root DockArea and its native split state.
pub(in crate::workbench) mod sizing;

use crate::{assistant::ConversationPanel, workbench::dock::EmptyEditor};
use gpui_kit::component::{
    Placement,
    dock::{
        BasePanelView, DockArea, DockLayout, DockPlacement, InsertTarget, NodeId, PaneNode,
        PaneRef, PanelId, panel_handle,
    },
};
use gpui_kit::{App, AppContext, Axis, Context, Focusable, Window};
use sizing::default_width;
use std::{any::TypeId, sync::Arc};

pub(in crate::workbench) fn is_conversation(panel: &Arc<dyn BasePanelView>) -> bool {
    panel.view().entity_type() == TypeId::of::<ConversationPanel>()
}

pub(in crate::workbench) fn is_empty_editor(panel: &Arc<dyn BasePanelView>) -> bool {
    panel.view().entity_type() == TypeId::of::<EmptyEditor>()
}

fn conversation_node(dock: &DockArea, node: &PaneNode) -> bool {
    let mut has_panels = false;
    let mut conversations_only = true;
    node.walk(&mut |node| {
        if let PaneRef::Tabs { panels, .. } = node.kind() {
            has_panels |= !panels.is_empty();
            conversations_only &= panels
                .iter()
                .all(|id| dock.panel(*id).is_some_and(is_conversation));
        }
    });
    has_panels && conversations_only
}

pub(in crate::workbench) fn conversation_edge(dock: &DockArea) -> Option<NodeId> {
    if dock.is_zoomed() {
        return None;
    }
    let tree = dock.layout(DockPlacement::Center)?;
    let PaneRef::Split {
        axis: Axis::Horizontal,
        children,
        ..
    } = tree.root().kind()
    else {
        return None;
    };
    children
        .iter()
        .take(children.len().saturating_sub(1))
        .take_while(|node| conversation_node(dock, node))
        .last()
        .map(PaneNode::id)
}

pub(in crate::workbench) fn full_height_conversation(dock: &DockArea, group: NodeId) -> bool {
    let Some(tree) = dock.layout(DockPlacement::Center) else {
        return false;
    };
    let Some(edge) = conversation_edge(dock) else {
        return false;
    };
    let PaneRef::Split { children, .. } = tree.root().kind() else {
        return false;
    };
    for child in children {
        let mut contains = false;
        child.walk(&mut |node| contains |= node.id() == group);
        if contains {
            return true;
        }
        if child.id() == edge {
            break;
        }
    }
    false
}

pub(in crate::workbench) fn conversation_zoomed(dock: &DockArea) -> bool {
    dock.zoomed_group()
        .and_then(|node| dock.layout(DockPlacement::Center)?.find_node(node))
        .is_some_and(|node| conversation_node(dock, node))
}

fn panels(dock: &DockArea) -> Vec<Arc<dyn BasePanelView>> {
    dock.layout(DockPlacement::Center)
        .into_iter()
        .flat_map(|tree| tree.panels())
        .filter_map(|id| dock.panel(id).cloned())
        .collect()
}

fn selected(dock: &DockArea) -> Vec<PanelId> {
    let mut result = vec![];
    if let Some(tree) = dock.layout(DockPlacement::Center) {
        tree.root().walk(&mut |node| {
            if let PaneRef::Tabs { panels, active_ix } = node.kind()
                && let Some(id) = panels.get(active_ix)
            {
                result.push(*id);
            }
        });
    }
    result
}

fn group(dock: &DockArea, conversation: bool) -> Option<NodeId> {
    let tree = dock.layout(DockPlacement::Center)?;
    let mut target = None;
    tree.root().walk(&mut |node| {
        if target.is_none()
            && let PaneRef::Tabs { panels, .. } = node.kind()
            && !panels.is_empty()
            && panels.iter().all(|id| {
                dock.panel(*id)
                    .is_some_and(|panel| is_conversation(panel) == conversation)
            })
        {
            target = Some(node.id());
        }
    });
    target
}

/// Preserve the other column's active tab while registering and placing a new panel.
pub(in crate::workbench) fn present(
    dock: &mut DockArea,
    panel: Arc<dyn BasePanelView>,
    window: &mut Window,
    cx: &mut Context<DockArea>,
) {
    let conversation = is_conversation(&panel);
    let id = panel.panel_id(cx);
    let active = selected(dock);
    if conversation && group(dock, false).is_none() && panels(dock).iter().all(is_conversation) {
        add_empty_editor(dock, None, window, cx);
    }
    let target = group(dock, conversation);
    let current = dock
        .layout(DockPlacement::Center)
        .and_then(|tree| tree.find_panel_node(id));
    if current.is_none() || current != target {
        let width = default_width(dock, window);
        if dock.panel(id).is_none() {
            dock.add_panel_view(panel, DockPlacement::Center, None, window, cx);
        }
        let target = match target {
            Some(node) => InsertTarget::Tabs {
                node,
                ix: None,
                activate: true,
            },
            None => InsertTarget::Split {
                node: dock
                    .layout(DockPlacement::Center)
                    .expect("center layout")
                    .root()
                    .id(),
                placement: if conversation {
                    Placement::Left
                } else {
                    Placement::Right
                },
                size: conversation.then_some(width),
            },
        };
        dock.move_panel(id, target, window, cx);
    }
    maintain_editor_space(dock, window, cx);
    for active in active {
        dock.select_panel(active, window, cx);
    }
    dock.select_panel(id, window, cx);
}

fn add_empty_editor(
    dock: &mut DockArea,
    target: Option<NodeId>,
    window: &mut Window,
    cx: &mut Context<DockArea>,
) {
    let active = selected(dock);
    let populated = !panels(dock).is_empty();
    let root = dock
        .layout(DockPlacement::Center)
        .expect("center layout")
        .root()
        .id();
    let empty = panel_handle(cx.new(EmptyEditor::new));
    let id = empty.panel_id(cx);
    dock.add_panel_view(empty, DockPlacement::Center, None, window, cx);
    if let Some(node) = target {
        dock.move_panel(
            id,
            InsertTarget::Tabs {
                node,
                ix: None,
                activate: false,
            },
            window,
            cx,
        );
    } else if populated {
        dock.move_panel(
            id,
            InsertTarget::Split {
                node: root,
                placement: Placement::Right,
                size: None,
            },
            window,
            cx,
        );
    }
    for active in active {
        dock.select_panel(active, window, cx);
    }
}

pub(in crate::workbench) fn maintain_editor_space(
    dock: &mut DockArea,
    window: &mut Window,
    cx: &mut Context<DockArea>,
) {
    let panels = panels(dock);
    let has_conversation = panels.iter().any(is_conversation);
    let has_editor = panels
        .iter()
        .any(|panel| !is_conversation(panel) && !is_empty_editor(panel));
    let empty = panels
        .iter()
        .filter(|panel| is_empty_editor(panel))
        .filter_map(|panel| panel.view().downcast::<EmptyEditor>().ok())
        .collect::<Vec<_>>();
    if has_conversation && !has_editor && empty.is_empty() {
        add_empty_editor(dock, None, window, cx);
    } else if !has_conversation || has_editor {
        let removed_empty = !empty.is_empty();
        for empty in empty {
            dock.remove_panel(empty, window, cx);
        }
        if removed_empty && dock.is_empty(DockPlacement::Center, cx) {
            dock.focus_handle(cx).focus(window, cx);
        }
    }
}

pub(in crate::workbench) fn last_editor(dock: &DockArea, id: PanelId, cx: &App) -> bool {
    let panels = panels(dock);
    panels.iter().any(is_conversation)
        && panels
            .iter()
            .filter(|panel| !is_conversation(panel))
            .map(|panel| panel.panel_id(cx))
            .eq([id])
}

pub(in crate::workbench) fn prepare_editor_close(
    dock: &mut DockArea,
    id: PanelId,
    window: &mut Window,
    cx: &mut Context<DockArea>,
) {
    if last_editor(dock, id, cx) {
        let node = dock
            .layout(DockPlacement::Center)
            .and_then(|tree| tree.find_panel_node(id));
        // Insert before closing so native normalization retains the editor slot and widths.
        add_empty_editor(dock, node, window, cx);
    }
}

pub(super) fn restore(dock: &mut DockArea, window: &mut Window, cx: &mut Context<DockArea>) {
    let active = selected(dock);
    for panel in panels(dock).into_iter().filter(is_conversation) {
        present(dock, panel, window, cx);
    }
    maintain_editor_space(dock, window, cx);
    for active in active {
        dock.select_panel(active, window, cx);
    }
}

pub(in crate::workbench) fn reset(
    dock: &mut DockArea,
    window: &mut Window,
    cx: &mut Context<DockArea>,
) {
    let active = selected(dock);
    let (conversations, mut editors): (Vec<_>, Vec<_>) = panels(dock)
        .into_iter()
        .filter(|panel| !is_empty_editor(panel))
        .partition(is_conversation);
    if conversations.is_empty() {
        maintain_editor_space(dock, window, cx);
        return;
    }
    if editors.is_empty() {
        editors.push(panel_handle(cx.new(EmptyEditor::new)));
    }
    let tabs = |panels: Vec<Arc<dyn BasePanelView>>| {
        let index = panels
            .iter()
            .position(|panel| active.contains(&panel.panel_id(cx)))
            .unwrap_or(0);
        panels
            .into_iter()
            .fold(DockLayout::tabs(), |layout, panel| {
                layout.panel_view(panel, cx)
            })
            .active_index(index)
    };
    let layout = DockLayout::h_split()
        .child(tabs(conversations), Some(default_width(dock, window)))
        .child(tabs(editors), None);
    dock.set_center(layout, window, cx);
}
