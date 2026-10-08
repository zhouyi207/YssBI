//! A Rust process for exercising the host's native form and task contract.
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use yss_plugin_protocol::{NativeCommandReply, PROTOCOL_MAJOR, PluginFailure, ResourceBudget};
use yss_plugin_sdk::{Handler, Peer};

struct Task {
    started: Instant,
    parameters: Value,
    cancelled: bool,
}

fn message(parameters: &Value) -> Result<String, PluginFailure> {
    let name = parameters["name"]
        .as_str()
        .ok_or_else(|| PluginFailure::new("plugin_view_input_invalid"))?;
    let number = parameters["number"]
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| PluginFailure::new("plugin_view_input_invalid"))?;
    Ok(format!("{name}: {}", number * number))
}

fn main() {
    let tasks = Mutex::new(BTreeMap::<String, Task>::new());
    let handler: Handler = Arc::new(move |peer, request| {
        let input = &request.params["input"];
        match request.method.as_str() {
            "lifecycle.initialize" => {
                let budget: ResourceBudget =
                    serde_json::from_value(input["resourceBudget"].clone())
                        .map_err(|_| PluginFailure::new("plugin_resource_invalid"))?;
                peer.apply_budget(budget)?;
                Ok(
                    json!({"pluginId":"example.native","version":"0.1.0","protocolMajor":PROTOCOL_MAJOR}),
                )
            }
            "commands.execute" if input["commandId"] == "calculate" => {
                serde_json::to_value(NativeCommandReply {
                    message: message(&input["parameters"])?,
                    view: None,
                })
                .map_err(|_| PluginFailure::new("plugin_response_invalid"))
            }
            "tasks.start" | "tasks.get" | "tasks.cancel" | "tasks.result" => {
                let id = input["taskId"]
                    .as_str()
                    .ok_or_else(|| PluginFailure::new("plugin_task_missing"))?;
                let mut tasks = tasks
                    .lock()
                    .map_err(|_| PluginFailure::new("plugin_state_unavailable"))?;
                if request.method == "tasks.start" {
                    message(&input["parameters"])?;
                    tasks.insert(
                        id.into(),
                        Task {
                            started: Instant::now(),
                            parameters: input["parameters"].clone(),
                            cancelled: false,
                        },
                    );
                    return Ok(Value::Null);
                }
                let task = tasks
                    .get_mut(id)
                    .ok_or_else(|| PluginFailure::new("plugin_task_missing"))?;
                if request.method == "tasks.cancel" {
                    task.cancelled = true;
                    return Ok(Value::Null);
                }
                let ready = task.started.elapsed() >= Duration::from_secs(3);
                if request.method == "tasks.result" {
                    if !ready || task.cancelled {
                        return Err(PluginFailure::new("plugin_result_missing"));
                    }
                    return Ok(json!({"viewData":{"message":message(&task.parameters)?}}));
                }
                Ok(
                    json!({"taskId":id,"state":if task.cancelled {"cancelled"} else if ready {"succeeded"} else {"running"}}),
                )
            }
            _ => Err(PluginFailure::new("plugin_method_unknown")),
        }
    });
    let peer = Peer::connect(
        std::io::stdin(),
        std::io::stdout(),
        ResourceBudget::default(),
        handler,
    );
    while peer.is_alive() {
        std::thread::park_timeout(Duration::from_millis(100));
    }
}
