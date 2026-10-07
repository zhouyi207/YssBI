use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, IntoElement, Render, Window, div,
    prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName,
    dock::{BasePanel, Panel, PanelEvent},
    menu::{ContextMenuExt, PopupMenuItem},
};
use std::{collections::BTreeSet, sync::Arc};
use yss_application::activity_panel::{ActivityItem, ActivityPanelDocument, ActivityRowContent};
use yss_node_catalog::NodeCreation;

use crate::{appearance, assets::NativeIcon, text::activity_text};

pub enum ActivityEvent {
    OpenGraph(String),
    CreateNode(NodeCreation),
    GraphResource(String, super::resources::GraphResourceAction),
}

pub struct ActivityPanel {
    document: Arc<ActivityPanelDocument>,
    collapsed: BTreeSet<String>,
    focus: FocusHandle,
    active_graph: Option<String>,
}

impl ActivityPanel {
    pub fn replace_document(
        &mut self,
        document: Arc<ActivityPanelDocument>,
        cx: &mut Context<Self>,
    ) {
        self.document = document;
        cx.notify();
    }
    pub fn new(document: Arc<ActivityPanelDocument>, cx: &mut Context<Self>) -> Self {
        let collapsed = document
            .rows
            .iter()
            .filter_map(|row| match &row.content {
                ActivityRowContent::Category {
                    default_expanded: false,
                    ..
                } => Some(row.id.clone()),
                _ => None,
            })
            .collect();
        Self {
            document,
            collapsed,
            focus: cx.focus_handle(),
            active_graph: None,
        }
    }
}

impl ActivityPanel {
    pub fn set_active_graph(&mut self, path: Option<&str>, cx: &mut Context<Self>) {
        if self.active_graph.as_deref() == path {
            return;
        }
        self.active_graph = path.map(str::to_owned);
        cx.notify();
    }
}

impl Render for ActivityPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut hidden_depth = None;
        let mut rows = Vec::new();
        for row in &self.document.rows {
            if hidden_depth.is_some_and(|depth| row.depth > depth) {
                continue;
            }
            hidden_depth = None;
            let id = row.id.clone();
            let item = div()
                .id(gpui::SharedString::from(id.clone()))
                .pl(px(8. + row.depth as f32 * 12.))
                .pr_2()
                .h(px(30.))
                .rounded_sm()
                .flex()
                .items_center()
                .gap_2()
                .text_size(px(13.));
            let item = match &row.content {
                ActivityRowContent::Category { label, count, .. } => {
                    let collapsed = self.collapsed.contains(&id);
                    if collapsed {
                        hidden_depth = Some(row.depth);
                    }
                    item.font_weight(gpui::FontWeight::SEMIBOLD)
                        .cursor_pointer()
                        .text_color(cx.theme().muted_foreground)
                        .hover(|style| style.bg(cx.theme().muted))
                        .child(
                            Icon::new(if collapsed {
                                IconName::ChevronRight
                            } else {
                                IconName::ChevronDown
                            })
                            .size_3(),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(activity_text(label)),
                        )
                        .children(count.map(|count| {
                            div().text_xs().px_1().rounded_sm().child(count.to_string())
                        }))
                        .on_click(cx.listener(move |view, _, _, cx| {
                            if !view.collapsed.insert(id.clone()) {
                                view.collapsed.remove(&id);
                            }
                            cx.notify();
                        }))
                        .into_any_element()
                }
                ActivityRowContent::Item(
                    ActivityItem::EventGraph { path, name }
                    | ActivityItem::FunctionGraph { path, name },
                ) => {
                    let path = path.clone();
                    let menu_path = path.clone();
                    let menu_owner = cx.entity().downgrade();
                    let expected_document = self.document.clone();
                    let active = self.active_graph.as_deref() == Some(&path);
                    item.child(
                        Icon::new(NativeIcon::Graph)
                            .size_3()
                            .text_color(gpui::rgb(appearance::BLUE)),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(name.clone()))
                    .when(active, |view| view.bg(cx.theme().sidebar_accent))
                    .cursor_pointer()
                    .hover(|style| style.bg(cx.theme().muted))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(ActivityEvent::OpenGraph(path.clone()))
                    }))
                    .context_menu(move |mut menu, _, _| {
                        use super::resources::GraphResourceAction;
                        for (label, action) in [
                            ("重命名", GraphResourceAction::Rename),
                            ("创建副本", GraphResourceAction::Duplicate),
                            ("复制相对路径", GraphResourceAction::CopyPath),
                            ("删除图", GraphResourceAction::Delete),
                        ] {
                            let owner = menu_owner.clone();
                            let path = menu_path.clone();
                            let expected = expected_document.clone();
                            menu =
                                menu.item(PopupMenuItem::new(label).on_click(move |_, _, cx| {
                                    let _ = owner.update(cx, |view, cx| {
                                        if Arc::ptr_eq(&view.document, &expected) {
                                            cx.emit(ActivityEvent::GraphResource(
                                                path.clone(),
                                                action,
                                            ));
                                        }
                                    });
                                }));
                        }
                        menu
                    })
                    .into_any_element()
                }
                ActivityRowContent::Item(ActivityItem::Node {
                    available,
                    key: _,
                    title,
                    creation,
                }) => {
                    let creation = creation.clone();
                    let available = *available;
                    item.child(
                        Icon::new(IconName::Frame)
                            .size_3()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(title.clone()))
                    .when(!available, |view| {
                        view.text_color(cx.theme().muted_foreground)
                    })
                    .when(available, |view| {
                        view.cursor_pointer()
                            .hover(|style| style.bg(cx.theme().muted))
                            .on_click(cx.listener(move |_, _, _, cx| {
                                cx.emit(ActivityEvent::CreateNode(creation.clone()))
                            }))
                    })
                    .into_any_element()
                }
                ActivityRowContent::Item(ActivityItem::Database { name, .. }) => item
                    .child(
                        Icon::new(NativeIcon::Database)
                            .size_3()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(name.clone()))
                    .into_any_element(),
                ActivityRowContent::Item(
                    ActivityItem::Chart { name, .. }
                    | ActivityItem::Mind { name, .. }
                    | ActivityItem::Doc { name, .. },
                ) => item
                    .child(
                        Icon::new(IconName::File)
                            .size_3()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(name.clone()))
                    .into_any_element(),
                ActivityRowContent::Message { label, .. } => item
                    .text_color(cx.theme().muted_foreground)
                    .child(activity_text(label))
                    .into_any_element(),
                _ => continue,
            };
            rows.push(item);
        }
        div()
            .id("activity-document")
            .track_focus(&self.focus)
            .size_full()
            .min_h_0()
            .overflow_y_scroll()
            .bg(cx.theme().sidebar)
            .p_2()
            .children(rows)
    }
}

impl EventEmitter<PanelEvent> for ActivityPanel {}
impl EventEmitter<ActivityEvent> for ActivityPanel {}
impl Focusable for ActivityPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for ActivityPanel {
    fn panel_name(&self) -> &'static str {
        self.document.panel_id
    }
    fn closable(&self, _: &App) -> bool {
        false
    }
}

impl Panel for ActivityPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                Icon::new(if self.document.panel_id == "project" {
                    IconName::Folder
                } else {
                    IconName::Frame
                })
                .size_3(),
            )
            .child(activity_text(&self.document.title))
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
