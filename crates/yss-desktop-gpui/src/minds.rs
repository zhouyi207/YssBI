//! Native topic canvas over Project's canonical Mind snapshot.
mod buffers;
mod commands;
mod details;
mod input;
mod layout;
mod render;
mod topics;

use crate::services::NativeServices;
use buffers::TopicBuffer;
pub(crate) use commands::{MindSaveOutcome, MindSaveRequest};
use gpui_kit::component::{
    dock::{BasePanel, Panel, PanelEvent, PanelInfo, PanelState},
    text::TextViewState,
};
use gpui_kit::{
    App, Bounds, Context, Entity, EventEmitter, FocusHandle, Focusable, Pixels, Point, Window,
    actions, point, px,
};
use layout::MindLayout;
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    rc::Rc,
    sync::Arc,
};
use yss_project::minds::MindSnapshot;

actions!(
    native_minds,
    [
        SaveMind,
        SelectTopics,
        DeleteTopics,
        CancelMindGesture,
        FitMind,
        FitTopics
    ]
);

pub enum MindEvent {
    Activated,
    Changed,
}

enum Gesture {
    Pan {
        press: Point<Pixels>,
        offset: Point<Pixels>,
    },
    Selection {
        press: Point<Pixels>,
        current: Point<Pixels>,
        previous: BTreeSet<String>,
        additive: bool,
    },
}

pub struct MindCanvas {
    services: Arc<NativeServices>,
    pub snapshot: MindSnapshot,
    focus: FocusHandle,
    layout: MindLayout,
    selected: BTreeSet<String>,
    collapsed: BTreeSet<String>,
    buffers: BTreeMap<String, TopicBuffer>,
    labels: BTreeMap<String, (String, Entity<TextViewState>)>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    offset: Point<Pixels>,
    zoom: f32,
    fit_pending: bool,
    gesture: Option<Gesture>,
    busy: bool,
    refreshing: bool,
    refresh_again: bool,
    pub error: Option<String>,
}

impl MindCanvas {
    pub fn new(
        services: Arc<NativeServices>,
        snapshot: MindSnapshot,
        cx: &mut Context<Self>,
    ) -> Self {
        let layout = MindLayout::new(&snapshot.content, &BTreeSet::new());
        Self {
            services,
            snapshot,
            focus: cx.focus_handle(),
            layout,
            selected: BTreeSet::new(),
            collapsed: BTreeSet::new(),
            buffers: BTreeMap::new(),
            labels: BTreeMap::new(),
            bounds: Rc::new(Cell::new(Bounds::default())),
            offset: point(px(48.), px(48.)),
            zoom: 1.,
            fit_pending: true,
            gesture: None,
            busy: false,
            refreshing: false,
            refresh_again: false,
            error: None,
        }
    }
    pub fn path(&self) -> &str {
        self.snapshot.path.as_str()
    }
    pub fn dirty(&self) -> bool {
        self.snapshot.dirty || self.buffers.values().any(|buffer| buffer.dirty)
    }
    pub fn busy(&self) -> bool {
        self.busy || self.refreshing
    }
    pub fn focus_canvas(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        cx.emit(MindEvent::Activated);
    }
    fn selected_topic(&self) -> Option<&yss_project_model::mind::MindNode> {
        if self.selected.len() != 1 {
            return None;
        }
        let id = self.selected.iter().next()?;
        self.snapshot
            .content
            .nodes
            .iter()
            .find(|node| &node.id == id)
    }
    fn rebuild_layout(&mut self) {
        self.layout = MindLayout::new(&self.snapshot.content, &self.collapsed);
        let visible = self
            .layout
            .nodes
            .iter()
            .map(|node| self.snapshot.content.nodes[node.index].id.as_str())
            .collect::<std::collections::HashSet<_>>();
        self.selected.retain(|id| visible.contains(id.as_str()));
        self.labels.retain(|id, _| visible.contains(id.as_str()));
    }
    fn changed(&self, cx: &mut Context<Self>) {
        cx.emit(MindEvent::Changed);
        cx.notify();
    }
    fn toggle_branch(&mut self, id: String, cx: &mut Context<Self>) {
        if !self.collapsed.remove(&id) {
            self.collapsed.insert(id);
        }
        self.cancel_gesture();
        self.rebuild_layout();
        self.changed(cx);
    }
}

impl EventEmitter<PanelEvent> for MindCanvas {}
impl EventEmitter<MindEvent> for MindCanvas {}
impl Focusable for MindCanvas {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for MindCanvas {
    fn panel_name(&self) -> &'static str {
        "mind-editor"
    }
    fn closable(&self, _: &App) -> bool {
        !self.dirty() && !self.busy()
    }
    fn set_active(&mut self, active: bool, _: &mut Window, cx: &mut Context<Self>) {
        if active {
            cx.emit(MindEvent::Activated);
        } else {
            self.cancel_gesture();
            cx.notify();
        }
    }
    fn dump(&self, _: &App) -> PanelState {
        let mut state = PanelState::new(self.panel_name());
        state.info = PanelInfo::Panel(serde_json::json!({ "mindPath": self.path() }));
        state
    }
}
impl Panel for MindCanvas {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        format!(
            "{}{}",
            self.snapshot.path.name(),
            if self.dirty() { " *" } else { "" }
        )
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
