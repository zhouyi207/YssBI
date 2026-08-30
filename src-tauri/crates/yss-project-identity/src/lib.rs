//! Stable project identities and monotonic revision value objects.
//!
//! This crate is the canonical owner of identities shared by project,
//! application, transport, and adapter layers. It deliberately contains no
//! project state or persistence behavior.

mod identity;
mod project_instance_id;
mod project_session_id;

pub use identity::{
    HistoryEntryId, OperationId, ProjectResourcePath, ProjectRevision, ResourceRevision,
    RevisionExhausted,
};
pub use project_instance_id::ProjectInstanceId;
pub use project_session_id::ProjectSessionId;

#[cfg(test)]
mod tests {
    use super::{OperationId, ProjectInstanceId, ProjectSessionId, ResourceRevision};

    #[test]
    fn typed_identifiers_preserve_their_wire_values() {
        let operation_id = OperationId::from_uuid(uuid::Uuid::from_u128(0x51));
        let project_instance_id =
            ProjectInstanceId::from_existing("00000000-0000-0000-0000-000000000601".into());
        let project_session_id = ProjectSessionId::new("session-7");

        assert_eq!(
            serde_json::to_value(operation_id).unwrap(),
            "00000000-0000-0000-0000-000000000051"
        );
        assert_eq!(
            serde_json::to_value(project_instance_id).unwrap(),
            "00000000-0000-0000-0000-000000000601"
        );
        assert_eq!(project_session_id.as_str(), "session-7");
    }

    #[test]
    fn revision_advancement_is_checked() {
        assert_eq!(ResourceRevision::new(4).checked_next().unwrap().get(), 5);
        assert_eq!(
            ResourceRevision::new(u64::MAX)
                .checked_next()
                .unwrap_err()
                .retained,
            u64::MAX
        );
    }

    #[test]
    fn graph_revision_conversion_remains_explicit() {
        let graph_revision = yss_graph_document::GraphRevision::new(9);
        let resource_revision = ResourceRevision::from_graph_revision(graph_revision);

        assert_eq!(resource_revision.get(), 9);
        assert_eq!(resource_revision.to_graph_revision(), graph_revision);
    }
}
