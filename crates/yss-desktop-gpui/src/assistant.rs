//! Native conversation views consume durable Harness facts and retain only input state.
mod commands;
mod composer;
mod header;
mod inspect;
mod projection;
mod render;
mod stream;
mod thread;

use crate::services::NativeServices;
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, Subscription, Window,
    actions,
};
use gpui_component::{
    dock::{BasePanel, Panel, PanelEvent, PanelInfo, PanelState},
    input::{InputEvent, InputState, TextareaState},
};
use std::{
    collections::{BTreeSet, VecDeque},
    sync::Arc,
};
use yss_harness_contract::{
    HarnessEventEnvelope, HarnessSessionRecord, HarnessTurnOptions, LanguageModelCatalog,
    LanguageModelSelection, PrincipalId, ProjectResourceRef,
};

actions!(native_assistant, [SendMessage, CancelResponse]);

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
    resource_search: Entity<InputState>,
    _search_subscription: Subscription,
    reference_picker: bool,
    resource_generation: u64,
    viewport: thread::Viewport,
    transcript: projection::Transcript,
    catalog: Option<Arc<LanguageModelCatalog>>,
    references: Vec<ProjectResourceRef>,
    resource_choices: Vec<yss_harness_contract::HarnessResourceReference>,
    options: HarnessTurnOptions,
    pending: Option<Submission>,
    unsent: Option<DraftMessage>,
    queue: VecDeque<DraftMessage>,
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
    expanded: BTreeSet<String>,
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
                .placeholder("向 YssBI 提问，或描述要完成的分析…")
        });
        let subscription = cx.subscribe(&input, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        let resource_search =
            cx.new(|cx| InputState::new(window, cx).placeholder("搜索资源名称或路径"));
        let search_subscription = cx.subscribe(&resource_search, |_, _, event, cx| {
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
            resource_search,
            _search_subscription: search_subscription,
            reference_picker: false,
            resource_generation: 0,
            viewport: Default::default(),
            transcript: Default::default(),
            catalog: None,
            references: vec![],
            resource_choices: vec![],
            options: Default::default(),
            pending: None,
            unsent: None,
            queue: VecDeque::new(),
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
            expanded: BTreeSet::new(),
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
    fn model_available(&self) -> bool {
        let Some(selected) = self.selection() else {
            return false;
        };
        self.catalog.as_ref().is_some_and(|catalog| {
            catalog.providers.iter().any(|provider| {
                provider.config.id == selected.provider_id
                    && (provider.has_api_key
                        || provider.config.authentication
                            == yss_harness_contract::LanguageModelAuthentication::None)
                    && provider
                        .config
                        .models
                        .iter()
                        .any(|model| model.id == selected.model_id)
            })
        })
    }
}
pub(crate) fn principal() -> PrincipalId {
    PrincipalId::try_new("local-user").expect("fixed desktop principal")
}
impl EventEmitter<PanelEvent> for ConversationPanel {}
impl EventEmitter<ConversationEvent> for ConversationPanel {}
impl Focusable for ConversationPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
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
