//! Native conversation views consume durable Harness facts and retain only input state.
mod activity;
mod commands;
mod composer;
mod drafts;
mod execution;
mod header;
mod inspect;
mod markdown;
mod models;
mod options;
mod projection;
mod references;
mod render;
mod resources;
mod sources;
mod stream;
mod tasks;
mod thread;
mod tools;
mod usage;

use crate::services::NativeServices;
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, Subscription, Window,
    actions,
};
use gpui_component::{
    dock::{BasePanel, Panel, PanelEvent, PanelInfo, PanelState},
    input::{InputEvent, TextareaState},
};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};
use yss_harness_contract::{
    HarnessEventEnvelope, HarnessSessionRecord, HarnessTurnOptions, LanguageModelCatalog,
    LanguageModelSelection, PrincipalId, ProjectResourceRef,
};

actions!(native_assistant, [CancelResponse]);

pub(crate) enum ConversationEvent {
    DirectoryChanged,
    OptionsChanged,
    OpenResource(ProjectResourceRef),
    OpenResult(yss_graph_execution::result::ResultReference),
    Settings,
}
#[derive(Clone)]
struct DraftMessage {
    id: uuid::Uuid,
    text: String,
    resources: Vec<ProjectResourceRef>,
    model: Option<LanguageModelSelection>,
    options: HarnessTurnOptions,
}
struct Submission {
    message: DraftMessage,
    after_sequence: u64,
    accepted: bool,
}
pub(crate) struct ConversationPanel {
    services: Arc<NativeServices>,
    pub(crate) session: HarnessSessionRecord,
    principal: PrincipalId,
    focus: FocusHandle,
    input: Entity<TextareaState>,
    _input_subscription: Subscription,
    viewport: thread::Viewport,
    transcript: projection::Transcript,
    catalog: Option<Arc<LanguageModelCatalog>>,
    references: Vec<ProjectResourceRef>,
    resource_catalog: Option<Arc<crate::project::resources::ResourceCatalog>>,
    options: HarnessTurnOptions,
    pending: Option<Submission>,
    unsent: Option<DraftMessage>,
    queue: VecDeque<DraftMessage>,
    queue_paused: bool,
    input_expanded: bool,
    selecting: bool,
    ready: bool,
    refreshing: bool,
    stopping: bool,
    generation: u64,
    selection_generation: u64,
    model_generation: u64,
    recovering: bool,
    buffered: Vec<HarnessEventEnvelope>,
    overflow: bool,
    event_task: Option<gpui::Task<()>>,
    expanded: BTreeMap<String, bool>,
    error: Option<String>,
    stream_error: Option<String>,
}
impl ConversationPanel {
    pub(crate) fn new(
        services: Arc<NativeServices>,
        session: HarnessSessionRecord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(3, 10)
                .submit_on_enter(true)
                .placeholder(crate::text::t("native.assistant.prompt"))
        });
        let subscription = cx.subscribe(&input, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let mut panel = Self {
            services,
            session,
            principal: principal(),
            focus: cx.focus_handle(),
            input,
            _input_subscription: subscription,
            viewport: Default::default(),
            transcript: Default::default(),
            catalog: None,
            references: vec![],
            resource_catalog: None,
            options: Default::default(),
            pending: None,
            unsent: None,
            queue: VecDeque::new(),
            queue_paused: true,
            input_expanded: false,
            selecting: false,
            ready: false,
            refreshing: false,
            stopping: false,
            generation: 0,
            selection_generation: 0,
            model_generation: 0,
            recovering: false,
            buffered: vec![],
            overflow: false,
            event_task: None,
            expanded: BTreeMap::new(),
            error: None,
            stream_error: None,
        };
        panel.connect(window, cx);
        panel.reload(false, window, cx);
        panel
    }
    pub(crate) fn submitting(&self) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|pending| !pending.accepted)
            || self.selecting
    }
    pub(crate) fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.refreshing {
            return;
        }
        self.error = None;
        self.reload(false, window, cx);
    }
    fn running(&self) -> bool {
        self.transcript.running() || self.pending.is_some()
    }
    fn selection(&self) -> Option<LanguageModelSelection> {
        self.session
            .conversation
            .as_ref()
            .and_then(|metadata| metadata.model.clone())
            .or_else(|| {
                self.catalog
                    .as_ref()
                    .and_then(|catalog| catalog.default_model.clone())
            })
    }
    fn can_send(&self) -> bool {
        self.ready && !self.selecting && !self.running() && self.model_available()
    }
    pub(crate) fn update_session(&mut self, session: HarnessSessionRecord) {
        self.session = session;
        self.reconcile_effort();
    }
}
pub(crate) fn principal() -> PrincipalId {
    PrincipalId::try_new("local-user").expect("fixed desktop principal")
}
impl EventEmitter<PanelEvent> for ConversationPanel {}
impl EventEmitter<ConversationEvent> for ConversationPanel {}
impl Focusable for ConversationPanel {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.focus_handle(cx)
    }
}
impl BasePanel for ConversationPanel {
    fn panel_name(&self) -> &'static str {
        "assistant-conversation"
    }
    fn dump(&self, _: &App) -> PanelState {
        let mut state = PanelState::new(self.panel_name());
        state.info = PanelInfo::panel(serde_json::json!({"sessionId":self.session.id.to_string()}));
        state
    }
}
impl Panel for ConversationPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
        self.display_title()
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
