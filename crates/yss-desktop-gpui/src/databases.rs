//! Native data pages consume revision-bound Application queries; Rust owns all dataset edits.
mod commands;
mod details;
mod grid;
pub(crate) mod query;
mod render;
mod selection;
mod semantic;
mod toolbar;

use crate::services::NativeServices;
pub(crate) use commands::{DatabaseSaveOutcome, DatabaseSaveRequest};
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, Subscription, Window,
    actions,
};
use gpui_component::{
    dock::{BasePanel, Panel, PanelEvent, PanelInfo, PanelState},
    input::TextareaState,
    table::{TableSelection, TableState},
};
use grid::DatabaseGrid;
use std::sync::Arc;
use yss_application::database::DatabaseMetaResult;
use yss_database_contract::EditState;
use yss_project_identity::{ProjectInstanceId, ResourceRevision};

actions!(
    native_databases,
    [
        CopyDatabaseSelection,
        ClearDatabaseSelection,
        SelectDatabasePage
    ]
);
pub enum DatabaseEvent {
    Activated,
    Changed,
    SessionChanged,
}

pub struct DatabaseEditor {
    services: Arc<NativeServices>,
    pub project: ProjectInstanceId,
    pub id: String,
    pub revision: ResourceRevision,
    pub name: String,
    meta: Option<Arc<DatabaseMetaResult>>,
    edit: Option<EditState>,
    grid: Entity<TableState<DatabaseGrid>>,
    selection_preview: Entity<TextareaState>,
    selection_preview_open: bool,
    details: details::DetailsState,
    _grid_subscription: Subscription,
    selection_cursor_sync: Option<TableSelection>,
    generation: u64,
    offset: usize,
    busy: bool,
    mutating: bool,
    ready: bool,
    read_failed: bool,
    refresh_again: bool,
    pub error: Option<String>,
}
impl DatabaseEditor {
    pub fn new(
        services: Arc<NativeServices>,
        project: ProjectInstanceId,
        id: String,
        revision: ResourceRevision,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let grid = cx.new(|cx| {
            TableState::new(DatabaseGrid::empty(), window, cx)
                .cell_selectable(true)
                .row_selectable(true)
                .row_header(false)
                .col_selectable(true)
                .col_movable(false)
                .sortable(false)
                .loop_selection(false)
        });
        let subscription = cx.subscribe_in(&grid, window, |view, _, event, window, cx| {
            view.selection_changed(event, window, cx);
        });
        Self {
            services,
            project,
            id,
            revision,
            name,
            meta: None,
            edit: None,
            grid,
            selection_preview: cx.new(|cx| TextareaState::new(window, cx).rows(5)),
            selection_preview_open: false,
            details: details::DetailsState::default(),
            _grid_subscription: subscription,
            selection_cursor_sync: None,
            generation: 0,
            offset: 0,
            busy: false,
            mutating: false,
            ready: false,
            read_failed: false,
            refresh_again: false,
            error: None,
        }
    }
    pub fn dirty(&self) -> bool {
        self.edit.as_ref().is_some_and(|edit| edit.is_modified)
    }
    pub fn busy(&self) -> bool {
        self.busy || self.mutating
    }
    pub fn read_failed(&self) -> bool {
        self.read_failed
    }
    pub fn fail_read(&mut self, cx: &mut Context<Self>) {
        self.ready = false;
        self.read_failed = true;
        self.error = Some(crate::text::translate("native.databases.readFailed"));
        self.changed(cx);
    }
    pub fn focus_table(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.grid.read(cx).focus_handle(cx), cx);
        cx.emit(DatabaseEvent::Activated);
    }
    fn changed(&self, cx: &mut Context<Self>) {
        cx.emit(DatabaseEvent::Changed);
        cx.notify();
    }
    pub fn install_read(
        &mut self,
        read: query::DatabaseRead,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.name = read.meta.name.clone();
        let meta = Arc::new(read.meta);
        self.details.retain_columns(&meta);
        self.meta = Some(meta.clone());
        self.edit = Some(read.edit);
        self.ready = true;
        self.read_failed = false;
        self.error = None;
        self.offset = read.offset;
        if let Some(page) = read.page {
            self.selection_cursor_sync = None;
            let offset = self.offset;
            self.grid.update(cx, |grid, cx| {
                *grid.delegate_mut() =
                    DatabaseGrid::from_page(page.rows, offset, meta, page.elapsed);
                grid.clear_selection(cx);
                grid.refresh(cx);
            });
        } else {
            self.grid.update(cx, |grid, cx| {
                grid.delegate_mut().schema = Some(meta);
                cx.notify();
            });
        }
        self.update_selection_preview(window, cx);
        self.changed(cx);
    }
    pub fn replace_index(
        &mut self,
        revision: ResourceRevision,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.name = name;
        if self.revision != revision || !self.ready {
            self.revision = revision;
            self.reload(false, window, cx);
        }
        self.changed(cx);
    }
    pub fn unavailable(&mut self, cx: &mut Context<Self>) {
        self.generation = self.generation.wrapping_add(1);
        self.ready = false;
        self.edit = None;
        self.read_failed = true;
        self.busy = false;
        self.error = Some(crate::text::t("native.databases.unavailable").into());
        self.changed(cx);
    }
}
impl EventEmitter<PanelEvent> for DatabaseEditor {}
impl EventEmitter<DatabaseEvent> for DatabaseEditor {}
impl Focusable for DatabaseEditor {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.grid.read(cx).focus_handle(cx)
    }
}
impl BasePanel for DatabaseEditor {
    fn panel_name(&self) -> &'static str {
        "database-editor"
    }
    fn closable(&self, _: &App) -> bool {
        !self.dirty() && !self.busy()
    }
    fn set_active(&mut self, active: bool, _: &mut Window, cx: &mut Context<Self>) {
        if active {
            cx.emit(DatabaseEvent::Activated);
        }
    }
    fn dump(&self, _: &App) -> PanelState {
        let mut state = PanelState::new(self.panel_name());
        state.info = PanelInfo::Panel(serde_json::json!({"databaseId": self.id}));
        state
    }
}
impl Panel for DatabaseEditor {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
        format!("{}{}", self.name, if self.dirty() { " *" } else { "" })
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
