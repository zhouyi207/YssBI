use crate::execution::{ChannelEventEmitter, ExecutionEvent, Executor, WindowDataStore};
use crate::graph::{GraphId, GraphKind};
use crate::log::LogLevel;
use crate::log_exec;
use crate::project::{ProjectData, ProjectStore};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, RwLock};
use tauri::ipc::Channel;

pub fn execute_project_data(
    project_data: ProjectData,
    project_data_state: Arc<RwLock<ProjectData>>,
    project_store: Arc<RwLock<ProjectStore>>,
    window_store: WindowDataStore,
    on_event: Channel<ExecutionEvent>,
    target_graph_id: Option<GraphId>,
) -> Result<Value, String> {
    let mut all_logs = Vec::new();
    let mut executed_count = 0;

    for (gid, graph) in project_data.graphs.iter() {
        if graph.kind != GraphKind::Event {
            continue;
        }

        if let Some(ref target) = target_graph_id {
            if gid != target {
                continue;
            }
        }

        let event_begin_nodes: Vec<_> = {
            let data_state = graph.data_state.read().map_err(|e| e.to_string())?;
            data_state
                .nodes
                .iter()
                .filter(|(_, n)| n.definition.node_type == "Event:Event Begin")
                .map(|(id, _)| *id)
                .collect()
        };

        if event_begin_nodes.is_empty() {
            log_exec!(
                LogLevel::Info,
                "[execute_project] Graph '{}' has no event_begin node, skipping",
                graph.name
            );
            continue;
        }

        let entry_node = event_begin_nodes[0];
        log_exec!(
            LogLevel::Info,
            "[execute_project] Starting graph '{}' from event_begin node {:?}",
            graph.name,
            entry_node
        );

        let runtime = crate::graph::GraphRuntime::new(
            Arc::new(graph.clone()),
            Arc::clone(&project_data_state),
            Arc::clone(&project_store),
        );

        let mut executor = Executor::new(
            Arc::new(Mutex::new(runtime)),
            ChannelEventEmitter(on_event.clone()),
            window_store.clone(),
        );
        executor.start(entry_node)?;

        for line in executor.logs() {
            all_logs.push(line.clone());
            log_exec!(LogLevel::Info, "[Execute] {}", line);
        }
        executed_count += 1;
    }

    Ok(json!({
        "executedGraphs": executed_count,
        "logs": all_logs,
    }))
}
