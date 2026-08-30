use crate::project::worksheet_io::{WorksheetDocument, WorksheetEncodings};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use yss_graph_document::{FunctionParameterId, GraphDocument, GraphResourcePath, GraphRevision};
#[cfg(test)]
use yss_graph_document_edit::{DocumentError, GraphDocumentPatch};
use yss_project_identity::{HistoryEntryId, OperationId, ProjectRevision, ResourceRevision};

fn checked_document_revision(revision: ResourceRevision) -> Result<ResourceRevision, u64> {
    revision.checked_next().map_err(|error| error.retained)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceKind {
    Graph,
    Function,
    Variable,
    Database,
    Worksheet,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "key", rename_all = "snake_case")]
pub enum ResourceKey {
    Graph(GraphResourcePath),
    Function(FunctionResourceKey),
    Variable(VariableResourceKey),
    Database(DatabaseResourceKey),
    Worksheet(WorksheetResourceKey),
}

impl ResourceKey {
    pub const fn kind(&self) -> ResourceKind {
        match self {
            Self::Graph(_) => ResourceKind::Graph,
            Self::Function(_) => ResourceKind::Function,
            Self::Variable(_) => ResourceKind::Variable,
            Self::Database(_) => ResourceKind::Database,
            Self::Worksheet(_) => ResourceKind::Worksheet,
        }
    }
}

/// Project-owned mutation envelope. Application and Command adapters may
/// carry this envelope across the session boundary, while Graph only receives
/// the typed payload it needs for a single graph operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MutationRequest<T> {
    pub resource: ResourceKey,
    pub base_revision: ResourceRevision,
    pub operation_id: OperationId,
    pub payload: T,
}

impl<T> MutationRequest<T> {
    pub const fn new(
        resource: ResourceKey,
        base_revision: ResourceRevision,
        operation_id: OperationId,
        payload: T,
    ) -> Self {
        Self {
            resource,
            base_revision,
            operation_id,
            payload,
        }
    }
}

macro_rules! opaque_resource_type {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub Box<str>);
    };
}

opaque_resource_type!(FunctionResourceKey);
opaque_resource_type!(VariableResourceKey);
opaque_resource_type!(DatabaseResourceKey);
opaque_resource_type!(WorksheetResourceKey);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionParameter {
    pub id: FunctionParameterId,
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionSignature {
    pub parameters: Vec<FunctionParameter>,
    pub return_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionDocument {
    pub revision: ResourceRevision,
    pub signature: FunctionSignature,
}

impl FunctionDocument {
    pub fn new(signature: FunctionSignature) -> Self {
        Self {
            revision: ResourceRevision::INITIAL,
            signature,
        }
    }

    fn apply_patch(&mut self, patch: &FunctionDocumentPatch) -> Result<(), u64> {
        let next_revision = checked_document_revision(self.revision)?;
        self.signature = patch.after.clone();
        self.revision = next_revision;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FunctionDocumentPatch {
    pub before: FunctionSignature,
    pub after: FunctionSignature,
}

impl FunctionDocumentPatch {
    pub fn new(before: FunctionSignature, after: FunctionSignature) -> Self {
        Self { before, after }
    }

    pub fn inverse(&self) -> Self {
        Self::new(self.after.clone(), self.before.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariableDocument {
    pub revision: ResourceRevision,
    pub value: Option<Value>,
}

impl VariableDocument {
    pub fn new(value: Value) -> Self {
        Self {
            revision: ResourceRevision::INITIAL,
            value: Some(value),
        }
    }

    fn apply_patch(&mut self, patch: &VariableDocumentPatch) -> Result<(), u64> {
        let next_revision = checked_document_revision(self.revision)?;
        self.value = patch.after.clone();
        self.revision = next_revision;
        Ok(())
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariableDocumentPatch {
    pub before: Option<Value>,
    pub after: Option<Value>,
}

impl VariableDocumentPatch {
    pub fn new(before: Option<Value>, after: Option<Value>) -> Self {
        Self { before, after }
    }

    pub fn inverse(&self) -> Self {
        Self::new(self.after.clone(), self.before.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceLifecycleKind {
    Event,
    Function,
    Worksheet,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLifecycleState {
    pub revision: ResourceRevision,
    pub path: Box<str>,
    pub kind: ResourceLifecycleKind,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLifecyclePatch {
    pub before: Option<ResourceLifecycleState>,
    pub after: Option<ResourceLifecycleState>,
}

impl ResourceLifecyclePatch {
    pub fn inverse(&self) -> Self {
        Self {
            before: self.after.clone(),
            after: self.before.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourcePathMovePatch {
    pub from: Box<str>,
    pub to: Box<str>,
}

impl ResourcePathMovePatch {
    pub fn inverse(&self) -> Self {
        Self {
            from: self.to.clone(),
            to: self.from.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorksheetDocumentState {
    pub database_id: String,
    pub chart_type: String,
    pub encodings: WorksheetEncodings,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorksheetDocumentPatch {
    pub before: WorksheetDocumentState,
    pub after: WorksheetDocumentState,
}

impl WorksheetDocumentPatch {
    pub fn inverse(&self) -> Self {
        Self {
            before: self.after.clone(),
            after: self.before.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatabaseDocumentPatch {
    pub before: Option<yss_database_contract::DatabaseDecl>,
    pub after: Option<yss_database_contract::DatabaseDecl>,
}

impl DatabaseDocumentPatch {
    pub fn inverse(&self) -> Self {
        Self {
            before: self.after.clone(),
            after: self.before.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "patch", rename_all = "snake_case")]
pub enum ResourceDocumentPatch {
    #[cfg(test)]
    Graph(GraphDocumentPatch),
    Function(FunctionDocumentPatch),
    Worksheet(WorksheetDocumentPatch),
    ResourceLifecycle(ResourceLifecyclePatch),
    ResourceMove(ResourcePathMovePatch),
    Variable(VariableDocumentPatch),
    VariableScopeMove(ResourcePathMovePatch),
    Database(DatabaseDocumentPatch),
}

impl ResourceDocumentPatch {
    pub const fn kind(&self) -> ResourceKind {
        match self {
            #[cfg(test)]
            Self::Graph(_) | Self::ResourceLifecycle(_) | Self::ResourceMove(_) => {
                ResourceKind::Graph
            }
            #[cfg(not(test))]
            Self::ResourceLifecycle(_) | Self::ResourceMove(_) => ResourceKind::Graph,
            Self::Function(_) => ResourceKind::Function,
            Self::Worksheet(_) => ResourceKind::Worksheet,
            Self::Variable(_) | Self::VariableScopeMove(_) => ResourceKind::Variable,
            Self::Database(_) => ResourceKind::Database,
        }
    }

    pub fn inverse(&self) -> Self {
        match self {
            #[cfg(test)]
            Self::Graph(patch) => Self::Graph(patch.inverse()),
            Self::Function(patch) => Self::Function(patch.inverse()),
            Self::Worksheet(patch) => Self::Worksheet(patch.inverse()),
            Self::ResourceLifecycle(patch) => Self::ResourceLifecycle(patch.inverse()),
            Self::ResourceMove(patch) => Self::ResourceMove(patch.inverse()),
            Self::Variable(patch) => Self::Variable(patch.inverse()),
            Self::VariableScopeMove(patch) => Self::VariableScopeMove(patch.inverse()),
            Self::Database(patch) => Self::Database(patch.inverse()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResourcePatch {
    pub resource: ResourceKey,
    pub before_revision: ResourceRevision,
    pub after_revision: ResourceRevision,
    pub forward: ResourceDocumentPatch,
    pub inverse: ResourceDocumentPatch,
}

fn candidate_after_revision(before: ResourceRevision) -> ResourceRevision {
    before.checked_next().unwrap_or(before)
}

impl ResourcePatch {
    #[cfg(test)]
    pub fn graph(
        graph_path: GraphResourcePath,
        before_revision: GraphRevision,
        forward: GraphDocumentPatch,
    ) -> Self {
        let inverse = forward.inverse();
        let before_revision = ResourceRevision::from_graph_revision(before_revision);
        Self {
            resource: ResourceKey::Graph(graph_path),
            before_revision,
            after_revision: candidate_after_revision(before_revision),
            forward: ResourceDocumentPatch::Graph(forward),
            inverse: ResourceDocumentPatch::Graph(inverse),
        }
    }

    pub fn function(
        function_key: FunctionResourceKey,
        before_revision: ResourceRevision,
        forward: FunctionDocumentPatch,
    ) -> Self {
        let inverse = forward.inverse();
        Self {
            resource: ResourceKey::Function(function_key),
            before_revision,
            after_revision: candidate_after_revision(before_revision),
            forward: ResourceDocumentPatch::Function(forward),
            inverse: ResourceDocumentPatch::Function(inverse),
        }
    }

    pub fn variable(
        variable_key: VariableResourceKey,
        before_revision: ResourceRevision,
        forward: VariableDocumentPatch,
    ) -> Self {
        let inverse = forward.inverse();
        Self {
            resource: ResourceKey::Variable(variable_key),
            before_revision,
            after_revision: candidate_after_revision(before_revision),
            forward: ResourceDocumentPatch::Variable(forward),
            inverse: ResourceDocumentPatch::Variable(inverse),
        }
    }

    pub fn worksheet(
        worksheet_key: WorksheetResourceKey,
        before_revision: ResourceRevision,
        forward: WorksheetDocumentPatch,
    ) -> Self {
        let inverse = forward.inverse();
        Self {
            resource: ResourceKey::Worksheet(worksheet_key),
            before_revision,
            after_revision: candidate_after_revision(before_revision),
            forward: ResourceDocumentPatch::Worksheet(forward),
            inverse: ResourceDocumentPatch::Worksheet(inverse),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryMutation {}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryStatusDto {
    pub can_undo: bool,
    pub can_redo: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResourceLifecycleHistoryPayload {
    Graph { persisted_document: Value },
    Worksheet { document: WorksheetDocument },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceLifecycleHistoryPatch {
    pub forward: ResourceLifecyclePatch,
    pub payload: ResourceLifecycleHistoryPayload,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResourceMoveHistoryPayload {
    Graph { persisted_move_payload: Value },
    Worksheet { document: WorksheetDocument },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceMoveHistoryPatch {
    pub from: Box<str>,
    pub to: Box<str>,
    pub kind: ResourceLifecycleKind,
    pub payload: ResourceMoveHistoryPayload,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryPersistencePolicy {
    #[default]
    InMemoryUntilSave,
    DurableVariableEffects,
    DurableResourceMove,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VariableEffectHistorySnapshots {
    pub before: BTreeMap<VariableResourceKey, Option<Value>>,
    pub after: BTreeMap<VariableResourceKey, Option<Value>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectHistoryTransaction {
    pub history_id: HistoryEntryId,
    pub caused_by: OperationId,
    pub changes: Vec<ResourcePatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_change: Option<ProjectGraphHistoryChange>,
    pub persistence: HistoryPersistencePolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variable_effect_snapshots: Option<VariableEffectHistorySnapshots>,
    pub resource_lifecycle: Option<ResourceLifecycleHistoryPatch>,
    pub resource_move: Option<ResourceMoveHistoryPatch>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceDeltaEvent {
    pub resource: ResourceKey,
    pub from_revision: ResourceRevision,
    pub to_revision: ResourceRevision,
    pub caused_by: Option<OperationId>,
    pub payload: ResourceDocumentPatch,
}

impl ProjectHistoryTransaction {
    pub fn new(caused_by: OperationId, changes: impl Into<Vec<ResourcePatch>>) -> Self {
        Self {
            history_id: HistoryEntryId::new(),
            caused_by,
            changes: changes.into(),
            graph_change: None,
            persistence: HistoryPersistencePolicy::InMemoryUntilSave,
            variable_effect_snapshots: None,
            resource_lifecycle: None,
            resource_move: None,
        }
    }

    #[cfg(test)]
    pub fn graph(
        caused_by: OperationId,
        graph_path: GraphResourcePath,
        before_revision: GraphRevision,
        forward: GraphDocumentPatch,
    ) -> Self {
        Self::new(
            caused_by,
            vec![ResourcePatch::graph(graph_path, before_revision, forward)],
        )
    }

    pub fn graph_change(caused_by: OperationId, change: ProjectGraphHistoryChange) -> Self {
        Self {
            history_id: HistoryEntryId::new(),
            caused_by,
            changes: Vec::new(),
            graph_change: Some(change),
            persistence: HistoryPersistencePolicy::InMemoryUntilSave,
            variable_effect_snapshots: None,
            resource_lifecycle: None,
            resource_move: None,
        }
    }

    pub fn durable_variable_effects(
        caused_by: OperationId,
        changes: impl Into<Vec<ResourcePatch>>,
        snapshots: VariableEffectHistorySnapshots,
    ) -> Self {
        Self {
            history_id: HistoryEntryId::new(),
            caused_by,
            changes: changes.into(),
            graph_change: None,
            persistence: HistoryPersistencePolicy::DurableVariableEffects,
            variable_effect_snapshots: Some(snapshots),
            resource_lifecycle: None,
            resource_move: None,
        }
    }

    pub fn resource_lifecycle(
        caused_by: OperationId,
        forward: ResourceLifecyclePatch,
        payload: ResourceLifecycleHistoryPayload,
    ) -> Self {
        Self {
            history_id: HistoryEntryId::new(),
            caused_by,
            changes: Vec::new(),
            graph_change: None,
            persistence: HistoryPersistencePolicy::InMemoryUntilSave,
            variable_effect_snapshots: None,
            resource_lifecycle: Some(ResourceLifecycleHistoryPatch { forward, payload }),
            resource_move: None,
        }
    }

    pub fn worksheet_resource_move(
        caused_by: OperationId,
        from: impl Into<Box<str>>,
        to: impl Into<Box<str>>,
        document: WorksheetDocument,
    ) -> Self {
        Self {
            history_id: HistoryEntryId::new(),
            caused_by,
            changes: Vec::new(),
            graph_change: None,
            persistence: HistoryPersistencePolicy::DurableResourceMove,
            variable_effect_snapshots: None,
            resource_lifecycle: None,
            resource_move: Some(ResourceMoveHistoryPatch {
                from: from.into(),
                to: to.into(),
                kind: ResourceLifecycleKind::Worksheet,
                payload: ResourceMoveHistoryPayload::Worksheet { document },
            }),
        }
    }

    pub fn graph_move(
        caused_by: OperationId,
        from: GraphResourcePath,
        to: GraphResourcePath,
        payload: Value,
    ) -> Self {
        let kind = if from.as_str().starts_with("functions/") {
            ResourceLifecycleKind::Function
        } else {
            ResourceLifecycleKind::Event
        };
        Self {
            history_id: HistoryEntryId::new(),
            caused_by,
            changes: Vec::new(),
            graph_change: None,
            persistence: HistoryPersistencePolicy::DurableResourceMove,
            variable_effect_snapshots: None,
            resource_lifecycle: None,
            resource_move: Some(ResourceMoveHistoryPatch {
                from: from.as_str().into(),
                to: to.as_str().into(),
                kind,
                payload: ResourceMoveHistoryPayload::Graph {
                    persisted_move_payload: payload,
                },
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectDocumentState {
    pub revision: ProjectRevision,
    pub graphs: BTreeMap<GraphResourcePath, GraphDocument>,
    pub functions: BTreeMap<FunctionResourceKey, FunctionDocument>,
    pub variables: BTreeMap<VariableResourceKey, VariableDocument>,
    pub worksheets: BTreeMap<WorksheetResourceKey, WorksheetDocument>,
    pub worksheet_revisions: BTreeMap<WorksheetResourceKey, ResourceRevision>,
}

impl Default for ProjectDocumentState {
    fn default() -> Self {
        Self {
            revision: ProjectRevision::INITIAL,
            graphs: BTreeMap::new(),
            functions: BTreeMap::new(),
            variables: BTreeMap::new(),
            worksheets: BTreeMap::new(),
            worksheet_revisions: BTreeMap::new(),
        }
    }
}

impl ProjectDocumentState {
    pub fn new(
        graphs: BTreeMap<GraphResourcePath, GraphDocument>,
        functions: BTreeMap<FunctionResourceKey, FunctionDocument>,
        variables: BTreeMap<VariableResourceKey, VariableDocument>,
    ) -> Self {
        Self {
            revision: ProjectRevision::INITIAL,
            graphs,
            functions,
            variables,
            worksheets: BTreeMap::new(),
            worksheet_revisions: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum HistoryError {
    EmptyTransaction,
    DuplicateResource(ResourceKey),
    ResourceNotFound(ResourceKey),
    ResourceKindMismatch {
        resource: ResourceKey,
        patch_kind: ResourceKind,
    },
    RevisionConflict {
        resource: ResourceKey,
        expected: ResourceRevision,
        actual: ResourceRevision,
    },
    NonMonotonicRevision {
        resource: ResourceKey,
        before: ResourceRevision,
        after: ResourceRevision,
    },
    RevisionExhausted {
        resource: Option<ResourceKey>,
        retained: u64,
    },
    InvalidInverse(ResourceKey),
    #[cfg(test)]
    Patch {
        resource: ResourceKey,
        source: DocumentError,
    },
    NothingToUndo,
    NothingToRedo,
    HistoryHeadChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProjectHistoryMutationError {
    #[error("project history operation observed a stale project lifecycle: {0}")]
    StaleProjectLifecycle(Box<str>),
    #[error("project history operation requires recovery: {0}")]
    RecoveryRequired(Box<str>),
    #[error("project history operation conflicted with a resource revision")]
    StaleRevision {
        base_revision: u64,
        current_revision: u64,
    },
    #[error("project history operation addressed the wrong resource")]
    ResourceMismatch {
        requested: Box<str>,
        store: Box<str>,
    },
    #[error("project history operation could not prepare its projection: {0}")]
    Projection(Box<str>),
    #[error("project history operation failed: {0}")]
    History(Box<str>),
}

impl fmt::Display for HistoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTransaction => formatter.write_str("history transaction has no changes"),
            Self::DuplicateResource(resource) => {
                write!(
                    formatter,
                    "history transaction patches resource {resource:?} twice"
                )
            }
            Self::ResourceNotFound(resource) => {
                write!(formatter, "resource {resource:?} not found")
            }
            Self::ResourceKindMismatch {
                resource,
                patch_kind,
            } => write!(
                formatter,
                "resource {resource:?} cannot receive a {patch_kind:?} patch"
            ),
            Self::RevisionConflict {
                resource,
                expected,
                actual,
            } => write!(
                formatter,
                "resource {resource:?} revision conflict: expected {}, found {}",
                expected.get(),
                actual.get()
            ),
            Self::NonMonotonicRevision {
                resource,
                before,
                after,
            } => write!(
                formatter,
                "resource {resource:?} revision must advance from {} to {}, found {}",
                before.get(),
                match before.checked_next() {
                    Ok(next) => next.get(),
                    Err(_) => before.get(),
                },
                after.get()
            ),
            Self::RevisionExhausted { resource, retained } => match resource {
                Some(resource) => {
                    write!(
                        formatter,
                        "resource {resource:?} revision is exhausted at {retained}"
                    )
                }
                None => write!(
                    formatter,
                    "project history revision is exhausted at {retained}"
                ),
            },
            Self::InvalidInverse(resource) => {
                write!(
                    formatter,
                    "resource {resource:?} has an invalid inverse patch"
                )
            }
            #[cfg(test)]
            Self::Patch { resource, source } => {
                write!(formatter, "resource {resource:?} patch failed: {source}")
            }
            Self::NothingToUndo => formatter.write_str("there is no transaction to undo"),
            Self::NothingToRedo => formatter.write_str("there is no transaction to redo"),
            Self::HistoryHeadChanged => {
                formatter.write_str("history head changed during filesystem transaction")
            }
        }
    }
}

impl std::error::Error for HistoryError {}

#[derive(Debug, Clone, Default)]
pub struct ProjectHistory {
    undo: Vec<ProjectHistoryTransaction>,
    redo: Vec<ProjectHistoryTransaction>,
}

impl ProjectHistory {
    pub fn status(&self) -> HistoryStatusDto {
        HistoryStatusDto {
            can_undo: self.can_undo(),
            can_redo: self.can_redo(),
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }

    pub fn reload(&mut self, state: &mut ProjectDocumentState, replacement: ProjectDocumentState) {
        *state = replacement;
        self.clear();
    }

    pub fn record_committed_transaction(&mut self, transaction: ProjectHistoryTransaction) {
        self.undo.push(transaction);
        self.redo.clear();
    }

    pub fn apply_transaction(
        &mut self,
        state: &mut ProjectDocumentState,
        transaction: ProjectHistoryTransaction,
    ) -> Result<(), HistoryError> {
        validate_new_transaction(state, &transaction)?;
        let mut staged = apply_changes(state, transaction.changes.iter(), PatchDirection::Forward)?;
        apply_graph_history(
            &mut staged,
            transaction.graph_change.as_ref(),
            PatchDirection::Forward,
        )?;
        apply_specialized_history(&mut staged, &transaction, PatchDirection::Forward)?;
        *state = staged;
        self.undo.push(transaction);
        self.redo.clear();
        Ok(())
    }

    pub fn undo(
        &mut self,
        state: &mut ProjectDocumentState,
    ) -> Result<ProjectHistoryTransaction, HistoryError> {
        let transaction = self.undo.last().ok_or(HistoryError::NothingToUndo)?;
        let mut staged = apply_changes(
            state,
            transaction.changes.iter().rev(),
            PatchDirection::Inverse,
        )?;
        apply_graph_history(
            &mut staged,
            transaction.graph_change.as_ref(),
            PatchDirection::Inverse,
        )?;
        apply_specialized_history(&mut staged, transaction, PatchDirection::Inverse)?;
        let transaction = self.undo.pop().expect("undo entry checked");
        *state = staged;
        self.redo.push(transaction.clone());
        Ok(transaction)
    }

    pub fn next_undo(&self) -> Option<&ProjectHistoryTransaction> {
        self.undo.last()
    }

    pub fn next_redo(&self) -> Option<&ProjectHistoryTransaction> {
        self.redo.last()
    }

    pub fn move_resource_head(
        &mut self,
        undo: bool,
        expected_history_id: &HistoryEntryId,
    ) -> Result<ProjectHistoryTransaction, HistoryError> {
        let source = if undo { &mut self.undo } else { &mut self.redo };
        let transaction = source.last().ok_or(if undo {
            HistoryError::NothingToUndo
        } else {
            HistoryError::NothingToRedo
        })?;
        if &transaction.history_id != expected_history_id || transaction.resource_move.is_none() {
            return Err(HistoryError::HistoryHeadChanged);
        }
        let transaction = source.pop().expect("history head checked");
        if undo {
            self.redo.push(transaction.clone());
        } else {
            self.undo.push(transaction.clone());
        }
        Ok(transaction)
    }

    pub fn redo(
        &mut self,
        state: &mut ProjectDocumentState,
    ) -> Result<ProjectHistoryTransaction, HistoryError> {
        let transaction = self.redo.last().ok_or(HistoryError::NothingToRedo)?;
        let mut staged = apply_changes(state, transaction.changes.iter(), PatchDirection::Forward)?;
        apply_graph_history(
            &mut staged,
            transaction.graph_change.as_ref(),
            PatchDirection::Forward,
        )?;
        apply_specialized_history(&mut staged, transaction, PatchDirection::Forward)?;
        let transaction = self.redo.pop().expect("redo entry checked");
        *state = staged;
        self.undo.push(transaction.clone());
        Ok(transaction)
    }
}

fn validate_new_transaction(
    state: &ProjectDocumentState,
    transaction: &ProjectHistoryTransaction,
) -> Result<(), HistoryError> {
    let specialized_count = usize::from(transaction.graph_change.is_some())
        + usize::from(transaction.resource_lifecycle.is_some())
        + usize::from(transaction.resource_move.is_some());
    if transaction.changes.is_empty() && specialized_count == 0 {
        return Err(HistoryError::EmptyTransaction);
    }
    if specialized_count > 1 || (specialized_count != 0 && !transaction.changes.is_empty()) {
        return Err(HistoryError::EmptyTransaction);
    }

    let mut resources = BTreeSet::new();
    for change in &transaction.changes {
        if !resources.insert(change.resource.clone()) {
            return Err(HistoryError::DuplicateResource(change.resource.clone()));
        }
        if change.resource.kind() != change.forward.kind() {
            return Err(HistoryError::ResourceKindMismatch {
                resource: change.resource.clone(),
                patch_kind: change.forward.kind(),
            });
        }
        if change.inverse.kind() != change.resource.kind() {
            return Err(HistoryError::ResourceKindMismatch {
                resource: change.resource.clone(),
                patch_kind: change.inverse.kind(),
            });
        }
        if change.inverse != change.forward.inverse() {
            return Err(HistoryError::InvalidInverse(change.resource.clone()));
        }
        let expected_after = change.before_revision.checked_next().map_err(|error| {
            HistoryError::RevisionExhausted {
                resource: Some(change.resource.clone()),
                retained: error.retained,
            }
        })?;
        if change.after_revision != expected_after {
            return Err(HistoryError::NonMonotonicRevision {
                resource: change.resource.clone(),
                before: change.before_revision,
                after: change.after_revision,
            });
        }
        let actual = resource_revision(state, &change.resource)?;
        if actual != change.before_revision {
            return Err(HistoryError::RevisionConflict {
                resource: change.resource.clone(),
                expected: change.before_revision,
                actual,
            });
        }
    }
    if let Some(change) = &transaction.graph_change {
        validate_graph_history_change(state, change)?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum PatchDirection {
    Forward,
    Inverse,
}

fn apply_changes<'a>(
    state: &ProjectDocumentState,
    changes: impl Iterator<Item = &'a ResourcePatch>,
    direction: PatchDirection,
) -> Result<ProjectDocumentState, HistoryError> {
    let mut staged = state.clone();
    for change in changes {
        let patch = match direction {
            PatchDirection::Forward => &change.forward,
            PatchDirection::Inverse => &change.inverse,
        };
        apply_resource_patch(&mut staged, &change.resource, patch)?;
    }
    staged.revision =
        staged
            .revision
            .checked_next()
            .map_err(|error| HistoryError::RevisionExhausted {
                resource: None,
                retained: error.retained,
            })?;
    Ok(staged)
}

fn validate_graph_history_change(
    state: &ProjectDocumentState,
    change: &ProjectGraphHistoryChange,
) -> Result<(), HistoryError> {
    let resource = ResourceKey::Graph(change.graph_path.clone());
    let current = state.graphs.get(&change.graph_path);
    match (&change.before.residency, current) {
        (ProjectGraphResidency::Loaded, Some(document)) => {
            let actual = ResourceRevision::from_graph_revision(document.revision);
            let expected = ResourceRevision::from_graph_revision(change.before.revision);
            if actual != expected {
                return Err(HistoryError::RevisionConflict {
                    resource,
                    expected,
                    actual,
                });
            }
        }
        (ProjectGraphResidency::Unloaded, None) => {}
        (ProjectGraphResidency::Loaded, None) | (ProjectGraphResidency::Unloaded, Some(_)) => {
            return Err(HistoryError::ResourceNotFound(resource));
        }
    }
    let expected_after =
        change
            .before
            .revision
            .checked_next()
            .map_err(|error| HistoryError::RevisionExhausted {
                resource: Some(ResourceKey::Graph(change.graph_path.clone())),
                retained: error.retained,
            })?;
    if change.after.revision != expected_after {
        return Err(HistoryError::NonMonotonicRevision {
            resource: ResourceKey::Graph(change.graph_path.clone()),
            before: ResourceRevision::from_graph_revision(change.before.revision),
            after: ResourceRevision::from_graph_revision(change.after.revision),
        });
    }
    Ok(())
}

fn apply_graph_history(
    state: &mut ProjectDocumentState,
    change: Option<&ProjectGraphHistoryChange>,
    direction: PatchDirection,
) -> Result<(), HistoryError> {
    let Some(change) = change else {
        return Ok(());
    };
    let selected = match direction {
        PatchDirection::Forward => &change.after,
        PatchDirection::Inverse => &change.before,
    };
    match selected.residency {
        ProjectGraphResidency::Loaded => {
            state
                .graphs
                .insert(change.graph_path.clone(), selected.document.clone());
        }
        ProjectGraphResidency::Unloaded => {
            state.graphs.remove(&change.graph_path);
        }
    }
    Ok(())
}

fn apply_specialized_history(
    state: &mut ProjectDocumentState,
    transaction: &ProjectHistoryTransaction,
    direction: PatchDirection,
) -> Result<(), HistoryError> {
    if let Some(lifecycle) = &transaction.resource_lifecycle {
        let ResourceLifecycleHistoryPayload::Worksheet { document } = &lifecycle.payload else {
            return Ok(());
        };
        let patch = match direction {
            PatchDirection::Forward => lifecycle.forward.clone(),
            PatchDirection::Inverse => lifecycle.forward.inverse(),
        };
        let lifecycle_state = patch
            .before
            .as_ref()
            .or(patch.after.as_ref())
            .ok_or(HistoryError::EmptyTransaction)?;
        let key = WorksheetResourceKey(lifecycle_state.path.clone());
        let revision = state
            .worksheet_revisions
            .get(&key)
            .copied()
            .or_else(|| state.worksheets.get(&key).map(|document| document.revision))
            .unwrap_or(lifecycle_state.revision)
            .checked_next()
            .map_err(|error| HistoryError::RevisionExhausted {
                resource: Some(ResourceKey::Worksheet(key.clone())),
                retained: error.retained,
            })?;
        if patch.after.is_some() {
            let mut restored = document.clone();
            restored.revision = revision;
            state.worksheets.insert(key.clone(), restored);
        } else {
            state.worksheets.remove(&key);
        }
        state.worksheet_revisions.insert(key, revision);
    }
    if let Some(resource_move) = &transaction.resource_move {
        let ResourceMoveHistoryPayload::Worksheet { document } = &resource_move.payload else {
            return Ok(());
        };
        let (from, to) = match direction {
            PatchDirection::Forward => (&resource_move.from, &resource_move.to),
            PatchDirection::Inverse => (&resource_move.to, &resource_move.from),
        };
        let from = WorksheetResourceKey(from.clone());
        let to = WorksheetResourceKey(to.clone());
        let revision = state
            .worksheet_revisions
            .get(&from)
            .copied()
            .or_else(|| {
                state
                    .worksheets
                    .get(&from)
                    .map(|document| document.revision)
            })
            .ok_or_else(|| HistoryError::ResourceNotFound(ResourceKey::Worksheet(from.clone())))?
            .checked_next()
            .map_err(|error| HistoryError::RevisionExhausted {
                resource: Some(ResourceKey::Worksheet(from.clone())),
                retained: error.retained,
            })?;
        state.worksheets.remove(&from);
        let mut moved = document.clone();
        moved.revision = revision;
        state.worksheets.insert(to.clone(), moved);
        state.worksheet_revisions.insert(from, revision);
        state.worksheet_revisions.insert(to, revision);
    }
    Ok(())
}

fn apply_resource_patch(
    state: &mut ProjectDocumentState,
    resource: &ResourceKey,
    patch: &ResourceDocumentPatch,
) -> Result<(), HistoryError> {
    if resource.kind() != patch.kind() {
        return Err(HistoryError::ResourceKindMismatch {
            resource: resource.clone(),
            patch_kind: patch.kind(),
        });
    }

    match (resource, patch) {
        #[cfg(all(test, any()))]
        (ResourceKey::Graph(path), ResourceDocumentPatch::Graph(patch)) => state
            .graphs
            .get_mut(path)
            .ok_or_else(|| HistoryError::ResourceNotFound(resource.clone()))?
            .apply_patch(patch)
            .map_err(|retained| HistoryError::RevisionExhausted {
                resource: Some(resource.clone()),
                retained,
            }),
        (ResourceKey::Function(key), ResourceDocumentPatch::Function(patch)) => state
            .functions
            .get_mut(key)
            .ok_or_else(|| HistoryError::ResourceNotFound(resource.clone()))?
            .apply_patch(patch)
            .map_err(|retained| HistoryError::RevisionExhausted {
                resource: Some(resource.clone()),
                retained,
            }),
        (ResourceKey::Variable(key), ResourceDocumentPatch::Variable(patch)) => state
            .variables
            .get_mut(key)
            .ok_or_else(|| HistoryError::ResourceNotFound(resource.clone()))?
            .apply_patch(patch)
            .map_err(|retained| HistoryError::RevisionExhausted {
                resource: Some(resource.clone()),
                retained,
            }),
        (ResourceKey::Worksheet(key), ResourceDocumentPatch::Worksheet(patch)) => {
            let revision = state
                .worksheet_revisions
                .get(key)
                .copied()
                .or_else(|| state.worksheets.get(key).map(|document| document.revision))
                .ok_or_else(|| HistoryError::ResourceNotFound(resource.clone()))?
                .checked_next()
                .map_err(|error| HistoryError::RevisionExhausted {
                    resource: Some(resource.clone()),
                    retained: error.retained,
                })?;
            let document = state
                .worksheets
                .get_mut(key)
                .ok_or_else(|| HistoryError::ResourceNotFound(resource.clone()))?;
            document.database_id = patch.after.database_id.clone();
            document.chart_type = patch.after.chart_type.clone();
            document.encodings = patch.after.encodings.clone();
            document.revision = revision;
            state.worksheet_revisions.insert(key.clone(), revision);
            Ok(())
        }
        _ => Err(HistoryError::ResourceKindMismatch {
            resource: resource.clone(),
            patch_kind: patch.kind(),
        }),
    }
}

fn resource_revision(
    state: &ProjectDocumentState,
    resource: &ResourceKey,
) -> Result<ResourceRevision, HistoryError> {
    match resource {
        ResourceKey::Graph(path) => state
            .graphs
            .get(path)
            .map(|document| ResourceRevision::from_graph_revision(document.revision))
            .ok_or_else(|| HistoryError::ResourceNotFound(resource.clone())),
        ResourceKey::Function(key) => state
            .functions
            .get(key)
            .map(|document| document.revision)
            .ok_or_else(|| HistoryError::ResourceNotFound(resource.clone())),
        ResourceKey::Variable(key) => state
            .variables
            .get(key)
            .map(|document| document.revision)
            .ok_or_else(|| HistoryError::ResourceNotFound(resource.clone())),
        ResourceKey::Worksheet(key) => state
            .worksheet_revisions
            .get(key)
            .copied()
            .or_else(|| state.worksheets.get(key).map(|document| document.revision))
            .ok_or_else(|| HistoryError::ResourceNotFound(resource.clone())),
        ResourceKey::Database(_) => Err(HistoryError::ResourceNotFound(resource.clone())),
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectGraphResidency {
    Loaded,
    Unloaded,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectGraphHistoryState {
    pub document: GraphDocument,
    pub revision: GraphRevision,
    pub residency: ProjectGraphResidency,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ProjectGraphHistoryChange {
    pub graph_path: GraphResourcePath,
    pub before: ProjectGraphHistoryState,
    pub after: ProjectGraphHistoryState,
}

impl ProjectGraphHistoryChange {
    pub fn inverse(&self) -> Self {
        Self {
            graph_path: self.graph_path.clone(),
            before: self.after.clone(),
            after: self.before.clone(),
        }
    }
}

#[cfg(test)]
mod wire_tests {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;
    use yss_graph_document::GraphResourcePath;

    #[test]
    fn resource_delta_wire_is_camel_case() {
        let caused_by = OperationId::from_uuid(Uuid::from_u128(905));
        let delta = ResourceDeltaEvent {
            resource: ResourceKey::Graph(
                GraphResourcePath::new("events/Main.yssbi-event").unwrap(),
            ),
            from_revision: ResourceRevision::new(4),
            to_revision: ResourceRevision::new(5),
            caused_by: Some(caused_by),
            payload: ResourceDocumentPatch::Graph(GraphDocumentPatch::new([])),
        };

        assert_eq!(
            serde_json::to_value(delta).unwrap(),
            json!({
                "resource": {
                    "kind": "graph",
                    "key": "events/Main.yssbi-event"
                },
                "fromRevision": 4,
                "toRevision": 5,
                "causedBy": "00000000-0000-0000-0000-000000000389",
                "payload": {
                    "kind": "graph",
                    "patch": { "operations": [] }
                }
            })
        );
    }
}
