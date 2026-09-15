//! Editing metadata for resident graphs. The document itself lives only in ProjectData.
use std::collections::VecDeque;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GraphEditVersion {
    pub session_id: uuid::Uuid,
    pub revision: ResourceRevision,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

    pub(crate) fn apply(&mut self, change: PreparedGraphEdit) {
        self.current_hash = change.after_hash;
        match change.action {
            GraphHistoryAction::Edit(patch) => {
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
    Undo,
    Redo,
    Saved,
}

pub(crate) struct PreparedGraphEdit {
    pub(crate) before_hash: [u8; 32],
    pub(crate) after_hash: [u8; 32],
    action: GraphHistoryAction,
    bytes: usize,
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
            GraphHistoryAction::Edit(patch) => serde_json::to_vec(patch)
                .map_err(|error| ProjectGraphCommitError::InvalidDocument(error.to_string()))?
                .len(),
            _ => 0,
        };
        Ok(Self {
            before_hash,
            after_hash,
            action,
            bytes,
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
        from: &GraphResourcePath,
        to: &GraphResourcePath,
        current: &GraphDocument,
        saved: &GraphDocument,
    ) -> Result<Option<PreparedGraphEditingUpdate>, ProjectOperationError> {
        let Some(mut metadata) = self.graph_editing.lock().unwrap().get(source).cloned() else {
            return Ok(None);
        };
        let remap_node = |node: &mut yss_graph_document::DocumentNode| {
            for value in node.parameters.values_mut() {
                if value.as_str() == Some(from.as_str()) {
                    *value = serde_json::Value::String(to.as_str().into());
                }
            }
        };
        for entry in metadata.undo.iter_mut().chain(metadata.redo.iter_mut()) {
            for operation in &mut entry.patch.operations {
                use yss_graph_document::GraphDocumentOperation;
                match operation {
                    GraphDocumentOperation::InsertNode { node }
                    | GraphDocumentOperation::RemoveNode { node } => remap_node(node),
                    GraphDocumentOperation::UpdateNode { before, after } => {
                        remap_node(before);
                        remap_node(after);
                    }
                    _ => {}
                }
            }
            entry.bytes = serde_json::to_vec(&entry.patch)
                .map_err(|error| ProjectOperationError::TransactionPrepareFailed {
                    message: error.to_string(),
                })?
                .len();
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
        let capture =
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
        Ok(capture)
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
            .capture_graph_edit(project, path, snapshot.state.version, OperationId::new())
            .unwrap();
        let before = snapshot.document.nodes[&id].clone();
        let mut after = before.clone();
        after.position.x = 42.0;
        let patch =
            GraphDocumentPatch::new(vec![GraphDocumentOperation::UpdateNode { before, after }]);
        let mut document = (*snapshot.document).clone();
        apply_graph_document_patch(&mut document, &patch).unwrap();
        state
            .commit_graph_edit(capture, Arc::new(document), GraphHistoryAction::Edit(patch))
            .unwrap()
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
                .capture_graph_edit(project, &path, snapshot.state.version, OperationId::new())
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
        let capture = state
            .capture_graph_edit(project, &path, snapshot.state.version, OperationId::new())
            .unwrap();
        let saved = state
            .save_graph_candidate(capture, snapshot.document)
            .unwrap();
        assert!(!saved.editing.dirty && !saved.editing.can_undo && !saved.editing.can_redo);
        assert_ne!(std::fs::read(&file).unwrap(), original_file);
    }

    #[test]
    fn stale_writes_and_disk_refresh_cannot_replace_unsaved_memory_edits() {
        let (fixture, path, node) = fixture();
        let state = fixture.state();
        let project = state.capture_project_session().unwrap().instance_id;
        let original = state.read_graph_editing(&project, &path).unwrap();
        let stale = state
            .capture_graph_edit(&project, &path, original.state.version, OperationId::new())
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
                .capture_graph_edit(&project, &path, preserved.state.version, OperationId::new())
                .is_err()
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
