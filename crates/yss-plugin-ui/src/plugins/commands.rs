//! Mutations capture the selected installation; completion rereads manager-owned facts.
use super::{PluginKey, PluginsPanel, failure};
use gpui_kit::component::{WindowExt, notification::Notification};
use gpui_kit::{Context, Window};
use yss_ui::lifecycle::LogReleased;
#[derive(Clone, Copy)]
pub(super) enum PluginAction {
    Enable(bool),
    Uninstall,
    ClearCache,
    ClearHistory,
    CollectPackages,
}
impl PluginAction {
    fn confirmation(
        self,
    ) -> Option<(
        std::borrow::Cow<'static, str>,
        std::borrow::Cow<'static, str>,
    )> {
        match self {
            Self::Uninstall => Some((
                yss_ui::text::t("native.plugins.uninstallTitle"),
                yss_ui::text::t("native.plugins.uninstallMessage"),
            )),
            Self::ClearCache => Some((
                yss_ui::text::t("native.plugins.clearCacheTitle"),
                yss_ui::text::t("native.plugins.clearCacheMessage"),
            )),
            Self::ClearHistory => Some((
                yss_ui::text::t("native.plugins.clearResultsTitle"),
                yss_ui::text::t("native.plugins.clearResultsMessage"),
            )),
            Self::CollectPackages => Some((
                yss_ui::text::t("native.plugins.collectPackagesTitle"),
                yss_ui::text::t("native.plugins.collectPackagesMessage"),
            )),
            Self::Enable(_) => None,
        }
    }
}
impl PluginsPanel {
    pub(super) fn request_action(
        &mut self,
        key: Option<PluginKey>,
        action: PluginAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() {
            return;
        }
        if let Some((title, message)) = action.confirmation() {
            let generation = self.generation;
            let prompt = yss_ui::modal_window::prompt(
                &title,
                Some(&message),
                &[
                    &yss_ui::text::t("native.plugins.confirm"),
                    &yss_ui::text::t("common.cancel"),
                ],
                window,
                cx,
            );
            cx.spawn_in(window, async move |view, cx| {
                if matches!(prompt.await, Ok(0)) {
                    view.update_in(cx, |view, window, cx| {
                        if view.generation == generation {
                            view.mutate(key, action, window, cx);
                        }
                    })
                    .log_released();
                }
            })
            .detach();
        } else {
            self.mutate(key, action, window, cx);
        }
    }
    fn mutate(
        &mut self,
        key: Option<PluginKey>,
        action: PluginAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.busy = true;
        let job = self.services.run(move |services| {
            let manager = &services.plugins;
            let result = (|| {
                if let Some(key) = &key {
                    key.validate(manager)?;
                }
                if matches!(action, PluginAction::CollectPackages) {
                    return manager.collect_garbage().map(|_| ());
                }
                let key = key.ok_or_else(|| {
                    yss_plugin_runtime::PluginFailure::new("plugin_stale_context")
                })?;
                match action {
                    PluginAction::Enable(enabled) => manager.set_enabled(&key.id, enabled),
                    PluginAction::Uninstall => manager.uninstall(&key.id),
                    PluginAction::ClearCache => manager.clear_private_cache(&key.id).map(|_| ()),
                    PluginAction::ClearHistory => manager.clear_task_history(&key.id),
                    PluginAction::CollectPackages => unreachable!(),
                }
            })();
            Ok((result, manager.list()))
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            view.update_in(cx, |view, window, cx| {
                if view.generation != generation {
                    return;
                }
                view.busy = false;
                let error = if let Some((result, entries)) = result {
                    let refresh_error = match entries {
                        Ok(entries) => {
                            view.install_entries(entries);
                            None
                        }
                        Err(error) => Some(error),
                    };
                    result.err().or(refresh_error).map(|error| failure(&error))
                } else {
                    Some(yss_ui::text::t("native.plugins.taskFailed").into())
                };
                if let Some(error) = error {
                    window.push_notification(Notification::error(error), cx);
                }
                view.cursor_stack = vec![None];
                if view.reload_again {
                    view.reload_again = false;
                    view.reload(window, cx);
                } else if let Some(key) = view.selected.clone() {
                    view.read_details(key, None, window, cx);
                }
                view.changed(cx);
            })
            .log_released();
        })
        .detach();
        self.changed(cx);
    }
}
