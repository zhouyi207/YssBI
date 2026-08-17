use crate::event::ResourceMutationResultDto;
use crate::graph::value::{DataType, DataValue};
use crate::node_system::document::{
    FunctionResourceKey, OperationId, ResourceKey, ResourceRevision, VariableResourceKey,
    WorksheetResourceKey,
};
use crate::project::{
    GraphDocument, GraphResourcePath, ProjectData, ProjectFilesystemError,
    ProjectFilesystemTransaction, ProjectInstanceId, ProjectSession, ProjectState,
    ProjectTransactionContext, ResourceDocumentPatch, ResourceName, StagedFilesystemMutation,
    WorksheetDocument, WorksheetResourcePath, allocate_unique_resource_name,
};
use crate::tabular::VariableTabularCache;
use crate::variable::{VariableId, VariableInstance, VariableScope};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

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

fn worksheet_document_state(
    document: &WorksheetDocument,
) -> crate::node_system::document::WorksheetDocumentState {
    crate::node_system::document::WorksheetDocumentState {
        database_id: document.database_id.clone(),
        chart_type: document.chart_type.clone(),
        encodings: document.encodings.clone(),
    }
}

fn worksheet_lifecycle_state(
    path: &WorksheetResourcePath,
    revision: ResourceRevision,
) -> crate::node_system::document::ResourceLifecycleState {
    crate::node_system::document::ResourceLifecycleState {
        revision,
        path: path.as_str().into(),
        kind: crate::node_system::document::ResourceLifecycleKind::Worksheet,
        name: path.display_name().as_str().to_string(),
    }
}

fn worksheet_move_delta(
    from: &WorksheetResourcePath,
    to: &WorksheetResourcePath,
    operation_id: OperationId,
    from_revision: ResourceRevision,
    to_revision: ResourceRevision,
) -> crate::node_system::document::ResourceDeltaEvent {
    crate::node_system::document::ResourceDeltaEvent {
        resource: worksheet_key(to),
        from_revision,
        to_revision,
        caused_by: Some(operation_id),
        payload: crate::node_system::document::ResourceDocumentPatch::ResourceMove(
            crate::node_system::document::ResourcePathMovePatch {
                from: from.as_str().into(),
                to: to.as_str().into(),
            },
        ),
    }
}

fn worksheet_resource_delta(
    path: &WorksheetResourcePath,
    operation_id: OperationId,
    retained_revision: Option<ResourceRevision>,
    before: Option<&WorksheetDocument>,
    after: Option<&WorksheetDocument>,
) -> crate::node_system::document::ResourceDeltaEvent {
    let (from_revision, to_revision, payload) = match (before, after) {
        (Some(before), Some(after)) => (
            before.revision,
            after.revision,
            crate::node_system::document::ResourceDocumentPatch::Worksheet(
                crate::node_system::document::WorksheetDocumentPatch {
                    before: worksheet_document_state(before),
                    after: worksheet_document_state(after),
                },
            ),
        ),
        (None, Some(after)) => (
            retained_revision.unwrap_or(after.revision),
            after.revision,
            crate::node_system::document::ResourceDocumentPatch::ResourceLifecycle(
                crate::node_system::document::ResourceLifecyclePatch {
                    before: None,
                    after: Some(worksheet_lifecycle_state(path, after.revision)),
                },
            ),
        ),
        (Some(before), None) => (
            before.revision,
            before.revision.next(),
            crate::node_system::document::ResourceDocumentPatch::ResourceLifecycle(
                crate::node_system::document::ResourceLifecyclePatch {
                    before: Some(worksheet_lifecycle_state(path, before.revision)),
                    after: None,
                },
            ),
        ),
        (None, None) => unreachable!("a worksheet resource delta must change a document"),
    };
    crate::node_system::document::ResourceDeltaEvent {
        resource: worksheet_key(path),
        from_revision,
        to_revision,
        caused_by: Some(operation_id),
        payload,
    }
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
    variable_revisions: std::collections::HashMap<
        crate::variable::VariableId,
        crate::project::project_state::VariableRevisionEntry,
    >,
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

fn worksheet_key(path: &WorksheetResourcePath) -> ResourceKey {
    ResourceKey::Worksheet(WorksheetResourceKey(path.as_str().into()))
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
                        .map(|entry| entry.revision)
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
        let mut worksheet_paths = snapshot.data.worksheets.keys().cloned().collect::<Vec<_>>();
        worksheet_paths.sort();
        for path in worksheet_paths {
            let document = &snapshot.data.worksheets[&path];
            expected.insert(worksheet_key(&path), document.revision);
            let (relative_path, contents) =
                crate::project::serialize_worksheet(&path, document).map_err(prepare_error)?;
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
                        .map(|entry| entry.revision)
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
                        .map(|entry| entry.revision)
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
                    .map(|entry| entry.revision)
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
                    .map(|entry| entry.revision)
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
        #[cfg(test)]
        if let Some(hook) = WRITER_SNAPSHOT_TEST_HOOK.lock().unwrap().clone() {
            hook();
        }
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
                variable_revisions.insert(
                    id,
                    crate::project::project_state::VariableRevisionEntry::present(
                        ResourceRevision::new(1),
                    ),
                );
            }
            GlobalVariableMutationKind::Update => {
                data.variables.insert(id, staged.variable.clone());
                Self::publish_variable_cache(&mut store, &id, staged.cache.clone());
                variable_revisions.insert(
                    id,
                    crate::project::project_state::VariableRevisionEntry::present(
                        staged.expected_revision.unwrap().next(),
                    ),
                );
            }
            GlobalVariableMutationKind::Delete => {
                data.variables.remove(&id);
                variable_revisions.insert(
                    id,
                    crate::project::project_state::VariableRevisionEntry::deleted(
                        staged.expected_revision.unwrap().next(),
                    ),
                );
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
            operation_id: context.operation_id,
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
            projection_replacements: Vec::new(),
            projection_status: crate::event::ProjectionStatusDto::Complete {
                expected_graph_paths: Vec::new(),
            },
            history,
        })
    }

    fn commit_local_variable_mutation(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        expected_collection_revision: Option<u64>,
        expected_revision: Option<ResourceRevision>,
        operation_id: OperationId,
        mutation: GlobalVariableMutation,
        scope: Option<VariableScope>,
    ) -> Result<GlobalVariableMutationResult, ProjectFilesystemError> {
        let session = self.capture_project_session()?;
        if &session.instance_id != expected_project_instance_id {
            return Err(ProjectFilesystemError::StaleProjectLifecycle {
                message: "variable command project instance is stale".into(),
            });
        }
        let reservation = self.reserve_resource_operation(&session.instance_id, operation_id)?;
        let (authority_generation, revisions, names, current) = {
            let publication = self.mutation_publication.lock().unwrap();
            if publication.project_instance_id != session.instance_id.as_str() {
                return Err(ProjectFilesystemError::StaleProjectLifecycle {
                    message: "project changed during local variable staging".into(),
                });
            }
            if let Some(expected) = expected_collection_revision
                && publication.resource_revision != expected
            {
                return Err(ProjectFilesystemError::ResourceRevisionConflict {
                    message: format!(
                        "local variable collection expected revision {expected}, found {}",
                        publication.resource_revision
                    ),
                });
            }
            let data = self.project_data.read().unwrap();
            let current = match &mutation {
                GlobalVariableMutation::Create { .. } => None,
                GlobalVariableMutation::Update { id, .. }
                | GlobalVariableMutation::Delete { id } => data.variables.get(id).cloned(),
            };
            (
                publication.authority_generation(),
                self.variable_revisions.read().unwrap().clone(),
                data.variables
                    .values()
                    .map(|variable| variable.name.clone())
                    .collect::<Vec<_>>(),
                current,
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
                    scope: scope.expect("local create requires scope"),
                    tags,
                };
                let (variable, cache) = Self::stage_variable(variable)?;
                let patch = crate::node_system::document::ResourcePatch::variable(
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
                    history_patch: Some(patch),
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
                let before =
                    current.ok_or_else(|| prepare_error(format!("variable '{id}' not found")))?;
                if matches!(before.scope, VariableScope::Global) {
                    return Err(prepare_error(format!("variable '{id}' is not local")));
                }
                let actual_revision = revisions
                    .get(&id)
                    .map(|entry| entry.revision)
                    .unwrap_or(ResourceRevision::INITIAL);
                let expected_revision = expected_revision.expect("update requires revision");
                if actual_revision != expected_revision {
                    return Err(ProjectFilesystemError::ResourceRevisionConflict {
                        message: format!(
                            "variable 'variables/{id}' expected revision {}, found {}",
                            expected_revision.get(),
                            actual_revision.get()
                        ),
                    });
                }
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
                let patch = crate::node_system::document::ResourcePatch::variable(
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
                    history_patch: Some(patch),
                }
            }
            GlobalVariableMutation::Delete { id } => {
                let variable =
                    current.ok_or_else(|| prepare_error(format!("variable '{id}' not found")))?;
                if matches!(variable.scope, VariableScope::Global) {
                    return Err(prepare_error(format!("variable '{id}' is not local")));
                }
                let actual_revision = revisions
                    .get(&id)
                    .map(|entry| entry.revision)
                    .unwrap_or(ResourceRevision::INITIAL);
                let expected_revision = expected_revision.expect("delete requires revision");
                if actual_revision != expected_revision {
                    return Err(ProjectFilesystemError::ResourceRevisionConflict {
                        message: format!(
                            "variable 'variables/{id}' expected revision {}, found {}",
                            expected_revision.get(),
                            actual_revision.get()
                        ),
                    });
                }
                let patch = crate::node_system::document::ResourcePatch::variable(
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
                    history_patch: Some(patch),
                }
            }
        };
        let key = variable_key(&staged.variable.id);
        let context = context(
            self,
            session,
            operation_id,
            staged
                .expected_revision
                .map(|revision| BTreeMap::from([(key.clone(), revision)]))
                .unwrap_or_default(),
            if staged.kind == GlobalVariableMutationKind::Create {
                BTreeSet::from([key])
            } else {
                BTreeSet::new()
            },
        );
        let result =
            self.publish_global_variable_mutation(&context, authority_generation, &staged)?;
        reservation.complete();
        Ok(GlobalVariableMutationResult {
            variable: staged.variable,
            result,
        })
    }

    pub fn create_local_variable_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        name: String,
        data_type: DataType,
        data_value: DataValue,
        description: String,
        scope: VariableScope,
        tags: Vec<String>,
        expected_collection_revision: u64,
        operation_id: OperationId,
    ) -> Result<GlobalVariableMutationResult, ProjectFilesystemError> {
        self.commit_local_variable_mutation(
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
            Some(scope),
        )
    }

    pub fn update_local_variable_transaction(
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
        self.commit_local_variable_mutation(
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
            None,
        )
    }

    pub fn delete_local_variable_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        id: VariableId,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<GlobalVariableMutationResult, ProjectFilesystemError> {
        self.commit_local_variable_mutation(
            expected_project_instance_id,
            None,
            Some(expected_revision),
            operation_id,
            GlobalVariableMutation::Delete { id },
            None,
        )
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

    pub fn create_worksheet_resource_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        name: &ResourceName,
        database_id: Option<String>,
        operation_id: OperationId,
    ) -> Result<ResourceMutationResultDto, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        let empty_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::new(),
            BTreeSet::new(),
        );
        self.validate_writer_context(&empty_context, snapshot.authority_generation)?;
        let current = self.project_data.read().unwrap().clone();
        let existing = current
            .worksheets
            .keys()
            .map(WorksheetResourcePath::display_name)
            .collect::<Vec<_>>();
        let unique = allocate_unique_resource_name(name, existing);
        let worksheet_path = WorksheetResourcePath::from_name(&unique);
        let mut document = WorksheetDocument::new(
            database_id
                .or_else(|| current.databases.keys().min().cloned())
                .unwrap_or_default(),
        );
        document.revision = self
            .worksheet_revisions
            .read()
            .unwrap()
            .get(&worksheet_path)
            .copied()
            .map(ResourceRevision::next)
            .unwrap_or(ResourceRevision::INITIAL);
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::new(),
            BTreeSet::from([worksheet_key(&worksheet_path)]),
        );
        let result = self.write_worksheet_patch(
            &snapshot,
            mutation_context,
            lease,
            worksheet_path,
            None,
            document,
        );
        if result.is_ok() {
            reservation.complete();
        }
        result
    }

    pub fn duplicate_worksheet_resource_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        source: &WorksheetResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<ResourceMutationResultDto, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        let current = self.project_data.read().unwrap().clone();
        let source_document = current.worksheets.get(source).cloned().ok_or_else(|| {
            ProjectFilesystemError::WorksheetNotFound {
                path: source.clone(),
            }
        })?;
        let existing = current
            .worksheets
            .keys()
            .map(WorksheetResourcePath::display_name)
            .collect::<Vec<_>>();
        let unique = allocate_unique_resource_name(source.display_name(), existing);
        let target = WorksheetResourcePath::from_name(&unique);
        let mut duplicate = source_document;
        duplicate.revision = self
            .worksheet_revisions
            .read()
            .unwrap()
            .get(&target)
            .copied()
            .map(ResourceRevision::next)
            .unwrap_or(ResourceRevision::INITIAL);
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::from([(worksheet_key(source), expected_revision)]),
            BTreeSet::from([worksheet_key(&target)]),
        );
        let result =
            self.write_worksheet_patch(&snapshot, mutation_context, lease, target, None, duplicate);
        if result.is_ok() {
            reservation.complete();
        }
        result
    }

    fn write_worksheet_patch(
        &self,
        snapshot: &WriterSnapshot,
        context: ProjectTransactionContext,
        lease: crate::project::ProjectFilesystemLeaseSet,
        worksheet_path: WorksheetResourcePath,
        before: Option<WorksheetDocument>,
        document: WorksheetDocument,
    ) -> Result<ResourceMutationResultDto, ProjectFilesystemError> {
        self.validate_writer_context(&context, snapshot.authority_generation)?;
        let retained_revision = self
            .worksheet_revisions
            .read()
            .unwrap()
            .get(&worksheet_path)
            .copied();
        let (new_path, contents) = crate::project::serialize_worksheet(&worksheet_path, &document)
            .map_err(prepare_error)?;
        let prepared = ProjectFilesystemTransaction::prepare_with_validator(
            context.clone(),
            lease,
            vec![StagedFilesystemMutation::Write {
                relative_path: new_path,
                contents,
            }],
            validate_document,
        )?;
        self.validate_writer_context(&context, snapshot.authority_generation)?;
        let projection_environment = self
            .capture_projection_environment_for_session(&snapshot.session)
            .map_err(|message| ProjectFilesystemError::TransactionPrepareFailed { message })?;
        let committed = prepared.commit()?;
        let mut result = match self.apply_resource_document_patch_with_environment(
            &context,
            ResourceDocumentPatch::UpsertWorksheet {
                path: worksheet_path.clone(),
                document: document.clone(),
            },
            projection_environment,
            None,
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
        result.deltas = vec![worksheet_resource_delta(
            &worksheet_path,
            context.operation_id,
            retained_revision,
            before.as_ref(),
            Some(&document),
        )];
        Ok(result)
    }

    pub fn save_worksheet_document(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        worksheet_path: &WorksheetResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
        mut document: WorksheetDocument,
    ) -> Result<ResourceMutationResultDto, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        let before = self
            .project_data
            .read()
            .unwrap()
            .worksheets
            .get(worksheet_path)
            .cloned()
            .ok_or_else(|| ProjectFilesystemError::WorksheetNotFound {
                path: worksheet_path.clone(),
            })?;
        document.revision = expected_revision.next();
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::from([(worksheet_key(worksheet_path), expected_revision)]),
            BTreeSet::new(),
        );
        let result = self.write_worksheet_patch(
            &snapshot,
            mutation_context,
            lease,
            worksheet_path.clone(),
            Some(before),
            document,
        );
        if result.is_ok() {
            reservation.complete();
        }
        result
    }

    pub fn rename_worksheet_resource_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        worksheet_path: &WorksheetResourcePath,
        expected_revision: ResourceRevision,
        new_name: &ResourceName,
        lifecycle_token: u64,
        operation_id: OperationId,
    ) -> Result<ResourceMutationResultDto, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let mut ownership = self.acquire_resource_rename_ownership(
            expected_project_instance_id,
            crate::project::LifecycleResourcePath::Worksheet(worksheet_path.clone()),
            lifecycle_token,
        )?;
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        self.validate_writer_context(
            &context(
                self,
                snapshot.session.clone(),
                operation_id,
                BTreeMap::from([(worksheet_key(worksheet_path), expected_revision)]),
                BTreeSet::new(),
            ),
            snapshot.authority_generation,
        )?;
        self.validate_resource_lifecycle_operation(&ownership.operation)?;

        let target = WorksheetResourcePath::from_name(new_name);
        let current = self.project_data.read().unwrap().clone();
        let mut moved = current
            .worksheets
            .get(worksheet_path)
            .cloned()
            .ok_or_else(|| ProjectFilesystemError::WorksheetNotFound {
                path: worksheet_path.clone(),
            })?;
        if current.worksheets.keys().any(|existing| {
            existing != worksheet_path
                && existing.display_name().portable_key() == new_name.portable_key()
        }) {
            return Err(ProjectFilesystemError::ResourceNameConflict {
                message: format!("a worksheet named '{}' already exists", new_name.as_str()),
            });
        }
        moved.revision = expected_revision.next();
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::from([(worksheet_key(worksheet_path), expected_revision)]),
            BTreeSet::from([worksheet_key(&target)]),
        );
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)?;
        let prepared = ProjectFilesystemTransaction::prepare(
            mutation_context.clone(),
            lease,
            vec![StagedFilesystemMutation::MoveFile {
                from: worksheet_path.relative_path().to_path_buf(),
                to: target.relative_path().to_path_buf(),
            }],
        )?;
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)?;
        self.validate_resource_lifecycle_operation(&ownership.operation)?;
        let projection_environment = self
            .capture_projection_environment_for_session(&snapshot.session)
            .map_err(|message| ProjectFilesystemError::TransactionPrepareFailed { message })?;
        let committed = prepared.commit()?;
        let mut result = match self.apply_resource_document_patch_with_environment(
            &mutation_context,
            ResourceDocumentPatch::MoveWorksheet {
                from: worksheet_path.clone(),
                to: target.clone(),
                moved: moved.clone(),
            },
            projection_environment,
            Some(&mut ownership),
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
        result.deltas = vec![worksheet_move_delta(
            worksheet_path,
            &target,
            operation_id,
            expected_revision,
            moved.revision,
        )];
        reservation.complete();
        Ok(result)
    }

    pub fn remove_worksheet_resource_transaction(
        &self,
        expected_project_instance_id: &ProjectInstanceId,
        worksheet_path: &WorksheetResourcePath,
        expected_revision: ResourceRevision,
        operation_id: OperationId,
    ) -> Result<ResourceMutationResultDto, ProjectFilesystemError> {
        let snapshot = self.capture_writer_snapshot(expected_project_instance_id)?;
        let reservation =
            self.reserve_resource_operation(expected_project_instance_id, operation_id)?;
        let lease = self.filesystem().acquire(snapshot.session.root.clone())?;
        let document = self
            .project_data
            .read()
            .unwrap()
            .worksheets
            .get(worksheet_path)
            .cloned()
            .ok_or_else(|| ProjectFilesystemError::WorksheetNotFound {
                path: worksheet_path.clone(),
            })?;
        let mutation_context = context(
            self,
            snapshot.session.clone(),
            operation_id,
            BTreeMap::from([(worksheet_key(worksheet_path), expected_revision)]),
            BTreeSet::new(),
        );
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)?;
        let prepared = ProjectFilesystemTransaction::prepare(
            mutation_context.clone(),
            lease,
            vec![StagedFilesystemMutation::RemoveFile {
                relative_path: worksheet_path.relative_path().to_path_buf(),
            }],
        )?;
        self.validate_writer_context(&mutation_context, snapshot.authority_generation)?;
        let projection_environment = self
            .capture_projection_environment_for_session(&snapshot.session)
            .map_err(|message| ProjectFilesystemError::TransactionPrepareFailed { message })?;
        let committed = prepared.commit()?;
        let mut result = match self.apply_resource_document_patch_with_environment(
            &mutation_context,
            ResourceDocumentPatch::RemoveWorksheet {
                path: worksheet_path.clone(),
                revision: expected_revision,
            },
            projection_environment,
            None,
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
        result.deltas = vec![worksheet_resource_delta(
            worksheet_path,
            operation_id,
            Some(document.revision),
            Some(&document),
            None,
        )];
        reservation.complete();
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::set_writer_snapshot_test_hook;
    use crate::graph::value::{DataType, DataValue};
    use crate::node_system::document::{
        FunctionResourceKey, OperationId, ResourceKey, ResourceRevision, VariableResourceKey,
    };
    use crate::project::{
        GraphDocument, GraphDocumentKind, GraphResourceDocument, GraphResourcePath, ProjectData,
        ProjectFilesystemFaultPoint, ProjectState, ResourceName, WorksheetDocument,
        WorksheetResourcePath, fixtures,
    };
    use crate::variable::VariableScope;
    use std::collections::BTreeMap;
    use std::sync::Arc;

    fn worksheet_files(project: &TestProject) -> Vec<std::path::PathBuf> {
        let worksheets = project.root.join(crate::project::WORKSHEETS_DIR);
        let Ok(entries) = std::fs::read_dir(worksheets) else {
            return Vec::new();
        };
        let mut paths = entries
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension().and_then(|value| value.to_str()) == Some("yssbi-worksheet")
            })
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }

    fn assert_two_distinct_worksheets_on_disk(
        project: &TestProject,
        first: &WorksheetResourcePath,
        second: &WorksheetResourcePath,
    ) {
        let files = worksheet_files(project);
        assert_eq!(
            files.len(),
            2,
            "each authoritative worksheet needs its own file"
        );
        assert!(project.root.join(first.relative_path()).is_file());
        assert!(project.root.join(second.relative_path()).is_file());
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
        state
            .set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::SecondLiveReplacement));
        let rename_error = state
            .rename_graph_resource_fixture(session.instance_id.as_str(), &graph_path, "After")
            .unwrap_err();
        state.set_project_filesystem_fault(None);
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
            worker_state.create_worksheet_resource_transaction(
                &worker_session.instance_id,
                &ResourceName::parse("Analysis").unwrap(),
                None,
                OperationId::new(),
            )
        });

        let (existing_path, existing) = fixtures::worksheet("Analysis", "");
        state
            .project_data
            .write()
            .unwrap()
            .worksheets
            .insert(existing_path, existing);
        drop(lease);

        let created = worker.join().unwrap().unwrap();
        assert_eq!(
            worksheet_path_from_lifecycle_result(&created)
                .display_name()
                .as_str(),
            "Analysis 2"
        );
    }

    #[test]
    fn worksheet_create_duplicate_name_keeps_distinct_authority_and_disk_documents() {
        let project = TestProject::new("worksheet-create-duplicate");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();

        let first = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &ResourceName::parse("Report").unwrap(),
                None,
                OperationId::new(),
            )
            .unwrap();
        let second = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &ResourceName::parse("Report").unwrap(),
                None,
                OperationId::new(),
            )
            .unwrap();
        let first_path = worksheet_path_from_lifecycle_result(&first);
        let second_path = worksheet_path_from_lifecycle_result(&second);

        assert_ne!(first_path, second_path);
        assert_two_distinct_worksheets_on_disk(&project, &first_path, &second_path);
    }

    #[test]
    fn worksheet_rename_moves_authority_file_revision_and_common_publication() {
        let project = TestProject::new("worksheet-rename-authority");
        let (source, document) = fixtures::worksheet("Report", "database");
        let target = WorksheetResourcePath::parse("worksheets/Renamed.yssbi-worksheet").unwrap();
        let mut data = ProjectData::new();
        data.worksheets.insert(source.clone(), document.clone());
        let state = active_state(&project, data);
        fixtures::write_worksheet(&project.root, &source, &document).unwrap();
        state.initialize_worksheet_revision_for_test(&source);
        let session = state.capture_project_session().unwrap();
        let operation_id = OperationId::new();

        let result = state
            .rename_worksheet_resource_transaction(
                &session.instance_id,
                &source,
                ResourceRevision::INITIAL,
                &ResourceName::parse("Renamed").unwrap(),
                1,
                operation_id,
            )
            .unwrap();

        let authority = state.get_data().unwrap();
        assert!(!authority.worksheets.contains_key(&source));
        assert_eq!(
            authority.worksheets[&target].revision,
            ResourceRevision::new(1)
        );
        assert!(!project.root.join(source.relative_path()).exists());
        assert!(project.root.join(target.relative_path()).is_file());
        assert_eq!(result.moves.len(), 1);
        assert_eq!(result.moves[0].from, source.as_str());
        assert_eq!(result.moves[0].to, target.as_str());
        assert_eq!(result.moves[0].name, "Renamed");
        assert_eq!(
            result.moves[0].kind,
            crate::node_system::document::ResourceLifecycleKind::Worksheet
        );
        assert_eq!(result.deltas.len(), 1);
        assert_eq!(result.deltas[0].resource, super::worksheet_key(&target));
        assert_eq!(result.deltas[0].from_revision, ResourceRevision::INITIAL);
        assert_eq!(result.deltas[0].to_revision, ResourceRevision::new(1));
        assert_eq!(result.deltas[0].caused_by, Some(operation_id));
        assert_eq!(
            result.history,
            crate::node_system::document::HistoryStatusDto {
                can_undo: true,
                can_redo: false,
            }
        );
    }

    #[test]
    fn worksheet_rename_rejects_exact_portable_conflict_without_suffixing() {
        let project = TestProject::new("worksheet-rename-conflict");
        let (source, source_document) = fixtures::worksheet("Source", "database");
        let (conflict, conflict_document) = fixtures::worksheet("Report", "database");
        let mut data = ProjectData::new();
        data.worksheets
            .insert(source.clone(), source_document.clone());
        data.worksheets
            .insert(conflict.clone(), conflict_document.clone());
        let state = active_state(&project, data);
        fixtures::write_worksheet(&project.root, &source, &source_document).unwrap();
        fixtures::write_worksheet(&project.root, &conflict, &conflict_document).unwrap();
        state.initialize_worksheet_revision_for_test(&source);
        state.initialize_worksheet_revision_for_test(&conflict);
        let session = state.capture_project_session().unwrap();

        let error = state
            .rename_worksheet_resource_transaction(
                &session.instance_id,
                &source,
                ResourceRevision::INITIAL,
                &ResourceName::parse("report").unwrap(),
                1,
                OperationId::new(),
            )
            .unwrap_err();

        assert_eq!(error.code(), "resource_name_conflict");
        let authority = state.get_data().unwrap();
        assert!(authority.worksheets.contains_key(&source));
        assert!(authority.worksheets.contains_key(&conflict));
        assert_eq!(worksheet_files(&project).len(), 2);
    }

    #[test]
    fn worksheet_save_never_overwrites_another_path() {
        let project = TestProject::new("worksheet-save-distinct-path");
        let (first_path, first) = fixtures::worksheet("Report", "database");
        let (second_path, mut second) = fixtures::worksheet("Other", "database");
        let mut data = ProjectData::new();
        data.worksheets.insert(first_path.clone(), first.clone());
        data.worksheets.insert(second_path.clone(), second.clone());
        let state = active_state(&project, data);
        fixtures::write_worksheet(&project.root, &first_path, &first).unwrap();
        fixtures::write_worksheet(&project.root, &second_path, &second).unwrap();
        state.initialize_worksheet_revision_for_test(&first_path);
        state.initialize_worksheet_revision_for_test(&second_path);
        let session = state.capture_project_session().unwrap();

        second.chart_type = "line".into();
        state
            .save_worksheet_document(
                &session.instance_id,
                &second_path,
                ResourceRevision::INITIAL,
                OperationId::new(),
                second,
            )
            .unwrap();

        assert_two_distinct_worksheets_on_disk(&project, &first_path, &second_path);
        let persisted: WorksheetDocument = serde_json::from_slice(
            &std::fs::read(project.root.join(first_path.relative_path())).unwrap(),
        )
        .unwrap();
        assert_eq!(persisted, first);
    }

    #[test]
    fn worksheet_delete_removes_only_its_canonical_path() {
        let project = TestProject::new("worksheet-delete-canonical");
        let (first_path, first) = fixtures::worksheet("First", "database");
        let (second_path, second) = fixtures::worksheet("Second", "database");
        let first_file = project.root.join(first_path.relative_path());
        let second_file = project.root.join(second_path.relative_path());
        let mut data = ProjectData::new();
        data.worksheets.insert(first_path.clone(), first.clone());
        data.worksheets.insert(second_path.clone(), second.clone());
        fixtures::write_worksheet(&project.root, &first_path, &first).unwrap();
        fixtures::write_worksheet(&project.root, &second_path, &second).unwrap();
        let state = active_state(&project, data);
        state.initialize_worksheet_revision_for_test(&first_path);
        state.initialize_worksheet_revision_for_test(&second_path);
        let session = state.capture_project_session().unwrap();

        state
            .remove_worksheet_resource_transaction(
                &session.instance_id,
                &first_path,
                ResourceRevision::INITIAL,
                OperationId::new(),
            )
            .unwrap();

        assert!(!first_file.exists());
        assert!(second_file.is_file());
        assert!(
            !state
                .get_data()
                .unwrap()
                .worksheets
                .contains_key(&first_path)
        );
        assert!(
            state
                .get_data()
                .unwrap()
                .worksheets
                .contains_key(&second_path)
        );
    }

    #[test]
    fn worksheet_create_rejects_invalid_resource_names_without_writing() {
        let project = TestProject::new("worksheet-invalid-name");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();

        for name in ["A/B", "A\\B"] {
            assert!(
                ResourceName::parse(name).is_err()
                    || state
                        .create_worksheet_resource_transaction(
                            &session.instance_id,
                            &ResourceName::parse(name).unwrap(),
                            None,
                            OperationId::new(),
                        )
                        .is_err()
            );
        }

        assert!(worksheet_files(&project).is_empty());
    }

    #[test]
    fn worksheet_casefold_collision_uses_portable_unique_suffix() {
        let project = TestProject::new("worksheet-casefold-collision");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();

        let first = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &ResourceName::parse("Report").unwrap(),
                None,
                OperationId::new(),
            )
            .unwrap();
        let second = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &ResourceName::parse("report").unwrap(),
                None,
                OperationId::new(),
            )
            .unwrap();
        let first_path = worksheet_path_from_lifecycle_result(&first);
        let second_path = worksheet_path_from_lifecycle_result(&second);

        assert_eq!(second_path.display_name().as_str(), "report 2");
        assert_two_distinct_worksheets_on_disk(&project, &first_path, &second_path);
    }

    #[test]
    fn worksheet_commit_failure_restores_file_and_nested_directory_topology() {
        let project = TestProject::new("worksheet-rollback");
        let nested = project.root.join("worksheets/nested/deeper");
        std::fs::create_dir_all(&nested).unwrap();
        let sentinel = nested.join("sentinel.txt");
        std::fs::write(&sentinel, b"untouched").unwrap();
        let (worksheet_path, mut document) = fixtures::worksheet("Original", "database");
        fixtures::write_worksheet(&project.root, &worksheet_path, &document).unwrap();
        let canonical_path = project.root.join(worksheet_path.relative_path());
        let original_bytes = std::fs::read(&canonical_path).unwrap();
        let mut data = ProjectData::new();
        data.worksheets
            .insert(worksheet_path.clone(), document.clone());
        let state = active_state(&project, data);
        state.initialize_worksheet_revision_for_test(&worksheet_path);
        let session = state.capture_project_session().unwrap();
        document.chart_type = "line".into();
        state.set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::FirstLiveReplacement));

        let error = state
            .save_worksheet_document(
                &session.instance_id,
                &worksheet_path,
                ResourceRevision::INITIAL,
                OperationId::new(),
                document,
            )
            .unwrap_err();
        state.set_project_filesystem_fault(None);

        assert_eq!(error.code(), "transaction_commit_failed");
        assert_eq!(std::fs::read(canonical_path).unwrap(), original_bytes);
        assert_eq!(std::fs::read(sentinel).unwrap(), b"untouched");
        assert!(nested.is_dir());
    }

    fn worksheet_path_from_lifecycle_result(
        result: &crate::event::ResourceMutationResultDto,
    ) -> WorksheetResourcePath {
        let delta = result.deltas.first().expect("worksheet lifecycle delta");
        let crate::node_system::document::ResourceDocumentPatch::ResourceLifecycle(patch) =
            &delta.payload
        else {
            panic!("expected worksheet lifecycle delta");
        };
        WorksheetResourcePath::parse(
            patch
                .after
                .as_ref()
                .expect("created worksheet lifecycle state")
                .path
                .as_ref(),
        )
        .unwrap()
    }

    #[test]
    fn worksheet_create_publishes_resource_lifecycle_delta() {
        let project = TestProject::new("worksheet-authoritative-create");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();
        let operation_id = OperationId::new();
        let name = ResourceName::parse("Report").unwrap();

        let result = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &name,
                Some("database".into()),
                operation_id,
            )
            .unwrap();

        let path = WorksheetResourcePath::parse("worksheets/Report.yssbi-worksheet").unwrap();
        assert_eq!(worksheet_path_from_lifecycle_result(&result), path);
        assert_eq!(result.operation_id, operation_id);
        assert_eq!(result.deltas[0].from_revision, ResourceRevision::INITIAL);
        assert_eq!(result.deltas[0].to_revision, ResourceRevision::INITIAL);
        let crate::node_system::document::ResourceDocumentPatch::ResourceLifecycle(lifecycle) =
            &result.deltas[0].payload
        else {
            panic!("expected worksheet lifecycle delta");
        };
        assert_eq!(
            lifecycle.after.as_ref().unwrap().revision,
            ResourceRevision::INITIAL
        );
        assert!(state.get_data().unwrap().worksheets.contains_key(&path));
        assert!(project.root.join(path.relative_path()).is_file());
    }

    #[test]
    fn worksheet_duplicate_allocates_first_free_authoritative_path() {
        let project = TestProject::new("worksheet-authoritative-duplicate");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();
        let name = ResourceName::parse("Report").unwrap();
        let first = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &name,
                Some("database".into()),
                OperationId::new(),
            )
            .unwrap();
        let first_path = worksheet_path_from_lifecycle_result(&first);
        let second = state
            .duplicate_worksheet_resource_transaction(
                &session.instance_id,
                &first_path,
                ResourceRevision::INITIAL,
                OperationId::new(),
            )
            .unwrap();
        let second_path = worksheet_path_from_lifecycle_result(&second);
        let third = state
            .duplicate_worksheet_resource_transaction(
                &session.instance_id,
                &first_path,
                ResourceRevision::INITIAL,
                OperationId::new(),
            )
            .unwrap();
        let third_path = worksheet_path_from_lifecycle_result(&third);

        assert_eq!(first_path.display_name().as_str(), "Report");
        assert_eq!(second_path.display_name().as_str(), "Report 2");
        assert_eq!(third_path.display_name().as_str(), "Report 3");
        assert_eq!(state.get_data().unwrap().worksheets.len(), 3);
        assert_eq!(worksheet_files(&project).len(), 3);
    }

    #[test]
    fn worksheet_save_publishes_document_delta() {
        let project = TestProject::new("worksheet-authoritative-save");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();
        let created = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &ResourceName::parse("Report").unwrap(),
                Some("database".into()),
                OperationId::new(),
            )
            .unwrap();
        let path = worksheet_path_from_lifecycle_result(&created);
        let mut document = state.get_data().unwrap().worksheets[&path].clone();
        document.chart_type = "line".into();
        let operation_id = OperationId::new();

        let result = state
            .save_worksheet_document(
                &session.instance_id,
                &path,
                ResourceRevision::INITIAL,
                operation_id,
                document,
            )
            .unwrap();

        assert_eq!(result.operation_id, operation_id);
        assert!(matches!(
            result.deltas.as_slice(),
            [crate::node_system::document::ResourceDeltaEvent {
                from_revision,
                to_revision,
                payload: crate::node_system::document::ResourceDocumentPatch::Worksheet(_),
                ..
            }] if *from_revision == ResourceRevision::INITIAL
                && *to_revision == ResourceRevision::new(1)
        ));
        assert_eq!(
            state.get_data().unwrap().worksheets[&path].revision,
            ResourceRevision::new(1)
        );
    }

    #[test]
    fn worksheet_remove_publishes_resource_lifecycle_delta() {
        let project = TestProject::new("worksheet-authoritative-remove");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();
        let created = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &ResourceName::parse("Report").unwrap(),
                None,
                OperationId::new(),
            )
            .unwrap();
        let path = worksheet_path_from_lifecycle_result(&created);
        let operation_id = OperationId::new();

        let result = state
            .remove_worksheet_resource_transaction(
                &session.instance_id,
                &path,
                ResourceRevision::INITIAL,
                operation_id,
            )
            .unwrap();

        assert_eq!(result.operation_id, operation_id);
        assert_eq!(result.deltas[0].from_revision, ResourceRevision::INITIAL);
        assert_eq!(result.deltas[0].to_revision, ResourceRevision::new(1));
        let crate::node_system::document::ResourceDocumentPatch::ResourceLifecycle(lifecycle) =
            &result.deltas[0].payload
        else {
            panic!("expected worksheet lifecycle delta");
        };
        assert_eq!(
            lifecycle.before.as_ref().unwrap().revision,
            ResourceRevision::INITIAL
        );
        assert!(lifecycle.after.is_none());
        assert!(!state.get_data().unwrap().worksheets.contains_key(&path));
        assert!(!project.root.join(path.relative_path()).exists());
    }

    #[test]
    fn worksheet_delete_recreate_preserves_tombstone_revision() {
        let project = TestProject::new("worksheet-authoritative-aba");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();
        let name = ResourceName::parse("Reusable").unwrap();
        let created = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &name,
                None,
                OperationId::new(),
            )
            .unwrap();
        let path = worksheet_path_from_lifecycle_result(&created);
        state
            .remove_worksheet_resource_transaction(
                &session.instance_id,
                &path,
                ResourceRevision::INITIAL,
                OperationId::new(),
            )
            .unwrap();

        let recreated = state
            .create_worksheet_resource_transaction(
                &session.instance_id,
                &name,
                None,
                OperationId::new(),
            )
            .unwrap();

        assert_eq!(worksheet_path_from_lifecycle_result(&recreated), path);
        assert_eq!(recreated.deltas[0].from_revision, ResourceRevision::new(1));
        assert_eq!(recreated.deltas[0].to_revision, ResourceRevision::new(2));
        let crate::node_system::document::ResourceDocumentPatch::ResourceLifecycle(lifecycle) =
            &recreated.deltas[0].payload
        else {
            panic!("expected worksheet lifecycle delta");
        };
        assert_eq!(
            lifecycle.after.as_ref().unwrap().revision,
            ResourceRevision::new(2)
        );
        assert_eq!(
            state.get_data().unwrap().worksheets[&path].revision,
            ResourceRevision::new(2)
        );
    }

    #[test]
    fn worksheet_mutation_failures_have_zero_authoritative_effects() {
        let project = TestProject::new("worksheet-authoritative-failure");
        let state = active_state(&project, ProjectData::new());
        let session = state.capture_project_session().unwrap();
        let operation_id = OperationId::new();
        let name = ResourceName::parse("Report").unwrap();
        state.set_project_filesystem_fault(Some(ProjectFilesystemFaultPoint::FirstLiveReplacement));

        let error = state
            .create_worksheet_resource_transaction(&session.instance_id, &name, None, operation_id)
            .unwrap_err();

        state.set_project_filesystem_fault(None);
        assert_eq!(error.code(), "transaction_commit_failed");
        assert!(state.get_data().unwrap().worksheets.is_empty());
        assert!(state.worksheet_revisions.read().unwrap().is_empty());
        assert!(worksheet_files(&project).is_empty());
        state
            .create_worksheet_resource_transaction(&session.instance_id, &name, None, operation_id)
            .unwrap();
    }

    #[test]
    fn global_update_revalidates_caller_revision_after_waiting_for_root_lease() {
        let project = TestProject::new("global-revision-wait");
        let state = std::sync::Arc::new(active_state(&project, ProjectData::new()));
        let session = state.capture_project_session().unwrap();
        let variable = state
            .add_variable(
                "global",
                DataType::Int64,
                DataValue::Int64(1),
                "",
                VariableScope::Global,
                vec![],
            )
            .unwrap();
        state
            .persist_global_variables(
                &session.instance_id,
                state.global_variable_revision_snapshot(),
                OperationId::new(),
            )
            .unwrap();
        let lease = state.filesystem().acquire(session.root.clone()).unwrap();
        let (staged_tx, staged_rx) = std::sync::mpsc::channel();
        set_writer_snapshot_test_hook(Some(std::sync::Arc::new(move || {
            staged_tx.send(()).unwrap();
        })));
        let worker_state = std::sync::Arc::clone(&state);
        let project_instance_id = session.instance_id.clone();
        let worker = std::thread::spawn(move || {
            worker_state.update_global_variable_transaction(
                &project_instance_id,
                variable.id,
                Some("stale".into()),
                None,
                None,
                None,
                None,
                ResourceRevision::INITIAL,
                OperationId::new(),
            )
        });
        staged_rx.recv().unwrap();
        state.variable_revisions.write().unwrap().insert(
            variable.id,
            crate::project::project_state::VariableRevisionEntry::present(ResourceRevision::new(1)),
        );
        drop(lease);

        let error = match worker.join().unwrap() {
            Ok(_) => panic!("stale variable update unexpectedly committed"),
            Err(error) => error,
        };
        set_writer_snapshot_test_hook(None);
        assert_eq!(error.code(), "resource_revision_conflict");
        assert_eq!(
            state.get_variable(&variable.id).unwrap().unwrap().name,
            "global"
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
        assert_eq!(error.code(), "stale_project_lifecycle");
        assert!(events.is_empty());
    }
}
