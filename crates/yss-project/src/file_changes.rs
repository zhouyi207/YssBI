//! Project interpretation of generic filesystem change facts.
use yss_filesystem::FilesystemChange;
use yss_project_identity::ProjectInstanceId;
use yss_project_layout::is_project_index_input_path;

pub fn filesystem_change_affects_project_index(change: &FilesystemChange) -> bool {
    match change {
        FilesystemChange::File(file) => is_project_index_input_path(file.relative_path().as_path()),
        FilesystemChange::RescanRequired => true,
    }
}

pub use yss_project_layout::is_project_index_input_path as is_project_index_watch_path;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectIndexInvalidation {
    project_instance_id: ProjectInstanceId,
    resources: Option<Vec<yss_project_history::ResourceKey>>,
}

impl ProjectIndexInvalidation {
    pub fn new(project_instance_id: ProjectInstanceId) -> Self {
        Self {
            project_instance_id,
            resources: None,
        }
    }

    pub(crate) fn for_resources(
        project_instance_id: ProjectInstanceId,
        resources: Vec<yss_project_history::ResourceKey>,
    ) -> Self {
        Self {
            project_instance_id,
            resources: Some(resources),
        }
    }

    /// None requires authoritative resync; Some contains the reconciled resource identities.
    pub fn resources(&self) -> Option<&[yss_project_history::ResourceKey]> {
        self.resources.as_deref()
    }

    pub fn project_instance_id(&self) -> &ProjectInstanceId {
        &self.project_instance_id
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use yss_filesystem::{FileChangeKind, RelativePath};
    #[test]
    fn file_changes_share_the_canonical_project_index_filter() {
        let relevant = FilesystemChange::file(
            RelativePath::try_new("events/foo.yssbi-event").unwrap(),
            FileChangeKind::Modified,
        );
        let unrelated = FilesystemChange::file(
            RelativePath::try_new("README.md").unwrap(),
            FileChangeKind::Modified,
        );

        assert!(filesystem_change_affects_project_index(&relevant));
        assert!(!filesystem_change_affects_project_index(&unrelated));
    }
}
