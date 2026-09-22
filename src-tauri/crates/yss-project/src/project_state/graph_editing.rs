//! Editing metadata for resident graphs. The document itself lives only in ProjectData.
use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;

use yss_graph_document::GraphDocumentPatch;
use yss_graph_document::{GraphDocument, GraphResourcePath};
use yss_project_identity::{OperationId, ProjectInstanceId, ResourceRevision};

use super::{
    GraphCommitReceipt, GraphOperationCapture, ProjectGraphCommitError, ProjectGraphOperationError,
    ProjectState,
};
use crate::ProjectOperationError;

const HISTORY_LIMIT: usize = 50;
const HISTORY_BYTE_LIMIT: usize = 16 * 1024 * 1024;
const REQUEST_LIMIT: usize = 128;
const REQUEST_BYTE_LIMIT: usize = 2 * 1024 * 1024;
const REQUEST_RESULT_BYTE_LIMIT: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct GraphEditVersion {
    pub session_id: uuid::Uuid,
    pub revision: ResourceRevision,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GraphEditingState {
    pub version: GraphEditVersion,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
}

pub struct GraphEditingSnapshot {
    pub document: Arc<GraphDocument>,
    pub state: GraphEditingState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum GraphEditCommandKind {
    Edit,
    Undo,
    Redo,
    Save,
}

/// Bounded caller-owned facts retained atomically with the original commit for replay.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GraphEditCorrelation {
    pub client_key: String,
    pub document_hash: String,
    pub created_nodes: BTreeMap<String, yss_graph_document::NodeId>,
    pub created_ports: BTreeMap<String, yss_graph_document::PortAddress>,
    pub result_facts: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GraphEditCommandReceipt {
    pub fingerprint: [u8; 32],
    pub request_version: GraphEditVersion,
    pub kind: GraphEditCommandKind,
    pub commit: GraphCommitReceipt,
    pub correlation: Option<GraphEditCorrelation>,
}

pub(super) struct PreparedGraphCommand {
    pub fingerprint: [u8; 32],
    pub version: GraphEditVersion,
    pub correlation: Option<GraphEditCorrelation>,
    bytes: usize,
}

impl PreparedGraphCommand {
    pub fn new(fingerprint: [u8; 32], version: GraphEditVersion, identity_bytes: usize) -> Self {
        // Conservatively reserve fixed receipt fields, including identities and revision digits.
        Self {
            fingerprint,
            version,
            correlation: None,
            bytes: 2048 + identity_bytes,
        }
    }

    pub fn set_correlation(
        &mut self,
        value: GraphEditCorrelation,
    ) -> Result<(), ProjectGraphCommitError> {
        let bytes = serde_json::to_vec(&value)
            .map_err(|error| ProjectGraphCommitError::InvalidDocument(error.to_string()))?
            .len();
        if bytes > REQUEST_RESULT_BYTE_LIMIT {
            return Err(ProjectGraphCommitError::InvalidDocument(
                "graph receipt exceeds its budget".into(),
            ));
        }
        if let Some(previous) = &self.correlation {
            self.bytes -= serde_json::to_vec(previous)
                .expect("validated receipt")
                .len();
        }
        self.bytes += bytes;
        self.correlation = Some(value);
        Ok(())
    }
}

#[derive(Clone)]
struct CommandEntry {
    receipt: GraphEditCommandReceipt,
    bytes: usize,
}

#[derive(Clone)]
struct HistoryEntry {
    patch: GraphDocumentPatch,
    bytes: usize,
}

#[derive(Clone)]
pub(crate) struct GraphEditingMetadata {
    pub(crate) session_id: uuid::Uuid,
    pub(crate) saved_hash: [u8; 32],
    pub(crate) current_hash: [u8; 32],
    undo: VecDeque<HistoryEntry>,
    redo: VecDeque<HistoryEntry>,
    commands: VecDeque<CommandEntry>,
    command_bytes: usize,
}

pub(crate) struct PreparedGraphEditingUpdate {
    pub(crate) path: GraphResourcePath,
    pub(crate) metadata: GraphEditingMetadata,
}

impl GraphEditingMetadata {
    pub(crate) fn new(hash: [u8; 32]) -> Self {
        Self {
            session_id: uuid::Uuid::new_v4(),
            saved_hash: hash,
            current_hash: hash,
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            commands: VecDeque::new(),
            command_bytes: 0,
        }
    }

    pub(crate) fn state(&self, revision: ResourceRevision) -> GraphEditingState {
        GraphEditingState {
            version: GraphEditVersion {
                session_id: self.session_id,
                revision,
            },
            dirty: self.current_hash != self.saved_hash,
            can_undo: !self.undo.is_empty(),
            can_redo: !self.redo.is_empty(),
        }
    }

    pub(crate) fn mark_saved(&mut self) {
        self.saved_hash = self.current_hash;
        self.undo.clear();
        self.redo.clear();
    }

    pub(super) fn record_command(
        &mut self,
        command: PreparedGraphCommand,
        kind: GraphEditCommandKind,
        commit: &GraphCommitReceipt,
    ) {
        self.command_bytes += command.bytes;
        self.commands.push_back(CommandEntry {
            bytes: command.bytes,
            receipt: GraphEditCommandReceipt {
                fingerprint: command.fingerprint,
                request_version: command.version,
                kind,
                commit: commit.clone(),
                correlation: command.correlation,
            },
        });
        while self.commands.len() > REQUEST_LIMIT || self.command_bytes > REQUEST_BYTE_LIMIT {
            if let Some(entry) = self.commands.pop_front() {
                self.command_bytes -= entry.bytes;
            }
        }
    }

    pub(crate) fn apply(&mut self, change: PreparedGraphEdit) {
        self.current_hash = change.after_hash;
        let saved_edit = matches!(&change.action, GraphHistoryAction::SavedEdit(_));
        match change.action {
            GraphHistoryAction::Edit(patch) | GraphHistoryAction::SavedEdit(patch) => {
                if change.before_hash != change.after_hash {
                    self.redo.clear();
                    if !patch.is_empty() {
                        self.undo.push_back(HistoryEntry {
                            patch,
                            bytes: change.bytes,
                        });
                    }
                }
            }
            GraphHistoryAction::Undo => {
                if let Some(entry) = self.undo.pop_back() {
                    self.redo.push_back(entry);
                }
            }
            GraphHistoryAction::Redo => {
                if let Some(entry) = self.redo.pop_back() {
                    self.undo.push_back(entry);
                }
            }
            GraphHistoryAction::Saved => {
                self.mark_saved();
            }
        }
        if saved_edit {
            self.saved_hash = self.current_hash;
        }
        while self.undo.len() + self.redo.len() > HISTORY_LIMIT
            || self
                .undo
                .iter()
                .chain(&self.redo)
                .map(|entry| entry.bytes)
                .sum::<usize>()
                > HISTORY_BYTE_LIMIT
        {
            if self.undo.pop_front().is_none() {
                self.redo.pop_front();
            }
        }
    }
}

pub enum GraphHistoryAction {
    Edit(GraphDocumentPatch),
    SavedEdit(GraphDocumentPatch),
    Undo,
    Redo,
    Saved,
}

pub(crate) struct PreparedGraphEdit {
    pub(crate) before_hash: [u8; 32],
    pub(crate) after_hash: [u8; 32],
    action: GraphHistoryAction,
    bytes: usize,
    pub(super) kind: GraphEditCommandKind,
}

impl PreparedGraphEdit {
    pub(crate) fn new(
        before: &GraphDocument,
        after: &GraphDocument,
        action: GraphHistoryAction,
    ) -> Result<Self, ProjectGraphCommitError> {
        let before_hash =
            document_hash(before).map_err(ProjectGraphCommitError::InvalidDocument)?;
        let after_hash = document_hash(after).map_err(ProjectGraphCommitError::InvalidDocument)?;
        let bytes = match &action {
            GraphHistoryAction::Edit(patch) | GraphHistoryAction::SavedEdit(patch) => {
                serde_json::to_vec(patch)
                    .map_err(|error| ProjectGraphCommitError::InvalidDocument(error.to_string()))?
                    .len()
            }
            _ => 0,
        };
        let kind = match &action {
            GraphHistoryAction::Edit(_) | GraphHistoryAction::SavedEdit(_) => {
                GraphEditCommandKind::Edit
            }
            GraphHistoryAction::Undo => GraphEditCommandKind::Undo,
            GraphHistoryAction::Redo => GraphEditCommandKind::Redo,
            GraphHistoryAction::Saved => GraphEditCommandKind::Save,
        };
        Ok(Self {
            before_hash,
            after_hash,
            action,
            bytes,
            kind,
        })
    }
}

pub(crate) fn document_hash(document: &GraphDocument) -> Result<[u8; 32], String> {
    yss_canonical_hash::hash_canonical("yssbi.graph-document.saved-content.v1", document)
        .map_err(|error| error.to_string())
}

impl ProjectState {
    pub(crate) fn prepare_graph_editing_remap(
        &self,
        source: &GraphResourcePath,
        target: &GraphResourcePath,
        remap: (&GraphResourcePath, &GraphResourcePath),
        current: &GraphDocument,
        saved: &GraphDocument,
        documents_changed: bool,
    ) -> Result<Option<PreparedGraphEditingUpdate>, ProjectOperationError> {
        let Some(mut metadata) = self.graph_editing.lock().unwrap().get(source).cloned() else {
            return Ok(None);
        };
        let mut history_changed = false;
        for entry in metadata.undo.iter_mut().chain(metadata.redo.iter_mut()) {
            if !super::graph_references::remap_patch(&mut entry.patch, remap.0, remap.1) {
                continue;
            }
            history_changed = true;
            entry.bytes = serde_json::to_vec(&entry.patch)
                .map_err(|error| ProjectOperationError::TransactionPrepareFailed {
                    message: error.to_string(),
                })?
                .len();
        }
        if source == target && !history_changed && !documents_changed {
            return Ok(None);
        }
        metadata.current_hash = document_hash(current)
            .map_err(|message| ProjectOperationError::TransactionPrepareFailed { message })?;
        metadata.saved_hash = document_hash(saved)
            .map_err(|message| ProjectOperationError::TransactionPrepareFailed { message })?;
        Ok(Some(PreparedGraphEditingUpdate {
            path: target.clone(),
            metadata,
        }))
    }

    pub fn read_graph_editing(
        &self,
        project: &ProjectInstanceId,
        path: &GraphResourcePath,
    ) -> Result<GraphEditingSnapshot, ProjectOperationError> {
        for _ in 0..3 {
            let (document, revision) = {
                let publication = self.mutation_publication.lock().unwrap();
                if publication.project_instance_id != project.as_str() {
                    return Err(editing_stale());
                }
                self.ensure_project_operational()?;
                let data = self.project_data.read().unwrap();
                let document = data
                    .graphs
                    .get(path)
                    .ok_or_else(editing_stale)?
                    .document
                    .clone();
                let revision = self
                    .graph_resource_revisions
                    .read()
                    .unwrap()
                    .get(path)
                    .copied()
                    .ok_or_else(editing_stale)?;
                (document, revision)
            };
            let hash = document_hash(&document)
                .map_err(|message| ProjectOperationError::TransactionPrepareFailed { message })?;
            let publication = self.mutation_publication.lock().unwrap();
            if publication.project_instance_id != project.as_str() {
                return Err(editing_stale());
            }
            if self
                .graph_resource_revisions
                .read()
                .unwrap()
                .get(path)
                .copied()
                != Some(revision)
            {
                continue;
            }
            let mut editing = self.graph_editing.lock().unwrap();
            let metadata = editing
                .entry(path.clone())
                .or_insert_with(|| GraphEditingMetadata::new(hash));
            // A disk refresh or resource transaction can replace a clean resident document.
            if metadata.current_hash != hash {
                *metadata = GraphEditingMetadata::new(hash);
            }
            return Ok(GraphEditingSnapshot {
                document: Arc::new(document),
                state: metadata.state(revision),
            });
        }
        Err(editing_stale())
    }

    pub fn capture_graph_edit(
        &self,
        project: &ProjectInstanceId,
        path: &GraphResourcePath,
        version: GraphEditVersion,
        operation_id: OperationId,
        fingerprint: [u8; 32],
    ) -> Result<GraphOperationCapture, ProjectGraphOperationError> {
        let current = self.read_graph_editing(project, path).map_err(|source| {
            ProjectGraphOperationError::Internal(super::ProjectGraphOperationSource::new(source))
        })?;
        if current.state.version != version {
            return Err(ProjectGraphOperationError::RevisionConflict {
                graph: path.clone(),
                expected: version.revision,
                current: current.state.version.revision,
            });
        }
        if self
            .graph_editing
            .lock()
            .unwrap()
            .get(path)
            .is_some_and(|metadata| {
                metadata
                    .commands
                    .iter()
                    .any(|entry| entry.receipt.commit.operation_id == operation_id)
            })
        {
            return Err(ProjectGraphOperationError::OperationOwnershipChanged { operation_id });
        }
        let mut capture =
            self.capture_graph_operation(project, path, version.revision, operation_id)?;
        if self
            .graph_editing
            .lock()
            .unwrap()
            .get(path)
            .map(|metadata| metadata.session_id)
            != Some(version.session_id)
        {
            return Err(ProjectGraphOperationError::ResourceLifecycleChanged {
                graph: path.clone(),
            });
        }
        let invalid_receipt = |message| {
            ProjectGraphOperationError::Internal(super::ProjectGraphOperationSource::new(
                ProjectOperationError::TransactionPrepareFailed { message },
            ))
        };
        let identity_bytes = serde_json::to_vec(project)
            .map_err(|error| invalid_receipt(error.to_string()))?
            .len();
        if identity_bytes > REQUEST_RESULT_BYTE_LIMIT {
            return Err(invalid_receipt(
                "graph receipt identity exceeds its budget".into(),
            ));
        }
        capture.prepare_edit_command(PreparedGraphCommand::new(
            fingerprint,
            version,
            identity_bytes,
        ));
        Ok(capture)
    }

    pub fn graph_edit_command_receipt(
        &self,
        project: &ProjectInstanceId,
        path: &GraphResourcePath,
        session_id: uuid::Uuid,
        operation_id: OperationId,
    ) -> Result<Option<GraphEditCommandReceipt>, ProjectOperationError> {
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != project.as_str() {
            return Err(editing_stale());
        }
        self.ensure_project_operational()?;
        let editing = self.graph_editing.lock().unwrap();
        let metadata = editing.get(path).ok_or_else(editing_stale)?;
        if metadata.session_id != session_id {
            return Err(editing_stale());
        }
        Ok(metadata
            .commands
            .iter()
            .find(|entry| entry.receipt.commit.operation_id == operation_id)
            .map(|entry| entry.receipt.clone()))
    }

    pub fn commit_graph_edit(
        &self,
        capture: GraphOperationCapture,
        candidate: Arc<GraphDocument>,
        action: GraphHistoryAction,
    ) -> Result<GraphCommitReceipt, ProjectGraphCommitError> {
        let change = PreparedGraphEdit::new(&capture.document, &candidate, action)?;
        self.commit_graph_candidate_with_history(capture.into_authority(), candidate, change)
    }

    pub fn graph_history_patch(
        &self,
        project: &ProjectInstanceId,
        path: &GraphResourcePath,
        version: GraphEditVersion,
        redo: bool,
    ) -> Result<Option<GraphDocumentPatch>, ProjectOperationError> {
        let publication = self.mutation_publication.lock().unwrap();
        if publication.project_instance_id != project.as_str() {
            return Err(editing_stale());
        }
        if self
            .graph_resource_revisions
            .read()
            .unwrap()
            .get(path)
            .copied()
            != Some(version.revision)
        {
            return Err(editing_stale());
        }
        let editing = self.graph_editing.lock().unwrap();
        let metadata = editing.get(path).ok_or_else(editing_stale)?;
        if metadata.session_id != version.session_id {
            return Err(editing_stale());
        }
        Ok((if redo { &metadata.redo } else { &metadata.undo })
            .back()
            .map(|entry| {
                if redo {
                    entry.patch.clone()
                } else {
                    entry.patch.inverse()
                }
            }))
    }

    pub fn is_graph_modified(&self, path: &GraphResourcePath) -> bool {
        self.graph_editing
            .lock()
            .unwrap()
            .get(path)
            .is_some_and(|metadata| metadata.current_hash != metadata.saved_hash)
    }
}

fn editing_stale() -> ProjectOperationError {
    ProjectOperationError::StaleResourceLifecycle {
        message: "graph editing identity changed".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_graph_document::GraphDocumentOperation;
    use yss_graph_document::{DocumentNode, GraphResourceKind, NodeId, NodePosition};
    use yss_graph_document_edit::apply_graph_document_patch;
    use yss_project_model::{GraphResourceDocument, ProjectData};

    fn fixture() -> (crate::fixtures::TempProject, GraphResourcePath, NodeId) {
        let path = GraphResourcePath::new("events/Current.yssbi-event").unwrap();
        let id = NodeId::new();
        let mut graph = GraphResourceDocument::new("Current", GraphResourceKind::Event);
        graph.document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: "yssbi.tests.node".parse().unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: Default::default(),
                user_label: None,
            },
        );
        let mut data = ProjectData::new();
        data.graphs.insert(path.clone(), graph);
        (
            crate::fixtures::TempProject::activate("current-graph-editing", data),
            path,
            id,
        )
    }

    fn move_node(
        state: &ProjectState,
        project: &ProjectInstanceId,
        path: &GraphResourcePath,
        id: NodeId,
    ) -> GraphCommitReceipt {
        let snapshot = state.read_graph_editing(project, path).unwrap();
        let capture = state
            .capture_graph_edit(
                project,
                path,
                snapshot.state.version,
                OperationId::new(),
                [0; 32],
            )
            .unwrap();
        let before = snapshot.document.nodes[&id].clone();
        let mut after = before.clone();
        after.position.x += 42.0;
        let patch =
            GraphDocumentPatch::new(vec![GraphDocumentOperation::UpdateNode { before, after }]);
        let mut document = (*snapshot.document).clone();
        apply_graph_document_patch(&mut document, &patch).unwrap();
        state
            .commit_graph_edit(capture, Arc::new(document), GraphHistoryAction::Edit(patch))
            .unwrap()
    }

    #[test]
    fn unload_retains_dirty_graph_and_discard_requires_the_confirmed_version() {
        let (fixture, path, node) = fixture();
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let project = &session.instance_id;
        let file = session.root.as_path().join(path.as_str());
        let saved = std::fs::read(&file).unwrap();
        let first = move_node(state, project, &path, node);
        assert!(
            !state
                .unload_graph_resource_for_lifecycle(project, &path, 100, None)
                .unwrap()
        );
        let retained = state.read_graph_editing(project, &path).unwrap();
        assert!(retained.state.dirty && retained.state.can_undo);
        let latest = move_node(state, project, &path, node);
        assert!(matches!(
            state.unload_graph_resource_for_lifecycle(
                project,
                &path,
                101,
                Some(first.editing.version)
            ),
            Err(ProjectOperationError::ResourceRevisionConflict { .. })
        ));
        assert_eq!(
            state
                .read_graph_editing(project, &path)
                .unwrap()
                .state
                .version,
            latest.editing.version
        );
        assert!(
            state
                .unload_graph_resource_for_lifecycle(
                    project,
                    &path,
                    102,
                    Some(latest.editing.version)
                )
                .unwrap()
        );
        assert!(state.read_resident_graph(&path).unwrap().is_none());
        assert_eq!(std::fs::read(&file).unwrap(), saved);
    }

    #[test]
    fn memory_edits_undo_redo_and_explicit_save_share_the_current_document() {
        let (fixture, path, node) = fixture();
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let project = &session.instance_id;
        let file = session.root.as_path().join(path.as_str());
        let original_file = std::fs::read(&file).unwrap();
        let receipt = move_node(state, project, &path, node);
        assert!(receipt.editing.dirty && receipt.editing.can_undo);
        assert_eq!(
            state
                .read_resident_graph(&path)
                .unwrap()
                .unwrap()
                .document
                .nodes[&node]
                .position
                .x,
            42.0
        );
        assert_eq!(std::fs::read(&file).unwrap(), original_file);

        for redo in [false, true] {
            let snapshot = state.read_graph_editing(project, &path).unwrap();
            let capture = state
                .capture_graph_edit(
                    project,
                    &path,
                    snapshot.state.version,
                    OperationId::new(),
                    [0; 32],
                )
                .unwrap();
            let patch = state
                .graph_history_patch(project, &path, snapshot.state.version, redo)
                .unwrap()
                .unwrap();
            let mut document = (*snapshot.document).clone();
            apply_graph_document_patch(&mut document, &patch).unwrap();
            let receipt = state
                .commit_graph_edit(
                    capture,
                    Arc::new(document),
                    if redo {
                        GraphHistoryAction::Redo
                    } else {
                        GraphHistoryAction::Undo
                    },
                )
                .unwrap();
            assert_eq!(receipt.editing.dirty, redo);
            assert!(receipt.to_revision > snapshot.state.version.revision);
            assert_eq!(std::fs::read(&file).unwrap(), original_file);
        }

        let snapshot = state.read_graph_editing(project, &path).unwrap();
        let failed_id = OperationId::new();
        let failed_capture = state
            .capture_graph_edit(project, &path, snapshot.state.version, failed_id, [3; 32])
            .unwrap();
        state.set_filesystem_fault(Some(
            yss_filesystem::FilesystemFaultPoint::FirstLiveReplacement,
        ));
        assert!(
            state
                .save_graph_candidate(failed_capture, snapshot.document.clone())
                .is_err()
        );
        state.set_filesystem_fault(None);
        assert_eq!(std::fs::read(&file).unwrap(), original_file);
        let preserved = state.read_graph_editing(project, &path).unwrap();
        assert_eq!(preserved.state, snapshot.state);
        assert_eq!(preserved.document, snapshot.document);
        assert!(
            state
                .graph_edit_command_receipt(
                    project,
                    &path,
                    snapshot.state.version.session_id,
                    failed_id
                )
                .unwrap()
                .is_none()
        );
        let capture = state
            .capture_graph_edit(
                project,
                &path,
                snapshot.state.version,
                OperationId::new(),
                [0; 32],
            )
            .unwrap();
        let saved = state
            .save_graph_candidate(capture, snapshot.document)
            .unwrap();
        assert!(!saved.editing.dirty && !saved.editing.can_undo && !saved.editing.can_redo);
        assert_ne!(std::fs::read(&file).unwrap(), original_file);
        let receipt = state
            .graph_edit_command_receipt(
                project,
                &path,
                saved.editing.version.session_id,
                saved.operation_id,
            )
            .unwrap()
            .unwrap();
        assert_eq!(receipt.commit, saved);
        assert_eq!(receipt.kind, GraphEditCommandKind::Save);
        assert_eq!(receipt.request_version, snapshot.state.version);
        assert!(saved.to_revision > snapshot.state.version.revision);
    }

    #[test]
    fn stale_writes_and_disk_refresh_cannot_replace_unsaved_memory_edits() {
        let (fixture, path, node) = fixture();
        let state = fixture.state();
        let project = state.capture_project_session().unwrap().instance_id;
        let original = state.read_graph_editing(&project, &path).unwrap();
        let stale_id = OperationId::new();
        let stale = state
            .capture_graph_edit(&project, &path, original.state.version, stale_id, [0; 32])
            .unwrap();
        let edited = move_node(state, &project, &path, node);
        assert!(matches!(
            state.commit_graph_edit(
                stale,
                original.document,
                GraphHistoryAction::Edit(GraphDocumentPatch::new(Vec::new()))
            ),
            Err(ProjectGraphCommitError::StaleAuthority { .. })
        ));
        assert!(
            state
                .graph_edit_command_receipt(
                    &project,
                    &path,
                    original.state.version.session_id,
                    stale_id
                )
                .unwrap()
                .is_none()
        );
        state
            .reconcile_project_change(
                &project,
                yss_filesystem::change::FilesystemChange::rescan_required(),
            )
            .unwrap();
        let preserved = state.read_graph_editing(&project, &path).unwrap();
        assert_eq!(preserved.state, edited.editing);
        assert_eq!(preserved.document.nodes[&node].position.x, 42.0);

        state.unload_graph_resource(&path).unwrap();
        assert_eq!(
            state.read_graph_editing(&project, &path).unwrap().state,
            preserved.state
        );
        state
            .unload_graph_resource_for_lifecycle(
                &project,
                &path,
                u64::MAX - 2,
                Some(preserved.state.version),
            )
            .unwrap();
        state
            .load_graph_document(&project, &path, u64::MAX - 1)
            .unwrap();
        let reopened = state.read_graph_editing(&project, &path).unwrap();
        assert_ne!(
            reopened.state.version.session_id,
            preserved.state.version.session_id
        );
        assert!(!reopened.state.dirty);
        assert!(
            state
                .graph_edit_command_receipt(
                    &project,
                    &path,
                    preserved.state.version.session_id,
                    edited.operation_id
                )
                .is_err()
        );
        assert!(
            state
                .capture_graph_edit(
                    &project,
                    &path,
                    preserved.state.version,
                    OperationId::new(),
                    [0; 32]
                )
                .is_err()
        );
    }

    #[test]
    fn command_receipts_are_bounded_and_evicted_requests_cannot_write_again() {
        let (fixture, path, node) = fixture();
        let state = fixture.state();
        let project = state.capture_project_session().unwrap().instance_id;
        let original = state.read_graph_editing(&project, &path).unwrap();
        let first = move_node(state, &project, &path, node);
        for _ in 0..REQUEST_LIMIT {
            move_node(state, &project, &path, node);
        }
        {
            let editing = state.graph_editing.lock().unwrap();
            let metadata = &editing[&path];
            assert_eq!(metadata.commands.len(), REQUEST_LIMIT);
            assert_eq!(metadata.undo.len(), HISTORY_LIMIT);
        }
        assert!(
            state
                .graph_edit_command_receipt(
                    &project,
                    &path,
                    original.state.version.session_id,
                    first.operation_id
                )
                .unwrap()
                .is_none()
        );
        assert!(
            state
                .capture_graph_edit(
                    &project,
                    &path,
                    original.state.version,
                    first.operation_id,
                    [0; 32]
                )
                .is_err()
        );

        let large_correlation = GraphEditCorrelation {
            client_key: "batch".into(),
            document_hash: "0".repeat(64),
            created_nodes: (0..512)
                .map(|index| (format!("node_{index:059}"), NodeId::new()))
                .collect(),
            created_ports: BTreeMap::new(),
            result_facts: serde_json::json!({}),
        };
        let mut first_noop = None;
        for _ in 0..65 {
            let snapshot = state.read_graph_editing(&project, &path).unwrap();
            let operation_id = OperationId::new();
            first_noop.get_or_insert((snapshot.state.version, operation_id));
            let mut capture = state
                .capture_graph_edit(
                    &project,
                    &path,
                    snapshot.state.version,
                    operation_id,
                    [1; 32],
                )
                .unwrap();
            capture
                .set_edit_correlation(large_correlation.clone())
                .unwrap();
            let committed = state
                .commit_graph_edit(
                    capture,
                    snapshot.document,
                    GraphHistoryAction::Edit(GraphDocumentPatch::new(Vec::new())),
                )
                .unwrap();
            assert!(!committed.invalidations.graph);
            assert!(committed.to_revision > snapshot.state.version.revision);
        }
        {
            let editing = state.graph_editing.lock().unwrap();
            let metadata = &editing[&path];
            assert!(metadata.command_bytes <= REQUEST_BYTE_LIMIT);
            assert!(metadata.commands.len() < 65);
            assert_eq!(metadata.undo.len(), HISTORY_LIMIT);
            assert!(
                metadata
                    .commands
                    .iter()
                    .all(|entry| serde_json::to_vec(&entry.receipt).unwrap().len() <= entry.bytes)
            );
        }
        let (version, operation_id) = first_noop.unwrap();
        assert!(
            state
                .graph_edit_command_receipt(&project, &path, version.session_id, operation_id)
                .unwrap()
                .is_none()
        );
        assert!(
            state
                .capture_graph_edit(&project, &path, version, operation_id, [1; 32])
                .is_err()
        );

        let before = state.read_graph_editing(&project, &path).unwrap();
        let rejected_id = OperationId::new();
        let mut capture = state
            .capture_graph_edit(&project, &path, before.state.version, rejected_id, [2; 32])
            .unwrap();
        let mut oversized = large_correlation;
        oversized.client_key = "x".repeat(REQUEST_RESULT_BYTE_LIMIT);
        assert!(capture.set_edit_correlation(oversized).is_err());
        drop(capture);
        let after = state.read_graph_editing(&project, &path).unwrap();
        assert_eq!(before.state, after.state);
        assert_eq!(before.document, after.document);
        assert!(
            state
                .graph_edit_command_receipt(
                    &project,
                    &path,
                    before.state.version.session_id,
                    rejected_id
                )
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn renaming_keeps_unsaved_content_and_its_history_out_of_the_file() {
        let (fixture, path, node) = fixture();
        let state = fixture.state();
        let session = state.capture_project_session().unwrap();
        let edited = move_node(state, &session.instance_id, &path, node);
        state
            .rename_graph_resource(
                &session.instance_id,
                &path,
                edited.to_revision,
                "Renamed",
                1000,
                OperationId::new(),
            )
            .unwrap();
        let target = GraphResourcePath::new("events/Renamed.yssbi-event").unwrap();
        let current = state
            .read_graph_editing(&session.instance_id, &target)
            .unwrap();
        assert!(current.state.dirty && current.state.can_undo);
        assert_eq!(current.document.nodes[&node].position.x, 42.0);
        let persisted = crate::project_io::load_project_graph_document_from_file(
            session.root.as_path().to_string_lossy().as_ref(),
            &target,
        )
        .unwrap();
        assert_eq!(persisted.document.nodes[&node].position.x, 0.0);
        let undo = state
            .graph_history_patch(&session.instance_id, &target, current.state.version, false)
            .unwrap()
            .unwrap();
        let mut restored = (*current.document).clone();
        apply_graph_document_patch(&mut restored, &undo).unwrap();
        assert_eq!(restored.nodes[&node].position.x, 0.0);
    }
}
