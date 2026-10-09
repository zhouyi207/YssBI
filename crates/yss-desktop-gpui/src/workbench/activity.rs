mod categories;
mod conversations;
mod drag;
mod feedback;
mod navigation;
mod nodes;
mod render;
mod resources;
mod rows;
pub(crate) use drag::{ActivityDrag, ActivityDrop};
pub(super) use feedback::ReadState;

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
    DocumentResource(String, super::resources::ResourceAction),
    MindResource(String, super::resources::ResourceAction),
    RevealResource(yss_project::RevealProjectResourceRequest),
    InspectNode(NodeTypeId),
    CreateNode(NodeCreation),
    GraphResource(String, super::resources::ResourceAction),
    DatabaseResource(String, super::resources::ResourceAction),
    ActivateConversation(String),
    RenameConversation(String, String),
    Tool(String),
    RefreshResources,
}

pub struct ActivityPanel {
    panel_id: &'static str,
    document: Option<Arc<ActivityPanelDocument>>,
    read_state: ReadState,
    focused_row: Option<String>,
    resources: Option<resources::ResourceRows>,
    expanded: BTreeMap<String, bool>,
    rows: Vec<usize>,
    scroll: gpui::UniformListScrollHandle,
    focus: FocusHandle,
    active_resource: Option<String>,
    search: Option<Entity<InputState>>,
    search_default_title: &'static str,
    conversation_owner: Option<gpui::WeakEntity<super::Workbench>>,
    search_subscription: Option<gpui::Subscription>,
    activation_subscription: Option<gpui::Subscription>,
}

impl ActivityPanel {
    pub fn replace_document(
        &mut self,
        document: Arc<ActivityPanelDocument>,
        cx: &mut Context<Self>,
    ) {
        self.set_read_state(ReadState::Ready, cx);
        if self.accepts(&document) {
            return;
        }
        if self
            .document
            .as_ref()
            .is_none_or(|previous| previous.project_instance_id != document.project_instance_id)
        {
            self.resources = None;
            self.expanded.clear();
            self.active_resource = None;
            self.focused_row = None;
            self.scroll.scroll_to_item(0, gpui::ScrollStrategy::Top);
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
        self.document = Some(document);
        self.rebuild_rows(cx);
        cx.notify();
    }
    pub fn new(document: Arc<ActivityPanelDocument>, cx: &mut Context<Self>) -> Self {
        let mut panel = Self::pending(document.panel_id, cx);
        panel.replace_document(document, cx);
        panel
    }

    fn pending(panel_id: &'static str, cx: &mut Context<Self>) -> Self {
        Self {
            panel_id,
            document: None,
            read_state: ReadState::Loading,
            focused_row: None,
            resources: None,
            expanded: BTreeMap::new(),
            rows: Vec::new(),
            scroll: gpui::UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            active_resource: None,
            search: None,
            search_default_title: conversations::title(""),
            conversation_owner: None,
            search_subscription: None,
            activation_subscription: None,
        }
    }

    fn accepts(&self, document: &Arc<ActivityPanelDocument>) -> bool {
        self.document
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, document))
    }

    pub fn pending_conversations(
        owner: gpui::WeakEntity<super::Workbench>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut panel = Self::pending("assistant", cx);
        panel.conversation_owner = Some(owner);
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
        panel.activation_subscription =
            Some(cx.observe_window_activation(window, |_, window, cx| {
                if window.is_window_active() {
                    cx.emit(ActivityEvent::RefreshResources);
                }
            }));
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
        self.panel_id
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
                match self.panel_id {
                    "project" => Icon::new(IconName::Folder),
                    "assistant" => Icon::new(IconName::MessageSquareText),
                    _ => Icon::new(IconName::Frame),
                }
                .size_3(),
            )
            .child(self.title_text())
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
