//! Stable project identities and monotonic revision value objects.
//!
//! This crate is the canonical owner of identities shared by project,
//! application, transport, and adapter layers. It deliberately contains no
//! project state or persistence behavior.

mod identity;
mod project_instance_id;
mod project_registration_id;
mod project_root_identity;
mod project_session_id;

pub use identity::{
    HistoryEntryId, OperationId, ProjectResourcePath, ProjectRevision, ResourceRevision,
    RevisionExhausted,
};
pub use project_instance_id::ProjectInstanceId;
pub use project_registration_id::ProjectRegistrationId;
pub use project_root_identity::ProjectRootIdentity;
pub use project_session_id::ProjectSessionId;

#[cfg(test)]
mod tests {
    use super::{
        OperationId, ProjectInstanceId, ProjectRegistrationId, ProjectRootIdentity,
        ProjectSessionId, ResourceRevision,
    };

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
    fn registry_and_runtime_identities_remain_distinct_string_contracts() {
        let registration = ProjectRegistrationId::from_existing("registration-7".into());
        let runtime = ProjectInstanceId::from_existing("runtime-9".into());
        let root = ProjectRootIdentity::from_canonical("native-root-11".into());

        assert_eq!(
            serde_json::to_value(registration).unwrap(),
            "registration-7"
        );
        assert_eq!(serde_json::to_value(runtime).unwrap(), "runtime-9");
        assert_eq!(serde_json::to_value(root).unwrap(), "native-root-11");
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
}
