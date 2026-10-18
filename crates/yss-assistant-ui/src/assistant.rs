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

use crate::AssistantServices;
use gpui_kit::component::{
    dock::{BasePanel, Panel, PanelEvent, PanelInfo, PanelState},
    input::{InputEvent, TextareaState},
};
use gpui_kit::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, Subscription, Window,
    actions,
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
pub struct ConversationPanel {
    services: Arc<AssistantServices>,
    session: HarnessSessionRecord,
    principal: PrincipalId,
    focus: FocusHandle,
    input: Entity<TextareaState>,
    _input_subscription: Subscription,
    viewport: thread::Viewport,
    transcript: projection::Transcript,
    catalog: Option<Arc<LanguageModelCatalog>>,
    references: Vec<ProjectResourceRef>,
    resource_catalog: Option<Arc<yss_desktop_services::resources::ResourceCatalog>>,
    options: HarnessTurnOptions,
    preferred_effort: Option<yss_harness_contract::ReasoningEffort>,
    pending: Option<Submission>,
    unsent: Option<DraftMessage>,
    queue: VecDeque<DraftMessage>,
    queue_paused: bool,
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
    event_task: Option<gpui_kit::Task<()>>,
    expanded: BTreeMap<String, bool>,
    error: Option<String>,
    notifications: yss_ui::notifications::Notifications,
    stream_error: Option<String>,
}
impl ConversationPanel {
    pub const PANEL_NAME: &'static str = "assistant-conversation";

    fn set_error(&mut self, error: String, cx: &mut Context<Self>) {
        self.notifications.error(error.clone(), cx);
        self.error = Some(error);
    }

    fn set_stream_error(&mut self, error: String, cx: &mut Context<Self>) {
        if self.stream_error.as_ref() != Some(&error) {
            self.notifications
                .error_with_retry(error.clone(), cx, |view, window, cx| {
                    view.reload(false, window, cx);
                });
        }
        self.stream_error = Some(error);
    }

    pub fn new(
        services: Arc<AssistantServices>,
        session: HarnessSessionRecord,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .auto_grow(2, 12)
                .submit_on_enter(true)
                .placeholder(yss_ui::text::t("native.assistant.prompt"))
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
            options: HarnessTurnOptions {
                mode: match yss_ui::preferences::current(cx).assistant.mode {
                    yss_settings::AssistantMode::Ask => yss_harness_contract::HarnessMode::Ask,
                    yss_settings::AssistantMode::Write => yss_harness_contract::HarnessMode::Write,
                },
                reasoning_effort: None,
            },
            preferred_effort: yss_ui::preferences::current(cx)
                .assistant
                .reasoning_effort
                .map(|effort| match effort {
                    yss_settings::ReasoningEffort::Low => {
                        yss_harness_contract::ReasoningEffort::Low
                    }
                    yss_settings::ReasoningEffort::Medium => {
                        yss_harness_contract::ReasoningEffort::Medium
                    }
                    yss_settings::ReasoningEffort::High => {
                        yss_harness_contract::ReasoningEffort::High
                    }
                }),
            pending: None,
            unsent: None,
            queue: VecDeque::new(),
            queue_paused: true,
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
            notifications: yss_ui::notifications::Notifications::new(window),
            stream_error: None,
        };
        panel.observe_scroll(cx);
        panel.connect(window, cx);
        panel.reload(false, window, cx);
        panel
    }
    pub fn submitting(&self) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|pending| !pending.accepted)
            || self.selecting
    }
    pub fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
    pub fn update_session(&mut self, session: HarnessSessionRecord) {
        self.session = session;
        self.reconcile_effort();
    }
    pub fn session(&self) -> &HarnessSessionRecord {
        &self.session
    }

    pub fn session_id(&self) -> &yss_harness_contract::HarnessSessionId {
        &self.session.id
    }

    pub fn project_identity(&self) -> &yss_project_identity::ProjectInstanceId {
        self.session.project.project_instance_id()
    }

    pub fn update_title(&mut self, title: &str, cx: &mut Context<Self>) -> bool {
        if let Some(metadata) = &mut self.session.conversation
            && metadata.title != title
        {
            metadata.title.clear();
            metadata.title.push_str(title);
            cx.notify();
            true
        } else {
            false
        }
    }
}
pub fn principal() -> PrincipalId {
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
        Self::PANEL_NAME
    }
    fn dump(&self, _: &App) -> PanelState {
        let mut state = PanelState::new(self.panel_name());
        state.info = PanelInfo::panel(serde_json::json!({"sessionId":self.session.id.to_string()}));
        state
    }
}
impl Panel for ConversationPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        self.display_title()
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
