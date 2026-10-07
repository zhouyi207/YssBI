//! Native chart configuration is a draft; Project owns persisted chart files and versions.
pub(crate) mod commands;
mod details;
mod plot;
pub(crate) mod query;
mod render;

use crate::services::NativeServices;
use gpui::{App, Context, EventEmitter, FocusHandle, Focusable, Window, actions};
use gpui_component::dock::{BasePanel, Panel, PanelEvent, PanelInfo, PanelState};
use std::sync::Arc;
use yss_application::database::DatabaseMetaResult;
use yss_chart_document::{ChartDocument, ChartResourcePath};
use yss_project::ProjectIndex;
use yss_project_identity::{ProjectInstanceId, ResourceRevision};

actions!(native_charts, [SaveChart]);
pub(crate) enum ChartEvent {
    Activated,
    Changed,
}
pub(crate) struct ChartEditor {
    services: Arc<NativeServices>,
    pub project: ProjectInstanceId,
    pub path: ChartResourcePath,
    revision: ResourceRevision,
    catalog: Arc<ProjectIndex>,
    saved: ChartDocument,
    draft: ChartDocument,
    focus: FocusHandle,
    meta: Option<DatabaseMetaResult>,
    meta_source: Option<(String, ResourceRevision)>,
    preview: Option<Arc<query::PreviewData>>,
    draft_epoch: u64,
    read_generation: u64,
    preview_generation: u64,
    preview_task: Option<gpui::Task<()>>,
    preview_loading: bool,
    reading: bool,
    saving: bool,
    refresh_again: bool,
    external_change: bool,
    available: bool,
    error: Option<String>,
}
impl ChartEditor {
    pub(crate) fn new(
        services: Arc<NativeServices>,
        read: query::ChartRead,
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
            meta_source: None,
            preview: None,
            draft_epoch: 0,
            read_generation: 0,
            preview_generation: 0,
            preview_task: None,
            preview_loading: false,
            reading: false,
            saving: false,
            refresh_again: false,
            external_change: false,
            available: true,
            error: None,
        }
    }
    pub(crate) fn dirty(&self) -> bool {
        self.draft != self.saved
    }
    pub(crate) fn busy(&self) -> bool {
        self.reading || self.saving
    }
    pub(crate) fn focus_chart(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus, cx);
        cx.emit(ChartEvent::Activated);
    }
    fn changed(&self, cx: &mut Context<Self>) {
        cx.emit(ChartEvent::Changed);
        cx.notify();
    }
    fn update_draft(
        &mut self,
        epoch: u64,
        publication: u64,
        update: impl FnOnce(&mut ChartDocument),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy()
            || !self.available
            || self.draft_epoch != epoch
            || self.catalog.publication_revision != publication
        {
            return;
        }
        update(&mut self.draft);
        self.draft_epoch = self.draft_epoch.wrapping_add(1);
        self.error = None;
        self.schedule_preview(window, cx);
        cx.emit(PanelEvent::LayoutChanged);
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
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
        format!(
            "{}{}",
            self.path.display_name().as_str(),
            if self.dirty() { " *" } else { "" }
        )
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
