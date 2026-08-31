use yss_project_identity::ProjectSessionId;

/// Project-scoped identity owned by the current project authority.
pub struct ProjectStore {
    pub project_session_id: ProjectSessionId,
}

impl ProjectStore {
    pub fn new() -> Self {
        Self {
            project_session_id: ProjectSessionId::new(uuid::Uuid::new_v4().to_string()),
        }
    }
}

impl Default for ProjectStore {
    fn default() -> Self {
        Self::new()
    }
}
