//! Native data pages consume revision-bound Application queries; Rust owns all dataset edits.
mod commands;
mod details;
mod grid;
pub mod query;
mod render;
mod selection;
mod semantic;
mod toolbar;

pub use commands::{DatabaseSaveOutcome, DatabaseSaveRequest};
pub use details::DetailsDisplay;
use gpui_kit::component::{
    dock::{BasePanel, Panel, PanelEvent, PanelInfo, PanelState},
    input::TextareaState,
    table::{TableSelection, TableState},
};
use gpui_kit::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, Subscription, Window,
    actions,
};
use grid::DatabaseGrid;
use std::sync::Arc;
use yss_application::database::DatabaseMetaResult;
use yss_database_contract::EditState;
use yss_desktop_services::ApplicationTasks;
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
    SessionChanged,
}

pub struct DatabaseEditor {
    services: Arc<ApplicationTasks>,
    project: ProjectInstanceId,
    id: String,
    revision: ResourceRevision,
    name: String,
    meta: Option<Arc<DatabaseMetaResult>>,
    edit: Option<EditState>,
    grid: Entity<TableState<DatabaseGrid>>,
    selection_preview: Entity<TextareaState>,
    selection_preview_open: bool,
    details: details::DetailsState,
    _grid_subscription: Subscription,
    _preferences_subscription: Subscription,
    selection_cursor_sync: Option<TableSelection>,
    generation: u64,
    offset: usize,
    page_size: usize,
    page_epoch: u64,
    busy: bool,
    mutating: bool,
    ready: bool,
    read_failed: bool,
    refresh_again: bool,
    error: Option<String>,
    notifications: yss_ui::notifications::Notifications,
}
impl DatabaseEditor {
    fn set_error(&mut self, error: Option<String>, cx: &mut gpui_kit::Context<Self>) {
        if let Some(error) = &error {
            self.notifications.error(error.clone(), cx);
        }
        self.error = error;
    }

    pub fn new(
        services: Arc<ApplicationTasks>,
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
        let preferences_subscription =
            cx.observe_global_in::<yss_ui::preferences::Preferences>(window, |view, window, cx| {
                view.preferences_changed(window, cx);
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
            _preferences_subscription: preferences_subscription,
            selection_cursor_sync: None,
            generation: 0,
            offset: 0,
            page_size: yss_ui::preferences::current(cx).tables.page_size,
            page_epoch: 0,
            busy: false,
            mutating: false,
            ready: false,
            read_failed: false,
            refresh_again: false,
            error: None,
            notifications: yss_ui::notifications::Notifications::new(window),
        }
    }
    pub fn project(&self) -> &ProjectInstanceId {
        &self.project
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn revision(&self) -> ResourceRevision {
        self.revision
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
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
        let message = yss_ui::text::translate("native.databases.readFailed");
        self.notifications
            .error_with_retry(message.clone(), cx, |view, window, cx| {
                view.reload(true, window, cx)
            });
        self.error = Some(message);
        self.changed(cx);
    }
    pub fn focus_table(&self, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.grid.read(cx).focus_handle(cx), cx);
        cx.emit(DatabaseEvent::Activated);
    }
    fn changed(&self, cx: &mut Context<Self>) {
        cx.notify();
    }
    pub fn install_read(
        &mut self,
        read: query::DatabaseRead,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if read.page_size != self.page_size {
            self.reload(true, window, cx);
            return;
        }
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
        self.set_error(
            Some(yss_ui::text::t("native.databases.unavailable").into()),
            cx,
        );
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
impl DatabaseEditor {
    /// The dock factory that restores database editors.
    pub const PANEL_NAME: &'static str = "database-editor";
    /// The persisted panel field holding the database ID.
    pub const ID_KEY: &'static str = "databaseId";
}
impl BasePanel for DatabaseEditor {
    fn panel_name(&self) -> &'static str {
        Self::PANEL_NAME
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
        state.info = PanelInfo::Panel(serde_json::json!({ (Self::ID_KEY): self.id }));
        state
    }
}
impl Panel for DatabaseEditor {
    fn tab_name(&self, _: &App) -> Option<gpui_kit::SharedString> {
        Some(format!("{}{}", self.name, if self.dirty() { " *" } else { "" }).into())
    }
    fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        self.tab_name(cx).unwrap_or_default()
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
