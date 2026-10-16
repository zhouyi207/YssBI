//! Native chart configuration is a draft; Project owns persisted chart files and versions.
mod commands;
mod details;
mod query;
mod render;

pub use commands::{ChartSaveOutcome, ChartSaveRequest};
pub use details::DetailsDisplay;
pub use query::{ChartRead, read};

use gpui_kit::component::dock::{BasePanel, Panel, PanelEvent, PanelInfo, PanelState};
use gpui_kit::{App, Context, EventEmitter, FocusHandle, Focusable, Window, actions};
use std::sync::Arc;
use yss_application::database::DatabaseMetaResult;
use yss_chart_document::{ChartDocument, ChartResourcePath};
use yss_desktop_services::ApplicationTasks;
use yss_project::ProjectIndex;
use yss_project_identity::{ProjectInstanceId, ResourceRevision};

actions!(native_charts, [SaveChart]);
pub enum ChartEvent {
    Activated,
}
pub struct ChartEditor {
    services: Arc<ApplicationTasks>,
    project: ProjectInstanceId,
    path: ChartResourcePath,
    revision: ResourceRevision,
    catalog: Arc<ProjectIndex>,
    saved: ChartDocument,
    draft: ChartDocument,
    focus: FocusHandle,
    meta: Option<Arc<DatabaseMetaResult>>,
    columns_page: usize,
    meta_source: Option<(String, ResourceRevision)>,
    preview: Option<Arc<query::PreviewData>>,
    draft_epoch: u64,
    read_generation: u64,
    preview_generation: u64,
    preview_task: Option<gpui_kit::Task<()>>,
    preview_limit: usize,
    _preferences_subscription: Option<gpui_kit::Subscription>,
    preview_loading: bool,
    reading: bool,
    saving: bool,
    refresh_again: bool,
    external_change: bool,
    available: bool,
    error: Option<String>,
    notifications: yss_ui::notifications::Notifications,
}
impl ChartEditor {
    fn set_error(&mut self, error: Option<String>, cx: &mut gpui_kit::Context<Self>) {
        if let Some(error) = &error {
            self.notifications.error(error.clone(), cx);
        }
        self.error = error;
    }

    pub fn new(
        services: Arc<ApplicationTasks>,
        read: query::ChartRead,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            services,
            project: read.project,
            path: read.path,
            revision: read.revision,
            catalog: read.catalog,
            saved: read.document.clone(),
            draft: read.document,
            focus: cx.focus_handle(),
            meta: None,
            columns_page: 0,
            meta_source: None,
            preview: None,
            draft_epoch: 0,
            read_generation: 0,
            preview_generation: 0,
            preview_task: None,
            preview_limit: yss_ui::preferences::current(cx).charts.max_preview_points,
            _preferences_subscription: None,
            preview_loading: false,
            reading: false,
            saving: false,
            refresh_again: false,
            external_change: false,
            available: true,
            error: None,
            notifications: yss_ui::notifications::Notifications::new(window),
        }
    }
    pub fn project(&self) -> &ProjectInstanceId {
        &self.project
    }
    pub fn path(&self) -> &ChartResourcePath {
        &self.path
    }
    pub fn dirty(&self) -> bool {
        self.draft != self.saved
    }
    pub fn busy(&self) -> bool {
        self.reading || self.saving
    }
    pub fn focus_chart(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        cx.emit(ChartEvent::Activated);
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
        if self.busy() || !self.available || self.draft_epoch != epoch {
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
        self.error = None;
        self.schedule_preview(window, cx);
        if was_dirty != self.dirty() {
            cx.emit(PanelEvent::LayoutChanged);
        }
        self.changed(cx);
    }
}
impl EventEmitter<PanelEvent> for ChartEditor {}
impl EventEmitter<ChartEvent> for ChartEditor {}
impl Focusable for ChartEditor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for ChartEditor {
    fn panel_name(&self) -> &'static str {
        "chart-editor"
    }
    fn closable(&self, _: &App) -> bool {
        !self.dirty() && !self.busy()
    }
    fn set_active(&mut self, active: bool, _: &mut Window, cx: &mut Context<Self>) {
        if active {
            cx.emit(ChartEvent::Activated);
        }
    }
    fn dump(&self, _: &App) -> PanelState {
        let mut state = PanelState::new(self.panel_name());
        state.info = PanelInfo::Panel(serde_json::json!({"chartPath": self.path.as_str()}));
        state
    }
}
impl Panel for ChartEditor {
    fn tab_name(&self, _: &App) -> Option<gpui_kit::SharedString> {
        Some(
            format!(
                "{}{}",
                self.path.display_name().as_str(),
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
