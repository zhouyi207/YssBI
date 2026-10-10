//! Native Markdown input and preview; Project owns the committed document and version.
pub(crate) mod commands;
mod details;
mod render;

use crate::services::NativeServices;
use gpui_kit::component::{
    dock::{BasePanel, Panel, PanelEvent, PanelInfo, PanelState},
    input::{EditorState, InputEvent},
    text::TextViewState,
};
use gpui_kit::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, Subscription, Window,
    actions,
};
use std::sync::Arc;
use yss_project::docs::DocSnapshot;

actions!(native_documents, [SaveDocument, ToggleDocumentPreview]);

pub enum DocumentEvent {
    Activated,
    Changed,
}

pub struct DocumentEditor {
    services: Arc<NativeServices>,
    pub snapshot: DocSnapshot,
    input: Entity<EditorState>,
    preview: Entity<TextViewState>,
    _input_subscription: Subscription,
    preview_visible: bool,
    preview_task: Option<gpui_kit::Task<()>>,
    draft_dirty: bool,
    busy: bool,
    refreshing: bool,
    refresh_again: bool,
    pub error: Option<String>,
}

impl DocumentEditor {
    pub fn new(
        services: Arc<NativeServices>,
        snapshot: DocSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input =
            cx.new(|cx| EditorState::new(window, cx).default_value(snapshot.content.0.clone()));
        let preview =
            cx.new(|cx| TextViewState::markdown(&snapshot.content.0, cx).selectable(true));
        let subscription = cx.subscribe(&input, |view, _, event, cx| match event {
            InputEvent::Change => {
                view.draft_dirty = view.input.read(cx).text() != view.snapshot.content.0.as_str();
                if view.preview_visible {
                    let timer = cx
                        .background_executor()
                        .timer(std::time::Duration::from_millis(200));
                    view.preview_task = Some(cx.spawn(async move |view, cx| {
                        timer.await;
                        let _ = view.update(cx, |view, cx| view.update_preview(cx));
                    }));
                }
                cx.emit(PanelEvent::LayoutChanged);
                cx.emit(DocumentEvent::Changed);
                cx.notify();
            }
            InputEvent::Focus => cx.emit(DocumentEvent::Activated),
            _ => {}
        });
        Self {
            services,
            snapshot,
            input,
            preview,
            _input_subscription: subscription,
            preview_visible: false,
            preview_task: None,
            draft_dirty: false,
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
        self.snapshot.dirty || self.draft_dirty
    }
    pub fn busy(&self) -> bool {
        self.busy || self.refreshing
    }

    pub fn is_editing(&self) -> bool {
        !self.preview_visible
    }

    pub fn focus_editor(&self, window: &mut Window, cx: &mut Context<Self>) {
        let handle = if self.preview_visible {
            self.preview.read(cx).focus_handle().clone()
        } else {
            self.input.read(cx).focus_handle(cx)
        };
        window.focus(&handle, cx);
        cx.emit(DocumentEvent::Activated);
    }

    fn update_preview(&self, cx: &mut Context<Self>) {
        let source = self.input.read(cx).value();
        self.preview
            .update(cx, |preview, cx| preview.set_text(&source, cx));
    }

    fn toggle_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preview_visible = !self.preview_visible;
        if self.preview_visible {
            self.update_preview(cx);
        }
        self.focus_editor(window, cx);
        cx.notify();
    }
}

impl EventEmitter<PanelEvent> for DocumentEditor {}
impl EventEmitter<DocumentEvent> for DocumentEditor {}
impl Focusable for DocumentEditor {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        if self.preview_visible {
            self.preview.read(cx).focus_handle().clone()
        } else {
            self.input.read(cx).focus_handle(cx)
        }
    }
}
impl BasePanel for DocumentEditor {
    fn panel_name(&self) -> &'static str {
        "document-editor"
    }
    fn closable(&self, _: &App) -> bool {
        !self.dirty() && !self.busy()
    }
    fn set_active(&mut self, active: bool, _: &mut Window, cx: &mut Context<Self>) {
        if active {
            cx.emit(DocumentEvent::Activated);
        }
    }
    fn dump(&self, _: &App) -> PanelState {
        let mut state = PanelState::new(self.panel_name());
        state.info = PanelInfo::Panel(serde_json::json!({"documentPath": self.path()}));
        state
    }
}
impl Panel for DocumentEditor {
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
