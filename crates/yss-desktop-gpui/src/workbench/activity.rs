mod nodes;
mod render;
mod rows;
pub(crate) use nodes::NodeDrag;

use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render,
    Window, div, prelude::*, px,
};
use gpui_component::{
    Icon,
    dock::{BasePanel, Panel, PanelEvent},
    input::{InputEvent, InputState},
};
use gpui_kit_assets::IconName;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use yss_application::activity_panel::{ActivityItem, ActivityPanelDocument, ActivityRowContent};
use yss_node_catalog::NodeCreation;
use yss_node_protocol::NodeTypeId;

use crate::text::activity_text;

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
    expanded: BTreeMap<String, bool>,
    rows: Vec<usize>,
    scroll: gpui::UniformListScrollHandle,
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
        if Arc::ptr_eq(&self.document, &document) {
            return;
        }
        if self.document.project_instance_id != document.project_instance_id {
            self.expanded.clear();
            self.active_resource = None;
        } else {
            let categories: BTreeSet<_> = document
                .rows
                .iter()
                .filter(|row| matches!(row.content, ActivityRowContent::Category { .. }))
                .map(|row| row.id.as_str())
                .collect();
            self.expanded
                .retain(|id, _| categories.contains(id.as_str()));
        }
        self.document = document;
        self.rebuild_rows(cx);
        cx.notify();
    }
    pub fn new(document: Arc<ActivityPanelDocument>, cx: &mut Context<Self>) -> Self {
        let mut panel = Self {
            document,
            expanded: BTreeMap::new(),
            rows: Vec::new(),
            scroll: gpui::UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            active_resource: None,
            search: None,
            search_subscription: None,
        };
        panel.rebuild_rows(cx);
        panel
    }

    pub fn with_search(
        document: Arc<ActivityPanelDocument>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut panel = Self::new(document, cx);
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(crate::text::translate(
                "native.workbench.searchConversations",
            ))
        });
        panel.search_subscription = Some(cx.subscribe(&search, |view, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                view.rebuild_rows(cx);
                view.scroll.scroll_to_item(0, gpui::ScrollStrategy::Top);
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
                match self.document.panel_id {
                    "project" => Icon::new(IconName::Folder),
                    "assistant" => Icon::new(IconName::MessageSquareText),
                    _ => Icon::new(IconName::Frame),
                }
                .size_3(),
            )
            .child(activity_text(&self.document.title))
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
