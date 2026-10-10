//! Native plugin management consumes the existing process-wide manager and typed read facts.
mod commands;
mod details;
mod installer;
mod native;
mod query;
mod render;
mod sidebar;
pub(crate) use native::{OpenNativeView, PluginViewPanel};
pub(crate) use sidebar::PluginsSidebar;

use crate::services::NativeServices;
use gpui_kit::component::{
    dock::{BasePanel, Panel, PanelEvent, PanelState},
    input::{InputEvent, InputState},
};
use gpui_kit::{
    App, AppContext, Context, Entity, EventEmitter, FocusHandle, Focusable, Subscription, Window,
};
use std::sync::Arc;
use yss_plugin_runtime::{InstalledPlugin, PluginFailure, PluginManager};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PluginKey {
    id: String,
    digest: String,
    generation: String,
}
pub(crate) enum PluginsEvent {
    OpenDetails,
}
#[derive(Clone, Copy, PartialEq)]
enum DetailTab {
    Overview,
    Tasks,
    Diagnostics,
}
impl PluginKey {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }
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
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder(crate::text::t("native.plugins.search"))
        });
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
        "plugin_busy" => crate::text::t("native.plugins.busy"),
        "plugin_stale_context" => crate::text::t("native.plugins.changed"),
        "plugin_package_changed" => crate::text::t("native.plugins.packageChanged"),
        "plugin_signer_change_requires_approval" => crate::text::t("native.plugins.signerChanged"),
        "plugin_version_content_conflict" => crate::text::t("native.plugins.versionConflict"),
        "plugin_trust_required" => crate::text::t("native.plugins.trustRequired"),
        "plugin_operation_expired" => crate::text::t("native.plugins.confirmationExpired"),
        "plugin_signature_invalid" | "plugin_hash_mismatch" => {
            crate::text::t("native.plugins.verificationFailed")
        }
        "plugin_not_installed" => crate::text::t("native.plugins.removed"),
        "plugin_platform_incompatible" => crate::text::t("native.plugins.wrongPlatform"),
        _ => crate::text::t("native.plugins.operationFailed"),
    };
    format!("{message} ({})", error.code)
}
impl EventEmitter<PanelEvent> for PluginsPanel {}
impl EventEmitter<OpenNativeView> for PluginsPanel {}
impl EventEmitter<PluginsEvent> for PluginsPanel {}
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
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        self.selected_plugin()
            .map(|plugin| plugin.manifest.name.clone())
            .unwrap_or_else(|| crate::text::t("activityBar.plugins").into())
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
