//! Native topic canvas over Project's canonical Mind snapshot.
mod buffers;
mod commands;
mod details;
mod input;
mod layout;
mod render;
mod topics;

use crate::file_session::{FileEditor, FileEditorEvent, FileSession};
use buffers::TopicBuffer;
pub use commands::{MindSaveOutcome, MindSaveRequest};
pub use details::DetailsDisplay;
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
use yss_desktop_services::ApplicationTasks;
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
    file: FileSession<yss_project_model::mind::MindDocument>,
    focus: FocusHandle,
    layout: MindLayout,
    selected: BTreeSet<String>,
    collapsed: BTreeSet<String>,
    buffers: BTreeMap<String, TopicBuffer>,
    labels: BTreeMap<String, (String, Entity<TextViewState>)>,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    offset: Point<Pixels>,
    zoom: f32,
    font_scale: f32,
    _appearance: gpui_kit::Subscription,
    fit_pending: bool,
    gesture: Option<Gesture>,
}

impl MindCanvas {
    pub fn new(
        services: Arc<ApplicationTasks>,
        snapshot: MindSnapshot,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let layout = MindLayout::new(&snapshot.content, &BTreeSet::new());
        let font_scale = yss_ui::preferences::current(cx).appearance.ui_font_size / 14.;
        Self {
            file: FileSession::new(services, snapshot, window),
            focus: cx.focus_handle(),
            layout,
            selected: BTreeSet::new(),
            collapsed: BTreeSet::new(),
            buffers: BTreeMap::new(),
            labels: BTreeMap::new(),
            bounds: Rc::new(Cell::new(Bounds::default())),
            offset: point(px(48. * font_scale), px(48. * font_scale)),
            zoom: font_scale,
            font_scale,
            _appearance: cx.observe_global::<yss_ui::preferences::Preferences>(|view, cx| {
                let scale = yss_ui::preferences::current(cx).appearance.ui_font_size / 14.;
                if scale == view.font_scale {
                    return;
                }
                view.cancel_gesture();
                let ratio = scale / view.font_scale;
                let center = point(
                    view.bounds.get().size.width / 2.,
                    view.bounds.get().size.height / 2.,
                );
                view.offset = center - (center - view.offset) * ratio;
                view.zoom *= ratio;
                view.font_scale = scale;
                cx.notify();
            }),
            fit_pending: true,
            gesture: None,
        }
    }
    pub fn snapshot(&self) -> &MindSnapshot {
        self.file.snapshot()
    }
    pub fn focus_canvas(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        cx.emit(FileEditorEvent::Activated);
    }
    fn selected_topic(&self) -> Option<&yss_project_model::mind::MindNode> {
        if self.selected.len() != 1 {
            return None;
        }
        let id = self.selected.iter().next()?;
        self.file
            .snapshot()
            .content
            .nodes
            .iter()
            .find(|node| &node.id == id)
    }
    fn rebuild_layout(&mut self) {
        self.layout = MindLayout::new(&self.file.snapshot().content, &self.collapsed);
        let visible = self
            .layout
            .nodes
            .iter()
            .map(|node| self.file.snapshot().content.nodes[node.index].id.as_str())
            .collect::<std::collections::HashSet<_>>();
        self.selected.retain(|id| visible.contains(id.as_str()));
        self.labels.retain(|id, _| visible.contains(id.as_str()));
    }
    fn changed(&self, cx: &mut Context<Self>) {
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
impl EventEmitter<FileEditorEvent> for MindCanvas {}
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
            cx.emit(FileEditorEvent::Activated);
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
    fn tab_name(&self, _: &App) -> Option<gpui_kit::SharedString> {
        Some(
            format!(
                "{}{}",
                self.file.snapshot().path.name(),
                if self.dirty() { " *" } else { "" }
            )
            .into(),
        )
    }
    fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        self.tab_name(cx).unwrap_or_default()
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
