use crate::GraphResourceDocument;
use std::collections::{BTreeMap, BTreeSet};
use yss_chart_document::{ChartDocument, ChartResourcePath};
use yss_graph_document::GraphResourcePath;
use yss_project_identity::ResourceRevision;

/// An atomic candidate change to the in-memory [`crate::ProjectData`] aggregate.
///
/// This is deliberately distinct from
/// [`yss_project_history::ResourceDocumentPatch`], which describes published
/// resource deltas rather than the complete state transition being committed.
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
    MoveGraph {
        from: GraphResourcePath,
        to: GraphResourcePath,
        moved: GraphResourceDocument,
        referenced_graphs: BTreeMap<GraphResourcePath, GraphResourceDocument>,
        loaded_referenced_graphs: BTreeSet<GraphResourcePath>,
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
