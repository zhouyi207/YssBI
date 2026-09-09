//! Bounded, immutable delivery baselines. These are projections, never project authority.
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::error::CommandError;
use crate::schema::activity_panel::ActivityPanelDocumentDto;

const MAX_BASELINES: usize = 32;
const MAX_BASELINE_BYTES: usize = 16 * 1024 * 1024;
const MAX_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Clone, Default)]
pub struct ActivityPanelSyncState(Arc<Mutex<VecDeque<Baseline>>>);

struct Baseline {
    window: String,
    locale: String,
    cursor: String,
    document: Arc<Value>,
    bytes: usize,
}

#[derive(Debug, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ActivityPanelUpdateDto {
    Snapshot {
        cursor: String,
        document: ArcDocument,
    },
    Patch {
        base_cursor: String,
        cursor: String,
        patch: Map<String, Value>,
        operations: Vec<RowOperation>,
    },
}

// Serialize through a reference; the cache and initial reply share the same allocation.
#[derive(Debug, Clone)]
pub struct ArcDocument(Arc<Value>);
impl Serialize for ArcDocument {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.as_ref().serialize(serializer)
    }
}

#[derive(Debug, Serialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RowOperation {
    Update {
        id: String,
        patch: Map<String, Value>,
    },
    Insert {
        after_id: Option<String>,
        row: Value,
    },
    Remove {
        id: String,
    },
    Move {
        id: String,
        after_id: Option<String>,
    },
}

impl ActivityPanelSyncState {
    pub(crate) fn publish(
        &self,
        window: &str,
        locale: &str,
        cursor: Option<&str>,
        document: ActivityPanelDocumentDto,
    ) -> Result<ActivityPanelUpdateDto, CommandError> {
        let document = serde_json::to_value(document).map_err(CommandError::internal)?;
        self.publish_value(window, locale, cursor, document)
    }

    fn publish_value(
        &self,
        window: &str,
        locale: &str,
        cursor: Option<&str>,
        document: Value,
    ) -> Result<ActivityPanelUpdateDto, CommandError> {
        let bytes = serde_json::to_vec(&document)
            .map_err(CommandError::internal)?
            .len();
        if bytes > MAX_DOCUMENT_BYTES {
            return Err(CommandError::expected("activity_panel_limit_exceeded"));
        }
        let baseline = {
            let mut cache = self
                .0
                .lock()
                .map_err(|_| CommandError::expected("activity_panel_sync_unavailable"))?;
            let index = cache.iter().position(|entry| {
                Some(entry.cursor.as_str()) == cursor
                    && entry.window == window
                    && entry.locale == locale
                    && entry.document["panelId"] == document["panelId"]
                    && entry.document["projectInstanceId"] == document["projectInstanceId"]
            });
            index.map(|index| {
                let entry = cache.remove(index).expect("located baseline exists");
                let result = (entry.cursor.clone(), entry.document.clone());
                cache.push_back(entry);
                result
            })
        };
        if let Some((cursor, previous)) = &baseline {
            if previous.as_ref() == &document {
                return Ok(ActivityPanelUpdateDto::Patch {
                    base_cursor: cursor.clone(),
                    cursor: cursor.clone(),
                    patch: Map::new(),
                    operations: vec![],
                });
            }
            if previous["publicationRevision"].as_u64() > document["publicationRevision"].as_u64() {
                return Err(CommandError::expected("activity_panel_stale_read"));
            }
        }

        let next_cursor = uuid::Uuid::new_v4().to_string();
        let document = Arc::new(document);
        let update = if let Some((base_cursor, previous)) = baseline {
            let patch = ["title", "tools", "emptyState", "publicationRevision"]
                .into_iter()
                .filter(|key| previous[key] != document[key])
                .map(|key| (key.to_owned(), document[key].clone()))
                .collect();
            ActivityPanelUpdateDto::Patch {
                base_cursor,
                cursor: next_cursor.clone(),
                patch,
                operations: row_operations(
                    previous["rows"]
                        .as_array()
                        .expect("validated document rows"),
                    document["rows"]
                        .as_array()
                        .expect("validated document rows"),
                ),
            }
        } else {
            ActivityPanelUpdateDto::Snapshot {
                cursor: next_cursor.clone(),
                document: ArcDocument(document.clone()),
            }
        };
        if matches!(update, ActivityPanelUpdateDto::Patch { .. })
            && serde_json::to_vec(&update)
                .map_err(CommandError::internal)?
                .len()
                > 2 * MAX_DOCUMENT_BYTES
        {
            return Err(CommandError::expected("activity_panel_limit_exceeded"));
        }
        let mut cache = self
            .0
            .lock()
            .map_err(|_| CommandError::expected("activity_panel_sync_unavailable"))?;
        let mut retained_bytes: usize = cache.iter().map(|entry| entry.bytes).sum();
        while cache.len() >= MAX_BASELINES || retained_bytes + bytes > MAX_BASELINE_BYTES {
            let Some(evicted) = cache.pop_front() else {
                break;
            };
            retained_bytes -= evicted.bytes;
        }
        cache.push_back(Baseline {
            window: window.into(),
            locale: locale.into(),
            cursor: next_cursor,
            document,
            bytes,
        });
        Ok(update)
    }
}

fn row_id(row: &Value) -> &str {
    row["id"].as_str().expect("validated row identity")
}

fn row_operations(previous: &[Value], next: &[Value]) -> Vec<RowOperation> {
    let old_rows: BTreeMap<_, _> = previous.iter().map(|row| (row_id(row), row)).collect();
    let new_rows: BTreeMap<_, _> = next.iter().map(|row| (row_id(row), row)).collect();
    let retained = |id: &str| {
        old_rows
            .get(id)
            .zip(new_rows.get(id))
            .is_some_and(|(old, new)| old["kind"] == new["kind"])
    };
    let mut operations = vec![];
    let mut order: Vec<&str> = previous
        .iter()
        .filter_map(|row| {
            let id = row_id(row);
            if retained(id) {
                Some(id)
            } else {
                operations.push(RowOperation::Remove { id: id.into() });
                None
            }
        })
        .collect();
    for (index, row) in next.iter().enumerate() {
        let id = row_id(row);
        let after_id = index
            .checked_sub(1)
            .map(|previous| row_id(&next[previous]).to_owned());
        if order.get(index).copied() != Some(id) {
            if let Some(from) = order.iter().position(|current| *current == id) {
                order.remove(from);
                operations.push(RowOperation::Move {
                    id: id.into(),
                    after_id,
                });
            } else {
                operations.push(RowOperation::Insert {
                    after_id,
                    row: row.clone(),
                });
            }
            order.insert(index, id);
        }
        if retained(id) && old_rows[id] != row {
            // Patches replace only changed top-level row fields. Null is a value, not deletion.
            let patch = row
                .as_object()
                .expect("validated row")
                .iter()
                .filter(|(key, value)| old_rows[id].get(*key) != Some(*value))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            operations.push(RowOperation::Update {
                id: id.into(),
                patch,
            });
        }
    }
    operations
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(id: &str, label: &str) -> Value {
        json!({"id":id,"kind":"message","depth":0,"label":{"text":label},"description":null})
    }
    fn document(rows: Vec<Value>, revision: u64) -> Value {
        json!({"schema":"yssbi.activity-panel.v1","panelId":"nodes","projectInstanceId":"p1","publicationRevision":revision,"title":{"key":"activityBar.project"},"tools":[],"rows":rows,"emptyState":null})
    }
    fn publish(cache: &ActivityPanelSyncState, cursor: Option<&str>, document: Value) -> Value {
        serde_json::to_value(
            cache
                .publish_value("main", "en-US", cursor, document)
                .unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn emits_only_changed_fields_and_order_operations() {
        let cache = ActivityPanelSyncState::default();
        let first = publish(
            &cache,
            None,
            document(vec![row("a", "A"), row("b", "B"), row("c", "C")], 1),
        );
        assert_eq!(first["kind"], "snapshot");
        let next = document(vec![row("c", "Updated"), row("d", "D"), row("a", "A")], 2);
        let update = publish(&cache, first["cursor"].as_str(), next.clone());
        assert_eq!(update["kind"], "patch");
        assert_eq!(update["baseCursor"], first["cursor"]);
        assert_ne!(update["cursor"], first["cursor"]);
        assert_eq!(update["patch"], json!({"publicationRevision":2}));
        assert_eq!(
            update["operations"],
            json!([
                {"op":"remove","id":"b"},
                {"op":"move","id":"c","afterId":null},
                {"op":"update","id":"c","patch":{"label":{"text":"Updated"}}},
                {"op":"insert","afterId":"c","row":row("d","D")}
            ])
        );
        assert!(update.get("document").is_none());
        let unchanged = publish(&cache, update["cursor"].as_str(), next);
        assert_eq!(unchanged["operations"], json!([]));
        assert_eq!(unchanged["patch"], json!({}));
        assert_eq!(unchanged["cursor"], update["cursor"]);
    }

    #[test]
    fn isolates_scopes_and_retains_bounded_baselines_for_lost_replies() {
        let cache = ActivityPanelSyncState::default();
        let first = publish(&cache, None, document(vec![row("a", "A")], 1));
        let base = first["cursor"].as_str();
        publish(&cache, base, document(vec![row("a", "Unacknowledged")], 2));
        let latest = publish(&cache, base, document(vec![row("a", "Latest")], 3));
        assert_eq!(latest["kind"], "patch");
        assert_eq!(latest["baseCursor"], first["cursor"]);
        for (window, locale, project) in [
            ("other", "en-US", "p1"),
            ("main", "zh-CN", "p1"),
            ("main", "en-US", "p2"),
        ] {
            let mut next = document(vec![], 1);
            next["projectInstanceId"] = json!(project);
            assert!(matches!(
                cache.publish_value(window, locale, base, next).unwrap(),
                ActivityPanelUpdateDto::Snapshot { .. }
            ));
        }
        for revision in 4..(MAX_BASELINES as u64 + 6) {
            publish(&cache, None, document(vec![], revision));
        }
        assert_eq!(cache.0.lock().unwrap().len(), MAX_BASELINES);
        assert_eq!(
            publish(&cache, base, document(vec![], 99))["kind"],
            "snapshot"
        );
    }
}
