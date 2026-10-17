//! Native chart configuration is a draft; Project owns persisted chart files and versions.
mod commands;
mod details;
mod query;
mod render;

pub use commands::{ChartSaveOutcome, ChartSaveRequest};
pub use details::DetailsDisplay;

use gpui_kit::component::dock::{BasePanel, Panel, PanelEvent, PanelState};
use gpui_kit::{App, Context, EventEmitter, FocusHandle, Focusable, Window, actions};
use std::sync::Arc;
use yss_application::database::DatabaseMetaResult;
use yss_chart_document::ChartDocument;
use yss_desktop_services::ApplicationTasks;
use yss_project::{ProjectIndex, charts::ChartSnapshot};
use yss_project_identity::{ProjectInstanceId, ResourceRevision};
use yss_resource_ui::file_session::{FileEditor, FileEditorEvent, FileSession};

actions!(native_charts, [SaveChart]);
pub struct ChartEditor {
    file: FileSession<ChartDocument>,
    catalog: Arc<ProjectIndex>,
    draft: ChartDocument,
    focus: FocusHandle,
    meta: Option<Arc<DatabaseMetaResult>>,
    columns_page: usize,
    meta_source: Option<(String, ResourceRevision)>,
    preview: Option<Arc<query::PreviewData>>,
    draft_epoch: u64,
    preview_generation: u64,
    preview_task: Option<gpui_kit::Task<()>>,
    preview_limit: usize,
    _preferences_subscription: Option<gpui_kit::Subscription>,
    preview_loading: bool,
}
impl ChartEditor {
    pub fn new(
        services: Arc<ApplicationTasks>,
        snapshot: ChartSnapshot,
        catalog: Arc<ProjectIndex>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            draft: snapshot.content.clone(),
            file: FileSession::new(services, snapshot, window),
            catalog,
            focus: cx.focus_handle(),
            meta: None,
            columns_page: 0,
            meta_source: None,
            preview: None,
            draft_epoch: 0,
            preview_generation: 0,
            preview_task: None,
            preview_limit: yss_ui::preferences::current(cx).charts.max_preview_points,
            _preferences_subscription: None,
            preview_loading: false,
        }
    }
    pub fn project(&self) -> &ProjectInstanceId {
        &self.file.snapshot().project_instance_id
    }
    pub fn focus_chart(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        cx.emit(FileEditorEvent::Activated);
    }
    /// A draft remains editable while a newer file version waits for the user's resolution.
    fn draft_editable(&self) -> bool {
        !self.file.busy() && !self.file.removed()
    }
    fn changed(&self, cx: &mut Context<Self>) {
        cx.notify();
    }
    fn update_draft(
        &mut self,
        epoch: u64,
        update: impl FnOnce(&mut ChartDocument),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.draft_editable() || self.draft_epoch != epoch {
            return;
        }
        let mut draft = self.draft.clone();
        update(&mut draft);
        if draft == self.draft {
            return;
        }
        let was_dirty = self.dirty();
        self.draft = draft;
        self.draft_epoch = self.draft_epoch.wrapping_add(1);
        self.file.clear_error();
        self.schedule_preview(window, cx);
        if was_dirty != self.dirty() {
            cx.emit(PanelEvent::LayoutChanged);
        }
        self.changed(cx);
    }
}
impl EventEmitter<PanelEvent> for ChartEditor {}
impl EventEmitter<FileEditorEvent> for ChartEditor {}
impl Focusable for ChartEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for ChartEditor {
    fn panel_name(&self) -> &'static str {
        Self::PANEL_NAME
    }
    fn closable(&self, _: &App) -> bool {
        !self.dirty() && !self.busy()
    }
    fn set_active(&mut self, active: bool, _: &mut Window, cx: &mut Context<Self>) {
        if active {
            cx.emit(FileEditorEvent::Activated);
        }
    }
    fn dump(&self, _: &App) -> PanelState {
        self.panel_state()
    }
}
impl Panel for ChartEditor {
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
