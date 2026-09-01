use crate::GraphResourceDocument;
use std::collections::{BTreeMap, BTreeSet};
use yss_chart_document::{ChartDocument, ChartResourcePath};
use yss_graph_document::GraphResourcePath;
use yss_project_identity::ResourceRevision;
use yss_variable_contract::{VariableId, VariableInstance};

/// An atomic candidate change to the in-memory [`crate::ProjectData`] aggregate.
///
/// This is deliberately distinct from
/// [`yss_project_history::ResourceDocumentPatch`], which describes persisted
/// history payloads rather than the complete state transition being committed.
#[derive(Clone, Debug)]
pub enum ProjectDataPatch {
    InsertGraph {
        path: GraphResourcePath,
        resource: GraphResourceDocument,
    },
    /// Publish an on-disk graph declaration without installing a resident document.
    DeclareGraph {
        path: GraphResourcePath,
        revision: ResourceRevision,
    },
    RemoveGraph {
        path: GraphResourcePath,
        revision: ResourceRevision,
    },
    UnloadGraph {
        path: GraphResourcePath,
    },
    MoveGraph {
        from: GraphResourcePath,
        to: GraphResourcePath,
        moved_before: Box<GraphResourceDocument>,
        moved: GraphResourceDocument,
        referenced_graphs_before: BTreeMap<GraphResourcePath, GraphResourceDocument>,
        referenced_graphs: BTreeMap<GraphResourcePath, GraphResourceDocument>,
        loaded_referenced_graphs: BTreeSet<GraphResourcePath>,
        referenced_variables_before: BTreeMap<VariableId, VariableInstance>,
        referenced_variables: BTreeMap<VariableId, VariableInstance>,
    },
    PatchVariables {
        updates: BTreeMap<VariableId, VariableInstance>,
        removals: BTreeSet<VariableId>,
    },
    UpsertChart {
        path: ChartResourcePath,
        document: ChartDocument,
    },
    RemoveChart {
        path: ChartResourcePath,
        revision: ResourceRevision,
    },
    MoveChart {
        from: ChartResourcePath,
        to: ChartResourcePath,
        moved: ChartDocument,
    },
}
