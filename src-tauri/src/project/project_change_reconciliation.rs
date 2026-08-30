use crate::project::{ProjectFilesystemError, ProjectState};
use yss_project_change::{ProjectChange, ProjectIndexInvalidation};
use yss_project_identity::ProjectInstanceId;

impl ProjectState {
    pub(crate) fn reconcile_project_change(
        &self,
        project: &ProjectInstanceId,
        change: ProjectChange,
    ) -> Result<Option<ProjectIndexInvalidation>, ProjectFilesystemError> {
        if !change.affects_project_index() {
            return Ok(None);
        }

        self.read_project_index(project)?;
        Ok(Some(ProjectIndexInvalidation::new(project.clone())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::fixtures;
    use yss_project_change::{ProjectFileChangeKind, ProjectRelativePath};
    use yss_project_model::ProjectData;

    #[test]
    fn unrelated_change_is_a_noop_instead_of_an_error() {
        let fixture = fixtures::TempProject::activate("watcher-unrelated", ProjectData::new());
        let project = ProjectInstanceId::from_existing(fixture.state().project_instance_id());
        let result = fixture
            .state()
            .reconcile_project_change(
                &project,
                ProjectChange::file(
                    ProjectRelativePath::try_new("README.md").unwrap(),
                    ProjectFileChangeKind::Modified,
                ),
            )
            .unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn project_reconciliation_returns_typed_index_invalidation() {
        let fixture = fixtures::TempProject::activate("watcher-reconcile", ProjectData::new());
        let project = ProjectInstanceId::from_existing(fixture.state().project_instance_id());
        let invalidation = fixture
            .state()
            .reconcile_project_change(
                &project,
                ProjectChange::file(
                    ProjectRelativePath::try_new("metadata.yssbi").unwrap(),
                    ProjectFileChangeKind::Modified,
                ),
            )
            .unwrap()
            .expect("metadata changes invalidate the project index");

        assert_eq!(invalidation.project_instance_id(), &project);
    }
}
