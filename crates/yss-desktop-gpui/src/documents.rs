//! Native Markdown input and preview; Project owns the committed document and version.
mod autosave;
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
    _subscriptions: [Subscription; 3],
    preview_visible: bool,
    preview_task: Option<gpui_kit::Task<()>>,
    autosave: autosave::AutosaveState,
    autosave_task: Option<gpui_kit::Task<()>>,
    _save_task: Option<gpui_kit::Task<()>>,
    _refresh_task: Option<gpui_kit::Task<()>>,
    autosave_in_flight: bool,
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
        let settings = crate::preferences::current(cx).editor.clone();
        let input = cx.new(|cx| {
            let mut input = EditorState::new(window, cx).default_value(snapshot.content.0.clone());
            input.set_soft_wrap(settings.soft_wrap, window, cx);
            input.set_line_number(settings.line_numbers, window, cx);
            input
        });
        let preview =
            cx.new(|cx| TextViewState::markdown(&snapshot.content.0, cx).selectable(true));
        let subscription =
            cx.subscribe_in(&input, window, |view, _, event, window, cx| match event {
                InputEvent::Change => {
                    view.draft_dirty =
                        view.input.read(cx).text() != view.snapshot.content.0.as_str();
                    view.autosave.edited();
                    view.schedule_autosave(window, cx);
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
                InputEvent::Blur => view.autosave_on_focus_change(window, cx),
                _ => {}
            });
        let preferences = cx
            .observe_global_in::<crate::preferences::Preferences>(window, |view, window, cx| {
                view.apply_preferences(window, cx)
            });
        let activation = cx.observe_window_activation(window, |view, window, cx| {
            if !window.is_window_active() {
                view.autosave_on_focus_change(window, cx);
            }
        });
        Self {
            services,
            snapshot,
            input,
            preview,
            _subscriptions: [subscription, preferences, activation],
            preview_visible: settings.open_in_preview,
            preview_task: None,
            autosave: autosave::AutosaveState::new(settings),
            autosave_task: None,
            _save_task: None,
            _refresh_task: None,
            autosave_in_flight: false,
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
    fn set_active(&mut self, active: bool, window: &mut Window, cx: &mut Context<Self>) {
        if active {
            cx.emit(DocumentEvent::Activated);
        } else {
            self.autosave_on_focus_change(window, cx);
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
