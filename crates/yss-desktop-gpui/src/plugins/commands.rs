//! Mutations capture the selected installation; completion rereads manager-owned facts.
use super::{PluginKey, PluginsPanel, failure};
use gpui::{Context, PromptLevel, Window};
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
                "卸载插件？",
                "插件包登记将移除；私有数据、签名信任记录和项目结果会保留。",
            )),
            Self::ClearCache => Some((
                "清理插件缓存？",
                "停止空闲插件并清理其声明的缓存目录；项目结果和缓存外设置会保留。",
            )),
            Self::ClearHistory => Some((
                "清理历史结果缓存？",
                "历史中的结果缓存将清除；任务状态与项目结果会保留。",
            )),
            Self::CollectPackages => Some((
                "回收未使用插件包？",
                "移除未被已安装插件或有效回执引用的包；项目资源和已安装插件会保留。",
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
            let prompt = window.prompt(
                PromptLevel::Warning,
                title,
                Some(message),
                &["确认", "取消"],
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
        self.task = Some("正在更新插件…");
        self.error = None;
        self.feedback = None;
        let job = self.services.run(move |services| {
            let manager = &services.plugins;
            let result = (|| {
                if let Some(key) = &key {
                    key.validate(manager)?;
                }
                if matches!(action, PluginAction::CollectPackages) {
                    return manager
                        .collect_garbage()
                        .map(|count| format!("已回收 {count} 个未使用插件包。"));
                }
                let key = key.ok_or_else(|| {
                    yss_plugin_runtime::PluginFailure::new("plugin_stale_context")
                })?;
                match action {
                    PluginAction::Enable(enabled) => {
                        manager.set_enabled(&key.id, enabled).map(|_| {
                            if enabled {
                                "插件已启用。"
                            } else {
                                "插件已停用。"
                            }
                            .into()
                        })
                    }
                    PluginAction::Uninstall => manager
                        .uninstall(&key.id)
                        .map(|_| "插件已卸载，私有数据与项目结果保留。".into()),
                    PluginAction::ClearCache => manager
                        .clear_private_cache(&key.id)
                        .map(|_| "插件缓存已清理。".into()),
                    PluginAction::ClearHistory => manager
                        .clear_task_history(&key.id)
                        .map(|_| "历史结果缓存已清理。".into()),
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
                    view.error = Some("插件任务未完成，请刷新并检查实际状态。".into());
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
