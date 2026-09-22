//! Bounded transport baselines for immutable graph read projections.
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use yss_ipc_contract::graph_editing::{
    GraphEditorDeliveryDto, GraphEditorSessionDto, GraphProjectionChangeDto,
};

use super::error::CommandError;

const MAX_BASELINES: usize = 32;
const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_CACHED_FRAME_BYTES: usize = 16 * 1024 * 1024;
const MAX_CHANGES: usize = 512;

struct Baseline {
    binding: Binding,
    cursor: String,
    data: Arc<Value>,
    bytes: usize,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Binding {
    pub window: String,
    pub project: String,
    pub graph: String,
    pub locale: String,
}

#[derive(Clone, Default)]
pub struct GraphEditorSyncState(Arc<Mutex<VecDeque<Baseline>>>);

impl GraphEditorSyncState {
    pub(crate) fn encode(
        &self,
        binding: Binding,
        cursor: Option<&str>,
        session: GraphEditorSessionDto,
    ) -> Result<GraphEditorDeliveryDto, CommandError> {
        let data = Arc::new(serde_json::to_value(session).map_err(CommandError::internal)?);
        let bytes = serde_json::to_vec(data.as_ref())
            .map_err(CommandError::internal)?
            .len();
        // Large valid projections remain readable without retaining an oversized baseline.
        // Cache admission must not turn an already-committed edit into a command failure.
        if bytes > MAX_CACHED_FRAME_BYTES {
            return Ok(GraphEditorDeliveryDto::Snapshot {
                cursor: uuid::Uuid::new_v4().to_string(),
                snapshot_bytes: bytes,
                data: data.as_ref().clone(),
            });
        }
        let previous = self
            .0
            .lock()
            .unwrap()
            .iter()
            .find(|entry| entry.binding == binding && Some(entry.cursor.as_str()) == cursor)
            .map(|entry| (entry.cursor.clone(), entry.data.clone()));
        let next_cursor = uuid::Uuid::new_v4().to_string();
        let delta = previous.and_then(|(base_cursor, previous)| {
            if previous["editing"]["version"]["sessionId"]
                != data["editing"]["version"]["sessionId"]
            {
                return None;
            }
            let mut changes = Vec::new();
            if !diff(&previous, &data, &mut Vec::new(), &mut changes) {
                return None;
            }
            let delta = GraphEditorDeliveryDto::Delta {
                base_cursor,
                cursor: next_cursor.clone(),
                snapshot_bytes: bytes,
                changes,
            };
            serde_json::to_vec(&delta)
                .ok()
                .filter(|encoded| encoded.len() < bytes)
                .map(|_| delta)
        });
        let update = delta.unwrap_or_else(|| GraphEditorDeliveryDto::Snapshot {
            cursor: next_cursor.clone(),
            snapshot_bytes: bytes,
            data: data.as_ref().clone(),
        });
        let mut retained = self.0.lock().unwrap();
        let mut retained_bytes = retained.iter().map(|entry| entry.bytes).sum::<usize>();
        while retained.len() >= MAX_BASELINES || retained_bytes + bytes > MAX_BYTES {
            let Some(evicted) = retained.pop_front() else {
                break;
            };
            retained_bytes -= evicted.bytes;
        }
        retained.push_back(Baseline {
            binding,
            cursor: next_cursor,
            data,
            bytes,
        });
        Ok(update)
    }
}

fn diff(
    before: &Value,
    after: &Value,
    path: &mut Vec<String>,
    changes: &mut Vec<GraphProjectionChangeDto>,
) -> bool {
    if before == after {
        return true;
    }
    if changes.len() >= MAX_CHANGES {
        return false;
    }
    if path.len() >= 32 {
        changes.push(GraphProjectionChangeDto::Set {
            path: path.clone(),
            value: after.clone(),
        });
        return true;
    }
    match (before, after) {
        (Value::Object(before), Value::Object(after)) => {
            for key in before.keys().filter(|key| !after.contains_key(*key)) {
                if changes.len() >= MAX_CHANGES {
                    return false;
                }
                path.push(key.clone());
                changes.push(GraphProjectionChangeDto::Remove { path: path.clone() });
                path.pop();
            }
            for (key, value) in after {
                path.push(key.clone());
                let complete = match before.get(key) {
                    Some(previous) => diff(previous, value, path, changes),
                    None if changes.len() < MAX_CHANGES => {
                        changes.push(GraphProjectionChangeDto::Set {
                            path: path.clone(),
                            value: value.clone(),
                        });
                        true
                    }
                    None => false,
                };
                path.pop();
                if !complete {
                    return false;
                }
            }
        }
        (Value::Array(before), Value::Array(after)) if before.len() == after.len() => {
            for (index, (before, after)) in before.iter().zip(after).enumerate() {
                path.push(index.to_string());
                let complete = diff(before, after, path, changes);
                path.pop();
                if !complete {
                    return false;
                }
            }
        }
        _ => changes.push(GraphProjectionChangeDto::Set {
            path: path.clone(),
            value: after.clone(),
        }),
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_ipc_contract::graph_editing::{GraphEditVersionDto, GraphEditingStateDto};

    #[test]
    fn small_edits_use_deltas_and_missing_or_foreign_baselines_use_snapshots() {
        let projection = serde_json::from_str(include_str!(
            "../../../../../src/tests/fixtures/node-system-contracts/editor-projection.json"
        ))
        .unwrap();
        let mut session = GraphEditorSessionDto {
            document: Default::default(),
            result_state: yss_ipc_contract::execution::GraphResultStateDto {
                revision: "0".into(),
                execution_session_id: uuid::Uuid::new_v4().to_string(),
                semantic_input_hash: "0".repeat(64),
                outputs: Box::new([]),
                connections: Box::new([]),
            },
            projection,
            editing: GraphEditingStateDto {
                version: GraphEditVersionDto {
                    session_id: uuid::Uuid::new_v4().to_string(),
                    revision: "0".into(),
                },
                dirty: false,
                can_undo: false,
                can_redo: false,
            },
        };
        let binding = Binding {
            window: "main".into(),
            project: "project".into(),
            graph: session.projection.graph_path.to_string(),
            locale: "en-US".into(),
        };
        let sync = GraphEditorSyncState::default();
        let GraphEditorDeliveryDto::Snapshot { cursor, .. } =
            sync.encode(binding.clone(), None, session.clone()).unwrap()
        else {
            panic!("first response must be a snapshot");
        };
        session.projection.nodes[0].position.x += 1.0;
        let delta = sync
            .encode(binding.clone(), Some(&cursor), session.clone())
            .unwrap();
        assert!(
            matches!(&delta, GraphEditorDeliveryDto::Delta { base_cursor, changes, .. } if base_cursor == &cursor && changes.len() == 1)
        );
        assert!(
            serde_json::to_vec(&delta).unwrap().len() < serde_json::to_vec(&session).unwrap().len()
        );
        let mut other_window = binding.clone();
        other_window.window = "other".into();
        assert!(matches!(
            sync.encode(other_window, Some(&cursor), session.clone())
                .unwrap(),
            GraphEditorDeliveryDto::Snapshot { .. }
        ));
        sync.0.lock().unwrap().clear();
        assert!(matches!(
            sync.encode(binding, Some(&cursor), session).unwrap(),
            GraphEditorDeliveryDto::Snapshot { .. }
        ));
    }
}
