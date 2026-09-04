use crate::ProjectSession;
use yss_project_filesystem::ProjectFilesystemError;
use yss_resource_lifecycle::{
    ResourceLifecycleBoundary, ResourceLifecycleGuard, ResourceLifecycleIntent,
    ResourceLifecycleOwner,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResourceLifecycleOperation {
    pub(crate) session: ProjectSession,
    pub(crate) owner: ResourceLifecycleOwner,
}

impl ResourceLifecycleOperation {
    pub(crate) fn from_guard(session: ProjectSession, guard: &ResourceLifecycleGuard) -> Self {
        Self {
            owner: guard.owner().clone(),
            session,
        }
    }

    pub(crate) fn stale_error(&self) -> ProjectFilesystemError {
        ProjectFilesystemError::StaleProjectLifecycle {
            message: format!(
                "stale project lifecycle for resource '{}' in project instance '{}'",
                self.owner.resource_path, self.owner.project_instance_id
            ),
        }
    }
}

pub(crate) struct ResourceRenameOwnershipLease {
    pub(crate) operation: ResourceLifecycleOperation,
    guard: ResourceLifecycleGuard,
}

impl ResourceRenameOwnershipLease {
    pub(crate) fn new(
        operation: ResourceLifecycleOperation,
        guard: ResourceLifecycleGuard,
    ) -> Self {
        Self { operation, guard }
    }

    pub(crate) fn commit_with_boundary(
        &mut self,
        boundary: &mut ResourceLifecycleBoundary<'_>,
    ) -> Result<(), ProjectFilesystemError> {
        boundary
            .commit_guard(&mut self.guard, ResourceLifecycleIntent::Unload)
            .map_err(ProjectFilesystemError::from)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ProjectState, fixtures};
    use std::sync::Arc;
    use yss_graph_document::{GraphResourceKind, GraphResourcePath};
    use yss_project_model::{GraphResourceDocument, ProjectData};

    #[test]
    fn load_rejects_owned_document_after_project_replacement() {
        let graph_path = GraphResourcePath::new("events/Shared.yssbi-event").unwrap();
        let root = std::env::temp_dir().join(format!(
            "yssbi-lifecycle-projection-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let mut project_a = ProjectData::new();
        project_a.graphs.insert(
            graph_path.clone(),
            GraphResourceDocument::new("Shared", GraphResourceKind::Event),
        );
        fixtures::write_project(&project_a, root.to_string_lossy().as_ref()).unwrap();
        fixtures::write_graph(&project_a, root.to_string_lossy().as_ref(), &graph_path).unwrap();
        let state = ProjectState::new();
        state.activate_project_fixture(root.to_string_lossy().into_owned(), ProjectData::new());
        let project_instance_id = state.capture_project_session().unwrap().instance_id;

        let project_b = project_a;
        let replacement_state = state.clone();
        state.set_graph_load_after_read_test_hook(Arc::new(move || {
            replacement_state.activate_project_fixture("project-b".into(), project_b.clone());
        }));

        let error = state
            .load_graph_document(&project_instance_id, &graph_path, 1)
            .unwrap_err();

        assert!(matches!(
            error,
            ProjectFilesystemError::StaleProjectLifecycle { .. }
        ));
        std::fs::remove_dir_all(root).unwrap();
    }
}
