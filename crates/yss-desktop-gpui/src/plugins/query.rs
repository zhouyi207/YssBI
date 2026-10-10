//! Each selected-plugin read checks the original installation identity before and after I/O.
use super::{PluginKey, PluginsEvent, PluginsPanel, failure};
use gpui_kit::{Context, Window};
use yss_plugin_runtime::{
    PluginDiagnostic, PluginFailure, PluginManager, PluginStorageUsage, TaskHistoryPage,
    TaskSnapshot,
};

pub(super) struct PluginDetail {
    pub key: PluginKey,
    pub storage: PluginStorageUsage,
    pub active: Vec<TaskSnapshot>,
    pub history: TaskHistoryPage,
    pub diagnostics: Vec<PluginDiagnostic>,
    pub diagnostic_lines: Vec<String>,
}
fn read(
    manager: &PluginManager,
    key: PluginKey,
    cursor: Option<String>,
) -> Result<PluginDetail, PluginFailure> {
    key.validate(manager)?;
    let storage = manager.storage_usage(&key.id)?;
    let active = manager
        .list_tasks()?
        .into_iter()
        .filter(|task| task.plugin_id == key.id)
        .collect();
    let history = manager.task_history(&key.id, cursor.as_deref(), 25)?;
    let diagnostics = manager.diagnostics(&key.id)?;
    key.validate(manager)?;
    let diagnostic_lines = diagnostics
        .iter()
        .flat_map(|entry| {
            std::iter::once(format!(
                "[{} {}]",
                entry.instance_id,
                entry.task_ids.join(", ")
            ))
            .chain(entry.stderr.lines().map(str::to_owned))
        })
        .collect();
    Ok(PluginDetail {
        key,
        storage,
        active,
        history,
        diagnostics,
        diagnostic_lines,
    })
}
impl PluginsPanel {
    pub(crate) fn reload(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            self.reload_again = true;
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.task = Some(crate::text::t("native.plugins.loadingCatalog"));
        self.error = None;
        let job = self.services.run(|services| Ok(services.plugins.list()));
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.generation != generation {
                    return;
                }
                view.task = None;
                match result {
                    Some(Ok(entries)) => view.install_entries(entries),
                    Some(Err(error)) => view.error = Some(failure(&error)),
                    None => {
                        view.error = Some(crate::text::t("native.plugins.catalogFailed").into())
                    }
                }
                if view.reload_again {
                    view.reload_again = false;
                    view.reload(window, cx);
                } else if let Some(key) = view.selected.clone() {
                    view.read_details(key, view.cursor_stack.last().cloned().flatten(), window, cx);
                }
                view.changed(cx);
            });
        })
        .detach();
        self.changed(cx);
    }
    pub(super) fn select(&mut self, key: PluginKey, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy()
            || !self
                .entries
                .iter()
                .any(|plugin| PluginKey::from_plugin(plugin) == key)
        {
            return;
        }
        self.selected = Some(key.clone());
        self.detail = None;
        self.cursor_stack = vec![None];
        self.error = None;
        self.feedback = None;
        self.read_details(key, None, window, cx);
        cx.emit(PluginsEvent::OpenDetails);
    }
    pub(super) fn read_details(
        &mut self,
        key: PluginKey,
        cursor: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy() {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.task = Some(crate::text::t("native.plugins.loadingDetails"));
        let expected = key.clone();
        let job = self
            .services
            .run(move |services| Ok(read(&services.plugins, key, cursor)));
        cx.spawn_in(window, async move |view, cx| {
            let result = job.await.ok().and_then(Result::ok);
            let _ = view.update_in(cx, |view, window, cx| {
                if view.generation != generation || view.selected.as_ref() != Some(&expected) {
                    return;
                }
                view.task = None;
                match result {
                    Some(Ok(detail)) => view.detail = Some(detail),
                    Some(Err(error)) => {
                        view.detail = None;
                        view.error = Some(failure(&error));
                    }
                    None => {
                        view.detail = None;
                        view.error = Some(crate::text::t("native.plugins.detailsFailed").into());
                    }
                }
                if view.reload_again {
                    view.reload_again = false;
                    view.reload(window, cx);
                }
                view.changed(cx);
            });
        })
        .detach();
        self.changed(cx);
    }
    pub(super) fn history_page(&mut self, next: bool, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let Some(detail) = &self.detail else {
            return;
        };
        let key = detail.key.clone();
        if next {
            let Some(cursor) = detail.history.next_cursor.clone() else {
                return;
            };
            self.cursor_stack.push(Some(cursor));
        } else if self.cursor_stack.len() > 1 {
            self.cursor_stack.pop();
        } else {
            return;
        }
        self.error = None;
        self.read_details(key, self.cursor_stack.last().cloned().flatten(), window, cx);
    }
}
