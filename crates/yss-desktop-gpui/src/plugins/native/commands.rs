use super::{OpenNativeView, PluginViewPanel};
use gpui_kit::{Context, Window};
use serde_json::{Value, json};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use yss_plugin_runtime::{
    NativeCommandReply, NativeOperation, PluginFailure, TaskSnapshot, operation_id,
};

enum Reply {
    Command(NativeCommandReply),
    Task(Box<TaskSnapshot>),
    Open(yss_plugin_runtime::PluginView),
    Saved,
}

impl PluginViewPanel {
    pub(super) fn execute(
        &mut self,
        operation: NativeOperation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.closed {
            return;
        }
        let parameters = if matches!(operation, NativeOperation::OpenView { .. }) {
            Value::Null
        } else {
            match self.values(cx) {
                Ok(value) => value,
                Err(error) => {
                    self.error = Some(error);
                    cx.notify();
                    return;
                }
            }
        };
        let (method, input) = match &operation {
            NativeOperation::ExecuteCommand { command_id } => (
                "commands.execute",
                json!({"commandId":command_id,"parameters":parameters}),
            ),
            NativeOperation::StartTask { task_type } => {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|value| value.as_millis() as u64);
                let id = now
                    .ok()
                    .and_then(|now| operation_id(now, &uuid::Uuid::new_v4().to_string()).ok());
                let Some(id) = id else {
                    self.error = Some(PluginFailure::new("plugin_operation_invalid"));
                    cx.notify();
                    return;
                };
                (
                    "tasks.start",
                    json!({"operationId":id,"taskType":task_type,"parameters":parameters}),
                )
            }
            NativeOperation::OpenView { view_id } => ("views.open", json!({"viewId":view_id})),
        };
        self.request(method, input, window, cx);
    }

    pub(super) fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.values(cx) {
            Ok(value) => self.request("views.set_state", value, window, cx),
            Err(error) => {
                self.error = Some(error);
                cx.notify();
            }
        }
    }

    pub(super) fn cancel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(task) = &self.task {
            self.request("tasks.cancel", json!({"taskId":task.task_id}), window, cx);
        }
    }

    fn request(
        &mut self,
        method: &'static str,
        input: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.closed {
            return;
        }
        let Some(lease) = &self.lease else {
            return;
        };
        self.busy = true;
        self.error = None;
        self.message = None;
        let session = lease.session_id.clone();
        let binding = lease.window.clone();
        let job = self.services.run(move |services| {
            Ok(services
                .plugins
                .call_view(&session, &binding, method, input))
        });
        cx.spawn_in(window, async move |panel, cx| {
            let result = job
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or_else(|| Err(PluginFailure::new("plugin_state_unavailable")))
                .and_then(|value| {
                    match method {
                        "commands.execute" => serde_json::from_value(value).map(Reply::Command),
                        "tasks.start" => serde_json::from_value(value).map(Reply::Task),
                        "views.open" => serde_json::from_value(value["openView"]["view"].clone())
                            .map(Reply::Open),
                        _ => Ok(Reply::Saved),
                    }
                    .map_err(|_| PluginFailure::new("plugin_response_invalid"))
                });
            let _ = panel.update_in(cx, |panel, window, cx| {
                if panel.closed {
                    return;
                }
                panel.busy = false;
                match result {
                    Ok(Reply::Command(reply)) => {
                        panel.message = Some(reply.message);
                        if let Some(form) = reply.view
                            && let Err(error) = panel.install_form(form, Value::Null, window, cx)
                        {
                            panel.error = Some(error);
                        }
                    }
                    Ok(Reply::Task(task)) => {
                        panel.task = Some(*task);
                        panel.poll_task(window, cx);
                    }
                    Ok(Reply::Open(view)) => {
                        if let Some(key) = &panel.key {
                            cx.emit(OpenNativeView {
                                key: key.clone(),
                                view,
                            });
                        }
                    }
                    Ok(Reply::Saved) => {}
                    Err(error) => panel.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    fn poll_task(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(task) = &self.task else {
            return;
        };
        if task.state.terminal() || self.closed {
            return;
        }
        let id = task.task_id.clone();
        let timer = cx.background_executor().timer(Duration::from_secs(1));
        cx.spawn_in(window, async move |panel, cx| {
            timer.await;
            let job = panel
                .update_in(cx, |panel, _, _| {
                    if panel.closed || panel.task.as_ref().is_none_or(|task| task.task_id != id) {
                        return None;
                    }
                    let lease = panel.lease.as_ref()?;
                    let session = lease.session_id.clone();
                    let binding = lease.window.clone();
                    Some(panel.services.run(move |services| {
                        Ok(services.plugins.call_view(
                            &session,
                            &binding,
                            "tasks.get",
                            json!({"taskId":id}),
                        ))
                    }))
                })
                .ok()
                .flatten();
            let Some(job) = job else {
                return;
            };
            let result = job
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or_else(|| Err(PluginFailure::new("plugin_state_unavailable")))
                .and_then(|value| {
                    serde_json::from_value(value)
                        .map_err(|_| PluginFailure::new("plugin_response_invalid"))
                });
            let _ = panel.update_in(cx, |panel, window, cx| {
                if panel.closed {
                    return;
                }
                match result {
                    Ok(task) => {
                        panel.task = Some(task);
                        panel.poll_task(window, cx);
                    }
                    Err(error) => panel.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
}
