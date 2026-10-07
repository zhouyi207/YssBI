use serde::{Deserialize, Serialize};
use yss_project_identity::ProjectInstanceId;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload")]
pub enum EventResource {
    #[serde(rename_all = "camelCase")]
    ProjectIndexInvalidated {
        project_instance_id: ProjectInstanceId,
        source: String,
        version: u64,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_index_invalidation_serializes_project_identity_and_watcher_version() {
        let event = crate::event::Event::Resource(EventResource::ProjectIndexInvalidated {
            project_instance_id: ProjectInstanceId::from_existing(
                "00000000-0000-0000-0000-000000000601".into(),
            ),
            source: "watcher".into(),
            version: 3,
        });

        let contract: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../src/tests/fixtures/project-event-wire/project-index-invalidated.json"
        )))
        .unwrap();

        assert_eq!(serde_json::to_value(event).unwrap(), contract);
    }
}
