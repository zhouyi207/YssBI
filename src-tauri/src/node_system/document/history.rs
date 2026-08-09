use super::{
    DocumentError, FunctionParameterId, GraphDocument, GraphDocumentPatch, GraphResourcePath,
    GraphRevision, HistoryEntryId, OperationId, ProjectRevision, ResourceRevision,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

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

    fn apply_patch(&mut self, patch: &FunctionDocumentPatch) {
        self.signature = patch.after.clone();
        self.revision.advance();
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

    fn apply_patch(&mut self, patch: &VariableDocumentPatch) {
        self.value = patch.after.clone();
        self.revision.advance();
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
pub enum GraphResourceLifecycleKind {
    Event,
    Function,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphResourceLifecycleState {
    pub revision: ResourceRevision,
    pub path: Box<str>,
    pub kind: GraphResourceLifecycleKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphResourceLifecyclePatch {
    pub before: Option<GraphResourceLifecycleState>,
    pub after: Option<GraphResourceLifecycleState>,
}

impl GraphResourceLifecyclePatch {
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DatabaseDocumentPatch {
    pub before: Option<crate::database::DatabaseDecl>,
    pub after: Option<crate::database::DatabaseDecl>,
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
    Graph(GraphDocumentPatch),
    GraphResourceLifecycle(GraphResourceLifecyclePatch),
    GraphResourceMove(ResourcePathMovePatch),
    Function(FunctionDocumentPatch),
    Variable(VariableDocumentPatch),
    VariableScopeMove(ResourcePathMovePatch),
    Database(DatabaseDocumentPatch),
}

impl ResourceDocumentPatch {
    pub const fn kind(&self) -> ResourceKind {
        match self {
            Self::Graph(_) | Self::GraphResourceLifecycle(_) | Self::GraphResourceMove(_) => {
                ResourceKind::Graph
            }
            Self::Function(_) => ResourceKind::Function,
            Self::Variable(_) | Self::VariableScopeMove(_) => ResourceKind::Variable,
            Self::Database(_) => ResourceKind::Database,
        }
    }

    pub fn inverse(&self) -> Self {
        match self {
            Self::Graph(patch) => Self::Graph(patch.inverse()),
            Self::GraphResourceLifecycle(patch) => Self::GraphResourceLifecycle(patch.inverse()),
            Self::GraphResourceMove(patch) => Self::GraphResourceMove(patch.inverse()),
            Self::Function(patch) => Self::Function(patch.inverse()),
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

impl ResourcePatch {
    pub fn graph(
        graph_path: GraphResourcePath,
        before_revision: GraphRevision,
        forward: GraphDocumentPatch,
    ) -> Self {
        let inverse = forward.inverse();
        Self {
            resource: ResourceKey::Graph(graph_path),
            before_revision,
            after_revision: before_revision.next(),
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
            after_revision: before_revision.next(),
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
            after_revision: before_revision.next(),
            forward: ResourceDocumentPatch::Variable(forward),
            inverse: ResourceDocumentPatch::Variable(inverse),
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphResourceMoveHistoryPatch {
    pub from: GraphResourcePath,
    pub to: GraphResourcePath,
    pub payload: Value,
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
    pub persistence: HistoryPersistencePolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variable_effect_snapshots: Option<VariableEffectHistorySnapshots>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_resource_move: Option<GraphResourceMoveHistoryPatch>,
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
            persistence: HistoryPersistencePolicy::InMemoryUntilSave,
            variable_effect_snapshots: None,
            graph_resource_move: None,
        }
    }

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

    pub fn durable_variable_effects(
        caused_by: OperationId,
        changes: impl Into<Vec<ResourcePatch>>,
        snapshots: VariableEffectHistorySnapshots,
    ) -> Self {
        Self {
            history_id: HistoryEntryId::new(),
            caused_by,
            changes: changes.into(),
            persistence: HistoryPersistencePolicy::DurableVariableEffects,
            variable_effect_snapshots: Some(snapshots),
            graph_resource_move: None,
        }
    }

    pub fn graph_resource_move(
        caused_by: OperationId,
        from: GraphResourcePath,
        to: GraphResourcePath,
        payload: Value,
    ) -> Self {
        Self {
            history_id: HistoryEntryId::new(),
            caused_by,
            changes: Vec::new(),
            persistence: HistoryPersistencePolicy::DurableResourceMove,
            variable_effect_snapshots: None,
            graph_resource_move: Some(GraphResourceMoveHistoryPatch { from, to, payload }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectDocumentState {
    pub revision: ProjectRevision,
    pub graphs: BTreeMap<GraphResourcePath, GraphDocument>,
    pub functions: BTreeMap<FunctionResourceKey, FunctionDocument>,
    pub variables: BTreeMap<VariableResourceKey, VariableDocument>,
}

impl Default for ProjectDocumentState {
    fn default() -> Self {
        Self {
            revision: ProjectRevision::INITIAL,
            graphs: BTreeMap::new(),
            functions: BTreeMap::new(),
            variables: BTreeMap::new(),
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
    InvalidInverse(ResourceKey),
    Patch {
        resource: ResourceKey,
        source: DocumentError,
    },
    NothingToUndo,
    NothingToRedo,
    HistoryHeadChanged,
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
                before.next().get(),
                after.get()
            ),
            Self::InvalidInverse(resource) => {
                write!(
                    formatter,
                    "resource {resource:?} has an invalid inverse patch"
                )
            }
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

    #[cfg(test)]
    pub(crate) fn move_undo_head_to_redo_for_test(&mut self) {
        let transaction = self.undo.pop().expect("test History has an undo head");
        self.redo.push(transaction);
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
        let staged = apply_changes(state, transaction.changes.iter(), PatchDirection::Forward)?;
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
        let staged = apply_changes(
            state,
            transaction.changes.iter().rev(),
            PatchDirection::Inverse,
        )?;
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

    pub fn move_graph_resource_head(
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
        if &transaction.history_id != expected_history_id
            || transaction.graph_resource_move.is_none()
        {
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
        let staged = apply_changes(state, transaction.changes.iter(), PatchDirection::Forward)?;
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
    if transaction.changes.is_empty() && transaction.graph_resource_move.is_none() {
        return Err(HistoryError::EmptyTransaction);
    }
    if transaction.graph_resource_move.is_some() && !transaction.changes.is_empty() {
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
        if change.after_revision != change.before_revision.next() {
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
    staged.revision.advance();
    Ok(staged)
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
        (ResourceKey::Graph(path), ResourceDocumentPatch::Graph(patch)) => state
            .graphs
            .get_mut(path)
            .ok_or_else(|| HistoryError::ResourceNotFound(resource.clone()))?
            .apply_patch(patch)
            .map_err(|source| HistoryError::Patch {
                resource: resource.clone(),
                source,
            }),
        (ResourceKey::Function(key), ResourceDocumentPatch::Function(patch)) => {
            state
                .functions
                .get_mut(key)
                .ok_or_else(|| HistoryError::ResourceNotFound(resource.clone()))?
                .apply_patch(patch);
            Ok(())
        }
        (ResourceKey::Variable(key), ResourceDocumentPatch::Variable(patch)) => {
            state
                .variables
                .get_mut(key)
                .ok_or_else(|| HistoryError::ResourceNotFound(resource.clone()))?
                .apply_patch(patch);
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
            .map(|document| document.revision)
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
        ResourceKey::Database(_) | ResourceKey::Worksheet(_) => {
            Err(HistoryError::ResourceNotFound(resource.clone()))
        }
    }
}

#[cfg(test)]
mod wire_tests {
    use super::*;
    use crate::node_system::document::{GraphDocumentPatch, GraphResourcePath};
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn resource_delta_wire_is_camel_case() {
        let caused_by = OperationId::from_uuid(Uuid::from_u128(905));
        let delta = ResourceDeltaEvent {
            resource: ResourceKey::Graph(GraphResourcePath("events/Main.yssbi-event".into())),
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
