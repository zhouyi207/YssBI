use crate::ProjectOperationError;
use crate::ProjectSession;
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

    pub(crate) fn stale_error(&self) -> ProjectOperationError {
        ProjectOperationError::StaleProjectLifecycle {
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
    ) -> Result<(), ProjectOperationError> {
        boundary
            .commit_guard(&mut self.guard, ResourceLifecycleIntent::Unload)
            .map_err(ProjectOperationError::from)?;
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
    fn resident_load_rejects_a_new_project_document_at_the_same_path() {
        use yss_graph_document::{DocumentNode, NodeId, NodePosition};

        let path = GraphResourcePath::new("events/Shared.yssbi-event").unwrap();
        let mut data = ProjectData::new();
        data.graphs.insert(
            path.clone(),
            GraphResourceDocument::new("Shared", GraphResourceKind::EventGraph),
        );
        let fixture = fixtures::TempProject::activate("resident-load-replaced", data.clone());
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let id = NodeId::new();
        Arc::make_mut(&mut data.graphs.get_mut(&path).unwrap().document)
            .nodes
            .insert(
                id,
                DocumentNode {
                    id,
                    node_type: "yssbi.tests.node".parse().unwrap(),
                    position: NodePosition { x: 0.0, y: 0.0 },
                    parameters: Default::default(),
                    user_label: Some("replacement project".into()),
                },
            );
        let replacement = state.clone();
        let root = session.root.as_path().to_string_lossy().into_owned();
        *state
            .test_hooks
            .graph_load_after_session_test_hook
            .write()
            .unwrap() = Some(Arc::new(move || {
            replacement.activate_project_fixture(root.clone(), data.clone());
        }));
        let result = state.load_graph_document(&session.instance_id, &path, 0);
        state
            .test_hooks
            .graph_load_after_session_test_hook
            .write()
            .unwrap()
            .take();
        assert_ne!(
            state.capture_project_session().unwrap().instance_id,
            session.instance_id
        );
        assert!(
            matches!(
                result,
                Err(crate::ProjectOperationError::StaleProjectLifecycle { .. })
            ),
            "old request received the replacement project's body: {result:?}"
        );
    }

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
            GraphResourceDocument::new("Shared", GraphResourceKind::EventGraph),
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

        let result = state.load_graph_document(&project_instance_id, &graph_path, 1);
        state
            .test_hooks
            .graph_load_after_read_test_hook
            .write()
            .unwrap()
            .take();

        assert!(matches!(
            result,
            Err(ProjectOperationError::StaleProjectLifecycle { .. })
        ));
        std::fs::remove_dir_all(root).unwrap();
    }
}
