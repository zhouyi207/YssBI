//! Existing resource operations remain with Workbench; menu callbacks guard their source document.
use super::*;
use crate::workbench::resources::ResourceAction;
use gpui_component::menu::{PopupMenu, PopupMenuItem};

pub(super) fn resource_menu(
    mut menu: PopupMenu,
    owner: WeakEntity<ActivityPanel>,
    document: Arc<ActivityPanelDocument>,
    row: usize,
) -> PopupMenu {
    let labels = match &document.rows[row].content {
        ActivityRowContent::Item(
            ActivityItem::EventGraph { .. } | ActivityItem::FunctionGraph { .. },
        ) => [
            "contextMenu.dialog.renameSubmit",
            "contextMenu.node.duplicate",
            "native.workbench.copyRelativePath",
            "native.workbench.deleteGraph",
        ],
        ActivityRowContent::Item(ActivityItem::Database { .. }) => [
            "contextMenu.dialog.renameSubmit",
            "contextMenu.node.duplicate",
            "native.workbench.copyResourcePath",
            "native.workbench.deleteDatabase",
        ],
        ActivityRowContent::Item(ActivityItem::Chart { .. }) => [
            "native.workbench.rename",
            "native.workbench.copyChart",
            "native.workbench.copyResourcePath",
            "native.workbench.delete",
        ],
        _ => return menu,
    };
    for (label, action) in labels.into_iter().zip([
        ResourceAction::Rename,
        ResourceAction::Duplicate,
        ResourceAction::CopyPath,
        ResourceAction::Delete,
    ]) {
        let owner = owner.clone();
        let document = document.clone();
        menu = menu.item(PopupMenuItem::new(crate::text::translate(label)).on_click(
            move |_, _, cx| {
                let _ = owner.update(cx, |view, cx| {
                    if !Arc::ptr_eq(&view.document, &document) {
                        return;
                    }
                    let event = match &document.rows[row].content {
                        ActivityRowContent::Item(
                            ActivityItem::EventGraph { path, .. }
                            | ActivityItem::FunctionGraph { path, .. },
                        ) => ActivityEvent::GraphResource(path.clone(), action),
                        ActivityRowContent::Item(ActivityItem::Database { id, .. }) => {
                            ActivityEvent::DatabaseResource(id.clone(), action)
                        }
                        ActivityRowContent::Item(ActivityItem::Chart { path, .. }) => {
                            ActivityEvent::ChartResource(path.clone(), action)
                        }
                        _ => return,
                    };
                    cx.emit(event);
                });
            },
        ));
    }
    menu
}
