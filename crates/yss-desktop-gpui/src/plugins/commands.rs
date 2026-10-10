//! Mutations capture the selected installation; completion rereads manager-owned facts.
use super::{PluginKey, PluginsPanel, failure};
use gpui::{Context, Window};
#[derive(Clone, Copy)]
pub(super) enum PluginAction {
    Enable(bool),
    Uninstall,
    ClearCache,
    ClearHistory,
    CollectPackages,
}
impl PluginAction {
    fn confirmation(self) -> Option<(&'static str, &'static str)> {
        match self {
            Self::Uninstall => Some((
                crate::text::t("native.plugins.uninstallTitle"),
                crate::text::t("native.plugins.uninstallMessage"),
            )),
            Self::ClearCache => Some((
                crate::text::t("native.plugins.clearCacheTitle"),
                crate::text::t("native.plugins.clearCacheMessage"),
            )),
            Self::ClearHistory => Some((
                crate::text::t("native.plugins.clearResultsTitle"),
                crate::text::t("native.plugins.clearResultsMessage"),
            )),
            Self::CollectPackages => Some((
                crate::text::t("native.plugins.collectPackagesTitle"),
                crate::text::t("native.plugins.collectPackagesMessage"),
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
            let prompt = crate::modal_window::prompt(
                title,
                Some(message),
                &[
                    crate::text::t("native.plugins.confirm"),
                    crate::text::t("common.cancel"),
                ],
                window,
                cx,
            );
            cx.spawn_in(window, async move |view, cx| {
                if matches!(prompt.await, Ok(0)) {
                    let _ = view.update_in(cx, |view, window, cx| {
                        if view.generation == generation {
                            view.mutate(key, action, window, cx);
                        }
                    });
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
        self.task = Some(crate::text::t("native.plugins.updating"));
        self.error = None;
        self.feedback = None;
        let job = self.services.run(move |services| {
            let manager = &services.plugins;
            let result = (|| {
                if let Some(key) = &key {
                    key.validate(manager)?;
                }
                if matches!(action, PluginAction::CollectPackages) {
                    return manager.collect_garbage().map(|count| {
                        crate::text::format(
                            "native.plugins.packagesCollected",
                            &[("count", count.to_string())],
                        )
                    });
                }
                let key = key.ok_or_else(|| {
                    yss_plugin_runtime::PluginFailure::new("plugin_stale_context")
                })?;
                match action {
                    PluginAction::Enable(enabled) => {
                        manager.set_enabled(&key.id, enabled).map(|_| {
                            if enabled {
                                crate::text::t("native.plugins.enabledFeedback")
                            } else {
                                crate::text::t("native.plugins.disabledFeedback")
                            }
                            .into()
                        })
                    }
                    PluginAction::Uninstall => manager
                        .uninstall(&key.id)
                        .map(|_| crate::text::t("native.plugins.uninstalled").into()),
                    PluginAction::ClearCache => manager
                        .clear_private_cache(&key.id)
                        .map(|_| crate::text::t("native.plugins.cacheCleared").into()),
                    PluginAction::ClearHistory => manager
                        .clear_task_history(&key.id)
                        .map(|_| crate::text::t("native.plugins.resultsCleared").into()),
                    PluginAction::CollectPackages => unreachable!(),
                }
            })();
            Ok((result, manager.list()))
        });
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.generation != generation {
                    return;
                }
                view.task = None;
                if let Some((result, entries)) = result {
                    match entries {
                        Ok(entries) => view.install_entries(entries),
                        Err(error) => view.error = Some(failure(&error)),
                    }
                    match result {
                        Ok(feedback) => view.feedback = Some(feedback),
                        Err(error) => view.error = Some(failure(&error)),
                    }
                } else {
                    view.error = Some(crate::text::t("native.plugins.taskFailed").into());
                }
                view.cursor_stack = vec![None];
                if view.reload_again {
                    view.reload_again = false;
                    view.reload(window, cx);
                } else if let Some(key) = view.selected.clone() {
                    view.read_details(key, None, window, cx);
                }
                view.changed(cx);
            });
        })
        .detach();
        self.changed(cx);
    }
}
