use crate::event::{ResourceMutationResultDto, WorksheetDeltaDto};
use crate::graph::value::{DataType, DataValue};
use crate::node_system::document::{
    FunctionResourceKey, OperationId, ResourceKey, ResourceRevision, VariableResourceKey,
    WorksheetResourceKey,
};
use crate::project::{
    GraphDocument, GraphResourcePath, ProjectData, ProjectFilesystemError,
    ProjectFilesystemTransaction, ProjectInstanceId, ProjectSession, ProjectState,
    ProjectTransactionContext, ResourceDocumentPatch, StagedFilesystemMutation, WorksheetDocument,
};
use crate::tabular::VariableTabularCache;
use crate::variable::{VariableId, VariableInstance, VariableScope};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSaveResultDto {
    pub project_instance_id: String,
    pub operation_id: OperationId,
    pub publication_revision: u64,
    pub affected_resources: Vec<ResourceKey>,
    pub index_invalidated: bool,
    pub history: crate::node_system::document::HistoryStatusDto,
}

pub struct GlobalVariableMutationResult {
    pub variable: VariableInstance,
    pub result: ResourceMutationResultDto,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorksheetMutationResultDto {
    pub operation_id: OperationId,
    pub result: ResourceMutationResultDto,
    pub document: WorksheetDocument,
}

#[cfg(test)]
static WRITER_SNAPSHOT_TEST_HOOK: std::sync::Mutex<Option<std::sync::Arc<dyn Fn() + Send + Sync>>> =
    std::sync::Mutex::new(None);

#[cfg(test)]
fn set_writer_snapshot_test_hook(hook: Option<std::sync::Arc<dyn Fn() + Send + Sync>>) {
    *WRITER_SNAPSHOT_TEST_HOOK.lock().unwrap() = hook;
}

struct WriterSnapshot {
    session: ProjectSession,
    data: ProjectData,
    variable_revisions: std::collections::HashMap<crate::variable::VariableId, ResourceRevision>,
    authority_generation: u64,
}

enum GlobalVariableMutation {
    Create {
        name: String,
        data_type: DataType,
        data_value: DataValue,
        description: String,
        tags: Vec<String>,
    },
    Update {
        id: VariableId,
        name: Option<String>,
        data_type: Option<DataType>,
        data_value: Option<DataValue>,
        description: Option<String>,
        tags: Option<Vec<String>>,
    },
    Delete {
        id: VariableId,
    },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum GlobalVariableMutationKind {
    Create,
    Update,
    Delete,
}

struct StagedGlobalVariableMutation {
    kind: GlobalVariableMutationKind,

    variable: VariableInstance,
    cache: Option<VariableTabularCache>,
    expected_revision: Option<ResourceRevision>,
    history_patch: Option<crate::node_system::document::ResourcePatch>,
}

struct CommittedProjectSave {
    project_instance_id: String,
    operation_id: OperationId,
    publication_revision: u64,
    affected_resources: Vec<ResourceKey>,
    history: crate::node_system::document::HistoryStatusDto,
}

impl CommittedProjectSave {
    fn complete(self) -> ProjectSaveResultDto {
        ProjectSaveResultDto {
            project_instance_id: self.project_instance_id,
            operation_id: self.operation_id,
            publication_revision: self.publication_revision,
            affected_resources: self.affected_resources,
            index_invalidated: true,
            history: self.history,
        }
    }
}

fn graph_key(path: &GraphResourcePath) -> ResourceKey {
    ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
        path.as_str().into(),
    ))
}

fn function_key(path: &GraphResourcePath) -> ResourceKey {
    ResourceKey::Function(FunctionResourceKey(path.as_str().into()))
}

fn variable_key(id: &crate::variable::VariableId) -> ResourceKey {
    ResourceKey::Variable(VariableResourceKey(format!("variables/{id}").into()))
}

fn worksheet_key(id: &str) -> ResourceKey {
    ResourceKey::Worksheet(WorksheetResourceKey(id.into()))
}

fn context(
    state: &ProjectState,
    session: ProjectSession,
    operation_id: OperationId,
    expected_revisions: BTreeMap<ResourceKey, ResourceRevision>,
    expected_absent_resources: BTreeSet<ResourceKey>,
) -> ProjectTransactionContext {
    ProjectTransactionContext {
        affected_resources: expected_revisions.keys().cloned().collect(),
        session,
        operation_id,
        expected_revisions,
        expected_absent_resources,
        recovery_marker: Some(state.project_recovery_marker()),
    }
}

fn prepare_error(error: impl ToString) -> ProjectFilesystemError {
    ProjectFilesystemError::TransactionPrepareFailed {
        message: error.to_string(),
    }
}

fn validate_document(path: &Path, contents: &[u8]) -> Result<(), String> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("yssbi-event" | "yssbi-function") => serde_json::from_slice::<GraphDocument>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string()),
        Some("yssbi-vars") => {
            serde_json::from_slice::<crate::project::GlobalVariablesDocument>(contents)
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        Some("yssbi-worksheet") => serde_json::from_slice::<WorksheetDocument>(contents)
            .map(|_| ())
            .map_err(|error| error.to_string()),
        _ if path == Path::new(crate::project::PROJECT_METADATA_FILE) => {
            serde_json::from_slice::<crate::project::ProjectManifest>(contents)
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        _ => Err(format!(
            "unsupported project document target '{}'",
            path.display()
        )),
    }
}

fn unique_worksheet_name(existing: impl Iterator<Item = String>, requested: &str) -> String {
    let existing = existing.collect::<BTreeSet<_>>();
    let base = match requested.trim() {
        "" => "New Worksheet",
        value => value,
    };
    if !existing.contains(base) {
        return base.to_string();
    }
    (2..)
        .map(|index| format!("{base} {index}"))
        .find(|candidate| !existing.contains(candidate))
        .expect("worksheet name sequence is unbounded")
}

fn collect_worksheet_files(directory: &Path, paths: &mut Vec<PathBuf>) -> std::io::Result<()> {
    let directory_metadata = match std::fs::symlink_metadata(directory) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    reject_worksheet_redirect(directory, &directory_metadata)?;
    if !directory_metadata.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "worksheet scan path '{}' is not a directory",
                directory.display()
            ),
        ));
    }
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        let metadata = std::fs::symlink_metadata(&path)?;
        reject_worksheet_redirect(&path, &metadata)?;
        if metadata.is_dir() {
            collect_worksheet_files(&path, paths)?;
        } else if metadata.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("yssbi-worksheet")
        {
            paths.push(path);
        }
    }
    Ok(())
}

fn reject_worksheet_redirect(path: &Path, metadata: &std::fs::Metadata) -> std::io::Result<()> {
    #[cfg(windows)]
    let is_redirect = {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        metadata.file_type().is_symlink()
            || metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
    };
    #[cfg(not(windows))]
    let is_redirect = metadata.file_type().is_symlink();

    if is_redirect {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "worksheet scan rejects redirect/reparse path '{}'",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn worksheet_disk_path(
    root: &Path,
    worksheet_id: &str,
) -> Result<Option<PathBuf>, ProjectFilesystemError> {
    let mut paths = Vec::new();
    collect_worksheet_files(&root.join(crate::project::WORKSHEETS_DIR), &mut paths)
        .map_err(prepare_error)?;
    paths.sort();
    for path in paths {
        let contents = std::fs::read(&path).map_err(prepare_error)?;
        let document =
            serde_json::from_slice::<WorksheetDocument>(&contents).map_err(prepare_error)?;
        if document.id == worksheet_id {
            return path
                .strip_prefix(root)
                .map(Path::to_path_buf)
                .map(Some)
                .map_err(prepare_error);
        }
    }
    Ok(None)
}

impl ProjectState {
    fn capture_writer_snapshot(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
    ) -> Result<WriterSnapshot, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "writer project instance is stale".into(),
            });
        }
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != session.instance_id.as_str() {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project changed during writer snapshot".into(),
            });
        }
        let data = self.project_data.read().unwrap().clone();
        let variable_revisions = self.variable_revisions.read().unwrap().clone();
        let snapshot = WriterSnapshot {
            session,
            data,
            variable_revisions,
            authority_generation: publication.authority_generation(),
        };
        drop(publication);
        #[cfg(test)]
        if let Some(hook) = WRITER_SNAPSHOT_TEST_HOOK.lock().unwrap().clone() {
            hook();
        }
        Ok(snapshot)
    }

    fn validate_writer_context(
        &self,
        context: &ProjectTransactionContext,
        authority_generation: u64,
    ) -> Result<(), ProjectFilesystemError> {
        self.validate_project_session(&context.session)?;
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != context.session.instance_id.as_str()
            || publication.authority_generation() != authority_generation
        {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project authority changed while writer was waiting".into(),
            });
        }
        let data = self.project_data.read().unwrap();
        let graph_revisions = self.graph_revisions.read().unwrap();
        let variable_revisions = self.variable_revisions.read().unwrap();
        let worksheet_revisions = self.worksheet_revisions.read().unwrap();
        super::project_state::validate_context_revisions(
            context,
            &data,
            &graph_revisions,
            &variable_revisions,
            &worksheet_revisions,
        )
    }

    fn publish_project_save(
        &self,
        context: &ProjectTransactionContext,
        authority_generation: u64,
    ) -> Result<CommittedProjectSave, ProjectFilesystemError> {
        self.validate_writer_context(context, authority_generation)?;
        let publication = self.mutation_publication.lock().unwrap();
        let history = self.history.read().unwrap().status();
        Ok(CommittedProjectSave {
            project_instance_id: publication.project_instance_id.clone(),
            operation_id: context.operation_id,
            publication_revision: publication.resource_revision,
            affected_resources: context.affected_resources.clone(),
            history,
        })
    }

    fn execute_save(
        &self,
        snapshot: &WriterSnapshot,
        context: ProjectTransactionContext,
        mutations: Vec<StagedFilesystemMutation>,
    ) -> Result<ProjectSaveResultDto, ProjectFilesystemError> {
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        self.validate_writer_context(&context, snapshot.authority_generation)?;
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.clone(),
            lease,
            mutations,
            validate_document,
        )?;
        self.validate_writer_context(&context, snapshot.authority_generation)?;
        let committed = prepared.commit()?;
        let receipt = match self.publish_project_save(&context, snapshot.authority_generation) {
            Ok(receipt) => receipt,
            Err(error) => {
                return match committed.rollback() {
                    Ok(()) => Err(error),
                    Err(rollback_error) => Err(rollback_error),
                };
            }
        };
        committed.finalize();
        Ok(receipt.complete())
    }

    pub fn flush_project_documents(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        operation_id: OperationId,
    ) -> Result<ProjectSaveResultDto, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let mut expected = BTreeMap::new();
        let mut mutations = vec![
            StagedFilesystemMutation::Write {
                relative_path: crate::project::PROJECT_METADATA_FILE.into(),
                contents: crate::project::serialize_project_manifest(&snapshot.data)
                    .map_err(prepare_error)?,
            },
            StagedFilesystemMutation::Write {
                relative_path: crate::project::GLOBAL_VARIABLES_FILE.into(),
                contents: crate::project::serialize_global_variables(&snapshot.data)
                    .map_err(prepare_error)?,
            },
        ];
        for (id, variable) in &snapshot.data.variables {
            if matches!(variable.scope, VariableScope::Global) {
                expected.insert(
                    variable_key(id),
                    snapshot
                        .variable_revisions
                        .get(id)
                        .copied()
                        .unwrap_or(ResourceRevision::INITIAL),
                );
            }
        }
        let mut graph_paths = snapshot.data.graphs.keys().cloned().collect::<Vec<_>>();
        graph_paths.sort();
        for path in graph_paths {
            let resource = &snapshot.data.graphs[&path];
            expected.insert(graph_key(&path), resource.document.revision);
            if let Some(function) = &resource.function {
                if function.revision != resource.document.revision {
                    return Err(prepare_error(format!(
                        "function '{}' signature and graph revisions differ",
                        path
                    )));
                }
                expected.insert(function_key(&path), function.revision);
            }
            let (relative_path, contents) =
                crate::project::serialize_graph_document(&snapshot.data, &path)
                    .map_err(prepare_error)?;
            mutations.push(StagedFilesystemMutation::Write {
                relative_path,
                contents,
            });
        }
        let context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            expected,
            BTreeSet::new(),
        );
        self.execute_save(&snapshot, context, mutations)
    }

    pub fn save_graph_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        graph_path: &GraphResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<ProjectSaveResultDto, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let resource = snapshot
            .data
            .graphs
            .get(graph_path)
            .ok_or_else(|| prepare_error(format!("graph '{}' is not loaded", graph_path)))?;
        if resource.document.revision != expected_revision {
            return Err(ProjectFilesystemError::ResourceRevisionConflict {
                message: format!("graph '{}' revision changed", graph_path),
            });
        }
        let mut expected = BTreeMap::from([(graph_key(graph_path), expected_revision)]);
        if let Some(function) = &resource.function {
            if function.revision != expected_revision {
                return Err(ProjectFilesystemError::ResourceRevisionConflict {
                    message: format!(
                        "function '{}' signature revision differs from graph",
                        graph_path
                    ),
                });
            }
            expected.insert(function_key(graph_path), expected_revision);
        }
        let (relative_path, contents) =
            crate::project::serialize_graph_document(&snapshot.data, graph_path)
                .map_err(prepare_error)?;
        let context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            expected,
            BTreeSet::new(),
        );
        self.execute_save(
            &snapshot,
            context,
            vec![StagedFilesystemMutation::Write {
                relative_path,
                contents,
            }],
        )
    }

    #[cfg(test)]
    pub(crate) fn global_variable_revision_snapshot(
        &self,
    ) -> BTreeMap<ResourceKey, ResourceRevision> {
        let data = self.project_data.read().unwrap();
        let revisions = self.variable_revisions.read().unwrap();
        data.variables
            .iter()
            .filter(|(_, variable)| matches!(variable.scope, VariableScope::Global))
            .map(|(id, _)| {
                (
                    variable_key(id),
                    revisions
                        .get(id)
                        .copied()
                        .unwrap_or(ResourceRevision::INITIAL),
                )
            })
            .collect()
    }

    pub fn persist_global_variables(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        expected_revisions: BTreeMap<ResourceKey, ResourceRevision>,
        operation_id: OperationId,
    ) -> Result<ProjectSaveResultDto, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let authoritative = snapshot
            .data
            .variables
            .iter()
            .filter(|(_, variable)| matches!(variable.scope, VariableScope::Global))
            .map(|(id, _)| {
                (
                    variable_key(id),
                    snapshot
                        .variable_revisions
                        .get(id)
                        .copied()
                        .unwrap_or(ResourceRevision::INITIAL),
                )
            })
            .collect::<BTreeMap<_, _>>();
        if authoritative != expected_revisions {
            return Err(ProjectFilesystemError::ResourceRevisionConflict {
                message: "global variable revisions changed".into(),
            });
        }
        let context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            expected_revisions,
            BTreeSet::new(),
        );
        self.execute_save(
            &snapshot,
            context,
            vec![StagedFilesystemMutation::Write {
                relative_path: crate::project::GLOBAL_VARIABLES_FILE.into(),
                contents: crate::project::serialize_global_variables(&snapshot.data)
                    .map_err(prepare_error)?,
            }],
        )
    }

    fn stage_global_variable_mutation(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        expected_collection_revision: Option<u64>,
        expected_revision: Option<ResourceRevision>,
        operation_id: OperationId,
        mutation: GlobalVariableMutation,
    ) -> Result<GlobalVariableMutationResult, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "variable command project instance is stale".into(),
            });
        }
        let (authority_generation, mut globals, revisions, names) = {
            let publication = self.mutation_publication.lock().unwrap();
            if publication.project_instance_id != session.instance_id.as_str() {
                return Err(ProjectFilesystemError::StaleProjectLifecycle {
                    message: "project changed during variable staging".into(),
                });
            }
            if let Some(expected) = expected_collection_revision
                && publication.resource_revision != expected
            {
                return Err(ProjectFilesystemError::ResourceRevisionConflict {
                    message: format!(
                        "global variable collection expected revision {expected}, found {}",
                        publication.resource_revision
                    ),
                });
            }
            let data = self.project_data.read().unwrap();
            (
                publication.authority_generation(),
                data.variables
                    .iter()
                    .filter(|(_, variable)| matches!(variable.scope, VariableScope::Global))
                    .map(|(id, variable)| (*id, variable.clone()))
                    .collect::<std::collections::HashMap<_, _>>(),
                self.variable_revisions.read().unwrap().clone(),
                data.variables
                    .values()
                    .map(|variable| variable.name.clone())
                    .collect::<Vec<_>>(),
            )
        };
        let staged = match mutation {
            GlobalVariableMutation::Create {
                name,
                data_type,
                data_value,
                description,
                tags,
            } => {
                let variable = VariableInstance {
                    id: VariableId::new(),
                    name: super::unique_name::unique_name(
                        &name,
                        names.iter().map(String::as_str).collect::<Vec<_>>(),
                    ),
                    data_type,
                    data_value,
                    tabular: None,
                    description,
                    scope: VariableScope::Global,
                    tags,
                };
                let (variable, cache) = Self::stage_variable(variable)?;
                let history_patch = crate::node_system::document::ResourcePatch::variable(
                    VariableResourceKey(format!("variables/{}", variable.id).into()),
                    ResourceRevision::INITIAL,
                    crate::node_system::document::VariableDocumentPatch::new(
                        None,
                        Some(serde_json::to_value(&variable).map_err(prepare_error)?),
                    ),
                );
                StagedGlobalVariableMutation {
                    kind: GlobalVariableMutationKind::Create,
                    variable,
                    cache,
                    expected_revision: None,
                    history_patch: Some(history_patch),
                }
            }
            GlobalVariableMutation::Update {
                id,
                name,
                data_type,
                data_value,
                description,
                tags,
            } => {
                let before = globals
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| prepare_error(format!("variable '{id}' not found")))?;
                let mut variable = before.clone();
                if let Some(name) = name {
                    variable.name = name;
                }
                if let Some(data_type) = data_type {
                    let changed = variable.data_type != data_type;
                    variable.data_type = data_type;
                    if changed && data_value.is_none() {
                        variable.data_value = variable.data_type.default_value();
                    }
                }
                if let Some(data_value) = data_value {
                    variable.data_value = data_value;
                }
                if let Some(description) = description {
                    variable.description = description;
                }
                if let Some(tags) = tags {
                    variable.tags = tags;
                }
                let actual_revision = revisions
                    .get(&id)
                    .copied()
                    .unwrap_or(ResourceRevision::INITIAL);
                let expected_revision =
                    expected_revision.expect("update command requires revision");
                if actual_revision != expected_revision {
                    return Err(ProjectFilesystemError::ResourceRevisionConflict {
                        message: format!(
                            "variable 'variables/{id}' expected revision {}, found {}",
                            expected_revision.get(),
                            actual_revision.get()
                        ),
                    });
                }
                let history_patch = crate::node_system::document::ResourcePatch::variable(
                    VariableResourceKey(format!("variables/{id}").into()),
                    expected_revision,
                    crate::node_system::document::VariableDocumentPatch::new(
                        Some(serde_json::to_value(&before).map_err(prepare_error)?),
                        Some(serde_json::to_value(&variable).map_err(prepare_error)?),
                    ),
                );
                let (variable, cache) = Self::stage_variable(variable)?;
                StagedGlobalVariableMutation {
                    kind: GlobalVariableMutationKind::Update,
                    variable,
                    cache,
                    expected_revision: Some(expected_revision),
                    history_patch: Some(history_patch),
                }
            }
            GlobalVariableMutation::Delete { id } => {
                let variable = globals
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| prepare_error(format!("variable '{id}' not found")))?;
                let actual_revision = revisions
                    .get(&id)
                    .copied()
                    .unwrap_or(ResourceRevision::INITIAL);
                let expected_revision =
                    expected_revision.expect("delete command requires revision");
                if actual_revision != expected_revision {
                    return Err(ProjectFilesystemError::ResourceRevisionConflict {
                        message: format!(
                            "variable 'variables/{id}' expected revision {}, found {}",
                            expected_revision.get(),
                            actual_revision.get()
                        ),
                    });
                }
                let history_patch = crate::node_system::document::ResourcePatch::variable(
                    VariableResourceKey(format!("variables/{id}").into()),
                    expected_revision,
                    crate::node_system::document::VariableDocumentPatch::new(
                        Some(serde_json::to_value(&variable).map_err(prepare_error)?),
                        None,
                    ),
                );
                StagedGlobalVariableMutation {
                    kind: GlobalVariableMutationKind::Delete,
                    variable,
                    cache: None,
                    expected_revision: Some(expected_revision),
                    history_patch: Some(history_patch),
                }
            }
        };
        if staged.kind == GlobalVariableMutationKind::Delete {
            globals.remove(&staged.variable.id);
        } else {
            globals.insert(staged.variable.id, staged.variable.clone());
        }
        let key = variable_key(&staged.variable.id);
        let expected_revisions = staged
            .expected_revision
            .map(|revision| BTreeMap::from([(key.clone(), revision)]))
            .unwrap_or_default();
        let expected_absent_resources = if staged.kind == GlobalVariableMutationKind::Create {
            BTreeSet::from([key])
        } else {
            BTreeSet::new()
        };
        let context = context(
            self,
            session.clone(),
            operation_id,
            expected_revisions,
            expected_absent_resources,
        );
        let contents =
            crate::project::serialize_global_variable_map(globals).map_err(prepare_error)?;
        let lease = self.filesystem().acquire(session.root.clone())?;
        self.validate_writer_context(&context, authority_generation)?;
        if let Some(expected) = expected_collection_revision {
            let publication = self.mutation_publication.lock().unwrap();
            if publication.project_instance_id != session.instance_id.as_str()
                || publication.resource_revision != expected
            {
                return Err(ProjectFilesystemError::ResourceRevisionConflict {
                    message: format!(
                        "global variable collection expected revision {expected}, found {}",
                        publication.resource_revision
                    ),
                });
            }
        }
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.clone(),
            lease,
            vec![StagedFilesystemMutation::Write {
                relative_path: crate::project::GLOBAL_VARIABLES_FILE.into(),
                contents,
            }],
            validate_document,
        )?;
        self.validate_writer_context(&context, authority_generation)?;
        let committed = prepared.commit()?;
        let save =
            match self.publish_global_variable_mutation(&context, authority_generation, &staged) {
                Ok(save) => save,
                Err(error) => {
                    return match committed.rollback() {
                        Ok(()) => Err(error),
                        Err(rollback_error) => Err(rollback_error),
                    };
                }
            };
        committed.finalize();
        self.recompile_graphs_for_variable(&staged.variable.id);
        Ok(GlobalVariableMutationResult {
            variable: staged.variable,
            result: save,
        })
    }

    fn publish_global_variable_mutation(
        &self,
        context: &ProjectTransactionContext,
        authority_generation: u64,
        staged: &StagedGlobalVariableMutation,
    ) -> Result<ResourceMutationResultDto, ProjectFilesystemError> {
        let mut publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != context.session.instance_id.as_str()
            || publication.authority_generation() != authority_generation
        {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "project authority changed before variable publication".into(),
            });
        }
        let mut data = self.project_data.write().unwrap();
        let mut store = self.project_store.write().unwrap();
        let graph_revisions = self.graph_revisions.read().unwrap();
        let mut variable_revisions = self.variable_revisions.write().unwrap();
        let worksheet_revisions = self.worksheet_revisions.read().unwrap();
        let mut history = self.history.write().unwrap();
        super::project_state::validate_context_revisions(
            context,
            &data,
            &graph_revisions,
            &variable_revisions,
            &worksheet_revisions,
        )?;
        let id = staged.variable.id;
        match staged.kind {
            GlobalVariableMutationKind::Create => {
                data.variables.insert(id, staged.variable.clone());
                Self::publish_variable_cache(&mut store, &id, staged.cache.clone());
                variable_revisions.insert(id, ResourceRevision::new(1));
            }
            GlobalVariableMutationKind::Update => {
                data.variables.insert(id, staged.variable.clone());
                Self::publish_variable_cache(&mut store, &id, staged.cache.clone());
                variable_revisions.insert(id, staged.expected_revision.unwrap().next());
            }
            GlobalVariableMutationKind::Delete => {
                data.variables.remove(&id);
                variable_revisions.insert(id, staged.expected_revision.unwrap().next());
                crate::tabular::remove_variable_cache(&mut store, &id);
            }
        }
        if let Some(patch) = staged.history_patch.clone() {
            let crate::node_system::document::ResourceDocumentPatch::Variable(document_patch) =
                &patch.forward
            else {
                unreachable!("global variable mutation records a variable patch")
            };
            let ResourceKey::Variable(variable_key) = &patch.resource else {
                unreachable!("global variable mutation records a variable resource")
            };
            let variable_key = variable_key.clone();
            let before = document_patch.before.clone();
            let after = document_patch.after.clone();
            history.record_committed_transaction(
                crate::node_system::document::ProjectHistoryTransaction::durable_variable_effects(
                    context.operation_id,
                    vec![patch],
                    crate::node_system::document::VariableEffectHistorySnapshots {
                        before: BTreeMap::from([(variable_key.clone(), before)]),
                        after: BTreeMap::from([(variable_key, after)]),
                    },
                ),
            );
        }
        let history = history.status();
        let publication_revision = publication.allocate_resource_revision();
        let history_patch = staged
            .history_patch
            .as_ref()
            .expect("global mutations record history");
        Ok(ResourceMutationResultDto {
            project_instance_id: publication.project_instance_id.clone(),
            publication_revision,
            moves: Vec::new(),
            deltas: vec![crate::node_system::document::ResourceDeltaEvent {
                resource: history_patch.resource.clone(),
                from_revision: history_patch.before_revision,
                to_revision: history_patch.after_revision,
                caused_by: Some(context.operation_id),
                payload: history_patch.forward.clone(),
            }],
            worksheet_deltas: Vec::new(),
            projection_replacements: Vec::new(),
            projection_status: crate::event::ProjectionStatusDto::Complete {
                expected_graph_paths: Vec::new(),
            },
            history,
        })
    }

    pub fn create_global_variable_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        name: String,
        data_type: DataType,
        data_value: DataValue,
        description: String,
        tags: Vec<String>,
        expected_collection_revision: u64,
        operation_id: OperationId,
    ) -> Result<GlobalVariableMutationResult, ProjectFilesystemError> {
        self.stage_global_variable_mutation(
            expected_project_instance_id,
            Some(expected_collection_revision),
            None,
            operation_id,
            GlobalVariableMutation::Create {
                name,
                data_type,
                data_value,
                description,
                tags,
            },
        )
    }

    pub fn update_global_variable_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        id: VariableId,
        name: Option<String>,
        data_type: Option<DataType>,
        data_value: Option<DataValue>,
        description: Option<String>,
        tags: Option<Vec<String>>,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<GlobalVariableMutationResult, ProjectFilesystemError> {
        self.stage_global_variable_mutation(
            expected_project_instance_id,
            None,
            Some(expected_revision),
            operation_id,
            GlobalVariableMutation::Update {
                id,
                name,
                data_type,
                data_value,
                description,
                tags,
            },
        )
    }

    pub fn delete_global_variable_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        id: VariableId,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<GlobalVariableMutationResult, ProjectFilesystemError> {
        self.stage_global_variable_mutation(
            expected_project_instance_id,
            None,
            Some(expected_revision),
            operation_id,
            GlobalVariableMutation::Delete { id },
        )
    }

    pub fn create_worksheet_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        name: Option<String>,
        database_id: Option<String>,
        operation_id: OperationId,
    ) -> Result<WorksheetMutationResultDto, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        self.validate_writer_context(
            &context(
                self,
                snapshot.session.clone(),
                operation_id,
                BTreeMap::new(),
                BTreeSet::new(),
            ),
            snapshot.authority_generation,
        )?;
        let current = self.project_data.read().unwrap().clone();
        let unique = unique_worksheet_name(
            current
                .worksheets
                .values()
                .map(|worksheet| worksheet.name.clone()),
            name.as_deref().unwrap_or("New Worksheet"),
        );
        let default_database = database_id
            .or_else(|| current.databases.keys().min().cloned())
            .unwrap_or_default();
        let document = WorksheetDocument::new(unique, default_database);
        let key = worksheet_key(&document.id);
        let context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::new(),
            BTreeSet::from([key]),
        );
        self.write_worksheet_patch(snapshot, context, lease, None, document)
    }

    fn write_worksheet_patch(
        &self,
        snapshot: WriterSnapshot,
        context: ProjectTransactionContext,
        lease: crate::project::ProjectFilesystemLeaseSet,
        before: Option<WorksheetDocument>,
        document: WorksheetDocument,
    ) -> Result<WorksheetMutationResultDto, ProjectFilesystemError> {
        self.validate_writer_context(&context, snapshot.authority_generation)?;
        let old_path = worksheet_disk_path(snapshot.session.root.as_path(), &document.id)?;
        let (new_path, contents) =
            crate::project::serialize_worksheet(&document).map_err(prepare_error)?;
        let mut mutations = Vec::new();
        if let Some(old_path) = old_path {
            if old_path != new_path {
                mutations.push(StagedFilesystemMutation::RemoveFile {
                    relative_path: old_path,
                });
            }
        }
        mutations.push(StagedFilesystemMutation::Write {
            relative_path: new_path,
            contents,
        });
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.clone(),
            lease,
            mutations,
            validate_document,
        )?;
        self.validate_writer_context(&context, snapshot.authority_generation)?;
        let committed = prepared.commit()?;
        let mut result = match self.apply_resource_document_patch(
            &context,
            ResourceDocumentPatch::UpsertWorksheet {
                id: document.id.clone(),
                document: document.clone(),
            },
        ) {
            Ok(result) => result,
            Err(error) => {
                return match committed.rollback() {
                    Ok(()) => Err(error),
                    Err(rollback_error) => Err(rollback_error),
                };
            }
        };
        committed.finalize();
        result.worksheet_deltas = vec![WorksheetDeltaDto {
            id: document.id.clone(),
            before,
            after: Some(document.clone()),
        }];
        Ok(WorksheetMutationResultDto {
            operation_id: context.operation_id,
            result,
            document,
        })
    }

    pub fn save_worksheet_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        mut document: WorksheetDocument,
        operation_id: OperationId,
    ) -> Result<WorksheetMutationResultDto, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let before = snapshot.data.worksheets.get(&document.id).cloned();
        let expected = before
            .as_ref()
            .map(|_| document.revision)
            .ok_or_else(|| prepare_error(format!("worksheet '{}' does not exist", document.id)))?;
        document.revision = expected.next();
        let key = worksheet_key(&document.id);
        let context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::from([(key, expected)]),
            BTreeSet::new(),
        );
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        self.write_worksheet_patch(snapshot, context, lease, before, document)
    }

    pub fn delete_worksheet_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        worksheet_id: &str,
        operation_id: OperationId,
    ) -> Result<WorksheetMutationResultDto, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let document = snapshot
            .data
            .worksheets
            .get(worksheet_id)
            .cloned()
            .ok_or_else(|| prepare_error(format!("worksheet '{worksheet_id}' does not exist")))?;
        let key = worksheet_key(worksheet_id);
        let context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::from([(key, document.revision)]),
            BTreeSet::new(),
        );
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        self.validate_writer_context(&context, snapshot.authority_generation)?;
        let path = worksheet_disk_path(snapshot.session.root.as_path(), worksheet_id)?
            .unwrap_or_else(|| crate::project::worksheet_relative_path(&document));
        let prepared = ProjectFilesystemTransaction::prepare(
            context.clone(),
            lease,
            vec![StagedFilesystemMutation::RemoveFile {
                relative_path: path,
            }],
        )?;
        self.validate_writer_context(&context, snapshot.authority_generation)?;
        let committed = prepared.commit()?;
        let mut result = match self.apply_resource_document_patch(
            &context,
            ResourceDocumentPatch::RemoveWorksheet {
                id: worksheet_id.into(),
            },
        ) {
            Ok(result) => result,
            Err(error) => {
                return match committed.rollback() {
                    Ok(()) => Err(error),
                    Err(rollback_error) => Err(rollback_error),
                };
            }
        };
        committed.finalize();
        result.worksheet_deltas = vec![WorksheetDeltaDto {
            id: worksheet_id.into(),
            before: Some(document.clone()),
            after: None,
        }];
        Ok(WorksheetMutationResultDto {
            operation_id: context.operation_id,
            result,
            document,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{collect_worksheet_files, set_writer_snapshot_test_hook, worksheet_disk_path};
    use crate::graph::value::{DataType, DataValue};
    use crate::node_system::document::{
        FunctionResourceKey, OperationId, ResourceKey, ResourceRevision, VariableResourceKey,
    };
    use crate::project::{
        GraphDocument, GraphDocumentKind, GraphResourceDocument, GraphResourcePath, ProjectData,
        ProjectFilesystemFaultPoint, ProjectState, WorksheetDocument, set_project_filesystem_fault,
    };
    use crate::variable::VariableScope;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn worksheet_files(project: &TestProject) -> Vec<std::path::PathBuf> {
        let mut paths = Vec::new();
        collect_worksheet_files(
            &project.root.join(crate::project::WORKSHEETS_DIR),
            &mut paths,
        )
        .unwrap();
        paths.sort();
        paths
    }

    fn assert_two_distinct_worksheets_on_disk(
        project: &TestProject,
        first_id: &str,
        second_id: &str,
    ) {
        let files = worksheet_files(project);
        assert_eq!(
            files.len(),
            2,
            "each authoritative worksheet needs its own file"
        );
        let ids = files
            .iter()
            .map(|path| {
                serde_json::from_slice::<WorksheetDocument>(&std::fs::read(path).unwrap())
                    .unwrap()
                    .id
            })
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            ids,
            std::collections::BTreeSet::from([first_id.to_string(), second_id.to_string(),])
        );
    }

    struct TestProject {
        root: std::path::PathBuf,
    }

    impl TestProject {
        fn new(label: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "yssbi-project-writer-{label}-{}",
                uuid::Uuid::new_v4()
            ));
            std::fs::create_dir_all(&root).unwrap();
            Self { root }
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn active_state(project: &TestProject, data: ProjectData) -> ProjectState {
        let state = ProjectState::new();
        state.activate_project_fixture(project.root.to_string_lossy().into_owned(), data);
        state
    }

    fn graph_key(path: &GraphResourcePath) -> ResourceKey {
        ResourceKey::Graph(crate::node_system::document::GraphResourcePath(
            path.as_str().into(),
        ))
    }

    #[test]
    fn graph_save_revalidates_revision_after_waiting_for_rename() {
        let project = TestProject::new("graph-revision-wait");
        let path = GraphResourcePath::new("events/Before.yssbi-event").unwrap();
        let mut data = ProjectData::new();
        data.graphs.insert(
            path.clone(),
            GraphResourceDocument::new("Before", GraphDocumentKind::Event),
        );
        let state = Arc::new(active_state(&project, data));
        let session = state.capture_project_session().unwrap();
        let lease = state.filesystem().acquire(session.root.clone()).unwrap();
        let worker_state = Arc::clone(&state);
        let worker_path = path.clone();
        let worker_session = session.clone();
        let worker = std::thread::spawn(move || {
            worker_state.save_graph_document(
                &worker_session.instance_id,
                &worker_path,
                ResourceRevision::INITIAL,
                OperationId::new(),
            )
        });

        {
            let mut authority = state.project_data.write().unwrap();
            let graph = authority.graphs.get_mut(&path).unwrap();
            graph.name = "After".into();
            graph.document.revision = ResourceRevision::new(1);
        }
        drop(lease);

        let error = worker.join().unwrap().unwrap_err();
        assert_eq!(error.code(), "resource_revision_conflict");
        assert!(!project.root.join(path.as_str()).exists());
    }

    #[test]
    fn flush_writes_one_coherent_authoritative_snapshot_without_recreating_removed_graphs() {
        let project = TestProject::new("coherent-flush");
        let loaded = GraphResourcePath::new("events/Loaded.yssbi-event").unwrap();
        let removed = GraphResourcePath::new("events/Removed.yssbi-event").unwrap();
        let unknown = project.root.join("events/Unknown.yssbi-event");
        std::fs::create_dir_all(unknown.parent().unwrap()).unwrap();
        std::fs::write(&unknown, b"unknown-resource").unwrap();

        let mut data = ProjectData::new();
        data.metadata.project_name = "coherent-authority".into();
        data.graphs.insert(
            loaded.clone(),
            GraphResourceDocument::new("Loaded", GraphDocumentKind::Event),
        );
        data.graphs.insert(
            removed.clone(),
            GraphResourceDocument::new("Removed", GraphDocumentKind::Event),
        );
        crate::project::fixtures::write_graph(
            &data,
            project.root.to_string_lossy().as_ref(),
            &removed,
        )
        .unwrap();
        let state = Arc::new(active_state(&project, data));
        let session = state.capture_project_session().unwrap();
        let (captured_tx, captured_rx) = std::sync::mpsc::channel();
        let (resume_tx, resume_rx) = std::sync::mpsc::channel();
        let resume_rx = std::sync::Mutex::new(resume_rx);
        set_writer_snapshot_test_hook(Some(Arc::new(move || {
            captured_tx.send(()).unwrap();
            resume_rx.lock().unwrap().recv().unwrap();
        })));
        let worker_state = Arc::clone(&state);
        let worker_instance_id = session.instance_id.clone();
        let worker = std::thread::spawn(move || {
            worker_state.flush_project_documents(&worker_instance_id, OperationId::new())
        });
        captured_rx.recv().unwrap();
        state.unload_graph_resource(&removed).unwrap();
        std::fs::remove_file(project.root.join(removed.as_str())).unwrap();
        resume_tx.send(()).unwrap();
        let stale_error = worker.join().unwrap().unwrap_err();
        set_writer_snapshot_test_hook(None);
        assert_eq!(stale_error.code(), "stale_project_lifecycle");

        let result = state
            .flush_project_documents(&session.instance_id, OperationId::new())
            .unwrap();

        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(project.root.join(crate::project::PROJECT_METADATA_FILE)).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["projectName"], "coherent-authority");
        assert!(project.root.join(loaded.as_str()).is_file());
        assert!(!project.root.join(removed.as_str()).exists());
        assert_eq!(std::fs::read(unknown).unwrap(), b"unknown-resource");
        assert_eq!(result.project_instance_id, session.instance_id.as_str());
    }

    #[test]
    fn global_variable_writer_cannot_be_overwritten_by_rename_rollback() {
        let project = TestProject::new("global-narrow-write");
        let metadata = project.root.join(crate::project::PROJECT_METADATA_FILE);
        std::fs::write(&metadata, br#"{\"sentinel\":true}"#).unwrap();
        let graph_path = GraphResourcePath::new("events/Before.yssbi-event").unwrap();
        let mut data = ProjectData::new();
        data.graphs.insert(
            graph_path.clone(),
            GraphResourceDocument::new("Before", GraphDocumentKind::Event),
        );
        crate::project::fixtures::write_graph(
            &data,
            project.root.to_string_lossy().as_ref(),
            &graph_path,
        )
        .unwrap();
        let state = active_state(&project, data);
        let session = state.capture_project_session().unwrap();
        set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::SecondLiveReplacement));
        let rename_error = state
            .rename_graph_resource(session.instance_id.as_str(), &graph_path, "After")
            .unwrap_err();
        set_project_filesystem_fault(None);
        assert_eq!(rename_error.code(), "transaction_commit_failed");
        assert!(project.root.join(graph_path.as_str()).is_file());
        assert!(!project.root.join("events/After.yssbi-event").exists());

        let variable = state
            .add_variable(
                "global",
                DataType::Int64,
                DataValue::Int64(7),
                "",
                VariableScope::Global,
                vec![],
            )
            .unwrap();
        let key = ResourceKey::Variable(VariableResourceKey(
            format!("variables/{}", variable.id).into(),
        ));
        let result = state
            .persist_global_variables(
                &session.instance_id,
                BTreeMap::from([(key.clone(), ResourceRevision::INITIAL)]),
                OperationId::new(),
            )
            .unwrap();

        let globals: crate::project::GlobalVariablesDocument = serde_json::from_slice(
            &std::fs::read(project.root.join(crate::project::GLOBAL_VARIABLES_FILE)).unwrap(),
        )
        .unwrap();
        let persisted = globals.variables.get(&variable.id).unwrap();
        assert_eq!(persisted.name, variable.name);
        assert_eq!(persisted.data_value, variable.data_value);
        assert_eq!(std::fs::read(metadata).unwrap(), br#"{\"sentinel\":true}"#);
        assert_eq!(result.affected_resources, vec![key]);
    }

    #[test]
    fn function_save_persists_signature_and_graph_at_one_revision() {
        let project = TestProject::new("function-revision");
        let path = GraphResourcePath::new("functions/Shared.yssbi-function").unwrap();
        let mut function = GraphResourceDocument::new("Shared", GraphDocumentKind::Function);
        function.document.revision = ResourceRevision::new(4);
        function.function.as_mut().unwrap().revision = ResourceRevision::new(4);
        let mut data = ProjectData::new();
        data.graphs.insert(path.clone(), function);
        let state = active_state(&project, data);
        let session = state.capture_project_session().unwrap();
        let result = state
            .save_graph_document(
                &session.instance_id,
                &path,
                ResourceRevision::new(4),
                OperationId::new(),
            )
            .unwrap();

        let persisted: GraphDocument =
            serde_json::from_slice(&std::fs::read(project.root.join(path.as_str())).unwrap())
                .unwrap();
        assert_eq!(persisted.revision, ResourceRevision::new(4));
        assert_eq!(
            persisted.function.unwrap().revision,
            ResourceRevision::new(4)
        );
        assert_eq!(
            result.affected_resources,
            vec![
                graph_key(&path),
                ResourceKey::Function(FunctionResourceKey(path.as_str().into()))
            ]
        );
    }

    #[test]
    fn worksheet_create_rechecks_unique_name_under_root_lease() {
        let project = TestProject::new("worksheet-name-wait");
        let state = Arc::new(active_state(&project, ProjectData::new()));
        let session = state.capture_project_session().unwrap();
        let lease = state.filesystem().acquire(session.root.clone()).unwrap();
        let worker_state = Arc::clone(&state);
        let worker_session = session.clone();
        let worker = std::thread::spawn(move || {
            worker_state.create_worksheet_document(
                &worker_session.instance_id,
                Some("Analysis".into()),
                None,
                OperationId::new(),
            )
        });

        let existing = WorksheetDocument::new("Analysis", "");
        state
            .project_data
            .write()
            .unwrap()
            .worksheets
            .insert(existing.id.clone(), existing);
        drop(lease);

        let created = worker.join().unwrap().unwrap();
        assert_eq!(created.document.name, "Analysis 2");
    }

    #[test]
    fn worksheet_create_duplicate_name_keeps_distinct_authority_and_disk_documents() {
        let project = TestProject::new("worksheet-create-duplicate");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();

        let first = state
            .create_worksheet_document(
                &session.instance_id,
                Some("Report".into()),
                None,
                OperationId::new(),
            )
            .unwrap();
        let second = state
            .create_worksheet_document(
                &session.instance_id,
                Some("Report".into()),
                None,
                OperationId::new(),
            )
            .unwrap();

        assert_ne!(first.document.id, second.document.id);
        assert_two_distinct_worksheets_on_disk(&project, &first.document.id, &second.document.id);
    }

    #[test]
    fn worksheet_save_duplicate_name_never_overwrites_another_worksheet() {
        let project = TestProject::new("worksheet-save-duplicate");
        let mut first = WorksheetDocument::new("Report", "database");
        let mut second = WorksheetDocument::new("Other", "database");
        let first_id = first.id.clone();
        let second_id = second.id.clone();
        let mut data = ProjectData::new();
        data.worksheets.insert(first_id.clone(), first.clone());
        data.worksheets.insert(second_id.clone(), second.clone());
        let state = active_state(&project, data);
        crate::project::fixtures::write_worksheet(&project.root, &first).unwrap();
        crate::project::fixtures::write_worksheet(&project.root, &second).unwrap();
        state.initialize_worksheet_revision_for_test(&first_id);
        state.initialize_worksheet_revision_for_test(&second_id);
        let session = state.capture_project_session().unwrap();

        second.name = first.name.clone();
        state
            .save_worksheet_document(&session.instance_id, second, OperationId::new())
            .unwrap();

        assert_two_distinct_worksheets_on_disk(&project, &first_id, &second_id);
        let first_path = worksheet_disk_path(&project.root, &first_id)
            .unwrap()
            .unwrap();
        first =
            serde_json::from_slice(&std::fs::read(project.root.join(first_path)).unwrap()).unwrap();
        assert_eq!(first.id, first_id);
        assert_eq!(first.name, "Report");
    }

    #[test]
    fn worksheet_sanitized_name_collision_never_overwrites_another_worksheet() {
        let project = TestProject::new("worksheet-sanitize-collision");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();

        let first = state
            .create_worksheet_document(
                &session.instance_id,
                Some("A/B".into()),
                None,
                OperationId::new(),
            )
            .unwrap();
        let second = state
            .create_worksheet_document(
                &session.instance_id,
                Some("A\\B".into()),
                None,
                OperationId::new(),
            )
            .unwrap();

        assert_two_distinct_worksheets_on_disk(&project, &first.document.id, &second.document.id);
    }

    #[cfg(windows)]
    #[test]
    fn worksheet_windows_casefold_collision_never_overwrites_another_worksheet() {
        let project = TestProject::new("worksheet-casefold-collision");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();

        let first = state
            .create_worksheet_document(
                &session.instance_id,
                Some("Report".into()),
                None,
                OperationId::new(),
            )
            .unwrap();
        let second = state
            .create_worksheet_document(
                &session.instance_id,
                Some("report".into()),
                None,
                OperationId::new(),
            )
            .unwrap();

        assert_two_distinct_worksheets_on_disk(&project, &first.document.id, &second.document.id);
    }

    #[cfg(windows)]
    fn create_test_junction(link: &std::path::Path, target: &std::path::Path) -> bool {
        std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                link.to_string_lossy().as_ref(),
                target.to_string_lossy().as_ref(),
            ])
            .status()
            .is_ok_and(|status| status.success())
    }

    #[cfg(windows)]
    #[test]
    fn worksheet_scan_rejects_external_directory_junction_before_reading() {
        let project = TestProject::new("worksheet-external-junction");
        let external = TestProject::new("worksheet-external-target");
        let external_worksheets = external.root.join("documents");
        std::fs::create_dir_all(&external_worksheets).unwrap();
        std::fs::write(
            external_worksheets.join("secret.yssbi-worksheet"),
            br#"{"not":"a worksheet"}"#,
        )
        .unwrap();
        let worksheets = project.root.join(crate::project::WORKSHEETS_DIR);
        std::fs::create_dir_all(&worksheets).unwrap();
        if !create_test_junction(&worksheets.join("external"), &external_worksheets) {
            eprintln!("skipping junction assertion: Windows junction creation is unavailable");
            return;
        }

        let error = worksheet_disk_path(&project.root, "missing").unwrap_err();
        assert_eq!(error.code(), "transaction_prepare_failed");
        let message = error.to_string().to_ascii_lowercase();
        assert!(message.contains("reparse") || message.contains("redirect"));
    }

    #[cfg(windows)]
    #[test]
    fn worksheet_scan_rejects_directory_junction_loop_without_recursing() {
        let project = TestProject::new("worksheet-junction-loop");
        let worksheets = project.root.join(crate::project::WORKSHEETS_DIR);
        std::fs::create_dir_all(&worksheets).unwrap();
        if !create_test_junction(&worksheets.join("loop"), &worksheets) {
            eprintln!("skipping junction assertion: Windows junction creation is unavailable");
            return;
        }

        let error = worksheet_disk_path(&project.root, "missing").unwrap_err();
        assert_eq!(error.code(), "transaction_prepare_failed");
        let message = error.to_string().to_ascii_lowercase();
        assert!(message.contains("reparse") || message.contains("redirect"));
    }

    #[cfg(unix)]
    #[test]
    fn worksheet_scan_rejects_external_directory_symlink_before_reading() {
        use std::os::unix::fs::symlink;

        let project = TestProject::new("worksheet-external-symlink");
        let external = TestProject::new("worksheet-external-target");
        let external_worksheets = external.root.join("documents");
        std::fs::create_dir_all(&external_worksheets).unwrap();
        std::fs::write(
            external_worksheets.join("secret.yssbi-worksheet"),
            br#"{"not":"a worksheet"}"#,
        )
        .unwrap();
        let worksheets = project.root.join(crate::project::WORKSHEETS_DIR);
        std::fs::create_dir_all(&worksheets).unwrap();
        symlink(&external_worksheets, worksheets.join("external")).unwrap();

        let error = worksheet_disk_path(&project.root, "missing").unwrap_err();
        assert_eq!(error.code(), "transaction_prepare_failed");
        assert!(
            error
                .to_string()
                .to_ascii_lowercase()
                .contains("symbolic link")
        );
    }

    #[cfg(unix)]
    #[test]
    fn worksheet_scan_rejects_directory_symlink_loop_without_recursing() {
        use std::os::unix::fs::symlink;

        let project = TestProject::new("worksheet-symlink-loop");
        let worksheets = project.root.join(crate::project::WORKSHEETS_DIR);
        std::fs::create_dir_all(&worksheets).unwrap();
        symlink(&worksheets, worksheets.join("loop")).unwrap();

        let error = worksheet_disk_path(&project.root, "missing").unwrap_err();
        assert_eq!(error.code(), "transaction_prepare_failed");
        assert!(
            error
                .to_string()
                .to_ascii_lowercase()
                .contains("symbolic link")
        );
    }

    #[test]
    fn worksheet_commit_failure_restores_file_and_nested_directory_topology() {
        let project = TestProject::new("worksheet-rollback");
        let nested = project.root.join("worksheets/nested/deeper");
        std::fs::create_dir_all(&nested).unwrap();
        let mut document = WorksheetDocument::new("Original", "database");
        let old_path = nested.join("Original.yssbi-worksheet");
        std::fs::write(&old_path, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
        let mut data = ProjectData::new();
        data.worksheets
            .insert(document.id.clone(), document.clone());
        let state = active_state(&project, data);
        let session = state.capture_project_session().unwrap();
        document.name = "Renamed".into();
        set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::SecondLiveReplacement));

        let error = state
            .save_worksheet_document(&session.instance_id, document, OperationId::new())
            .unwrap_err();
        set_project_filesystem_fault(None);

        assert_eq!(error.code(), "transaction_commit_failed");
        assert!(old_path.is_file());
        assert!(nested.is_dir());
        assert!(
            !project
                .root
                .join("worksheets/Renamed.yssbi-worksheet")
                .exists()
        );
    }

    #[test]
    fn stale_writer_emits_no_result_or_event() {
        let project = TestProject::new("stale-writer");
        let state = active_state(&project, ProjectData::new());
        let stale = state.capture_project_session().unwrap();
        state.activate_project_fixture(
            project.root.to_string_lossy().into_owned(),
            ProjectData::new(),
        );

        let mut events = Vec::new();
        let error = crate::commands::command_project::lifecycle::flush_project_with_emitter(
            &state,
            stale.instance_id,
            OperationId::new(),
            |event| events.push(event),
        )
        .unwrap_err();
        assert_eq!(error.code, "stale_project_lifecycle");
        assert!(events.is_empty());
    }
}
