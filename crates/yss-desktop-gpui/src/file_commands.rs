//! Captured authored-file commits shared by the two native resource editors.
use crate::services::NativeServices;
use std::sync::Arc;
use yss_application::{
    file_resources::{FileApplicationError, FileMutation},
    runtime::ApplicationServices,
};
use yss_project::file_resources::{FileCommand, FileSnapshot};
use yss_project_identity::{OperationId, ProjectInstanceId};
use yss_project_model::{
    doc::DocDocument,
    file::{FileContent, FilePath, FileVersion},
    mind::MindDocument,
};

pub(crate) trait NativeFile: FileContent {
    fn read(
        services: &ApplicationServices,
        project: ProjectInstanceId,
        path: FilePath<Self>,
    ) -> Result<FileSnapshot<Self>, FileApplicationError>;
    fn apply_command(
        services: &ApplicationServices,
        project: ProjectInstanceId,
        command: FileCommand<Self>,
    ) -> Result<FileMutation<Self>, FileApplicationError>;
}

impl NativeFile for DocDocument {
    fn read(
        services: &ApplicationServices,
        project: ProjectInstanceId,
        path: FilePath<Self>,
    ) -> Result<FileSnapshot<Self>, FileApplicationError> {
        services.application.read_doc(project, path)
    }
    fn apply_command(
        services: &ApplicationServices,
        project: ProjectInstanceId,
        command: FileCommand<Self>,
    ) -> Result<FileMutation<Self>, FileApplicationError> {
        services
            .application
            .apply_doc_command(project, OperationId::new(), command)
    }
}

impl NativeFile for MindDocument {
    fn read(
        services: &ApplicationServices,
        project: ProjectInstanceId,
        path: FilePath<Self>,
    ) -> Result<FileSnapshot<Self>, FileApplicationError> {
        services.application.read_mind(project, path)
    }
    fn apply_command(
        services: &ApplicationServices,
        project: ProjectInstanceId,
        command: FileCommand<Self>,
    ) -> Result<FileMutation<Self>, FileApplicationError> {
        services
            .application
            .apply_mind_command(project, OperationId::new(), command)
    }
}

pub(crate) struct FileSaveRequest<T: NativeFile> {
    pub project: ProjectInstanceId,
    pub path: FilePath<T>,
    pub version: FileVersion,
    pub edits: Vec<T::Edit>,
}

pub(crate) struct FileSaveOutcome<T: FileContent> {
    pub snapshot: Option<FileSnapshot<T>>,
    pub failed: bool,
}

impl<T: NativeFile> FileSaveRequest<T> {
    pub fn commit(
        self,
        services: &ApplicationServices,
        owner: &Arc<NativeServices>,
    ) -> FileSaveOutcome<T> {
        let mut version = self.version;
        let mut snapshot = None;
        if !self.edits.is_empty() {
            let receipt = T::apply_command(
                services,
                self.project.clone(),
                FileCommand::Edit {
                    path: self.path.clone(),
                    version: version.clone(),
                    edits: self.edits,
                },
            );
            let Ok(receipt) = receipt else {
                return FileSaveOutcome {
                    snapshot: None,
                    failed: true,
                };
            };
            owner.publish_resource(receipt.mutation);
            let Some(edited) = receipt.snapshot else {
                return FileSaveOutcome {
                    snapshot: None,
                    failed: true,
                };
            };
            version = edited.version.clone();
            snapshot = Some(edited);
        }
        match T::apply_command(
            services,
            self.project,
            FileCommand::Save {
                path: self.path,
                version,
            },
        ) {
            Ok(receipt) => {
                owner.publish_resource(receipt.mutation);
                FileSaveOutcome {
                    failed: receipt.snapshot.is_none(),
                    snapshot: receipt.snapshot,
                }
            }
            Err(_) => FileSaveOutcome {
                snapshot,
                failed: true,
            },
        }
    }
}
