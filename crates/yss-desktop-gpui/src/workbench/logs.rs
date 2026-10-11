//! Native log controls and bounded read projections; LogRuntime owns persisted records.
mod details;
mod entry;
mod filter;
mod render;
mod selection;
mod stream;
mod toolbar;
mod window;

use crate::services::NativeServices;
pub(super) use details::LogDetails;
use entry::LogEntry;
use gpui_kit::assets::IconName;
use gpui_kit::component::{
    Icon,
    dock::{BasePanel, Panel, PanelEvent},
    input::{InputEvent, InputState},
};
use gpui_kit::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    UniformListScrollHandle, Window, div, prelude::*, px,
};
use std::{collections::VecDeque, rc::Rc, sync::Arc};
use yss_logging::{LogDomain, LogLevel};

const VIEW_CAPACITY: usize = 1_000;
const ROW_HEIGHT: f32 = 26.;
const LEVELS: [LogLevel; 5] = [
    LogLevel::Error,
    LogLevel::Warn,
    LogLevel::Info,
    LogLevel::Debug,
    LogLevel::Trace,
];

pub(super) enum LogsEvent {
    Inspect(Entity<LogDetails>),
    Clear(gpui_kit::EntityId),
}

pub struct LogsPanel {
    services: Arc<NativeServices>,
    focus: FocusHandle,
    entries: VecDeque<Rc<LogEntry>>,
    visible: Vec<usize>,
    selected: Option<Entity<LogDetails>>,
    domain: Option<LogDomain>,
    levels: Vec<LogLevel>,
    search: Entity<InputState>,
    query: String,
    view_locale: &'static str,
    scroll: UniformListScrollHandle,
    auto_scroll: bool,
    truncated: bool,
    recovery_attempts: u8,
    stream: String,
    sequence: u64,
    epoch: u64,
    lease: Option<stream::LogLease>,
    task: Option<gpui_kit::Task<()>>,
    connecting: bool,
    error: Option<&'static str>,
    _search_subscription: gpui_kit::Subscription,
    _preferences_subscription: gpui_kit::Subscription,
    preferences: yss_settings::LogSettings,
}

impl LogsPanel {
    pub fn new(services: Arc<NativeServices>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(crate::text::translate("log.searchPlaceholder"))
        });
        let subscription = cx.subscribe(&search, |view, input, event, cx| {
            if matches!(event, InputEvent::Change) {
                let query = input.read(cx).value().trim().to_lowercase();
                if view.query != query {
                    view.query = query;
                    view.refilter();
                    cx.notify();
                }
            }
        });
        let preferences = crate::preferences::current(cx).logs.clone();
        let preferences_subscription =
            cx.observe_global::<crate::preferences::Preferences>(|view, cx| {
                view.apply_preferences(cx)
            });
        let mut panel = Self {
            services,
            focus: cx.focus_handle(),
            entries: VecDeque::new(),
            visible: vec![],
            selected: None,
            domain: None,
            levels: preferences
                .visible_levels
                .iter()
                .copied()
                .map(display_level)
                .collect(),
            search,
            query: String::new(),
            view_locale: crate::text::locale(),
            scroll: UniformListScrollHandle::new(),
            auto_scroll: preferences.auto_scroll,
            truncated: false,
            recovery_attempts: 0,
            stream: String::new(),
            sequence: 0,
            epoch: 0,
            lease: None,
            task: None,
            connecting: false,
            error: None,
            _search_subscription: subscription,
            _preferences_subscription: preferences_subscription,
            preferences,
        };
        panel.connect(window, cx);
        panel
    }

    fn apply_preferences(&mut self, cx: &mut Context<Self>) {
        let preferences = &crate::preferences::current(cx).logs;
        // Toolbar choices remain local until that particular preference changes;
        // saving an unrelated preference must not reset an active investigation.
        let levels_changed = self.preferences.visible_levels != preferences.visible_levels;
        let scroll_changed = self.preferences.auto_scroll != preferences.auto_scroll;
        if levels_changed {
            self.levels = preferences
                .visible_levels
                .iter()
                .copied()
                .map(display_level)
                .collect();
        }
        if scroll_changed {
            self.auto_scroll = preferences.auto_scroll;
        }
        self.preferences = preferences.clone();
        if levels_changed {
            self.refilter();
        }
        if scroll_changed && self.auto_scroll {
            self.scroll.scroll_to_bottom();
        }
        if levels_changed || scroll_changed {
            cx.notify();
        }
    }
}

fn display_level(level: yss_settings::LogLevel) -> LogLevel {
    match level {
        yss_settings::LogLevel::Error => LogLevel::Error,
        yss_settings::LogLevel::Warn => LogLevel::Warn,
        yss_settings::LogLevel::Info => LogLevel::Info,
        yss_settings::LogLevel::Debug => LogLevel::Debug,
        yss_settings::LogLevel::Trace => LogLevel::Trace,
    }
}
impl EventEmitter<PanelEvent> for LogsPanel {}
impl EventEmitter<LogsEvent> for LogsPanel {}
impl Focusable for LogsPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for LogsPanel {
    fn panel_name(&self) -> &'static str {
        "logs"
    }
    fn closable(&self, _: &App) -> bool {
        false
    }
}
impl Panel for LogsPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(Icon::new(IconName::SquareTerminal).size_3())
            .child(crate::text::translate("log.title"))
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
