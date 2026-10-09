use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
    Window, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme, Icon, IconName, Sizable,
    button::{Button, ButtonVariants},
    dock::{BasePanel, Panel, PanelEvent},
    input::{Input, InputEvent, InputState},
    menu::{ContextMenuExt, PopupMenuItem},
};
use std::{collections::BTreeSet, sync::Arc};
use yss_application::activity_panel::{ActivityItem, ActivityPanelDocument, ActivityRowContent};
use yss_node_catalog::NodeCreation;
use yss_node_protocol::NodeTypeId;

use crate::{appearance, assets::NativeIcon, text::activity_text};

pub enum ActivityEvent {
    OpenGraph(String),
    OpenDocument(String),
    OpenMind(String),
    OpenDatabase(String),
    OpenChart(String),
    ChartResource(String, super::resources::ResourceAction),
    InspectNode(NodeTypeId),
    CreateNode(NodeCreation),
    GraphResource(String, super::resources::ResourceAction),
    DatabaseResource(String, super::resources::ResourceAction),
    ImportData,
    OpenConversation(String),
    RenameConversation(String, String),
    Tool(String),
}

pub struct ActivityPanel {
    document: Arc<ActivityPanelDocument>,
    collapsed: BTreeSet<String>,
    focus: FocusHandle,
    active_resource: Option<String>,
    search: Option<Entity<InputState>>,
    search_subscription: Option<gpui::Subscription>,
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
            active_resource: None,
            search: None,
            search_subscription: None,
        }
    }

    pub fn with_search(
        document: Arc<ActivityPanelDocument>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut panel = Self::new(document, cx);
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索会话"));
        panel.search_subscription = Some(cx.subscribe(&search, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        }));
        panel.search = Some(search);
        panel
    }
}

impl ActivityPanel {
    pub fn set_active_resource(&mut self, path: Option<&str>, cx: &mut Context<Self>) {
        if self.active_resource.as_deref() == path {
            return;
        }
        self.active_resource = path.map(str::to_owned);
        cx.notify();
    }
}

impl Render for ActivityPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self
            .search
            .as_ref()
            .map(|search| search.read(cx).value().to_lowercase())
            .unwrap_or_default();
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
                .pl(px(8. + row.depth as f32 * 14.))
                .pr_2()
                .h(px(26.))
                .rounded_sm()
                .flex()
                .items_center()
                .gap_1p5()
                .text_size(px(13.));
            let item = match &row.content {
                ActivityRowContent::Item(ActivityItem::Conversation {
                    session_id, title, ..
                }) => {
                    if !title.to_lowercase().contains(&query)
                        && !session_id.to_lowercase().contains(&query)
                    {
                        continue;
                    }
                    let active = self.active_resource.as_deref() == Some(session_id.as_str());
                    let open_id = session_id.clone();
                    let rename_id = session_id.clone();
                    let title = title.clone();
                    let expected = self.document.clone();
                    let owner = cx.entity().downgrade();
                    item.child(Icon::new(NativeIcon::Chat).size_3())
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .child(if title.is_empty() {
                                    "新对话".to_owned()
                                } else {
                                    title.clone()
                                }),
                        )
                        .cursor_pointer()
                        .when(active, |view| view.bg(cx.theme().sidebar_accent))
                        .hover(|view| view.bg(cx.theme().muted))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(ActivityEvent::OpenConversation(open_id.clone()))
                        }))
                        .context_menu(move |menu, _, _| {
                            let owner = owner.clone();
                            let expected = expected.clone();
                            let id = rename_id.clone();
                            let title = title.clone();
                            menu.item(PopupMenuItem::new("重命名").on_click(move |_, _, cx| {
                                let _ = owner.update(cx, |view, cx| {
                                    if Arc::ptr_eq(&view.document, &expected) {
                                        cx.emit(ActivityEvent::RenameConversation(
                                            id.clone(),
                                            title.clone(),
                                        ));
                                    }
                                });
                            }))
                        })
                        .into_any_element()
                }
                ActivityRowContent::Category { label, count, .. } => {
                    let collapsed = self.collapsed.contains(&id);
                    if collapsed {
                        hidden_depth = Some(row.depth);
                    }
                    item.font_weight(gpui::FontWeight::MEDIUM)
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
                    let active = self.active_resource.as_deref() == Some(&path);
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
                        use super::resources::ResourceAction;
                        for (label, action) in [
                            ("重命名", ResourceAction::Rename),
                            ("创建副本", ResourceAction::Duplicate),
                            ("复制相对路径", ResourceAction::CopyPath),
                            ("删除图", ResourceAction::Delete),
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
                    ..
                }) => {
                    let node_type = match creation {
                        NodeCreation::Static { node_type_id }
                        | NodeCreation::ParameterizedStatic { node_type_id, .. }
                        | NodeCreation::ResourceBound { node_type_id, .. } => node_type_id.clone(),
                    };
                    let creation = creation.clone();
                    let available = *available;
                    let expected = self.document.clone();
                    let expected_creation = expected.clone();
                    item.child(
                        Icon::new(gpui_kit_assets::IconName::Frame)
                            .size_3()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(title.clone()))
                    .when(!available, |view| {
                        view.text_color(cx.theme().muted_foreground).child(
                            div()
                                .text_xs()
                                .child(crate::text::translate("bayes.results.ratings.unavailable")),
                        )
                    })
                    .cursor_pointer()
                    .hover(|style| style.bg(cx.theme().muted))
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if Arc::ptr_eq(&view.document, &expected) {
                            cx.emit(ActivityEvent::InspectNode(node_type.clone()));
                        }
                    }))
                    .when(available, |view| {
                        view.child(
                            Button::new(gpui::SharedString::from(format!("add-{id}")))
                                .small()
                                .ghost()
                                .size_5()
                                .icon(gpui_kit_assets::IconName::Plus)
                                .tooltip(crate::text::translate("native.workbench.addToGraph"))
                                .on_click(cx.listener(move |view, _, _, cx| {
                                    cx.stop_propagation();
                                    if Arc::ptr_eq(&view.document, &expected_creation) {
                                        cx.emit(ActivityEvent::CreateNode(creation.clone()));
                                    }
                                })),
                        )
                    })
                    .into_any_element()
                }
                ActivityRowContent::Item(ActivityItem::Doc { path, name }) => {
                    let active = self.active_resource.as_deref() == Some(path.as_str());
                    let path = path.clone();
                    item.child(Icon::new(IconName::File).size_3())
                        .child(div().flex_1().min_w_0().truncate().child(name.clone()))
                        .cursor_pointer()
                        .when(active, |view| view.bg(cx.theme().sidebar_accent))
                        .hover(|style| style.bg(cx.theme().muted))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(ActivityEvent::OpenDocument(path.clone()))
                        }))
                        .into_any_element()
                }
                ActivityRowContent::Item(ActivityItem::Database { id, name, .. }) => {
                    let active = self.active_resource.as_deref() == Some(id.as_str());
                    let id = id.clone();
                    let menu_id = id.clone();
                    let owner = cx.entity().downgrade();
                    let expected = self.document.clone();
                    item.child(Icon::new(NativeIcon::Database).size_3())
                        .child(div().flex_1().min_w_0().truncate().child(name.clone()))
                        .cursor_pointer()
                        .when(active, |view| view.bg(cx.theme().sidebar_accent))
                        .hover(|style| style.bg(cx.theme().muted))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(ActivityEvent::OpenDatabase(id.clone()))
                        }))
                        .context_menu(move |mut menu, _, _| {
                            use super::resources::ResourceAction;
                            for (label, action) in [
                                ("重命名", ResourceAction::Rename),
                                ("创建副本", ResourceAction::Duplicate),
                                ("复制资源路径", ResourceAction::CopyPath),
                                ("删除数据库", ResourceAction::Delete),
                            ] {
                                let owner = owner.clone();
                                let id = menu_id.clone();
                                let expected = expected.clone();
                                menu = menu.item(PopupMenuItem::new(label).on_click(
                                    move |_, _, cx| {
                                        let _ = owner.update(cx, |view, cx| {
                                            if Arc::ptr_eq(&view.document, &expected) {
                                                cx.emit(ActivityEvent::DatabaseResource(
                                                    id.clone(),
                                                    action,
                                                ));
                                            }
                                        });
                                    },
                                ));
                            }
                            menu
                        })
                        .into_any_element()
                }
                ActivityRowContent::Item(ActivityItem::Mind { path, name }) => {
                    let active = self.active_resource.as_deref() == Some(path.as_str());
                    let path = path.clone();
                    item.child(Icon::new(IconName::File).size_3())
                        .child(div().flex_1().min_w_0().truncate().child(name.clone()))
                        .cursor_pointer()
                        .when(active, |view| view.bg(cx.theme().sidebar_accent))
                        .hover(|style| style.bg(cx.theme().muted))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(ActivityEvent::OpenMind(path.clone()))
                        }))
                        .into_any_element()
                }
                ActivityRowContent::Item(ActivityItem::Chart { path, name, .. }) => {
                    let path = path.clone();
                    let menu_path = path.clone();
                    let owner = cx.entity().downgrade();
                    let active = self.active_resource.as_deref() == Some(path.as_str());
                    item.child(
                        Icon::new(NativeIcon::Chart)
                            .size_3()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(div().flex_1().min_w_0().truncate().child(name.clone()))
                    .cursor_pointer()
                    .when(active, |view| view.bg(cx.theme().sidebar_accent))
                    .hover(|style| style.bg(cx.theme().muted))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        cx.emit(ActivityEvent::OpenChart(path.clone()))
                    }))
                    .context_menu(move |mut menu, _, _| {
                        for (title, action) in [
                            ("重命名…", super::resources::ResourceAction::Rename),
                            ("复制图表", super::resources::ResourceAction::Duplicate),
                            ("复制资源路径", super::resources::ResourceAction::CopyPath),
                            ("删除…", super::resources::ResourceAction::Delete),
                        ] {
                            let owner = owner.clone();
                            let path = menu_path.clone();
                            menu =
                                menu.item(PopupMenuItem::new(title).on_click(move |_, _, cx| {
                                    let _ = owner.update(cx, |_, cx| {
                                        cx.emit(ActivityEvent::ChartResource(path.clone(), action))
                                    });
                                }));
                        }
                        menu
                    })
                    .into_any_element()
                }
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
            .px_1()
            .py_1()
            .when_some(self.search.as_ref(), |view, search| {
                view.child(div().px_1().pb_1().child(Input::new(search).small()))
            })
            .children(self.document.tools.iter().map(|tool| {
                let id = tool.id.to_owned();
                Button::new(gpui::SharedString::from(format!("activity-tool-{id}")))
                    .small()
                    .ghost()
                    .label(activity_text(&tool.label))
                    .on_click(
                        cx.listener(move |_, _, _, cx| cx.emit(ActivityEvent::Tool(id.clone()))),
                    )
            }))
            .when(self.document.panel_id == "project", |view| {
                view.child(
                    Button::new("activity-import")
                        .small()
                        .ghost()
                        .icon(NativeIcon::Database)
                        .label("导入数据…")
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(ActivityEvent::ImportData))),
                )
            })
            .children(rows)
            .when(self.document.rows.is_empty(), |view| {
                view.when_some(self.document.empty_state.as_ref(), |view, (_, message)| {
                    view.child(
                        div()
                            .p_3()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(activity_text(message)),
                    )
                })
            })
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
