//! Captured authored-file commits shared by the two native resource editors.
use yss_application::{
    file_resources::{FileApplicationError, FileMutation},
    session::ApplicationState,
};
use yss_desktop_services::ApplicationTasks;
use yss_project::file_resources::{FileCommand, FileSnapshot};
use yss_project_identity::{OperationId, ProjectInstanceId};
use yss_project_model::{
    doc::DocDocument,
    file::{FileContent, FilePath, FileVersion},
    mind::MindDocument,
};

pub trait NativeFile: FileContent {
    fn read(
        services: &ApplicationState,
        project: ProjectInstanceId,
        path: FilePath<Self>,
    ) -> Result<FileSnapshot<Self>, FileApplicationError>;
    fn apply_command(
        services: &ApplicationState,
        project: ProjectInstanceId,
        command: FileCommand<Self>,
    ) -> Result<FileMutation<Self>, FileApplicationError>;
}

impl NativeFile for DocDocument {
    fn read(
        services: &ApplicationState,
        project: ProjectInstanceId,
        path: FilePath<Self>,
    ) -> Result<FileSnapshot<Self>, FileApplicationError> {
        services.read_doc(project, path)
    }
    fn apply_command(
        services: &ApplicationState,
        project: ProjectInstanceId,
        command: FileCommand<Self>,
    ) -> Result<FileMutation<Self>, FileApplicationError> {
        services.apply_doc_command(project, OperationId::new(), command)
    }
}

impl NativeFile for MindDocument {
    fn read(
        services: &ApplicationState,
        project: ProjectInstanceId,
        path: FilePath<Self>,
    ) -> Result<FileSnapshot<Self>, FileApplicationError> {
        services.read_mind(project, path)
    }
    fn apply_command(
        services: &ApplicationState,
        project: ProjectInstanceId,
        command: FileCommand<Self>,
    ) -> Result<FileMutation<Self>, FileApplicationError> {
        services.apply_mind_command(project, OperationId::new(), command)
    }
}

pub struct FileSaveRequest<T: NativeFile> {
    pub(crate) project: ProjectInstanceId,
    pub(crate) path: FilePath<T>,
    pub(crate) version: FileVersion,
    pub(crate) edits: Vec<T::Edit>,
}

pub struct FileSaveOutcome<T: FileContent> {
    pub(crate) snapshot: Option<FileSnapshot<T>>,
    pub(crate) failed: bool,
}

impl<T: FileContent> FileSaveOutcome<T> {
    pub fn new(snapshot: Option<FileSnapshot<T>>, failed: bool) -> Self {
        Self { snapshot, failed }
    }

    pub fn failed(&self) -> bool {
        self.failed
    }
}

impl<T: NativeFile> FileSaveRequest<T> {
    pub fn commit(self, owner: &ApplicationTasks) -> FileSaveOutcome<T> {
        let services = owner.application();
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
