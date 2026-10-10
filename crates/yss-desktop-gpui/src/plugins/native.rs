//! Native controls own drafts; the manager owns sessions, persisted state and tasks.
mod commands;
mod fields;
mod render;

use super::PluginKey;
use crate::services::NativeServices;
use gpui_kit::component::dock::{BasePanel, Panel, PanelEvent, PanelInfo, PanelState};
use gpui_kit::{App, Context, EventEmitter, FocusHandle, Focusable, Window};
use serde_json::{Value, json};
use std::sync::Arc;
use yss_plugin_runtime::{NativeView, PluginFailure, PluginManifest, TaskSnapshot};

pub(crate) struct OpenNativeView {
    pub key: PluginKey,
    pub view: yss_plugin_runtime::PluginView,
}

struct ViewLease {
    services: Arc<NativeServices>,
    session_id: String,
    window: String,
}

impl Drop for ViewLease {
    fn drop(&mut self) {
        let session = self.session_id.clone();
        let window = self.window.clone();
        self.services
            .run(move |services| Ok(services.plugins.detach_view(&session, &window)));
    }
}

struct Attachment {
    lease: ViewLease,
    view: NativeView,
    saved: Value,
    key: PluginKey,
    manifest: PluginManifest,
    title: String,
}

pub(crate) struct PluginViewPanel {
    services: Arc<NativeServices>,
    key: Option<PluginKey>,
    plugin_id: String,
    view_id: String,
    title: String,
    manifest: Option<PluginManifest>,
    focus: FocusHandle,
    lease: Option<ViewLease>,
    model: Option<NativeView>,
    fields: Vec<fields::Field>,
    subscriptions: Vec<gpui_kit::Subscription>,
    form_generation: u64,
    busy: bool,
    closed: bool,
    error: Option<PluginFailure>,
    message: Option<String>,
    task: Option<TaskSnapshot>,
}

impl PluginViewPanel {
    pub(crate) fn new(
        services: Arc<NativeServices>,
        plugin_id: String,
        view_id: String,
        expected: Option<PluginKey>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let binding = format!("{:?}", window.window_handle().window_id());
        cx.defer_in(window, move |view, window, cx| {
            view.attach(binding, window, cx)
        });
        Self {
            services,
            key: expected,
            plugin_id,
            view_id,
            title: crate::text::t("native.plugins.loadingView").into(),
            manifest: None,
            focus: cx.focus_handle(),
            lease: None,
            model: None,
            fields: vec![],
            subscriptions: vec![],
            busy: true,
            closed: false,
            form_generation: 0,
            error: None,
            message: None,
            task: None,
        }
    }

    pub(crate) fn matches(&self, key: &PluginKey) -> bool {
        !self.closed
            && self.plugin_id == key.id
            && self.key.as_ref().is_none_or(|current| current == key)
    }

    pub(crate) fn matches_view(&self, key: &PluginKey, view_id: &str) -> bool {
        self.view_id == view_id && self.matches(key)
    }

    pub(crate) fn close(&mut self) {
        self.closed = true;
        self.lease = None;
    }

    fn allows(&self, method: &str) -> bool {
        self.manifest
            .as_ref()
            .is_some_and(|manifest| manifest.ui_methods.iter().any(|allowed| allowed == method))
    }

    fn attach(&mut self, binding: String, window: &mut Window, cx: &mut Context<Self>) {
        if self.closed {
            return;
        }
        let expected = self.key.clone();
        let plugin_id = self.plugin_id.clone();
        let id = self.view_id.clone();
        let owner = self.services.clone();
        let job = self.services.run(move |services| {
            let result = (|| {
                let plugin = services
                    .plugins
                    .list()?
                    .into_iter()
                    .find(|plugin| plugin.manifest.id == plugin_id)
                    .ok_or_else(|| PluginFailure::new("plugin_not_installed"))?;
                let key = PluginKey::from_plugin(&plugin);
                if expected.as_ref().is_some_and(|expected| *expected != key) {
                    return Err(PluginFailure::new("plugin_stale_context"));
                }
                let declaration = plugin
                    .manifest
                    .contributes
                    .views
                    .iter()
                    .find(|view| view.id == id)
                    .ok_or_else(|| PluginFailure::new("plugin_view_missing"))?;
                let title = declaration.title.clone();
                let session = services.plugins.attach_view(&key.id, &id, &binding)?;
                // Own cleanup before any subsequent fallible read or identity check.
                let lease = ViewLease {
                    services: owner,
                    session_id: session.session_id,
                    window: binding,
                };
                key.validate(&services.plugins)?;
                let saved = if services
                    .plugins
                    .manifest(&key.id)?
                    .ui_methods
                    .iter()
                    .any(|method| method == "views.get_state")
                {
                    services.plugins.call_view(
                        &lease.session_id,
                        &lease.window,
                        "views.get_state",
                        Value::Null,
                    )?
                } else {
                    Value::Null
                };
                Ok::<_, PluginFailure>(Attachment {
                    lease,
                    view: session.view,
                    saved,
                    key,
                    manifest: plugin.manifest,
                    title,
                })
            })();
            Ok(result)
        });
        cx.spawn_in(window, async move |panel, cx| {
            let result = job
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or_else(|| Err(PluginFailure::new("plugin_state_unavailable")));
            let _ = panel.update_in(cx, |panel, window, cx| {
                if panel.closed {
                    return;
                }
                panel.busy = false;
                match result {
                    Ok(attachment) => {
                        panel.key = Some(attachment.key);
                        panel.manifest = Some(attachment.manifest);
                        panel.title = attachment.title;
                        if let Err(error) =
                            panel.install_form(attachment.view, attachment.saved, window, cx)
                        {
                            panel.error = Some(error);
                        } else {
                            panel.lease = Some(attachment.lease);
                        }
                    }
                    Err(error) => panel.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl EventEmitter<PanelEvent> for PluginViewPanel {}
impl EventEmitter<OpenNativeView> for PluginViewPanel {}
impl Focusable for PluginViewPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl BasePanel for PluginViewPanel {
    fn panel_name(&self) -> &'static str {
        "plugin-view"
    }
    fn dump(&self, _: &App) -> PanelState {
        let mut state = PanelState::new(self.panel_name());
        state.info = PanelInfo::Panel(json!({"pluginId":self.plugin_id,"viewId":self.view_id}));
        state
    }
    fn on_removed(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.close();
    }
}
impl Panel for PluginViewPanel {
    fn title(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl gpui_kit::IntoElement {
        self.title.clone()
    }
    fn inner_padding(&self, _: &App) -> bool {
        false
    }
}
