use super::types::{
    CompilationOutcomeDto, DiagnosticDto, EditorConnectionProjectionDto, EditorGraphProjectionDto,
    EditorNodeProjectionDto, ProjectionBasis,
};
use crate::graph_document::GraphRevision;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A revision transition containing complete node replacements. Port additions and
/// removals are intentionally not represented as independently applicable fragments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphProjectionDelta {
    pub from_basis: ProjectionBasis,
    pub to_basis: ProjectionBasis,
    pub removed_node_ids: Vec<Box<str>>,
    pub node_replacements: Vec<EditorNodeProjectionDto>,
    pub connections: Vec<EditorConnectionProjectionDto>,
    pub diagnostics: Vec<DiagnosticDto>,
    pub outcome: CompilationOutcomeDto,
    pub has_blocking_diagnostics: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectionError {
    RevisionMismatch {
        analysis: GraphRevision,
        document: GraphRevision,
    },
    RegistryMismatch,
    StaleProjectionBasis,
    IncompatibleProjectionGraphs,
    InvalidDelta,
}

impl std::fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RevisionMismatch { analysis, document } => write!(
                formatter,
                "analysis revision {} does not match document revision {}",
                analysis.get(),
                document.get()
            ),
            Self::RegistryMismatch => {
                formatter.write_str("analysis registry fingerprint does not match registry")
            }
            Self::StaleProjectionBasis => {
                formatter.write_str("projection delta does not start at the current basis")
            }
            Self::IncompatibleProjectionGraphs => {
                formatter.write_str("projection snapshots belong to different graphs")
            }
            Self::InvalidDelta => {
                formatter.write_str("projection delta is internally inconsistent")
            }
        }
    }
}

impl std::error::Error for ProjectionError {}

impl EditorGraphProjectionDto {
    /// Applies a complete revision transition only when its old basis exactly
    /// matches this projection. Validation and replacement happen before commit.
    pub fn apply_delta(&mut self, delta: GraphProjectionDelta) -> Result<(), ProjectionError> {
        if self.basis != delta.from_basis {
            return Err(ProjectionError::StaleProjectionBasis);
        }
        validate_delta(&delta)?;

        let removed = delta.removed_node_ids.iter().collect::<BTreeSet<_>>();
        let replacements = delta
            .node_replacements
            .iter()
            .map(|node| (node.node_id.as_ref(), node))
            .collect::<BTreeMap<_, _>>();
        let mut next_nodes = self
            .nodes
            .iter()
            .filter(|node| !removed.contains(&node.node_id))
            .filter(|node| !replacements.contains_key(node.node_id.as_ref()))
            .cloned()
            .collect::<Vec<_>>();
        next_nodes.extend(delta.node_replacements.iter().cloned());
        next_nodes.sort_by(|left, right| left.node_id.cmp(&right.node_id));

        self.basis = delta.to_basis;
        self.graph_path = self.basis.graph_path.clone();
        self.source_revision = self.basis.graph_revision;
        self.nodes = next_nodes;
        self.connections = delta.connections;
        self.diagnostics = delta.diagnostics;
        self.outcome = delta.outcome;
        self.has_blocking_diagnostics = delta.has_blocking_diagnostics;
        Ok(())
    }
}

impl GraphProjectionDelta {
    pub fn between(
        previous: &EditorGraphProjectionDto,
        next: &EditorGraphProjectionDto,
    ) -> Result<Self, ProjectionError> {
        if previous.basis.graph_path != next.basis.graph_path {
            return Err(ProjectionError::IncompatibleProjectionGraphs);
        }
        let previous_nodes = previous
            .nodes
            .iter()
            .map(|node| (node.node_id.as_ref(), node))
            .collect::<BTreeMap<_, _>>();
        let next_nodes = next
            .nodes
            .iter()
            .map(|node| (node.node_id.as_ref(), node))
            .collect::<BTreeMap<_, _>>();
        let removed_node_ids = previous_nodes
            .keys()
            .filter(|node_id| !next_nodes.contains_key(**node_id))
            .map(|node_id| Box::<str>::from(*node_id))
            .collect();
        let node_replacements = next
            .nodes
            .iter()
            .filter(|node| previous_nodes.get(node.node_id.as_ref()).copied() != Some(*node))
            .cloned()
            .collect();

        Ok(Self {
            from_basis: previous.basis.clone(),
            to_basis: next.basis.clone(),
            removed_node_ids,
            node_replacements,
            connections: next.connections.clone(),
            diagnostics: next.diagnostics.clone(),
            outcome: next.outcome.clone(),
            has_blocking_diagnostics: next.has_blocking_diagnostics,
        })
    }
}
fn validate_delta(delta: &GraphProjectionDelta) -> Result<(), ProjectionError> {
    if delta.from_basis.graph_path != delta.to_basis.graph_path
        || delta.to_basis.graph_revision < delta.from_basis.graph_revision
        || matches!(delta.outcome, CompilationOutcomeDto::Success) == delta.has_blocking_diagnostics
        || delta.node_replacements.iter().any(|node| {
            node.graph_path != delta.to_basis.graph_path
                || node.source_revision != delta.to_basis.graph_revision
        })
    {
        return Err(ProjectionError::InvalidDelta);
    }
    let mut identities = BTreeSet::new();
    if delta
        .node_replacements
        .iter()
        .any(|node| !identities.insert(node.node_id.as_ref()))
        || delta
            .removed_node_ids
            .iter()
            .any(|node_id| identities.contains(node_id.as_ref()))
    {
        return Err(ProjectionError::InvalidDelta);
    }
    Ok(())
}
