//! Native plugin management consumes the existing process-wide manager and typed read facts.
mod commands;
mod details;
mod installer;
mod query;
mod render;

use crate::services::NativeServices;
use gpui::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, Subscription, Window,
};
use gpui_component::{
    dock::{BasePanel, Panel, PanelEvent, PanelState},
    input::{InputEvent, InputState},
};
use std::sync::Arc;
use yss_plugin_runtime::{InstalledPlugin, PluginFailure, PluginManager};

#[derive(Clone, PartialEq, Eq)]
struct PluginKey {
    id: String,
    digest: String,
    generation: String,
}
#[derive(Clone, Copy, PartialEq)]
enum DetailTab {
    Overview,
    Tasks,
    Diagnostics,
}
impl PluginKey {
    fn from_plugin(plugin: &InstalledPlugin) -> Self {
        Self {
            id: plugin.manifest.id.clone(),
            digest: plugin.package_digest.clone(),
            generation: plugin.installation_generation.clone(),
        }
    }
    fn validate(&self, manager: &PluginManager) -> Result<(), PluginFailure> {
        if manager
            .list()?
            .iter()
            .any(|plugin| Self::from_plugin(plugin) == *self)
        {
            Ok(())
        } else {
            Err(PluginFailure::new("plugin_stale_context"))
        }
    }
}
pub(crate) struct PluginsPanel {
    services: Arc<NativeServices>,
    focus: FocusHandle,
    entries: Vec<InstalledPlugin>,
    selected: Option<PluginKey>,
    detail: Option<query::PluginDetail>,
    tab: DetailTab,
    search: Entity<InputState>,
    _search_subscription: Subscription,
    generation: u64,
    task: Option<&'static str>,
    reload_again: bool,
    error: Option<String>,
    feedback: Option<String>,
    cursor_stack: Vec<Option<String>>,
}
impl PluginsPanel {
    pub(crate) fn new(
        services: Arc<NativeServices>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("搜索插件名称或发布者"));
        let subscription = cx.subscribe(&search, |_, _, event, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        Self {
            services,
            focus: cx.focus_handle(),
            entries: vec![],
            selected: None,
            detail: None,
            tab: DetailTab::Overview,
            search,
            _search_subscription: subscription,
            generation: 0,
            task: None,
            reload_again: false,
            error: None,
            feedback: None,
            cursor_stack: vec![],
        }
    }
    pub(crate) fn busy(&self) -> bool {
        self.task.is_some()
    }
    fn selected_plugin(&self) -> Option<&InstalledPlugin> {
        self.entries
            .iter()
            .find(|plugin| self.selected.as_ref() == Some(&PluginKey::from_plugin(plugin)))
    }
    fn changed(&self, cx: &mut Context<Self>) {
        cx.notify();
    }
    fn install_entries(&mut self, entries: Vec<InstalledPlugin>) {
        self.entries = entries;
        if let Some(selected) = &self.selected {
            let id = selected.id.clone();
            let next = self
                .entries
                .iter()
                .find(|plugin| plugin.manifest.id == id)
                .map(PluginKey::from_plugin);
            if next != self.selected {
                self.detail = None;
                self.cursor_stack.clear();
            }
            self.selected = next;
        }
    }
}
fn failure(error: &PluginFailure) -> String {
    let message = match error.code.as_str() {
        "plugin_busy" => "插件正在执行任务或被其他视图使用，请完成后重试。",
        "plugin_stale_context" => "插件已更新或移除，请重新选择。",
        "plugin_package_changed" => "安装包已变化，请重新检查后确认。",
        "plugin_signer_change_requires_approval" => "签名者已变化，请重新检查并确认签名变更。",
        "plugin_version_content_conflict" => "相同发布版本的内容不同，请使用新的插件版本。",
        "plugin_trust_required" => "安装前请确认允许运行此原生插件。",
        "plugin_operation_expired" => "此次安装确认已过期，请重新检查安装包。",
        "plugin_signature_invalid" | "plugin_hash_mismatch" => "安装包签名或内容校验未通过。",
        "plugin_not_installed" => "插件已移除，请刷新列表。",
        "plugin_platform_incompatible" => "安装包不适用于当前平台，请选择当前系统和架构的安装包。",
        _ => "插件操作未完成，请检查安装包或插件存储后重试。",
    };
    format!("{message} ({})", error.code)
}
impl EventEmitter<PanelEvent> for PluginsPanel {}
impl Focusable for PluginsPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for PluginsPanel {
    fn panel_name(&self) -> &'static str {
        "plugins"
    }
    fn closable(&self, _: &App) -> bool {
        !self.busy()
    }
    fn dump(&self, _: &App) -> PanelState {
        PanelState::new(self.panel_name())
    }
}
impl Panel for PluginsPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
        "插件"
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
