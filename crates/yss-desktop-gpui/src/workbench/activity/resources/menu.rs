//! Resource menus borrow the original directory and reject actions after it changes.
use super::*;
use crate::workbench::resources::ResourceAction;
use gpui_component::menu::{PopupMenu, PopupMenuItem};
use yss_project::RevealProjectResourceRequest;

pub(super) fn resource_menu(
    menu: PopupMenu,
    owner: WeakEntity<ActivityPanel>,
    document: Arc<ActivityPanelDocument>,
    row: usize,
) -> PopupMenu {
    let open_owner = owner.clone();
    let open_document = document.clone();
    let reveal_owner = owner.clone();
    let reveal_document = document.clone();
    let mut menu = menu
        .item(
            PopupMenuItem::new(crate::text::translate("contextMenu.sidebar.open"))
                .icon(IconName::ChevronRight)
                .on_click(move |_, _, cx| {
                    let _ = open_owner
                        .update(cx, |view, cx| view.open_resource(&open_document, row, cx));
                }),
        )
        .item(
            PopupMenuItem::new(crate::text::translate(
                "contextMenu.sidebar.revealInExplorer",
            ))
            .icon(IconName::FolderOpen)
            .on_click(move |_, _, cx| {
                let _ = reveal_owner.update(cx, |view, cx| {
                    if Arc::ptr_eq(&view.document, &reveal_document)
                        && let ActivityRowContent::Item(item) = &reveal_document.rows[row].content
                        && let Some(request) = reveal_request(item)
                    {
                        cx.emit(ActivityEvent::RevealResource(request));
                    }
                });
            }),
        );
    for (label, icon, action) in [
        (
            "contextMenu.sidebar.rename",
            IconName::Pencil,
            ResourceAction::Rename,
        ),
        (
            "contextMenu.sidebar.duplicate",
            IconName::Copy,
            ResourceAction::Duplicate,
        ),
        (
            "native.workbench.copyResourcePath",
            IconName::Clipboard,
            ResourceAction::CopyPath,
        ),
        (
            "contextMenu.sidebar.delete",
            IconName::Trash,
            ResourceAction::Delete,
        ),
    ] {
        let owner = owner.clone();
        let document = document.clone();
        if matches!(action, ResourceAction::Delete) {
            menu = menu.separator();
        }
        let item = if matches!(action, ResourceAction::Delete) {
            PopupMenuItem::element(move |_, cx| {
                div()
                    .text_color(cx.theme().danger)
                    .child(crate::text::translate(label))
            })
        } else {
            PopupMenuItem::new(crate::text::translate(label))
        };
        menu = menu.item(item.icon(icon).on_click(move |_, _, cx| {
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
                    ActivityRowContent::Item(ActivityItem::Doc { path, .. }) => {
                        ActivityEvent::DocumentResource(path.clone(), action)
                    }
                    ActivityRowContent::Item(ActivityItem::Mind { path, .. }) => {
                        ActivityEvent::MindResource(path.clone(), action)
                    }
                    _ => return,
                };
                cx.emit(event);
            });
        }));
    }
    menu
}

fn reveal_request(item: &ActivityItem) -> Option<RevealProjectResourceRequest> {
    Some(match item {
        ActivityItem::EventGraph { path, .. } => RevealProjectResourceRequest::EventGraph {
            path: yss_graph_document::GraphResourcePath::new(path.clone()).ok()?,
        },
        ActivityItem::FunctionGraph { path, .. } => RevealProjectResourceRequest::FunctionGraph {
            path: yss_graph_document::GraphResourcePath::new(path.clone()).ok()?,
        },
        ActivityItem::Doc { path, .. } => RevealProjectResourceRequest::Doc {
            path: yss_project_model::doc::DocPath::parse(path).ok()?,
        },
        ActivityItem::Mind { path, .. } => RevealProjectResourceRequest::Mind {
            path: yss_project_model::mind::MindPath::parse(path).ok()?,
        },
        ActivityItem::Chart { path, .. } => RevealProjectResourceRequest::Chart {
            chart_path: yss_chart_document::ChartResourcePath::parse(path).ok()?,
        },
        ActivityItem::Database { id, .. } => RevealProjectResourceRequest::Database {
            database_id: id.clone(),
        },
        _ => return None,
    })
}
