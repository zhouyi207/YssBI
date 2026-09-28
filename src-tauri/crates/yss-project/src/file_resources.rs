//! Shared lifecycle for authored Mind and Markdown files.
use crate::project_writers::{ProjectResourceMutationFacts, context};
use crate::{ProjectError, ProjectOperationError, ProjectState};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use yss_filesystem::{FilesystemTransaction, StagedFilesystemMutation, read_secure_file};
use yss_project_history::{
    ResourceDeltaEvent, ResourceDocumentPatch, ResourceKey, ResourceLifecycleKind,
    ResourceLifecyclePatch, ResourceLifecycleState, ResourcePathMovePatch,
};
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};
use yss_project_model::file::{
    FileContent, FilePatch, FilePath, FileState, FileVersion, MAX_FILE_BYTES,
};
use yss_project_model::{ProjectData, ProjectDataPatch};
use yss_resource_naming::{ResourceName, allocate_unique_resource_name};

pub trait ResourceFile: FileContent {
    const LIFECYCLE_KIND: ResourceLifecycleKind;
    fn files(data: &ProjectData) -> &HashMap<FilePath<Self>, FileState<Self>>;
    fn files_mut(data: &mut ProjectData) -> &mut HashMap<FilePath<Self>, FileState<Self>>;
    fn patch(patch: FilePatch<Self>) -> ProjectDataPatch;
    fn key(path: &FilePath<Self>) -> ResourceKey;
}
pub(crate) fn apply_patch<T: ResourceFile>(data: &mut ProjectData, patch: FilePatch<T>) {
    let files = T::files_mut(data);
    match patch {
        FilePatch::Put { path, document } => {
            files.insert(path, document);
        }
        FilePatch::Remove { path } => {
            files.remove(&path);
        }
        FilePatch::Move { from, to, document } => {
            files.remove(&from);
            files.insert(to, document);
        }
    }
}
pub(crate) fn moves<T: ResourceFile>(
    patch: &FilePatch<T>,
) -> Vec<crate::project_writers::ProjectResourceMove> {
    match patch {
        FilePatch::Move { from, to, .. } => vec![crate::project_writers::ProjectResourceMove {
            from: from.as_str().into(),
            to: to.as_str().into(),
            kind: T::LIFECYCLE_KIND,
            name: to.name().into(),
        }],
        _ => vec![],
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSnapshot<T: FileContent> {
    pub project_instance_id: ProjectInstanceId,
    pub path: FilePath<T>,
    pub version: FileVersion,
    pub kind: &'static str,
    pub content: T,
    pub dirty: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileIndexEntry<T: FileContent> {
    pub path: FilePath<T>,
    pub kind: &'static str,
    pub name: String,
    pub revision: ResourceRevision,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(
    tag = "op",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields,
    bound(deserialize = "T::Edit: Deserialize<'de>")
)]
pub enum FileCommand<T: FileContent> {
    Create {
        name: String,
    },
    Edit {
        path: FilePath<T>,
        version: FileVersion,
        edits: Vec<T::Edit>,
    },
    Save {
        path: FilePath<T>,
        version: FileVersion,
    },
    Discard {
        path: FilePath<T>,
        version: FileVersion,
    },
    Rename {
        path: FilePath<T>,
        version: FileVersion,
        name: String,
    },
    Duplicate {
        path: FilePath<T>,
        version: FileVersion,
    },
    Delete {
        path: FilePath<T>,
        version: FileVersion,
    },
}

pub struct FileCommandResult<T: FileContent> {
    pub snapshot: Option<FileSnapshot<T>>,
    pub mutation: ProjectResourceMutationFacts,
}

fn error(message: impl ToString) -> ProjectOperationError {
    ProjectOperationError::TransactionPrepareFailed {
        message: message.to_string(),
    }
}
fn key<T: ResourceFile>(path: &FilePath<T>) -> ResourceKey {
    T::key(path)
}
fn lifecycle<T: ResourceFile>(
    path: &FilePath<T>,
    revision: ResourceRevision,
) -> ResourceLifecycleState {
    ResourceLifecycleState {
        path: path.as_str().into(),
        kind: T::LIFECYCLE_KIND,
        name: path.name().into(),
        revision,
    }
}

pub(crate) fn publication_deltas<T: ResourceFile>(
    operation_id: OperationId,
    patch: &FilePatch<T>,
    data: &ProjectData,
) -> Result<Vec<ResourceDeltaEvent>, ProjectOperationError> {
    let (path, before, after, payload) = match patch {
        FilePatch::Put { path, document } => {
            let previous = T::files(data).get(path).map(|doc| doc.version.revision);
            let next = document.version.revision;
            (
                path,
                previous.unwrap_or(next),
                next,
                ResourceDocumentPatch::ResourceLifecycle(ResourceLifecyclePatch {
                    before: previous.map(|revision| lifecycle(path, revision)),
                    after: Some(lifecycle(path, next)),
                }),
            )
        }
        FilePatch::Remove { path } => {
            let previous = T::files(data)
                .get(path)
                .ok_or_else(|| error("document not found"))?
                .version
                .revision;
            let next = crate::project_state::checked_resource_revision(path.as_str(), previous)?;
            (
                path,
                previous,
                next,
                ResourceDocumentPatch::ResourceLifecycle(ResourceLifecyclePatch {
                    before: Some(lifecycle(path, previous)),
                    after: None,
                }),
            )
        }
        FilePatch::Move { from, to, document } => {
            let previous = T::files(data)
                .get(from)
                .ok_or_else(|| error("document not found"))?
                .version
                .revision;
            (
                to,
                previous,
                document.version.revision,
                ResourceDocumentPatch::ResourceMove(ResourcePathMovePatch {
                    from: from.as_str().into(),
                    to: to.as_str().into(),
                }),
            )
        }
    };
    Ok(vec![ResourceDeltaEvent {
        resource: key(path),
        from_revision: before,
        to_revision: after,
        caused_by: Some(operation_id),
        payload,
    }])
}

fn snapshot<T: ResourceFile>(
    project: &ProjectInstanceId,
    path: &FilePath<T>,
    document: &FileState<T>,
) -> Result<FileSnapshot<T>, ProjectOperationError> {
    Ok(FileSnapshot {
        project_instance_id: project.clone(),
        path: path.clone(),
        version: document.version.clone(),
        kind: T::KIND,
        content: document.document.clone(),
        dirty: document.dirty().map_err(error)?,
    })
}

impl ProjectState {
    pub(crate) fn read_file<T: ResourceFile>(
        &self,
        project: &ProjectInstanceId,
        path: &FilePath<T>,
    ) -> Result<FileSnapshot<T>, ProjectOperationError> {
        self.ensure_project_operational()?;
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != project.as_str() {
            return Err(ProjectOperationError::StaleProjectLifecycle {
                message: "document project changed".into(),
            });
        }
        let data = self.project_data.read().unwrap();
        let document = T::files(&data)
            .get(path)
            .cloned()
            .ok_or_else(|| error("document not found"))?;
        drop(data);
        drop(publication);
        snapshot(project, path, &document)
    }

    /// GUI and automation both edit the same current document; only Save writes its body.
    pub(crate) fn apply_file_command<T: ResourceFile>(
        &self,
        project: &ProjectInstanceId,
        operation_id: OperationId,
        command: FileCommand<T>,
    ) -> Result<FileCommandResult<T>, ProjectOperationError> {
        let captured = self.capture_writer_snapshot(project)?;
        let reservation = self.reserve_resource_operation(project, operation_id)?;
        let lease = self.filesystem().acquire(captured.session.root.clone())?;
        let empty_context = context(
            self,
            captured.session.clone(),
            operation_id,
            BTreeMap::new(),
            BTreeSet::new(),
        );
        self.validate_writer_context(&empty_context, captured.authority_generation)?;
        let data = &captured.data;
        let mut expected = BTreeMap::new();
        let mut absent = BTreeSet::new();
        let mut writes = Vec::new();

        let source = match &command {
            FileCommand::Create { .. } => None,
            FileCommand::Edit { path, version, .. }
            | FileCommand::Save { path, version }
            | FileCommand::Discard { path, version }
            | FileCommand::Rename { path, version, .. }
            | FileCommand::Duplicate { path, version }
            | FileCommand::Delete { path, version } => {
                let current = T::files(data)
                    .get(path)
                    .ok_or_else(|| error("document not found"))?;
                if &current.version != version {
                    return Err(ProjectOperationError::ResourceRevisionConflict {
                        message: "document edit version changed".into(),
                    });
                }
                expected.insert(key(path), version.revision);
                Some((path.clone(), current.clone()))
            }
        };
        let unique_path = |name: &str| -> Result<FilePath<T>, ProjectOperationError> {
            let requested = ResourceName::parse(name).map_err(ProjectOperationError::from)?;
            let names = scan_file_paths::<T>(captured.session.root.as_path())
                .map_err(error)?
                .iter()
                .chain(T::files(data).keys())
                .map(|path| ResourceName::parse(path.name()).map_err(error))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(FilePath::<T>::from_name(&allocate_unique_resource_name(
                &requested,
                names.iter(),
            )))
        };
        let mut result_snapshot = None;
        let patch = match command {
            FileCommand::Create { name } => {
                let path = unique_path(&name)?;
                let document = FileState::<T>::new(
                    T::new(path.name(), &mut || uuid::Uuid::new_v4().to_string()),
                    uuid::Uuid::new_v4().to_string(),
                )
                .map_err(error)?;
                absent.insert(key(&path));
                writes.push(write(&path, &document)?);
                result_snapshot = Some(snapshot(project, &path, &document)?);
                FilePatch::Put { path, document }
            }
            FileCommand::Duplicate { .. } => {
                let (source_path, source) = source.expect("source command");
                let path = unique_path(source_path.name())?;
                let body = source
                    .document
                    .duplicate(&mut || uuid::Uuid::new_v4().to_string());
                let document =
                    FileState::<T>::new(body, uuid::Uuid::new_v4().to_string()).map_err(error)?;
                absent.insert(key(&path));
                writes.push(write(&path, &document)?);
                result_snapshot = Some(snapshot(project, &path, &document)?);
                FilePatch::Put { path, document }
            }
            command => {
                let (path, mut document) = source.expect("source command");
                document.version.revision = crate::project_state::checked_resource_revision(
                    path.as_str(),
                    document.version.revision,
                )?;
                match command {
                    FileCommand::Delete { .. } => {
                        writes.push(StagedFilesystemMutation::RemoveFile {
                            relative_path: path.as_str().into(),
                        });
                        FilePatch::Remove { path }
                    }
                    FileCommand::Discard { .. } => {
                        match read_file_content(captured.session.root.as_path(), &path) {
                            Ok(body) => {
                                document.document = body;
                                document.saved_hash =
                                    document.document.fingerprint().map_err(error)?;
                                result_snapshot = Some(snapshot(project, &path, &document)?);
                                FilePatch::Put { path, document }
                            }
                            Err(ProjectError::Io(cause))
                                if cause.kind() == std::io::ErrorKind::NotFound =>
                            {
                                FilePatch::Remove { path }
                            }
                            Err(cause) => return Err(error(cause)),
                        }
                    }
                    FileCommand::Rename { name, .. } => {
                        let name =
                            ResourceName::parse(&name).map_err(ProjectOperationError::from)?;
                        let target = FilePath::<T>::from_name(&name);
                        if target != path {
                            if scan_file_paths::<T>(captured.session.root.as_path())
                                .map_err(error)?
                                .iter()
                                .chain(T::files(data).keys())
                                .any(|other| {
                                    other != &path
                                        && ResourceName::parse(other.name())
                                            .is_ok_and(|n| n.portable_key() == name.portable_key())
                                })
                            {
                                return Err(ProjectOperationError::ResourceNameConflict {
                                    message: "document name already exists".into(),
                                });
                            }
                            absent.insert(key(&target));
                            writes.push(StagedFilesystemMutation::MoveFile {
                                from: path.as_str().into(),
                                to: target.as_str().into(),
                            });
                        }
                        result_snapshot = Some(snapshot(project, &target, &document)?);
                        if target == path {
                            FilePatch::Put { path, document }
                        } else {
                            FilePatch::Move {
                                from: path,
                                to: target,
                                document,
                            }
                        }
                    }
                    command => {
                        match command {
                            FileCommand::Edit { edits, .. } => {
                                if edits.is_empty() || edits.len() > 512 {
                                    return Err(error("invalid document edit batch size"));
                                }
                                for edit in edits {
                                    document.document.apply(edit).map_err(error)?;
                                }
                            }
                            FileCommand::Save { .. } => {
                                if read_file_content(captured.session.root.as_path(), &path)
                                    .map_err(error)?
                                    .fingerprint()
                                    .map_err(error)?
                                    != document.saved_hash
                                {
                                    return Err(ProjectOperationError::ResourceRevisionConflict { message: "saved document changed externally; discard or reopen before saving".into() });
                                }
                                document.saved_hash =
                                    document.document.fingerprint().map_err(error)?;
                                writes.push(write(&path, &document)?);
                            }
                            _ => unreachable!(),
                        }
                        result_snapshot = Some(snapshot(project, &path, &document)?);
                        FilePatch::Put { path, document }
                    }
                }
            }
        };
        let context = context(
            self,
            captured.session.clone(),
            operation_id,
            expected,
            absent,
        );
        if writes.is_empty() && matches!(&patch, FilePatch::Remove { .. }) {
            // Discarding resident edits must remain possible after external file removal.
            self.validate_writer_authority(&context, captured.authority_generation)?;
        } else {
            self.validate_writer_context(&context, captured.authority_generation)?;
        }
        let mutation = if writes.is_empty() {
            self.apply_project_resource_document_patch(&context, T::patch(patch), None, vec![])?
        } else {
            let prepared = FilesystemTransaction::prepare_with_validator(
                context.filesystem_context(),
                lease,
                writes,
                crate::project_writers::validate_document,
            )?;
            self.validate_writer_context(&context, captured.authority_generation)?;
            let committed = prepared.commit()?;
            match self.apply_project_resource_document_patch(
                &context,
                T::patch(patch),
                None,
                vec![],
            ) {
                Ok(result) => {
                    committed.finalize();
                    result
                }
                Err(failure) => {
                    committed.rollback()?;
                    return Err(failure);
                }
            }
        };
        reservation.complete();
        Ok(FileCommandResult {
            snapshot: result_snapshot,
            mutation,
        })
    }
}

fn write<T: ResourceFile>(
    path: &FilePath<T>,
    document: &FileState<T>,
) -> Result<StagedFilesystemMutation, ProjectOperationError> {
    Ok(StagedFilesystemMutation::Write {
        relative_path: path.as_str().into(),
        contents: document.document.encode().map_err(error)?,
    })
}

pub(crate) fn scan_file_paths<T: ResourceFile>(
    root: &Path,
) -> Result<Vec<FilePath<T>>, ProjectError> {
    let mut paths = Vec::new();
    let mut portable = HashSet::new();
    {
        let directory = T::DIRECTORY;
        let extension = T::EXTENSION;
        let parent = root.join(directory);
        let metadata = match std::fs::symlink_metadata(&parent) {
            Ok(metadata) => metadata,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(paths),
            Err(e) => return Err(e.into()),
        };
        if yss_filesystem::metadata_is_redirect(&metadata) || !metadata.is_dir() {
            return Err(ProjectError::InvalidProjectFormat(
                "invalid document directory".into(),
            ));
        }
        for entry in std::fs::read_dir(parent)? {
            let entry = entry?;
            let metadata = std::fs::symlink_metadata(entry.path())?;
            if yss_filesystem::metadata_is_redirect(&metadata) || metadata.is_dir() {
                return Err(ProjectError::InvalidProjectFormat(
                    "document redirects and nested directories are unsupported".into(),
                ));
            }
            if entry.path().extension().and_then(|e| e.to_str()) != Some(extension) {
                continue;
            }
            let path = FilePath::<T>::parse(&format!(
                "{directory}/{}",
                entry.file_name().to_string_lossy()
            ))
            .map_err(ProjectError::InvalidProjectFormat)?;
            let name = ResourceName::parse(path.name())
                .map_err(|e| ProjectError::InvalidProjectFormat(e.to_string()))?;
            if !portable.insert(format!("{directory}/{}", name.portable_key())) {
                return Err(ProjectError::InvalidProjectFormat(
                    "duplicate portable document name".into(),
                ));
            }
            paths.push(path);
        }
    }
    paths.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    Ok(paths)
}

pub(crate) fn read_file_content<T: ResourceFile>(
    root: &Path,
    path: &FilePath<T>,
) -> Result<T, ProjectError> {
    let relative = PathBuf::from(path.as_str());
    if std::fs::metadata(root.join(&relative))?.len() > MAX_FILE_BYTES as u64 {
        return Err(ProjectError::InvalidProjectFormat(
            "document size limit exceeded".into(),
        ));
    }
    T::decode(&read_secure_file(root, &relative)?).map_err(ProjectError::InvalidProjectFormat)
}

pub(crate) fn load_files<T: ResourceFile>(
    root: &Path,
) -> Result<HashMap<FilePath<T>, FileState<T>>, ProjectError> {
    scan_file_paths::<T>(root)?
        .into_iter()
        .map(|path| {
            let document = FileState::<T>::new(
                read_file_content(root, &path)?,
                uuid::Uuid::new_v4().to_string(),
            )
            .map_err(ProjectError::InvalidProjectFormat)?;
            Ok((path, document))
        })
        .collect()
}

pub(crate) fn file_index<T: ResourceFile>(
    root: &Path,
) -> Result<Vec<FileIndexEntry<T>>, ProjectError> {
    Ok(scan_file_paths::<T>(root)?
        .into_iter()
        .map(|path| FileIndexEntry {
            kind: T::KIND,
            name: path.name().into(),
            path,
            revision: ResourceRevision::INITIAL,
        })
        .collect())
}

pub(crate) fn external_changes<T: ResourceFile>(
    root: &Path,
    data: &ProjectData,
) -> Result<Vec<FilePatch<T>>, ProjectOperationError> {
    let incoming = load_files::<T>(root).map_err(error)?;
    let mut changes = Vec::new();
    for (path, mut document) in incoming.clone() {
        if let Some(previous) = T::files(data).get(&path) {
            if previous.dirty().map_err(error)? || previous.document == document.document {
                continue;
            }
            document.version.session_id = previous.version.session_id.clone();
            document.version.revision = crate::project_state::checked_resource_revision(
                path.as_str(),
                previous.version.revision,
            )?;
        }
        changes.push(FilePatch::Put { path, document });
    }
    for (path, previous) in T::files(data) {
        if !incoming.contains_key(path) && !previous.dirty().map_err(error)? {
            changes.push(FilePatch::Remove { path: path.clone() });
        }
    }
    Ok(changes)
}
#[cfg(test)]
mod tests {
    use super::*;
    use yss_project_model::{
        doc::{DocDocument, DocEdit},
        mind::{MindDocument, MindEdit},
    };

    #[test]
    fn typed_commands_reject_cross_kind_paths_and_edits() {
        use crate::{docs::DocCommand, minds::MindCommand};
        use serde_json::json;
        let version = json!({"sessionId":"session", "revision":0});
        let markdown = json!({"op":"edit", "path":"docs/Report.md", "version":version,
            "edits":[{"op":"set_markdown", "markdown":"# Report"}]});
        let mind = json!({"op":"edit", "path":"minds/Plan.yssbi-mind", "version":version,
            "edits":[{"op":"set_content", "nodeId":"root", "content":"Plan"}]});
        assert!(serde_json::from_value::<DocCommand>(markdown.clone()).is_ok());
        assert!(serde_json::from_value::<MindCommand>(mind.clone()).is_ok());
        assert!(serde_json::from_value::<MindCommand>(markdown.clone()).is_err());
        assert!(serde_json::from_value::<DocCommand>(mind.clone()).is_err());
        let mut wrong_doc_edit = markdown;
        wrong_doc_edit["edits"] = mind["edits"].clone();
        assert!(serde_json::from_value::<DocCommand>(wrong_doc_edit).is_err());
        let mut wrong_mind_edit = mind;
        wrong_mind_edit["edits"] = json!([{"op":"set_markdown","markdown":"wrong"}]);
        assert!(serde_json::from_value::<MindCommand>(wrong_mind_edit).is_err());
    }

    fn apply<T: ResourceFile>(
        state: &ProjectState,
        project: &ProjectInstanceId,
        command: FileCommand<T>,
    ) -> FileSnapshot<T> {
        state
            .apply_file_command(project, OperationId::new(), command)
            .unwrap()
            .snapshot
            .unwrap()
    }

    #[test]
    fn authored_document_lifecycle_preserves_unsaved_content_and_rejects_stale_edits() {
        let fixture =
            crate::fixtures::TempProject::activate("authored-lifecycle", ProjectData::new());
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let project = &session.instance_id;
        let created = apply(
            state,
            project,
            FileCommand::<DocDocument>::Create {
                name: "Report".into(),
            },
        );
        assert!(created.content.0.is_empty());
        let edited = apply(
            state,
            project,
            FileCommand::Edit {
                path: created.path.clone(),
                version: created.version.clone(),
                edits: vec![DocEdit::SetMarkdown {
                    markdown: "# Draft\n\n中文 **report**\n".into(),
                }],
            },
        );
        assert!(edited.dirty);
        assert_eq!(
            std::fs::read_to_string(session.root.as_path().join(created.path.as_str())).unwrap(),
            ""
        );
        assert!(
            state
                .apply_file_command(
                    project,
                    OperationId::new(),
                    FileCommand::Save {
                        path: created.path.clone(),
                        version: created.version
                    }
                )
                .is_err()
        );
        let moved = apply(
            state,
            project,
            FileCommand::Rename {
                path: edited.path.clone(),
                version: edited.version,
                name: "Renamed".into(),
            },
        );
        assert!(moved.dirty);
        assert_eq!(
            std::fs::read_to_string(session.root.as_path().join(moved.path.as_str())).unwrap(),
            ""
        );
        assert!(!session.root.as_path().join(created.path.as_str()).exists());
        let saved = apply(
            state,
            project,
            FileCommand::Save {
                path: moved.path.clone(),
                version: moved.version,
            },
        );
        assert!(!saved.dirty);
        assert_eq!(
            std::fs::read_to_string(session.root.as_path().join(saved.path.as_str())).unwrap(),
            "# Draft\n\n中文 **report**\n"
        );
        let changed = apply(
            state,
            project,
            FileCommand::Edit {
                path: saved.path.clone(),
                version: saved.version,
                edits: vec![DocEdit::SetMarkdown {
                    markdown: "temporary".into(),
                }],
            },
        );
        let discarded = apply(
            state,
            project,
            FileCommand::Discard {
                path: changed.path.clone(),
                version: changed.version,
            },
        );
        assert!(!discarded.dirty);
        assert_eq!(discarded.content, saved.content);
        let copied = apply(
            state,
            project,
            FileCommand::Duplicate {
                path: discarded.path.clone(),
                version: discarded.version,
            },
        );
        assert_ne!(copied.path, discarded.path);
        assert_eq!(copied.content, discarded.content);
        state
            .apply_file_command(
                project,
                OperationId::new(),
                FileCommand::Delete {
                    path: copied.path.clone(),
                    version: copied.version,
                },
            )
            .unwrap();
        assert!(!session.root.as_path().join(copied.path.as_str()).exists());
        assert_eq!(state.read_project_index(project).unwrap().docs.len(), 1);

        let mind = apply(
            state,
            project,
            FileCommand::<MindDocument>::Create {
                name: "Plan".into(),
            },
        );
        let before = mind.content.clone();
        assert!(
            state
                .apply_file_command(
                    project,
                    OperationId::new(),
                    FileCommand::Edit {
                        path: mind.path.clone(),
                        version: mind.version.clone(),
                        edits: vec![
                            MindEdit::SetContent {
                                node_id: before.root_id.clone(),
                                content: "must not commit".into()
                            },
                            MindEdit::AddNode {
                                node: yss_project_model::mind::MindNode {
                                    id: "bad".into(),
                                    parent_id: Some("absent".into()),
                                    content: "bad".into(),
                                    reference: None
                                }
                            }
                        ]
                    }
                )
                .is_err()
        );
        assert_eq!(
            state.read_file(project, &mind.path).unwrap().content,
            before
        );
        let copy = apply(
            state,
            project,
            FileCommand::Duplicate {
                path: mind.path,
                version: mind.version,
            },
        );
        let (source, copy) = (before, copy.content);
        assert_ne!(source.root_id, copy.root_id);
        assert_eq!(source.nodes[0].content, copy.nodes[0].content);
        let reopened =
            crate::load_project_from_file(session.root.as_path().to_str().unwrap()).unwrap();
        assert_eq!(reopened.docs.len(), 1);
        assert_eq!(reopened.minds.len(), 2);
    }

    #[test]
    fn external_document_changes_refresh_clean_state_and_cannot_overwrite_dirty_state() {
        let fixture =
            crate::fixtures::TempProject::activate("authored-watcher", ProjectData::new());
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let project = &session.instance_id;
        let created = apply(
            state,
            project,
            FileCommand::<DocDocument>::Create {
                name: "Report".into(),
            },
        );
        let file = session.root.as_path().join(created.path.as_str());
        std::fs::write(&file, "external").unwrap();
        state
            .reconcile_project_change(
                project,
                yss_filesystem::change::FilesystemChange::rescan_required(),
            )
            .unwrap();
        let refreshed = state.read_file(project, &created.path).unwrap();
        assert_eq!(refreshed.content, DocDocument("external".into()));
        let dirty = apply(
            state,
            project,
            FileCommand::Edit {
                path: refreshed.path,
                version: refreshed.version,
                edits: vec![DocEdit::SetMarkdown {
                    markdown: "local".into(),
                }],
            },
        );
        std::fs::write(&file, "external again").unwrap();
        state
            .reconcile_project_change(
                project,
                yss_filesystem::change::FilesystemChange::rescan_required(),
            )
            .unwrap();
        assert_eq!(
            state.read_file(project, &dirty.path).unwrap().content,
            DocDocument("local".into())
        );
        assert!(
            state
                .apply_file_command(
                    project,
                    OperationId::new(),
                    FileCommand::Save {
                        path: dirty.path.clone(),
                        version: dirty.version.clone()
                    }
                )
                .is_err()
        );
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "external again");
        std::fs::remove_file(&file).unwrap();
        state
            .reconcile_project_change(
                project,
                yss_filesystem::change::FilesystemChange::rescan_required(),
            )
            .unwrap();
        assert!(state.read_file(project, &dirty.path).unwrap().dirty);
        let discarded = state
            .apply_file_command(
                project,
                OperationId::new(),
                FileCommand::Discard {
                    path: dirty.path.clone(),
                    version: dirty.version,
                },
            )
            .unwrap();
        assert!(discarded.snapshot.is_none());
        assert!(state.read_file(project, &dirty.path).is_err());
        assert!(!file.exists());
    }
}
