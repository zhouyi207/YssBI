//! Retains conversation entities across Dock closes; project epochs reject late async receipts.
mod deletion;
mod renaming;

use crate::{
    AssistantServices, ConversationPanel,
    assistant::{ConversationEvent, principal},
};
use gpui_kit::{App, AppContext, Context, Entity, EventEmitter, Subscription, Window};
use std::{collections::BTreeMap, sync::Arc};
use yss_application::activity_panel::{ActivityItem, ActivityPanelDocument, ActivityRowContent};
use yss_desktop_services::resources::ResourceCatalog;
use yss_harness_contract::{HarnessSessionId, HarnessSessionRecord, ProjectResourceRef};
use yss_project_identity::ProjectInstanceId;
use yss_ui::lifecycle::LogReleased;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DirectoryReadState {
    Loading,
    Ready,
    Failed(&'static str),
}

/// Semantic requests; only the shell decides Dock visibility and completes UI intents.
pub enum AssistantEventKind {
    DirectoryChanged,
    PresentConversation(Entity<ConversationPanel>),
    RemoveConversation(Entity<ConversationPanel>),
    PresentDirectory,
    MetadataChanged,
    OpenResource(ProjectResourceRef),
    OpenResult(yss_graph_execution::result::ResultReference),
    Settings,
    Error(Option<String>),
}

pub struct AssistantEvent {
    project: Option<ProjectInstanceId>,
    kind: AssistantEventKind,
}

impl AssistantEvent {
    pub fn project(&self) -> Option<&ProjectInstanceId> {
        self.project.as_ref()
    }
    pub fn kind(&self) -> &AssistantEventKind {
        &self.kind
    }
}

/// Owns cached views and directory operations, never the Dock's visibility or active selection.
pub struct AssistantController {
    services: Arc<AssistantServices>,
    project: Option<ProjectInstanceId>,
    epoch: u64,
    blocked: Box<dyn Fn(&App) -> bool>,
    resource_catalog: Option<Arc<ResourceCatalog>>,
    conversations: BTreeMap<String, Entity<ConversationPanel>>,
    subscriptions: BTreeMap<String, Vec<Subscription>>,
    directory: Option<Arc<ActivityPanelDocument>>,
    read_state: DirectoryReadState,
    generation: u64,
    reading: bool,
    again: bool,
    busy: bool,
    reopen: bool,
    last_closed: Option<String>,
}

impl EventEmitter<AssistantEvent> for AssistantController {}

impl AssistantController {
    /// `blocked` reads live shell lifecycle guards without reading this controller recursively.
    /// It is called from deferred completion/modal callbacks; shell command adapters guard entry.
    pub fn new(services: Arc<AssistantServices>, blocked: impl Fn(&App) -> bool + 'static) -> Self {
        Self {
            services,
            project: None,
            epoch: 0,
            blocked: Box::new(blocked),
            resource_catalog: None,
            conversations: BTreeMap::new(),
            subscriptions: BTreeMap::new(),
            directory: None,
            read_state: DirectoryReadState::Loading,
            generation: 0,
            reading: false,
            again: false,
            busy: false,
            reopen: false,
            last_closed: None,
        }
    }
    pub fn is_busy(&self) -> bool {
        self.busy
    }
    pub fn is_reopening(&self) -> bool {
        self.reopen
    }
    pub fn read_state(&self) -> DirectoryReadState {
        self.read_state
    }
    pub fn directory(&self) -> Option<&Arc<ActivityPanelDocument>> {
        self.directory.as_ref()
    }
    pub fn conversation(&self, id: &str) -> Option<&Entity<ConversationPanel>> {
        self.conversations.get(id)
    }
    pub fn has_pending_submission(&self, cx: &App) -> bool {
        self.conversations
            .values()
            .any(|panel| panel.read(cx).submitting())
    }

    fn unavailable(&self, cx: &App) -> bool {
        self.busy || self.has_pending_submission(cx) || (self.blocked)(cx)
    }

    fn emit(&self, kind: AssistantEventKind, cx: &mut Context<Self>) {
        cx.emit(AssistantEvent {
            project: self.project.clone(),
            kind,
        });
    }

    /// Invalidates captured operations without discarding drafts if project activation fails.
    pub fn begin_project_transition(&mut self, cx: &mut Context<Self>) {
        self.epoch = self.epoch.wrapping_add(1);
        self.generation = self.generation.wrapping_add(1);
        self.reading = false;
        self.again = false;
        self.busy = false;
        self.reopen = false;
        self.last_closed = None;
        cx.notify();
    }

    /// A committed project identity change releases the old project's strong views.
    pub fn reset(&mut self, project: Option<ProjectInstanceId>, cx: &mut Context<Self>) {
        self.epoch = self.epoch.wrapping_add(1);
        self.generation = self.generation.wrapping_add(1);
        self.project = project;
        self.resource_catalog = None;
        self.subscriptions.clear();
        self.conversations.clear();
        self.directory = None;
        self.read_state = DirectoryReadState::Loading;
        self.reading = false;
        self.again = false;
        self.busy = false;
        self.reopen = false;
        self.last_closed = None;
        cx.notify();
    }

    pub fn set_resource_catalog(
        &mut self,
        catalog: Option<Arc<ResourceCatalog>>,
        cx: &mut Context<Self>,
    ) {
        self.resource_catalog =
            catalog.filter(|catalog| Some(catalog.project()) == self.project.as_ref());
        for panel in self.conversations.values() {
            panel.update(cx, |panel, cx| {
                panel.set_resource_catalog(self.resource_catalog.clone(), cx)
            });
        }
    }

    pub fn reload_conversations(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for panel in self.conversations.values() {
            panel.update(cx, |panel, cx| panel.reload(false, window, cx));
        }
    }

    pub fn refresh_directory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.reading {
            self.again = true;
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let epoch = self.epoch;
        self.reading = true;
        self.read_state = DirectoryReadState::Loading;
        self.emit(AssistantEventKind::DirectoryChanged, cx);
        let project = self.project.clone();
        let services = self.services.clone();
        let job = self.services.tasks.executor().spawn(async move {
            services
                .tasks
                .application()
                .assistant_activity_panel(&services.host, &principal(), project)
                .await
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            view.update_in(cx, |view, window, cx| {
                if view.epoch != epoch || view.generation != generation {
                    return;
                }
                view.reading = false;
                if let Some(document) = result {
                    view.resume_conversation(&document, window, cx);
                    view.install_document(document, cx);
                } else {
                    view.read_state =
                        DirectoryReadState::Failed("native.workbench.assistantDirectoryFailed");
                    yss_ui::notifications::Notifications::new(window).error_with_retry(
                        yss_ui::text::t("native.workbench.assistantDirectoryFailed"),
                        cx,
                        move |view, window, cx| {
                            if view.epoch == epoch {
                                view.refresh_directory(window, cx);
                            }
                        },
                    );
                    view.emit(AssistantEventKind::DirectoryChanged, cx);
                    if std::mem::take(&mut view.reopen) && !view.unavailable(cx) {
                        view.emit(AssistantEventKind::PresentDirectory, cx);
                    }
                    tracing::warn!(
                        code = "native_assistant_directory_failed",
                        "Native conversation directory refresh failed"
                    );
                }
                if std::mem::take(&mut view.again) {
                    view.refresh_directory(window, cx);
                }
                cx.notify();
            })
            .log_released();
        })
        .detach();
        cx.notify();
    }

    pub fn invalidate_directory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reading = false;
        self.again = false;
        self.refresh_directory(window, cx);
    }

    fn install_document(&mut self, document: ActivityPanelDocument, cx: &mut Context<Self>) {
        let mut titles_changed = false;
        for row in &document.rows {
            if let ActivityRowContent::Item(ActivityItem::Conversation {
                session_id, title, ..
            }) = &row.content
                && let Some(panel) = self.conversations.get(session_id)
            {
                titles_changed |= panel.update(cx, |panel, cx| panel.update_title(title, cx));
            }
        }
        self.directory = Some(Arc::new(document));
        self.read_state = DirectoryReadState::Ready;
        if titles_changed {
            self.emit(AssistantEventKind::MetadataChanged, cx);
        }
        self.emit(AssistantEventKind::DirectoryChanged, cx);
    }

    /// Restored snapshots supersede pending queries, including a query started before restoration.
    pub fn install_restored_directory(
        &mut self,
        document: ActivityPanelDocument,
        cx: &mut Context<Self>,
    ) {
        if document.project_instance_id.as_deref() != self.project.as_ref().map(|id| id.as_str()) {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        self.reading = false;
        self.again = false;
        self.install_document(document, cx);
        cx.notify();
    }

    pub fn new_conversation(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.has_pending_submission(cx) {
            return;
        }
        let reveal_directory_on_failure = std::mem::take(&mut self.reopen);
        self.busy = true;
        let epoch = self.epoch;
        let services = self.services.clone();
        let job = self.services.tasks.executor().spawn(async move {
            services
                .tasks
                .application()
                .create_harness_session(&services.host, principal())
                .await
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            view.update_in(cx, |view, window, cx| {
                if view.epoch != epoch {
                    return;
                }
                view.busy = false;
                if let Some(session) = result {
                    view.install_conversation(session, window, cx);
                } else {
                    view.emit(
                        AssistantEventKind::Error(Some(
                            yss_ui::text::t("native.workbench.newConversationUnconfirmed").into(),
                        )),
                        cx,
                    );
                    if reveal_directory_on_failure {
                        view.emit(AssistantEventKind::PresentDirectory, cx);
                    }
                }
                view.invalidate_directory(window, cx);
                cx.notify();
            })
            .log_released();
        })
        .detach();
        cx.notify();
    }

    pub fn open_conversation(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let Ok(session_id) = HarnessSessionId::try_new(id) else {
            return;
        };
        let reveal_directory_on_failure = std::mem::take(&mut self.reopen);
        self.busy = true;
        let epoch = self.epoch;
        let services = self.services.clone();
        let job = self.services.tasks.executor().spawn(async move {
            services
                .tasks
                .application()
                .open_harness_session(&services.host, &principal(), &session_id)
                .await
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            view.update_in(cx, |view, window, cx| {
                if view.epoch != epoch {
                    return;
                }
                view.busy = false;
                if let Some(session) = result {
                    view.install_conversation(session, window, cx);
                } else {
                    view.emit(
                        AssistantEventKind::Error(Some(
                            yss_ui::text::t("native.workbench.conversationUnavailable").into(),
                        )),
                        cx,
                    );
                    if reveal_directory_on_failure {
                        view.emit(AssistantEventKind::PresentDirectory, cx);
                    }
                }
                view.invalidate_directory(window, cx);
                cx.notify();
            })
            .log_released();
        })
        .detach();
        cx.notify();
    }

    /// Returns the same strong entity for repeated opens, retaining drafts, queues and subscriptions.
    pub fn retain_conversation(
        &mut self,
        session: HarnessSessionRecord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<ConversationPanel> {
        let id = session.id.to_string();
        if let Some(panel) = self.conversations.get(&id) {
            panel.update(cx, |panel, cx| {
                panel.update_session(session);
                cx.notify();
            });
            return panel.clone();
        }
        let panel = cx.new(|cx| ConversationPanel::new(self.services.clone(), session, window, cx));
        panel.update(cx, |panel, cx| {
            panel.set_resource_catalog(self.resource_catalog.clone(), cx)
        });
        let event_subscription =
            cx.subscribe_in(&panel, window, |view, panel, event, window, cx| {
                let project = Some(panel.read(cx).project_identity().clone());
                match event {
                    ConversationEvent::DirectoryChanged => view.refresh_directory(window, cx),
                    ConversationEvent::OptionsChanged => {
                        view.emit(AssistantEventKind::MetadataChanged, cx)
                    }
                    ConversationEvent::Settings => view.emit(AssistantEventKind::Settings, cx),
                    ConversationEvent::OpenResource(resource) => cx.emit(AssistantEvent {
                        project,
                        kind: AssistantEventKind::OpenResource(resource.clone()),
                    }),
                    ConversationEvent::OpenResult(reference) => cx.emit(AssistantEvent {
                        project,
                        kind: AssistantEventKind::OpenResult(*reference),
                    }),
                }
            });
        let observation = cx.observe(&panel, |_, _, cx| cx.notify());
        self.subscriptions
            .insert(id.clone(), vec![event_subscription, observation]);
        self.conversations.insert(id, panel.clone());
        panel
    }

    pub fn install_conversation(
        &mut self,
        session: HarnessSessionRecord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.last_closed = None;
        let panel = self.retain_conversation(session, window, cx);
        self.emit(AssistantEventKind::PresentConversation(panel), cx);
    }

    pub fn note_closed(&mut self, id: Option<String>, cx: &mut Context<Self>) {
        self.reopen = false;
        if id.is_some() {
            self.last_closed = id;
        }
        cx.notify();
    }

    pub fn request_reopen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.has_pending_submission(cx) || self.reopen {
            return;
        }
        self.reopen = true;
        // A read started before the click must not choose an obsolete reopening target.
        self.invalidate_directory(window, cx);
    }

    fn resume_conversation(
        &mut self,
        directory: &ActivityPanelDocument,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.reopen {
            return;
        }
        if self.unavailable(cx) {
            self.reopen = false;
            return;
        }
        let mut sessions = directory.rows.iter().filter_map(|row| match &row.content {
            ActivityRowContent::Item(ActivityItem::Conversation { session_id, .. }) => {
                Some(session_id)
            }
            _ => None,
        });
        let recent = sessions.next();
        let previous = self.last_closed.as_ref().and_then(|previous| {
            recent
                .into_iter()
                .chain(sessions)
                .find(|id| *id == previous)
        });
        if let Some(id) = previous.or(recent) {
            self.open_conversation(id.clone(), window, cx);
        } else {
            self.new_conversation(window, cx);
        }
    }
}
