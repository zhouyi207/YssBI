use crate::execution::{ChannelEventEmitter, ExecutionEvent, Executor, ResultSourceStore};
use crate::graph::{GraphInstance, GraphKind, NodeId};
use crate::log::LogLevel;
use crate::log_exec;
use crate::project::{GraphResourcePath, ProjectData, ProjectStore};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex, RwLock};
use tauri::ipc::Channel;

use crate::graph::register::event::EVENT_BEGIN_NODE_TYPE;

/// Collect Event graphs to run under a brief read lock (no full `ProjectData` clone).
fn collect_event_graphs(
    project_data: &ProjectData,
    target_graph_path: Option<GraphResourcePath>,
) -> Vec<GraphInstance> {
    project_data
        .graphs
        .iter()
        .filter(|(graph_path, graph)| {
            graph.kind == GraphKind::Event
                && target_graph_path
                    .as_ref()
                    .is_none_or(|target| graph_path == &target)
        })
        .map(|(_, graph)| graph.clone())
        .collect()
}

fn find_event_begin_entry(graph: &GraphInstance) -> Result<Option<NodeId>, String> {
    let data_state = graph.data_state.read().map_err(|e| e.to_string())?;
    Ok(data_state
        .nodes
        .iter()
        .find(|(_, node)| node.definition.node_type == EVENT_BEGIN_NODE_TYPE)
        .map(|(id, _)| *id))
}

pub fn execute_project_data(
    project_data_state: Arc<RwLock<ProjectData>>,
    project_store: Arc<RwLock<ProjectStore>>,
    source_store: ResultSourceStore,
    on_event: Channel<ExecutionEvent>,
    target_graph_path: Option<GraphResourcePath>,
    cancel: Arc<std::sync::atomic::AtomicBool>,
) -> Result<Value, String> {
    let event_graphs = {
        let data = project_data_state.read().map_err(|e| e.to_string())?;
        collect_event_graphs(&data, target_graph_path)
    };

    let mut all_logs = Vec::new();
    let mut executed_count = 0;

    for graph in event_graphs {
        let Some(entry_node) = find_event_begin_entry(&graph)? else {
            log_exec!(
                LogLevel::Info,
                "[execute_project] Graph '{}' has no event_begin node, skipping",
                graph.name
            );
            continue;
        };

        log_exec!(
            LogLevel::Info,
            "[execute_project] Starting graph '{}' from event_begin node {:?}",
            graph.name,
            entry_node
        );

        let runtime = crate::graph::GraphRuntime::new(
            Arc::new(graph),
            Arc::clone(&project_data_state),
            Arc::clone(&project_store),
        );

        let mut executor = Executor::with_cancel(
            Arc::new(Mutex::new(runtime)),
            ChannelEventEmitter(on_event.clone()),
            source_store.clone(),
            Some(cancel.clone()),
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
