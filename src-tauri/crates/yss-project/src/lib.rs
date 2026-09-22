//! Project session、resource revision、durable transaction 与 publication 的权威边界。

pub mod execution_authority;
pub mod graph_resource_index;
mod manifest;
pub use manifest::CURRENT_PROJECT_SCHEMA_VERSION;
mod file_changes;
mod filesystem;
mod operation_error;
pub(crate) mod project_change_reconciliation;
pub mod project_error;
pub use file_changes::{
    ProjectIndexInvalidation, filesystem_change_affects_project_index, is_project_index_watch_path,
};
pub use filesystem::project_root_from_path;
pub use operation_error::ProjectOperationError;
mod resource_lifecycle_operation;

pub mod database_authority;
pub mod project_activation;
pub mod project_io;
pub mod project_lifecycle;
mod project_operation_admission;
pub mod project_reads;
pub mod project_session;
pub mod project_state;
pub mod project_writers;

pub mod project_store;

pub mod chart_io;
pub mod external_resources;
pub mod resource_reveal;

pub use graph_resource_index::*;
pub use project_error::*;
pub(crate) use resource_lifecycle_operation::{
    ResourceLifecycleOperation, ResourceRenameOwnershipLease,
};

pub use database_authority::ProjectDatabaseError;
pub use project_activation::*;
pub use project_io::*;
pub use project_lifecycle::*;
pub use project_session::*;
pub use project_state::*;
pub use project_store::*;

pub use chart_io::*;
pub use resource_reveal::*;

use yss_graph_document::GraphResourcePath;

#[cfg(any(test, feature = "test-support"))]
pub mod fixtures {
    use super::{GraphResourcePath, ProjectError};
    use std::path::{Path, PathBuf};
    use yss_chart_document::{ChartDocument, ChartResourcePath};
    use yss_project_model::ProjectData;

    pub struct TempProject {
        state: Option<super::ProjectState>,
        root: PathBuf,
    }

    impl TempProject {
        pub fn activate(label: &str, project: ProjectData) -> Self {
            let root = std::env::temp_dir().join(format!("yssbi-{label}-{}", uuid::Uuid::new_v4()));
            let mut fixture = Self { state: None, root };
            std::fs::create_dir_all(&fixture.root).unwrap();
            write_project(&project, fixture.root.to_string_lossy().as_ref()).unwrap();
            let state = super::ProjectState::new();
            state.activate_project_fixture(fixture.root.to_string_lossy().into_owned(), project);
            fixture.state = Some(state);
            fixture
        }

        pub fn state(&self) -> &super::ProjectState {
            self.state.as_ref().expect("temporary project is active")
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            drop(self.state.take());
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    pub fn write_project(project_data: &ProjectData, path: &str) -> Result<(), ProjectError> {
        super::project_io::initialize_project_directory(
            project_data,
            crate::project_root_from_path(path).as_path(),
        )
    }

    pub fn write_graph(
        project_data: &ProjectData,
        path: &str,
        graph_path: &GraphResourcePath,
    ) -> Result<String, ProjectError> {
        let root = crate::project_root_from_path(path);
        std::fs::create_dir_all(&root)?;
        let (relative_path, contents) =
            super::project_io::serialize_graph_document(project_data, graph_path)?;
        let target = root.join(&relative_path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(target, contents)?;
        Ok(relative_path.to_string_lossy().replace('\\', "/"))
    }

    pub fn chart(name: &str, database_id: &str) -> (ChartResourcePath, ChartDocument) {
        let name = yss_resource_naming::ResourceName::parse(name).unwrap();
        (
            ChartResourcePath::from_name(&name),
            ChartDocument::new(database_id),
        )
    }

    pub fn write_chart(
        root: &Path,
        path: &ChartResourcePath,
        document: &ChartDocument,
    ) -> Result<(), ProjectError> {
        let (relative_path, contents) = super::chart_io::serialize_chart(path, document)?;
        let target = root.join(relative_path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(target, contents)?;
        Ok(())
    }
}
