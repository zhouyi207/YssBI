use crate::ProjectOperationError;
use crate::{PreparedProjectActivation, ProjectSession, ProjectState, ProjectTransactionContext};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use yss_filesystem::{
    FilesystemTransaction, NormalizedRoot, RootBinding, StagedFilesystemMutation, ensure_directory,
    read_file_inventory, remove_directory_if_created, validate_destination_policy,
};
use yss_project_identity::{OperationId, ProjectInstanceId};
use yss_project_layout::{CHART_EXTENSION, PROJECT_CONTENT_DIRECTORIES, PROJECT_METADATA_FILE};
use yss_project_model::file::{FileContent, FilePath, FileState};
use yss_project_model::{ProjectData, normalize_project_name};

pub struct PreparedProjectCopy {
    pub metadata_path: PathBuf,
    pub prepared_activation: PreparedProjectActivation,
}

impl std::fmt::Debug for PreparedProjectCopy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PreparedProjectCopy")
            .field("metadata_path", &self.metadata_path)
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct CreatedProject {
    pub metadata_path: PathBuf,
    pub project_name: String,
}

struct DestinationRootGuard {
    binding: RootBinding,
    remove_on_drop: bool,
}

impl DestinationRootGuard {
    fn ensure(binding: &RootBinding) -> Result<Self, ProjectOperationError> {
        let created = ensure_directory(binding.normalized().as_path())?;
        Ok(Self {
            binding: binding.bind_existing()?,
            remove_on_drop: created,
        })
    }

    fn disarm(&mut self) {
        self.remove_on_drop = false;
    }
}

impl Drop for DestinationRootGuard {
    fn drop(&mut self) {
        remove_directory_if_created(&self.binding, self.remove_on_drop);
    }
}

impl ProjectState {
    pub fn save_project_as_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        destination: &Path,
        operation_id: OperationId,
    ) -> Result<PreparedProjectCopy, ProjectOperationError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(stale("save-as project instance is stale"));
        }
        let basis_before = self
            .capture_prepared_authority_basis(&session.root)?
            .ok_or_else(|| stale("save-as source authority is no longer active"))?;
        let authority = self.coherent_project_read(&session, |data, _| data.clone())?;
        let authority_basis = self
            .capture_prepared_authority_basis(&session.root)?
            .filter(|basis| basis == &basis_before)
            .ok_or_else(|| stale("save-as authority changed during snapshot capture"))?;
        let destination_binding =
            RootBinding::for_destination(crate::project_root_from_path(destination))?;
        let destination_root = destination_binding.normalized().clone();
        if session.root == destination_root {
            return Err(invalid_root(
                destination,
                "save-as destination equals the source project",
            ));
        }
        validate_destination_policy(destination_root.as_path())?;

        let lease = self
            .filesystem()
            .acquire_many([session.root.clone(), destination_root.clone()]);
        self.validate_project_session(&session)?;
        if self.capture_prepared_authority_basis(&session.root)? != Some(authority_basis.clone()) {
            return Err(stale(
                "save-as authority changed while waiting for root leases",
            ));
        }
        validate_destination_policy(destination_root.as_path())?;
        let mut root_guard = DestinationRootGuard::ensure(&destination_binding)?;
        let mutations = copy_mutations(session.root.as_path(), &authority)?;
        let context = lifecycle_context(
            ProjectSession {
                instance_id: session.instance_id.clone(),
                root: destination_root.clone(),
            },
            operation_id,
            self,
        );
        let prepared = FilesystemTransaction::prepare_with_file_validator(
            context.filesystem_context(),
            lease,
            mutations,
            validate_project_copy_staged_file,
        )?;
        self.validate_project_session(&session)?;
        root_guard.binding.revalidate()?;
        let committed = prepared.commit()?;
        root_guard.binding.revalidate()?;
        self.validate_project_session(&session)?;
        if self.capture_prepared_authority_basis(&session.root)? != Some(authority_basis.clone()) {
            return Err(stale(
                "save-as authority changed before destination publication",
            ));
        }
        let activation_data = self.read_activation_data(&destination_root)?;
        let prepared_activation = PreparedProjectActivation::from_data(
            Some(destination_root.clone()),
            activation_data,
            Some(authority_basis),
            false,
        )?;
        committed.finalize();
        root_guard.disarm();
        Ok(PreparedProjectCopy {
            metadata_path: destination_root.as_path().join(PROJECT_METADATA_FILE),
            prepared_activation,
        })
    }

    pub fn create_project_transaction(
        &self,
        name: &str,
        destination: &Path,
        operation_id: OperationId,
    ) -> Result<CreatedProject, ProjectOperationError> {
        let project_name = normalize_project_name(name);
        let destination_binding =
            RootBinding::for_destination(crate::project_root_from_path(destination))?;
        let destination_root = destination_binding.normalized().clone();
        validate_destination_policy(destination_root.as_path())?;
        let lease = self.filesystem().acquire(destination_root.clone());
        validate_destination_policy(destination_root.as_path())?;
        let mut root_guard = DestinationRootGuard::ensure(&destination_binding)?;
        let mut data = ProjectData::new();
        data.metadata.project_name = project_name.clone();
        data.metadata.export_time = current_export_time();
        let context = lifecycle_context(
            ProjectSession {
                instance_id: ProjectInstanceId::new(),
                root: destination_root.clone(),
            },
            operation_id,
            self,
        );
        let prepared = FilesystemTransaction::prepare_with_validator(
            context.filesystem_context(),
            lease,
            new_project_mutations(&data)?,
            crate::project_writers::validate_document,
        )?;
        root_guard.binding.revalidate()?;
        let committed = prepared.commit()?;
        root_guard.binding.revalidate()?;
        yss_database_store::DatasetStore::create(destination_root.as_path())
            .map_err(ProjectOperationError::prepare_failed)?;
        committed.finalize();
        root_guard.disarm();
        Ok(CreatedProject {
            metadata_path: destination_root.as_path().join(PROJECT_METADATA_FILE),
            project_name,
        })
    }
}

fn current_export_time() -> String {
    chrono::Local::now()
        .naive_local()
        .format("%Y-%m-%dT%H:%M:%S%.3f")
        .to_string()
}

fn lifecycle_context(
    session: ProjectSession,
    operation_id: OperationId,
    state: &ProjectState,
) -> ProjectTransactionContext {
    ProjectTransactionContext {
        session,
        operation_id,
        affected_resources: Vec::new(),
        expected_revisions: BTreeMap::new(),
        expected_absent_resources: BTreeSet::new(),
        recovery_marker: Some(state.project_recovery_marker()),
    }
}

fn new_project_mutations(
    data: &ProjectData,
) -> Result<Vec<StagedFilesystemMutation>, ProjectOperationError> {
    let mut mutations = PROJECT_CONTENT_DIRECTORIES
        .into_iter()
        .map(create_directory)
        .collect::<Vec<_>>();
    mutations.extend([write_mutation(
        PROJECT_METADATA_FILE,
        crate::serialize_project_manifest(data).map_err(ProjectOperationError::prepare_failed)?,
    )]);
    Ok(mutations)
}

fn copy_mutations(
    source: &Path,
    authority: &ProjectData,
) -> Result<Vec<StagedFilesystemMutation>, ProjectOperationError> {
    let catalog = yss_database_store::DatasetStore::open(source)
        .and_then(|store| store.catalog_snapshot())
        .map_err(ProjectOperationError::prepare_failed)?;
    let source_tree = read_file_inventory(source)?;
    let mut directories = source_tree.directories;
    directories.extend(PROJECT_CONTENT_DIRECTORIES.map(PathBuf::from));
    let source_root = NormalizedRoot::from_path(crate::project_root_from_path(source))?;
    let mut files = source_tree
        .files
        .into_iter()
        .map(|relative_path| {
            let mutation = StagedFilesystemMutation::CopyFile {
                relative_path: relative_path.clone(),
                source_root: source_root.clone(),
                source_relative_path: relative_path.clone(),
            };
            (relative_path, mutation)
        })
        .collect::<BTreeMap<_, _>>();
    let catalog_path = crate::relative_dataset_catalog_path();
    files.remove(Path::new(&format!("{catalog_path}-wal")));
    files.remove(Path::new(&format!("{catalog_path}-shm")));
    files.insert(
        PathBuf::from(&catalog_path),
        write_mutation(catalog_path, catalog),
    );
    files.remove(Path::new(PROJECT_METADATA_FILE));
    files.insert(
        PathBuf::from(PROJECT_METADATA_FILE),
        write_mutation(
            PROJECT_METADATA_FILE,
            crate::serialize_project_manifest(authority)
                .map_err(ProjectOperationError::prepare_failed)?,
        ),
    );
    for graph_path in authority.graphs.keys() {
        let (path, contents) = crate::serialize_graph_document(authority, graph_path)
            .map_err(ProjectOperationError::prepare_failed)?;
        files.insert(path.clone(), write_mutation(path, contents));
    }
    resident_file_writes(&mut files, &authority.charts)?;
    resident_file_writes(&mut files, &authority.minds)?;
    resident_file_writes(&mut files, &authority.docs)?;
    let mut mutations = directories
        .into_iter()
        .map(|relative_path| StagedFilesystemMutation::CreateDirectory { relative_path })
        .collect::<Vec<_>>();
    mutations.extend(files.into_values());
    Ok(mutations)
}

// Resident bodies include unsaved edits; they replace the copied source files.
fn resident_file_writes<T: FileContent>(
    files: &mut BTreeMap<PathBuf, StagedFilesystemMutation>,
    resident: &std::collections::HashMap<FilePath<T>, FileState<T>>,
) -> Result<(), ProjectOperationError> {
    for (path, state) in resident {
        let path = PathBuf::from(path.as_str());
        let contents = state
            .content()
            .encode()
            .map_err(ProjectOperationError::prepare_failed)?;
        files.insert(path.clone(), write_mutation(path, contents));
    }
    Ok(())
}

fn validate_project_copy_staged_file(relative: &Path, staged: &Path) -> Result<(), String> {
    if relative == Path::new(PROJECT_METADATA_FILE)
        || matches!(
            relative
                .extension()
                .and_then(|extension| extension.to_str()),
            Some(
                "yssbi-event"
                    | "yssbi-function"
                    | CHART_EXTENSION
                    | yss_project_layout::MIND_EXTENSION
                    | yss_project_layout::DOC_EXTENSION
            )
        )
    {
        let contents = std::fs::read(staged).map_err(|error| error.to_string())?;
        crate::project_writers::validate_document(relative, &contents)
    } else {
        Ok(())
    }
}

fn create_directory(path: impl Into<PathBuf>) -> StagedFilesystemMutation {
    StagedFilesystemMutation::CreateDirectory {
        relative_path: path.into(),
    }
}

fn write_mutation(path: impl Into<PathBuf>, contents: Vec<u8>) -> StagedFilesystemMutation {
    StagedFilesystemMutation::Write {
        relative_path: path.into(),
        contents,
    }
}

fn invalid_root(path: impl AsRef<Path>, message: impl Into<String>) -> ProjectOperationError {
    ProjectOperationError::InvalidRoot {
        path: path.as_ref().to_path_buf(),
        message: message.into(),
    }
}

fn stale(message: impl Into<String>) -> ProjectOperationError {
    ProjectOperationError::StaleProjectLifecycle {
        message: message.into(),
    }
}
