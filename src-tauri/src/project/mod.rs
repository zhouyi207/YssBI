//! 项目管理模块

mod compile_publication;
pub mod filesystem;
pub mod graph_lifecycle;
pub mod graph_resource_index;
pub mod graph_resource_path;
mod history_hydration;
pub mod path_format;
pub mod project_data;
pub mod project_error;

pub mod project_activation;
pub mod project_io;
pub mod project_lifecycle;
pub mod project_metadata;
pub mod project_picker_task;
pub mod project_reads;
pub mod project_registry;
pub mod project_scan;
pub mod project_session;
pub mod project_state;
pub mod project_state_database;
pub mod project_traces;
pub mod project_writers;
pub mod resource_mutations;

pub mod project_state_variable;
pub mod project_store;

pub mod project_watcher;
pub mod resource_patch;
pub mod resource_reveal;
pub mod unique_name;
pub mod worksheet_io;

pub use filesystem::*;
pub use graph_lifecycle::*;
pub use graph_resource_index::*;
pub use graph_resource_path::*;
pub use path_format::*;
pub use project_data::*;
pub use project_error::*;

pub use project_activation::*;
pub use project_io::*;
pub use project_lifecycle::*;
pub use project_metadata::*;
pub use project_picker_task::*;
pub use project_reads::{
    CatalogMutationResource, CatalogMutationValidationSnapshot, CatalogProjectSnapshot,
};
pub use project_registry::*;
pub use project_scan::*;
pub use project_session::*;
pub use project_state::*;
pub use project_store::*;
pub use project_traces::*;

pub use project_watcher::*;
pub use resource_patch::*;
pub use resource_reveal::*;
pub use worksheet_io::*;

#[cfg(test)]
pub(crate) mod fixtures {
    use super::{GraphResourcePath, ProjectData, ProjectError, WorksheetDocument};
    use std::path::{Path, PathBuf};

    pub(crate) struct TempProject {
        state: Option<super::ProjectState>,
        root: PathBuf,
    }

    impl TempProject {
        pub(crate) fn activate(label: &str, project: ProjectData) -> Self {
            let root = std::env::temp_dir().join(format!("yssbi-{label}-{}", uuid::Uuid::new_v4()));
            let mut fixture = Self { state: None, root };
            std::fs::create_dir_all(&fixture.root).unwrap();
            write_project(&project, fixture.root.to_string_lossy().as_ref()).unwrap();
            let state = super::ProjectState::new();
            state.activate_project_fixture(fixture.root.to_string_lossy().into_owned(), project);
            fixture.state = Some(state);
            fixture
        }

        pub(crate) fn state(&self) -> &super::ProjectState {
            self.state.as_ref().expect("temporary project is active")
        }
    }

    impl Drop for TempProject {
        fn drop(&mut self) {
            drop(self.state.take());
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    pub(crate) fn write_project(
        project_data: &ProjectData,
        path: &str,
    ) -> Result<(), ProjectError> {
        super::project_io::initialize_project_directory(
            project_data,
            super::project_io::project_root_from_path(path).as_path(),
        )
    }

    pub(crate) fn write_graph(
        project_data: &ProjectData,
        path: &str,
        graph_path: &GraphResourcePath,
    ) -> Result<String, ProjectError> {
        let root = super::project_io::project_root_from_path(path);
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

    pub(crate) fn write_worksheet(
        root: &Path,
        document: &WorksheetDocument,
    ) -> Result<(), ProjectError> {
        let (relative_path, contents) = super::worksheet_io::serialize_worksheet(document)?;
        let target = root.join(relative_path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(target, contents)?;
        Ok(())
    }

    pub(crate) fn flush_state(
        state: &super::ProjectState,
    ) -> Result<super::project_writers::ProjectSaveResultDto, super::ProjectFilesystemError> {
        let session = state.capture_project_session()?;
        state.flush_project_documents(
            &session.instance_id,
            crate::node_system::document::OperationId::new(),
        )
    }

    pub(crate) fn write_state_graph(
        state: &super::ProjectState,
        graph_path: &GraphResourcePath,
    ) -> Result<super::project_writers::ProjectSaveResultDto, super::ProjectFilesystemError> {
        let session = state.capture_project_session()?;
        let revision = state
            .get_data()?
            .graphs
            .get(graph_path)
            .ok_or_else(|| super::ProjectFilesystemError::TransactionPrepareFailed {
                message: format!("graph '{}' is not loaded", graph_path),
            })?
            .document
            .revision;
        state.save_graph_document(
            &session.instance_id,
            graph_path,
            revision,
            crate::node_system::document::OperationId::new(),
        )
    }
}

#[cfg(test)]
mod production_tests;
#[cfg(test)]
mod project_trace_query_tests;
#[cfg(test)]
mod structured_control_production_tests;
