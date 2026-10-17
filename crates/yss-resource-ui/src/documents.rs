//! Native Markdown input and preview; Project owns the committed document and version.
mod autosave;
mod commands;
mod details;
mod render;
pub use commands::{DocumentSaveOutcome, DocumentSaveRequest};
pub use details::DetailsDisplay;

use crate::file_session::FileEditor;
use crate::file_session::{FileEditorEvent, FileSession};
use gpui_kit::component::{
    dock::{BasePanel, Panel, PanelEvent, PanelState},
    input::{EditorState, InputEvent},
    text::{TextView, TextViewState},
};
use gpui_kit::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, Pixels, Subscription,
    Window, actions,
};
use std::{sync::Arc, time::Duration};
use yss_desktop_services::ApplicationTasks;
use yss_project::docs::DocSnapshot;
use yss_ui::lifecycle::LogReleased;

actions!(native_documents, [SaveDocument, ToggleDocumentPreview]);

const PREVIEW_RESIZE_SETTLE: Duration = Duration::from_millis(120);

pub struct DocumentEditor {
    file: FileSession<yss_project_model::doc::DocDocument>,
    input: Entity<EditorState>,
    preview: Entity<TextViewState>,
    preview_view: TextView,
    _subscriptions: [Subscription; 3],
    preview_visible: bool,
    preview_task: Option<gpui_kit::Task<()>>,
    preview_width: Option<Pixels>,
    pending_preview_width: Option<Pixels>,
    preview_resize_task: Option<gpui_kit::Task<()>>,
    autosave: autosave::AutosaveState,
    autosave_task: Option<gpui_kit::Task<()>>,
    autosave_in_flight: bool,
    draft_dirty: bool,
}

impl DocumentEditor {
    pub fn new(
        services: Arc<ApplicationTasks>,
        snapshot: DocSnapshot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let settings = yss_ui::preferences::current(cx).editor.clone();
        let input = cx.new(|cx| {
            let mut input = EditorState::new(window, cx)
                .language("markdown")
                .default_value(snapshot.content.0.clone());
            input.set_soft_wrap(settings.soft_wrap, window, cx);
            input.set_line_number(settings.line_numbers, window, cx);
            input
        });
        let preview =
            cx.new(|cx| TextViewState::markdown(&snapshot.content.0, cx).selectable(true));
        // Keep extension registrations and callbacks stable across Dock resize frames.
        let preview_view = TextView::new(&preview)
            .plugin(yss_ui::markdown::Markdown::default())
            .scrollable(true);
        let subscription =
            cx.subscribe_in(&input, window, |view, _, event, window, cx| match event {
                InputEvent::Change => {
                    let was_dirty = view.dirty();
                    view.draft_dirty =
                        view.input.read(cx).text() != view.file.snapshot().content.0.as_str();
                    view.autosave.edited();
                    view.schedule_autosave(window, cx);
                    if view.preview_visible {
                        let timer = cx
                            .background_executor()
                            .timer(std::time::Duration::from_millis(200));
                        view.preview_task = Some(cx.spawn(async move |view, cx| {
                            timer.await;
                            view.update(cx, |view, cx| view.update_preview(cx))
                                .log_released();
                        }));
                    }
                    if was_dirty != view.dirty() {
                        cx.emit(PanelEvent::LayoutChanged);
                    }
                    cx.notify();
                }
                InputEvent::Focus => cx.emit(FileEditorEvent::Activated),
                InputEvent::Blur => view.autosave_on_focus_change(window, cx),
                _ => {}
            });
        let preferences = cx
            .observe_global_in::<yss_ui::preferences::Preferences>(window, |view, window, cx| {
                view.apply_preferences(window, cx)
            });
        let activation = cx.observe_window_activation(window, |view, window, cx| {
            if !window.is_window_active() {
                view.autosave_on_focus_change(window, cx);
            }
        });
        Self {
            file: FileSession::new(services, snapshot, window),
            input,
            preview,
            preview_view,
            _subscriptions: [subscription, preferences, activation],
            preview_visible: settings.open_in_preview,
            preview_task: None,
            preview_width: None,
            pending_preview_width: None,
            preview_resize_task: None,
            autosave: autosave::AutosaveState::new(settings),
            autosave_task: None,
            autosave_in_flight: false,
            draft_dirty: false,
        }
    }

    pub fn snapshot(&self) -> &DocSnapshot {
        self.file.snapshot()
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
        cx.emit(FileEditorEvent::Activated);
    }

    fn update_preview(&self, cx: &mut Context<Self>) {
        let source = self.input.read(cx).value();
        self.preview
            .update(cx, |preview, cx| preview.set_text(&source, cx));
    }

    /// The preview's virtualized list measures every block again whenever its
    /// width changes, so following a dock drag frame by frame laid out the whole
    /// document on each frame. The preview keeps its settled width until the
    /// resize pauses.
    fn observe_preview_width(&mut self, width: Pixels, cx: &mut Context<Self>) {
        if self.preview_width.is_none_or(|settled| settled == width) {
            self.preview_width = Some(width);
            self.pending_preview_width = None;
            self.preview_resize_task = None;
            return;
        }
        if self.pending_preview_width == Some(width) {
            return;
        }
        self.pending_preview_width = Some(width);
        let timer = cx.background_executor().timer(PREVIEW_RESIZE_SETTLE);
        self.preview_resize_task = Some(cx.spawn(async move |view, cx| {
            timer.await;
            view.update(cx, |view, cx| {
                if let Some(width) = view.pending_preview_width.take() {
                    view.preview_width = Some(width);
                    cx.notify();
                }
            })
            .log_released();
        }));
    }

    fn toggle_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preview_visible = !self.preview_visible;
        if self.preview_visible {
            // The hidden preview may have missed resizes; take the current width at once.
            self.preview_width = None;
            self.pending_preview_width = None;
            self.preview_resize_task = None;
            self.update_preview(cx);
        }
        self.focus_editor(window, cx);
        cx.notify();
    }
}

impl EventEmitter<PanelEvent> for DocumentEditor {}
impl EventEmitter<FileEditorEvent> for DocumentEditor {}
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
        Self::PANEL_NAME
    }
    fn closable(&self, _: &App) -> bool {
        !self.dirty() && !self.busy()
    }
    fn set_active(&mut self, active: bool, window: &mut Window, cx: &mut Context<Self>) {
        if active {
            cx.emit(FileEditorEvent::Activated);
        } else {
            self.autosave_on_focus_change(window, cx);
        }
    }
    fn dump(&self, _: &App) -> PanelState {
        self.panel_state()
    }
}
impl Panel for DocumentEditor {
    fn tab_name(&self, _: &App) -> Option<gpui_kit::SharedString> {
        Some(self.tab_title())
    }
    fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        self.tab_name(cx).unwrap_or_default()
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
